use crate::AppState;
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::PathBuf;
use std::time::{SystemTime, UNIX_EPOCH};
use tauri::{AppHandle, Manager, State};

#[derive(Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase", default)]
pub struct Settings {
    pub fps: u32,
    pub resolution: String,
    pub show_cursor: bool,
    pub countdown: bool,
    pub save_folder: String,
    pub format: String,
    pub mic: bool,
    pub system_audio: bool,
    pub draw_tools: bool,
    pub pen_color: String,
    pub theme: String,
    pub overlay_follows_theme: bool,
    pub srt_default: bool,
    pub ai_engine: String,
    pub ai_endpoint: String,
    pub ai_model: String,
    pub ai_language: String,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            fps: 30,
            resolution: "source".to_string(),
            show_cursor: true,
            countdown: true,
            save_folder: "".to_string(), // will be populated at runtime with video_dir
            format: "mp4".to_string(),
            mic: false,
            system_audio: false,
            draw_tools: false,
            pen_color: "#e5484d".to_string(),
            theme: "system".to_string(),
            overlay_follows_theme: true,
            srt_default: false,
            ai_engine: "Ollama".to_string(),
            ai_endpoint: "http://localhost:11434".to_string(),
            ai_model: "whisper".to_string(),
            ai_language: "en".to_string(),
        }
    }
}

fn get_settings_path(app: &AppHandle) -> PathBuf {
    let config_dir = app
        .path()
        .app_config_dir()
        .expect("failed to get config dir");
    if !config_dir.exists() {
        let _ = fs::create_dir_all(&config_dir);
    }
    config_dir.join("settings.json")
}

pub fn load_settings(app: &AppHandle) -> Settings {
    let path = get_settings_path(app);
    let loaded = path
        .exists()
        .then(|| fs::read_to_string(path).ok())
        .flatten()
        .and_then(|content| serde_json::from_str::<Settings>(&content).ok());

    let mut settings = loaded.unwrap_or_default();

    if settings.save_folder.is_empty() {
        if let Ok(video_dir) = app.path().video_dir() {
            let screenr_dir = video_dir.join("ScreenR");
            if !screenr_dir.exists() {
                let _ = fs::create_dir_all(&screenr_dir);
            }
            settings.save_folder = screenr_dir.to_string_lossy().to_string();
        }
    }
    settings
}

/// Let the webview read takes in `folder` over the asset protocol.
///
/// Without this the Review screen's video points at a URL the asset handler
/// refuses to serve, and playback fails silently.
pub fn allow_asset_access(app: &AppHandle, folder: &str) {
    if folder.is_empty() {
        return;
    }
    if let Err(e) = app.asset_protocol_scope().allow_directory(folder, false) {
        eprintln!("could not grant asset access to {folder}: {e}");
    }
}

/// Timestamped destination for the next take.
///
/// Creating the folder here means a save location that was deleted or is on a
/// disconnected drive fails before ffmpeg is spawned.
pub fn next_take_path(settings: &Settings) -> Result<String, String> {
    if settings.save_folder.is_empty() {
        return Err("No save folder configured".to_string());
    }

    let folder = PathBuf::from(&settings.save_folder);
    fs::create_dir_all(&folder)
        .map_err(|e| format!("Cannot write to {}: {e}", folder.display()))?;

    let stamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_err(|e| e.to_string())?
        .as_secs();

    let extension = match settings.format.as_str() {
        "webm" => "webm",
        "mkv" => "mkv",
        _ => "mp4",
    };

    Ok(folder
        .join(format!("ScreenR-{stamp}.{extension}"))
        .to_string_lossy()
        .to_string())
}

#[tauri::command]
pub fn get_settings(state: State<'_, AppState>) -> Result<Settings, String> {
    state.settings()
}

#[tauri::command]
pub fn save_settings(
    settings: Settings,
    app: AppHandle,
    state: State<'_, AppState>,
) -> Result<Settings, String> {
    let mut settings = settings;

    // A client that round-trips an unset folder must not clear the resolved one.
    if settings.save_folder.is_empty() {
        settings.save_folder = state.settings()?.save_folder;
    }

    let json = serde_json::to_string_pretty(&settings).map_err(|e| e.to_string())?;
    fs::write(get_settings_path(&app), json).map_err(|e| e.to_string())?;

    // A newly chosen folder needs asset access too, or Review cannot play it.
    allow_asset_access(&app, &settings.save_folder);

    state.replace_settings(settings.clone())?;
    Ok(settings)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn settings_in(folder: &std::path::Path, format: &str) -> Settings {
        Settings {
            save_folder: folder.to_string_lossy().to_string(),
            format: format.to_string(),
            ..Settings::default()
        }
    }

    fn scratch(name: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("screenr-test-{name}"));
        let _ = fs::remove_dir_all(&dir);
        dir
    }

    #[test]
    fn defaults_match_the_designed_capture_settings() {
        let settings = Settings::default();
        assert_eq!(settings.fps, 30);
        assert_eq!(settings.resolution, "source");
        assert_eq!(settings.format, "mp4");
        assert!(settings.countdown);
    }

    #[test]
    fn defaults_leave_unimplemented_features_off() {
        let settings = Settings::default();
        assert!(!settings.mic);
        assert!(!settings.system_audio);
        assert!(!settings.srt_default);
    }

    #[test]
    fn take_extension_follows_the_chosen_format() {
        let dir = scratch("take-extension");
        for (format, extension) in [("mp4", "mp4"), ("webm", "webm"), ("mkv", "mkv")] {
            let path = next_take_path(&settings_in(&dir, format)).unwrap();
            assert!(path.ends_with(extension), "{format} produced {path}");
        }
    }

    #[test]
    fn an_unknown_format_falls_back_to_mp4() {
        let dir = scratch("take-fallback");
        let path = next_take_path(&settings_in(&dir, "avi")).unwrap();
        assert!(path.ends_with(".mp4"), "got {path}");
    }

    #[test]
    fn takes_land_in_the_save_folder_and_create_it() {
        let dir = scratch("take-creates-folder");
        assert!(!dir.exists());

        let path = next_take_path(&settings_in(&dir, "mp4")).unwrap();

        assert!(dir.exists(), "save folder should be created up front");
        assert_eq!(PathBuf::from(&path).parent().unwrap(), dir);
    }

    #[test]
    fn a_missing_save_folder_is_an_error_rather_than_a_silent_default() {
        let settings = Settings {
            save_folder: String::new(),
            ..Settings::default()
        };
        assert!(next_take_path(&settings).is_err());
    }

    #[test]
    fn settings_round_trip_through_json_as_camel_case() {
        let settings = Settings::default();
        let json = serde_json::to_string(&settings).unwrap();

        // The frontend model is camelCase; a rename here would break it silently.
        assert!(json.contains("\"saveFolder\""), "{json}");
        assert!(json.contains("\"systemAudio\""), "{json}");
        assert!(json.contains("\"overlayFollowsTheme\""), "{json}");

        let parsed: Settings = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed.fps, settings.fps);
        assert_eq!(parsed.format, settings.format);
    }

    #[test]
    fn unknown_and_missing_fields_fall_back_to_defaults() {
        // Settings written by an older or newer build must still load.
        let parsed: Settings = serde_json::from_str(r#"{"fps":30,"somethingNew":true}"#).unwrap();
        assert_eq!(parsed.fps, 30);
        assert_eq!(parsed.format, Settings::default().format);
    }
}
