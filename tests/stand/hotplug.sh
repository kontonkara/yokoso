# Outputs come and go, again and again: a probe each, nothing breaks.
surfaces() { "$binary" msg dump | jq '.surfaces | length'; }
[ "$(surfaces)" = 1 ] || fail "not one probe at the start"
for round in 1 2 3; do
	swaymsg -q create_output
	sleep 0.5
	[ "$(surfaces)" = 2 ] || fail "round $round: no probe on the new output"
	"$binary" msg dump | jq -e '.surfaces | all(.frames > 0)' >/dev/null || fail "round $round: a probe never drew"
	added=$(swaymsg -t get_outputs | jq -r '.[-1].name')
	swaymsg -q output "$added" unplug
	sleep 0.5
	[ "$(surfaces)" = 1 ] || fail "round $round: the probe stayed after its output went"
done
log | grep -c 'output gone' | grep -q 3 || fail "not three outputs gone in the log"
