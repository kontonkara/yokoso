# Media, on the stand's own bus: nothing without a player; a player with a
# track shows; a click pauses and plays it, the middle button goes on to
# the next track, and the bar follows what the player says.
dump() { "$binary" msg dump; }
media() { dump | jq -c '.media'; }
sleep 0.3
[ "$(media)" = null ] || fail "no player, and still media: $(media)"
"$fake_player" "$out/player.log" >"$out/fake_player.log" 2>&1 &
own $!
for _ in $(seq 50); do
	[ "$(media)" != null ] && break
	sleep 0.1
done
[ "$(media)" = '{"playing":true,"title":"First song"}' ] || fail "the player came as $(media)"
sleep 0.3
shot playing
read -r x y <<EOT
$(dump | jq -r '.bars[0].media | "\(8 + .[0] + .[2] / 2 | floor) \(8 + .[1] + .[3] / 2 | floor)"')
EOT
move "$x" "$y"
click
sleep 0.4
[ "$(media)" = '{"playing":false,"title":"First song"}' ] || fail "a click left $(media)"
shot paused
click middle
sleep 0.4
[ "$(dump | jq -r .media.title)" = "Second song" ] || fail "the middle button left $(media)"
[ "$(tr '\n' ' ' <"$out/player.log")" = "play-pause next " ] || fail "the player heard: $(cat "$out/player.log")"
