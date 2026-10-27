use crate::{Context, Error, dependencies, output};
use cli_command_kit::{Mode, clap::ArgMatches};
use deepgram_transcription::{
    RequestOptions,
    audio::{self, PreprocessOptions, Selection},
    cache::{Envelope, JobRecord, request_key, source_key},
    cost,
    markdown::{self, MetaInfo},
    terms,
};
use std::{io::Write, path::PathBuf};
use tokio_util::sync::CancellationToken;
fn flag(args: &ArgMatches, name: &str) -> bool {
    args.get_flag(name)
}
pub async fn create(
    global: &ArgMatches,
    args: &ArgMatches,
    context: &Context,
    out: &mut dyn Write,
    err: &mut dyn Write,
    mode: Mode,
    cancel: &CancellationToken,
) -> Result<(), Error> {
    if cancel.is_cancelled() {
        return Err(deepgram_transcription::Error::Cancelled.into());
    }
    let input = args.get_one::<PathBuf>("audio-file").expect("required");
    let source = std::fs::read(input)?;
    let mut vocabulary = vec![];
    if !flag(args, "no-tech-terms") {
        vocabulary.push(terms::tech_terms());
    }
    if let Some(raw) = args.get_many::<String>("term") {
        vocabulary.push(terms::parse(raw.map(String::as_str)));
    }
    if let Some(path) = args
        .get_one::<PathBuf>("terms-file")
        .filter(|p| !p.as_os_str().is_empty())
    {
        vocabulary.push(terms::load(path)?);
    }
    let options = RequestOptions {
        model: args.get_one::<String>("model").expect("default").clone(),
        language: args.get_one::<String>("language").expect("default").clone(),
        diarize_model: if flag(args, "no-diarize") {
            String::new()
        } else {
            "latest".into()
        },
        terms: terms::combine(vocabulary),
        ..Default::default()
    };
    let cache = context.cache(global);
    let key = request_key(&source, &options)?;
    let source_key = source_key(&source);
    let cache_enabled = !flag(args, "no-cache");
    let mut envelope = None;
    if cache_enabled && !flag(args, "force") {
        envelope = cache
            .envelope(&key)
            .ok()
            .filter(|entry| entry.response.is_some())
            .or_else(|| cache.find_source(&source_key).ok());
        if envelope.is_some() {
            output::status(
                err,
                mode,
                "INFO",
                "Serving cached audio; --force requests another billable transcription",
            )?;
        }
    }
    let cached = envelope.is_some();
    let mut prepared = None;
    if envelope.is_none() {
        let service = context.service(global)?.ok_or_else(|| {
            Error::Message("DEEPGRAM_API_KEY is required for a fresh transcription".into())
        })?;
        let processing = PreprocessOptions {
            mono: !flag(args, "no-preprocess") && !flag(args, "no-mono"),
            trim_silence: !flag(args, "no-preprocess") && !flag(args, "no-trim-silence"),
            silence_threshold: args
                .get_one::<String>("silence-threshold")
                .expect("default")
                .clone(),
            silence_duration: args
                .get_one::<String>("silence-duration")
                .expect("default")
                .clone(),
        };
        if processing.mono || processing.trim_silence {
            let reports = dependencies::manager(context)?.verify(cancel).await?;
            let unusable = reports
                .iter()
                .filter(|report| !report.satisfied)
                .map(|report| report.name.as_str())
                .collect::<Vec<_>>();
            if unusable.is_empty() {
                let result = audio::prepare(
                    input,
                    &processing,
                    &context.temporary_audio,
                    context.runner.as_ref(),
                    cancel,
                )
                .await?;
                match &result.selection {
                    Selection::Shorter {
                        original,
                        processed,
                    } => {
                        output::status(
                            err,
                            mode,
                            "INFO",
                            &format!(
                                "Prepared duration {processed:.3}s is shorter than {original:.3}s"
                            ),
                        )?;
                        report_size(
                            err,
                            mode,
                            std::fs::metadata(result.path())?.len(),
                            source.len() as u64,
                        )?;
                    }
                    Selection::NotShorter { .. } => output::status(
                        err,
                        mode,
                        "WARN",
                        "Prepared duration is not shorter; uploading original audio",
                    )?,
                    Selection::Unmeasured { reason } => output::status(
                        err,
                        mode,
                        "WARN",
                        &format!("Cannot measure durations; uploading original audio: {reason}"),
                    )?,
                    Selection::Disabled => {}
                }
                prepared = Some(result);
            } else {
                output::status(
                    err,
                    mode,
                    "WARN",
                    &format!(
                        "{} unavailable or outdated; uploading original audio. Run deepgram-transcribe dependency install",
                        unusable.join(", ")
                    ),
                )?;
            }
        }
        let path = prepared
            .as_ref()
            .map_or(input.as_path(), |audio| audio.path());
        let bytes = if path == input {
            source.clone()
        } else {
            std::fs::read(path)?
        };
        output::status(err, mode, "INFO", "Uploading audio for transcription")?;
        let response = service
            .transcribe(bytes, audio::mime_type(path), &options, cancel)
            .await?;
        let record = JobRecord {
            request_id: response.metadata.request_id.clone(),
            filename: input
                .file_name()
                .unwrap_or_default()
                .to_string_lossy()
                .into_owned(),
            filepath: std::path::absolute(input)?.display().to_string(),
            source_sha256: source_key,
            sha256: key.clone(),
            timestamp: Some(context.now),
            duration_seconds: response.metadata.duration,
            channels: response.metadata.channels,
            model: options.effective_model().into(),
            diarize_model: options.diarize_model.clone(),
            preprocessed: processing.mono || processing.trim_silence,
            terms: options.terms.clone(),
            cost_usd: cost::estimate(
                response.metadata.duration,
                response.metadata.channels,
                &options,
            )
            .unwrap_or_else(|| "unknown".into()),
            cost_is_actual: false,
        };
        let fresh = Envelope {
            record: Some(record),
            response: Some(response),
        };
        if cache_enabled && let Err(error) = cache.save(&key, &fresh) {
            output::status(
                err,
                mode,
                "WARN",
                &format!("Could not save transcription history: {error}"),
            )?;
        }
        envelope = Some(fresh);
    }
    let envelope = envelope.expect("cached or transcribed");
    let record = envelope.record.as_ref();
    // A legacy raw response has no recorded options; do not claim the current request produced it.
    let meta = MetaInfo {
        filename: input
            .file_name()
            .unwrap_or_default()
            .to_string_lossy()
            .into_owned(),
        file_size: format!("{} B", source.len()),
        model: record
            .map(|r| r.model.clone())
            .unwrap_or_else(|| "unknown (raw cached response)".into()),
        diarized: record.is_some_and(|r| !r.diarize_model.is_empty()),
        keyterms: record.map(|r| r.terms.clone()).unwrap_or_default(),
        timestamp: record.and_then(|r| r.timestamp),
    };
    let markdown = markdown::format(envelope.response.as_ref(), &meta);
    if let Some(path) = args
        .get_one::<PathBuf>("output")
        .filter(|p| !p.as_os_str().is_empty())
    {
        if let Some(parent) = path.parent().filter(|p| !p.as_os_str().is_empty()) {
            std::fs::create_dir_all(parent)?;
        }
        std::fs::write(path, markdown)?;
        output::status(
            err,
            mode,
            "OK",
            &format!(
                "Saved transcript to {}",
                cli_command_kit::display_path(path)
            ),
        )?;
    } else {
        out.write_all(markdown.as_bytes())?;
    }
    if !cached {
        output::status(
            err,
            mode,
            "INFO",
            &format!(
                "Estimated charge: {}",
                record.map_or("unknown", |r| r.cost_usd.as_str())
            ),
        )?;
    }
    drop(prepared);
    Ok(())
}
fn report_size(
    err: &mut dyn Write,
    mode: Mode,
    processed: u64,
    original: u64,
) -> Result<(), Error> {
    output::status(
        err,
        mode,
        "INFO",
        &match processed.cmp(&original) {
            std::cmp::Ordering::Less => format!("Upload is {processed} B, down from {original} B"),
            std::cmp::Ordering::Greater => format!("Upload is {processed} B, up from {original} B"),
            std::cmp::Ordering::Equal => {
                format!("Upload is {processed} B, unchanged from the original")
            }
        },
    )?;
    Ok(())
}
