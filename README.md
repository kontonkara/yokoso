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
right = ["privacy", "tray", "layout", "network", "sound", "battery", "notifications"]
hover_switch = true       # with a panel open, the pointer on another part opens its own
panels = "grow"           # out of their part's edge; "drop": down from the bar's edge

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

[bar.privacy]             # a click mutes the microphone, the wheel turns it
on_click = []             # the middle button

[notifications]
visible = 3               # toasts at once; more wait
timeout = 5               # seconds; a low one, a critical one (0: until closed)
timeout_low = 3
timeout_critical = 0
over_fullscreen = true    # critical ones over full-screen windows
history = 100
quiet_from = ""           # "23:00": quiet from then
quiet_until = ""          # "08:00": until then
quiet_sharing = false     # quiet while the screen is shared

[[notifications.rules]]   # any number of them
app = "Chat"              # by app, any case
text = ""                 # and/or words in it
action = "quiet"          # hide | quiet | through | run
run = []                  # for run: a command, as words

[probe]                   # the test card of `yokoso probe`
accent = "#f3a17a"
radius = 8

[debug]
overlay = false   # frame times over every surface
```

`yokoso msg bar show`, `hide` or `toggle` puts the bar away and back.

`yokoso msg` talks to the running shell: `ping`, `stats`, `dump`, `reload`,
`overlay [on|off]`, `move [seconds]`.
`yokoso msg notifications quiet [on|off|toggle]` turns quiet on and off;
`clear [app]` clears the history.

As a user service: `dist/yokoso.service` (the package installs it), part of
`graphical-session.target`. Failing three times within a minute, it stays
down and `yokoso-fallback.service` runs `$XDG_CONFIG_HOME/yokoso/fallback`, a
script of your own that brings back what yokoso took over, if there is one:
once, when it gives up, not at each crash (systemd 254 or later).

## The bar

From its start to its end, as the config orders them: the workspaces as a
map of their columns, media, the clock, who records (microphone, camera,
the screen shared), the tray, the keyboard layout, the network, the sound,
the battery and the bell. A click on the clock opens a calendar, a right
click on a tray icon its menu, a click on the bell the history; with a
panel open, the pointer on another part with one opens that one. A click on
the microphone mutes it. With every radio blocked, airplane mode as the
system has it, the network shows a plane. The wheel turns the workspaces,
the layout, the volume, the microphone and the calendar's months; the
middle button mutes the sound, skips a track and turns quiet on and off on
the bell. Tooltips show just past the bar, on a strip the pointer goes
through.

Its services are niri's event stream, UPower, NetworkManager, MPRIS and the
StatusNotifierItem tray on D-Bus, PipeWire through `pw-dump --monitor` and
`wpctl`, and the radios' switches in `/dev/rfkill`. Each is read once and
then on its own signals: nothing is asked on a timer, and at rest the bar
draws nothing.

## Notifications

yokoso is the notification server (`org.freedesktop.Notifications`); with
another one running, it waits and takes over once that one lets go. Toasts
show at the top right of the screen in focus, below the bar: three at once,
the rest waiting, their time still while the pointer is on them; a click is
their default action, their buttons and a reply typed in them go to the
app. Critical ones stay until closed and show over full-screen windows. The
history is kept for its owner alone in
`$XDG_STATE_HOME/yokoso/notifications.json`, and so is quiet. Text comes as
plain text: markup in it is taken out, and nothing in it is loaded.

## Test

```sh
nix develop -c tests/all.sh
```

Unit tests, then scripts on a stand: a headless compositor on the integrated
GPU, so nothing on the desktop is touched.

## License

MIT
