# What the shell says to apps, heard on the bus.
dbus-monitor --session "type='signal',interface='org.freedesktop.Notifications'" >"$out/signals.log" 2>&1 &
others="${others:-} $!"
