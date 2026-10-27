use deepgram_transcription::{
    RequestOptions, Response,
    cache::{Cache, request_key, source_key},
};
use serde_json::json;

fn response() -> Response {
    serde_json::from_value(json!({"metadata":{"request_id":"request-one","duration":12.5,"channels":1,"models":["nova-model"]},"results":{"channels":[],"utterances":[{"start":0,"end":1,"speaker":2,"speaker_confidence":0.9,"transcript":"Hello"}]}})).unwrap()
}

#[test]
fn request_identity_includes_every_effective_option_and_field_boundaries() {
    let base = RequestOptions::default();
    let key = request_key(b"audio", &base).unwrap();
    for options in [
        RequestOptions {
            utterances: !base.utterances,
            ..base.clone()
        },
        RequestOptions {
            punctuate: !base.punctuate,
            ..base.clone()
        },
    ] {
        assert_ne!(key, request_key(b"audio", &options).unwrap());
    }
    let first = RequestOptions {
        model: "nova-3a".into(),
        language: "bc".into(),
        ..base.clone()
    };
    let second = RequestOptions {
        model: "nova-3ab".into(),
        language: "c".into(),
        ..base.clone()
    };
    assert_ne!(
        request_key(b"audio", &first).unwrap(),
        request_key(b"audio", &second).unwrap()
    );
    let normalized = RequestOptions {
        model: " nova-3 ".into(),
        terms: vec!["  ".into()],
        ..base.clone()
    };
    assert_eq!(key, request_key(b"audio", &normalized).unwrap());
    assert_ne!(key, request_key(b"other audio", &base).unwrap());
}

#[test]
fn raw_responses_remain_readable_but_are_not_invented_jobs() {
    let dir = tempfile::tempdir().unwrap();
    let cache = Cache::new(dir.path());
    let key = source_key(b"legacy");
    std::fs::write(
        dir.path().join(format!("{key}.json")),
        serde_json::to_vec(&response()).unwrap(),
    )
    .unwrap();
    assert_eq!(
        cache.response(&key).unwrap().metadata.request_id,
        "request-one"
    );
    assert!(cache.records().unwrap().is_empty());
    assert_eq!(cache.response_count().unwrap(), 1);
    assert!(cache.envelope("../escape").is_err());
}

#[test]
fn saving_responses_preserves_billed_history_and_history_only_is_not_a_transcript() {
    let dir = tempfile::tempdir().unwrap();
    let cache = Cache::new(dir.path());
    let key = source_key(b"job");
    let record = serde_json::from_value(json!({"request_id":"request-one","sha256":key,"source_sha256":source_key(b"source"),"filename":"talk.wav","timestamp":"2026-10-14T12:00:00Z","cost_usd":"$0.123","cost_is_actual":true})).unwrap();
    cache.save_record(record).unwrap();
    assert!(cache.response(&key).is_err());
    assert_eq!(cache.response_count().unwrap(), 0);
    cache.save_response(&key, &response()).unwrap();
    assert_eq!(cache.response_count().unwrap(), 1);
    cache.save_response(&key, &response()).unwrap();
    let stored = cache.envelope(&key).unwrap();
    cache.save_record(stored.record.clone().unwrap()).unwrap();
    assert_eq!(cache.response(&key).unwrap(), response());
    assert_eq!(stored.record.as_ref().unwrap().cost_usd, "$0.123");
    assert!(stored.record.as_ref().unwrap().cost_is_actual);
    assert_eq!(
        cache.find_source(&source_key(b"source")).unwrap().record,
        stored.record
    );
    assert_eq!(cache.find_request(" REQUEST-ONE ").unwrap().sha256, key);
    std::fs::write(dir.path().join("unrelated.json"), "{}").unwrap();
    std::fs::create_dir(
        dir.path()
            .join(format!("{}.json", source_key(b"directory"))),
    )
    .unwrap();
    assert_eq!(cache.clear().unwrap(), 1);
    assert!(dir.path().join("unrelated.json").exists());
    assert!(cache.records().unwrap().is_empty());
}

#[test]
fn content_lookup_selects_newest_record_and_never_matches_a_reused_filename() {
    let dir = tempfile::tempdir().unwrap();
    let cache = Cache::new(dir.path().join("cache"));
    let source = dir.path().join("talk.wav");
    std::fs::write(&source, b"recording one").unwrap();
    for day in [12, 14] {
        let key = source_key(format!("options {day}").as_bytes());
        let record = serde_json::from_value(json!({"request_id":format!("request-{day}"),"sha256":key,"source_sha256":source_key(b"recording one"),"filename":"talk.wav","timestamp":format!("2026-10-{day}T12:00:00Z")})).unwrap();
        cache.save_record(record).unwrap();
        cache.save_response(&key, &response()).unwrap();
    }
    assert_eq!(
        cache
            .find_target(source.to_str().unwrap())
            .unwrap()
            .request_id,
        "request-14"
    );
    assert_eq!(
        cache
            .find_source(&source_key(b"recording one"))
            .unwrap()
            .record
            .unwrap()
            .request_id,
        "request-14"
    );
    std::fs::write(&source, b"different recording").unwrap();
    assert!(cache.find_target(source.to_str().unwrap()).is_err());
    assert_eq!(
        cache.find_target("REQUEST-12").unwrap().request_id,
        "request-12"
    );
    assert!(cache.find_request("unknown").is_err());
    assert!(cache.find_source("unknown").is_err());
    let corrupt = source_key(b"corrupt");
    std::fs::write(cache.directory().join(format!("{corrupt}.json")), "{").unwrap();
    assert!(cache.save_response(&corrupt, &response()).is_err());
    assert_eq!(cache.records().unwrap().len(), 2);
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        assert_eq!(
            std::fs::metadata(cache.directory())
                .unwrap()
                .permissions()
                .mode()
                & 0o777,
            0o700
        );
        assert_eq!(
            std::fs::metadata(
                cache
                    .directory()
                    .join(format!("{}.json", source_key(b"options 14")))
            )
            .unwrap()
            .permissions()
            .mode()
                & 0o777,
            0o600
        );
    }
}

#[test]
fn cache_missing_locations_and_destructive_root_aliases_are_handled() {
    let dir = tempfile::tempdir().unwrap();
    let missing = Cache::new(dir.path().join("missing"));
    assert!(missing.records().unwrap().is_empty());
    assert_eq!(missing.clear().unwrap(), 0);
    assert!(Cache::new("").clear().is_err());
    assert!(Cache::new("/").clear().is_err());
    if let Some(home) = dirs::home_dir() {
        assert!(Cache::new(home).clear().is_err());
    }
    #[cfg(unix)]
    {
        let link = dir.path().join("alias");
        std::os::unix::fs::symlink("/", &link).unwrap();
        assert!(Cache::new(link).clear().is_err());
    }
    let file = dir.path().join("ordinary-file");
    std::fs::write(&file, "content").unwrap();
    assert!(Cache::new(&file).records().is_err());
    assert!(
        Cache::new(&file)
            .save_response(&source_key(b"x"), &response())
            .is_err()
    );
    assert!(Cache::new(file).clear().is_err());
    let options = RequestOptions {
        model: "".into(),
        language: "".into(),
        diarize_model: "".into(),
        smart_format: false,
        utterances: false,
        punctuate: false,
        terms: vec![],
    };
    assert_eq!(options.query_pairs(), [("model", "nova-3")]);
    assert!(Cache::default_directory().ends_with("deepgram-transcribe"));
}
