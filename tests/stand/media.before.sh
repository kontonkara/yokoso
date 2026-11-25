# The stand's own media player, built beside the shell.
cargo build -q --release --example fake_player
fake_player=$(dirname "$binary")/examples/fake_player
