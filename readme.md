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
- PAM's prompts and messages are shown as PAM sends them.
- With more than one account or session, a click on the name (or F2, F3)
  picks another, and the next login starts with the last one's.
- After a minute without input the screen goes dark and a laptop's backlight
  low (through logind). A key, a click or a move brings it back; that key or
  click does nothing else.
- A clock in the system's locale, a Caps Lock warning, and the power
  buttons logind allows (a second click confirms).
- If yokoso fails, greetd's text greeter (agreety) comes up in its place
  and logs in to the same session.

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

## Config

`/etc/yokoso/config.json`. Every key is optional:

| Key | Default | |
|---|---|---|
| `user` | the first account | account to start with, listed or not (LDAP) |
| `session` | the first one | session to start with, by desktop file name (`niri`, `i3`); Wayland's come first |
| `remember` | `true` | start with the last login's user and that user's last session instead |
| `theme` | `dark` | `dark`, `light`, a theme's name or a theme file's path; see below |
| `output` | see below | screen with the form (`DP-1`), or `cursor` for the one the pointer is on |
| `scale` | `0` | UI scale; `0` takes it from the screen's size |
| `idle` | `60` | seconds without input before the screen goes dark and the backlight low; `0` for never |
| `x11Wrapper` | `startx /usr/bin/env` | what starts an X server for an X11 session, with its command after it |

The last login is kept in `/var/lib/yokoso/state.json`, so the greeter's
account has to own `/var/lib/yokoso` (the NixOS module makes it). Without
it nothing is remembered, and `user` and `session` decide.

A user's picture is `avatars/<name>` next to the config
(`/etc/yokoso/avatars/alice`, in any format Qt reads), else AccountsService's
(`/var/lib/AccountsService/icons/alice`); without one, it's their initial. On
NixOS, `services.yokoso.avatars.alice = ./alice.png;` puts it there.

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
| `colors.subtext` | the date, the session, hints, the power buttons' icons and the status in the corner |
| `colors.dim` | the field's placeholder, its arrow until something is typed and the arrows by the names |
| `colors.field` | the password field and the user and session lists |
| `colors.accent` | the avatar, what's under the pointer, an armed power button, the picked entry in a list, the field's outline and its button once something is typed, and the spinner while a password is checked |
| `colors.error` | errors (and the field's outline with one), the Caps Lock warning and a low battery |
| `font` | a font family; `""` for the default one |
| `background` | an image for every screen, or one per output with `"*"` for the rest (`{"DP-1": "a.jpg", "*": "b.jpg"}`), cropped to fill each screen; paths are relative to the theme file |
| `backgroundDim` | how much `colors.background` covers the image, `0` to `1` |
| `animations` | the parts coming in one after another at the start and going as the session starts, the form sliding to another screen, buttons that answer the pointer, a ring running out around an armed power button, a shake at a wrong password, the spinner and lists that drop in; `false` for none |
| `status` | what a top corner shows: `"hostname"`, `"battery"` (a laptop's), both by default; `[]` for nothing |
| `layout.form` | the form in the `"center"` of the screen, on the `"left"` or on the `"right"` |
| `layout.clock` | the clock `"above"` the form, at the `"top"` of the screen, or `"none"` |
| `layout.power` | the power buttons at the `"bottom"`, in its right `"corner"`, or `"none"` |
| `layout.status` | the status in the `"top-right"` or `"top-left"` corner |
| `clockFormat` | the time as a [Qt format](https://doc.qt.io/qt-6/qml-qtqml-qt.html#formatDateTime-method) (`"HH:mm"`); `""` for the locale's short one |
| `dateFormat` | the date likewise (`"dddd d MMMM"`); `""` for the locale's long one without the year |
| `radius` | the corners of the field, the buttons, the lists and the avatar, in pixels; `null` for round |
| `fieldWidth` | the form's width, in pixels |

A theme that can't be read is skipped (with a warning in the journal), so a
broken one never stands between anyone and the login.
[`examples/catppuccin-mocha.json`](examples/catppuccin-mocha.json) is one to
start from. On NixOS, `services.yokoso.themes.<name>` writes them.
