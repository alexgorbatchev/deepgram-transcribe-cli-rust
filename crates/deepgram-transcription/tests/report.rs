use deepgram_transcription::{
    RequestOptions, Response,
    cost::estimate,
    markdown::{MetaInfo, format, format_seconds},
    terms::{combine, load, parse},
};
use serde_json::json;

#[test]
fn estimates_use_effective_batch_model_language_channels_and_nonempty_vocabulary() {
    for (model, language, terms, duration, channels, expected) in [
        ("", "en", vec![], 600.0, 1, Some("$0.043")),
        ("nova-3", " multi ", vec!["Rust"], 600.0, 2, Some("$0.130")),
        ("whisper-large", "en", vec![" "], 600.0, 0, Some("$0.048")),
        ("nova-2", "en", vec![], 600.0, 1, None),
        ("nova-3", "en", vec![], -1.0, 1, None),
        ("nova-3", "en", vec![], f64::NAN, 1, None),
    ] {
        let options = RequestOptions {
            model: model.into(),
            language: language.into(),
            terms: terms.into_iter().map(String::from).collect(),
            ..Default::default()
        };
        assert_eq!(
            estimate(duration, channels, &options),
            expected.map(String::from)
        );
    }
}

#[test]
fn vocabulary_combines_files_flags_and_defaults_without_losing_first_spelling() {
    let dir = tempfile::tempdir().unwrap();
    let file = dir.path().join("terms.txt");
    std::fs::write(&file, "# comment\n Rust, Go\n\nOpenAI\n").unwrap();
    let custom = parse([" go, TypeScript, ", "OpenAI"]);
    let terms = combine([load(&file).unwrap(), custom]);
    assert_eq!(terms, ["Rust", "Go", "OpenAI", "TypeScript"]);
    assert!(load(&dir.path().join("missing")).is_err());
    let combined = combine([terms, deepgram_transcription::terms::tech_terms()]);
    assert_eq!(
        combined
            .iter()
            .filter(|term| term.eq_ignore_ascii_case("go"))
            .count(),
        1
    );
    assert!(combined.iter().any(|term| term == "OpenTelemetry"));
}

#[test]
fn markdown_channel_fallback_empty_input_and_metadata_are_rendered() {
    for (text, expected) in [
        (
            "Paragraph one.\nParagraph two.",
            "Paragraph one.\nParagraph two.\n",
        ),
        (" ", "*(Empty transcript)*\n"),
    ] {
        let response = serde_json::from_value(json!({"metadata":{"request_id":"id"},"results":{"channels":[{"alternatives":[{"transcript":text,"words":null}]}]}})).unwrap();
        let meta = MetaInfo {
            filename: "file\nname".into(),
            file_size: "5 | MB".into(),
            model: "custom|model".into(),
            timestamp: Some(
                chrono::DateTime::parse_from_rfc3339("2026-10-14T12:00:00Z")
                    .unwrap()
                    .to_utc(),
            ),
            keyterms: vec!["A|B".into()],
            ..Default::default()
        };
        let out = format(Some(&response), &meta);
        assert!(out.ends_with(expected));
        assert!(out.contains("file name"));
        assert!(out.contains("5 \\| MB"));
        assert!(out.contains("2026-10-14 12:00:00 UTC"));
        assert!(out.contains("A\\|B"));
    }
    let empty =
        serde_json::from_value(json!({"metadata":{"request_id":"id"},"results":{}})).unwrap();
    assert!(format(Some(&empty), &MetaInfo::default()).ends_with("*(Empty transcript)*\n"));
    assert_eq!(format_seconds(f64::INFINITY), "unknown");
}

#[test]
fn markdown_groups_adjacent_speakers_and_preserves_unknown_speaker_identity() {
    let response: Response = serde_json::from_value(json!({"metadata":{"request_id":"id","duration":3661.9,"channels":1,"models":null},"results":{"channels":null,"utterances":[
        {"start":0,"end":2,"speaker":0,"transcript":" First. "},
        {"start":2,"end":3,"speaker":1,"transcript":" "},
        {"start":3,"end":5,"speaker":0,"transcript":"Second."},
        {"start":5,"end":6,"transcript":"Unattributed."},
        {"start":6,"end":7,"speaker":1,"transcript":"Other."}
    ]}})).unwrap();
    let meta = MetaInfo {
        filename: "talk.wav".into(),
        model: "nova-3".into(),
        diarized: true,
        keyterms: (0..17).map(|index| format!("term{index}")).collect(),
        ..Default::default()
    };
    let report = format(Some(&response), &meta);
    assert!(report.contains("# Transcript: talk.wav\n"));
    assert!(report.contains("| **Duration** | 01:01:01 |"));
    assert!(report.contains("### Speaker 0 (00:00 - 00:05)\n\nFirst.\n\nSecond."));
    assert!(report.contains("### Speaker unknown (00:05 - 00:06)"));
    assert!(report.contains("*(and 2 more)*"));
    assert_eq!(format_seconds(65.9), "01:05");
    assert_eq!(format_seconds(-1.0), "unknown");
    assert!(format(None, &MetaInfo::default()).contains("No transcript data available"));
}
