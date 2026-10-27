use cli_command_kit::Mode;
use deepgram_transcribe_cli::{Context, entry};
use tokio_util::sync::CancellationToken;
#[tokio::main]
async fn main() {
    let mode = Mode::from_env();
    let cancel = CancellationToken::new();
    let token = cancel.clone();
    let signal = tokio::spawn(async move {
        #[cfg(unix)]
        {
            let mut terminate =
                tokio::signal::unix::signal(tokio::signal::unix::SignalKind::terminate())
                    .expect("register SIGTERM");
            tokio::select! {_=tokio::signal::ctrl_c()=>{},_=terminate.recv()=>{}};
        }
        #[cfg(not(unix))]
        {
            let _ = tokio::signal::ctrl_c().await;
        }
        token.cancel();
    });
    let code = entry(
        std::env::args_os(),
        Context::from_env,
        &mut std::io::stdout().lock(),
        &mut std::io::stderr().lock(),
        mode,
        &cancel,
    )
    .await;
    signal.abort();
    std::process::exit(i32::from(code));
}
