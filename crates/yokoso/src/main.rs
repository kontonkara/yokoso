//! yokoso: a desktop shell for Wayland.

mod bar;
mod bluez;
mod config;
mod demo;
mod icon_theme;
mod ipc;
mod mpris;
mod niri;
mod nm;
mod panel;
mod probe;
mod pw;
mod sni;
mod upower;

use std::process::ExitCode;

const USAGE: &str = "usage: yokoso [shell [--window]]
       yokoso probe [--window]         (a test card on every output, or in a window)
       yokoso demo [--layer]           (every element, in a window or as a layer)
       yokoso validate [file]          (the config and its overrides, or one file)
       yokoso msg <command> [args…]   (ping, stats, dump, reload, overlay [on|off], move [seconds])";

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
        Some("probe") => probe(args.iter().any(|arg| arg == "--window")),
        Some("demo") => demo(args.iter().any(|arg| arg == "--layer")),
        Some("msg") if args.len() > 1 => ipc::send(&args[1..]),
        Some("validate") => validate(args.get(1)),
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

/// The demo of every element.
fn demo(layer: bool) -> Result<(), String> {
    let mut socket = None;
    let done = yokoso_ui::app::run(demo::Demo::new(layer), |app| {
        socket = Some(ipc::listen(&app.ui)?);
        Ok(())
    });
    if let Some(path) = socket {
        let _ = std::fs::remove_file(path);
    }
    done
}

/// Checks `file`, or the config and its overrides where they are.
fn validate(file: Option<&String>) -> Result<(), String> {
    let files: Vec<std::path::PathBuf> = match file {
        Some(file) => vec![file.into()],
        None => config::FILES
            .iter()
            .map(|name| config::dir().join(name))
            .filter(|path| path.exists())
            .collect(),
    };
    for path in &files {
        config::validate(path)?;
        println!("{}: ok", path.display());
    }
    if files.is_empty() {
        println!("no config in {}: the defaults are in force", config::dir().display());
    }
    Ok(())
}

/// The shell: the bar on the main screen, or in a window to try it.
fn shell(window: bool) -> Result<(), String> {
    let dir = config::dir();
    let config = config::load(&dir).unwrap_or_else(|error| {
        tracing::warn!(%error, "config not read: the defaults are in force");
        config::Config::default()
    });
    let mut socket = None;
    let done = yokoso_ui::app::run(bar::Bar::new(window, config), |app| {
        socket = Some(ipc::listen(&app.ui)?);
        bar::Bar::start(&mut app.ui, &mut app.shell);
        let sender = app.ui.sender();
        config::watch(dir, move |loaded| {
            let _ = sender.send(bar::Message::Config(loaded));
        })?;
        Ok(())
    });
    if let Some(path) = socket {
        let _ = std::fs::remove_file(path);
    }
    done
}

/// The probe of stage 0: a card on every output, or in a window.
fn probe(window: bool) -> Result<(), String> {
    let dir = config::dir();
    let config = config::load(&dir).unwrap_or_else(|error| {
        tracing::warn!(%error, "config not read: the defaults are in force");
        config::Config::default()
    });
    let mut socket = None;
    let done = yokoso_ui::app::run(probe::Probe::new(window, config), |app| {
        socket = Some(ipc::listen(&app.ui)?);
        let sender = app.ui.sender();
        config::watch(dir, move |loaded| {
            let _ = sender.send(probe::Message::Config(loaded));
        })?;
        Ok(())
    });
    if let Some(path) = socket {
        let _ = std::fs::remove_file(path);
    }
    done
}
