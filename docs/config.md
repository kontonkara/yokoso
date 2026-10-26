# Config

`/etc/yokoso/config.json`; on NixOS, `services.yokoso.settings`. Every key
is optional:

| Key | Default | |
|---|---|---|
| `user` | the first account | account to start with, listed or not (LDAP) |
| `userList` | `true` | list the accounts to pick from; `false` asks for a name every time and shows no pictures |
| `session` | the first one | session to start with: a desktop file's name (`niri`, `i3`) or one of `sessions` |
| `remember` | `true` | start with the last login's user and that user's last session instead |
| `reveal` | `true` | an eye in the field that shows the password as it's typed |
| `sessions` | `[]` | sessions of your own, first in the list; see below |
| `env` | `{}` | variables for every session (`{"QT_QPA_PLATFORMTHEME": "qt6ct"}`); a session's own `env` wins |
| `theme` | `dark` | `dark`, `light`, `high-contrast`, a theme's name or a theme file's path; see [themes](themes.md) |
| `output` | see [several screens](usage.md#several-screens) | screen with the form (`DP-1`), or `cursor` for the one the pointer is on |
| `scale` | `0` | UI scale; `0` takes it from the screen's size |
| `idle` | `60` | seconds without input before the screen goes dark and the backlight low; `0` for never |
| `language` | the locale's | the language of yokoso's own few words: `"en"` or `"ru"`; PAM's follow greetd's locale |
| `x11Wrapper` | `startx /usr/bin/env` | what starts an X server for an X11 session, with its command after it |

A theme's keys can go here too, on top of the theme's (`"colors":
{"accent": "#fab387"}` changes just that).

A config that isn't a JSON object is left out, and keys yokoso doesn't know
(a misspelt `thme`) are named, both in the journal (`journalctl -b -t
yokoso`).

## Sessions of your own

A desktop file's session with a twist, or one without a file at all:

```json
"sessions": [
  {"name": "Niri (debug)", "exec": "niri-session", "desktopNames": ["niri"], "env": {"RUST_LOG": "debug"}}
]
```

`name` and `exec` it needs; `type` is `"wayland"` (the default) or
`"x11"`, `desktopNames` goes to `XDG_CURRENT_DESKTOP`, and `env` to the
session.

## On NixOS

| Option | |
|---|---|
| `services.yokoso.enable` | greetd, with yokoso as its greeter |
| `settings` | `/etc/yokoso/config.json` |
| `themes.<name>` | `/etc/yokoso/themes/<name>.json`, for `settings.theme = "<name>"`; a `look = ./mine.qml;` in one is copied to the store |
| `avatars.<user>` | the user's picture (`./alice.png`) |
| `cursor.package`, `cursor.name`, `cursor.size` | the cursor theme (`pkgs.bibata-cursors`, `"Bibata-Modern-Classic"`) and its size at scale 1 (24) |
| `package` | yokoso itself |

The module also passes on the keyboard layout from
`services.xserver.xkb`, turns on `startx` for X11 sessions when
`services.xserver.enable` is set, and makes `/var/lib/yokoso` (the last
login) and `/var/cache/yokoso` (Mesa's and Qt's shader caches).
