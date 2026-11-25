# Toggles: the quick one flips at once, the slow one waits for its answer and
# pulses meanwhile, the disabled one takes nothing.
"$binary" msg page toggles
sleep 0.3
toggle() { # the middle of the Nth toggle, on the screen
	"$binary" msg dump | jq -r "[.nodes[] | select(.role == \"Toggle\")][$1].rect | \"\\(16 + .[0] + .[2] / 2 | floor) \\(16 + .[1] + .[3] / 2 | floor)\""
}
state() { "$binary" msg dump | jq -r ".$1"; }
# shellcheck disable=SC2046 # the words are x and y
move $(toggle 0)
click
[ "$(state quick)" = false ] || fail "the quick toggle did not flip"
# shellcheck disable=SC2046
move $(toggle 1)
click
[ "$(state slow_pending)" = true ] || fail "the slow toggle is not waiting"
[ "$(state slow)" = false ] || fail "the slow toggle flipped before its answer"
frames=$("$binary" msg stats | jq .frames)
sleep 0.5
shot waiting
[ "$("$binary" msg stats | jq .frames)" -gt "$((frames + 10))" ] || fail "the waiting toggle does not pulse"
sleep 1
[ "$(state slow)" = true ] && [ "$(state slow_pending)" = false ] || fail "no answer came to the slow toggle"
frames=$("$binary" msg stats | jq .frames)
sleep 1
[ "$("$binary" msg stats | jq .frames)" = "$frames" ] || fail "frames go on after the answer"
# shellcheck disable=SC2046
move $(toggle 2)
click
[ "$(state quick)" = false ] && [ "$(state slow)" = true ] || fail "the disabled toggle changed something"
shot toggles
