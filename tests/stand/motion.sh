# The square moves at the output's rate while asked to, then frames stop.
shot before
"$binary" msg move 2
sleep 1
dump=$("$binary" msg dump)
[ "$(echo "$dump" | jq '.surfaces[0].moving')" = true ] || fail "the square does not move"
refresh=$(echo "$dump" | jq '.surfaces[0].refresh')
fps=$(echo "$dump" | jq '.surfaces[0].fps')
echo "moving: $fps frames a second at $refresh Hz"
# A nested compositor may not know its refresh: then at least 50.
awk "BEGIN { exit !($refresh > 0 ? $fps > $refresh * 0.9 : $fps > 50) }" || fail "$fps frames a second at $refresh Hz"
shot moving
sleep 1.5
[ "$("$binary" msg dump | jq '.surfaces[0].moving')" = false ] || fail "still moving after its time"
frames=$("$binary" msg stats | jq .frames)
sleep 1
[ "$("$binary" msg stats | jq .frames)" = "$frames" ] || fail "frames go on after the motion"

# While it moved, frames drew only its track; after, the card is whole.
partial=$("$binary" msg stats | jq .partial)
[ "$partial" -gt 30 ] || fail "only $partial frames drew just the track"
echo "partial frames: $partial"
shot after
# Back at its start, the card is what it was, to the pixel.
differ=$(magick compare -metric AE "$out/before.png" "$out/after.png" null: 2>&1 || true)
[ "${differ%% *}" = 0 ] || fail "$differ pixels differ after partial frames"
