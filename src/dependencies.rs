use crate::{Context, Error, output};
use cli_command_kit::{Mode, clap::ArgMatches};
use cli_deps::{CancellationToken, Config, Dependency, Manager, PackageInstaller};
use std::{io::Write, sync::Arc};
pub fn manager(context: &Context) -> Result<Manager, Error> {
    let installer = Arc::new(PackageInstaller {
        package: "ffmpeg".into(),
        runner: context.runner.clone(),
    });
    let dependencies = ["ffmpeg", "ffprobe"]
        .into_iter()
        .map(|name| {
            let mut dep = Dependency::new(name, "4.4");
            dep.version_args = vec!["-version".into()];
            dep.install_url = "https://ffmpeg.org/download.html".into();
            dep.installer = Some(installer.clone());
            dep
        })
        .collect();
    Ok(Manager::new(Config {
        app_name: "deepgram-transcribe".into(),
        dependencies,
        runner: Some(context.runner.clone()),
        bin_dir: Some(context.managed_bin.clone()),
        ..Default::default()
    })?)
}
pub async fn run(
    args: &ArgMatches,
    context: &Context,
    out: &mut dyn Write,
    err: &mut dyn Write,
    mode: Mode,
    cancel: &CancellationToken,
) -> Result<(), Error> {
    let manager = manager(context)?;
    let verb = args.subcommand_name().expect("verb");
    if verb == "list" {
        let rows = manager
            .verify(cancel)
            .await?
            .into_iter()
            .map(|r| {
                vec![
                    r.name,
                    r.min_version,
                    if r.detected_version.is_empty() {
                        "none".into()
                    } else {
                        r.detected_version
                    },
                    if r.satisfied {
                        "ok"
                    } else if r.installed {
                        "outdated"
                    } else {
                        "not installed"
                    }
                    .into(),
                ]
            })
            .collect();
        output::table(
            out,
            mode,
            &["program", "needs", "installed", "status"],
            rows,
        )?;
    } else {
        let outcome = if verb == "install" {
            manager.install_unsatisfied(cancel).await?
        } else {
            manager.update_all(cancel).await?
        };
        for failure in &outcome.failures {
            output::status(
                err,
                mode,
                "WARN",
                &format!(
                    "{}: {}; install manually from https://ffmpeg.org/download.html",
                    failure.name, failure.error
                ),
            )?;
        }
        if !outcome.is_success() {
            return Err(cli_deps::Error::Batch(outcome.failures.len()).into());
        }
        output::status(
            err,
            mode,
            "OK",
            &if outcome.completed.is_empty() {
                "All dependencies already installed".into()
            } else {
                format!("Completed: {}", outcome.completed.join(", "))
            },
        )?;
    }
    Ok(())
}
