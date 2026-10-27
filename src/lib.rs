pub mod args;
mod dependencies;
mod history;
mod output;
mod transcript;
use cli_command_kit::{
    Mode,
    clap::{ArgMatches, error::ErrorKind},
};
use cli_deps::{CommandRunner, ProcessRunner};
use deepgram_transcription::{cache::Cache, service::Service};
use std::{ffi::OsString, io::Write, path::PathBuf, sync::Arc};
use tokio_util::sync::CancellationToken;

#[derive(Clone)]
pub struct Context {
    pub default_cache: PathBuf,
    pub managed_bin: PathBuf,
    pub temporary_audio: PathBuf,
    pub endpoint: String,
    pub api_key: String,
    pub runner: Arc<dyn CommandRunner>,
    pub now: chrono::DateTime<chrono::Utc>,
}
impl Context {
    pub fn from_env() -> Result<Self, Error> {
        let managed_bin = cli_deps::managed_bin_dir("deepgram-transcribe")?;
        let path = cli_deps::managed_path(&managed_bin, std::env::var_os("PATH").as_deref())?;
        Ok(Self {
            default_cache: Cache::default_directory(),
            managed_bin,
            temporary_audio: PathBuf::from(".tmp/audio_preprocess"),
            endpoint: "https://api.deepgram.com".into(),
            api_key: std::env::var("DEEPGRAM_API_KEY").unwrap_or_default(),
            runner: Arc::new(ProcessRunner { path: Some(path) }),
            now: chrono::Utc::now(),
        })
    }
    fn cache(&self, matches: &ArgMatches) -> Cache {
        Cache::new(
            matches
                .get_one::<PathBuf>("cache-dir")
                .filter(|p| !p.as_os_str().is_empty())
                .unwrap_or(&self.default_cache),
        )
    }
    fn service(&self, matches: &ArgMatches) -> Result<Option<Service>, Error> {
        let key = matches
            .get_one::<String>("api-key")
            .filter(|key| !key.is_empty())
            .unwrap_or(&self.api_key);
        if key.is_empty() {
            Ok(None)
        } else {
            Ok(Some(Service::new(&self.endpoint, key)?))
        }
    }
}
#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error(transparent)]
    Transcription(#[from] deepgram_transcription::Error),
    #[error(transparent)]
    Dependency(#[from] cli_deps::Error),
    #[error(transparent)]
    Io(#[from] std::io::Error),
    #[error("{0}")]
    Message(String),
}
pub async fn entry(
    argv: impl IntoIterator<Item = OsString>,
    context: impl FnOnce() -> Result<Context, Error>,
    mut out: &mut dyn Write,
    err: &mut dyn Write,
    mode: Mode,
    cancel: &CancellationToken,
) -> u8 {
    let mut command = args::command(mode);
    let matches = match command.clone().try_get_matches_from(argv) {
        Ok(matches) => matches,
        Err(error) => {
            let info = matches!(
                error.kind(),
                ErrorKind::DisplayHelp | ErrorKind::DisplayVersion
            );
            let target: &mut dyn Write = if info { &mut *out } else { &mut *err };
            return if write!(
                target,
                "{}",
                cli_command_kit::display_message(&error.to_string())
            )
            .is_err()
            {
                1
            } else if info {
                0
            } else {
                1
            };
        }
    };
    let result = if matches.get_flag("version") {
        writeln!(out, "{}", env!("CARGO_PKG_VERSION")).map_err(Into::into)
    } else if matches.subcommand_name() == Some("skill") {
        out.write_all(args::SKILL.as_bytes()).map_err(Into::into)
    } else if let Some((name, sub)) = matches.subcommand()
        && sub.subcommand().is_none()
        && matches!(
            name,
            "transcript" | "job" | "cache" | "dependency" | "completion"
        )
    {
        command
            .find_subcommand_mut(name)
            .expect("declared group")
            .write_long_help(&mut out)
            .map_err(Into::into)
    } else if let Some(("completion", sub)) = matches.subcommand() {
        let (shell, options) = sub.subcommand().expect("validated shell");
        if options.get_flag("no-descriptions") {
            strip_descriptions(&mut command);
        }
        let shell: clap_complete::Shell = shell.parse().expect("declared completion shell");
        let mut completion = Vec::new();
        clap_complete::generate(shell, &mut command, "deepgram-transcribe", &mut completion);
        out.write_all(&completion).map_err(Into::into)
    } else if matches.subcommand_name().is_none() {
        command.write_long_help(&mut out).map_err(Into::into)
    } else {
        match context() {
            Ok(context) => run(&matches, &context, out, err, mode, cancel).await,
            Err(error) => Err(error),
        }
    };
    match result {
        Ok(()) => 0,
        Err(error) => {
            let code = if matches!(
                error,
                Error::Transcription(deepgram_transcription::Error::Cancelled)
                    | Error::Dependency(cli_deps::Error::Cancelled)
            ) {
                130
            } else {
                1
            };
            let _ = output::status(err, mode, "ERR", &error.to_string());
            code
        }
    }
}
fn strip_descriptions(command: &mut cli_command_kit::clap::Command) {
    *command = command
        .clone()
        .about("")
        .long_about("")
        .mut_args(|arg| arg.help("").long_help(""));
    for child in command.get_subcommands_mut() {
        strip_descriptions(child);
    }
}
async fn run(
    matches: &ArgMatches,
    context: &Context,
    out: &mut dyn Write,
    err: &mut dyn Write,
    mode: Mode,
    cancel: &CancellationToken,
) -> Result<(), Error> {
    if cancel.is_cancelled() {
        return Err(deepgram_transcription::Error::Cancelled.into());
    }
    match matches.subcommand().expect("domain command") {
        ("transcript", sub) => {
            transcript::create(
                matches,
                sub.subcommand().expect("verb").1,
                context,
                out,
                err,
                mode,
                cancel,
            )
            .await
        }
        ("job", sub) => history::run(matches, sub, context, out, err, mode, cancel).await,
        ("dependency", sub) => dependencies::run(sub, context, out, err, mode, cancel).await,
        ("cache", sub) => {
            let cache = context.cache(matches);
            match sub.subcommand_name().expect("verb") {
                "status" => output::fields(
                    out,
                    mode,
                    vec![
                        (
                            "cache_directory",
                            cli_command_kit::display_path(cache.directory()),
                        ),
                        ("cached_responses", cache.response_count()?.to_string()),
                    ],
                )?,
                "clear" => output::status(
                    err,
                    mode,
                    "OK",
                    &format!("Removed {} cached files", cache.clear()?),
                )?,
                _ => unreachable!(),
            };
            Ok(())
        }
        _ => unreachable!(),
    }
}
