use async_trait::async_trait;
use cli_command_kit::Mode;
use cli_deps::{CancellationToken, CommandRunner};
use deepgram_transcribe_cli::{Context, entry};
use deepgram_transcription::{
    RequestOptions,
    cache::{Cache, Envelope, JobRecord, request_key, source_key},
};
use std::sync::{Arc, Mutex};

#[derive(Default)]
struct Runner {
    calls: Mutex<Vec<String>>,
    missing: bool,
}
#[async_trait]
impl CommandRunner for Runner {
    async fn run(
        &self,
        name: &str,
        args: &[String],
        _: &CancellationToken,
    ) -> cli_deps::Result<Vec<u8>> {
        self.calls.lock().unwrap().push(name.into());
        if self.missing {
            return Err(std::io::Error::from(std::io::ErrorKind::NotFound).into());
        }
        if args == ["-version"] {
            return Ok(format!("{name} version 7.1.1 Copyright fixture").into_bytes());
        }
        if name == "ffmpeg" {
            std::fs::write(args.last().unwrap(), b"larger processed audio")?;
            return Ok(vec![]);
        }
        Ok(format!(
            "{{\"streams\":[{{\"duration\":\"{}\"}}]}}",
            if args.last().unwrap().contains("transcribe-proc-") {
                5
            } else {
                10
            }
        )
        .into_bytes())
    }
}
fn context(root: &std::path::Path, runner: Arc<Runner>) -> Context {
    Context {
        default_cache: root.join("cache"),
        managed_bin: root.join("bin"),
        temporary_audio: root.join("prepared"),
        endpoint: "http://127.0.0.1:1".into(),
        api_key: String::new(),
        runner,
        now: chrono::DateTime::parse_from_rfc3339("2026-10-14T18:00:00Z")
            .unwrap()
            .to_utc(),
    }
}
async fn invoke(context: &Context, args: &[&str], mode: Mode) -> (u8, String, String) {
    let mut out = vec![];
    let mut err = vec![];
    let code = entry(
        std::iter::once("deepgram-transcribe")
            .chain(args.iter().copied())
            .map(Into::into),
        || Ok(context.clone()),
        &mut out,
        &mut err,
        mode,
        &CancellationToken::new(),
    )
    .await;
    (
        code,
        String::from_utf8(out).unwrap(),
        String::from_utf8(err).unwrap(),
    )
}
fn response() -> deepgram_transcription::Response {
    serde_json::from_value(serde_json::json!({"metadata":{"request_id":"original","duration":12.5,"channels":1},"results":{"channels":[],"utterances":[{"start":0,"end":1,"speaker":1,"transcript":"Saved speech"}]}})).unwrap()
}
#[tokio::test]
async fn cache_hits_are_offline_preserve_billed_history_and_use_original_metadata() {
    let root = tempfile::tempdir().unwrap();
    let runner = Arc::new(Runner::default());
    let context = context(root.path(), runner.clone());
    let input = root.path().join("talk.wav");
    std::fs::write(&input, b"audio").unwrap();
    let options = RequestOptions {
        model: "nova-2".into(),
        terms: vec!["OriginalVocabulary".into()],
        ..Default::default()
    };
    let key = request_key(b"audio", &options).unwrap();
    let record = JobRecord {
        request_id: "original".into(),
        sha256: key.clone(),
        source_sha256: source_key(b"audio"),
        model: "nova-2".into(),
        terms: options.terms,
        diarize_model: "latest".into(),
        cost_usd: "$0.123".into(),
        cost_is_actual: true,
        ..Default::default()
    };
    let cache = Cache::new(&context.default_cache);
    cache
        .save(
            &key,
            &Envelope {
                record: Some(record.clone()),
                response: Some(response()),
            },
        )
        .unwrap();
    for mode in [Mode::Agent, Mode::Human] {
        let (code, out, err) = invoke(
            &context,
            &[
                "transcript",
                "create",
                input.to_str().unwrap(),
                "--model",
                "nova-3",
                "--no-tech-terms",
            ],
            mode,
        )
        .await;
        assert_eq!(code, 0, "{err}");
        assert!(out.contains("Saved speech"));
        assert!(out.contains("nova-2"));
        assert!(out.contains("OriginalVocabulary"));
        assert!(!out.contains("WARN:"));
        assert_eq!(cache.envelope(&key).unwrap().record, Some(record.clone()));
    }
    assert!(runner.calls.lock().unwrap().is_empty());
}
#[tokio::test]
async fn metadata_and_invalid_arguments_have_no_filesystem_or_process_side_effects() {
    let root = tempfile::tempdir().unwrap();
    let runner = Arc::new(Runner::default());
    let context = context(root.path(), runner.clone());
    for args in [
        vec!["--version"],
        vec!["skill"],
        vec!["--help"],
        vec!["transcript", "--help"],
        vec!["job", "list", "--limit", "-1"],
        vec!["dependency", "install", "extra"],
        vec!["cache", "clear", "extra"],
        vec!["transcript", "create"],
    ] {
        let (code, out, err) = invoke(&context, &args, Mode::Agent).await;
        if args == ["--version"] {
            assert_eq!(code, 0);
            assert_eq!(out, concat!(env!("CARGO_PKG_VERSION"), "\n"));
        } else if args == ["skill"] {
            assert_eq!(out, include_str!("../skills/deepgram-transcribe/SKILL.md"));
        } else if args.contains(&"--help") {
            assert_eq!(code, 0);
            assert!(out.contains("skill"));
        } else {
            assert_eq!(code, 1);
            assert!(out.is_empty());
            assert!(!err.is_empty());
        }
    }
    assert!(runner.calls.lock().unwrap().is_empty());
    assert!(!context.managed_bin.exists());
    assert!(!context.default_cache.exists());
}
#[tokio::test]
async fn history_only_status_and_safe_clear_count_real_responses() {
    let root = tempfile::tempdir().unwrap();
    let context = context(root.path(), Arc::new(Runner::default()));
    let cache = Cache::new(&context.default_cache);
    cache
        .save_record(JobRecord {
            sha256: source_key(b"history"),
            request_id: "history".into(),
            ..Default::default()
        })
        .unwrap();
    let (code, out, err) = invoke(&context, &["cache", "status"], Mode::Agent).await;
    assert_eq!(code, 0, "{err}");
    assert!(out.contains("cached_responses: 0"));
    std::fs::write(cache.directory().join("unrelated.json"), "retain").unwrap();
    let (code, out, err) = invoke(&context, &["cache", "clear"], Mode::Agent).await;
    assert_eq!(code, 0);
    assert!(out.is_empty());
    assert!(err.contains('1'));
    assert!(cache.directory().join("unrelated.json").exists());
    let (code, out, err) = invoke(&context, &["job", "inspect", "absent"], Mode::Agent).await;
    assert_eq!(code, 1);
    assert!(out.is_empty());
    assert!(err.contains("job list"));
    assert_eq!(err.matches("ERR:").count(), 1);
}

#[tokio::test]
async fn dependency_reports_and_explicit_package_actions_use_injected_runner() {
    for missing in [false, true] {
        let root = tempfile::tempdir().unwrap();
        let runner = Arc::new(Runner {
            missing,
            ..Default::default()
        });
        let context = context(root.path(), runner.clone());
        for mode in [Mode::Agent, Mode::Human] {
            let (code, out, err) = invoke(&context, &["dependency", "list"], mode).await;
            assert_eq!(code, 0, "{err}");
            assert!(out.contains("ffmpeg"));
            assert!(out.contains("ffprobe"));
            assert!(out.contains(if missing { "not installed" } else { "7.1.1" }));
        }
        let (code, out, err) = invoke(&context, &["dependency", "install"], Mode::Agent).await;
        assert!(out.is_empty());
        if missing {
            assert_eq!(code, 1);
            assert!(err.contains("install manually"));
            assert_eq!(err.matches("ERR:").count(), 1);
        } else {
            assert_eq!(code, 0, "{err}");
            assert!(err.contains("already installed"));
        }
        #[cfg(any(target_os = "linux", target_os = "macos"))]
        {
            let (code, out, err) = invoke(&context, &["dependency", "update"], Mode::Agent).await;
            assert!(out.is_empty());
            if missing {
                assert_eq!(code, 1);
            } else {
                assert_eq!(code, 0, "{err}");
                assert!(err.contains("ffmpeg, ffprobe"));
            }
        }
    }
}

#[tokio::test]
async fn native_completions_groups_and_output_failures_are_reported() {
    let root = tempfile::tempdir().unwrap();
    let context = context(root.path(), Arc::new(Runner::default()));
    for shell in ["bash", "zsh", "fish", "powershell"] {
        for descriptions in [true, false] {
            let mut args = vec!["completion", shell];
            if !descriptions {
                args.push("--no-descriptions");
            }
            let (code, out, err) = invoke(&context, &args, Mode::Agent).await;
            assert_eq!(code, 0, "{err}");
            assert!(out.contains("deepgram-transcribe"));
        }
    }
    for args in [
        vec![],
        vec!["help", "job", "inspect"],
        vec!["job"],
        vec!["cache"],
        vec!["transcript"],
        vec!["dependency"],
        vec!["completion"],
    ] {
        let (code, out, err) = invoke(&context, &args, Mode::Human).await;
        assert_eq!(code, 0, "{err}");
        let expected = match args.first().copied() {
            Some("help") => "job",
            Some(subject) => subject,
            None => "deepgram-transcribe",
        };
        assert!(out.contains(expected), "{args:?}: {out}");
    }
    struct Broken;
    impl std::io::Write for Broken {
        fn write(&mut self, _: &[u8]) -> std::io::Result<usize> {
            Err(std::io::ErrorKind::BrokenPipe.into())
        }
        fn flush(&mut self) -> std::io::Result<()> {
            Ok(())
        }
    }
    for args in [
        vec!["skill"],
        vec!["completion", "fish"],
        vec!["cache", "status"],
        vec!["--help"],
        vec!["--version"],
    ] {
        let mut err = vec![];
        let code = entry(
            std::iter::once("deepgram-transcribe")
                .chain(args)
                .map(Into::into),
            || Ok(context.clone()),
            &mut Broken,
            &mut err,
            Mode::Agent,
            &CancellationToken::new(),
        )
        .await;
        assert_eq!(code, 1);
    }
}

#[test]
fn native_binary_keeps_metadata_streams_and_runtime_errors_separate() {
    let root = tempfile::tempdir().unwrap();
    for args in [
        vec!["--version"],
        vec!["skill"],
        vec!["transcript", "create"],
        vec!["transcript", "create", "missing.wav"],
        vec!["cache", "status"],
    ] {
        let output = std::process::Command::new(env!("CARGO_BIN_EXE_deepgram-transcribe"))
            .args(&args)
            .env("AGENT", "1")
            .env("HOME", root.path())
            .env("XDG_CACHE_HOME", root.path().join("cache"))
            .env("XDG_DATA_HOME", root.path().join("data"))
            .env_remove("DEEPGRAM_API_KEY")
            .output()
            .unwrap();
        if args.first() == Some(&"--version") {
            assert_eq!(
                output.stdout,
                concat!(env!("CARGO_PKG_VERSION"), "\n").as_bytes()
            );
            assert!(output.status.success());
            assert!(output.stderr.is_empty());
        } else if args.first() == Some(&"skill") {
            assert_eq!(
                output.stdout,
                include_bytes!("../skills/deepgram-transcribe/SKILL.md")
            );
            assert!(output.status.success());
        } else if args.first() == Some(&"cache") {
            assert!(output.status.success());
            assert!(
                String::from_utf8(output.stdout)
                    .unwrap()
                    .contains("cached_responses: 0")
            );
        } else {
            assert_eq!(output.status.code(), Some(1));
            assert!(output.stdout.is_empty());
            if args.len() == 3 {
                let error = String::from_utf8(output.stderr).unwrap();
                assert_eq!(error.matches("ERR:").count(), 1);
                assert!(!error.contains("Usage:"));
            }
        }
    }
    assert!(!root.path().join("data").exists());
}

#[test]
fn native_metadata_ignores_invalid_runtime_path_environment() {
    let root = tempfile::tempdir().unwrap();
    for args in [["--version"], ["skill"], ["--help"]] {
        let output = std::process::Command::new(env!("CARGO_BIN_EXE_deepgram-transcribe"))
            .args(args)
            .env("XDG_DATA_HOME", root.path().join("invalid:path"))
            .env_remove("DEEPGRAM_API_KEY")
            .output()
            .unwrap();
        assert!(
            output.status.success(),
            "{}",
            String::from_utf8_lossy(&output.stderr)
        );
        assert!(!output.stdout.is_empty());
        assert!(output.stderr.is_empty());
    }
}

#[tokio::test]
async fn cancelled_runtime_operations_do_not_clear_cache_or_probe_dependencies() {
    let root = tempfile::tempdir().unwrap();
    let runner = Arc::new(Runner::default());
    let context = context(root.path(), runner.clone());
    let cache = Cache::new(&context.default_cache);
    let record = JobRecord {
        sha256: source_key(b"keep"),
        request_id: "keep".into(),
        ..Default::default()
    };
    cache.save_record(record.clone()).unwrap();
    let cancel = CancellationToken::new();
    cancel.cancel();
    for args in [["cache", "clear"], ["job", "list"], ["dependency", "list"]] {
        let mut out = vec![];
        let mut err = vec![];
        let code = entry(
            std::iter::once("deepgram-transcribe")
                .chain(args)
                .map(Into::into),
            || Ok(context.clone()),
            &mut out,
            &mut err,
            Mode::Agent,
            &cancel,
        )
        .await;
        assert_eq!(code, 130);
        assert!(out.is_empty());
        assert_eq!(String::from_utf8(err).unwrap().matches("ERR:").count(), 1);
    }
    assert_eq!(cache.records().unwrap(), vec![record]);
    assert!(runner.calls.lock().unwrap().is_empty());
}
