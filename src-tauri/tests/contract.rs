//! Contract tests across the Rust and TypeScript boundary.
//!
//! Tauri commands and events are addressed by string on both sides, so a rename
//! compiles cleanly and fails at runtime. These tests read both trees and assert
//! the names line up.

use regex::Regex;
use std::collections::BTreeSet;
use std::path::{Path, PathBuf};

fn repo_root() -> PathBuf {
    // CARGO_MANIFEST_DIR is src-tauri.
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .expect("repo root")
        .to_path_buf()
}

fn read(path: &Path) -> String {
    std::fs::read_to_string(path).unwrap_or_else(|e| panic!("reading {}: {e}", path.display()))
}

/// Concatenated contents of every file under `dir` with one of `extensions`.
fn sources(dir: &Path, extensions: &[&str]) -> String {
    let mut combined = String::new();
    let mut stack = vec![dir.to_path_buf()];

    while let Some(current) = stack.pop() {
        let Ok(entries) = std::fs::read_dir(&current) else {
            continue;
        };
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_dir() {
                stack.push(path);
            } else if path
                .extension()
                .map(|e| extensions.contains(&e.to_string_lossy().as_ref()))
                .unwrap_or(false)
            {
                combined.push_str(&read(&path));
                combined.push('\n');
            }
        }
    }
    combined
}

fn captures(haystack: &str, pattern: &str) -> BTreeSet<String> {
    Regex::new(pattern)
        .expect("valid pattern")
        .captures_iter(haystack)
        .map(|c| c[1].to_string())
        .collect()
}

fn frontend() -> String {
    // Specs stub the backend, so their strings are not real call sites.
    let all = sources(&repo_root().join("src/app"), &["ts", "html"]);
    all.lines()
        .filter(|line| !line.contains("useValue"))
        .collect::<Vec<_>>()
        .join("\n")
}

fn backend() -> String {
    sources(&repo_root().join("src-tauri/src"), &["rs"])
}

/// Frontend source with test assertions stripped out.
fn production_frontend() -> String {
    sources(&repo_root().join("src/app"), &["ts"])
        .lines()
        .filter(|l| !l.contains("expect(") && !l.contains("toHaveBeenCalledWith"))
        .collect::<Vec<_>>()
        .join("\n")
}

/// Literal `invoke('name')` call sites. Strict, so it never claims the backend
/// is missing something the frontend does not really call.
fn invoked_commands() -> BTreeSet<String> {
    captures(
        &production_frontend(),
        r#"invoke(?:<[^>]*>)?\(\s*'([a-z_]+)'"#,
    )
}

/// Every snake_case literal the frontend mentions. Loose, because Review picks
/// between `trim_video` and `export_gif` through a variable, and a strict scan
/// would report both as dead.
fn referenced_names() -> BTreeSet<String> {
    captures(&production_frontend(), r#"'([a-z]+(?:_[a-z]+)+)'"#)
}

fn registered_commands() -> BTreeSet<String> {
    let lib = read(&repo_root().join("src-tauri/src/lib.rs"));
    let block = lib
        .split("generate_handler![")
        .nth(1)
        .expect("generate_handler! block")
        .split(']')
        .next()
        .expect("closing bracket");

    block
        .split(',')
        .map(|entry| entry.trim())
        .filter(|entry| !entry.is_empty())
        .map(|entry| entry.rsplit("::").next().unwrap_or(entry).to_string())
        .collect()
}

#[test]
fn every_invoked_command_is_registered() {
    let registered = registered_commands();
    let missing: Vec<_> = invoked_commands()
        .into_iter()
        .filter(|command| !registered.contains(command))
        .collect();

    assert!(
        missing.is_empty(),
        "frontend invokes commands the backend does not register: {missing:?}\nregistered: {registered:?}"
    );
}

#[test]
fn every_registered_command_is_reachable() {
    // A command nobody calls is dead weight, or worse, a feature that quietly
    // stopped being wired up. init_ffmpeg was exactly that.
    let referenced = referenced_names();
    let unused: Vec<_> = registered_commands()
        .into_iter()
        .filter(|command| !referenced.contains(command))
        .collect();

    assert!(
        unused.is_empty(),
        "backend registers commands nothing calls: {unused:?}"
    );
}

#[test]
fn every_listened_event_is_emitted() {
    // `emit(` often wraps its payload across lines, so match past newlines.
    let emitted = captures(&backend(), r#"(?s)emit\(\s*"([a-z_]+)""#);
    let listened = captures(&frontend(), r#"listen(?:<[^>]*>)?\(\s*'([a-z_]+)'"#);

    let missing: Vec<_> = listened
        .iter()
        .filter(|event| !emitted.contains(*event))
        .collect();

    assert!(
        missing.is_empty(),
        "frontend listens for events the backend never emits: {missing:?}\nemitted: {emitted:?}"
    );
}

#[test]
fn every_emitted_event_has_a_listener() {
    let emitted = captures(&backend(), r#"(?s)emit\(\s*"([a-z_]+)""#);
    let listened = captures(&frontend(), r#"listen(?:<[^>]*>)?\(\s*'([a-z_]+)'"#);

    let ignored: Vec<_> = emitted
        .iter()
        .filter(|event| !listened.contains(*event))
        .collect();

    assert!(
        ignored.is_empty(),
        "backend emits events nothing listens for: {ignored:?}"
    );
}

#[test]
fn command_arguments_reach_the_backend_by_the_right_name() {
    // Tauri converts camelCase arguments to snake_case parameters. A rename on
    // either side is silently ignored and the command sees a default value.
    let ts = frontend();
    let rs = backend();

    for (command, js_arg, rust_param) in [
        ("start_recording", "sourceIds", "source_ids"),
        ("trim_video", "startSecs", "start_secs"),
        ("trim_video", "endSecs", "end_secs"),
        ("delete_take", "path", "path"),
        ("save_settings", "settings", "settings"),
    ] {
        assert!(
            ts.contains(js_arg),
            "frontend no longer passes {js_arg} to {command}"
        );
        assert!(
            rs.contains(rust_param),
            "{command} no longer accepts {rust_param}"
        );
    }
}
