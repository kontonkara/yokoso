# The keyboard layout as niri has it: hidden with one, its short name with
# more; a click goes to the next, the wheel back, and niri's own switch
# shows at once.
dump() { "$binary" msg dump; }
current() { niri msg -j keyboard-layouts | jq .current_idx; }
sleep 0.3
[ "$(dump | jq -c '.bars[0].layout')" = null ] || fail "one layout, and still a part for it"
# niri reads its config again as it changes.
printf 'input {\n\tkeyboard {\n\t\txkb {\n\t\t\tlayout "us,ru"\n\t\t}\n\t}\n}\n' >>"$work/niri.kdl"
for _ in $(seq 30); do
	[ "$(dump | jq -r .layout)" = "English (US)" ] && break
	sleep 0.1
done
[ "$(dump | jq -r .layout)" = "English (US)" ] || fail "the layouts never came: $(dump | jq -c .layouts)"
sleep 0.3
shot layouts
read -r x y <<EOT
$(dump | jq -r '.bars[0].layout | "\(8 + .[0] + .[2] / 2 | floor) \(8 + .[1] + .[3] / 2 | floor)"')
EOT
move "$x" "$y"
click
sleep 0.3
[ "$(current)" = 1 ] || fail "a click did not go to the next layout"
[ "$(dump | jq -r .layout)" = Russian ] || fail "the bar did not follow: $(dump | jq -r .layout)"
wlrctl pointer scroll -40 0
sleep 0.3
[ "$(current)" = 0 ] || fail "the wheel up did not go back"
niri msg action switch-layout next >/dev/null
sleep 0.3
[ "$(dump | jq -r .layout)" = Russian ] || fail "niri's own switch did not show"
