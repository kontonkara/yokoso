# The probe is on the output, bottom right, in its colors.
shot probe
[ "$(pixel 1300 850)" = "#1D1B20" ] || fail "no card where the probe should be: $(pixel 1300 850)"
[ "$(pixel 100 100)" != "#1D1B20" ] || fail "the card is where it should not be"
log | grep -q 'first frame' || fail "no first frame in the log"
