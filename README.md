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

[bar.network]
on_click = []             # a command, as words, until it has a panel
vpn = []                  # connections that count as a VPN, by name

[bar.sound]
on_click = []
step = 5                  # percent a notch of the wheel
max = 100                 # how loud the wheel goes; up to 150

[bar.battery]
on_click = []

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
`graphical-session.target`. Failing three times within a minute, it stays
down and `yokoso-fallback.service` runs `$XDG_CONFIG_HOME/yokoso/fallback`, a
script of your own that brings back what yokoso took over, if there is one.

## The bar

From its start to its end, as the config orders them: the workspaces as a
map of their columns, media, the clock, who records (microphone, camera,
the screen shared), the tray, the keyboard layout, the network, the sound
and the battery. A click on the clock opens a calendar, a right click on a
tray icon its menu; with a panel open, the pointer on another part with one
opens that one. The wheel turns the workspaces, the layout, the volume and
the calendar's months; the middle button mutes the sound and skips a track.

Its services are niri's event stream, UPower, NetworkManager, MPRIS and the
StatusNotifierItem tray on D-Bus, and PipeWire through `pw-dump --monitor`
and `wpctl`. Each is read once and then on its own signals: nothing is
asked on a timer, and at rest the bar draws nothing.

## Test

```sh
nix develop -c tests/all.sh
```

Unit tests, then scripts on a stand: a headless compositor on the integrated
GPU, so nothing on the desktop is touched.

## License

MIT
