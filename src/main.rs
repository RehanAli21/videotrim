use clap::Parser;
use hf_hub::api::tokio::ApiBuilder;
use std::path::PathBuf;
use std::process::Command;

#[derive(Parser, Debug)]
#[command(version, about, long_about = None)]
struct Args {
    /// Input file path like myvideo.mp4
    #[arg(short, long)]
    input: String,

    /// output path to save file like ./output/editied_video.mp4
    #[arg(short, long)]
    output: String,
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

#[tokio::main]
async fn main() {
    let args = Args::parse();

    println!("input path: {}", args.input);
    println!("output path: {}", args.output);

    let response = extract_audio_from_video(&args.input, &args.output);

    println!("{:#?}", response);

    let api = match ApiBuilder::new().build() {
        Ok(api) => api,
        Err(err) => panic!("error in building api: {err}"),
    };

    let repo = api.model("openai/whisper-small.en".to_string());

    // Asynchronously fetch the file
    let model_path = repo.get("model.safetensors").await;
    println!("Async model downloaded to: {:?}", model_path);
}
