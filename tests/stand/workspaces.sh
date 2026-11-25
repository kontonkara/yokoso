# The workspaces as niri has them: a pill each with a column per column of
# windows, the active one framed. A click goes to a workspace, the wheel
# goes up and down them, and the map follows niri at once.
field() { "$binary" msg dump | jq -c ".bars[0].$1"; }
# Windows of their own, each with a socket of its own beside the bar's.
display="$XDG_RUNTIME_DIR/$WAYLAND_DISPLAY"
open_window() {
	mkdir -p "$out/run-$1"
	XDG_RUNTIME_DIR="$out/run-$1" WAYLAND_DISPLAY="$display" env -u __EGL_VENDOR_LIBRARY_FILENAMES \
		"$binary" probe --window >"$out/window-$1.log" 2>&1 &
	own $!
	for _ in $(seq 50); do
		[ "$(niri msg -j windows | jq length)" -ge "$1" ] && return
		sleep 0.1
	done
	fail "window $1 never opened"
}
columns() { field "workspaces | map(.columns)"; }
sleep 0.3
[ "$(columns)" = "[0]" ] || fail "an empty niri is not one empty workspace: $(columns)"
open_window 1
open_window 2
sleep 0.3
[ "$(columns)" = "[2,0]" ] || fail "two windows are not two columns: $(columns)"
# One window down to the next workspace: niri follows it there.
niri msg action move-window-to-workspace-down >/dev/null
sleep 0.4
[ "$(columns)" = "[1,1,0]" ] || fail "the map did not follow the move: $(columns)"
[ "$(field 'workspaces | map(.active)')" = "[false,true,false]" ] || fail "the second is not the active one"
sleep 0.5
shot map
# The frame sits on the active workspace once it has glided there.
[ "$(field 'frame | map(floor)')" = "$(field 'workspaces[1].rect | map(floor)')" ] ||
	fail "the frame is at $(field frame), not on the active workspace"
# A click on the first goes there.
read -r x y <<EOT
$(field 'workspaces[0].rect | "\(8 + .[0] + .[2] / 2 | floor) \(8 + .[1] + .[3] / 2 | floor)"' | tr -d '"')
EOT
move "$x" "$y"
click
sleep 0.4
[ "$(niri msg -j workspaces | jq '[.[] | select(.is_active)][0].idx')" = 1 ] || fail "the click did not go to the first workspace"
[ "$(field 'workspaces | map(.active)')" = "[true,false,false]" ] || fail "the map did not follow the click"
# The wheel down the workspaces, and up again.
# The wheel (as fingers here: a step of every few pixels) down the
# workspaces, and up again.
active() { niri msg -j workspaces | jq '[.[] | select(.is_active)][0].idx'; }
wlrctl pointer scroll 40 0
sleep 0.4
[ "$(active)" = 2 ] || fail "the wheel down did not go to the next workspace: $(active)"
wlrctl pointer scroll -40 0
sleep 0.4
[ "$(active)" = 1 ] || fail "the wheel up did not come back: $(active)"
