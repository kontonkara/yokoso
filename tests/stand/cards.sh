# Tiles: a quick one flips, a slow one waits, its arrow asks for details, the
# disabled one takes nothing.
"$binary" msg page cards
sleep 0.3
tile() { # the Nth tile on the screen: "x y w h" of it
	"$binary" msg dump | jq -r "[.nodes[] | select(.role == \"Tile\")][$1].rect | \"\\(16 + .[0] | floor) \\(16 + .[1] | floor) \\(.[2] | floor) \\(.[3] | floor)\""
}
field() { "$binary" msg dump | jq -c ".$1"; }
shot cards
read -r x y w h <<EOT
$(tile 2)
EOT
move $((x + 30)) $((y + h / 2))
click
[ "$(field 'tiles[2]')" = true ] || fail "the quick tile did not flip"
read -r x y w h <<EOT
$(tile 0)
EOT
move $((x + 30)) $((y + h / 2))
click
[ "$(field 'tiles_pending[0]')" = true ] || fail "the slow tile is not waiting"
move $((x + w - 15)) $((y + h / 2))
click
[ "$(field details)" = 0 ] || fail "the arrow did not ask for details: $(field details)"
sleep 1.5
[ "$(field 'tiles[0]')" = false ] || fail "the slow tile got no answer"
read -r x y w h <<EOT
$(tile 4)
EOT
before=$(field tiles)
move $((x + 30)) $((y + h / 2))
click
[ "$(field tiles)" = "$before" ] || fail "the disabled tile changed something"
