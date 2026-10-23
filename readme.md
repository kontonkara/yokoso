# yokoso

*ようこそ, "welcome".* A [greetd](https://sr.ht/~kennylevinsen/greetd/)
greeter on [Quickshell](https://quickshell.org). It runs in
[cage](https://github.com/cage-kiosk/cage), so it works whatever desktop it
logs into.

- The UI scale comes from the screen's physical size (EDID) before the first
  frame, or from the config.
- Only the GPU that drives the screens is used. With no screen on an NVIDIA
  GPU, only Mesa's EGL is loaded, so a PRIME laptop's discrete GPU stays
  asleep.
- PAM's prompts and messages are shown as PAM sends them, a fingerprint
  reader's included; one too long for the field goes under it. greetd can't
  change an expired password, so for one yokoso says where to change it.
- A click on the name (or F2) picks another account, or "Other…" one that
  isn't listed (a directory's, LDAP's), by its name; with more than one
  session, a click on it (or F3) picks another. The next login starts with
  the last one's. Without the list (`userList: false`), a name is typed
  every time.
- After a minute without input the screen goes dark and a laptop's backlight
  low (through logind). A key, a click or a move brings it back; that key or
  click does nothing else.
- A clock in the system's locale, a Caps Lock warning, a word when Num Lock
  is on, and the power buttons logind allows (a second click confirms).
- Early in the boot, while the GPU's driver is still loading (when it
  isn't in the initrd), it waits for it instead of starting on the
  firmware's framebuffer (simpledrm), which goes away under it.
- A session that ends within seconds of starting (a broken profile, a
  missing program) doesn't just bring the login screen back: it says so.
- If yokoso fails, greetd's text greeter (agreety) comes up in its place
  and logs in to the same session.
- Ctrl+Alt+Shift+D brings up the built-in look in the built-in theme, for a
  theme that keeps anyone from logging in.

## Try it

```sh
nix run github:kontonkara/yokoso#preview
```

This opens the greeter in a window. It logs in against greetd's `fakegreet`:
the password is `password`, then `9`. Set `YOKOSO_CONFIG=<file>` to try a
config.

## NixOS

```nix
{
  inputs.yokoso.url = "github:kontonkara/yokoso";

  # in a module:
  imports = [ inputs.yokoso.nixosModules.default ];
  services.yokoso = {
    enable = true;
    settings.scale = 1.0;
  };
}
```

The module turns on greetd, writes `/etc/yokoso/config.json` (and
`services.yokoso.themes` into `/etc/yokoso/themes`), and passes on the
keyboard layout from `services.xserver.xkb`. With `services.xserver.enable`
it also turns on `startx` for X11 sessions. Mesa's shader cache is kept in
`/var/cache/yokoso`, and the last login's user and session in
`/var/lib/yokoso`.

## Other systems

yokoso needs greetd (with agreety), Quickshell 0.3.1 or newer, cage, jq,
awk and systemd (`busctl`, `systemd-cat`); `yokoso preview` needs greetd's
`fakegreet` too, which not every distribution ships (Arch doesn't). On Arch,
[`dist/arch/PKGBUILD`](dist/arch/PKGBUILD) builds `yokoso-git`; elsewhere:

```sh
make install PREFIX=/usr
systemd-tmpfiles --create
```

Then greetd runs it, in `/etc/greetd/config.toml`:

```toml
[default_session]
command = "yokoso"
user = "greeter"
```

and `systemctl enable greetd` (with the display manager before it
disabled). `/var/lib/yokoso` and `/var/cache/yokoso` belong to the account
`greeter`; for another one, copy `/usr/lib/tmpfiles.d/yokoso.conf` to
`/etc/tmpfiles.d/` with its name. The keyboard layout is the system's
(`localectl set-x11-keymap us,de "" "" grp:alt_shift_toggle`), unless the
command sets `XKB_DEFAULT_LAYOUT` and the rest; a cursor theme is
`XCURSOR_THEME` and `XCURSOR_SIZE` there (`command = "env
XCURSOR_THEME=Adwaita yokoso"`). The config is
`/etc/yokoso/config.json`, below.

## Config

`/etc/yokoso/config.json`. Every key is optional:

| Key | Default | |
|---|---|---|
| `user` | the first account | account to start with, listed or not (LDAP) |
| `userList` | `true` | list the accounts to pick from; `false` asks for a name every time (not the last login's) and shows no pictures |
| `session` | the first one | session to start with, by desktop file name (`niri`, `i3`); Wayland's come first |
| `remember` | `true` | start with the last login's user and that user's last session instead |
| `theme` | `dark` | `dark`, `light`, a theme's name or a theme file's path; see below |
| `output` | see below | screen with the form (`DP-1`), or `cursor` for the one the pointer is on |
| `scale` | `0` | UI scale; `0` takes it from the screen's size |
| `idle` | `60` | seconds without input before the screen goes dark and the backlight low; `0` for never |
| `language` | the locale's | the language of yokoso's own few words: `"en"` or `"ru"` (from `shell/i18n`); PAM's follow greetd's locale |
| `x11Wrapper` | `startx /usr/bin/env` | what starts an X server for an X11 session, with its command after it |

A config that isn't a JSON object is left out, and keys yokoso doesn't know
(a misspelt `thme`) are named, both in the journal (`journalctl -b -t
yokoso`).

The last login is kept in `/var/lib/yokoso/state.json`, so the greeter's
account has to own `/var/lib/yokoso` (the NixOS module makes it). Without
it nothing is remembered, and `user` and `session` decide.

A user's picture is `avatars/<name>` next to the config
(`/etc/yokoso/avatars/alice`, in any format Qt reads), else AccountsService's
(`/var/lib/AccountsService/icons/alice`); without one, it's their initial. On
NixOS, `services.yokoso.avatars.alice = ./alice.png;` puts it there.

The cursor is `XCURSOR_THEME`'s, else the system's default theme, at
`XCURSOR_SIZE` (24 by default) times the UI scale. On NixOS,
`services.yokoso.cursor = { package = pkgs.bibata-cursors; name =
"Bibata-Modern-Classic"; };` sets them.

With several screens the form goes on `output`, else on a laptop's own panel,
or on an external screen while the lid is closed, else on the first screen.
The other screens stay black. With `cursor`, the form starts there too and
then moves to whichever screen the pointer moves onto, except a closed
laptop panel.

## Themes

The built-in ones are `dark` (the default) and `light`. Any other theme is a
JSON file: `"theme": "mine"` reads `themes/mine.json` next to the config
(`/etc/yokoso/themes/mine.json`), and a path reads that file.

```json
{
  "extends": "dark",
  "colors": {
    "background": "#1e1e2e",
    "text": "#cdd6f4",
    "subtext": "#a6adc8",
    "dim": "#7f849c",
    "field": "#313244",
    "accent": "#fab387",
    "error": "#f38ba8"
  },
  "font": "Inter",
  "background": "wallpaper.jpg",
  "backgroundDim": 0.4
}
```

Every key is optional. A theme goes on top of the one it `extends` (`dark`
when it doesn't say), and the config's own keys go on top of the theme, so
`"colors": {"accent": "#fab387"}` in the config changes just that (`layout`
merges the same way).

| Key | |
|---|---|
| `colors.background` | behind everything, where there's no image |
| `colors.text` | the clock, the username, the lists' entries, what's typed and the icon of a power button under the pointer |
| `colors.subtext` | the date, the session, hints, the power buttons' icons, the status in the corner and Num Lock |
| `colors.dim` | the field's placeholder, its arrow until something is typed and the arrows by the names |
| `colors.field` | the password field and the user and session lists |
| `colors.accent` | the avatar, what's under the pointer, an armed power button, the picked entry in a list, the field's outline and its button once something is typed, and the spinner while a password is checked |
| `colors.error` | errors (and the field's outline with one), the Caps Lock warning and a low battery |
| `font` | a font family; `""` for the default one |
| `background` | an image for every screen, or one per output with `"*"` for the rest (`{"DP-1": "a.jpg", "*": "b.jpg"}`), cropped to fill each screen; paths are relative to the theme file |
| `backgroundDim` | how much `colors.background` covers the image, `0` to `1` |
| `animations` | the parts coming in one after another at the start and going as the session starts, the form sliding to another screen, buttons that answer the pointer, a ring running out around an armed power button, a shake at a wrong password, the spinner, password dots that pop up, the clock's figures rolling over, lists that drop in with a highlight that slides; `false` for none |
| `status` | what a top corner shows: `"hostname"`, `"battery"` (a laptop's), both by default; `[]` for nothing |
| `layout.form` | the form in the `"center"` of the screen, on the `"left"` or on the `"right"` |
| `layout.clock` | the clock `"above"` the form, at the `"top"` of the screen, or `"none"` |
| `layout.power` | the power buttons at the `"bottom"`, in its right `"corner"`, or `"none"` |
| `layout.status` | the status in the `"top-right"` or `"top-left"` corner |
| `clockFormat` | the time as a [Qt format](https://doc.qt.io/qt-6/qml-qtqml-qt.html#formatDateTime-method) (`"HH:mm"`); `""` for the locale's short one |
| `dateFormat` | the date likewise (`"dddd d MMMM"`); `""` for the locale's long one without the year |
| `radius` | the corners of the field, the buttons, the lists and the avatar, in pixels; `null` for round |
| `fieldWidth` | the form's width, in pixels |
| `look` | a QML file that draws the login screen instead; see below |

A theme that can't be read is skipped (with a warning in the journal), so a
broken one never stands between anyone and the login.
[`examples/catppuccin-mocha.json`](examples/catppuccin-mocha.json) is one to
start from. On NixOS, `services.yokoso.themes.<name>` writes them.

## Your own look

A theme's `look` is a QML file (a path relative to the theme, like
`background`) that draws the login screen in place of the built-in one,
[`shell/Look.qml`](shell/Look.qml). If it can't be loaded, the built-in one
comes up instead, with a warning in the journal. For one that loads but
doesn't work (or a theme whose colors hide the field), **Ctrl+Alt+Shift+D**
brings up the built-in look in the built-in dark theme until the greeter
starts again: safe mode. Still, try a look with `yokoso preview` first
(`YOKOSO_CONFIG=<file>`). [`examples/minimal.qml`](examples/minimal.qml),
with [`examples/minimal.json`](examples/minimal.json), is about the smallest
one that works. On NixOS, `look = ./mine.qml;` in a theme copies the file to
the store; a look in several files is `look = "${./mine}/Look.qml";`.

Its root item has `required property var greeter`: yokoso's core, which
does the login, the power buttons and the memory, and watches the pointer.

| | |
|---|---|
| `config`, `colors`, `fontFamily` | the config over the theme, its colors, its font |
| `tr(text)` | yokoso's English words in the config's language, when `shell/i18n` has them |
| `screens`, `area`, `backgroundFor(name)` | every screen in the window as `{ name, rect }`; the rectangle of the one with the form; a screen's background as a URL |
| `u(n)`, `dpr` | `n` pixels at the UI scale; the screen's device pixel ratio |
| `now`, `locale` | the time, by the minute; the system's locale for dates |
| `users`, `user`, `pickUser(u)` | the accounts as `{ name, realName, icon }` (none with `userList: false`); the one logging in; another one, whose login then starts over |
| `needsUser`, `enterUser(name)`, `canEnterUser()` | no user yet (ask for a name); a typed name, listed or not, whose login then starts over; whether one can be typed now |
| `sessions`, `session`, `pickSession(s)`, `sessionName(s)` | the sessions as `{ id, name, exec, type }`; the one to start; another one; a name to show |
| `canPick(which)` | whether another `"user"` or `"session"` can be picked now |
| `auth.phase` | `"starting"`, `"prompt"`, `"verifying"`, `"failed"` or `"launching"` |
| `auth.prompt`, `auth.secret`, `auth.submit(text)` | PAM's question and whether the answer is secret; answering it |
| `auth.message`, `auth.messageIsError` | PAM's last message, or "Wrong password", and whether it's an error |
| `problem` | why no login can start (no session), else `""` |
| `capsLock`, `numLock`, `hostname`, `battery` | Caps Lock; Num Lock; the machine's name; `{ level, plugged }`, or `null` without one |
| `power`, `powerLabels`, `press(action)` | the power actions logind allows, their names, and a press: a second one within `armedFor` ms acts |
| `armed`, `armedFor`, `powerError` | the action waiting for its second press; how long it waits; why one failed |
| `idle`, `wake()` | whether the screen is asleep (the core dims the backlight, the look darkens the rest); call `wake()` on every key and click, and when it returns `true` the input only woke the screen |

## Developing

`nix run .#shots -- <dir>` renders the working tree's greeter headless, with
fake users and sessions, in the scenarios of
[`tests/shots.sh`](tests/shots.sh) (the lists, a wrong password, two
screens, idle, Russian, a layout, a look of one's own, ...) into `<dir>`;
`--against <an earlier dir>` counts the pixels that changed. `nix flake
check` runs the VM tests in [`tests/vm`](tests/vm): the greeter under the
real greetd, PAM and cage, logging in through the lists, idle, the power
buttons, PAM's messages, an expired password, a fingerprint, a session that
ends at once, a GPU driver that loads after the greeter starts, a layout
from localed and an install with `make` instead of the module.
