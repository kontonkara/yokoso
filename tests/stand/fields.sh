# Fields: typing, words, the clipboard with other programs, a password's
# dots, Return, and a caret that stops blinking: no frames once idle.
"$binary" msg page fields
sleep 0.3
field() { "$binary" msg dump | jq -r ".$1"; }
spot() { # the middle of the Nth field, on the screen
	"$binary" msg dump | jq -r "[.nodes[] | select(.role == \"TextField\")][$1].rect | \"\\(16 + .[0] + .[2] / 2 | floor) \\(16 + .[1] + .[3] / 2 | floor)\""
}
# shellcheck disable=SC2046 # the words are x and y
move $(spot 0)
click
wtype "Hello world"
sleep 0.2
[ "$(field name)" = "Hello world" ] || fail "typed: $(field name)"
# Ctrl+Backspace takes the last word.
wtype -M ctrl -k BackSpace -m ctrl
sleep 0.2
[ "$(field name)" = "Hello " ] || fail "after ctrl+backspace: $(field name)"
# Copied out to another program, and pasted in from one.
wtype -M ctrl a -m ctrl -M ctrl c -m ctrl
sleep 0.3
[ "$(wl-paste --no-newline)" = "Hello " ] || fail "the clipboard has: $(wl-paste --no-newline)"
printf 'from outside' | wl-copy
sleep 0.3
wtype -k End -M ctrl v -m ctrl
sleep 0.5
[ "$(field name)" = "Hello from outside" ] || fail "after a paste: $(field name)"
wtype -k Return
sleep 0.2
[ "$(field sent)" = "Hello from outside" ] || fail "Return did not send"
shot name
# The password: dots, and nothing to copy.
# shellcheck disable=SC2046
move $(spot 1)
click
wtype "abc"
sleep 0.2
[ "$(field secret_length)" = 3 ] || fail "the password has $(field secret_length) characters"
printf 'kept' | wl-copy
wtype -M ctrl a -m ctrl -M ctrl c -m ctrl
sleep 0.3
[ "$(wl-paste --no-newline)" = kept ] || fail "a password was copied"
shot password
# Left alone, the caret stops blinking, and the frames stop with it.
sleep 11
frames=$("$binary" msg stats | jq .frames)
sleep 1.5
[ "$("$binary" msg stats | jq .frames)" = "$frames" ] || fail "frames go on in an idle field"
