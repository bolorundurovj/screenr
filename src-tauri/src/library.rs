use crate::AppState;
use serde::Serialize;
use std::fs;
use std::path::Path;
use std::time::UNIX_EPOCH;
use tauri::State;

const VIDEO_EXTENSIONS: [&str; 3] = ["mp4", "webm", "mkv"];

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Take {
    pub name: String,
    pub absolute_path: String,
    pub size: u64,
    /// Seconds since the Unix epoch.
    pub modified_time: u64,
    pub has_srt: bool,
}

fn read_take(path: &Path) -> Option<Take> {
    let extension = path.extension()?.to_string_lossy().to_lowercase();
    if !VIDEO_EXTENSIONS.contains(&extension.as_str()) {
        return None;
    }

    let metadata = fs::metadata(path).ok()?;
    let modified_time = metadata
        .modified()
        .ok()?
        .duration_since(UNIX_EPOCH)
        .ok()?
        .as_secs();

    Some(Take {
        name: path.file_name()?.to_string_lossy().to_string(),
        absolute_path: path.to_string_lossy().to_string(),
        size: metadata.len(),
        modified_time,
        has_srt: path.with_extension("srt").exists(),
    })
}

#[tauri::command]
pub fn get_takes(state: State<'_, AppState>) -> Result<Vec<Take>, String> {
    let folder = state.settings()?.save_folder;
    if folder.is_empty() {
        return Ok(Vec::new());
    }

    // A folder that has not been created yet is empty, not an error.
    let entries = match fs::read_dir(&folder) {
        Ok(entries) => entries,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(Vec::new()),
        Err(e) => return Err(format!("Cannot read {folder}: {e}")),
    };

    let mut takes: Vec<Take> = entries
        .flatten()
        .map(|entry| entry.path())
        .filter(|path| path.is_file())
        .filter_map(|path| read_take(&path))
        .collect();

    takes.sort_by(|a, b| b.modified_time.cmp(&a.modified_time));
    Ok(takes)
}

/// Reject anything outside the configured save folder before touching disk.
fn take_in_library(path: &str, state: &State<'_, AppState>) -> Result<std::path::PathBuf, String> {
    let folder = state.settings()?.save_folder;
    if folder.is_empty() {
        return Err("No save folder configured".to_string());
    }

    let path = Path::new(path);
    let parent = path.parent().ok_or("Not a file path")?;

    let same_folder = match (parent.canonicalize(), Path::new(&folder).canonicalize()) {
        (Ok(a), Ok(b)) => a == b,
        _ => false,
    };
    if !same_folder {
        return Err("That file is not in the library folder".to_string());
    }

    Ok(path.to_path_buf())
}

#[tauri::command]
pub fn delete_take(path: String, state: State<'_, AppState>) -> Result<(), String> {
    let path = take_in_library(&path, &state)?;

    fs::remove_file(&path).map_err(|e| format!("Could not delete take: {e}"))?;

    let srt = path.with_extension("srt");
    if srt.exists() {
        let _ = fs::remove_file(srt);
    }
    Ok(())
}

#[tauri::command]
pub fn reveal_take(path: String, state: State<'_, AppState>) -> Result<(), String> {
    let path = take_in_library(&path, &state)?;
    tauri_plugin_opener::reveal_item_in_dir(&path).map_err(|e| e.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn scratch(name: &str) -> std::path::PathBuf {
        let dir = std::env::temp_dir().join(format!("screenr-test-{name}"));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).expect("scratch dir");
        dir
    }

    fn write(dir: &Path, name: &str, bytes: &[u8]) -> std::path::PathBuf {
        let path = dir.join(name);
        fs::write(&path, bytes).unwrap();
        path
    }

    #[test]
    fn reads_size_and_name_from_a_video() {
        let dir = scratch("lib-reads");
        let path = write(&dir, "take.mp4", b"0123456789");

        let take = read_take(&path).expect("should read an mp4");

        assert_eq!(take.name, "take.mp4");
        assert_eq!(take.size, 10);
        assert_eq!(take.absolute_path, path.to_string_lossy());
    }

    #[test]
    fn accepts_every_supported_container() {
        let dir = scratch("lib-containers");
        for name in ["a.mp4", "b.webm", "c.mkv"] {
            let path = write(&dir, name, b"x");
            assert!(read_take(&path).is_some(), "{name} should be listed");
        }
    }

    #[test]
    fn extension_matching_ignores_case() {
        let dir = scratch("lib-case");
        let path = write(&dir, "SHOUTING.MP4", b"x");
        assert!(read_take(&path).is_some());
    }

    #[test]
    fn skips_files_that_are_not_recordings() {
        let dir = scratch("lib-skips");
        for name in ["notes.txt", "take.srt", "noextension"] {
            let path = write(&dir, name, b"x");
            assert!(read_take(&path).is_none(), "{name} should be skipped");
        }
    }

    #[test]
    fn flags_a_take_that_has_subtitles_beside_it() {
        let dir = scratch("lib-srt");
        let with_subs = write(&dir, "narrated.mp4", b"x");
        write(&dir, "narrated.srt", b"1\n");
        let without = write(&dir, "silent.mp4", b"x");

        assert!(read_take(&with_subs).unwrap().has_srt);
        assert!(!read_take(&without).unwrap().has_srt);
    }

    #[test]
    fn a_missing_file_yields_nothing_rather_than_panicking() {
        let dir = scratch("lib-missing");
        assert!(read_take(&dir.join("ghost.mp4")).is_none());
    }
}
