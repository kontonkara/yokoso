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
