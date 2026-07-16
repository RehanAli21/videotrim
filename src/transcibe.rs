use crate::types::Segment;

use std::fs;
use std::io::Write;
use std::path::PathBuf;
use std::result::Result;

use whisper_rs::install_logging_hooks;
use whisper_rs::{FullParams, SamplingStrategy, WhisperContext, WhisperContextParameters};

pub struct Transcribe {
    ctx: WhisperContext,
}

impl Transcribe {
    async fn download_model(model_name: &str) -> PathBuf {
        let model_dir = dirs::cache_dir()
            .unwrap()
            .join("videotrim_rust_cli_whisper_model");

        fs::create_dir_all(&model_dir).unwrap();

        let model_path = model_dir.join(format!("ggml-{}.bin", model_name));

        // if already downloaded, skip
        if model_path.exists() {
            println!("Model already exists, skipping download...");
            return model_path;
        }

        let url = format!(
            "https://huggingface.co/ggerganov/whisper.cpp/resolve/main/ggml-{}.bin",
            model_name
        );

        println!("Downloading model from: {}", url);

        let mut response = reqwest::get(&url).await.unwrap();
        let mut file = fs::File::create(&model_path).unwrap();

        while let Some(chunk) = response.chunk().await.unwrap() {
            file.write_all(&chunk).unwrap();
        }

        println!("Model downloaded to: {:?}", model_path);
        model_path
    }

    pub async fn new(model_name: &str) -> Result<Self, String> {
        //let model_path = Transcribe::download_model("small.en").await;
        let model_path = Self::download_model(model_name).await;

        println!("Async model downloaded to: {:?}", model_path);

        install_logging_hooks();

        let mut whisper_context_parameters = WhisperContextParameters::default();
        whisper_context_parameters.use_gpu(true);

        // load a context and model
        let ctx = match WhisperContext::new_with_params(model_path, whisper_context_parameters) {
            Ok(context) => context,
            Err(err) => return Err(format!("failed to load model, error => {}", err)),
        };

        Ok(Self { ctx })
    }

    pub fn transcribe(&self, audio_data: Vec<f32>) -> Result<String, String> {
        // create a params object
        let mut params = FullParams::new(SamplingStrategy::BeamSearch {
            beam_size: 5,
            patience: -1.0,
        });
        // Language — defaults to "en" already, but explicit is better
        params.set_language(Some("en"));
        // Timestamps — critical for project as we need timestaps
        params.set_token_timestamps(false);
        // Don't print whisper's internal progress to terminal
        params.set_print_progress(false);
        params.set_print_realtime(false);
        // Suppress blank outputs
        params.set_suppress_blank(true);
        // Temperature — 0.0 is most deterministic (default is already 0.0)
        params.set_temperature(0.0);

        // now we can run the model
        let mut state = match self.ctx.create_state() {
            Ok(state) => state,
            Err(err) => return Err(format!("failed to create state in whisper, err => {}", err)),
        };

        match state.full(params, &audio_data[..]) {
            Ok(_) => (),
            Err(err) => return Err(format!("failed to run whisper model, err => {}", err)),
        };

        let mut segments: Vec<Segment> = vec![];

        //  fetch the results
        for segment in state.as_iter() {
            segments.push(Segment {
                start: segment.start_timestamp() as f64 / 100.0,
                end: segment.end_timestamp() as f64 / 100.0,
                text: segment.to_string(),
            });
        }

        let json = match serde_json::to_string_pretty(&segments) {
            Ok(string) => string,
            Err(err) => {
                return Err(format!(
                    "failed to convert segments in json string, err => {}",
                    err
                ))
            }
        };

        Ok(json)
    }
}
