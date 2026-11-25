# A list of ten thousand rows: it scrolls, a click chooses a row, the keys
# choose the next and open it, and it builds only the rows in view.
"$binary" msg page lists
sleep 0.3
field() { "$binary" msg dump | jq -c ".$1"; }
read -r x y w h <<EOT
$("$binary" msg dump | jq -r '[.nodes[] | select(.role == "List")][0].rect | "\(16 + .[0] | floor) \(16 + .[1] | floor) \(.[2] | floor) \(.[3] | floor)"')
EOT
nodes=$(field 'nodes | length')
[ "$nodes" -lt 200 ] || fail "$nodes elements for a list of ten thousand"
move $((x + w / 2)) $((y + 30))
wlrctl pointer scroll 480 0
sleep 0.3
scroll=$(field scroll)
[ "${scroll%.*}" -ge 400 ] || fail "the list did not scroll: $scroll"
shot scrolled
click
selected=$(field selected)
[ "$selected" != null ] || fail "a click chose no row"
wtype -k Down
sleep 0.2
[ "$(field selected)" = $((selected + 1)) ] || fail "Down did not choose the next row: $(field selected)"
wtype -k Return
sleep 0.2
[ "$(field opened)" = $((selected + 1)) ] || fail "Return did not open the row"
wtype -k End
sleep 0.5
[ "$(field selected)" = 9999 ] || fail "End did not choose the last row"
[ "$(field scroll | cut -d. -f1)" -gt 400000 ] || fail "the last row is not in view: $(field scroll)"
shot end
# The chosen row, scrolled just out of view, draws nothing over what is
# above the list.
wtype -k Home
sleep 0.5
top=$("$binary" msg dump | jq -r '[.nodes[] | select(.role == "List")][0].rect[1] | floor')
shot header_before
move $((x + w / 2)) $((y + 60))
wlrctl pointer scroll 100 0
sleep 0.5
shot header_after
differ=$(magick compare -metric AE "$out/header_before.png[680x$((top + 10))+0+0]" "$out/header_after.png[680x$((top + 10))+0+0]" null: 2>&1 || true)
[ "${differ%% *}" = 0 ] || fail "the chosen row showed outside the list: $differ pixels changed above it"
