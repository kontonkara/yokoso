# The theme and motion change on the fly: light, zoom, motion off; menus
# open on a click and pick by the keys.
"$binary" msg page theme
sleep 0.3
field() { "$binary" msg dump | jq -r ".$1"; }
menu() { # the middle of the Nth menu, on the screen, as zoomed
	"$binary" msg dump | jq -r ".zoom as \$z | [.nodes[] | select(.role == \"Menu\")][$1].rect | \"\\(16 + (.[0] + .[2] / 2) * \$z | floor) \\(16 + (.[1] + .[3] / 2) * \$z | floor)\""
}
pick() { # opens the Nth menu and goes down `by` items from the chosen one
	# shellcheck disable=SC2046 # the words are x and y
	move $(menu "$1")
	click
	for _ in $(seq "$2"); do wtype -k Down; done
	wtype -k Return
	sleep 0.4
}
shot dark
pick 0 1
[ "$(field dark)" = false ] || fail "the theme did not turn light"
shot light
[ "$(pixel 600 600)" != "$(magick "$out/dark.png" -crop 1x1+600+600 -depth 8 txt:- | grep -o '#[0-9A-F]\{6\}' | head -n 1)" ] ||
	fail "light looks the same as dark"
pick 5 2
[ "$(field mode)" = Off ] || fail "motion is $(field mode)"
pick 6 2
[ "$(field zoom)" = 1.5 ] || fail "zoom is $(field zoom)"
shot zoomed
