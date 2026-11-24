//! `yokoso msg`: a Unix socket of the running shell. One JSON array of words
//! a line in, one JSON object a line out: `{"ok": …}` or `{"error": "…"}`.

use std::{
    io::{BufRead, BufReader, Read, Write},
    os::unix::net::{UnixListener, UnixStream},
    path::PathBuf,
    time::Duration,
};

use serde_json::{Value, json};
use yokoso_ui::{
    app::{App, Shell, Ui},
    calloop::{Interest, Mode, PostAction, generic::Generic},
};

/// What a shell answers on the socket.
pub trait Answer: Shell {
    fn answer(&mut self, ui: &mut Ui<Self>, words: &[String]) -> Result<Value, String>;
}

/// The socket of the shell on this Wayland display.
pub fn path() -> Result<PathBuf, String> {
    let run = std::env::var_os("XDG_RUNTIME_DIR").ok_or("XDG_RUNTIME_DIR is not set")?;
    let display = std::env::var("WAYLAND_DISPLAY").unwrap_or_else(|_| "wayland-0".into());
    let display = display.rsplit('/').next().unwrap_or(&display).to_owned();
    Ok(PathBuf::from(run).join(format!("yokoso-{display}.sock")))
}

/// Opens the socket and answers it on the loop. A socket left by a shell
/// that is gone is taken over; one with a shell behind it is not.
pub fn listen<S: Answer>(ui: &Ui<S>) -> Result<PathBuf, String> {
    let path = path()?;
    if UnixStream::connect(&path).is_ok() {
        return Err(format!("another yokoso answers on {}", path.display()));
    }
    let _ = std::fs::remove_file(&path);
    let listener = UnixListener::bind(&path).map_err(|error| format!("{}: {error}", path.display()))?;
    listener.set_nonblocking(true).map_err(|error| error.to_string())?;
    let handle = ui.handle();
    ui.handle()
        .insert_source(
            Generic::new(listener, Interest::READ, Mode::Level),
            move |_, listener, _| {
                while let Ok((stream, _)) = listener.accept() {
                    if let Err(error) = take(&handle, stream) {
                        tracing::warn!(%error, "ipc");
                    }
                }
                Ok(PostAction::Continue)
            },
        )
        .map_err(|error| error.to_string())?;
    tracing::info!(path = %path.display(), "ipc");
    Ok(path)
}

/// Reads one request from `stream` as it comes, without ever blocking the
/// loop, and answers it.
fn take<S: Answer>(handle: &yokoso_ui::calloop::LoopHandle<'static, App<S>>, stream: UnixStream) -> Result<(), String> {
    stream.set_nonblocking(true).map_err(|error| error.to_string())?;
    let mut read = Vec::new();
    handle
        .insert_source(
            Generic::new(stream, Interest::READ, Mode::Level),
            move |_, stream, app| {
                let mut chunk = [0u8; 4096];
                // SAFETY: the stream is not dropped or replaced here.
                let stream = unsafe { stream.get_mut() };
                let ended = match stream.read(&mut chunk) {
                    Ok(0) => true,
                    Ok(n) => {
                        read.extend_from_slice(&chunk[..n]);
                        read.contains(&b'\n')
                    }
                    Err(error) if error.kind() == std::io::ErrorKind::WouldBlock => false,
                    Err(_) => return Ok(PostAction::Remove),
                };
                if !ended && read.len() < 65536 {
                    return Ok(PostAction::Continue);
                }
                let line = read.split(|b| *b == b'\n').next().unwrap_or_default();
                let reply = match serde_json::from_slice::<Vec<String>>(line) {
                    Ok(words) => match app.shell.answer(&mut app.ui, &words) {
                        Ok(value) => json!({ "ok": value }),
                        Err(error) => json!({ "error": error }),
                    },
                    Err(error) => json!({ "error": format!("not a JSON array of words: {error}") }),
                };
                let _ = stream.set_nonblocking(false);
                let _ = stream.set_write_timeout(Some(Duration::from_secs(1)));
                let _ = writeln!(stream, "{reply}");
                Ok(PostAction::Remove)
            },
        )
        .map_err(|error| error.to_string())?;
    Ok(())
}

/// The client: sends `words`, prints the answer. Exit code 1 for an error.
pub fn send(words: &[String]) -> Result<(), String> {
    let path = path()?;
    let mut stream = UnixStream::connect(&path).map_err(|error| format!("no yokoso on {}: {error}", path.display()))?;
    stream
        .set_read_timeout(Some(Duration::from_secs(3)))
        .map_err(|error| error.to_string())?;
    writeln!(stream, "{}", json!(words)).map_err(|error| error.to_string())?;
    let mut line = String::new();
    BufReader::new(stream)
        .read_line(&mut line)
        .map_err(|error| format!("no answer: {error}"))?;
    let reply: Value = serde_json::from_str(&line).map_err(|error| format!("a garbled answer: {error}"))?;
    match (reply.get("ok"), reply.get("error")) {
        (Some(Value::String(text)), _) => println!("{text}"),
        (Some(Value::Null), _) => {}
        (Some(value), _) => println!("{}", serde_json::to_string_pretty(value).unwrap_or_default()),
        (_, Some(error)) => return Err(error.as_str().unwrap_or("error").to_owned()),
        _ => return Err("a garbled answer".into()),
    }
    Ok(())
}
