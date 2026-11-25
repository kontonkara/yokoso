//! The screen's backlight: how bright it is, as the kernel says. It tells
//! each change itself, whatever made it (keys, an app, the dimming when idle),
//! through `actual_brightness`: a read, then a wait on it for the next. Nothing
//! is asked on a timer. YOKOSO_BACKLIGHT names a pipe to read levels from
//! instead, a line each, for the stand.

use std::{
    io::{BufRead, Read, Seek},
    path::{Path, PathBuf},
    time::Duration,
};

/// The device: the firmware's over the platform's over a driver's own, as
/// the kernel's documentation orders them.
fn device() -> Option<PathBuf> {
    let mut all: Vec<(u8, PathBuf)> = std::fs::read_dir("/sys/class/backlight")
        .ok()?
        .flatten()
        .map(|entry| {
            let kind = std::fs::read_to_string(entry.path().join("type")).unwrap_or_default();
            let rank = match kind.trim() {
                "firmware" => 0,
                "platform" => 1,
                _ => 2,
            };
            (rank, entry.path())
        })
        .collect();
    all.sort();
    all.into_iter().next().map(|(_, path)| path)
}

fn number(path: &Path) -> Option<f32> {
    std::fs::read_to_string(path).ok()?.trim().parse().ok()
}

/// Waits on `device`'s brightness, sending each change as a share of the
/// most, 0 to 1. Returns when it can't be read, or `send` says to stop.
fn watch(device: &Path, send: &impl Fn(f32) -> bool) -> bool {
    let Some(most) = number(&device.join("max_brightness")).filter(|most| *most > 0.0) else {
        return true;
    };
    let Ok(mut file) = std::fs::File::open(device.join("actual_brightness")) else {
        return true;
    };
    let mut last = None;
    loop {
        let mut text = String::new();
        if file.rewind().is_err() || file.read_to_string(&mut text).is_err() {
            return true;
        }
        if let Ok(now) = text.trim().parse::<f32>() {
            let level = (now / most).clamp(0.0, 1.0);
            if last != Some(level) {
                if !send(level) {
                    return false;
                }
                last = Some(level);
            }
        }
        // The kernel wakes this as the brightness changes.
        let mut fds = [rustix::event::PollFd::new(&file, rustix::event::PollFlags::PRI)];
        if rustix::event::poll(&mut fds, None).is_err() {
            return true;
        }
    }
}

/// Levels from a pipe, a line each, as the stand writes them.
fn watch_pipe(path: &Path, send: &impl Fn(f32) -> bool) -> bool {
    let Ok(file) = std::fs::File::open(path) else {
        return true;
    };
    for line in std::io::BufReader::new(file).lines() {
        let Ok(line) = line else { return true };
        if let Ok(level) = line.trim().parse::<f32>()
            && !send(level.clamp(0.0, 1.0))
        {
            return false;
        }
    }
    true
}

/// Watches the backlight on a thread of its own; without one, nothing is
/// sent and the thread ends.
pub fn listen(send: impl Fn(f32) -> bool + Send + 'static) {
    let pipe = std::env::var_os("YOKOSO_BACKLIGHT").map(PathBuf::from);
    let spawned = std::thread::Builder::new().name("backlight".into()).spawn(move || {
        loop {
            let goes_on = match &pipe {
                Some(pipe) => watch_pipe(pipe, &send),
                None => match device() {
                    Some(device) => watch(&device, &send),
                    None => return,
                },
            };
            if !goes_on {
                return;
            }
            std::thread::sleep(Duration::from_secs(5));
        }
    });
    if let Err(error) = spawned {
        tracing::warn!(%error, "no thread for the backlight");
    }
}
