# The panel grows from its button and back; turned mid-way it turns where
# it is; a hundred openings leave no memory behind and no frames after.
"$binary" msg page panel
sleep 0.3
field() { "$binary" msg dump | jq -r ".$1"; }
"$binary" msg panel >/dev/null
sleep 0.08
shot growing
[ "$(field panel_visible)" = true ] || fail "the panel does not show while it grows"
sleep 1
shot open
# Closing, then turned back mid-way: it ends open.
"$binary" msg panel >/dev/null
sleep 0.08
"$binary" msg panel >/dev/null
sleep 1
[ "$(field panel_open)" = true ] && [ "$(field panel_visible)" = true ] || fail "the turned panel did not end open"
"$binary" msg panel >/dev/null
sleep 1
[ "$(field panel_visible)" = false ] || fail "the closed panel still shows"
before=$(rss)
for _ in $(seq 100); do
	"$binary" msg panel >/dev/null
	sleep 0.05
done
sleep 1.5
after=$(rss)
echo "memory: $before KiB, then $after KiB after 100 openings and closings"
[ $((after - before)) -lt 4096 ] || fail "memory grew by $((after - before)) KiB"
# The clock ticks on this page, a frame a second: elsewhere, nothing moves.
"$binary" msg page buttons
sleep 1.2
frames=$("$binary" msg stats | jq .frames)
sleep 1.2
[ "$("$binary" msg stats | jq .frames)" = "$frames" ] || fail "frames go on after the panel rests"
