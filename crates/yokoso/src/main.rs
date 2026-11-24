//! yokoso: a desktop shell for Wayland.

mod probe;

use std::process::ExitCode;

const USAGE: &str = "usage: yokoso [shell]";

fn main() -> ExitCode {
    // First, before any thread: it changes the environment.
    yokoso_ui::gpu::prefer_mesa();
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_env("YOKOSO_LOG")
                .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new("info")),
        )
        .with_writer(std::io::stderr)
        .with_ansi(std::io::IsTerminal::is_terminal(&std::io::stderr()))
        .init();
    let args: Vec<String> = std::env::args().skip(1).collect();
    match args.first().map(String::as_str) {
        None | Some("shell") => match yokoso_ui::app::run(probe::Probe, |_| {}) {
            Ok(()) => ExitCode::SUCCESS,
            Err(error) => {
                tracing::error!(%error, "stopped");
                ExitCode::FAILURE
            }
        },
        Some(_) => {
            eprintln!("{USAGE}");
            ExitCode::from(2)
        }
    }
}
