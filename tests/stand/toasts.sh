# Toasts: one shows at the top right below the bar and goes in its time, its
# column with it; three show and the rest wait their turn; the close button
# lets the next come; the pointer on one stops its time and keeps it where
# it is, the others going and coming below it; a click and a typed reply
# reach the app. A second toast shows all of itself, not clipped.
dump() { "$binary" msg dump; }
notes() { dump | jq -c ".notifications$1"; }
notify() {
	busctl --user call org.freedesktop.Notifications /org/freedesktop/Notifications \
		org.freedesktop.Notifications Notify 'susssasa{sv}i' "$@" | cut -d' ' -f2
}
until_says() {
	for _ in $(seq 40); do
		[ "$(notes "$1")" = "$2" ] && return 0
		sleep 0.1
	done
	fail "notifications$1 is $(notes "$1"), not $2"
}
# The column's place on the screen: at the right edge, below the bar.
left=$((1400 - 380))
top=$((8 + 40))
# Where in toast `n` (from the top) a point `dx`, `dy` in it is.
in_toast() { notes ".toast_rects[$1]" | jq -r "\"\(.[0] + $left + $2 | floor) \(.[1] + $top + $3 | floor)\""; }
sleep 0.3
# One, for 1.2 s.
notify Mail 0 "" "Letter" "From the bank" 0 0 1200 >/dev/null
until_says .toasts '[1]'
[ "$(dump | jq '[.surfaces[] | select(.key | test("Toasts"))] | length')" = 1 ] || fail "no column for the toast"
until_says .toasts '[]'
sleep 0.5
[ "$(dump | jq '[.surfaces[] | select(.key | test("Toasts"))] | length')" = 0 ] || fail "the column stayed"
# Five that stay: three show, two wait.
for n in 1 2 3 4 5; do notify App 0 "" "Note $n" "" 0 0 0 >/dev/null; done
until_says '| [.toasts, .waiting]' '[[4,3,2],[5,6]]'
sleep 0.5
shot three
# The close button of the top one: the next comes.
read -r x y <<EOT
$(in_toast 0 336 24)
EOT
move "$x" "$y"
click
until_says '| [.toasts, .waiting]' '[[5,3,2],[6]]'
"$binary" msg notifications clear >/dev/null
for id in 5 3 2 6; do busctl --user call org.freedesktop.Notifications /org/freedesktop/Notifications org.freedesktop.Notifications CloseNotification u "$id"; done
until_says .toasts '[]'
# The pointer on one stops its time; off it, it goes.
id=$(notify Chat 0 "" "Ann" "Are we still on for tonight? I booked the table at eight." 6 default Open mark "Mark as read" inline-reply Reply 0 1000)
other=$(notify Mail 0 "" "Letter" "From the bank" 2 default Open 0 0)
until_says .toasts "[$other,$id]"
sleep 0.5
shot two
# The second one, all of it: its summary's line has words in it.
read -r x y <<EOT
$(in_toast 1 14 40)
EOT
colors=$(magick "$last_shot" -crop "200x14+$x+$y" -format %k info:)
[ "$colors" -gt 3 ] || fail "the second toast's words are not there: $colors colors"
read -r x y <<EOT
$(in_toast 1 200 80)
EOT
move "$x" "$y"
sleep 1.5
[ "$(notes .toasts)" = "[$other,$id]" ] || fail "it went with the pointer on it"
# A typed reply reaches the app.
read -r x y <<EOT
$(notes '.toast_rects[1]' | jq -r "\"\(.[0] + $left + 100 | floor) \(.[1] + .[3] + $top - 30 | floor)\"")
EOT
move "$x" "$y"
click
wtype "see you"
wtype -k Return
sleep 0.4
grep -A2 'member=NotificationReplied' "$out/signals.log" | grep -q '"see you"' || fail "no reply heard: $(cat "$out/signals.log")"
until_says .toasts "[$other]"
# A click on one is its default action.
read -r x y <<EOT
$(in_toast 0 100 50)
EOT
move "$x" "$y"
click
sleep 0.4
grep -A2 'member=ActionInvoked' "$out/signals.log" | grep -q '"default"' || fail "no default action heard"
until_says .toasts '[]'
# The pointer on the top one of five: it stays where it is, its time still;
# the others go in theirs, and the two waiting come in below it.
for n in 1 2 3 4 5; do notify Queue 0 "" "Number $n" "" 0 0 1500 >/dev/null; done
sleep 0.5
[ "$(notes '.waiting | length')" = 2 ] || fail "not two waiting: $(notes '| [.toasts, .waiting]')"
first=$(notes '.toasts[0]')
at=$(notes '.toast_rects[0]')
read -r x y <<EOT
$(in_toast 0 200 30)
EOT
move "$x" "$y"
until_says .waiting '[]'
sleep 0.5
[ "$(notes '.toasts[0]')" = "$first" ] || fail "another on top under the pointer: $(notes .toasts)"
[ "$(notes '.toast_rects[0]')" = "$at" ] || fail "it moved under the pointer: $at, then $(notes '.toast_rects[0]')"
[ "$(notes '.toasts | length')" = 3 ] || fail "the two waiting are not in: $(notes .toasts)"
move 200 500
until_says .toasts '[]'
