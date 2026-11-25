# The notification server: it waits for the name while another holds it and
# takes it once let go; what apps send is kept as plain text with its
# actions; actions and replies go back to the app, closing tells it why;
# the quiet switch and the history are kept on disk for the owner alone; a
# rule hides what it names.
dump() { "$binary" msg dump; }
notes() { dump | jq -c "${1:-.notifications}"; }
notify() {
	busctl --user call org.freedesktop.Notifications /org/freedesktop/Notifications \
		org.freedesktop.Notifications Notify 'susssasa{sv}i' "$@"
}
until_says() {
	for _ in $(seq 30); do
		[ "$(notes "$1")" = "$2" ] && return 0
		sleep 0.1
	done
	fail "$1 is $(notes "$1"), not $2"
}
sleep 0.3
[ "$(notes .notifications.serving)" = false ] || fail "it took the name another holds"
kill "$holder"
until_says .notifications.serving true
info=$(busctl --user call org.freedesktop.Notifications /org/freedesktop/Notifications org.freedesktop.Notifications GetServerInformation)
echo "$info" | grep -qx 'ssss "yokoso" "yokoso" "[0-9.]*" "1.2"' ||
	fail "the server says it is $info"
id=$(notify Chat 0 "" "<b>Ann</b>" "hi &amp; bye" 6 default Open mark "Mark as read" inline-reply Reply 1 urgency y 1 5000 | cut -d' ' -f2)
until_says '.notifications.list | length' 1
[ "$(notes '.notifications.list[0] | [.summary, .body, .actions, .reply]')" = '["Ann","hi & bye",[["mark","Mark as read"]],"Reply"]' ] ||
	fail "it came as $(notes '.notifications.list[0]')"
# An action and a reply go back to the app.
"$binary" msg notifications invoke "$id" mark >/dev/null
"$binary" msg notifications reply "$id" fine thanks >/dev/null
sleep 0.3
grep -A2 'member=ActionInvoked' "$out/signals.log" | grep -q '"mark"' || fail "no action heard: $(cat "$out/signals.log")"
grep -A2 'member=NotificationReplied' "$out/signals.log" | grep -q '"fine thanks"' || fail "no reply heard"
# The app closes it: gone, and the app told so, by its asking.
busctl --user call org.freedesktop.Notifications /org/freedesktop/Notifications org.freedesktop.Notifications CloseNotification u "$id"
until_says '.notifications.list | length' 0
grep -A2 'member=NotificationClosed' "$out/signals.log" | grep -q 'uint32 3' || fail "not told it was closed as asked"
# Quiet, kept on disk for the owner alone.
notify Mail 0 "" "Letter" "" 0 0 5000 >/dev/null
"$binary" msg notifications quiet on >/dev/null
file=$XDG_STATE_HOME/yokoso/notifications.json
[ "$(stat -c %a "$file")" = 600 ] || fail "the history is readable by others: $(stat -c %a "$file")"
[ "$(jq -c '[.quiet, (.kept | map(.summary))]' "$file")" = '[true,["Letter"]]' ] || fail "kept as $(cat "$file")"
# A rule hides what it names: never kept, and the app told it was dismissed.
printf '[[notifications.rules]]\napp = "spam"\naction = "hide"\n' >"$XDG_CONFIG_HOME/yokoso/config.toml"
sleep 0.5
notify Spam 0 "" "Buy" "" 0 0 5000 >/dev/null
sleep 0.3
[ "$(notes '[.notifications.list[].summary]')" = '["Letter"]' ] || fail "the rule let it in: $(notes)"
grep -A2 'member=NotificationClosed' "$out/signals.log" | grep -q 'uint32 2' || fail "not told it was dismissed"
