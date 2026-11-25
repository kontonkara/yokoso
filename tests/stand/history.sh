# The bell and the history: the bell counts what is new; the history opens
# from it, apps in the order of their newest, each folding away on a click;
# one taken out tells its app; quiet turned on keeps toasts away but not the
# history, and the middle button on the bell turns it off; all cleared.
dump() { "$binary" msg dump; }
notes() { dump | jq -c ".notifications$1"; }
popup() { dump | jq -c ".popup${1:+.$1}"; }
notify() {
	busctl --user call org.freedesktop.Notifications /org/freedesktop/Notifications \
		org.freedesktop.Notifications Notify 'susssasa{sv}i' "$@" | cut -d' ' -f2
}
until_says() {
	for _ in $(seq 40); do
		[ "$($1)" = "$2" ] && return 0
		sleep 0.1
	done
	fail "$1 says $($1), not $2"
}
rows() { popup shows.history; }
toasts() { notes .toasts; }
# Row `n` of the history, from the top: its middle, or a point `dx` from its right.
row() { popup to | jq -r "\"\(.[0] + .[2] - ${2:-200} | floor) \(.[1] + 10 + 52 + $1 * 52 + 26 | floor)\""; }
sleep 0.3
notify Chat 0 "" "Ann" "Are we still on?" 0 0 300 >/dev/null
mail=$(notify Mail 0 "" "Letter" "From the bank" 0 0 300)
notify Chat 0 "" "Bob" "A photo" 0 0 300 >/dev/null
until_says toasts '[]'
[ "$(notes .unread)" = 3 ] || fail "the bell counts $(notes .unread), not 3"
read -r bx by <<EOT
$(dump | jq -r '.bars[0].notifications | "\(8 + .[0] + .[2] / 2 | floor) \(8 + .[1] + .[3] / 2 | floor)"')
EOT
move "$bx" "$by"
click
until_says rows 5
[ "$(notes .unread)" = 0 ] || fail "still $(notes .unread) new with the history open"
shot history
# Chat, the newest app, first: a click on its row folds its two away.
read -r x y <<EOT
$(row 0)
EOT
move "$x" "$y"
click
until_says rows 3
# Mail's one taken out: the app told it was dismissed.
read -r x y <<EOT
$(row 2 24)
EOT
move "$x" "$y"
click
until_says rows 1
grep -B1 -A2 'member=NotificationClosed' "$out/signals.log" | grep -q "uint32 $mail" || fail "Mail was not told"
# Quiet, by its switch: what comes is kept, no toast.
read -r x y <<EOT
$(popup shows.quiet | jq -r '"\(.[0] + .[2] / 2 | floor) \(.[1] + .[3] / 2 | floor)"')
EOT
move "$x" "$y"
click
until_says "notes .quiet" true
wtype -k Escape
sleep 0.6
notify Chat 0 "" "Cat" "Meow" 0 0 0 >/dev/null
sleep 0.4
[ "$(toasts)" = '[]' ] || fail "a toast while quiet: $(toasts)"
[ "$(notes '.list | length')" = 3 ] || fail "not kept while quiet: $(notes .list)"
# The middle button on the bell: quiet off.
move "$bx" "$by"
click middle
until_says "notes .quiet" false
"$binary" msg notifications clear >/dev/null
until_says "notes .list" '[]'
