# Scope

What yokoso does before 1.0, in the order it is built, and what it does not.
Each stage is done when its scenarios pass live, in a real session, and the
part is in daily use. Each accepted stage gets a tag.

## Before 1.0

| Stage | What |
| --- | --- |
| 0. Skeleton | Workspace, event loop, drawing on the integrated GPU, text, input, surfaces on every output with hotplug and scale, config with reload, `yokoso msg`, window mode |
| 1. Elements and motion | Element set, theme, motion system, demo window |
| 2. Bar | The bar and its services: niri, sound, network, battery, tray, layout |
| 3. Notifications and OSD | Own notification server, toasts, history, do not disturb, rules, OSD |
| 4. Settings window | The window, pages for stages 2–3 |
| 5. Launcher | Search, modes, dmenu, launching in a scope |
| 6. Control center | Sound, brightness, Wi-Fi with its agent, Bluetooth with its agent, power profile, night light |
| 7. Lock and login | Lock screen, sleep and idle, greeter, polkit |
| 8. The rest | Wallpaper (change, slideshow), clipboard, recording and screenshots |

## After 1.0

Dock, desktop widgets and weather, plugins, other compositors, more bar
shapes.

## Not at all

- Not a compositor, and no X11.
- yokoso does not edit the compositor's config: it ships a snippet and a Nix
  module instead.
- No toolkit underneath (GTK, Qt): yokoso draws itself.
