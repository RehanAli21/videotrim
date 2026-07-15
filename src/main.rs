mod cli;
use cli::Args;

use videotrim::transcibe::Transcribe;
use videotrim::{audio, editor, llm};

use clap::Parser;
use std::fs;

/// To run this program use command
/// cargo run -- --input video_file_path --output directory_path_to_save_all_files
/// --user_instructions "additional instructions for AI model to consider when cutting video"
/// --whisper_model "model_name like small.en" --ollama_model "model_name like qwen2.5:7b"
#[tokio::main]
async fn main() {
    let args = Args::parse();

    let _ = fs::create_dir_all(&args.output).map_err(|e| e.to_string());

    let audio_file: String = format!("{}/audio.wav", args.output);

    let file_extension = match &args.input.rsplit_once(".") {
        Some((_, extension)) => extension.to_string(),
        None => panic!("Can't get file extension."),
    };

    match audio::extract_audio_from_video(&args.input, &audio_file) {
        Ok(_) => {}
        Err(err) => panic!("Error on extraction audio from video, err => {err}"),
    };

    let (audio_data, total_duration) = audio::read_audio(&audio_file);

    let transcriber = match Transcribe::new(&args.whisper_model).await {
        Ok(t) => t,
        Err(err) => panic!("Failed to setup whisper model, err => {err}"),
    };

    let json = match transcriber.transcribe(audio_data) {
        Ok(json_str) => json_str,
        Err(err) => panic!("Failed to transcribe audio, err => {err}"),
    };

    //saving transciption in a file to check later
    match fs::write(format!("{}/transcript.json", &args.output), &json) {
        Ok(_) => (),
        Err(err) => panic!("failed to save treancription in file, err => {err}"),
    };

    let transcript = json;

    let plan =
        match llm::get_plan_from_model(&args.ollama_model, &args.user_instructions, &transcript)
            .await
        {
            Ok(p) => p,
            Err(err) => panic!("failed to get plan from ollama model, err => {err}"),
        };

    if plan.edits.is_empty() {
        let reason = plan.reasoning;
        panic!(
            "AI Model did not create any editing for the video, reasoning of the model: {reason}"
        );
    }

    let (parts_to_keep, parts_to_cut) =
        editor::cuts_and_keeps(&plan.edits, total_duration, args.show_reasons);

    match editor::process_video(
        &args.input,
        &parts_to_keep,
        &parts_to_cut,
        &args.output,
        &file_extension,
    ) {
        Ok(_) => println!(
            "Your files are creating and can be found at {}",
            &args.output
        ),
        Err(err) => println!("Error on video processing, err => {}", err),
    };
}
