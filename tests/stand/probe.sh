# The probe is on the output, bottom right, in its colors.
shot probe
[ "$(pixel 1300 868)" = "#1D1B20" ] || fail "no card where the probe should be: $(pixel 1300 868)"
[ "$(pixel 100 100)" != "#1D1B20" ] || fail "the card is where it should not be"
log | grep -q 'first frame' || fail "no first frame in the log"
# Its text is there: a block of many shades, as antialiased glyphs make.
shades=$(magick "$last_shot" -crop 380x80+972+704 -format %k info:)
[ "$shades" -gt 20 ] || fail "no text on the probe: $shades shades"
