# Each scale reaches the probe, and its buffer grows with it: still crisp,
# still where it was. The card's corner, 32 logical pixels from the screen's, below its track.
for scale in 1.25 1.5 2; do
	niri msg output winit scale "$scale"
	sleep 0.5
	log | grep -q "scale=$scale" || fail "the probe never heard of scale $scale"
	shot "scale-$scale"
	at=$(awk "BEGIN { print int(32 * $scale) }")
	[ "$(pixel $((1400 - at)) $((900 - at)))" = "#1D1B20" ] || fail "no card at scale $scale"
done
