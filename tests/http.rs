use async_trait::async_trait;
use axum::{
    Router,
    body::Bytes,
    extract::{OriginalUri, State},
    http::{HeaderMap, StatusCode},
    routing::{get, post},
};
use cli_command_kit::Mode;
use cli_deps::{CancellationToken, CommandRunner};
use deepgram_transcribe_cli::{Context, entry};
use deepgram_transcription::cache::{Cache, Envelope, JobRecord, source_key};
use std::{
    future::IntoFuture,
    sync::{
        Arc, Mutex,
        atomic::{AtomicUsize, Ordering},
    },
};

#[derive(Clone, Default)]
struct Seen {
    audio: Arc<Mutex<Vec<Vec<u8>>>>,
    queries: Arc<Mutex<Vec<String>>>,
    count: Arc<AtomicUsize>,
}
async fn listen(
    State(seen): State<Seen>,
    OriginalUri(uri): OriginalUri,
    headers: HeaderMap,
    body: Bytes,
) -> (StatusCode, String) {
    assert_eq!(headers["authorization"], "Token fixture-key");
    seen.audio.lock().unwrap().push(body.to_vec());
    seen.queries.lock().unwrap().push(uri.to_string());
    let number = seen.count.fetch_add(1, Ordering::SeqCst) + 1;
    if body.as_ref() == b"fail" {
        return (StatusCode::PAYMENT_REQUIRED, "credits unavailable".into());
    }
    if body.as_ref() == b"slow" {
        tokio::time::sleep(std::time::Duration::from_secs(2)).await;
    }
    (StatusCode::OK,serde_json::json!({"metadata":{"request_id":format!("request-{number}"),"duration":5,"channels":1},"results":{"channels":[],"utterances":[{"start":0,"end":1,"transcript":"Remote speech"}]}}).to_string())
}
async fn server() -> (String, Seen, tokio::task::JoinHandle<std::io::Result<()>>) {
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let origin = format!("http://{}", listener.local_addr().unwrap());
    let seen = Seen::default();
    let router = Router::new()
        .route("/v1/listen", post(listen))
        .route(
            "/v1/projects",
            get(|| async {
                axum::Json(serde_json::json!({"projects":[{"project_id":"project"}]}))
            }),
        )
        .route(
            "/v1/projects/project/requests/{id}",
            get(
                |axum::extract::Path(id): axum::extract::Path<String>| async move {
                    if id == "pending" {
                        "null".to_string()
                    } else {
                        r#"{"response":{"details":{"usd":0.1234}}}"#.to_string()
                    }
                },
            ),
        )
        .with_state(seen.clone());
    (
        origin,
        seen,
        tokio::spawn(axum::serve(listener, router).into_future()),
    )
}
struct AudioRunner {
    calls: Mutex<Vec<String>>,
    duration: Option<&'static str>,
    missing: bool,
}
#[async_trait]
impl CommandRunner for AudioRunner {
    async fn run(
        &self,
        name: &str,
        args: &[String],
        cancel: &CancellationToken,
    ) -> cli_deps::Result<Vec<u8>> {
        if cancel.is_cancelled() {
            return Err(cli_deps::Error::Cancelled);
        }
        self.calls.lock().unwrap().push(name.into());
        if self.missing {
            return Err(std::io::Error::from(std::io::ErrorKind::NotFound).into());
        }
        if args == ["-version"] {
            return Ok(format!("{name} version 7.1.1 fixture").into_bytes());
        }
        if name == "ffmpeg" {
            std::fs::write(
                args.last().unwrap(),
                b"processed audio larger than original",
            )?;
            return Ok(vec![]);
        }
        let duration = if args.last().unwrap().contains("transcribe-proc-") {
            self.duration
        } else {
            Some("10")
        };
        Ok(serde_json::json!({"streams":[{"duration":duration}]})
            .to_string()
            .into_bytes())
    }
}
fn context(root: &std::path::Path, origin: String, runner: Arc<AudioRunner>) -> Context {
    Context {
        default_cache: root.join("cache"),
        managed_bin: root.join("bin"),
        temporary_audio: root.join("prepared"),
        endpoint: origin,
        api_key: "fixture-key".into(),
        runner,
        now: chrono::Utc::now(),
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

#[tokio::test]
async fn fresh_preprocessed_upload_cache_force_and_no_cache_preserve_spending_boundaries() {
    let (origin, seen, server) = server().await;
    let root = tempfile::tempdir().unwrap();
    let runner = Arc::new(AudioRunner {
        calls: Mutex::new(vec![]),
        duration: Some("5"),
        missing: false,
    });
    let context = context(root.path(), origin, runner.clone());
    let input = root.path().join("talk.wav");
    std::fs::write(&input, b"audio").unwrap();
    let args = [
        "transcript",
        "create",
        input.to_str().unwrap(),
        "--term",
        "Envoy,Rust",
        "--term",
        "rust",
        "--no-tech-terms",
    ];
    let (code, out, err) = invoke(&context, &args, Mode::Agent).await;
    assert_eq!(code, 0, "{err}");
    assert!(out.contains("Remote speech"));
    assert!(out.contains("Speaker unknown"));
    assert!(!out.contains("INFO:"));
    assert!(err.contains("up from 5 B"));
    assert_eq!(
        seen.audio.lock().unwrap()[0],
        b"processed audio larger than original"
    );
    assert!(seen.queries.lock().unwrap()[0].contains("diarize_model=latest"));
    assert!(seen.queries.lock().unwrap()[0].contains("keyterm=Envoy"));
    assert!(
        std::fs::read_dir(&context.temporary_audio)
            .unwrap()
            .next()
            .is_none()
    );
    let cache = Cache::new(&context.default_cache);
    let mut record = cache.records().unwrap().remove(0);
    assert!(record.preprocessed);
    record.cost_usd = "$0.321".into();
    record.cost_is_actual = true;
    cache.save_record(record.clone()).unwrap();
    for _ in 0..2 {
        assert_eq!(invoke(&context, &args, Mode::Human).await.0, 0);
    }
    assert_eq!(seen.count.load(Ordering::SeqCst), 1);
    assert_eq!(cache.records().unwrap(), vec![record]);
    let mut forced = args.to_vec();
    forced.push("--force");
    assert_eq!(invoke(&context, &forced, Mode::Agent).await.0, 0);
    assert_eq!(seen.count.load(Ordering::SeqCst), 2);
    let stored = cache.records().unwrap();
    let mut uncached = args.to_vec();
    uncached.extend(["--no-cache", "--no-preprocess", "--no-diarize"]);
    assert_eq!(invoke(&context, &uncached, Mode::Agent).await.0, 0);
    assert_eq!(seen.count.load(Ordering::SeqCst), 3);
    assert_eq!(cache.records().unwrap(), stored);
    assert_eq!(seen.audio.lock().unwrap()[2], b"audio");
    assert!(!seen.queries.lock().unwrap()[2].contains("diarize"));
    server.abort();
}

#[tokio::test]
async fn file_delivery_vocabulary_cache_warnings_and_api_failures_keep_one_paid_attempt() {
    let (origin, seen, server) = server().await;
    let root = tempfile::tempdir().unwrap();
    let mut context = context(
        root.path(),
        origin,
        Arc::new(AudioRunner {
            calls: Mutex::new(vec![]),
            duration: None,
            missing: true,
        }),
    );
    let input = root.path().join("talk.wav");
    std::fs::write(&input, b"audio").unwrap();
    let terms = root.path().join("terms.txt");
    std::fs::write(&terms, "# comment\nEnvoy\nrust\n").unwrap();
    let output = root.path().join("notes/result.md");
    let args = [
        "transcript",
        "create",
        input.to_str().unwrap(),
        "--no-preprocess",
        "--no-tech-terms",
        "--term",
        "Rust, Envoy",
        "--terms-file",
        terms.to_str().unwrap(),
        "--output",
        output.to_str().unwrap(),
    ];
    let (code, out, err) = invoke(&context, &args, Mode::Human).await;
    assert_eq!(code, 0, "{err}");
    assert!(out.is_empty());
    assert!(
        std::fs::read_to_string(&output)
            .unwrap()
            .contains("Remote speech")
    );
    let query = seen.queries.lock().unwrap()[0].clone();
    assert_eq!(query.matches("keyterm=").count(), 2);
    std::fs::write(&output, "overwrite me").unwrap();
    assert_eq!(invoke(&context, &args, Mode::Agent).await.0, 0);
    assert_eq!(seen.count.load(Ordering::SeqCst), 1);
    assert!(
        std::fs::read_to_string(&output)
            .unwrap()
            .contains("Remote speech")
    );
    std::fs::remove_file(&terms).unwrap();
    assert_eq!(invoke(&context, &args, Mode::Agent).await.0, 1);
    assert_eq!(seen.count.load(Ordering::SeqCst), 1);
    context.default_cache = root.path().join("cache-is-file");
    std::fs::write(&context.default_cache, "keep").unwrap();
    let (code, out, err) = invoke(
        &context,
        &[
            "transcript",
            "create",
            input.to_str().unwrap(),
            "--no-preprocess",
        ],
        Mode::Agent,
    )
    .await;
    assert_eq!(code, 0, "{err}");
    assert!(out.contains("Remote speech"));
    assert!(err.contains("Could not save transcription history"));
    std::fs::write(&input, b"fail").unwrap();
    let (code, out, err) = invoke(
        &context,
        &[
            "transcript",
            "create",
            input.to_str().unwrap(),
            "--no-cache",
            "--no-preprocess",
        ],
        Mode::Agent,
    )
    .await;
    assert_eq!(code, 1);
    assert!(out.is_empty());
    assert_eq!(err.matches("ERR:").count(), 1);
    assert!(err.contains("402"));
    assert_eq!(seen.count.load(Ordering::SeqCst), 3);
    server.abort();
}

#[tokio::test]
async fn cancellation_aborts_an_inflight_upload_without_retry_or_history() {
    let (origin, seen, server) = server().await;
    let root = tempfile::tempdir().unwrap();
    let context = context(
        root.path(),
        origin,
        Arc::new(AudioRunner {
            calls: Mutex::new(vec![]),
            duration: None,
            missing: true,
        }),
    );
    let input = root.path().join("slow.wav");
    std::fs::write(&input, b"slow").unwrap();
    let cancel = CancellationToken::new();
    let token = cancel.clone();
    let counter = seen.count.clone();
    let task = tokio::spawn(async move {
        tokio::time::timeout(std::time::Duration::from_secs(3), async {
            while counter.load(Ordering::SeqCst) == 0 {
                tokio::task::yield_now().await;
            }
        })
        .await
        .unwrap();
        token.cancel();
    });
    let mut out = vec![];
    let mut err = vec![];
    let code = entry(
        [
            "deepgram-transcribe",
            "transcript",
            "create",
            input.to_str().unwrap(),
            "--no-preprocess",
        ]
        .map(Into::into),
        || Ok(context.clone()),
        &mut out,
        &mut err,
        Mode::Agent,
        &cancel,
    )
    .await;
    task.await.unwrap();
    assert_eq!(code, 130);
    assert!(out.is_empty());
    assert_eq!(seen.count.load(Ordering::SeqCst), 1);
    assert!(!context.default_cache.exists());
    assert_eq!(String::from_utf8(err).unwrap().matches("ERR:").count(), 1);
    server.abort();
}
#[tokio::test]
async fn unavailable_equal_longer_and_unknown_duration_upload_original_with_clean_fallbacks() {
    let (origin, seen, server) = server().await;
    for (duration, missing) in [
        (Some("10"), false),
        (Some("12"), false),
        (None, false),
        (Some("5"), true),
    ] {
        let root = tempfile::tempdir().unwrap();
        let runner = Arc::new(AudioRunner {
            calls: Mutex::new(vec![]),
            duration,
            missing,
        });
        let context = context(root.path(), origin.clone(), runner);
        let input = root.path().join("talk.wav");
        std::fs::write(&input, b"audio").unwrap();
        let (code, out, err) = invoke(
            &context,
            &[
                "transcript",
                "create",
                input.to_str().unwrap(),
                "--no-tech-terms",
                "--no-trim-silence",
            ],
            Mode::Agent,
        )
        .await;
        assert_eq!(code, 0, "{err}");
        assert!(out.contains("Remote speech"));
        assert!(err.contains("original audio"));
        assert_eq!(seen.audio.lock().unwrap().last().unwrap(), b"audio");
        if context.temporary_audio.exists() {
            assert!(
                std::fs::read_dir(&context.temporary_audio)
                    .unwrap()
                    .next()
                    .is_none()
            );
        }
    }
    server.abort();
}
#[tokio::test]
async fn billed_history_refresh_preserves_responses_limits_and_pending_estimates() {
    let (origin, _, server) = server().await;
    let root = tempfile::tempdir().unwrap();
    let context = context(
        root.path(),
        origin,
        Arc::new(AudioRunner {
            calls: Mutex::new(vec![]),
            duration: None,
            missing: true,
        }),
    );
    let cache = Cache::new(&context.default_cache);
    for (id, day) in [("paid", 14), ("pending", 13)] {
        let record = JobRecord {
            sha256: source_key(id.as_bytes()),
            request_id: id.into(),
            filename: format!("{id}.wav"),
            timestamp: Some(
                chrono::DateTime::parse_from_rfc3339(&format!("2026-10-{day}T18:00:00Z"))
                    .unwrap()
                    .to_utc(),
            ),
            duration_seconds: 60.0,
            model: "nova-3".into(),
            cost_usd: "$0.006".into(),
            terms: vec!["Envoy".into()],
            ..Default::default()
        };
        cache.save(&record.sha256,&Envelope{record:Some(record.clone()),response:Some(serde_json::from_value(serde_json::json!({"metadata":{"request_id":id,"duration":60},"results":{}})).unwrap())}).unwrap();
    }
    let (code, out, err) = invoke(&context, &["job", "list"], Mode::Agent).await;
    assert_eq!(code, 0, "{err}");
    assert!(out.contains("date\tfile\tduration\tchannels\tmodel\tcost\trequest_id"));
    assert!(out.contains("$0.123"));
    assert!(out.contains("not billed yet"));
    assert!(out.contains("1/2 billed"));
    let record = cache.find_request("paid").unwrap();
    assert!(record.cost_is_actual);
    assert!(cache.response(&record.sha256).is_ok());
    let (code, out, err) = invoke(&context, &["job", "list", "--limit", "1"], Mode::Human).await;
    assert_eq!(code, 0, "{err}");
    assert!(!out.contains("pending.wav"));
    assert!(out.contains("paid.wav"));
    let (code, out, err) = invoke(&context, &["job", "inspect", "pending"], Mode::Agent).await;
    assert_eq!(code, 0, "{err}");
    assert!(out.contains("estimated at $0.006"));
    assert!(out.contains("preprocessing_requested: false"));
    let (code, out, err) = invoke(&context, &["job", "inspect", "paid"], Mode::Human).await;
    assert_eq!(code, 0, "{err}");
    assert!(out.contains("$0.123"));
    server.abort();
}
