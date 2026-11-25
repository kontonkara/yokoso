# The network as the network manager says, a fake one on the stand's bus:
# Wi-Fi by its strength, off, back; a tunnel the config names counts as a
# VPN. A burst of signals, as a scan sends, leaves the bar following still.
dump() { "$binary" msg dump; }
nm() { busctl --user call org.freedesktop.NetworkManager /stand stand.Network "$@"; }
link() { dump | jq -r .network.link; }
# Waits up to two seconds for `jq-filter` on the dump to say `want`.
until_says() {
	for _ in $(seq 20); do
		[ "$(dump | jq -c "$1")" = "$2" ] && return 0
		sleep 0.1
	done
	fail "$1 is $(dump | jq -c "$1"), not $2"
}
printf '[bar.network]\nvpn = ["myvpn"]\n' >"$XDG_CONFIG_HOME/yokoso/config.toml"
sleep 0.5
[ "$(dump | jq -c '.bars[0].network')" != null ] || fail "no network part"
until_says .network.name '"Stand"'
until_says .network.link '"Wifi { ssid: \"Stand\", strength: 72 }"'
shot wifi
nm Wifi b false
until_says .network.link '"None"'
shot off
nm Wifi b true
until_says .network.name '"Stand"'
nm Vpn b true
until_says .network.vpns '["myvpn"]'
shot vpn
# A scan's worth of signals while the bar reads: it still hears what comes
# after.
nm Burst u 2000
nm Vpn b false
until_says .network.vpns '[]'
nm Wifi b false
until_says .network.link '"None"'
# Its words on resting, below the bar: on a strip of their own, as the bar
# has no room for them, and gone with the pointer.
read -r x y <<EOT
$(dump | jq -r '.bars[0].network | "\(8 + .[0] + .[2] / 2 | floor) \(8 + .[1] + .[3] / 2 | floor)"')
EOT
move "$x" "$y"
sleep 0.2
[ "$(dump | jq .tips)" = 0 ] || fail "the tooltip showed at once"
sleep 1
[ "$(dump | jq .tips)" = 1 ] || fail "no tooltip after resting"
shot tooltip
below=$((8 + 40 + 12))
[ "$(pixel "$x" "$below")" != "$(pixel 4 "$below")" ] || fail "nothing below the bar where the tooltip is: $(pixel "$x" "$below")"
move 700 600
sleep 0.8
[ "$(dump | jq .tips)" = 0 ] || fail "the tooltip stayed after the pointer left"
[ "$(dump | jq '[.surfaces[] | select(.key | test("Tip"))] | length')" = 0 ] || fail "its strip stayed"
