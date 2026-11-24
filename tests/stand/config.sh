# The config is read as it changes: a good one takes effect at once, a
# broken one is reported with its place and the last good one stays.
conf=$XDG_CONFIG_HOME/yokoso/config.toml
dump() { "$binary" msg dump | jq -r ".$1"; }

printf '[probe]\naccent = "#00ff00"\n' >"$conf"
sleep 0.5
shot green
[ "$(pixel 1170 532)" = "#00FF00" ] || fail "the accent did not change: $(pixel 1170 532)"

printf '[probe]\naccent = \n' >"$conf"
sleep 0.5
dump config_error | grep -q 'config.toml:2:' || fail "no place in the error: $(dump config_error)"
"$binary" validate >/dev/null 2>&1 && fail "validate passed a broken config"
"$binary" validate 2>&1 | grep -q 'config.toml:2:' || fail "validate says no place"
shot kept
[ "$(pixel 1170 532)" = "#00FF00" ] || fail "a broken config changed the shell"

printf '[probe]\naccent = "#0000ff"\n' >"$conf"
sleep 0.5
[ "$(dump config_error)" = null ] || fail "the error stayed: $(dump config_error)"
"$binary" validate >/dev/null || fail "validate failed a good config"
"$binary" msg reload || fail "reload failed"
