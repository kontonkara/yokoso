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
`/var/cache/yokoso`.

## Config

`/etc/yokoso/config.json`. Every key is optional:

| Key | Default | |
|---|---|---|
| `user` | the first account | account to log in, listed or not (LDAP) |
| `session` | the first one | session, by desktop file name (`niri`, `i3`); Wayland's come first |
| `theme` | `dark` | `dark`, `light`, a theme's name or a theme file's path; see below |
| `output` | see below | screen with the form (`DP-1`), or `cursor` for the one the pointer is on |
| `scale` | `0` | UI scale; `0` takes it from the screen's size |
| `x11Wrapper` | `startx /usr/bin/env` | what starts an X server for an X11 session, with its command after it |

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
`"colors": {"accent": "#fab387"}` in the config changes just that.

| Key | |
|---|---|
| `colors.background` | behind everything, where there's no image |
| `colors.text` | the clock, the username and what's typed |
| `colors.subtext` | the date and hints |
| `colors.dim` | the field's placeholder and the power buttons |
| `colors.field` | the password field |
| `colors.accent` | a power button under the pointer, or armed |
| `colors.error` | errors and the Caps Lock warning |
| `font` | a font family; `""` for the default one |
| `background` | an image for every screen, or one per output with `"*"` for the rest (`{"DP-1": "a.jpg", "*": "b.jpg"}`), cropped to fill each screen; paths are relative to the theme file |
| `backgroundDim` | how much `colors.background` covers the image, `0` to `1` |

A theme that can't be read is skipped (with a warning in the journal), so a
broken one never stands between anyone and the login.
[`examples/catppuccin-mocha.json`](examples/catppuccin-mocha.json) is one to
start from. On NixOS, `services.yokoso.themes.<name>` writes them.
