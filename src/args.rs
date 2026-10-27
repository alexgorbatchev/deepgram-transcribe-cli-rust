use cli_command_kit::{
    HelpOptions, Mode, TreeOptions,
    clap::{Arg, ArgAction, Command, value_parser},
};
use std::path::PathBuf;

pub const SKILL: &str = include_str!("../skills/deepgram-transcribe/SKILL.md");
pub fn help_options() -> HelpOptions {
    let width = cli_command_kit::terminal_width();
    HelpOptions {
        skill: Some(SKILL.into()),
        tree: TreeOptions {
            terminal_width: Some(if width == 0 { 100 } else { width }),
            ..Default::default()
        },
        ..Default::default()
    }
}
fn boolean(name: &'static str, help: &'static str) -> Arg {
    Arg::new(name)
        .long(name)
        .action(ArgAction::Set)
        .num_args(0..=1)
        .require_equals(true)
        .default_missing_value("true")
        .default_value("false")
        .value_parser(value_parser!(bool))
        .help(help)
}
fn text(name: &'static str, default: &'static str, help: &'static str) -> Arg {
    Arg::new(name).long(name).default_value(default).help(help)
}
pub fn command(mode: Mode) -> Command {
    let mut create = Command::new("create")
        .about("Transcribe an audio file to Markdown")
        .arg(
            Arg::new("audio-file")
                .value_name("audio-file")
                .required(true)
                .value_parser(value_parser!(PathBuf))
                .help("Audio recording to upload"),
        )
        .arg(
            Arg::new("term")
                .long("term")
                .short('t')
                .action(ArgAction::Append)
                .help("Additional vocabulary; repeat or comma separate"),
        )
        .arg(
            Arg::new("terms-file")
                .long("terms-file")
                .value_parser(value_parser!(PathBuf))
                .help("Additional vocabulary file; one term per line"),
        )
        .arg(text("model", "nova-3", "Deepgram model").short('m'))
        .arg(text("language", "en", "Audio language").short('l'))
        .arg(
            Arg::new("output")
                .long("output")
                .short('o')
                .value_parser(value_parser!(PathBuf))
                .help("Write Markdown to this file; omit for stdout"),
        )
        .arg(boolean("force", "Request a fresh billable transcription").short('f'))
        .arg(
            text(
                "silence-threshold",
                "-30dB",
                "FFmpeg silence amplitude or decibel threshold",
            )
            .allow_hyphen_values(true),
        )
        .arg(text(
            "silence-duration",
            "2.0",
            "FFmpeg silence duration in seconds",
        ));
    for (name, help) in [
        ("no-diarize", "Disable speaker labeling"),
        ("no-tech-terms", "Exclude built-in engineering vocabulary"),
        ("no-cache", "Bypass all cache reads and writes"),
        ("no-preprocess", "Disable all audio preprocessing"),
        ("no-mono", "Disable mono downmixing"),
        ("no-trim-silence", "Disable silence removal"),
    ] {
        create = create.arg(boolean(name, help));
    }
    let completion = Command::new("completion").about("Generate shell completions");
    let completion =
        ["bash", "zsh", "fish", "powershell"]
            .into_iter()
            .fold(completion, |command, shell| {
                command.subcommand(
                    Command::new(shell)
                        .about("Print a shell completion script")
                        .arg(boolean("no-descriptions", "Omit completion descriptions")),
                )
            });
    let root=Command::new("deepgram-transcribe").about("Transcribe speech to Markdown and track spending")
      .long_about("Transcribe audio into speaker-labelled Markdown. Cached responses avoid repeat uploads; --force requests another billable transcription.")
      .arg(Arg::new("cache-dir").long("cache-dir").global(true).value_parser(value_parser!(PathBuf)).help("Response and history cache; default follows XDG"))
      .arg(text("api-key","","Deepgram key; otherwise DEEPGRAM_API_KEY").global(true))
      .arg(Arg::new("version").long("version").short('v').action(ArgAction::SetTrue).help("Print the raw version"))
      .subcommand(Command::new("transcript").about("Create audio transcripts").subcommand(create))
      .subcommand(Command::new("job").about("Inspect transcription history and cost")
        .subcommand(Command::new("list").about("List recent jobs and spending").arg(Arg::new("limit").long("limit").default_value("0").value_parser(value_parser!(u32)).help("Number of recent jobs; zero lists all")))
        .subcommand(Command::new("inspect").about("Inspect a job by contents or request ID").arg(Arg::new("target").value_name("audio-file|request-id").required(true).help("Audio file matched by contents, or a recorded request ID"))))
      .subcommand(Command::new("cache").about("Manage saved transcripts and history")
        .subcommand(Command::new("status").about("Show cache location and real response count"))
        .subcommand(Command::new("clear").about("Remove owned cached responses and job history")))
      .subcommand(Command::new("dependency").about("Manage FFmpeg and FFprobe")
        .subcommand(Command::new("list").about("Report installed versions"))
        .subcommand(Command::new("install").about("Install missing or outdated programs"))
        .subcommand(Command::new("update").about("Run package installation for every declared program")))
      .subcommand(completion);
    cli_command_kit::setup(root, mode, &help_options()).expect("unique skill command")
}
