# The overlay shows frame times over the probe, kept fresh while it moves,
# and turning it on draws one frame, not a stream of them.
[ "$("$binary" msg overlay on)" = true ] || fail "overlay not on"
sleep 0.3
shot overlay
shades=$(magick "$last_shot" -crop 300x20+956+474 -format %k info:)
[ "$shades" -gt 10 ] || fail "no overlay above the card: $shades shades"
frames=$("$binary" msg stats | jq .frames)
sleep 1
[ "$("$binary" msg stats | jq .frames)" = "$frames" ] || fail "the overlay keeps drawing while nothing moves"
"$binary" msg move 1
sleep 1.5
shot moved
"$binary" msg overlay off >/dev/null
