# The demo opens as a window, its elements laid out and named.
nodes=$("$binary" msg dump | jq '.nodes | length')
[ "$nodes" -ge 3 ] || fail "only $nodes elements"
"$binary" msg dump | jq -e '.nodes[1].role == "Text"' >/dev/null || fail "the title is not text"
shot demo
shades=$(magick "$last_shot" -crop 300x40+30+30 -format %k info:)
[ "$shades" -gt 10 ] || fail "no title drawn: $shades shades"
