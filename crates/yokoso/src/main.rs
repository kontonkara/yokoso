//! yokoso: a desktop shell for Wayland.

mod ipc;
mod probe;

use std::process::ExitCode;

const USAGE: &str = "usage: yokoso [shell [--window]]
       yokoso msg <command> [args…]   (ping, stats, dump, move [seconds])";

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
    let done = match args.first().map(String::as_str) {
        None | Some("shell") => shell(args.iter().any(|arg| arg == "--window")),
        Some("msg") if args.len() > 1 => ipc::send(&args[1..]),
        Some("-h" | "--help" | "help") => {
            println!("{USAGE}");
            Ok(())
        }
        Some(_) => {
            eprintln!("{USAGE}");
            return ExitCode::from(2);
        }
    };
    match done {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            eprintln!("yokoso: {error}");
            ExitCode::FAILURE
        }
    }
}

fn shell(window: bool) -> Result<(), String> {
    let mut socket = None;
    let done = yokoso_ui::app::run(probe::Probe::new(window), |app| {
        socket = Some(ipc::listen(&app.ui)?);
        Ok(())
    });
    if let Some(path) = socket {
        let _ = std::fs::remove_file(path);
    }
    done
}
