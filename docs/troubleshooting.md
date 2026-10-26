# When something goes wrong

- `journalctl -b -t yokoso` has what the launcher, cage and Quickshell
  said: a theme or a config left out, unknown keys, the wait for a GPU's
  driver, a session left out for its `TryExec`, why yokoso fell back to
  agreety.
- If yokoso can't start, greetd's text greeter (agreety) logs in to the
  same session instead.
- A theme or a look that keeps anyone from logging in: Ctrl+Alt+Shift+D
  brings up the built-in look in the built-in theme. Try them with `yokoso
  preview` (`YOKOSO_CONFIG=<file>`) before they're the login screen.
- A session that ends right after it starts says so under the field; its
  own messages are in `journalctl -b`.
- An expired password: greetd can't change it, so change it with `passwd` on
  a text console (Ctrl+Alt+F2).
- A power button that isn't there: logind doesn't allow it to the greeter
  (no polkit, or its rules), the firmware isn't UEFI, or, for an entry, the
  boot loader isn't systemd-boot or has no entry by that id.
- A screen that stays dark although it's connected: click on it, or press
  F4, and the form goes there.
- Up to 10 s of black at boot: the GPU's driver isn't in the initrd, and
  yokoso waits for it rather than start on the firmware's framebuffer.
  Putting it there (`hardware.amdgpu.initrd.enable`,
  `boot.initrd.kernelModules`, or the distribution's way) saves the wait.
