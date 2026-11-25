# The battery as UPower has it, the real one on the system bus, only read:
# the same percentage; a click runs what the config says it opens. Where
# there is no battery, there is no part.
dump() { "$binary" msg dump; }
property() { busctl --system get-property org.freedesktop.UPower /org/freedesktop/UPower/devices/DisplayDevice org.freedesktop.UPower.Device "$1" | cut -d' ' -f2; }
sleep 0.5
if [ "$(property IsPresent)" != true ]; then
	[ "$(dump | jq -c '.bars[0].battery')" = null ] || fail "no battery here, and still a part for it"
	echo "no battery here: only its absence checked"
	return 0
fi
want=$(property Percentage)
[ "$(dump | jq ".battery.percentage == $want")" = true ] || fail "the bar says $(dump | jq -c .battery), UPower $want"
shot battery
# A click opens what the config says.
printf '[bar.battery]\non_click = ["touch", "%s/clicked"]\n' "$out" >"$XDG_CONFIG_HOME/yokoso/config.toml"
sleep 0.5
read -r x y <<EOT
$(dump | jq -r '.bars[0].battery | "\(8 + .[0] + .[2] / 2 | floor) \(8 + .[1] + .[3] / 2 | floor)"')
EOT
move "$x" "$y"
click
sleep 0.3
[ -e "$out/clicked" ] || fail "a click did not run what the config says"
