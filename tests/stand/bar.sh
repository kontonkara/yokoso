# The bar floats at the top of the screen, as tall as asked and as far from
# the edges; windows open below it; it hides and comes back on a word.
dump() { "$binary" msg dump; }
sleep 0.3
shot bar
read -r w h <<EOT
$(dump | jq -r '.surfaces[0].size | "\(.[0]) \(.[1])"')
EOT
[ "$w" = $((1400 - 16)) ] && [ "$h" = 40 ] || fail "the bar is ${w}x$h, not 1384x40"
# Its color from 8 pixels in, nothing of it in the margin above.
inside=$(pixel 700 28)
[ "$inside" != "$(pixel 700 3)" ] || fail "nothing at the bar's place: $inside"
[ "$(pixel 4 28)" = "$(pixel 700 3)" ] || fail "the bar reaches the screen's side"
# The clock says the time, in the middle.
[ "$(dump | jq -r .time)" = "$(date +%H:%M)" ] || fail "the clock says $(dump | jq -r .time)"
read -r cx cw <<EOT
$(dump | jq -r '.bars[0].clock | "\(.[0]) \(.[2])"')
EOT
middle=$(awk "BEGIN { print int($cx + $cw / 2 + 8) }")
[ $((middle - 700)) -le 1 ] && [ $((700 - middle)) -le 1 ] || fail "the clock is not in the middle: $middle"
# A window opens below the bar, not under it: as tall as the screen less
# the bar, its margin and niri's two gaps. The window's own socket goes
# elsewhere, or it would meet the bar's.
mkdir -p "$out/window-run"
display="$XDG_RUNTIME_DIR/$WAYLAND_DISPLAY"
XDG_RUNTIME_DIR="$out/window-run" WAYLAND_DISPLAY="$display" env -u __EGL_VENDOR_LIBRARY_FILENAMES "$binary" probe --window >"$out/window.log" 2>&1 &
own $!
for _ in $(seq 50); do
	niri msg -j windows | jq -e 'length > 0' >/dev/null && break
	sleep 0.1
done
tall=$(niri msg -j windows | jq '.[0].layout.tile_size[1] | floor')
[ "$tall" -le $((900 - 48 - 2 * 16)) ] || fail "a window reaches under the bar: $tall tall"
# Hidden, and back.
"$binary" msg bar hide >/dev/null
sleep 0.3
[ "$(dump | jq '.surfaces | length')" = 0 ] || fail "the hidden bar still shows"
"$binary" msg bar show >/dev/null
sleep 0.5
[ "$(dump | jq '.surfaces | length')" = 1 ] || fail "the bar did not come back"
# At rest, nothing is drawn: no frames but the clock's, once a minute.
sleep 1.2
frames=$("$binary" msg stats | jq .frames)
sleep 2
now=$("$binary" msg stats | jq .frames)
[ $((now - frames)) -le 1 ] || fail "$((now - frames)) frames in 2 s at rest"
