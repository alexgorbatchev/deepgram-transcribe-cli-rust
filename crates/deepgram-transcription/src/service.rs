use crate::{Error, RequestOptions, Response};
use reqwest::Url;
use serde::Deserialize;
use std::{future::Future, time::Duration};
use tokio_util::sync::CancellationToken;

/// Deepgram's prerecorded and billing operations with a single-attempt policy.
#[derive(Clone, Debug)]
pub struct Service {
    http: reqwest::Client,
    base_url: Url,
    pub timeout: Duration,
}

impl Service {
    pub fn new(base_url: &str, api_key: &str) -> Result<Self, Error> {
        if api_key.is_empty() {
            return Err(Error::Invalid(
                "DEEPGRAM_API_KEY is not set or empty".into(),
            ));
        }
        let mut base_url = Url::parse(base_url)
            .map_err(|error| Error::Invalid(format!("invalid Deepgram URL: {error}")))?;
        if !matches!(base_url.scheme(), "http" | "https")
            || base_url.host_str().is_none()
            || base_url.query().is_some()
            || base_url.fragment().is_some()
            || !base_url.username().is_empty()
            || base_url.password().is_some()
        {
            return Err(Error::Invalid(
                "Deepgram base URL must be an HTTP origin or path prefix".into(),
            ));
        }
        if !base_url.path().ends_with('/') {
            base_url.set_path(&format!("{}/", base_url.path()));
        }
        let mut auth = reqwest::header::HeaderValue::from_str(&format!("Token {api_key}"))
            .map_err(|_| {
                Error::Invalid("Deepgram API key contains invalid HTTP header characters".into())
            })?;
        auth.set_sensitive(true);
        let mut headers = reqwest::header::HeaderMap::new();
        headers.insert(reqwest::header::AUTHORIZATION, auth);
        let http = reqwest::Client::builder()
            .default_headers(headers)
            .redirect(reqwest::redirect::Policy::none())
            .retry(reqwest::retry::never())
            .build()?;
        Ok(Self {
            http,
            base_url,
            timeout: Duration::from_secs(300),
        })
    }

    fn endpoint(&self, path: &str) -> Result<Url, Error> {
        self.base_url
            .join(path)
            .map_err(|error| Error::Invalid(format!("invalid Deepgram endpoint: {error}")))
    }

    async fn send<T: serde::de::DeserializeOwned>(
        &self,
        request: reqwest::RequestBuilder,
        cancel: &CancellationToken,
    ) -> Result<T, Error> {
        controlled(self.timeout, cancel, async {
            let response = request.send().await?;
            let status = response.status();
            let body = response.bytes().await?;
            if status != reqwest::StatusCode::OK {
                return Err(Error::Api {
                    status: status.as_u16(),
                    body: String::from_utf8_lossy(&body).into_owned(),
                });
            }
            Ok(serde_json::from_slice(&body)?)
        })
        .await
    }

    pub async fn transcribe(
        &self,
        audio: Vec<u8>,
        mime: &str,
        options: &RequestOptions,
        cancel: &CancellationToken,
    ) -> Result<Response, Error> {
        let mime = if mime.is_empty() {
            "application/octet-stream"
        } else {
            mime
        };
        let request = self
            .http
            .post(self.endpoint("v1/listen")?)
            .query(&options.query_pairs())
            .header(reqwest::header::CONTENT_TYPE, mime)
            .body(audio);
        self.send(request, cancel).await
    }

    pub async fn actual_cost(
        &self,
        request_id: &str,
        cancel: &CancellationToken,
    ) -> Result<f64, Error> {
        if request_id.trim().is_empty()
            || matches!(request_id, "." | "..")
            || request_id.contains(['/', '?', '#'])
        {
            return Err(Error::Invalid(
                "request ID is empty or contains a URL delimiter".into(),
            ));
        }
        let projects: Projects = self
            .send(self.http.get(self.endpoint("v1/projects")?), cancel)
            .await?;
        let project = projects
            .projects
            .first()
            .ok_or_else(|| Error::Missing("Deepgram project for this API key".into()))?;
        let mut endpoint = self.endpoint("v1/projects/")?;
        endpoint
            .path_segments_mut()
            .map_err(|_| Error::Invalid("invalid projects endpoint".into()))?
            .pop_if_empty()
            .push(&project.project_id)
            .push("requests")
            .push(request_id);
        let details: Option<BilledRequest> = self.send(self.http.get(endpoint), cancel).await?;
        details
            .and_then(|request| request.response)
            .and_then(|response| response.details)
            .and_then(|details| details.usd)
            .filter(|value| value.is_finite() && *value >= 0.0)
            .ok_or_else(|| {
                Error::Missing(
                    "actual billed cost; request logging may be pending or disabled".into(),
                )
            })
    }
}

async fn controlled<T>(
    timeout: Duration,
    cancel: &CancellationToken,
    future: impl Future<Output = Result<T, Error>>,
) -> Result<T, Error> {
    tokio::select! {
        biased;
        () = cancel.cancelled() => Err(Error::Cancelled),
        result = tokio::time::timeout(timeout, future) => result.map_err(|_| Error::Deadline)?,
    }
}

#[derive(Deserialize)]
struct Projects {
    projects: Vec<Project>,
}
#[derive(Deserialize)]
struct Project {
    project_id: String,
}
#[derive(Deserialize)]
struct BilledRequest {
    response: Option<BilledResponse>,
}
#[derive(Deserialize)]
struct BilledResponse {
    details: Option<BilledDetails>,
}
#[derive(Deserialize)]
struct BilledDetails {
    usd: Option<f64>,
}
