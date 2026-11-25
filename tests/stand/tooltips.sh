# A tooltip shows after the pointer rests a while, not at once, and goes
# when the pointer leaves or clicks.
tips() { "$binary" msg dump | jq .tooltips; }
center() {
	"$binary" msg dump | jq -r "[.nodes[] | select(.role == \"Button\" and .rect[0] > 200)][$1].rect | \"\\(16 + .[0] + .[2] / 2 | floor) \\(16 + .[1] + .[3] / 2 | floor)\""
}
# shellcheck disable=SC2046 # the words are x and y
move $(center 0)
sleep 0.2
[ "$(tips)" = 0 ] || fail "the tooltip showed at once"
sleep 0.8
[ "$(tips)" = 1 ] || fail "no tooltip after resting"
shot tooltip
move 700 600
sleep 0.5
[ "$(tips)" = 0 ] || fail "the tooltip stayed after the pointer left"
