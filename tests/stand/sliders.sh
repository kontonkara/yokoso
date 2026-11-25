# Sliders: a click sets the value under the pointer, the keys move the
# focused one, the disabled one takes nothing. Dragging and the wheel are in
# the unit tests: the stand has no held button to drag with.
"$binary" msg page sliders
sleep 0.3
track() { # where the Nth slider's handle runs, on the screen: "x y w"
	"$binary" msg dump | jq -r "[.nodes[] | select(.role == \"Slider\")][$1].rect | \"\\(16 + .[0] | floor) \\(16 + .[1] + .[3] / 2 | floor) \\(.[2] - 56 | floor)\""
}
value() { "$binary" msg dump | jq ".$1"; }
read -r x y w <<EOT
$(track 0)
EOT
# The middle of the track is about 50.
move $((x + w / 2)) "$y"
click
v=$(value volume)
[ "${v%.*}" -ge 48 ] && [ "${v%.*}" -le 52 ] || fail "a click at the middle gave $v"
shot clicked
wtype -k Home
sleep 0.2
[ "$(value volume)" = 0 ] || [ "$(value volume)" = 0.0 ] || fail "Home on the focused slider gave $(value volume)"
wtype -k Right
sleep 0.2
[ "$(value volume)" = 1 ] || [ "$(value volume)" = 1.0 ] || fail "Right gave $(value volume)"
read -r x y w <<EOT
$(track 3)
EOT
before=$("$binary" msg dump)
move $((x + 20)) "$y"
click
[ "$("$binary" msg dump | jq '[.volume, .brightness, .fine]')" = "$(echo "$before" | jq '[.volume, .brightness, .fine]')" ] ||
	fail "the disabled slider changed something"
shot sliders
