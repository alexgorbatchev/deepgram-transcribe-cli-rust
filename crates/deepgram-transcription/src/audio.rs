use crate::{Error, io};
use cli_deps::{CancellationToken, CommandRunner};
use serde::Deserialize;
use std::path::{Path, PathBuf};

#[derive(Clone, Debug)]
pub struct PreprocessOptions {
    pub mono: bool,
    pub trim_silence: bool,
    pub silence_threshold: String,
    pub silence_duration: String,
}

impl Default for PreprocessOptions {
    fn default() -> Self {
        Self {
            mono: true,
            trim_silence: true,
            silence_threshold: "-30dB".into(),
            silence_duration: "2.0".into(),
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub enum Selection {
    Disabled,
    Shorter { original: f64, processed: f64 },
    NotShorter { original: f64, processed: f64 },
    Unmeasured { reason: String },
}

#[derive(Debug)]
pub struct PreparedAudio {
    original: PathBuf,
    temporary: Option<tempfile::TempPath>,
    pub selection: Selection,
}

impl PreparedAudio {
    pub fn path(&self) -> &Path {
        self.temporary.as_deref().unwrap_or(&self.original)
    }
    pub fn preprocessed(&self) -> bool {
        self.temporary.is_some()
    }
}

fn command_error(error: cli_deps::Error) -> Error {
    if matches!(error, cli_deps::Error::Cancelled) {
        Error::Cancelled
    } else {
        Error::Audio(error)
    }
}

fn command_path(path: &Path) -> Result<String, Error> {
    path.to_str()
        .map(str::to_owned)
        .ok_or_else(|| Error::Invalid("audio preprocessing requires UTF-8 paths".into()))
}

pub fn ffmpeg_args(
    input: &Path,
    output: &Path,
    options: &PreprocessOptions,
) -> Result<Vec<String>, Error> {
    let mut args = vec!["-y".into(), "-i".into(), command_path(input)?];
    if options.mono {
        args.extend(["-ac".into(), "1".into()]);
    }
    if options.trim_silence {
        let threshold = if options.silence_threshold.is_empty() {
            "-30dB"
        } else {
            &options.silence_threshold
        };
        let duration = if options.silence_duration.is_empty() {
            "2.0"
        } else {
            &options.silence_duration
        };
        args.extend([
            "-af".into(),
            format!(
                "silenceremove=stop_periods=-1:stop_duration={duration}:stop_threshold={threshold}"
            ),
        ]);
    }
    args.push(command_path(output)?);
    Ok(args)
}

pub async fn duration(
    path: &Path,
    runner: &dyn CommandRunner,
    cancel: &CancellationToken,
) -> Result<f64, Error> {
    let mut args: Vec<String> = [
        "-v",
        "error",
        "-select_streams",
        "a:0",
        "-show_entries",
        "stream=duration",
        "-of",
        "json",
        "-i",
    ]
    .into_iter()
    .map(str::to_owned)
    .collect();
    args.push(command_path(path)?);
    parse_duration(
        &runner
            .run("ffprobe", &args, cancel)
            .await
            .map_err(command_error)?,
    )
}

pub fn parse_duration(data: &[u8]) -> Result<f64, Error> {
    #[derive(Deserialize)]
    struct Probe {
        streams: Vec<Stream>,
    }
    #[derive(Deserialize)]
    struct Stream {
        duration: Option<String>,
    }
    let probe: Probe = serde_json::from_slice(data)?;
    if probe.streams.len() != 1 {
        return Err(Error::Invalid(
            "audio stream duration is unavailable".into(),
        ));
    }
    let value = probe.streams[0]
        .duration
        .as_deref()
        .and_then(|duration| duration.parse::<f64>().ok())
        .filter(|duration| duration.is_finite() && *duration > 0.0);
    value.ok_or_else(|| Error::Invalid("audio stream duration must be finite and positive".into()))
}

pub async fn prepare(
    input: &Path,
    options: &PreprocessOptions,
    temporary_directory: &Path,
    runner: &dyn CommandRunner,
    cancel: &CancellationToken,
) -> Result<PreparedAudio, Error> {
    if cancel.is_cancelled() {
        return Err(Error::Cancelled);
    }
    let mut prepared = PreparedAudio {
        original: input.into(),
        temporary: None,
        selection: Selection::Disabled,
    };
    if !options.mono && !options.trim_silence {
        return Ok(prepared);
    }
    std::fs::create_dir_all(temporary_directory)
        .map_err(|error| io("creating temporary audio directory", error))?;
    let extension = input
        .extension()
        .and_then(|extension| extension.to_str())
        .unwrap_or("m4a");
    let temporary = tempfile::Builder::new()
        .prefix("transcribe-proc-")
        .suffix(&format!(".{extension}"))
        .tempfile_in(temporary_directory)
        .map_err(|error| io("reserving prepared audio", error))?
        .into_temp_path();
    runner
        .run("ffmpeg", &ffmpeg_args(input, &temporary, options)?, cancel)
        .await
        .map_err(command_error)?;
    let original = duration(input, runner, cancel).await;
    let processed = duration(&temporary, runner, cancel).await;
    match (original, processed) {
        (Ok(original), Ok(processed)) if processed < original => {
            prepared.selection = Selection::Shorter {
                original,
                processed,
            };
            prepared.temporary = Some(temporary);
        }
        (Ok(original), Ok(processed)) => {
            prepared.selection = Selection::NotShorter {
                original,
                processed,
            }
        }
        (Err(Error::Cancelled), _) | (_, Err(Error::Cancelled)) => return Err(Error::Cancelled),
        (Err(error), _) | (_, Err(error)) => {
            prepared.selection = Selection::Unmeasured {
                reason: error.to_string(),
            }
        }
    }
    Ok(prepared)
}

pub fn mime_type(path: &Path) -> &'static str {
    match path
        .extension()
        .and_then(|extension| extension.to_str())
        .unwrap_or("")
        .to_ascii_lowercase()
        .as_str()
    {
        "mp3" => "audio/mpeg",
        "m4a" => "audio/m4a",
        "mp4" => "audio/mp4",
        "wav" => "audio/wav",
        "ogg" => "audio/ogg",
        "flac" => "audio/flac",
        "aac" => "audio/aac",
        _ => "application/octet-stream",
    }
}
