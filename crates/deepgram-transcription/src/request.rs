use serde::{Deserialize, Serialize};

/// The options exposed by the transcription workflow, including the current diarizer.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct RequestOptions {
    pub model: String,
    pub language: String,
    pub diarize_model: String,
    pub smart_format: bool,
    pub utterances: bool,
    pub punctuate: bool,
    pub terms: Vec<String>,
}

impl Default for RequestOptions {
    fn default() -> Self {
        Self {
            model: "nova-3".into(),
            language: "en".into(),
            diarize_model: "latest".into(),
            smart_format: true,
            utterances: true,
            punctuate: true,
            terms: Vec::new(),
        }
    }
}

impl RequestOptions {
    pub fn effective_model(&self) -> &str {
        match self.model.trim() {
            "" => "nova-3",
            model => model,
        }
    }

    pub fn keyterms(&self) -> Vec<&str> {
        self.terms
            .iter()
            .map(|term| term.trim())
            .filter(|term| !term.is_empty())
            .collect()
    }

    pub fn query_pairs(&self) -> Vec<(&str, &str)> {
        let mut pairs = vec![("model", self.effective_model())];
        if !self.language.trim().is_empty() {
            pairs.push(("language", self.language.trim()));
        }
        if !self.diarize_model.is_empty() {
            pairs.push(("diarize_model", &self.diarize_model));
        }
        for (key, enabled) in [
            ("smart_format", self.smart_format),
            ("utterances", self.utterances),
            ("punctuate", self.punctuate),
        ] {
            if enabled {
                pairs.push((key, "true"));
            }
        }
        let term_key = if self.effective_model().starts_with("nova-3")
            || self.effective_model().starts_with("flux")
        {
            "keyterm"
        } else {
            "keywords"
        };
        pairs.extend(self.keyterms().into_iter().map(|term| (term_key, term)));
        pairs
    }
}
