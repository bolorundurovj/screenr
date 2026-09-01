use ffmpeg_sidecar::command::FfmpegCommand;
use std::path::{Path, PathBuf};

/// Destination beside the source, avoiding collisions with earlier exports.
fn export_path(source: &Path, suffix: &str, extension: &str) -> Result<PathBuf, String> {
    let parent = source.parent().ok_or("Take has no parent folder")?;
    let stem = source
        .file_stem()
        .ok_or("Take has no file name")?
        .to_string_lossy()
        .to_string();

    let mut candidate = parent.join(format!("{stem}{suffix}.{extension}"));
    let mut counter = 2;
    while candidate.exists() {
        candidate = parent.join(format!("{stem}{suffix}-{counter}.{extension}"));
        counter += 1;
    }
    Ok(candidate)
}

fn validate_range(start_secs: f64, end_secs: f64) -> Result<f64, String> {
    if !start_secs.is_finite() || !end_secs.is_finite() || start_secs < 0.0 {
        return Err("Invalid trim range".to_string());
    }
    let duration = end_secs - start_secs;
    if duration <= 0.0 {
        return Err("Trim range is empty".to_string());
    }
    Ok(duration)
}

fn run(command: &mut FfmpegCommand) -> Result<(), String> {
    let mut child = command.spawn().map_err(|e| e.to_string())?;
    let status = child.wait().map_err(|e| e.to_string())?;
    if !status.success() {
        return Err("ffmpeg exited with an error".to_string());
    }
    Ok(())
}

/// Trim a take to the given span without re-encoding.
///
/// Seeking before `-i` lets ffmpeg jump straight to the nearest keyframe
/// instead of decoding from the start, and `-t` is used rather than `-to`
/// because after an input seek the output timestamps restart at zero.
#[tauri::command]
pub fn trim_video(path: String, start_secs: f64, end_secs: f64) -> Result<String, String> {
    let source = PathBuf::from(&path);
    if !source.exists() {
        return Err("Take not found".to_string());
    }
    let duration = validate_range(start_secs, end_secs)?;

    let extension = source
        .extension()
        .map(|e| e.to_string_lossy().to_string())
        .unwrap_or_else(|| "mp4".to_string());
    let destination = export_path(&source, "-trimmed", &extension)?;

    let mut command = FfmpegCommand::new();
    command
        .args(["-y", "-ss", &start_secs.to_string()])
        .args(["-i", &path])
        .args(["-t", &duration.to_string()])
        // Stream copy keeps this near-instant; the cut lands on the nearest
        // preceding keyframe, which is the usual trade for not re-encoding.
        .args(["-c", "copy", "-avoid_negative_ts", "make_zero"])
        .arg(destination.to_string_lossy().to_string());
    run(&mut command)?;

    Ok(destination.to_string_lossy().to_string())
}

/// Render a trimmed span to an animated GIF using a two-pass palette.
///
/// The generated palette is what separates a usable GIF from a badly dithered
/// one. Frame rate and width are held down to keep the file size reasonable.
#[tauri::command]
pub fn export_gif(
    path: String,
    start_secs: f64,
    end_secs: f64,
    fps: Option<u32>,
    width: Option<u32>,
) -> Result<String, String> {
    let source = PathBuf::from(&path);
    if !source.exists() {
        return Err("Take not found".to_string());
    }
    let duration = validate_range(start_secs, end_secs)?;

    let fps = fps.unwrap_or(12).clamp(1, 30);
    let width = width.unwrap_or(640).clamp(120, 1920);
    let destination = export_path(&source, "", "gif")?;

    let palette = std::env::temp_dir().join(format!("screenr-palette-{}.png", std::process::id()));
    let filters = format!("fps={fps},scale={width}:-1:flags=lanczos");

    let mut pass_one = FfmpegCommand::new();
    pass_one
        .args(["-y", "-ss", &start_secs.to_string()])
        .args(["-i", &path])
        .args(["-t", &duration.to_string()])
        .args(["-vf", &format!("{filters},palettegen=stats_mode=diff")])
        .arg(palette.to_string_lossy().to_string());
    run(&mut pass_one)?;

    let mut pass_two = FfmpegCommand::new();
    pass_two
        .args(["-y", "-ss", &start_secs.to_string()])
        .args(["-i", &path])
        .args(["-t", &duration.to_string()])
        .args(["-i", &palette.to_string_lossy()])
        .args([
            "-lavfi",
            &format!("{filters}[x];[x][1:v]paletteuse=dither=bayer:bayer_scale=3"),
        ])
        .arg(destination.to_string_lossy().to_string());
    let result = run(&mut pass_two);

    let _ = std::fs::remove_file(&palette);
    result?;

    Ok(destination.to_string_lossy().to_string())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    /// Unique scratch directory so parallel tests never collide.
    fn scratch(name: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("screenr-test-{name}"));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).expect("scratch dir");
        dir
    }

    #[test]
    fn rejects_a_backwards_range() {
        assert!(validate_range(10.0, 4.0).is_err());
    }

    #[test]
    fn rejects_an_empty_range() {
        assert!(validate_range(5.0, 5.0).is_err());
    }

    #[test]
    fn rejects_a_negative_start() {
        assert!(validate_range(-1.0, 5.0).is_err());
    }

    #[test]
    fn rejects_non_finite_bounds() {
        assert!(validate_range(f64::NAN, 5.0).is_err());
        assert!(validate_range(0.0, f64::INFINITY).is_err());
    }

    #[test]
    fn returns_the_duration_of_a_valid_range() {
        assert_eq!(validate_range(2.5, 7.5).unwrap(), 5.0);
    }

    #[test]
    fn export_path_sits_beside_the_source() {
        let dir = scratch("export-beside");
        let source = dir.join("take.mp4");

        let out = export_path(&source, "-trimmed", "mp4").unwrap();

        assert_eq!(out.parent().unwrap(), dir);
        assert_eq!(out.file_name().unwrap(), "take-trimmed.mp4");
    }

    #[test]
    fn export_path_changes_the_extension_for_gif() {
        let dir = scratch("export-gif");
        let source = dir.join("take.mkv");

        let out = export_path(&source, "", "gif").unwrap();

        assert_eq!(out.file_name().unwrap(), "take.gif");
    }

    #[test]
    fn export_path_does_not_overwrite_an_earlier_export() {
        let dir = scratch("export-collide");
        let source = dir.join("take.mp4");
        fs::write(dir.join("take-trimmed.mp4"), b"first").unwrap();
        fs::write(dir.join("take-trimmed-2.mp4"), b"second").unwrap();

        let out = export_path(&source, "-trimmed", "mp4").unwrap();

        assert_eq!(out.file_name().unwrap(), "take-trimmed-3.mp4");
        assert!(!out.exists());
    }
}
