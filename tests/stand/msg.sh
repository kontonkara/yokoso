# yokoso msg answers; an idle shell draws nothing and hardly wakes.
[ "$("$binary" msg ping)" = pong ] || fail "no pong"
"$binary" msg nonsense 2>/dev/null && fail "a made-up command passed"
[ "$("$binary" msg dump | jq '.surfaces | length')" = 1 ] || fail "not one surface"

frames() { "$binary" msg stats | jq .frames; }
wakeups() { "$binary" msg stats | jq .wakeups; }
before_frames=$(frames)
before_wakeups=$(wakeups)
sleep 3
after_frames=$(frames)
after_wakeups=$(wakeups)
[ "$after_frames" = "$before_frames" ] || fail "idle drew $((after_frames - before_frames)) frames"
# Only the asking itself wakes it: a few for each `msg`.
woke=$((after_wakeups - before_wakeups))
[ "$woke" -le 8 ] || fail "idle woke $woke times in 3 s"
echo "idle: 0 frames, $woke wakeups in 3 s (the asking included)"
