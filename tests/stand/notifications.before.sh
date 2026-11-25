# Another server holds the name first, as one already running would; the
# names on the bus watched, to hear what the shell says to apps.
cargo build -q --release --example hold_name
"$(dirname "$binary")/examples/hold_name" org.freedesktop.Notifications >"$out/hold_name.log" 2>&1 &
holder=$!
others="${others:-} $holder"
for _ in $(seq 50); do
	busctl --user status org.freedesktop.Notifications >/dev/null 2>&1 && break
	sleep 0.1
done
dbus-monitor --session "type='signal',interface='org.freedesktop.Notifications'" >"$out/signals.log" 2>&1 &
others="${others:-} $!"
