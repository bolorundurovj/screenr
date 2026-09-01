use crate::AppState;
use base64::{engine::general_purpose, Engine as _};
use ffmpeg_sidecar::command::FfmpegCommand;
use image::RgbaImage;
use serde::Serialize;
use std::sync::atomic::Ordering;
use std::sync::mpsc::Receiver;
use std::time::Duration;
use tauri::{AppHandle, Emitter, State};
use xcap::{Frame, Monitor, VideoRecorder, Window};

const THUMB_WIDTH: u32 = 320;
const THUMB_HEIGHT: u32 = 180;

#[derive(Serialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct RecordingStatePayload {
    pub is_recording: bool,
    pub is_paused: bool,
    pub elapsed_secs: u64,
}

#[derive(Serialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct RecordingFinishedPayload {
    pub path: String,
    pub duration_secs: u64,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct VideoSource {
    pub id: String,
    pub name: String,
    /// base64 JPEG data URL, empty when the capture failed.
    pub thumbnail: String,
    pub width: u32,
    pub height: u32,
    /// Owning application, for windows only.
    pub app: Option<String>,
    pub is_primary: bool,
}

/// Encode a captured frame as a base64 JPEG data URL.
///
/// JPEG has no alpha channel, so the RGBA capture must be flattened to RGB
/// first. Skipping that makes the encoder abort and yield an empty thumbnail.
fn encode_jpeg_data_url(image: &RgbaImage, width: u32, height: u32) -> String {
    let resized =
        image::imageops::resize(image, width, height, image::imageops::FilterType::Nearest);
    let rgb = image::DynamicImage::ImageRgba8(resized).into_rgb8();
    let mut buf = std::io::Cursor::new(Vec::new());
    if rgb.write_to(&mut buf, image::ImageFormat::Jpeg).is_err() {
        return String::new();
    }
    format!(
        "data:image/jpeg;base64,{}",
        general_purpose::STANDARD.encode(buf.into_inner())
    )
}

#[tauri::command]
pub async fn get_displays() -> Result<Vec<VideoSource>, String> {
    let monitors = Monitor::all().unwrap_or_default();

    let sources: Vec<VideoSource> = monitors
        .into_iter()
        .map(|monitor| {
            let id = monitor.id().unwrap_or(0);
            let mut name = monitor.name().unwrap_or_default().to_string();
            if name.is_empty() {
                name = format!("Monitor {id}");
            }

            let thumbnail = monitor
                .capture_image()
                .map(|image| encode_jpeg_data_url(&image, THUMB_WIDTH, THUMB_HEIGHT))
                .unwrap_or_default();

            VideoSource {
                id: format!("monitor:{id}"),
                name,
                thumbnail,
                width: monitor.width().unwrap_or(0),
                height: monitor.height().unwrap_or(0),
                app: None,
                is_primary: monitor.is_primary().unwrap_or(false),
            }
        })
        .collect();

    Ok(sources)
}

#[tauri::command]
pub async fn get_windows() -> Result<Vec<VideoSource>, String> {
    use rayon::prelude::*;
    let windows = Window::all().unwrap_or_default();

    // Only ids cross the thread boundary: xcap's Window handles are !Send.
    // Capturing sequentially costs ~500ms for a couple dozen windows, so the
    // per-window screenshot is fanned out across cores instead.
    let window_ids: Vec<_> = windows
        .into_iter()
        .filter_map(|w| {
            let name = w.title().unwrap_or_default().to_string();
            if name.is_empty() || w.is_minimized().unwrap_or(false) {
                return None;
            }
            Some(w.id().unwrap_or(0))
        })
        .collect();

    let sources: Vec<VideoSource> = window_ids
        .into_par_iter()
        .filter_map(|id| {
            let window = Window::all()
                .ok()?
                .into_iter()
                .find(|w| w.id().unwrap_or(0) == id)?;

            let thumbnail = window
                .capture_image()
                .map(|image| encode_jpeg_data_url(&image, THUMB_WIDTH, THUMB_HEIGHT))
                .unwrap_or_default();

            Some(VideoSource {
                id: format!("window:{id}"),
                name: window.title().unwrap_or_default().to_string(),
                thumbnail,
                width: window.width().unwrap_or(0),
                height: window.height().unwrap_or(0),
                app: window.app_name().ok().map(|n| n.to_string()),
                is_primary: false,
            })
        })
        .collect();

    Ok(sources)
}

/// A source resolved to an xcap handle, held for the whole session.
///
/// Displays use a persistent capture session, which the OS feeds continuously.
/// Polling `capture_image()` instead re-does the setup every frame: measured on
/// a 3840x2160 display that is 71ms a frame against 31ms, so polling alone caps
/// the recording near 14fps and makes motion visibly jump.
///
/// Windows have no streaming API in xcap, so they still poll.
enum CaptureTarget {
    Display {
        recorder: VideoRecorder,
        frames: Receiver<Frame>,
        latest: Option<RgbaImage>,
        size: (u32, u32),
    },
    /// A display whose driver refused a capture session.
    PolledDisplay {
        monitor: Monitor,
        latest: Option<RgbaImage>,
        size: (u32, u32),
    },
    Window {
        window: Window,
        latest: Option<RgbaImage>,
    },
}

impl CaptureTarget {
    fn resolve(source_id: &str) -> Option<Self> {
        let (kind, raw_id) = source_id.split_once(':')?;
        let id = raw_id.parse::<u32>().ok()?;

        match kind {
            "monitor" => {
                let monitor = Monitor::all()
                    .ok()?
                    .into_iter()
                    .find(|m| m.id().unwrap_or(0) == id)?;
                let size = (
                    monitor.width().unwrap_or(1920),
                    monitor.height().unwrap_or(1080),
                );

                match monitor.video_recorder() {
                    Ok((recorder, frames)) => {
                        if recorder.start().is_err() {
                            return None;
                        }
                        Some(CaptureTarget::Display {
                            recorder,
                            frames,
                            latest: None,
                            size,
                        })
                    }
                    // Some drivers refuse a session. Polling is much slower but
                    // still records, which beats failing outright.
                    Err(e) => {
                        eprintln!("no capture session for monitor {id}, polling instead: {e}");
                        Some(CaptureTarget::PolledDisplay {
                            monitor,
                            latest: None,
                            size,
                        })
                    }
                }
            }
            "window" => Window::all()
                .ok()?
                .into_iter()
                .find(|w| w.id().unwrap_or(0) == id)
                .map(|window| CaptureTarget::Window {
                    window,
                    latest: None,
                }),
            _ => None,
        }
    }

    fn size(&self) -> (u32, u32) {
        match self {
            CaptureTarget::Display { size, .. } => *size,
            CaptureTarget::PolledDisplay { size, .. } => *size,
            CaptureTarget::Window { window, .. } => (
                window.width().unwrap_or(1920),
                window.height().unwrap_or(1080),
            ),
        }
    }

    /// Take at most one new frame, in the order the source produced it.
    ///
    /// Returns true when a genuinely new frame was consumed.
    ///
    /// Deliberately one frame per call rather than draining to the newest. The
    /// capture session delivers in bursts, so skipping to the last frame throws
    /// away real motion while the output timestamp still advances by a single
    /// interval. Every frame is then unique and evenly stamped, yet the content
    /// samples uneven moments, which is what makes playback jump.
    fn advance(&mut self) -> bool {
        match self {
            CaptureTarget::Display { frames, latest, .. } => match frames.try_recv() {
                Ok(frame) => match RgbaImage::from_raw(frame.width, frame.height, frame.raw) {
                    Some(image) => {
                        *latest = Some(image);
                        true
                    }
                    None => false,
                },
                Err(_) => false,
            },
            CaptureTarget::PolledDisplay {
                monitor, latest, ..
            } => match monitor.capture_image() {
                Ok(image) => {
                    *latest = Some(image);
                    true
                }
                Err(_) => false,
            },
            CaptureTarget::Window { window, latest } => match window.capture_image() {
                Ok(image) => {
                    *latest = Some(image);
                    true
                }
                Err(_) => false,
            },
        }
    }

    /// The frame most recently taken, reused when the source produced none.
    fn frame(&self) -> Option<&RgbaImage> {
        match self {
            CaptureTarget::Display { latest, .. }
            | CaptureTarget::PolledDisplay { latest, .. }
            | CaptureTarget::Window { latest, .. } => latest.as_ref(),
        }
    }
}

impl Drop for CaptureTarget {
    fn drop(&mut self) {
        if let CaptureTarget::Display { recorder, .. } = self {
            let _ = recorder.stop();
        }
    }
}

/// Grid used to tile several sources into one frame.
///
/// Cells are uniform and sized to the largest source. Each capture is
/// letterboxed into its cell so nothing is stretched or cropped.
struct Layout {
    columns: u32,
    cell_width: u32,
    cell_height: u32,
}

impl Layout {
    fn plan(sizes: &[(u32, u32)]) -> Self {
        let count = sizes.len().max(1) as u32;
        // 1 -> 1x1, 2 -> 2x1, 3..4 -> 2x2, 5..9 -> 3x3, and so on.
        let columns = (count as f64).sqrt().ceil() as u32;

        Layout {
            columns: columns.max(1),
            cell_width: sizes.iter().map(|(w, _)| *w).max().unwrap_or(1920).max(1),
            cell_height: sizes.iter().map(|(_, h)| *h).max().unwrap_or(1080).max(1),
        }
    }

    fn rows(&self, count: u32) -> u32 {
        count.div_ceil(self.columns).max(1)
    }

    fn canvas_size(&self, count: u32) -> (u32, u32) {
        (
            self.cell_width * self.columns.min(count.max(1)),
            self.cell_height * self.rows(count),
        )
    }

    fn cell_origin(&self, index: u32) -> (u32, u32) {
        (
            (index % self.columns) * self.cell_width,
            (index / self.columns) * self.cell_height,
        )
    }
}

fn fit_within(size: (u32, u32), cell: (u32, u32)) -> (u32, u32) {
    let (width, height) = size;
    if width == 0 || height == 0 {
        return (1, 1);
    }
    let scale = (cell.0 as f64 / width as f64).min(cell.1 as f64 / height as f64);
    (
        ((width as f64 * scale).round() as u32).clamp(1, cell.0),
        ((height as f64 * scale).round() as u32).clamp(1, cell.1),
    )
}

/// Timestamp scale that turns `frames` at `nominal_fps` into `real_secs`.
///
/// Returns None when the recording already runs at real time, within a small
/// tolerance, so an unnecessary remux is skipped.
pub fn retime_scale(frames: u64, nominal_fps: u32, real_secs: f64) -> Option<f64> {
    if frames < 2 || nominal_fps == 0 || !real_secs.is_finite() || real_secs <= 0.0 {
        return None;
    }
    let claimed_secs = frames as f64 / nominal_fps as f64;
    if claimed_secs <= 0.0 {
        return None;
    }
    let scale = real_secs / claimed_secs;
    // Under a couple of percent is imperceptible and not worth a rewrite.
    if (scale - 1.0).abs() < 0.02 {
        return None;
    }
    Some(scale)
}

/// Stretch a take's timestamps so it plays back at real time.
///
/// The encoder cannot always consume frames as fast as the target rate demands,
/// especially at 4K, so the file ends up holding fewer frames than its declared
/// rate implies and plays fast. Rescaling the timestamps is a stream copy: no
/// re-encode, no quality loss.
pub fn retime_to_real_duration(path: &str, scale: f64) -> Result<(), String> {
    let source = std::path::Path::new(path);
    let retimed = source.with_extension("retimed.mp4");

    let mut command = FfmpegCommand::new();
    command
        .args(["-y", "-itsscale", &format!("{scale:.6}")])
        .args(["-i", path])
        .args(["-c", "copy"])
        .arg(retimed.to_string_lossy().to_string());

    let ok = command
        .spawn()
        .and_then(|mut child| child.wait())
        .map(|status| status.success())
        .unwrap_or(false);

    if !ok {
        let _ = std::fs::remove_file(&retimed);
        return Err("could not correct the recording speed".to_string());
    }

    std::fs::rename(&retimed, source).map_err(|e| e.to_string())
}

/// WebM cannot carry H.264.
fn codec_for_format(format: &str) -> &'static str {
    match format {
        "webm" => "libvpx-vp9",
        _ => "libx264",
    }
}

/// Target frame size for a resolution preference, preserving aspect ratio.
/// FFmpeg's H.264 encoder requires even dimensions.
fn target_size(source: (u32, u32), resolution: &str) -> (u32, u32) {
    let (width, height) = source;
    let target_height = match resolution {
        "1080p" => 1080,
        "720p" => 720,
        _ => height,
    };

    let (mut width, mut height) = if target_height >= height || height == 0 {
        (width, height)
    } else {
        let scaled = (width as f64 * target_height as f64 / height as f64).round() as u32;
        (scaled, target_height)
    };

    if width % 2 != 0 {
        width += 1;
    }
    if height % 2 != 0 {
        height += 1;
    }
    (width.max(2), height.max(2))
}

#[tauri::command]
pub fn start_recording(
    source_ids: Vec<String>,
    state: State<'_, AppState>,
    app: AppHandle,
) -> Result<String, String> {
    if state.is_recording.load(Ordering::SeqCst) {
        return Err("Already recording".to_string());
    }
    if source_ids.is_empty() {
        return Err("Select at least one source".to_string());
    }
    if !ffmpeg_sidecar::command::ffmpeg_is_installed() {
        return Err("FFmpeg is still downloading, try again in a moment".to_string());
    }

    let settings = state.settings()?;

    // Resolve here so a bad id fails the command rather than dying silently
    // inside the capture thread. xcap's handles are !Send, so only dimensions
    // cross the boundary and the thread resolves its own, once, not per frame.
    let (layout, width, height) = {
        let sizes = source_ids
            .iter()
            .map(|id| {
                CaptureTarget::resolve(id)
                    .map(|target| target.size())
                    .ok_or_else(|| format!("Source {id} is no longer available"))
            })
            .collect::<Result<Vec<_>, _>>()?;

        let layout = Layout::plan(&sizes);
        let canvas = layout.canvas_size(sizes.len() as u32);
        let (width, height) = target_size(canvas, &settings.resolution);
        (layout, width, height)
    };

    let path = crate::settings::next_take_path(&settings)?;
    let fps = settings.fps.clamp(1, 240);

    state.is_recording.store(true, Ordering::SeqCst);
    let is_recording = state.is_recording.clone();
    let is_paused = state.is_paused.clone();
    is_paused.store(false, Ordering::SeqCst);

    let output_path = path.clone();

    std::thread::spawn(move || {
        let mut targets: Vec<CaptureTarget> = source_ids
            .iter()
            .filter_map(|id| CaptureTarget::resolve(id))
            .collect();
        if targets.is_empty() {
            is_recording.store(false, Ordering::SeqCst);
            eprintln!("every source disappeared before capture started");
            return;
        }

        if settings.countdown {
            for tick in (1..=3).rev() {
                let _ = app.emit("countdown_tick", tick);
                std::thread::sleep(Duration::from_secs(1));
            }
        }
        let _ = app.emit("countdown_tick", 0);

        // Started after the countdown so the tracks share a start point, and
        // before ffmpeg so no speech is clipped from the opening moment.
        let mic = if settings.mic {
            let wav = std::path::PathBuf::from(&output_path).with_extension("mic.wav");
            match crate::audio::start(wav) {
                Ok(recording) => Some(recording),
                Err(e) => {
                    // A missing mic downgrades the take to silent rather than
                    // losing the recording entirely.
                    eprintln!("continuing without microphone: {e}");
                    let _ = app.emit("recording_warning", e);
                    None
                }
            }
        } else {
            None
        };

        // Cancelled during the countdown: leave without spawning ffmpeg, which
        // would otherwise write and announce an empty take.
        if !is_recording.load(Ordering::SeqCst) {
            if let Some(mic) = mic {
                if let Some(wav) = mic.finish() {
                    let _ = std::fs::remove_file(wav);
                }
            }
            crate::overlay::close(&app);
            let _ = app.emit(
                "recording_state",
                RecordingStatePayload {
                    is_recording: false,
                    is_paused: false,
                    elapsed_secs: 0,
                },
            );
            return;
        }

        // `-framerate` is what the take aims for, not what it achieves. The
        // rawvideo demuxer stamps frames at this nominal rate whatever the pipe
        // really delivers, so the container is corrected once the rate is known.
        let mut ffmpeg = match FfmpegCommand::new()
            .args([
                "-y",
                "-f",
                "rawvideo",
                "-pix_fmt",
                "rgba",
                "-s",
                &format!("{width}x{height}"),
                "-framerate",
                &fps.to_string(),
                "-i",
                "pipe:0",
                "-c:v",
                codec_for_format(&settings.format),
                "-preset",
                "veryfast",
                "-pix_fmt",
                "yuv420p",
            ])
            .arg(&output_path)
            .spawn()
        {
            Ok(f) => f,
            Err(e) => {
                is_recording.store(false, Ordering::SeqCst);
                eprintln!("Failed to spawn ffmpeg: {}", e);
                return;
            }
        };

        let mut stdin = match ffmpeg.take_stdin() {
            Some(s) => s,
            None => {
                is_recording.store(false, Ordering::SeqCst);
                eprintln!("Failed to open ffmpeg stdin");
                return;
            }
        };

        use std::io::Write;

        let frame_duration = Duration::from_nanos(1_000_000_000 / fps as u64);

        let mut next_frame_time = std::time::Instant::now();
        // Measured against wall time at the end to work out the real rate.
        let mut frames_written: u64 = 0;
        let mut consecutive_failures = 0;

        let (canvas_width, canvas_height) = layout.canvas_size(targets.len() as u32);
        let mut canvas = RgbaImage::new(canvas_width, canvas_height);
        // Only populated when the composite needs downscaling to the target.
        let mut scaled_canvas: Option<RgbaImage> = None;

        // One source at its native size is already the output frame, so the
        // capture goes straight to the pipe. Compositing it would resize it to
        // its own dimensions and copy it into an identically sized canvas: two
        // full passes over 33MB a frame at 4K, for no change.
        let passthrough = targets.len() == 1 && canvas_width == width && canvas_height == height;
        let mut advanced_sources: Vec<usize> = Vec::with_capacity(targets.len());

        let started_at = std::time::Instant::now();
        let mut paused_total = Duration::ZERO;
        let mut paused_since: Option<std::time::Instant> = None;
        let mut reported_secs = u64::MAX;

        while is_recording.load(Ordering::SeqCst) {
            if is_paused.load(Ordering::SeqCst) {
                if paused_since.is_none() {
                    paused_since = Some(std::time::Instant::now());
                    let _ = app.emit(
                        "recording_state",
                        RecordingStatePayload {
                            is_recording: true,
                            is_paused: true,
                            elapsed_secs: reported_secs.min(started_at.elapsed().as_secs()),
                        },
                    );
                }
                std::thread::sleep(Duration::from_millis(100));
                // Restart pacing on resume rather than racing to catch up.
                next_frame_time = std::time::Instant::now();
                continue;
            }

            if let Some(since) = paused_since.take() {
                paused_total += since.elapsed();
            }

            let elapsed_secs = started_at.elapsed().saturating_sub(paused_total).as_secs();
            if elapsed_secs != reported_secs {
                reported_secs = elapsed_secs;
                let _ = app.emit(
                    "recording_state",
                    RecordingStatePayload {
                        is_recording: true,
                        is_paused: false,
                        elapsed_secs,
                    },
                );
            }

            // One captured frame, written once. Padding the stream up to the
            // requested rate only works while the encoder can keep up, and at 4K
            // a frame is 33MB: `write_all` blocks and the take falls behind for
            // good. Retiming the container afterwards corrects the shortfall
            // instead.
            advanced_sources.clear();
            for (index, target) in targets.iter_mut().enumerate() {
                if target.advance() && target.frame().is_some() {
                    advanced_sources.push(index);
                }
            }

            let direct = passthrough
                && targets[0]
                    .frame()
                    .is_some_and(|f| f.width() == width && f.height() == height);

            if !direct {
                for &index in &advanced_sources {
                    let Some(frame) = targets[index].frame() else {
                        continue;
                    };

                    let cell = (layout.cell_width, layout.cell_height);
                    let (fit_width, fit_height) = fit_within((frame.width(), frame.height()), cell);

                    // Centre within the cell so mismatched aspect ratios letterbox.
                    let (origin_x, origin_y) = layout.cell_origin(index as u32);
                    let x = (origin_x + (cell.0 - fit_width) / 2) as i64;
                    let y = (origin_y + (cell.1 - fit_height) / 2) as i64;

                    if (fit_width, fit_height) == (frame.width(), frame.height()) {
                        image::imageops::replace(&mut canvas, frame, x, y);
                    } else {
                        let scaled = image::imageops::resize(
                            frame,
                            fit_width,
                            fit_height,
                            image::imageops::FilterType::Nearest,
                        );
                        image::imageops::replace(&mut canvas, &scaled, x, y);
                    }
                }
            }

            if advanced_sources.is_empty() {
                // Nothing new to send. Only give up once no source has produced
                // a frame for a sustained stretch.
                if targets.iter().all(|t| t.frame().is_none()) {
                    consecutive_failures += 1;
                    if consecutive_failures > fps {
                        eprintln!("capture sources became unavailable");
                        break;
                    }
                }
                std::thread::sleep(Duration::from_millis(1));
                continue;
            }
            consecutive_failures = 0;

            let output: &RgbaImage = if direct {
                match targets[0].frame() {
                    Some(frame) => frame,
                    None => continue,
                }
            } else if canvas.width() == width && canvas.height() == height {
                &canvas
            } else {
                scaled_canvas.insert(image::imageops::resize(
                    &canvas,
                    width,
                    height,
                    image::imageops::FilterType::Nearest,
                ))
            };

            if stdin.write_all(output.as_raw()).is_err() {
                eprintln!("ffmpeg stdin closed unexpectedly");
                break;
            }
            frames_written += 1;

            // Only sleep while ahead of schedule; when capture is the bottleneck
            // the next iteration should start immediately.
            next_frame_time += frame_duration;
            let now = std::time::Instant::now();
            if next_frame_time > now {
                std::thread::sleep(next_frame_time - now);
            } else {
                next_frame_time = now;
            }
        }

        let recorded = started_at.elapsed().saturating_sub(paused_total);
        let duration_secs = recorded.as_secs();

        drop(stdin);
        let _ = ffmpeg.wait();

        let achieved = frames_written as f64 / recorded.as_secs_f64().max(0.001);
        eprintln!(
            "captured {frames_written} frames in {:.1}s ({achieved:.1} fps against a {fps} fps target)",
            recorded.as_secs_f64()
        );

        // Correct the container before the audio is merged, so both tracks agree.
        if let Some(scale) = retime_scale(frames_written, fps, recorded.as_secs_f64()) {
            if let Err(e) = retime_to_real_duration(&output_path, scale) {
                eprintln!("{e}");
                let _ = app.emit("recording_warning", e);
            }
        }

        // Muxing happens after the video is closed, so the take exists on disk
        // even if merging the microphone track fails.
        if let Some(mic) = mic {
            if let Some(wav) = mic.finish() {
                let video = std::path::Path::new(&output_path);
                if let Err(e) = crate::audio::mux_into_video(video, &wav, &settings.format) {
                    eprintln!("keeping the silent take: {e}");
                    let _ = app.emit("recording_warning", e);
                }
                let _ = std::fs::remove_file(&wav);
            }
        }

        is_recording.store(false, Ordering::SeqCst);
        is_paused.store(false, Ordering::SeqCst);

        let _ = app.emit(
            "recording_finished",
            RecordingFinishedPayload {
                path: output_path,
                duration_secs,
            },
        );
        let _ = app.emit(
            "recording_state",
            RecordingStatePayload {
                is_recording: false,
                is_paused: false,
                elapsed_secs: 0,
            },
        );
    });

    Ok(path)
}

#[tauri::command]
pub fn stop_recording(state: State<'_, AppState>) -> Result<(), String> {
    state.is_recording.store(false, Ordering::SeqCst);
    Ok(())
}

#[tauri::command]
pub fn pause_recording(state: State<'_, AppState>) -> Result<(), String> {
    state.is_paused.store(true, Ordering::SeqCst);
    Ok(())
}

#[tauri::command]
pub fn resume_recording(state: State<'_, AppState>) -> Result<(), String> {
    state.is_paused.store(false, Ordering::SeqCst);
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn single_source_keeps_its_own_size() {
        let layout = Layout::plan(&[(1920, 1080)]);
        assert_eq!(layout.columns, 1);
        assert_eq!(layout.canvas_size(1), (1920, 1080));
    }

    #[test]
    fn two_sources_sit_side_by_side() {
        let layout = Layout::plan(&[(1920, 1080), (1920, 1080)]);
        assert_eq!(layout.columns, 2);
        assert_eq!(layout.rows(2), 1);
        assert_eq!(layout.canvas_size(2), (3840, 1080));
    }

    #[test]
    fn three_and_four_sources_use_a_two_by_two_grid() {
        let layout = Layout::plan(&[(800, 600); 3]);
        assert_eq!(layout.columns, 2);
        assert_eq!(layout.rows(3), 2);
        assert_eq!(layout.canvas_size(3), (1600, 1200));

        let layout = Layout::plan(&[(800, 600); 4]);
        assert_eq!(layout.rows(4), 2);
        assert_eq!(layout.canvas_size(4), (1600, 1200));
    }

    #[test]
    fn five_sources_grow_to_three_columns() {
        let layout = Layout::plan(&[(640, 480); 5]);
        assert_eq!(layout.columns, 3);
        assert_eq!(layout.rows(5), 2);
    }

    #[test]
    fn cells_are_sized_to_the_largest_source() {
        let layout = Layout::plan(&[(1280, 720), (2560, 1080), (800, 1440)]);
        assert_eq!(layout.cell_width, 2560);
        assert_eq!(layout.cell_height, 1440);
    }

    #[test]
    fn cell_origins_advance_across_then_down() {
        let layout = Layout::plan(&[(100, 50); 4]);
        assert_eq!(layout.cell_origin(0), (0, 0));
        assert_eq!(layout.cell_origin(1), (100, 0));
        assert_eq!(layout.cell_origin(2), (0, 50));
        assert_eq!(layout.cell_origin(3), (100, 50));
    }

    #[test]
    fn fit_within_preserves_aspect_ratio() {
        // A 16:9 source in a 4:3 cell is limited by width.
        assert_eq!(fit_within((1920, 1080), (800, 600)), (800, 450));
        // A 3:4 source in a wide cell is limited by height.
        assert_eq!(fit_within((600, 800), (800, 600)), (450, 600));
    }

    #[test]
    fn fit_within_never_exceeds_the_cell() {
        let (w, h) = fit_within((4000, 4000), (1920, 1080));
        assert!(w <= 1920 && h <= 1080, "got {w}x{h}");
    }

    #[test]
    fn fit_within_survives_a_zero_sized_capture() {
        assert_eq!(fit_within((0, 0), (1920, 1080)), (1, 1));
    }

    #[test]
    fn resolution_source_keeps_the_original_size() {
        assert_eq!(target_size((2560, 1440), "source"), (2560, 1440));
    }

    #[test]
    fn resolution_downscales_and_keeps_aspect() {
        assert_eq!(target_size((1920, 1080), "720p"), (1280, 720));
        assert_eq!(target_size((2560, 1440), "1080p"), (1920, 1080));
    }

    #[test]
    fn resolution_never_upscales() {
        assert_eq!(target_size((1280, 720), "1080p"), (1280, 720));
    }

    #[test]
    fn dimensions_are_rounded_up_to_even() {
        // H.264 rejects odd dimensions.
        let (w, h) = target_size((1919, 1079), "source");
        assert_eq!((w, h), (1920, 1080));
        assert_eq!(w % 2, 0);
        assert_eq!(h % 2, 0);
    }

    #[test]
    fn a_recording_at_the_requested_rate_needs_no_retiming() {
        // 300 frames at 30fps really did take 10 seconds.
        assert_eq!(retime_scale(300, 30, 10.0), None);
    }

    #[test]
    fn a_shortfall_stretches_the_timestamps() {
        // Only 100 frames arrived in the 10 seconds a 30fps take claims to
        // cover, so the container must be stretched 3x to play at real speed.
        let scale = retime_scale(100, 30, 10.0).expect("should retime");
        assert!((scale - 3.0).abs() < 0.001, "got {scale}");
    }

    #[test]
    fn small_drift_is_left_alone() {
        // Half a percent out; remuxing would cost more than it fixes.
        assert_eq!(retime_scale(299, 30, 10.0), None);
    }

    #[test]
    fn a_take_too_short_to_measure_is_left_alone() {
        assert_eq!(retime_scale(0, 30, 5.0), None);
        assert_eq!(retime_scale(1, 30, 5.0), None);
    }

    #[test]
    fn a_nonsense_duration_is_rejected() {
        assert_eq!(retime_scale(100, 30, 0.0), None);
        assert_eq!(retime_scale(100, 30, f64::NAN), None);
    }

    #[test]
    fn codecs_match_their_container() {
        assert_eq!(codec_for_format("webm"), "libvpx-vp9");
        assert_eq!(codec_for_format("mp4"), "libx264");
        assert_eq!(codec_for_format("mkv"), "libx264");
        assert_eq!(codec_for_format("anything-else"), "libx264");
    }
}
