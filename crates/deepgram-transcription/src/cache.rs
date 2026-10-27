use crate::{Error, RequestOptions, Response, io};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::{
    fs,
    io::Write,
    path::{Path, PathBuf},
};

pub fn source_key(audio: &[u8]) -> String {
    format!("{:x}", Sha256::digest(audio))
}

/// Hash a structured, complete effective request rather than ambiguous concatenation.
pub fn request_key(audio: &[u8], options: &RequestOptions) -> Result<String, Error> {
    let mut terms = options.keyterms();
    terms.sort_unstable();
    let encoded = serde_json::to_vec(&(
        "deepgram-request-v2",
        source_key(audio),
        options.effective_model(),
        options.language.trim(),
        &options.diarize_model,
        options.smart_format,
        options.utterances,
        options.punctuate,
        terms,
    ))?;
    Ok(source_key(&encoded))
}

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct JobRecord {
    pub request_id: String,
    pub filename: String,
    pub filepath: String,
    pub source_sha256: String,
    pub sha256: String,
    pub timestamp: Option<DateTime<Utc>>,
    pub duration_seconds: f64,
    pub channels: u32,
    pub model: String,
    pub diarize_model: String,
    pub preprocessed: bool,
    pub terms: Vec<String>,
    pub cost_usd: String,
    pub cost_is_actual: bool,
}

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct Envelope {
    pub record: Option<JobRecord>,
    pub response: Option<Response>,
}

#[derive(Clone, Debug)]
pub struct Cache {
    directory: PathBuf,
}

impl Cache {
    pub fn new(directory: impl Into<PathBuf>) -> Self {
        Self {
            directory: directory.into(),
        }
    }
    pub fn directory(&self) -> &Path {
        &self.directory
    }
    pub fn default_directory() -> PathBuf {
        std::env::var_os("XDG_CACHE_HOME")
            .filter(|value| !value.is_empty())
            .map(PathBuf::from)
            .or_else(|| dirs::home_dir().map(|home| home.join(".cache")))
            .unwrap_or_else(|| PathBuf::from(".cache"))
            .join("deepgram-transcribe")
    }

    fn path(&self, key: &str) -> Result<PathBuf, Error> {
        if !valid_key(key) {
            return Err(Error::Invalid(
                "cache key must be 64 lowercase hexadecimal characters".into(),
            ));
        }
        Ok(self.directory.join(format!("{key}.json")))
    }

    pub fn envelope(&self, key: &str) -> Result<Envelope, Error> {
        let path = self.path(key)?;
        let data =
            fs::read(&path).map_err(|error| io(format!("reading {}", path.display()), error))?;
        let value: serde_json::Value = serde_json::from_slice(&data)?;
        if value.get("metadata").is_some() && value.get("results").is_some() {
            return Ok(Envelope {
                record: None,
                response: Some(serde_json::from_value(value)?),
            });
        }
        let mut envelope: Envelope = serde_json::from_value(value)?;
        if let Some(record) = &mut envelope.record
            && record.sha256.is_empty()
        {
            record.sha256 = key.into();
        }
        Ok(envelope)
    }

    pub fn response(&self, key: &str) -> Result<Response, Error> {
        self.envelope(key)?
            .response
            .ok_or_else(|| Error::Missing(key.into()))
    }

    pub fn save(&self, key: &str, envelope: &Envelope) -> Result<(), Error> {
        let path = self.path(key)?;
        let mut builder = fs::DirBuilder::new();
        builder.recursive(true);
        #[cfg(unix)]
        {
            use std::os::unix::fs::DirBuilderExt;
            builder.mode(0o700);
        }
        builder
            .create(&self.directory)
            .map_err(|error| io("creating transcription cache", error))?;
        let mut file = tempfile::NamedTempFile::new_in(&self.directory)
            .map_err(|error| io("creating cache transaction", error))?;
        file.write_all(&serde_json::to_vec_pretty(envelope)?)
            .map_err(|error| io("writing cache transaction", error))?;
        file.as_file()
            .sync_all()
            .map_err(|error| io("syncing cache transaction", error))?;
        file.persist(&path)
            .map_err(|error| io(format!("publishing {}", path.display()), error.error))?;
        Ok(())
    }

    fn existing(&self, key: &str) -> Result<Envelope, Error> {
        match self.envelope(key) {
            Err(Error::Io { source, .. }) if source.kind() == std::io::ErrorKind::NotFound => {
                Ok(Envelope::default())
            }
            result => result,
        }
    }

    pub fn save_response(&self, key: &str, response: &Response) -> Result<(), Error> {
        let mut envelope = self.existing(key)?;
        envelope.response = Some(response.clone());
        self.save(key, &envelope)
    }

    pub fn save_record(&self, record: JobRecord) -> Result<(), Error> {
        let key = record.sha256.clone();
        let mut envelope = self.existing(&key)?;
        envelope.record = Some(record);
        self.save(&key, &envelope)
    }

    fn entries(&self) -> Result<Vec<String>, Error> {
        let entries = match fs::read_dir(&self.directory) {
            Ok(entries) => entries,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(Vec::new()),
            Err(error) => return Err(io("reading transcription cache", error)),
        };
        let mut keys = Vec::new();
        for entry in entries {
            let entry = entry.map_err(|error| io("reading cache entry", error))?;
            if !entry
                .file_type()
                .map_err(|error| io("reading cache entry type", error))?
                .is_file()
            {
                continue;
            }
            if let Some(key) = entry
                .file_name()
                .to_str()
                .and_then(|name| name.strip_suffix(".json"))
                && valid_key(key)
            {
                keys.push(key.to_owned());
            }
        }
        keys.sort();
        Ok(keys)
    }

    pub fn records(&self) -> Result<Vec<JobRecord>, Error> {
        let mut records: Vec<_> = self
            .entries()?
            .iter()
            .filter_map(|key| self.envelope(key).ok()?.record)
            .collect();
        records.sort_by(|left, right| {
            right
                .timestamp
                .cmp(&left.timestamp)
                .then_with(|| left.sha256.cmp(&right.sha256))
        });
        Ok(records)
    }

    pub fn response_count(&self) -> Result<usize, Error> {
        Ok(self
            .entries()?
            .iter()
            .filter(|key| {
                self.envelope(key)
                    .is_ok_and(|entry| entry.response.is_some())
            })
            .count())
    }

    pub fn find_source(&self, source: &str) -> Result<Envelope, Error> {
        for record in self.records()? {
            if record.source_sha256 == source || record.sha256 == source {
                let envelope = self.envelope(&record.sha256)?;
                if envelope.response.is_some() {
                    return Ok(envelope);
                }
            }
        }
        Err(Error::Missing(source.into()))
    }

    pub fn find_request(&self, request: &str) -> Result<JobRecord, Error> {
        self.records()?
            .into_iter()
            .find(|record| record.request_id.eq_ignore_ascii_case(request.trim()))
            .ok_or_else(|| Error::Missing(request.into()))
    }

    pub fn find_target(&self, target: &str) -> Result<JobRecord, Error> {
        if !Path::new(target).is_file() {
            return self.find_request(target);
        }
        let data = fs::read(target).map_err(|error| io(format!("reading {target}"), error))?;
        let key = source_key(&data);
        self.records()?
            .into_iter()
            .find(|record| record.source_sha256 == key || record.sha256 == key)
            .ok_or_else(|| Error::Missing(target.into()))
    }

    pub fn clear(&self) -> Result<usize, Error> {
        if self.directory.as_os_str().is_empty() {
            return Err(Error::Invalid("no cache directory was given".into()));
        }
        let canonical = match self.directory.canonicalize() {
            Ok(path) => path,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(0),
            Err(error) => return Err(io("resolving transcription cache", error)),
        };
        if canonical.parent().is_none()
            || dirs::home_dir()
                .and_then(|home| home.canonicalize().ok())
                .as_ref()
                == Some(&canonical)
        {
            return Err(Error::Invalid(format!(
                "refusing to clear {}",
                canonical.display()
            )));
        }
        let mut removed = 0;
        for key in self.entries()? {
            fs::remove_file(self.path(&key)?)
                .map_err(|error| io("removing cached transcription", error))?;
            removed += 1;
        }
        Ok(removed)
    }
}

fn valid_key(key: &str) -> bool {
    key.len() == 64
        && key
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
}
