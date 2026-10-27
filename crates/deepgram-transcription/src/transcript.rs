use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::BTreeMap;

/// Persisted API response. Unmodeled provider fields survive cache rewrites.
/// Optional speaker labels remain absent when diarization did not run.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Response {
    pub metadata: Metadata,
    pub results: Results,
    #[serde(flatten)]
    pub extra: BTreeMap<String, Value>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Metadata {
    pub request_id: String,
    #[serde(default)]
    pub duration: f64,
    #[serde(default)]
    pub channels: u32,
    #[serde(default, deserialize_with = "null_vec")]
    pub models: Vec<String>,
    #[serde(default)]
    pub created: Option<String>,
    #[serde(flatten)]
    pub extra: BTreeMap<String, Value>,
}

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct Results {
    #[serde(default, deserialize_with = "null_vec")]
    pub channels: Vec<Channel>,
    #[serde(default)]
    pub utterances: Option<Vec<Utterance>>,
    #[serde(flatten)]
    pub extra: BTreeMap<String, Value>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Channel {
    #[serde(default, deserialize_with = "null_vec")]
    pub alternatives: Vec<Alternative>,
    #[serde(flatten)]
    pub extra: BTreeMap<String, Value>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Alternative {
    pub transcript: String,
    #[serde(default)]
    pub confidence: Option<f64>,
    #[serde(default, deserialize_with = "null_vec")]
    pub words: Vec<Word>,
    #[serde(flatten)]
    pub extra: BTreeMap<String, Value>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Utterance {
    pub start: f64,
    pub end: f64,
    pub transcript: String,
    #[serde(default)]
    pub speaker: Option<u32>,
    #[serde(default)]
    pub speaker_confidence: Option<f64>,
    #[serde(default)]
    pub confidence: Option<f64>,
    #[serde(default)]
    pub channel: Option<u32>,
    #[serde(default, deserialize_with = "null_vec")]
    pub words: Vec<Word>,
    #[serde(flatten)]
    pub extra: BTreeMap<String, Value>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Word {
    pub word: String,
    pub start: f64,
    pub end: f64,
    #[serde(default)]
    pub speaker: Option<u32>,
    #[serde(default)]
    pub speaker_confidence: Option<f64>,
    #[serde(default)]
    pub confidence: Option<f64>,
    #[serde(default)]
    pub punctuated_word: Option<String>,
    #[serde(flatten)]
    pub extra: BTreeMap<String, Value>,
}

fn null_vec<'de, D, T>(deserializer: D) -> Result<Vec<T>, D::Error>
where
    D: serde::Deserializer<'de>,
    T: Deserialize<'de>,
{
    Ok(Option::<Vec<T>>::deserialize(deserializer)?.unwrap_or_default())
}
