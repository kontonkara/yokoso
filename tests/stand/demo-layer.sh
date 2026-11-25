# The demo as a layer over the desktop, in the middle of the output.
nodes=$("$binary" msg dump | jq '.nodes | length')
[ "$nodes" -ge 3 ] || fail "only $nodes elements"
shot layer
[ "$(pixel 700 450)" != "$(pixel 100 100)" ] || fail "no layer in the middle"
# Escape, with nothing to unwind, closes it.
wtype -k Escape
sleep 0.5
kill -0 "$shell_pid" 2>/dev/null && fail "Escape left the layer open"
may_quit=1
