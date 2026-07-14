use clap::Parser;

#[derive(Parser, Debug)]
#[command(version, about, long_about = None)]
pub struct Args {
    /// Input file path like myvideo.mp4
    #[arg(short, long)]
    pub input: String,

    /// output path to save file like ./output/editied_video.mp4
    #[arg(short, long)]
    pub output: String,

    /// instuctions from user
    #[arg(long)]
    pub user_instructions: String,

    /// model name for whisper
    #[arg(long)]
    pub whisper_model: String,

    /// model name for ollama
    #[arg(long)]
    pub ollama_model: String,
}
