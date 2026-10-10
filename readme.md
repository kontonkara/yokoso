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

The module turns on greetd, writes `/etc/yokoso/config.json`, and passes on
the keyboard layout from `services.xserver.xkb`.

## Config

`/etc/yokoso/config.json`. Every key is optional:

| Key | Default | |
|---|---|---|
| `user` | the first account | account to log in |
| `session` | the first one | Wayland session, by desktop file name (`niri`) |
| `scale` | `0` | UI scale; `0` takes it from the screen's size |
