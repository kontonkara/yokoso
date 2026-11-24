# In a window, the probe is a plain toplevel: tiled by niri in its column on
# the left, and closed by it.
[ "$("$binary" msg dump | jq -r '.surfaces[0].key')" = Window ] || fail "not a window"
shot window
[ "$(pixel 350 600)" = "#1D1B20" ] || fail "no window in the left column: $(pixel 350 600)"
niri msg action close-window
sleep 0.5
kill -0 "$shell_pid" 2>/dev/null && fail "closing the window left the shell running"
may_quit=1
