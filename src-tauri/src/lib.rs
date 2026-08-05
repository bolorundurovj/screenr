use base64::{engine::general_purpose, Engine as _};
use ffmpeg_sidecar::command::FfmpegCommand;
use ffmpeg_sidecar::download::auto_download;
use rayon::prelude::*;
use serde::{Deserialize, Serialize};
use std::io::Write;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Duration;
use tauri::{AppHandle, Emitter, State};
use xcap::{Monitor, Window};

#[derive(Serialize)]
struct VideoSource {
    id: String,
    name: String,
    thumbnail: String, // base64 JPEG
}

struct AppState {
    is_recording: Arc<AtomicBool>,
}

#[tauri::command]
async fn init_ffmpeg() -> Result<(), String> {
    auto_download().map_err(|e| e.to_string())?;
    Ok(())
}

#[tauri::command]
async fn get_displays() -> Result<Vec<VideoSource>, String> {
    let monitors = Monitor::all().unwrap_or_default();

    let sources: Vec<VideoSource> = monitors.into_iter().map(|monitor| {
        let mut name = monitor.name().unwrap_or_default().to_string();
        if name.is_empty() { name = format!("Monitor {}", monitor.id().unwrap_or(0)); }

        let mut thumbnail = String::new();
        if let Ok(image) = monitor.capture_image() {
            let resized = image::imageops::resize(&image, 320, 180, image::imageops::FilterType::Nearest);
            let rgb_image = image::DynamicImage::ImageRgba8(resized).into_rgb8();
            let mut buf = std::io::Cursor::new(Vec::new());
            if rgb_image.write_to(&mut buf, image::ImageFormat::Jpeg).is_ok() {
                thumbnail = format!("data:image/jpeg;base64,{}", general_purpose::STANDARD.encode(buf.into_inner()));
            }
        }

        VideoSource {
            id: format!("monitor:{}", monitor.id().unwrap_or(0)),
            name,
            thumbnail,
        }
    }).collect();

    Ok(sources)
}

#[tauri::command]
async fn get_windows() -> Result<Vec<VideoSource>, String> {
    let windows = Window::all().unwrap_or_default();

    let window_ids: Vec<_> = windows.into_iter().filter_map(|w| {
        let name = w.title().unwrap_or_default().to_string();
        if name.is_empty() || w.is_minimized().unwrap_or(false) {
            return None;
        }
        Some((w.id().unwrap_or(0), name))
    }).collect();

    let sources: Vec<VideoSource> = window_ids.into_par_iter().filter_map(|(id, name)| {
        let all_windows = Window::all().ok()?;
        let window = all_windows.into_iter().find(|w| w.id().unwrap_or(0) == id)?;
        
        let mut thumbnail = String::new();
        if let Ok(image) = window.capture_image() {
            let resized = image::imageops::resize(&image, 320, 180, image::imageops::FilterType::Nearest);
            let rgb_image = image::DynamicImage::ImageRgba8(resized).into_rgb8();
            let mut buf = std::io::Cursor::new(Vec::new());
            if rgb_image.write_to(&mut buf, image::ImageFormat::Jpeg).is_ok() {
                thumbnail = format!("data:image/jpeg;base64,{}", general_purpose::STANDARD.encode(buf.into_inner()));
            }
        }

        Some(VideoSource {
            id: format!("window:{}", id),
            name,
            thumbnail,
        })
    }).collect();

    Ok(sources)
}

#[tauri::command]
fn start_recording(source_id: String, path: String, state: State<'_, AppState>, app: AppHandle) -> Result<(), String> {
    if state.is_recording.load(Ordering::SeqCst) {
        return Err("Already recording".to_string());
    }

    state.is_recording.store(true, Ordering::SeqCst);
    let is_recording = state.is_recording.clone();

    std::thread::spawn(move || {
        let is_monitor = source_id.starts_with("monitor:");
        let real_id = source_id.split(':').nth(1).unwrap_or("0").parse::<u32>().unwrap_or(0);

        let mut width = 1920;
        let mut height = 1080;

        if is_monitor {
            if let Ok(monitors) = Monitor::all() {
                if let Some(m) = monitors.iter().find(|m| m.id().unwrap_or(0) == real_id) {
                    width = m.width().unwrap_or(1920);
                    height = m.height().unwrap_or(1080);
                }
            }
        } else {
            if let Ok(windows) = Window::all() {
                if let Some(w) = windows.iter().find(|w| w.id().unwrap_or(0) == real_id) {
                    width = w.width().unwrap_or(1920);
                    height = w.height().unwrap_or(1080);
                }
            }
        }

        if width % 2 != 0 { width += 1; }
        if height % 2 != 0 { height += 1; }

        let mut ffmpeg = match FfmpegCommand::new()
            .args(&[
                "-y",
                "-f", "rawvideo",
                "-pix_fmt", "rgba",
                "-s", &format!("{}x{}", width, height),
                "-use_wallclock_as_timestamps", "1",
                "-i", "pipe:0",
                "-c:v", "libx264",
                "-preset", "ultrafast",
                "-r", "30",
                "-pix_fmt", "yuv420p",
            ])
            .arg(&path)
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

        let frame_duration = Duration::from_millis(1000 / 30);
        let mut next_frame_time = std::time::Instant::now();
        let mut frame_count = 0;

        while is_recording.load(Ordering::SeqCst) {
            let mut image = None;

            if is_monitor {
                if let Ok(monitors) = Monitor::all() {
                    if let Some(m) = monitors.iter().find(|m| m.id().unwrap_or(0) == real_id) {
                        if let Ok(img) = m.capture_image() {
                            image = Some(img);
                        }
                    }
                }
            } else {
                if let Ok(windows) = Window::all() {
                    if let Some(w) = windows.iter().find(|w| w.id().unwrap_or(0) == real_id) {
                        if let Ok(img) = w.capture_image() {
                            image = Some(img);
                        }
                    }
                }
            }

            if let Some(mut img) = image {
                if img.width() != width || img.height() != height {
                    img = image::imageops::resize(&img, width, height, image::imageops::FilterType::Nearest);
                }
                
                // Emit preview BEFORE writing to stdin to ensure UI updates even if ffmpeg is slow/blocked
                frame_count += 1;
                if frame_count % 3 == 0 {
                    let resized = image::imageops::resize(&img, 640, 360, image::imageops::FilterType::Nearest);
                    let rgb_image = image::DynamicImage::ImageRgba8(resized).into_rgb8();
                    let mut buf = std::io::Cursor::new(Vec::new());
                    if rgb_image.write_to(&mut buf, image::ImageFormat::Jpeg).is_ok() {
                        let base64 = format!("data:image/jpeg;base64,{}", general_purpose::STANDARD.encode(buf.into_inner()));
                        let _ = app.emit("preview_frame", base64);
                    }
                }

                if stdin.write_all(img.as_raw()).is_err() {
                    eprintln!("FFmpeg stdin closed unexpectedly!");
                    break;
                }
            }

            next_frame_time += frame_duration;
            let now = std::time::Instant::now();
            if next_frame_time > now {
                std::thread::sleep(next_frame_time - now);
            } else {
                next_frame_time = now + frame_duration;
            }
        }
        
        drop(stdin);
        let _ = ffmpeg.wait();
    });

    Ok(())
}

#[tauri::command]
fn stop_recording(state: State<'_, AppState>) -> Result<(), String> {
    state.is_recording.store(false, Ordering::SeqCst);
    Ok(())
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_fs::init())
        .plugin(tauri_plugin_notification::init())
        .manage(AppState {
            is_recording: Arc::new(AtomicBool::new(false)),
        })
        .invoke_handler(tauri::generate_handler![
            init_ffmpeg,
            get_displays,
            get_windows,
            start_recording,
            stop_recording
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
