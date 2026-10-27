# Themes

The built-in ones are `dark` (the default), `light` and `high-contrast`.
Any other is a JSON file: `"theme": "mine"` reads `themes/mine.json` next to
the config (`/etc/yokoso/themes/mine.json`), and a path reads that file. On
NixOS, `services.yokoso.themes.<name>` writes them.
`"$schema": "https://raw.githubusercontent.com/kontonkara/yokoso/main/shell/schema/theme.json"`
lets an editor check one.

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
when it doesn't say; by name, a theme in the same folder first), and the
config's keys on top of the theme; `colors` and `layout` merge key by key.
A theme that can't be read is left out, with a warning in the journal.
[`examples/catppuccin-mocha.json`](../examples/catppuccin-mocha.json) is one
to start from.

| Key | |
|---|---|
| `colors.background` | behind everything, where there's no image |
| `colors.text` | the clock, the name, the lists, what's typed |
| `colors.subtext` | the date, the session, hints, the power buttons, the status in the corner |
| `colors.dim` | the field's placeholder and the arrows by the names |
| `colors.field` | the field and the lists |
| `colors.accent` | the avatar, what's under the pointer or Tab's ring, an armed power button, the field's outline and its button once something is typed |
| `colors.error` | errors, the Caps Lock and layout warnings and a low battery |
| `colors.outline` | a solid outline around the field, the session, the power buttons and the lists; `""` for a faint one around the field and the lists |
| `font` | a font family; `""` for the default one |
| `background` | a picture for every screen, or one per output with `"*"` for the rest (`{"DP-1": "a.jpg", "*": "b.jpg"}`); a directory of them, one after another; a video (`.mp4`, `.webm`, `.mkv`, `.mov`, with `services.yokoso.video` on NixOS, Qt Multimedia elsewhere). Paths are relative to the theme; a user's own (`backgrounds/<name>` next to the config) goes over it |
| `backgroundDim` | how much `colors.background` covers the image, `0` to `1` |
| `backgroundEvery` | with a directory, the seconds before the next picture (`600`); `0` for one, picked at random, the whole time |
| `animations` | `"auto"` (the default) for things that move unless everything is drawn in software (a VM without 3D), `true`, or `false` for none |
| `status` | what a top corner shows: `"hostname"`, `"battery"`, both by default; `[]` for nothing |
| `layout.form` | the form in the `"center"`, on the `"left"` or on the `"right"` |
| `layout.clock` | the clock `"above"` the form, at the `"top"`, or `"none"` |
| `layout.power` | the power buttons at the `"bottom"`, in its right `"corner"`, or `"none"` |
| `layout.status` | the status in the `"top-right"` or `"top-left"` corner |
| `clockFormat`, `dateFormat` | the time and the date as a [Qt format](https://doc.qt.io/qt-6/qml-qtqml-qt.html#formatDateTime-method) (`"HH:mm"`, `"dddd d MMMM"`); `""` for the locale's |
| `radius` | the corners of the field, the buttons, the lists and the avatar, in pixels; `null` for round |
| `fieldWidth` | the form's width, in pixels |
| `look` | a QML file that draws the login screen instead: [your own look](looks.md) |

<img src="images/layout.webp" width="100%" alt="Catppuccin Mocha with the form on the left, the clock at the top and the power buttons in a corner">

*`catppuccin-mocha.json`, with `"layout": {"form": "left", "clock": "top",
"power": "corner", "status": "top-left"}` and `"radius": 12`.*
