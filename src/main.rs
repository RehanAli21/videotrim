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

#[derive(Serialize, Deserialize, JsonSchema, Debug)]
struct EditCommand {
    start: i64,
    end: i64,
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
        Ok(())
    } else {
        Err("FFmpeg failed".to_string())
    }
}

fn read_audio(path: &str) -> Vec<f32> {
    let mut reader = WavReader::open(path).expect("Failed to open WAV file");

    reader
        .samples::<i16>() // read raw samples as i16 (16-bit integers)
        .map(|s| s.unwrap() as f32 / 32768.0) // convert to f32 between -1.0 and 1.0
        .collect()
}

#[tokio::main]
async fn main() {
    let args = Args::parse();

    println!("input path: {}", args.input);
    println!("output path: {}", args.output);

    let response = extract_audio_from_video(&args.input, &args.output);

    println!("{:#?}", response);

    /*let api = match ApiBuilder::new().build() {
        Ok(api) => api,
        Err(err) => panic!("error in building api: {err}"),
    };

    let repo = api.model("ggerganov/whisper-small.en".to_string());

    // Asynchronously fetch the file
    let model_path = match repo.get("ggml-small.en.bin").await {
        Ok(path) => path,
        Err(err) => panic!("Can't get whisper model path, err : {err}"),
    };*/

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

    let audio_data = read_audio(&args.output);

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
        //println!(
        //   "[{} - {}]: {}",
        // note start and end timestamps are in centiseconds
        // (10s of milliseconds)
        //   segment.start_timestamp(),
        //   segment.end_timestamp(),
        //     the Display impl for WhisperSegment will replace invalid UTF-8 with the Unicode replacement character
        //  segment
        //);
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

    /*let prompt = format!(
        "You are a video editing assistant. Follow the user's instruction below as your primary task.\n\n\
         USER INSTRUCTION (follow this above all else):\n{}\n\n\
         The transcript below has timestamps in SECONDS, one line per segment as: [start - end] text\n\n\
         STEP 1 — In the \"reasoning\" field, go segment by segment and note the time ranges that match \
         the user instruction. State where each matching topic STARTS and ENDS using the timestamps.\n\n\
         STEP 2 — In the \"edits\" array, output the ranges to REMOVE. If a topic spans several consecutive \
         lines, MERGE them into ONE edit whose cut_from is the first line's start and cut_to is the last line's end.\n\n\
         Each edit: cut_from (seconds, number), cut_to (seconds, number), reason (string).\n\n\
         Transcript:\n{}",
        args.user_instructions,
        transcript
    );
        let prompt = format!(
        "You are a video editor. Below is a transcript with timestamps in seconds.\n\
         Identify segments to CUT: filler words (um, uh, like), long silences, \
         repeated sentences, false starts, and off-topic rambling.\n\n\
         Return a JSON object with an \"edits\" array. Each edit has:\n\
         - \"cut_from\": start time in seconds (number)\n\
         - \"cut_to\": end time in seconds (number)\n\
         - \"reason\": why it should be cut (string)\n\n\
         Example: {{\"edits\": [{{\"cut_from\": 2.5, \"cut_to\": 4.0, \"reason\": \"filler word um\"}}]}}\n\n\
         Instructions from superwiser that you have to follow:\n{}\n
         Transcript:\n{}",
         &args.user_instructions,
        transcript
        );*/

    println!("{}", prompt);

    //let format = FormatType::StructuredJson(Box::new(JsonStructure::new::<EditPlan>()));

    //let options = ModelOptions::default().temperature(0.0);
    //let request = GenerationRequest::new(model, prompt)
    //  .format(format)
    //   .options(options);

    //let res = ollama.generate(request).await;

    //let response = match res {
    // Ok(r) => r.response,
    //  Err(err) => panic!("Err in getting response. err => {}", err),
    //};

    //let plan: EditPlan = match serde_json::from_str(&response) {
    //   Ok(json) => json,
    //    Err(err) => panic!("Err on converting to json. err => {}", err),
    //};

    //println!("{:#?}", plan);

    //let prompt = format!("You are a video editor. Tell me what is best for videos");

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
}
