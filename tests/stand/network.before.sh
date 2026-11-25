# The stand's own network manager, built beside the shell, on the stand's
# bus standing in for the system's.
cargo build -q --release --example fake_nm
fake_nm=$(dirname "$binary")/examples/fake_nm
export DBUS_SYSTEM_BUS_ADDRESS=$DBUS_SESSION_BUS_ADDRESS
"$fake_nm" >"$out/fake_nm.log" 2>&1 &
others="${others:-} $!"
for _ in $(seq 50); do
	busctl --user status org.freedesktop.NetworkManager >/dev/null 2>&1 && break
	sleep 0.1
done
