//! Measures where the capture loop actually spends its time.
//!
//! Not an assertion suite; run it to get numbers:
//!
//! ```text
//! cargo test --test capture_bench -- --ignored --nocapture
//! ```

use ffmpeg_sidecar::command::FfmpegCommand;
use image::RgbaImage;
use std::io::Write;
use std::time::Instant;
use xcap::Monitor;

#[test]
#[ignore = "needs a display, reports timings"]
fn how_fast_can_we_capture_a_monitor() {
    let monitor = Monitor::all()
        .expect("monitors")
        .into_iter()
        .next()
        .expect("at least one monitor");

    let (width, height) = (monitor.width().unwrap_or(0), monitor.height().unwrap_or(0));
    println!("\nmonitor: {width}x{height}");

    // Warm up so first-call initialisation is not counted as steady state.
    let _ = monitor.capture_image();

    let samples = 30;
    let mut timings = Vec::with_capacity(samples);
    for _ in 0..samples {
        let start = Instant::now();
        let frame = monitor.capture_image();
        let elapsed = start.elapsed();
        assert!(frame.is_ok(), "capture failed mid-benchmark");
        timings.push(elapsed.as_secs_f64() * 1000.0);
    }

    timings.sort_by(|a, b| a.partial_cmp(b).unwrap());
    let median = timings[samples / 2];
    let best = timings[0];
    let worst = timings[samples - 1];
    let mean: f64 = timings.iter().sum::<f64>() / samples as f64;

    println!("capture_image() over {samples} frames:");
    println!("  best   {best:.1} ms");
    println!(
        "  median {median:.1} ms  -> {:.1} fps ceiling",
        1000.0 / median
    );
    println!("  mean   {mean:.1} ms");
    println!("  worst  {worst:.1} ms");
    println!("\n  a 30fps target needs <= 33.3 ms, 60fps needs <= 16.7 ms\n");
}

#[test]
#[ignore = "needs a display, reports timings"]
fn how_fast_is_the_persistent_video_recorder() {
    let monitor = Monitor::all()
        .expect("monitors")
        .into_iter()
        .next()
        .expect("at least one monitor");

    let (recorder, frames) = match monitor.video_recorder() {
        Ok(pair) => pair,
        Err(e) => {
            println!("\nvideo_recorder unavailable: {e}\n");
            return;
        }
    };

    recorder.start().expect("recorder should start");

    // Discard the first frame: it carries session setup cost.
    let _ = frames.recv();

    let samples = 30;
    let mut timings = Vec::with_capacity(samples);
    for _ in 0..samples {
        let start = Instant::now();
        match frames.recv() {
            Ok(_) => timings.push(start.elapsed().as_secs_f64() * 1000.0),
            Err(e) => {
                println!("frame stream ended early: {e}");
                break;
            }
        }
    }
    let _ = recorder.stop();

    if timings.is_empty() {
        println!("\nno frames arrived from the recorder\n");
        return;
    }

    timings.sort_by(|a, b| a.partial_cmp(b).unwrap());
    let median = timings[timings.len() / 2];
    println!("\nvideo_recorder over {} frames:", timings.len());
    println!(
        "  median {median:.1} ms between frames -> {:.1} fps\n",
        1000.0 / median
    );
}

/// How long the compositing step costs for a single full-size source.
///
/// This is the work the capture loop does between receiving a frame and writing
/// it: fit the capture into its cell, then copy it into the shared canvas.
#[test]
#[ignore = "reports timings"]
fn how_much_does_compositing_one_4k_source_cost() {
    let (width, height) = (3840, 2160);
    let frame = RgbaImage::new(width, height);
    let mut canvas = RgbaImage::new(width, height);

    let samples = 20;
    let mut resize_ms = Vec::with_capacity(samples);
    let mut replace_ms = Vec::with_capacity(samples);

    for _ in 0..samples {
        let start = Instant::now();
        let scaled =
            image::imageops::resize(&frame, width, height, image::imageops::FilterType::Nearest);
        resize_ms.push(start.elapsed().as_secs_f64() * 1000.0);

        let start = Instant::now();
        image::imageops::replace(&mut canvas, &scaled, 0, 0);
        replace_ms.push(start.elapsed().as_secs_f64() * 1000.0);
    }

    let median = |mut v: Vec<f64>| {
        v.sort_by(|a, b| a.partial_cmp(b).unwrap());
        v[v.len() / 2]
    };
    println!("\ncompositing one {width}x{height} source:");
    println!("  resize to the same size {:.1} ms", median(resize_ms));
    println!("  replace into canvas     {:.1} ms", median(replace_ms));
    println!("  (a 60fps budget is 16.7 ms for everything)\n");
}

/// How fast ffmpeg drains raw 4K RGBA from the pipe, with the recorder's flags.
///
/// The capture loop blocks in `write_all` once the pipe is full, so whatever
/// this reports is a hard ceiling on the recording's frame rate.
#[test]
#[ignore = "needs ffmpeg, reports timings"]
fn how_fast_does_ffmpeg_drain_the_pipe() {
    if !ffmpeg_sidecar::command::ffmpeg_is_installed() {
        println!("\nffmpeg not installed\n");
        return;
    }

    for (width, height, preset) in [
        (3840u32, 2160u32, "veryfast"),
        (3840, 2160, "ultrafast"),
        (1920, 1080, "veryfast"),
    ] {
        let output = std::env::temp_dir().join(format!("drain-{width}-{preset}.mp4"));
        let frame = vec![0u8; (width * height * 4) as usize];

        let mut ffmpeg = FfmpegCommand::new()
            .args([
                "-y",
                "-f",
                "rawvideo",
                "-pix_fmt",
                "rgba",
                "-s",
                &format!("{width}x{height}"),
                "-framerate",
                "60",
                "-i",
                "pipe:0",
                "-c:v",
                "libx264",
                "-preset",
                preset,
                "-pix_fmt",
                "yuv420p",
            ])
            .arg(output.to_string_lossy().to_string())
            .spawn()
            .expect("spawn ffmpeg");
        let mut stdin = ffmpeg.take_stdin().expect("stdin");

        let frames = 60;
        let start = Instant::now();
        for _ in 0..frames {
            stdin.write_all(&frame).expect("write frame");
        }
        let write_elapsed = start.elapsed();
        drop(stdin);
        let _ = ffmpeg.wait();
        let total = start.elapsed();

        println!(
            "\n{width}x{height} {preset}: {frames} frames accepted in {:.2}s -> {:.1} fps sustained",
            write_elapsed.as_secs_f64(),
            frames as f64 / write_elapsed.as_secs_f64()
        );
        println!("  including flush: {:.2}s", total.as_secs_f64());
        let _ = std::fs::remove_file(&output);
    }
    println!();
}

/// The whole loop, as the recorder runs it, with a breakdown per stage.
///
/// The isolated stages each look fast enough, so this runs them together
/// against real screen content and reports where the wall clock actually goes.
fn run_pipeline(composite: bool) {
    if !ffmpeg_sidecar::command::ffmpeg_is_installed() {
        println!("\nffmpeg not installed\n");
        return;
    }

    let monitor = Monitor::all()
        .expect("monitors")
        .into_iter()
        .next()
        .expect("at least one monitor");
    let (width, height) = (monitor.width().unwrap(), monitor.height().unwrap());

    let (recorder, frames) = monitor.video_recorder().expect("video_recorder");
    recorder.start().expect("start");
    let _ = frames.recv();

    let output = std::env::temp_dir().join("pipeline-bench.mp4");
    let mut ffmpeg = FfmpegCommand::new()
        .args([
            "-y",
            "-f",
            "rawvideo",
            "-pix_fmt",
            "rgba",
            "-s",
            &format!("{width}x{height}"),
            "-framerate",
            "60",
            "-i",
            "pipe:0",
            "-c:v",
            "libx264",
            "-preset",
            "veryfast",
            "-pix_fmt",
            "yuv420p",
        ])
        .arg(output.to_string_lossy().to_string())
        .spawn()
        .expect("spawn ffmpeg");
    let mut stdin = ffmpeg.take_stdin().expect("stdin");

    let mut canvas = RgbaImage::new(width, height);
    let (mut recv_ms, mut composite_ms, mut write_ms, mut idle_ms) = (0.0, 0.0, 0.0, 0.0);
    let mut written = 0u64;
    let mut backlog_peek = 0u64;

    let run = std::time::Duration::from_secs(10);
    let started = Instant::now();
    while started.elapsed() < run {
        let mark = Instant::now();
        let frame = match frames.try_recv() {
            Ok(f) => {
                recv_ms += mark.elapsed().as_secs_f64() * 1000.0;
                f
            }
            Err(_) => {
                std::thread::sleep(std::time::Duration::from_millis(1));
                idle_ms += mark.elapsed().as_secs_f64() * 1000.0;
                continue;
            }
        };

        let mark = Instant::now();
        let image = RgbaImage::from_raw(frame.width, frame.height, frame.raw).expect("frame");
        if composite {
            let scaled = image::imageops::resize(
                &image,
                width,
                height,
                image::imageops::FilterType::Nearest,
            );
            image::imageops::replace(&mut canvas, &scaled, 0, 0);
        }
        composite_ms += mark.elapsed().as_secs_f64() * 1000.0;

        let mark = Instant::now();
        let payload: &RgbaImage = if composite { &canvas } else { &image };
        stdin.write_all(payload.as_raw()).expect("write");
        write_ms += mark.elapsed().as_secs_f64() * 1000.0;
        written += 1;

        // How many frames the source queued while that iteration ran.
        let mut queued = 0;
        while let Ok(_extra) = frames.try_recv() {
            queued += 1;
            if queued > 200 {
                break;
            }
        }
        backlog_peek = backlog_peek.max(queued);
        // Drained frames are discarded here only to size the backlog; the real
        // loop keeps them, which is the point of the measurement.
    }

    let elapsed = started.elapsed().as_secs_f64();
    let _ = recorder.stop();
    drop(stdin);
    let _ = ffmpeg.wait();
    let _ = std::fs::remove_file(&output);

    let label = if composite {
        "compositing"
    } else {
        "passthrough"
    };
    println!("\n{label} pipeline at {width}x{height} over {elapsed:.1}s:");
    println!(
        "  frames written {written} -> {:.1} fps",
        written as f64 / elapsed
    );
    println!("  recv      {recv_ms:.0} ms total");
    println!(
        "  composite {composite_ms:.0} ms total ({:.1} ms/frame)",
        composite_ms / written.max(1) as f64
    );
    println!(
        "  write     {write_ms:.0} ms total ({:.1} ms/frame)",
        write_ms / written.max(1) as f64
    );
    println!("  idle      {idle_ms:.0} ms total");
    println!("  largest backlog seen after one iteration: {backlog_peek} frames\n");
}

/// The loop as it ran before the passthrough fast path: every frame resized to
/// its own size and copied into an identically sized canvas.
#[test]
#[ignore = "needs a display and ffmpeg, reports timings"]
fn the_pipeline_when_every_frame_is_composited() {
    run_pipeline(true);
}

/// A single source at its native size, written straight to the pipe.
#[test]
#[ignore = "needs a display and ffmpeg, reports timings"]
fn the_pipeline_when_a_single_source_passes_through() {
    run_pipeline(false);
}

/// Whether frames arrive evenly, which is what stretching timestamps assumes.
///
/// If the source only emits on change, arrival gaps are irregular and mapping
/// them onto evenly spaced output slots misrepresents when each frame happened.
#[test]
#[ignore = "needs a display, reports timings"]
fn how_evenly_do_frames_arrive() {
    let monitor = Monitor::all()
        .expect("monitors")
        .into_iter()
        .next()
        .expect("at least one monitor");
    let (recorder, frames) = monitor.video_recorder().expect("video_recorder");
    recorder.start().expect("start");
    let _ = frames.recv();

    let mut gaps = Vec::new();
    let run = std::time::Duration::from_secs(8);
    let started = Instant::now();
    let mut last = Instant::now();
    while started.elapsed() < run {
        // A timeout is recorded like any other gap: a long wait is exactly the
        // unevenness this is looking for.
        let _ = frames.recv_timeout(std::time::Duration::from_millis(500));
        gaps.push(last.elapsed().as_secs_f64() * 1000.0);
        last = Instant::now();
    }
    let _ = recorder.stop();

    gaps.sort_by(|a, b| a.partial_cmp(b).unwrap());
    let at = |p: f64| gaps[((gaps.len() - 1) as f64 * p) as usize];
    println!("\nframe arrival gaps over {} frames in 8s:", gaps.len());
    println!("  p10 {:.1} ms", at(0.10));
    println!("  p50 {:.1} ms", at(0.50));
    println!("  p90 {:.1} ms", at(0.90));
    println!("  max {:.1} ms", at(1.0));
    println!("  even delivery would show p10 and p90 close together\n");
}
