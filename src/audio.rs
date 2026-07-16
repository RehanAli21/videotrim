use hound::WavReader;
use std::process::Command;

pub fn extract_audio_from_video(input: &str, output: &str) -> Result<(), String> {
    let cmd_output = Command::new("ffmpeg")
        .args([
            "-y",
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
        .output()
        .map_err(|e| format!("Failed to run ffmpeg: {}", e))?;

    if cmd_output.status.success() {
        Ok(())
    } else {
        Err("FFmpeg failed".to_string())
    }
}

pub fn read_audio(path: &str) -> (Vec<f32>, f64) {
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
