pub mod audio;
pub mod capture;
pub mod export;
pub mod library;
pub mod overlay;
pub mod settings;

use std::sync::atomic::AtomicBool;
use std::sync::{Arc, Mutex};
use tauri::Manager;

pub struct AppState {
    pub is_recording: Arc<AtomicBool>,
    pub is_paused: Arc<AtomicBool>,
    pub settings: Arc<Mutex<settings::Settings>>,
}

impl AppState {
    /// A poisoned lock is reported to the caller rather than panicking, which
    /// would otherwise take down every later command too.
    pub fn settings(&self) -> Result<settings::Settings, String> {
        self.settings
            .lock()
            .map(|guard| guard.clone())
            .map_err(|_| "Settings are unavailable".to_string())
    }

    pub fn replace_settings(&self, next: settings::Settings) -> Result<(), String> {
        let mut guard = self
            .settings
            .lock()
            .map_err(|_| "Settings are unavailable".to_string())?;
        *guard = next;
        Ok(())
    }
}

/// Fetch the bundled ffmpeg build if this machine does not have one yet.
///
/// Runs off the main thread because the first launch downloads roughly a
/// hundred megabytes. Recording checks availability separately rather than
/// assuming this finished.
fn ensure_ffmpeg(app: tauri::AppHandle) {
    std::thread::spawn(move || {
        use tauri::Emitter;

        if ffmpeg_sidecar::command::ffmpeg_is_installed() {
            let _ = app.emit("ffmpeg_ready", true);
            return;
        }

        match ffmpeg_sidecar::download::auto_download() {
            Ok(()) => {
                let _ = app.emit("ffmpeg_ready", true);
            }
            Err(e) => {
                eprintln!("could not obtain ffmpeg: {e}");
                let _ = app.emit("ffmpeg_ready", false);
            }
        }
    });
}

/// Register the system-wide record toggle.
///
/// Registered here rather than from the webview so it keeps working while the
/// main window is hidden or unfocused, which is the whole point of it.
#[cfg(desktop)]
fn register_shortcuts(app: &tauri::AppHandle) -> Result<(), Box<dyn std::error::Error>> {
    use tauri::Emitter;
    use tauri_plugin_global_shortcut::{
        Code, GlobalShortcutExt, Modifiers, Shortcut, ShortcutState,
    };

    let toggle = Shortcut::new(Some(Modifiers::CONTROL | Modifiers::SHIFT), Code::KeyR);

    app.plugin(
        tauri_plugin_global_shortcut::Builder::new()
            .with_handler(move |app, shortcut, event| {
                // Fires on press and release; acting on both would toggle twice.
                if shortcut == &toggle && event.state() == ShortcutState::Pressed {
                    let _ = app.emit("shortcut_toggle_recording", ());
                }
            })
            .build(),
    )?;

    app.global_shortcut().register(toggle)?;
    Ok(())
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_dialog::init())
        .setup(|app| {
            let settings = settings::load_settings(app.handle());

            // tauri.conf.json only scopes the default videos folder, so a save
            // folder pointed anywhere else has to be granted at runtime.
            settings::allow_asset_access(app.handle(), &settings.save_folder);

            ensure_ffmpeg(app.handle().clone());

            app.manage(AppState {
                is_recording: Arc::new(AtomicBool::new(false)),
                is_paused: Arc::new(AtomicBool::new(false)),
                settings: Arc::new(Mutex::new(settings)),
            });

            // A shortcut already claimed by another app is not fatal; the app
            // stays usable through the UI.
            #[cfg(desktop)]
            if let Err(e) = register_shortcuts(app.handle()) {
                eprintln!("could not register global shortcut: {e}");
            }

            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            capture::get_displays,
            capture::get_windows,
            capture::start_recording,
            capture::stop_recording,
            capture::pause_recording,
            capture::resume_recording,
            settings::get_settings,
            settings::save_settings,
            overlay::open_overlay,
            overlay::close_overlay,
            library::get_takes,
            library::delete_take,
            library::reveal_take,
            export::trim_video,
            export::export_gif
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
