#!/bin/sh
# Renders the working tree's greeter headless (`yokoso shot`, against
# fakegreet) in a set of scenarios, with the users and sessions in
# tests/fixtures, and saves <out>/<scenario>.png and .log. Run in the repo:
#
#   nix run .#shots -- <out> [scenario ...]
#   nix run .#shots -- <out> --against <earlier out> [scenario ...]
#
# --against counts the pixels that differ from an earlier run (the clock's
# minute and the caret's blink account for a few thousand). It exits 1 when
# QML complained in any of them.
set -eu

scenarios="base sessions light wrong one screens cursor idle russian layout look ended other nolist typed locks safe software"

[ -f bin/yokoso ] && [ -d shell ] || {
	echo "shots: run it in yokoso's repository" >&2
	exit 2
}
repo=$PWD
fixtures=$repo/tests/fixtures
out=${1:?usage: shots <out> [--against <earlier out>] [scenario ...]}
shift
against=
if [ "${1:-}" = --against ]; then
	against=${2:?--against needs a directory}
	shift 2
fi
# shellcheck disable=SC2086 # the names, split
[ $# -gt 0 ] || set -- $scenarios
mkdir -p "$out"

work=$(mktemp -d)
trap 'rm -rf "$work"' EXIT

# No keyboard LEDs but a scenario's (not this machine's Num Lock), two
# backgrounds, a picture for bob, the configs (avatars/ sits next to
# them) and the last login: bob with Niri.
magick -size 1280x720 gradient:'#1b2a18'-'#506850' "$work/bg1.png"
magick -size 1280x720 gradient:'#1e3048'-'#6a3e1e' "$work/bg2.png"
mkdir -p "$work/conf/avatars" "$work/leds"
magick -size 400x400 plasma:'#3a7bd5'-'#f0a050' -seed 7 \
	\( -size 400x400 xc:none -fill '#ffffffcc' -draw 'circle 200,160 200,95' \
	-draw 'ellipse 200,400 140,120 180,360' \) -composite "png:$work/conf/avatars/bob"
conf() {
	echo "$2" >"$work/conf/$1.json"
}
conf plain '{}'
conf light '{"theme": "light"}'
conf screens "{\"background\": {\"HEADLESS-2\": \"$work/bg1.png\", \"*\": \"$work/bg2.png\"}}"
conf cursor '{"output": "cursor"}'
conf idle '{"idle": 2}'
conf nolist '{"userList": false}'
conf layout '{"layout": {"form": "left", "clock": "top", "power": "corner", "status": "top-left"}, "radius": 8, "fieldWidth": 300, "clockFormat": "HH:mm"}'
conf look "{\"theme\": \"$repo/examples/minimal.json\"}"
echo '{"user": "bob", "sessions": {"bob": {"id": "niri.desktop", "type": "wayland"}}}' >"$work/state.json"
state=$work/state.json

# shot <name> <delay ms> <command, run against the compositor after 1.5 s>
# [VAR=value ...]: users, sessions, config, outputs, answers and LANG are
# the scenario's.
shot() {
	name=$1 delay=$2 command=$3
	shift 3
	rt=$work/rt-$name
	mkdir -m 700 "$rt"
	cp "$state" "$rt/state.json"
	env -i HOME="$HOME" PATH="$fixtures/users/several:$PATH" XDG_RUNTIME_DIR="$rt" \
		YOKOSO_SHELL_DIR="$repo/shell" YOKOSO_CONFIG="$work/conf/plain.json" YOKOSO_STATE="$rt/state.json" \
		YOKOSO_SESSION_DIRS="$fixtures/sessions/several/wayland-sessions:$fixtures/sessions/several/xsessions" \
		YOKOSO_SHOT_DELAY="$delay" YOKOSO_SHOT_SIZE=1280x720 WLR_HEADLESS_OUTPUTS=1 LANG=en_US.UTF-8 \
		YOKOSO_LEDS="$work/leds" \
		"$@" "$repo/bin/yokoso" shot "$out/$name.png" >"$out/$name.log" 2>&1 &
	pid=$!
	if [ -n "$command" ]; then
		sleep 1.5
		socket=
		for s in "$rt"/wayland-[0-9]; do
			[ -e "$s" ] && socket=${s##*/}
		done
		WAYLAND_DISPLAY=$socket XDG_RUNTIME_DIR=$rt sh -c "$command" >>"$out/$name.log" 2>&1 || true
	fi
	wait "$pid" || true
}

keys() {
	echo "sleep 0.5; wtype -k Escape; sleep 0.3; $*"
}

complained=0
for name; do
	case $name in
	base) shot base 1500 '' ;;
	sessions) shot sessions 3000 "$(keys 'wtype -k F3; sleep 0.2; wtype -k Down; sleep 0.3')" ;;
	light) shot light 3000 "$(keys 'wtype -k F2; sleep 0.2; wtype -k Down; sleep 0.3')" \
		YOKOSO_CONFIG="$work/conf/light.json" ;;
	wrong) shot wrong 3500 '' YOKOSO_TEST_ANSWERS=nope,5 ;;
	one) shot one 1500 '' PATH="$fixtures/users/one:$PATH" \
		YOKOSO_SESSION_DIRS="$fixtures/sessions/one/wayland-sessions" ;;
	screens) shot screens 1500 '' WLR_HEADLESS_OUTPUTS=2 YOKOSO_CONFIG="$work/conf/screens.json" ;;
	# wlrctl's single moves can get lost: a few small ones after each.
	cursor) shot cursor 3000 'sleep 0.5; wlrctl pointer move -9000 -9000; wlrctl pointer move 300 300;
		wlrctl pointer move 5 5; sleep 0.5; wlrctl pointer move 1500 0; wlrctl pointer move 5 5;
		wlrctl pointer move 5 5; sleep 1' WLR_HEADLESS_OUTPUTS=2 YOKOSO_CONFIG="$work/conf/cursor.json" ;;
	idle) shot idle 3500 '' YOKOSO_CONFIG="$work/conf/idle.json" ;;
	russian) shot russian 3500 '' YOKOSO_TEST_ANSWERS=nope,5 LANG=ru_RU.UTF-8 ;;
	layout) shot layout 1500 '' YOKOSO_CONFIG="$work/conf/layout.json" ;;
	look) shot look 1500 '' YOKOSO_CONFIG="$work/conf/look.json" ;;
	# "Other…" from the list (bob is the 2nd of 3), and a name typed.
	other) shot other 3000 "$(keys 'wtype -k F2; sleep 0.2; wtype -k Down; wtype -k Down; sleep 0.2;
		wtype -k Return; sleep 0.3; wtype dave; sleep 0.3')" ;;
	# No list: a name first, then (carol's typed) the password.
	nolist) shot nolist 1500 '' YOKOSO_CONFIG="$work/conf/nolist.json" ;;
	typed) shot typed 3000 "$(keys 'wtype carol; wtype -k Return; sleep 0.8')" YOKOSO_CONFIG="$work/conf/nolist.json" ;;
	# A look of one's own, then safe mode: the built-in one, dark, taking
	# the keys.
	safe) shot safe 3500 "$(keys 'wtype -M ctrl -M alt -M shift -k d -m shift -m alt -m ctrl; sleep 0.5; wtype abc; sleep 0.3')" \
		YOKOSO_CONFIG="$work/conf/look.json" ;;
	# Drawn in software: still at 0.3 s, as nothing fades in.
	software) shot software 300 '' LIBGL_ALWAYS_SOFTWARE=1 ;;
	# Caps Lock and Num Lock on.
	locks) shot locks 1500 '' YOKOSO_LEDS="$fixtures/leds" ;;
	# The last session, Niri, ended a second ago.
	ended)
		echo "{\"started\": {\"name\": \"Niri\", \"at\": $(($(date +%s) * 1000 - 1000))}}" >"$work/ended.json"
		state=$work/ended.json
		shot ended 1500 '' LANG=ru_RU.UTF-8
		state=$work/state.json
		;;
	*)
		echo "shots: no scenario $name (there are: $scenarios)" >&2
		exit 2
		;;
	esac
	line="$name:"
	[ -s "$out/$name.png" ] || line="$line no picture!"
	if [ -n "$against" ] && [ -f "$against/$name.png" ]; then
		line="$line $(magick compare -metric AE "$against/$name.png" "$out/$name.png" null: 2>&1 | cut -d' ' -f1) pixels differ"
	fi
	echo "$line"
	# Quickshell colors its log; without the colors, QML's warnings (not
	# yokoso's own).
	if sed 's/\x1b\[[0-9;]*m//g' "$out/$name.log" | grep -E 'WARN +(scene|qml)' | grep -v 'qml: yokoso:'; then
		complained=1
	fi
done
exit "$complained"
