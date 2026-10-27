use axum::{
    Router,
    body::Bytes,
    extract::{OriginalUri, State},
    http::{HeaderMap, StatusCode},
    routing::post,
};
use deepgram_transcription::service::Service;
use deepgram_transcription::{Error, RequestOptions};
use std::{
    future::IntoFuture,
    sync::{Arc, Mutex},
    time::Duration,
};
use tokio_util::sync::CancellationToken;

#[derive(Clone, Default)]
struct Seen(Arc<Mutex<Vec<SeenRequest>>>);
type SeenRequest = (String, String, String, Vec<u8>);

async fn listen(
    State(seen): State<Seen>,
    OriginalUri(uri): OriginalUri,
    headers: HeaderMap,
    body: Bytes,
) -> (StatusCode, String) {
    seen.0.lock().unwrap().push((
        uri.to_string(),
        headers["authorization"].to_str().unwrap().into(),
        headers["content-type"].to_str().unwrap().into(),
        body.to_vec(),
    ));
    if body.as_ref() == b"slow" {
        tokio::time::sleep(Duration::from_millis(150)).await;
    }
    if body.as_ref() == b"fail" {
        return (StatusCode::TOO_MANY_REQUESTS, "credits exhausted".into());
    }
    if body.as_ref() == b"invalid" {
        return (StatusCode::OK, "{".into());
    }
    (StatusCode::OK, r#"{"metadata":{"request_id":"id","duration":5,"channels":1,"diarize_info":{"arch":"v2"}},"results":{"channels":[],"utterances":[{"start":0,"end":1,"transcript":"hello","speaker":1,"speaker_confidence":0.98}]}}"#.into())
}

#[tokio::test]
async fn request_preserves_auth_body_current_diarizer_and_keyword_dialects() {
    let seen = Seen::default();
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let service = Service::new(
        format!("http://{}/", listener.local_addr().unwrap()).as_str(),
        "fixture-key",
    )
    .unwrap();
    let server = tokio::spawn(
        axum::serve(
            listener,
            Router::new()
                .route("/v1/listen", post(listen))
                .with_state(seen.clone()),
        )
        .into_future(),
    );
    let token = CancellationToken::new();
    for model in ["nova-3", "flux-general-en", "nova-2"] {
        let options = RequestOptions {
            model: model.into(),
            terms: vec!["Rust & Go".into(), " second ".into(), " ".into()],
            ..Default::default()
        };
        let response = service
            .transcribe(b"binary\0audio".to_vec(), "audio/wav", &options, &token)
            .await
            .unwrap();
        assert_eq!(
            response.results.utterances.unwrap()[0].speaker_confidence,
            Some(0.98)
        );
        assert_eq!(response.metadata.extra["diarize_info"]["arch"], "v2");
        let requests = seen.0.lock().unwrap();
        let (uri, auth, mime, bytes) = requests.last().unwrap();
        assert_eq!(auth, "Token fixture-key");
        assert_eq!(mime, "audio/wav");
        assert_eq!(bytes, b"binary\0audio");
        let url = url::Url::parse(&format!("http://example.test{uri}")).unwrap();
        let pairs: Vec<_> = url
            .query_pairs()
            .map(|(key, value)| (key.into_owned(), value.into_owned()))
            .collect();
        assert!(pairs.contains(&("diarize_model".into(), "latest".into())));
        assert!(!pairs.iter().any(|(key, _)| key == "diarize"));
        let keyword = if model == "nova-2" {
            "keywords"
        } else {
            "keyterm"
        };
        assert_eq!(
            pairs
                .iter()
                .filter(|(key, _)| key == keyword)
                .map(|(_, value)| value.as_str())
                .collect::<Vec<_>>(),
            ["Rust & Go", "second"]
        );
    }
    server.abort();
}

#[tokio::test]
async fn failures_are_typed_single_attempt_and_cancellable() {
    let seen = Seen::default();
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let service = Service::new(
        format!("http://{}/", listener.local_addr().unwrap()).as_str(),
        "fixture-key",
    )
    .unwrap();
    let server = tokio::spawn(
        axum::serve(
            listener,
            Router::new()
                .route("/v1/listen", post(listen))
                .with_state(seen.clone()),
        )
        .into_future(),
    );
    let token = CancellationToken::new();
    let options = RequestOptions::default();
    let error = service
        .transcribe(b"fail".to_vec(), "", &options, &token)
        .await
        .unwrap_err();
    assert!(matches!(error, Error::Api { status:429, ref body } if body == "credits exhausted"));
    assert_eq!(seen.0.lock().unwrap().len(), 1);
    assert!(
        service
            .transcribe(b"invalid".to_vec(), "audio/wav", &options, &token)
            .await
            .is_err()
    );
    let mut service = service;
    service.timeout = Duration::from_millis(10);
    assert!(matches!(
        service
            .transcribe(b"slow".to_vec(), "audio/wav", &options, &token)
            .await,
        Err(Error::Deadline)
    ));
    token.cancel();
    assert!(matches!(
        service
            .transcribe(Vec::new(), "audio/wav", &options, &token)
            .await,
        Err(Error::Cancelled)
    ));
    server.abort();
}

#[tokio::test]
async fn billing_distinguishes_zero_missing_pending_denied_and_malformed_logs() {
    use axum::{extract::Path, routing::get};
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let origin = format!("http://{}", listener.local_addr().unwrap());
    let router = Router::new()
        .route(
            "/gateway/v1/projects",
            get(|| async {
                axum::Json(
                    serde_json::json!({"projects":[{"project_id":"project","name":"local"}]}),
                )
            }),
        )
        .route(
            "/empty/v1/projects",
            get(|| async { axum::Json(serde_json::json!({"projects":[]})) }),
        )
        .route(
            "/gateway/v1/projects/project/requests/{request}",
            get(
                |Path(request): Path<String>, headers: HeaderMap| async move {
                    assert_eq!(headers["authorization"], "Token fixture-key");
                    let (status, body) = match request.as_str() {
                        "positive" => (StatusCode::OK, r#"{"response":{"details":{"usd":0.125}}}"#),
                        "zero" => (StatusCode::OK, r#"{"response":{"details":{"usd":0}}}"#),
                        "negative" => (StatusCode::OK, r#"{"response":{"details":{"usd":-1}}}"#),
                        "missing" => (StatusCode::OK, r#"{"response":{"details":{}}}"#),
                        "pending" => (StatusCode::OK, "null"),
                        "no-details" => (StatusCode::OK, r#"{"response":{}}"#),
                        "no-response" => (StatusCode::OK, "{}"),
                        "denied" => (StatusCode::FORBIDDEN, "permission denied"),
                        _ => (StatusCode::OK, "{"),
                    };
                    (status, body)
                },
            ),
        );
    let server = tokio::spawn(axum::serve(listener, router).into_future());
    let service = Service::new(&format!("{origin}/gateway"), "fixture-key").unwrap();
    let cancel = CancellationToken::new();
    assert_eq!(
        service.actual_cost("positive", &cancel).await.unwrap(),
        0.125
    );
    assert_eq!(service.actual_cost("zero", &cancel).await.unwrap(), 0.0);
    for request in [
        "negative",
        "missing",
        "pending",
        "no-details",
        "no-response",
    ] {
        assert!(matches!(
            service.actual_cost(request, &cancel).await,
            Err(Error::Missing(_))
        ));
    }
    assert!(matches!(
        service.actual_cost("denied", &cancel).await,
        Err(Error::Api { status: 403, .. })
    ));
    assert!(matches!(
        service.actual_cost("malformed", &cancel).await,
        Err(Error::Json(_))
    ));
    for request in ["", ".", "..", "../escape", "x?key", "x#fragment"] {
        assert!(matches!(
            service.actual_cost(request, &cancel).await,
            Err(Error::Invalid(_))
        ));
    }
    assert!(matches!(
        Service::new(&format!("{origin}/empty/"), "fixture-key")
            .unwrap()
            .actual_cost("id", &cancel)
            .await,
        Err(Error::Missing(_))
    ));
    assert!(!format!("{service:?}").contains("fixture-key"));
    server.abort();
}

#[tokio::test]
async fn billable_redirects_are_returned_without_replaying_audio() {
    use std::sync::atomic::{AtomicUsize, Ordering};
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let service = Service::new(
        &format!("http://{}/", listener.local_addr().unwrap()),
        "fixture-key",
    )
    .unwrap();
    let replayed = Arc::new(AtomicUsize::new(0));
    let counter = replayed.clone();
    let router = Router::new()
        .route(
            "/v1/listen",
            post(|| async { (StatusCode::TEMPORARY_REDIRECT, [("location", "/replayed")]) }),
        )
        .route(
            "/replayed",
            post(move || {
                let counter = counter.clone();
                async move {
                    counter.fetch_add(1, Ordering::SeqCst);
                    "replayed"
                }
            }),
        );
    let server = tokio::spawn(axum::serve(listener, router).into_future());
    assert!(matches!(
        service
            .transcribe(
                b"audio".to_vec(),
                "",
                &RequestOptions::default(),
                &CancellationToken::new()
            )
            .await,
        Err(Error::Api { status: 307, .. })
    ));
    assert_eq!(replayed.load(Ordering::SeqCst), 0);
    server.abort();
}

#[test]
fn invalid_configuration_is_rejected_without_exposing_credentials() {
    for base in [
        "invalid",
        "file:///home",
        "https://example.test/?x",
        "https://example.test/#fragment",
        "https://user:secret@example.test/",
    ] {
        assert!(Service::new(base, "key").is_err());
    }
    for key in ["", "key\nheader"] {
        assert!(Service::new("https://api.deepgram.com", key).is_err());
    }
}
