# The tray, on the stand's own bus: yokoso watches for items, a fake one
# registers, and shows; a click activates it, the middle button its second
# action, the wheel scrolls it.
dump() { "$binary" msg dump; }
log=$out/tray.log
"$fake_tray" "$log" >"$out/fake_tray.log" 2>&1 &
own $!
for _ in $(seq 50); do
	[ "$(dump | jq '.tray | length')" = 1 ] && break
	sleep 0.1
done
[ "$(dump | jq -c '.tray[0] | [.title, .icon]')" = '["Fake tray item",[48,48]]' ] || fail "the item came as $(dump | jq -c .tray)"
sleep 0.3
shot tray
read -r x y <<EOT
$(dump | jq -r '.bars[0].tray[0] | "\(8 + .[0] + .[2] / 2 | floor) \(8 + .[1] + .[3] / 2 | floor)"')
EOT
# Its teal, drawn where it is.
[ "$(pixel "$x" "$y")" = "#00A0A0" ] || fail "no icon at $x $y: $(pixel "$x" "$y")"
move "$x" "$y"
click
click middle
wlrctl pointer scroll -40 0
sleep 0.5
[ "$(grep -v registered "$log" | tr '\n' ' ')" = "activate secondary scroll 1 vertical " ] || fail "the item heard: $(cat "$log")"
