use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use cpal::SampleFormat;
use ffmpeg_sidecar::command::FfmpegCommand;
use std::path::{Path, PathBuf};
use std::sync::mpsc::{self, Receiver, Sender};
use std::sync::{Arc, Mutex};
use std::time::Duration;

type Writer = Arc<Mutex<Option<hound::WavWriter<std::io::BufWriter<std::fs::File>>>>>;

/// A microphone capture running on its own thread.
///
/// cpal's `Stream` is `!Send`, so it is built, played and dropped entirely
/// inside one thread; the handle talks to it over channels.
pub struct MicRecording {
    path: PathBuf,
    stop: Sender<()>,
    finished: Receiver<Result<(), String>>,
}

impl MicRecording {
    /// Stop capturing and finalise the WAV header.
    ///
    /// Returns the path only if samples were actually written, so a mic that
    /// produced nothing does not add a silent track.
    pub fn finish(self) -> Option<PathBuf> {
        let _ = self.stop.send(());

        match self.finished.recv_timeout(Duration::from_secs(5)) {
            Ok(Ok(())) => {}
            Ok(Err(e)) => {
                eprintln!("microphone capture failed: {e}");
                return None;
            }
            Err(_) => {
                eprintln!("microphone thread did not stop in time");
                return None;
            }
        }

        match std::fs::metadata(&self.path) {
            // A bare WAV header is 44 bytes; anything at or below that is silence.
            Ok(meta) if meta.len() > 44 => Some(self.path),
            _ => None,
        }
    }
}

/// Begin capturing the default input device to `path` as 16-bit WAV.
pub fn start(path: PathBuf) -> Result<MicRecording, String> {
    let (stop, stop_rx) = mpsc::channel();
    let (done, finished) = mpsc::channel();
    let (ready, ready_rx) = mpsc::channel();

    let target = path.clone();
    std::thread::spawn(move || {
        let result = capture(target, stop_rx, ready);
        let _ = done.send(result);
    });

    // Surface a missing or busy device now rather than after the take.
    match ready_rx.recv_timeout(Duration::from_secs(5)) {
        Ok(Ok(())) => Ok(MicRecording {
            path,
            stop,
            finished,
        }),
        Ok(Err(e)) => Err(e),
        Err(_) => Err("Microphone did not start in time".to_string()),
    }
}

fn capture(
    path: PathBuf,
    stop: Receiver<()>,
    ready: Sender<Result<(), String>>,
) -> Result<(), String> {
    let device = match cpal::default_host().default_input_device() {
        Some(device) => device,
        None => {
            let _ = ready.send(Err("No microphone found".to_string()));
            return Ok(());
        }
    };

    let config = match device.default_input_config() {
        Ok(config) => config,
        Err(e) => {
            let _ = ready.send(Err(format!("Microphone unavailable: {e}")));
            return Ok(());
        }
    };

    let spec = hound::WavSpec {
        channels: config.channels(),
        sample_rate: config.sample_rate().0,
        bits_per_sample: 16,
        sample_format: hound::SampleFormat::Int,
    };

    let writer: Writer = match hound::WavWriter::create(&path, spec) {
        Ok(w) => Arc::new(Mutex::new(Some(w))),
        Err(e) => {
            let _ = ready.send(Err(format!("Cannot write microphone track: {e}")));
            return Ok(());
        }
    };

    let on_error = |e| eprintln!("microphone stream error: {e}");
    let stream_config = config.clone().into();

    // Everything is normalised to i16 so the WAV is uniform whatever the
    // device hands us.
    let sink = writer.clone();
    let stream = match config.sample_format() {
        SampleFormat::F32 => device.build_input_stream(
            &stream_config,
            move |data: &[f32], _: &_| {
                write_samples(
                    &sink,
                    data.iter().map(|s| (s.clamp(-1.0, 1.0) * 32767.0) as i16),
                )
            },
            on_error,
            None,
        ),
        SampleFormat::I16 => device.build_input_stream(
            &stream_config,
            move |data: &[i16], _: &_| write_samples(&sink, data.iter().copied()),
            on_error,
            None,
        ),
        SampleFormat::U16 => device.build_input_stream(
            &stream_config,
            move |data: &[u16], _: &_| {
                write_samples(&sink, data.iter().map(|s| (*s as i32 - 32768) as i16))
            },
            on_error,
            None,
        ),
        other => {
            let _ = ready.send(Err(format!("Unsupported microphone format: {other:?}")));
            return Ok(());
        }
    };

    let stream = match stream {
        Ok(stream) => stream,
        Err(e) => {
            let _ = ready.send(Err(format!("Could not open the microphone: {e}")));
            return Ok(());
        }
    };

    if let Err(e) = stream.play() {
        let _ = ready.send(Err(format!("Could not start the microphone: {e}")));
        return Ok(());
    }

    let _ = ready.send(Ok(()));
    let _ = stop.recv();

    drop(stream);
    if let Some(writer) = writer
        .lock()
        .map_err(|_| "Microphone writer poisoned")?
        .take()
    {
        writer.finalize().map_err(|e| e.to_string())?;
    }
    Ok(())
}

fn write_samples(writer: &Writer, samples: impl Iterator<Item = i16>) {
    let Ok(mut guard) = writer.lock() else {
        return;
    };
    if let Some(writer) = guard.as_mut() {
        for sample in samples {
            let _ = writer.write_sample(sample);
        }
    }
}

/// Audio codec for a container. WebM takes Opus rather than AAC.
fn audio_codec_for(format: &str) -> &'static str {
    match format {
        "webm" => "libopus",
        _ => "aac",
    }
}

/// Mux a recorded WAV alongside the captured video, replacing `video` in place.
///
/// Video is stream-copied so this costs no quality and little time; only the
/// audio is encoded.
pub fn mux_into_video(video: &Path, audio: &Path, format: &str) -> Result<(), String> {
    let extension = video
        .extension()
        .map(|e| e.to_string_lossy().to_string())
        .unwrap_or_else(|| "mp4".to_string());
    let merged = video.with_extension(format!("muxed.{extension}"));

    let mut command = FfmpegCommand::new();
    command
        .args(["-y", "-i"])
        .arg(video.to_string_lossy().to_string())
        .args(["-i"])
        .arg(audio.to_string_lossy().to_string())
        .args(["-c:v", "copy"])
        .args(["-c:a", audio_codec_for(format)])
        .args(["-b:a", "160k"])
        // The tracks start together; end with whichever finishes first so a
        // trailing partial buffer cannot extend the take.
        .args(["-shortest"])
        .arg(merged.to_string_lossy().to_string());

    let mut child = command.spawn().map_err(|e| e.to_string())?;
    let status = child.wait().map_err(|e| e.to_string())?;
    if !status.success() {
        let _ = std::fs::remove_file(&merged);
        return Err("ffmpeg could not merge the microphone track".to_string());
    }

    std::fs::rename(&merged, video).map_err(|e| e.to_string())?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn webm_uses_opus_and_everything_else_aac() {
        assert_eq!(audio_codec_for("webm"), "libopus");
        assert_eq!(audio_codec_for("mp4"), "aac");
        assert_eq!(audio_codec_for("mkv"), "aac");
    }

    #[test]
    fn a_wav_with_only_a_header_counts_as_silence() {
        let dir = std::env::temp_dir().join("screenr-test-mic-empty");
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("mic.wav");
        std::fs::write(&path, vec![0u8; 44]).unwrap();

        let (stop, stop_rx) = mpsc::channel();
        let (done, finished) = mpsc::channel();
        drop(stop_rx);
        done.send(Ok(())).unwrap();

        let recording = MicRecording {
            path: path.clone(),
            stop,
            finished,
        };
        assert!(recording.finish().is_none());
    }

    #[test]
    fn a_wav_with_samples_is_kept() {
        let dir = std::env::temp_dir().join("screenr-test-mic-samples");
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("mic.wav");
        std::fs::write(&path, vec![0u8; 4096]).unwrap();

        let (stop, stop_rx) = mpsc::channel();
        let (done, finished) = mpsc::channel();
        drop(stop_rx);
        done.send(Ok(())).unwrap();

        let recording = MicRecording {
            path: path.clone(),
            stop,
            finished,
        };
        assert_eq!(recording.finish(), Some(path));
    }
}
