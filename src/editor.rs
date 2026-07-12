use crate::EditCommand;

use std::fs;
use std::process::Command;

type CutsAndKeepsType = (Vec<(f64, f64)>, Vec<(f64, f64)>);

pub fn cuts_and_keeps(cuts: &[EditCommand], total_duration: f64) -> CutsAndKeepsType {
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

pub fn cut_video(
    input: &str,
    parts_to_keep: &[(f64, f64)],
    parts_to_cut: &[(f64, f64)],
    output: &str,
    file_extension: &str,
) -> Result<(), String> {
    let parts_to_keep_dir = "used_clips";
    let parts_to_cut_dir = "removed_clips";

    let _ = fs::create_dir_all(parts_to_keep_dir).map_err(|e| e.to_string());

    let _ = fs::create_dir_all(parts_to_cut_dir).map_err(|e| e.to_string());

    for (i, (start, end)) in parts_to_cut.iter().enumerate() {
        let clip = format!("{}/clip_{:03}.{}", parts_to_cut_dir, i, file_extension);
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
        let clip = format!("{}/clip_{:03}.{}", parts_to_keep_dir, i, file_extension);
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
