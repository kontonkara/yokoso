# The stand's own network manager and Bluetooth, built beside the shell, on
# the stand's bus standing in for the system's.
cargo build -q --release --example fake_nm --example fake_bluez
examples=$(dirname "$binary")/examples
export DBUS_SYSTEM_BUS_ADDRESS=$DBUS_SESSION_BUS_ADDRESS
"$examples/fake_nm" >"$out/fake_nm.log" 2>&1 &
others="${others:-} $!"
"$examples/fake_bluez" >"$out/fake_bluez.log" 2>&1 &
others="${others:-} $!"
for _ in $(seq 50); do
	busctl --user status org.freedesktop.NetworkManager >/dev/null 2>&1 &&
		busctl --user status org.bluez >/dev/null 2>&1 && break
	sleep 0.1
done
