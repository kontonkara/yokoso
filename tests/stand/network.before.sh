# The stand's own network manager, built beside the shell, on the stand's
# bus standing in for the system's; the radios' switches a pipe the test
# writes the kernel's events to, held open here.
cargo build -q --release --example fake_nm
fake_nm=$(dirname "$binary")/examples/fake_nm
export DBUS_SYSTEM_BUS_ADDRESS=$DBUS_SESSION_BUS_ADDRESS
"$fake_nm" >"$out/fake_nm.log" 2>&1 &
others="${others:-} $!"
for _ in $(seq 50); do
	busctl --user status org.freedesktop.NetworkManager >/dev/null 2>&1 && break
	sleep 0.1
done
mkfifo "$out/rfkill"
exec 3<>"$out/rfkill"
export YOKOSO_RFKILL="$out/rfkill"
# Two radios, as the kernel tells them at first: Wi-Fi, and Bluetooth
# blocked, as it is off most of the time.
printf '\000\000\000\000\001\000\000\000\001\000\000\000\002\000\001\000' >&3
