mod cli;
use cli::Args;

use videotrim::ProgressSpinner;
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

    println!("\n---------------------");
    let spinner = ProgressSpinner::run("Converting video to audio".to_string(), 300);
    match audio::extract_audio_from_video(&args.input, &audio_file) {
        Ok(_) => spinner.finish("Video converted to audio successfully".to_string()),
        Err(err) => panic!("Error on extraction audio from video, err => {err}"),
    };

    println!("\n---------------------");
    let spinner = ProgressSpinner::run("Getting data from audio file".to_string(), 300);
    let (audio_data, total_duration) = audio::read_audio(&audio_file);
    spinner.finish("Fetched data from file successfully".to_string());

    println!("\n---------------------");
    let spinner = ProgressSpinner::run("Setting up whisper model".to_string(), 300);
    let transcriber = match Transcribe::new(&args.whisper_model).await {
        Ok(t) => t,
        Err(err) => panic!("Failed to setup whisper model, err => {err}"),
    };
    spinner.finish("Whisper setup completed".to_string());

    println!("\n---------------------");
    let spinner = ProgressSpinner::run("Generating transciption".to_string(), 300);
    let json = match transcriber.transcribe(audio_data) {
        Ok(json_str) => json_str,
        Err(err) => panic!("Failed to transcribe audio, err => {err}"),
    };

    //saving transciption in a file to check later
    match fs::write(format!("{}/transcript.json", &args.output), &json) {
        Ok(_) => (),
        Err(err) => panic!("failed to save treancription in file, err => {err}"),
    };
    spinner.finish("Transciption generation completed and saved.".to_string());

    let transcript = json;

    println!("\n---------------------");
    let spinner = ProgressSpinner::run("Generating plan for video edits".to_string(), 300);
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
    spinner.finish("Plan generated successfully.".to_string());

    println!("\n---------------------");
    let spinner = ProgressSpinner::run("Getting video parts according to plan".to_string(), 300);
    let (parts_to_keep, parts_to_cut) =
        editor::cuts_and_keeps(&plan.edits, total_duration, args.show_reasons);
    spinner.finish(format!(
        "Got {} clips to use, and {} clips to remove",
        parts_to_keep.len(),
        parts_to_cut.len()
    ));

    println!("\n---------------------");
    let spinner = ProgressSpinner::run("Generating final output file".to_string(), 300);
    match editor::process_video(
        &args.input,
        &parts_to_keep,
        &parts_to_cut,
        &args.output,
        &file_extension,
    ) {
        Ok(_) => spinner.finish(format!(
            "Your files are created and can be found at {}",
            &args.output
        )),
        Err(err) => spinner.finish(format!("Error on video processing, err => {}", err)),
    };
}
