use cli_command_kit::Mode;
use deepgram_transcribe_cli::{args, entry};
use tokio_util::sync::CancellationToken;

#[tokio::test]
async fn canonical_embedded_skill_and_every_help_path_match_live_interface_offline() {
    let omissions = cli_command_kit::skill_omissions(
        &args::command(Mode::Agent),
        &args::help_options(),
        args::SKILL,
    );
    assert!(omissions.is_empty(), "{omissions:#?}");
    let mut tree = args::command(Mode::Agent);
    tree.build();
    fn paths(
        command: &cli_command_kit::clap::Command,
        path: Vec<String>,
        all: &mut Vec<Vec<String>>,
    ) {
        all.push(path.clone());
        for child in command.get_subcommands() {
            let mut next = path.clone();
            next.push(child.get_name().into());
            paths(child, next, all);
        }
    }
    let mut all = vec![];
    paths(&tree, vec!["deepgram-transcribe".into()], &mut all);
    for mode in [Mode::Human, Mode::Agent] {
        for mut path in all.clone() {
            if !path.iter().any(|part| part == "help") {
                path.push("--help".into());
            }
            let mut out = vec![];
            let mut err = vec![];
            let code = entry(
                path.into_iter().map(Into::into),
                || panic!("metadata must remain offline"),
                &mut out,
                &mut err,
                mode,
                &CancellationToken::new(),
            )
            .await;
            assert_eq!(code, 0, "{}", String::from_utf8_lossy(&err));
            assert!(err.is_empty());
            if mode == Mode::Agent {
                assert!(
                    String::from_utf8(out)
                        .unwrap()
                        .starts_with("ALERT: Agents must read `AGENT=1 deepgram-transcribe skill`")
                );
            }
        }
        let mut out = vec![];
        assert_eq!(
            entry(
                ["deepgram-transcribe", "skill"].map(Into::into),
                || panic!("skill must remain offline"),
                &mut out,
                &mut vec![],
                mode,
                &CancellationToken::new()
            )
            .await,
            0
        );
        assert_eq!(out, args::SKILL.as_bytes());
    }
}
