use crate::Response;
use chrono::{DateTime, Utc};
use std::fmt::Write;

#[derive(Clone, Debug, Default)]
pub struct MetaInfo {
    pub filename: String,
    pub file_size: String,
    pub model: String,
    pub diarized: bool,
    pub keyterms: Vec<String>,
    pub timestamp: Option<DateTime<Utc>>,
}

pub fn format_seconds(seconds: f64) -> String {
    let Ok(duration) = std::time::Duration::try_from_secs_f64(seconds) else {
        return "unknown".into();
    };
    let seconds = duration.as_secs();
    if seconds >= 3600 {
        format!(
            "{:02}:{:02}:{:02}",
            seconds / 3600,
            seconds / 60 % 60,
            seconds % 60
        )
    } else {
        format!("{:02}:{:02}", seconds / 60, seconds % 60)
    }
}

fn table_text(value: &str) -> String {
    value.replace('|', "\\|").replace(['\r', '\n'], " ")
}

pub fn format(response: Option<&Response>, meta: &MetaInfo) -> String {
    let mut out = format!(
        "# Transcript: {}\n\n## Metadata\n\n| Property | Value |\n| :--- | :--- |\n",
        meta.filename.replace(['\r', '\n'], " ")
    );
    if let Some(timestamp) = meta.timestamp {
        writeln!(
            out,
            "| **Date** | {} |",
            timestamp.format("%Y-%m-%d %H:%M:%S UTC")
        )
        .unwrap();
    }
    if !meta.file_size.is_empty() {
        writeln!(out, "| **File Size** | {} |", table_text(&meta.file_size)).unwrap();
    }
    if let Some(response) = response.filter(|response| response.metadata.duration > 0.0) {
        writeln!(
            out,
            "| **Duration** | {} |",
            format_seconds(response.metadata.duration)
        )
        .unwrap();
    }
    if !meta.model.is_empty() {
        writeln!(out, "| **Model** | `{}` |", table_text(&meta.model)).unwrap();
    }
    writeln!(out, "| **Diarization** | `{}` |", meta.diarized).unwrap();
    if !meta.keyterms.is_empty() {
        let mut summary = meta
            .keyterms
            .iter()
            .take(15)
            .map(|term| table_text(term))
            .collect::<Vec<_>>()
            .join(", ");
        if meta.keyterms.len() > 15 {
            write!(summary, " *(and {} more)*", meta.keyterms.len() - 15).unwrap();
        }
        writeln!(out, "| **Key Terms** | {summary} |").unwrap();
    }
    out.push_str("\n---\n\n## Transcript\n\n");
    let Some(response) = response else {
        out.push_str("*(No transcript data available)*\n");
        return out;
    };
    let utterances: Vec<_> = response
        .results
        .utterances
        .iter()
        .flatten()
        .filter(|utterance| !utterance.transcript.trim().is_empty())
        .collect();
    if utterances.is_empty() {
        let text = response
            .results
            .channels
            .first()
            .and_then(|channel| channel.alternatives.first())
            .map(|alternative| alternative.transcript.as_str())
            .unwrap_or("");
        if text.trim().is_empty() {
            out.push_str("*(Empty transcript)*\n");
        } else {
            out.push_str(text);
            out.push('\n');
        }
        return out;
    }
    let mut start = 0;
    while start < utterances.len() {
        let speaker = utterances[start].speaker;
        let mut end = start + 1;
        while end < utterances.len() && utterances[end].speaker == speaker {
            end += 1;
        }
        let label = speaker
            .map(|speaker| speaker.to_string())
            .unwrap_or_else(|| "unknown".into());
        writeln!(
            out,
            "### Speaker {label} ({} - {})\n",
            format_seconds(utterances[start].start),
            format_seconds(utterances[end - 1].end)
        )
        .unwrap();
        for utterance in &utterances[start..end] {
            writeln!(out, "{}\n", utterance.transcript.trim()).unwrap();
        }
        start = end;
    }
    out
}
