# yokoso

A desktop shell for Wayland, written in Rust and drawn on the GPU without a
toolkit. niri first.

**Status:** stage 2, the bar, under way. Not yet a shell to use every day;
see [SCOPE.md](SCOPE.md) for the road and [ARCHITECTURE.md](ARCHITECTURE.md)
for how it is built.

## Build

```sh
nix build            # the package
nix develop          # a shell with everything to build and test
cargo build --release
```

Without Nix: a Rust toolchain (1.88 or newer), `pkg-config`, and the
development files of `wayland` and `libxkbcommon`. EGL and GLES are loaded at
run time from `libglvnd` or Mesa.

## Run

```sh
yokoso shell            # the bar, on the main screen
yokoso shell --window   # in a plain window, to try it without the session
yokoso probe            # the test card of stage 0, on every output (--window too)
yokoso msg ping         # talk to the running shell
yokoso validate         # check the config
```

The config lives in `$XDG_CONFIG_HOME/yokoso/config.toml`, with
`overrides.toml` beside it laid over it. Both are read again as they change;
a broken file is reported with its line and column, and the last good config
stays.

```toml
[bar]
position = "top"          # or "bottom"
height = 40
margin = 8                # how far it floats from the edges
screens = []              # by description or connector; none: the main one
left = ["workspaces", "media"]
center = ["clock"]
right = ["privacy", "tray", "layout", "network", "sound", "battery"]

[bar.clock]
format = "%H:%M"          # strftime
date = false
date_format = "%a %-d %b"

[probe]                   # the test card of `yokoso probe`
accent = "#f3a17a"
radius = 8

[debug]
overlay = false   # frame times over every surface
```

`yokoso msg bar show`, `hide` or `toggle` puts the bar away and back.

`yokoso msg` talks to the running shell: `ping`, `stats`, `dump`, `reload`,
`overlay [on|off]`, `move [seconds]`.

As a user service: `dist/yokoso.service` (the package installs it), part of
`graphical-session.target`.

## Test

```sh
nix develop -c tests/all.sh
```

Unit tests, then scripts on a stand: a headless compositor on the integrated
GPU, so nothing on the desktop is touched.

## License

MIT
