# Development

`nix run .#preview` opens the working tree's greeter in a window, against
greetd's `fakegreet` (the password is `password`, then `9`);
`YOKOSO_CONFIG=<file>` tries a config. Outside Nix, `bin/yokoso preview`
needs `fakegreet` on the `PATH`, which not every distribution ships (Arch
doesn't).

`nix run .#shots -- <dir>` renders the working tree's greeter headless, with
fake users and sessions, in the scenarios of
[`tests/shots.sh`](../tests/shots.sh) (the lists, a wrong password, two
screens, idle, Russian, Tab, the high-contrast theme, looks of one's own,
...) into `<dir>`; `--against <an earlier dir>` counts the pixels that
changed. `nix run .#docs-shots` redraws the pictures in
[`docs/images`](images).

`nix flake check` runs shellcheck, holds the schemas up to the core and the
themes, and runs the VM tests in
[`tests/vm`](../tests/vm): the greeter under the real greetd, PAM and cage,
logging in through the lists, idle, the power buttons (a boot loader
entry's under systemd-boot too), PAM's messages, an expired password, a
fingerprint, sessions of one's own, a session that ends at once, a GPU
driver that loads after the greeter starts, a layout from localed, a login
without the greeter, the on-screen keyboard, `yokoso lock` in sway, and an
install with `make` instead of the module. GitHub runs it on every push
and pull request.
