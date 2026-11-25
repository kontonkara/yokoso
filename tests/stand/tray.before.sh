# The stand's own tray item, built beside the shell.
cargo build -q --release --example fake_tray
fake_tray=$(dirname "$binary")/examples/fake_tray
