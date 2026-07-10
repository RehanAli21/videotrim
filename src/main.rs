use clap::Parser;
use hound::WavReader;
use ollama_rs::{
    generation::{
        completion::request::GenerationRequest,
        parameters::{FormatType, JsonStructure},
    },
    models::ModelOptions,
    Ollama,
};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use std::fs;
use std::io::Write;
use std::path::PathBuf;
use std::process::Command;
use whisper_rs::{FullParams, SamplingStrategy, WhisperContext, WhisperContextParameters};

#[derive(Serialize, Deserialize)]
struct Segment {
    start: f64,
    end: f64,
    text: String,
}

#[derive(Serialize, Deserialize, JsonSchema, Debug, Clone)]
struct EditCommand {
    start: f64,
    end: f64,
    text: String,
}

#[derive(Serialize, Deserialize, JsonSchema, Debug)]
struct EditPlan {
    reasoning: String,
    edits: Vec<EditCommand>,
}

async fn download_model(model_name: &str) -> PathBuf {
    let model_dir = dirs::cache_dir().unwrap().join("rust-video-editor");

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

#[derive(Parser, Debug)]
#[command(version, about, long_about = None)]
struct Args {
    /// Input file path like myvideo.mp4
    #[arg(short, long)]
    input: String,

    /// output path to save file like ./output/editied_video.mp4
    #[arg(short, long)]
    output: String,

    /// instuctions from user
    #[arg(short, long)]
    user_instructions: String,
}

fn extract_audio_from_video(input: &str, output: &str) -> Result<(), String> {
    let status = Command::new("ffmpeg")
        .args([
            "-i",
            input,
            "-vn",
            "-acodec",
            "pcm_s16le",
            "-ar",
            "16000",
            "-ac",
            "1",
            output,
        ])
        .status()
        .map_err(|e| format!("Failed to run ffmpeg: {}", e))?;

    if status.success() {
        println!("{:#?}", status);
        Ok(())
    } else {
        Err("FFmpeg failed".to_string())
    }
}

fn read_audio(path: &str) -> (Vec<f32>, f64) {
    let mut reader = WavReader::open(path).expect("Failed to open WAV file");
    let spec = reader.spec();

    let samples: Vec<f32> = reader
        .samples::<i16>() // read raw samples as i16 (16-bit integers)
        .map(|s| s.unwrap() as f32 / 32768.0) // convert to f32 between -1.0 and 1.0
        .collect();

    // duration formula = number of samples / (sample rate * channels)
    let duration = samples.len() as f64 / (spec.sample_rate as f64 * spec.channels as f64);

    (samples, duration)
}

#[tokio::main]
async fn main() {
    const AUDIOFILE: &str = "audio.wav";
    let args = Args::parse();

    println!("input path: {}", args.input);
    println!("output path: {}", args.output);

    let response = extract_audio_from_video(&args.input, AUDIOFILE);

    println!("{:#?}", response);

    let model_path = download_model("small.en").await;

    println!("Async model downloaded to: {:?}", model_path);

    let mut whisper_context_parameters = WhisperContextParameters::default();
    whisper_context_parameters.use_gpu(true);

    // load a context and model
    let ctx = WhisperContext::new_with_params(model_path, whisper_context_parameters)
        .expect("failed to load model");

    // create a params object
    let mut params = FullParams::new(SamplingStrategy::BeamSearch {
        beam_size: 5,
        patience: -1.0,
    });

    // Language — defaults to "en" already, but explicit is better
    params.set_language(Some("en"));

    // Timestamps — critical for your project
    params.set_token_timestamps(true);

    // Don't print whisper's internal progress to terminal
    params.set_print_progress(false);
    params.set_print_realtime(false);

    // Suppress blank outputs
    params.set_suppress_blank(true);

    // Temperature — 0.0 is most deterministic (default is already 0.0)
    params.set_temperature(0.0);

    let (audio_data, total_duration) = read_audio(AUDIOFILE);

    // now we can run the model
    let mut state = ctx.create_state().expect("failed to create state");
    state
        .full(params, &audio_data[..])
        .expect("failed to run model");

    let mut segments: Vec<Segment> = vec![];

    //  fetch the results
    for segment in state.as_iter() {
        segments.push(Segment {
            start: segment.start_timestamp() as f64 / 100.0,
            end: segment.end_timestamp() as f64 / 100.0,
            text: segment.to_string(),
        });
    }

    let json = serde_json::to_string_pretty(&segments).unwrap();
    fs::write("transcript.json", &json).unwrap();

    let transcript = json;

    //println!("{}", transcript);
    let ollama = Ollama::default();

    let model = "qwen2.5:7b".to_string();

    let prompt = format!(
        "You are a video editor. Below is a transcript with timestamps in seconds.\n\
        Identify segments to CUT: filler words (um, uh, like), long silences, \
        repeated sentences, false starts, and off-topic rambling.\n\n\
        Additional instructions from the user:\n{}\n\n\
        Return a JSON object with an \"edits\" array. Each edit has:\n\
        - \"cut_from\": start time in seconds (number)\n\
        - \"cut_to\": end time in seconds (number)\n\
        - \"reason\": why it should be cut (string)\n\n\
        Example: {{\"edits\": [{{\"cut_from\": 2.5, \"cut_to\": 4.0, \"reason\": \"filler word um\"}}]}}\n\n\
        Transcript:\n{}",
        &args.user_instructions,
        transcript
    );

    println!("{}", prompt);

    let format = FormatType::StructuredJson(Box::new(JsonStructure::new::<EditPlan>()));

    let options = ModelOptions::default().temperature(0.0);
    let request = GenerationRequest::new(model, prompt)
        .format(format)
        .options(options);

    let res = ollama.generate(request).await;

    let response = match res {
        Ok(r) => r.response,
        Err(err) => panic!("Err in getting response. err => {}", err),
    };

    let plan: EditPlan = match serde_json::from_str(&response) {
        Ok(json) => json,
        Err(err) => panic!("Err on converting to json. err => {}", err),
    };

    println!("{:#?}", plan);

    let (parts_to_keep, parts_to_cut) = cuts_and_keeps(&plan.edits, total_duration);

    let _ = cut_video(&args.input, &parts_to_keep, &parts_to_cut, &args.output);
}

type CutsAndKeepsType = (Vec<(f64, f64)>, Vec<(f64, f64)>);

fn cuts_and_keeps(cuts: &[EditCommand], total_duration: f64) -> CutsAndKeepsType {
    let mut parts_to_keep: Vec<(f64, f64)> = vec![];
    let mut parts_to_cut: Vec<(f64, f64)> = vec![];

    let mut cursor: f64 = 0.0;

    // sort cuts by start time first (LLM may return them out of order)
    let mut sorted = cuts.to_vec();
    sorted.sort_by(|a, b| a.start.partial_cmp(&b.start).unwrap());

    for cut in sorted {
        parts_to_cut.push((cut.start, cut.end));

        if cut.start > cursor {
            parts_to_keep.push((cursor, cut.start))
        }

        cursor = cursor.max(cut.end);
    }

    if cursor < total_duration {
        parts_to_keep.push((cursor, total_duration));
    }

    (parts_to_keep, parts_to_cut)
}

fn cut_video(
    input: &str,
    parts_to_keep: &[(f64, f64)],
    parts_to_cut: &[(f64, f64)],
    output: &str,
) -> Result<(), String> {
    let parts_to_keep_dir = "used_clips";
    let parts_to_cut_dir = "removed_clips";

    let _ = fs::create_dir_all(parts_to_keep_dir).map_err(|e| e.to_string());

    let _ = fs::create_dir_all(parts_to_cut_dir).map_err(|e| e.to_string());

    for (i, (start, end)) in parts_to_cut.iter().enumerate() {
        let clip = format!("{}/clip_{:03}.mp4", parts_to_cut_dir, i);
        let duration = end - start;

        let status = Command::new("ffmpeg")
            .args([
                "-ss",
                &start.to_string(), // seek to start
                "-i",
                input,
                "-t",
                &duration.to_string(), // keep this many seconds
                "-c",
                "copy", // stream copy = no re-encode (fast)
                "-y",   // overwrite if exists
                &clip,
            ])
            .status()
            .map_err(|err| err.to_string())?;

        if !status.success() {
            return Err(format!("Failed to extract clip {}", i));
        }
    }

    let mut keep_clip_paths = vec![];

    // saving each keep clip using it's range
    for (i, (start, end)) in parts_to_keep.iter().enumerate() {
        let clip = format!("{}/clip_{:03}.mp4", parts_to_keep_dir, i);
        let duration = end - start;

        let status = Command::new("ffmpeg")
            .args([
                "-ss",
                &start.to_string(), // seek to start
                "-i",
                input,
                "-t",
                &duration.to_string(), // keep this many seconds
                "-c",
                "copy", // stream copy = no re-encode (fast)
                "-y",   // overwrite if exists
                &clip,
            ])
            .status()
            .map_err(|err| err.to_string())?;

        if !status.success() {
            return Err(format!("Failed to extract clip {}", i));
        }

        keep_clip_paths.push(clip);
    }

    // write a list for concating (ffmpeg needs this format)
    let list_path = format!("{}/list.txt", parts_to_keep_dir);
    let list_content: String = keep_clip_paths
        .iter()
        .map(|p| format!("file '{}'\n", p.replace(parts_to_keep_dir, ".")))
        .collect();

    fs::write(&list_path, list_content).map_err(|e| e.to_string())?;

    //concat all clips into on video
    let status = Command::new("ffmpeg")
        .args([
            "-f", "concat", "-safe", "0", "-i", &list_path, "-c", "copy", "-y", output,
        ])
        .status()
        .map_err(|e| e.to_string())?;

    if !status.success() {
        return Err("Failed to join clips".to_string());
    }

    Ok(())
}
