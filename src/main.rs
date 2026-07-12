mod cli;
use cli::Args;

use rust_video_editor::transcibe::Transcribe;
use rust_video_editor::{audio, editor, llm};

use clap::Parser;
use std::fs;

#[tokio::main]
async fn main() {
    const AUDIOFILE: &str = "audio.wav";
    let args = Args::parse();

    let file_extension = match &args.input.rsplit_once(".") {
        Some((_, extension)) => extension.to_string(),
        None => panic!("Can't get file extension."),
    };

    let _ = audio::extract_audio_from_video(&args.input, AUDIOFILE);

    let (audio_data, total_duration) = audio::read_audio(AUDIOFILE);

    let transcriber = match Transcribe::new("small.en").await {
        Ok(t) => t,
        Err(err) => panic!("Failed to setup whisper model, err => {err}"),
    };

    let json = match transcriber.transcribe(audio_data) {
        Ok(json_str) => json_str,
        Err(err) => panic!("Failed to transcribe audio, err => {err}"),
    };

    //saving transciption in a file to check later
    match fs::write("transcript.json", &json) {
        Ok(_) => (),
        Err(err) => panic!("failed to save treancription in file, err => {err}"),
    };

    let transcript = json;

    let plan = match llm::get_plan_from_model(&args.user_instructions, &transcript).await {
        Ok(p) => p,
        Err(err) => panic!("failed to get plan from ollama model, err => {err}"),
    };

    let (parts_to_keep, parts_to_cut) = editor::cuts_and_keeps(&plan.edits, total_duration);

    let _ = editor::cut_video(
        &args.input,
        &parts_to_keep,
        &parts_to_cut,
        &args.output,
        &file_extension,
    );
}
