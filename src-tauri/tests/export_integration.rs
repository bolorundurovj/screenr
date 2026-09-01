//! Integration tests that drive a real ffmpeg.
//!
//! Marked `#[ignore]` so a plain `cargo test` stays fast and works without the
//! binary present. Run them with:
//!
//! ```text
//! cargo test --test export_integration -- --ignored
//! ```

use ffmpeg_sidecar::command::FfmpegCommand;
use screenr_lib::export;
use std::path::{Path, PathBuf};

fn scratch(name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("screenr-it-{name}"));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).expect("scratch dir");
    dir
}

/// Synthesise a clip so the tests do not depend on a real recording.
fn make_clip(path: &Path, seconds: u32) -> bool {
    let ok = FfmpegCommand::new()
        .args(["-y", "-f", "lavfi"])
        .args([
            "-i",
            &format!("testsrc=size=320x240:rate=30:duration={seconds}"),
        ])
        .args(["-c:v", "libx264", "-pix_fmt", "yuv420p"])
        // Frequent keyframes keep the stream-copy trim close to the ask.
        .args(["-g", "15"])
        .arg(path.to_string_lossy().to_string())
        .spawn()
        .and_then(|mut child| child.wait())
        .map(|status| status.success())
        .unwrap_or(false);

    ok && path.exists()
}

/// Container duration in seconds, via ffprobe.
fn duration_of(path: &Path) -> f64 {
    let output = std::process::Command::new("ffprobe")
        .args(["-v", "error", "-show_entries", "format=duration"])
        .args(["-of", "default=noprint_wrappers=1:nokey=1"])
        .arg(path)
        .output()
        .expect("ffprobe should be available alongside ffmpeg");

    String::from_utf8_lossy(&output.stdout)
        .trim()
        .parse()
        .unwrap_or(0.0)
}

#[test]
#[ignore = "needs ffmpeg"]
fn trimming_produces_a_shorter_clip_beside_the_source() {
    let dir = scratch("trim");
    let source = dir.join("take.mp4");
    assert!(make_clip(&source, 6), "could not synthesise a clip");

    let out = export::trim_video(source.to_string_lossy().to_string(), 1.0, 4.0)
        .expect("trim should succeed");
    let out = PathBuf::from(out);

    assert!(out.exists(), "trimmed file was not written");
    assert_eq!(
        out.parent().unwrap(),
        dir,
        "export should sit beside the take"
    );

    let trimmed = duration_of(&out);
    assert!(
        (trimmed - 3.0).abs() < 1.0,
        "expected roughly 3s, got {trimmed}s"
    );
    assert!(trimmed < duration_of(&source));
}

#[test]
#[ignore = "needs ffmpeg"]
fn trimming_twice_does_not_overwrite_the_first_export() {
    let dir = scratch("trim-twice");
    let source = dir.join("take.mp4");
    assert!(make_clip(&source, 4), "could not synthesise a clip");

    let path = source.to_string_lossy().to_string();
    let first = export::trim_video(path.clone(), 0.0, 2.0).expect("first trim");
    let second = export::trim_video(path, 0.0, 2.0).expect("second trim");

    assert_ne!(first, second);
    assert!(Path::new(&first).exists() && Path::new(&second).exists());
}

#[test]
#[ignore = "needs ffmpeg"]
fn gif_export_writes_a_real_gif() {
    let dir = scratch("gif");
    let source = dir.join("take.mp4");
    assert!(make_clip(&source, 3), "could not synthesise a clip");

    let out = export::export_gif(
        source.to_string_lossy().to_string(),
        0.0,
        2.0,
        Some(10),
        Some(160),
    )
    .expect("gif export should succeed");
    let out = PathBuf::from(out);

    assert_eq!(out.extension().unwrap(), "gif");
    let bytes = std::fs::read(&out).expect("gif should be readable");
    assert!(
        bytes.len() > 128,
        "gif looks empty at {} bytes",
        bytes.len()
    );
    assert_eq!(&bytes[..6], b"GIF89a", "missing GIF magic number");
}

#[test]
#[ignore = "needs ffmpeg"]
fn an_out_of_range_trim_fails_before_touching_ffmpeg() {
    let dir = scratch("trim-invalid");
    let source = dir.join("take.mp4");
    assert!(make_clip(&source, 2), "could not synthesise a clip");

    let path = source.to_string_lossy().to_string();
    assert!(export::trim_video(path.clone(), 5.0, 1.0).is_err());
    assert!(export::trim_video(path, 1.0, 1.0).is_err());
}

#[test]
#[ignore = "needs ffmpeg"]
fn a_muxed_take_carries_an_audio_stream() {
    let dir = scratch("mux");
    let video = dir.join("take.mp4");
    let audio = dir.join("take.mic.wav");
    assert!(make_clip(&video, 3), "could not synthesise a clip");

    // A silent tone stands in for a microphone capture.
    let made_audio = FfmpegCommand::new()
        .args(["-y", "-f", "lavfi"])
        .args(["-i", "sine=frequency=440:duration=3"])
        .arg(audio.to_string_lossy().to_string())
        .spawn()
        .and_then(|mut child| child.wait())
        .map(|s| s.success())
        .unwrap_or(false);
    assert!(made_audio, "could not synthesise audio");

    screenr_lib::audio::mux_into_video(&video, &audio, "mp4").expect("mux should succeed");

    let probe = std::process::Command::new("ffprobe")
        .args([
            "-v",
            "error",
            "-select_streams",
            "a",
            "-show_entries",
            "stream=codec_type",
        ])
        .args(["-of", "default=noprint_wrappers=1:nokey=1"])
        .arg(&video)
        .output()
        .expect("ffprobe");

    assert!(
        String::from_utf8_lossy(&probe.stdout).contains("audio"),
        "muxed file has no audio stream"
    );
}

#[test]
#[ignore = "needs ffmpeg"]
fn retiming_stretches_a_take_to_real_time() {
    use screenr_lib::capture;

    let dir = scratch("retime");
    let source = dir.join("take.mp4");
    // 60 frames written at 30fps: the container claims 2 seconds.
    assert!(make_clip(&source, 2), "could not synthesise a clip");
    let claimed = duration_of(&source);
    assert!((claimed - 2.0).abs() < 0.3, "fixture is {claimed}s");

    // The capture really took 6 seconds, so it must be stretched 3x.
    let scale = capture::retime_scale(60, 30, 6.0).expect("should need retiming");
    capture::retime_to_real_duration(&source.to_string_lossy(), scale).expect("retime");

    let corrected = duration_of(&source);
    assert!(
        (corrected - 6.0).abs() < 0.5,
        "expected about 6s after retiming, got {corrected}s"
    );
}
