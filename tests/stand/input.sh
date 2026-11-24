# The pointer and the keyboard reach the probe: a click gives it the
# keyboard, typing shows in its field, scrolling says from what. wlrctl
# scrolls as a continuous source: a real wheel and fingers are for the live
# test.
dump() { "$binary" msg dump | jq -r ".$1"; }
move 1100 795
dump heard | grep -q "pointer at" || fail "no pointer: $(dump heard)"
click
sleep 0.2
wtype "Hi, Γεια σου"
sleep 0.3
[ "$(dump typed)" = "Hi, Γεια σου" ] || fail "typed: $(dump typed)"
wtype -k BackSpace
sleep 0.2
[ "$(dump typed)" = "Hi, Γεια σο" ] || fail "after backspace: $(dump typed)"
wlrctl pointer scroll 1 0
sleep 0.2
[ "$(dump scrolled)" = "continuous: 0.0 1.0 px" ] || fail "scrolled: $(dump scrolled)"
shot input
