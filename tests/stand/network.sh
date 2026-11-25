# The network as NetworkManager has it, the real one on the system bus, only
# read: the same connection; the connections the config names count as a
# VPN, others don't.
dump() { "$binary" msg dump; }
nm() { busctl --system get-property org.freedesktop.NetworkManager "$1" "$2" "$3" | cut -d' ' -f2- | tr -d '"'; }
sleep 0.5
[ "$(dump | jq -c '.bars[0].network')" != null ] || fail "no network part"
primary=$(nm /org/freedesktop/NetworkManager org.freedesktop.NetworkManager PrimaryConnection)
if [ "$primary" = / ]; then
	[ "$(dump | jq -r .network.name)" = "" ] || fail "no connection here, and the bar names one"
	echo "not connected here: only that checked"
	return 0
fi
want=$(nm "$primary" org.freedesktop.NetworkManager.Connection.Active Id)
[ "$(dump | jq -r .network.name)" = "$want" ] || fail "the bar says $(dump | jq -r .network.name), NetworkManager $want"
shot network
# The primary connection named as a VPN: the bar counts it so at once.
printf '[bar.network]\nvpn = ["%s"]\n' "$want" >"$XDG_CONFIG_HOME/yokoso/config.toml"
sleep 0.5
[ "$(dump | jq -c '.network.vpns')" = "[\"$want\"]" ] || fail "a named connection is not a VPN: $(dump | jq -c .network.vpns)"
