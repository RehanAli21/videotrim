use crate::{EditCommand, TimelineSegment};

use std::fs;
use std::process::Command;

type CutsAndKeepsType = (Vec<TimelineSegment>, Vec<TimelineSegment>);

pub fn cuts_and_keeps(
    cuts: &[EditCommand],
    total_duration: f64,
    show_reasoning: bool,
) -> CutsAndKeepsType {
    let mut parts_to_keep: Vec<TimelineSegment> = vec![];
    let mut parts_to_cut: Vec<TimelineSegment> = vec![];

    let mut cursor: f64 = 0.0;
    let mut index: usize = 0;
    // sort cuts by start time first (LLM may return them out of order)
    let mut sorted = cuts.to_vec();
    sorted.sort_by(|a, b| a.start.partial_cmp(&b.start).unwrap());

    for cut in sorted {
        if cut.start > cursor {
            parts_to_keep.push(TimelineSegment {
                index,
                start: cursor,
                end: cut.start,
            });
            index += 1;
        }

        parts_to_cut.push(TimelineSegment {
            index,
            start: cut.start,
            end: cut.end,
        });

        if show_reasoning {
            println!("Removing Clip {}, reason: {}", index, cut.reason);
        }
        index += 1;

        cursor = cursor.max(cut.end);
    }

    if cursor < total_duration {
        parts_to_keep.push(TimelineSegment {
            index,
            start: cursor,
            end: total_duration,
        });
    }

    (parts_to_keep, parts_to_cut)
}

pub fn process_video(
    input: &str,
    parts_to_keep: &[TimelineSegment],
    parts_to_cut: &[TimelineSegment],
    output: &str,
    file_extension: &str,
) -> Result<(), String> {
    let parts_to_keep_dir = format!("{output}/used_clips");
    let parts_to_cut_dir = format!("{output}/removed_clips");

    let _ = fs::create_dir_all(&parts_to_keep_dir).map_err(|e| e.to_string());

    let _ = fs::create_dir_all(&parts_to_cut_dir).map_err(|e| e.to_string());

    for segment in parts_to_cut {
        let clip = format!(
            "{}/clip_{:03}.{}",
            parts_to_cut_dir, segment.index, file_extension
        );
        let duration = segment.end - segment.start;

        let status = Command::new("ffmpeg")
            .args([
                "-ss",
                &segment.start.to_string(), // seek to start
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
            return Err(format!("Failed to extract clip {}", segment.index));
        }
    }

    let mut keep_clip_paths = vec![];

    // saving each keep clip using it's range
    for segment in parts_to_keep {
        let clip = format!(
            "{}/clip_{:03}.{}",
            parts_to_keep_dir, segment.index, file_extension
        );
        let duration = segment.end - segment.start;

        let status = Command::new("ffmpeg")
            .args([
                "-ss",
                &segment.start.to_string(), // seek to start
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
            return Err(format!("Failed to extract clip {}", segment.index));
        }

        keep_clip_paths.push(clip);
    }

    // write a list for concating (ffmpeg needs this format)
    let list_path = format!("{}/list.txt", &parts_to_keep_dir);
    let list_content: String = keep_clip_paths
        .iter()
        .map(|p| format!("file '{}'\n", p.replace(&parts_to_keep_dir, ".")))
        .collect();

    fs::write(&list_path, list_content).map_err(|e| e.to_string())?;

    let file_output_path = format!("{output}/edited_video.{file_extension}");

    //concat all clips into on video
    let status = Command::new("ffmpeg")
        .args([
            "-f",
            "concat",
            "-safe",
            "0",
            "-i",
            &list_path,
            "-c",
            "copy",
            "-y",
            &file_output_path,
        ])
        .status()
        .map_err(|e| e.to_string())?;

    if !status.success() {
        return Err("Failed to join clips".to_string());
    }

    Ok(())
}
