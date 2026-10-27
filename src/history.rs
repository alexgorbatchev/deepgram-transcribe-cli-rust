use crate::{Context, Error, output};
use cli_command_kit::{Mode, clap::ArgMatches};
use deepgram_transcription::{
    cache::{Cache, JobRecord},
    markdown::format_seconds,
    service::Service,
};
use futures::{StreamExt, stream};
use std::io::Write;
use tokio_util::sync::CancellationToken;
async fn refresh(
    records: Vec<JobRecord>,
    service: Option<Service>,
    cache: &Cache,
    err: &mut dyn Write,
    mode: Mode,
    cancel: &CancellationToken,
) -> Result<Vec<JobRecord>, Error> {
    let Some(service) = service else {
        return Ok(records);
    };
    let requests = records.into_iter().map(|mut record| {
        let service = service.clone();
        async move {
            let result = if record.cost_is_actual {
                None
            } else {
                Some(service.actual_cost(&record.request_id, cancel).await)
            };
            if let Some(Ok(cost)) = result.as_ref() {
                record.cost_usd = format!("${cost:.3}");
                record.cost_is_actual = true;
            }
            (record, result)
        }
    });
    let mut outcomes = stream::iter(requests).buffered(8);
    let mut records = vec![];
    while let Some((record, result)) = outcomes.next().await {
        match result {
            Some(Ok(_)) => {
                if let Err(error) = cache.save_record(record.clone()) {
                    output::status(
                        err,
                        mode,
                        "WARN",
                        &format!("Could not save billed history: {error}"),
                    )?;
                }
            }
            Some(Err(deepgram_transcription::Error::Cancelled)) => {
                return Err(deepgram_transcription::Error::Cancelled.into());
            }
            Some(Err(error)) if mode == Mode::Agent => {
                output::status(
                    err,
                    mode,
                    "INFO",
                    &format!("{} charge pending: {error}", record.request_id),
                )?;
            }
            None | Some(Err(_)) => {}
        }
        records.push(record);
    }
    Ok(records)
}
pub async fn run(
    global: &ArgMatches,
    args: &ArgMatches,
    context: &Context,
    out: &mut dyn Write,
    err: &mut dyn Write,
    mode: Mode,
    cancel: &CancellationToken,
) -> Result<(), Error> {
    let cache = context.cache(global);
    let (verb, args) = args.subcommand().expect("verb");
    let records = if verb == "inspect" {
        vec![
            cache
                .find_target(args.get_one::<String>("target").expect("required"))
                .map_err(|error| {
                    Error::Message(format!(
                        "{error}; use deepgram-transcribe job list to find a recorded request"
                    ))
                })?,
        ]
    } else {
        let mut records = cache.records()?;
        let limit = *args.get_one::<u32>("limit").expect("default");
        if limit > 0 {
            records.truncate(limit as usize);
        }
        records
    };
    if records.is_empty() {
        output::status(err, mode, "INFO", "No transcriptions recorded")?;
        return Ok(());
    }
    let records = refresh(records, context.service(global)?, &cache, err, mode, cancel).await?;
    if verb == "inspect" {
        let record = &records[0];
        let cost = if record.cost_is_actual {
            record.cost_usd.clone()
        } else if record.cost_usd.starts_with('$') {
            format!("not billed yet (estimated at {})", record.cost_usd)
        } else {
            "not billed yet".into()
        };
        output::fields(
            out,
            mode,
            vec![
                ("file", record.filename.clone()),
                ("original_path", record.filepath.clone()),
                ("request_id", record.request_id.clone()),
                (
                    "timestamp",
                    record
                        .timestamp
                        .map_or_else(|| "unknown".into(), |t| t.to_rfc3339()),
                ),
                ("duration", format_seconds(record.duration_seconds)),
                ("channels", record.channels.to_string()),
                ("model", record.model.clone()),
                ("preprocessing_requested", record.preprocessed.to_string()),
                (
                    "speaker_labels",
                    if record.diarize_model.is_empty() {
                        "not requested".into()
                    } else {
                        record.diarize_model.clone()
                    },
                ),
                ("keyterms", record.terms.join(", ")),
                ("cost", cost),
            ],
        )?;
    } else {
        let rows = records
            .iter()
            .map(|r| {
                vec![
                    r.timestamp.map_or_else(
                        || "unknown".into(),
                        |t| t.format("%Y-%m-%d %H:%M").to_string(),
                    ),
                    r.filename.clone(),
                    format_seconds(r.duration_seconds),
                    r.channels.to_string(),
                    r.model.clone(),
                    if r.cost_is_actual {
                        r.cost_usd.clone()
                    } else {
                        "not billed yet".into()
                    },
                    r.request_id.clone(),
                ]
            })
            .collect();
        output::table(
            out,
            mode,
            &[
                "date",
                "file",
                "duration",
                "channels",
                "model",
                "cost",
                "request_id",
            ],
            rows,
        )?;
        let duration = records.iter().map(|r| r.duration_seconds).sum::<f64>();
        let actual = records.iter().filter(|r| r.cost_is_actual).count();
        let billed = records
            .iter()
            .filter(|r| r.cost_is_actual)
            .filter_map(|r| {
                r.cost_usd
                    .trim()
                    .trim_start_matches('$')
                    .parse::<f64>()
                    .ok()
            })
            .filter(|v| v.is_finite() && *v >= 0.0)
            .sum::<f64>();
        output::fields(
            out,
            mode,
            vec![
                ("jobs", records.len().to_string()),
                ("total_duration", format_seconds(duration)),
                (
                    "total_billed",
                    if actual == 0 {
                        "not billed yet".into()
                    } else if actual < records.len() {
                        format!("${billed:.3} ({actual}/{} billed)", records.len())
                    } else {
                        format!("${billed:.3}")
                    },
                ),
            ],
        )?;
    }
    Ok(())
}
