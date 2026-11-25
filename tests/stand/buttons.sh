# Buttons: hovered under the pointer, pressed by a click, and the disabled
# ones take nothing. niri puts the window 16 px in from the corner.
center() { # the middle of the Nth button, on the screen
	"$binary" msg dump | jq -r "[.nodes[] | select(.role == \"Button\")][$1].rect | \"\\(16 + .[0] + .[2] / 2 | floor) \\(16 + .[1] + .[3] / 2 | floor)\""
}
presses() { "$binary" msg dump | jq .presses; }
# shellcheck disable=SC2046 # the words are x and y
move $(center 0)
[ "$("$binary" msg dump | jq -r .hovered)" != null ] || fail "the first button is not hovered"
click
[ "$(presses)" = 1 ] || fail "a click on a button did not count"
move $(center 1)
click
[ "$(presses)" = 2 ] || fail "a click on the second button did not count"
move $(center 4)
click
[ "$(presses)" = 2 ] || fail "a disabled button counted a click"
shot buttons
# The keyboard: Tab shows the focus ring on the first button, Return presses.
wtype -k Tab
wtype -k Return
sleep 0.3
[ "$(presses)" = 3 ] || fail "Return on the focused button did not count: $(presses)"
shot focused
