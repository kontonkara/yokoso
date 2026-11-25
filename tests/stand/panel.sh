# The panel grows from its button and back; turned mid-way it turns where
# it is; a hundred openings leave no memory behind and no frames after.
"$binary" msg page panel
sleep 0.3
field() { "$binary" msg dump | jq -r ".$1"; }
shot empty
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
# Mid-way, held still with its content all there, nothing of the content
# shows outside its shape.
"$binary" msg page panel
sleep 0.3
"$binary" msg panel >/dev/null
sleep 1.2
read -r cx cy <<EOT
$("$binary" msg dump | jq -r '.panel_content | "\(16 + .[0] + .[2] * 0.78 | floor) \(16 + .[1] + 82 | floor)"')
EOT
"$binary" msg panel at 0.4 >/dev/null
sleep 0.3
shot closing
[ "$(pixel "$cx" "$cy")" = "$(magick "$out/empty.png" -crop "1x1+$cx+$cy" -depth 8 txt:- | grep -o '#[0-9A-F]\{6\}' | head -n 1)" ] ||
	fail "the panel's content shows outside its shape at $cx $cy"
# Open, then another page: the panel goes with its page.
sleep 1
"$binary" msg panel >/dev/null
sleep 1
"$binary" msg page buttons
sleep 0.2
[ "$("$binary" msg dump | jq -r .panel_visible)" = false ] || fail "the panel stayed on another page"
# The clock at rest after a tick and the clock while the panel moves are
# drawn alike: the hours and minutes stand still to the pixel.
"$binary" msg page panel
sleep 0.5
read -r kx ky kw kh <<EOT
$("$binary" msg dump | jq -r '.clock | map(floor) | "\(16 + .[0]) \(16 + .[1]) \(.[2] * 0.6 | floor) \(.[3])"')
EOT
sleep "$(awk "BEGIN { print 1.7 - $(date +%N) / 1e9 }")"
shot clock-rest
"$binary" msg panel >/dev/null
sleep 0.08
shot clock-moving
moved=$(magick compare -metric AE "$out/clock-rest.png" "$out/clock-moving.png" -crop "${kw}x${kh}+$kx+$ky" null: 2>&1 | cut -d' ' -f1)
[ "$moved" = 0 ] || fail "the clock shifts as the panel moves ($moved pixels)"
