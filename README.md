# yokoso

A desktop shell for Wayland, written in Rust and drawn on the GPU without a
toolkit. niri first.

**Status:** stage 0, the skeleton. Nothing here is a shell you can use yet;
see [SCOPE.md](SCOPE.md) for the road and [ARCHITECTURE.md](ARCHITECTURE.md)
for how it is built.

## Build

```sh
nix build            # the package
nix develop          # a shell with everything to build and test
cargo build --release
```

Without Nix: a Rust toolchain (1.88 or newer), `pkg-config`, and the
development files of `wayland` and `libxkbcommon`. EGL and GLES are loaded at
run time from `libglvnd` or Mesa.

## Run

```sh
yokoso shell            # on every output
yokoso shell --window   # in a plain window, to try it without the session
yokoso msg ping         # talk to the running shell
yokoso validate         # check the config
```

The config lives in `$XDG_CONFIG_HOME/yokoso/config.toml`.

## Test

```sh
nix develop -c tests/all.sh
```

Unit tests, then scripts on a stand: a headless compositor on the integrated
GPU, so nothing on the desktop is touched.

## License

MIT
