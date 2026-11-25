# Panels from the bar: the clock's calendar and a tray item's menu. They
# grow from their part, the wheel turns the months, the pointer on another
# part opens its panel in place, Escape goes back a level and then closes,
# a click elsewhere closes, and a chosen row reaches the item.
dump() { "$binary" msg dump; }
popup() { dump | jq -c ".popup${1:+.$1}"; }
centre() { jq -r "\"\(.[0] + .[2] / 2 | floor) \(.[1] + .[3] / 2 | floor)\""; }
on_bar() { dump | jq -c ".bars[0].$1" | jq -r '"\(8 + .[0] + .[2] / 2 | floor) \(8 + .[1] + .[3] / 2 | floor)"'; }
sleep 0.3
# The clock opens its calendar, today's month.
read -r x y <<EOT
$(on_bar clock)
EOT
move "$x" "$y"
click
sleep 0.5
[ "$(popup 'shows.calendar')" = "[$(date +%Y),$(date +%-m)]" ] || fail "the clock opened $(popup)"
shot calendar
# The wheel on it: a month on, two back; its name back to today's.
read -r px py <<EOT
$(popup to | centre)
EOT
read -r bx by <<EOT
$(popup to | jq -r '"\(.[0] + .[2] / 2 | floor) \(.[1] + .[3] - 5 | floor)"')
EOT
calm=$(pixel "$bx" "$by")
move "$px" "$py"
sleep 0.4
# The pointer on it lights nothing: only what a click does something to.
shot calendar-hovered
[ "$(pixel "$bx" "$by")" = "$calm" ] || fail "the calendar lit up under the pointer: $calm, now $(pixel "$bx" "$by")"
wlrctl pointer scroll 40 0
sleep 0.2
want=$(date -d "$(date +%Y-%m-15) +1 month" +%Y,%-m)
[ "$(popup 'shows.calendar')" = "[$want]" ] || fail "the wheel down went to $(popup shows), not [$want]"
wlrctl pointer scroll -40 0
wlrctl pointer scroll -40 0
sleep 0.2
want=$(date -d "$(date +%Y-%m-15) -1 month" +%Y,%-m)
[ "$(popup 'shows.calendar')" = "[$want]" ] || fail "the wheel up went to $(popup shows), not [$want]"
# Escape closes it; its layer goes once it has shrunk.
wtype -k Escape
sleep 0.8
[ "$(popup)" = null ] || fail "Escape left $(popup)"
# A tray item's menu, by the right button.
"$fake_tray" "$out/tray.log" >"$out/fake_tray.log" 2>&1 &
own $!
for _ in $(seq 50); do
	[ "$(dump | jq '.tray | length')" = 1 ] && break
	sleep 0.1
done
read -r tx ty <<EOT
$(on_bar 'tray[0]')
EOT
move "$tx" "$ty"
click right
sleep 0.6
[ "$(popup 'shows.menu')" = '["Open","","Muted","More","Quit"]' ] || fail "the menu came as $(popup shows)"
shot menu
# The pointer on the clock: the calendar in its place; back on the item,
# its menu again.
move "$x" "$y"
sleep 0.4
[ "$(popup 'shows.calendar')" != null ] || fail "the clock's panel did not take the menu's place: $(popup shows)"
move "$tx" "$ty"
sleep 0.6
[ "$(popup 'shows.menu[0]')" = '"Open"' ] || fail "the menu did not come back: $(popup shows)"
# More opens its rows; Escape goes back, then closes.
read -r mx my <<EOT
$(popup 'rows[2]' | centre)
EOT
move "$mx" "$my"
click
sleep 0.6
[ "$(popup 'shows')" = '{"depth":1,"menu":["Deeper"]}' ] || fail "More opened $(popup shows)"
wtype -k Escape
sleep 0.6
[ "$(popup 'shows.depth')" = 0 ] || fail "Escape did not go back: $(popup shows)"
# A row chosen reaches the item, and the menu closes.
read -r ox oy <<EOT
$(popup 'rows[0]' | centre)
EOT
move "$ox" "$oy"
click
sleep 0.8
grep -qx 'event 1 clicked' "$out/tray.log" || fail "the item heard: $(cat "$out/tray.log")"
[ "$(popup)" = null ] || fail "the menu stayed after a choice"
# Open again, and a click elsewhere closes it.
move "$x" "$y"
click
sleep 0.5
move 700 700
click
sleep 0.8
[ "$(popup)" = null ] || fail "a click elsewhere left $(popup)"
# Thirty openings of the calendar leave no memory behind, and no frames.
before=$(rss)
for _ in $(seq 30); do
	"$binary" msg dump >/dev/null
	move "$x" "$y"
	click
	sleep 0.15
	click
	sleep 0.15
done
sleep 1.5
after=$(rss)
echo "memory: $before KiB, then $after KiB after 30 calendars"
[ $((after - before)) -lt 4096 ] || fail "memory grew by $((after - before)) KiB"
[ "$(popup)" = null ] || fail "a calendar was left open: $(popup)"
