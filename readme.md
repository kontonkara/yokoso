# yokoso

[![check](https://github.com/kontonkara/yokoso/actions/workflows/check.yml/badge.svg)](https://github.com/kontonkara/yokoso/actions/workflows/check.yml)

*ようこそ, "welcome".* A [greetd](https://sr.ht/~kennylevinsen/greetd/)
greeter on [Quickshell](https://quickshell.org). It runs in
[cage](https://github.com/cage-kiosk/cage), so it works whatever desktop it
logs into, and it's light enough for an integrated GPU.

<p>
  <img src="docs/images/dark.webp" width="49%" alt="The dark theme over a wallpaper">
  <img src="docs/images/light.webp" width="49%" alt="The light theme, with the user list open">
</p>

- PAM's prompts as PAM sends them, a fingerprint reader's included; the
  accounts to pick from, or a name to type; the sessions in the system's
  language, X11 ones too, and any of your own.
- The last login's user and session to start with, the system's keyboard
  layout, a Caps Lock warning, the power buttons logind allows, a reboot
  to the firmware's setup or into Windows among them.
- `dark`, `light` and `high-contrast` themes; any other is a small JSON
  file, and can bring a QML look of its own, made of yokoso's parts or
  from scratch. Tab reaches everything, and an eye shows the password.
- The UI scale from the screen's size, only the GPU that drives the screens
  (a PRIME laptop's NVIDIA one stays asleep), the form on the screen in
  use, and the backlight low after a minute idle.
- greetd's text greeter if yokoso can't start, a broken config or theme
  left out with a warning, Ctrl+Alt+Shift+D for the built-in look, and a
  word when a session ends right after it starts.

## Try it

```sh
nix run github:kontonkara/yokoso#preview
```

The greeter in a window, logging in against greetd's `fakegreet`: the
password is `password`, then `9`. `YOKOSO_CONFIG=<file>` tries a config.

## On NixOS

```nix
{
  inputs.yokoso.url = "github:kontonkara/yokoso";

  # in a module:
  imports = [ inputs.yokoso.nixosModules.default ];
  services.yokoso = {
    enable = true; # greetd, with yokoso as its greeter
    settings.theme = "light"; # /etc/yokoso/config.json
  };
}
```

The module's other options (themes, pictures, the cursor) are with the
[config](docs/config.md#on-nixos).

## On other systems

yokoso needs greetd (with agreety), Quickshell 0.3.1 or newer, cage, jq,
awk and systemd. On Arch, [`dist/arch/PKGBUILD`](dist/arch/PKGBUILD) builds
`yokoso-git`; elsewhere:

```sh
make install PREFIX=/usr
systemd-tmpfiles --create
```

Then in `/etc/greetd/config.toml`:

```toml
[default_session]
command = "yokoso"
user = "greeter"
```

and `systemctl enable greetd`, with the display manager before it
disabled. `/var/lib/yokoso` and `/var/cache/yokoso` belong to `greeter`;
for another account, copy `/usr/lib/tmpfiles.d/yokoso.conf` to
`/etc/tmpfiles.d/` with its name.

## Documentation

- [Using yokoso](docs/usage.md): logging in, the keys, several screens,
  idle, the keyboard layout and the cursor
- [Config](docs/config.md): `/etc/yokoso/config.json`, every key, and the
  NixOS module's options
- [Themes](docs/themes.md): colors, a background, where the parts go
- [Your own look](docs/looks.md): a look in QML, what it can use, the parts
- [When something goes wrong](docs/troubleshooting.md)
- [Development](docs/development.md): the preview, the shots, the tests
