use async_trait::async_trait;
use cli_deps::{CancellationToken, CommandRunner};
use deepgram_transcription::audio::{
    PreprocessOptions, Selection, mime_type, parse_duration, prepare,
};
use std::{path::Path, sync::Mutex};

struct Runner {
    duration: Option<&'static str>,
    calls: Mutex<Vec<Vec<String>>>,
}
#[async_trait]
impl CommandRunner for Runner {
    async fn run(
        &self,
        name: &str,
        args: &[String],
        token: &CancellationToken,
    ) -> cli_deps::Result<Vec<u8>> {
        if token.is_cancelled() {
            return Err(cli_deps::Error::Cancelled);
        }
        self.calls.lock().unwrap().push(args.to_vec());
        if name == "ffmpeg" {
            std::fs::write(args.last().unwrap(), b"larger processed audio file")?;
            return Ok(Vec::new());
        }
        let duration = if args.last().unwrap().ends_with("original.wav") {
            Some("10.0")
        } else {
            self.duration
        };
        Ok(serde_json::to_vec(&serde_json::json!({"streams":[{"duration":duration}]})).unwrap())
    }
}

#[tokio::test]
async fn upload_selection_uses_measured_duration_and_owns_temporary_cleanup() {
    for (duration, processed) in [
        (Some("5.0"), true),
        (Some("10.0"), false),
        (Some("12.0"), false),
        (None, false),
    ] {
        let dir = tempfile::tempdir().unwrap();
        let input = dir.path().join("original.wav");
        std::fs::write(&input, b"original").unwrap();
        let runner = Runner {
            duration,
            calls: Mutex::new(Vec::new()),
        };
        let prepared = prepare(
            &input,
            &PreprocessOptions::default(),
            &dir.path().join("processed"),
            &runner,
            &CancellationToken::new(),
        )
        .await
        .unwrap();
        assert_eq!(prepared.preprocessed(), processed);
        assert_eq!(prepared.path() != input, processed);
        if processed {
            assert!(matches!(
                prepared.selection,
                Selection::Shorter {
                    original: 10.0,
                    processed: 5.0
                }
            ));
            assert!(
                std::fs::metadata(prepared.path()).unwrap().len()
                    > std::fs::metadata(&input).unwrap().len()
            );
        }
        let calls = runner.calls.lock().unwrap();
        assert_eq!(calls.len(), 3);
        assert!(calls[0].contains(
            &"silenceremove=stop_periods=-1:stop_duration=2.0:stop_threshold=-30dB".into()
        ));
        let temporary = calls[0].last().unwrap();
        assert_eq!(Path::new(temporary).exists(), processed);
        drop(prepared);
        assert!(!Path::new(temporary).exists());
        assert!(input.exists());
    }
}

#[test]
fn probes_reject_missing_multiple_nonfinite_and_nonpositive_durations() {
    for value in ["0", "-1", "NaN", "inf", "N/A"] {
        assert!(
            parse_duration(
                &serde_json::to_vec(&serde_json::json!({"streams":[{"duration":value}]})).unwrap()
            )
            .is_err()
        );
    }
    for value in ["{}", "{\"streams\":[]}", "{\"streams\":[{},{}]}", "broken"] {
        assert!(parse_duration(value.as_bytes()).is_err());
    }
    assert_eq!(
        parse_duration(br#"{"streams":[{"duration":"0.001"}]}"#).unwrap(),
        0.001
    );
    assert_eq!(mime_type(Path::new("VOICE.MP3")), "audio/mpeg");
    assert_eq!(mime_type(Path::new("VOICE")), "application/octet-stream");
}

#[cfg(unix)]
#[tokio::test]
async fn non_utf8_paths_are_rejected_instead_of_silently_addressing_another_file() {
    use std::{ffi::OsString, os::unix::ffi::OsStringExt};
    let dir = tempfile::tempdir().unwrap();
    let input = dir
        .path()
        .join(OsString::from_vec(b"audio-\xff.wav".to_vec()));
    std::fs::write(&input, b"audio").unwrap();
    let runner = Runner {
        duration: Some("5"),
        calls: Mutex::new(Vec::new()),
    };
    assert!(
        prepare(
            &input,
            &PreprocessOptions::default(),
            &dir.path().join("processed"),
            &runner,
            &CancellationToken::new()
        )
        .await
        .is_err()
    );
    assert!(runner.calls.lock().unwrap().is_empty());
}

#[tokio::test]
async fn disabled_and_cancelled_preparation_do_not_run_external_commands() {
    let dir = tempfile::tempdir().unwrap();
    let runner = Runner {
        duration: Some("5"),
        calls: Mutex::new(Vec::new()),
    };
    let cancel = CancellationToken::new();
    let input = dir.path().join("original.wav");
    let options = PreprocessOptions {
        mono: false,
        trim_silence: false,
        ..Default::default()
    };
    let audio = prepare(
        &input,
        &options,
        &dir.path().join("processed"),
        &runner,
        &cancel,
    )
    .await
    .unwrap();
    assert_eq!(audio.path(), input);
    assert_eq!(audio.selection, Selection::Disabled);
    assert!(!dir.path().join("processed").exists());
    cancel.cancel();
    assert!(matches!(
        prepare(
            &input,
            &PreprocessOptions::default(),
            dir.path(),
            &runner,
            &cancel
        )
        .await,
        Err(deepgram_transcription::Error::Cancelled)
    ));
    assert!(runner.calls.lock().unwrap().is_empty());
}
