#!/bin/sh
# The README's pictures, docs/*.webp: the working tree's greeter rendered
# headless (`yokoso shot`, against fakegreet) at 1600x900, with the fake
# users and sessions of tests/fixtures, generated wallpapers, and the
# hostname "laptop". Run in the repo:
#
#   nix run .#docs-shots
set -eu

[ -f bin/yokoso ] && [ -d shell ] || {
	echo "docs-shots: run it in yokoso's repository" >&2
	exit 2
}
repo=$PWD
fixtures=$repo/tests/fixtures
work=$(mktemp -d)
trap 'rm -rf "$work"' EXIT

# Two wallpapers of soft light, a picture for alice, the configs (avatars/
# sits next to them) and the last login: alice with Niri.
magick -size 1600x900 xc:'#141020' \
	\( -size 1600x900 radial-gradient:'#6a4c93'-none -geometry -400-250 \) -composite \
	\( -size 1400x1400 radial-gradient:'#e07a5f'-none -geometry +700+250 \) -composite \
	\( -size 1200x1200 radial-gradient:'#3d5a80'-none -geometry -200+300 \) -composite \
	-blur 0x40 "$work/wall-dark.png"
magick -size 1600x900 xc:'#eef0f6' \
	\( -size 1500x1500 radial-gradient:'#f9c6a0'-none -geometry +600-500 \) -composite \
	\( -size 1400x1400 radial-gradient:'#a9c5f0'-none -geometry -500+200 \) -composite \
	-blur 0x40 "$work/wall-light.png"
mkdir -p "$work/conf/avatars" "$work/leds"
magick -seed 7 -size 400x400 plasma:'#3a7bd5'-'#f0a050' \
	\( -size 400x400 xc:none -fill '#ffffffcc' -draw 'circle 200,160 200,95' \
	-draw 'ellipse 200,400 140,120 180,360' \) -composite "png:$work/conf/avatars/alice"
conf() {
	echo "$2" >"$work/conf/$1.json"
}
conf dark "{\"scale\": 1.25, \"background\": \"$work/wall-dark.png\"}"
conf light "{\"scale\": 1.25, \"theme\": \"light\", \"background\": \"$work/wall-light.png\"}"
conf sidebar "{\"scale\": 1.25, \"theme\": \"$repo/examples/sidebar.json\", \"background\": \"$work/wall-dark.png\"}"
conf layout "{\"scale\": 1.25, \"theme\": \"$repo/examples/catppuccin-mocha.json\", \"background\": \"$work/wall-dark.png\",
	\"layout\": {\"form\": \"left\", \"clock\": \"top\", \"power\": \"corner\", \"status\": \"top-left\"}, \"radius\": 12}"
echo '{"user": "alice", "sessions": {"alice": {"id": "niri.desktop", "type": "wayland"}}}' >"$work/state.json"

# shot <name> <config> <keys, typed after 1.5 s>
shot() {
	name=$1 config=$2 keys=$3
	rt=$work/rt-$name
	mkdir -m 700 "$rt"
	cp "$work/state.json" "$rt/state.json"
	unshare -r -u sh -c 'hostname laptop; exec "$@"' sh env -i HOME="$HOME" \
		PATH="$fixtures/users/several:$PATH" XDG_RUNTIME_DIR="$rt" \
		YOKOSO_SHELL_DIR="$repo/shell" YOKOSO_CONFIG="$work/conf/$config.json" YOKOSO_STATE="$rt/state.json" \
		YOKOSO_SESSION_DIRS="$fixtures/sessions/several/wayland-sessions:$fixtures/sessions/several/xsessions" \
		YOKOSO_SHOT_DELAY=3000 YOKOSO_SHOT_SIZE=1600x900 WLR_HEADLESS_OUTPUTS=1 LANG=en_US.UTF-8 \
		YOKOSO_LEDS="$work/leds" "$repo/bin/yokoso" shot "$work/$name.png" >"$work/$name.log" 2>&1 &
	pid=$!
	if [ -n "$keys" ]; then
		sleep 1.5
		for s in "$rt"/wayland-[0-9]; do
			[ -e "$s" ] && socket=${s##*/}
		done
		# The first key can be lost.
		WAYLAND_DISPLAY=$socket XDG_RUNTIME_DIR=$rt sh -c "sleep 0.5; wtype -k Escape; sleep 0.3; $keys" || true
	fi
	wait "$pid" || true
	if [ -s "$work/$name.png" ]; then
		magick "$work/$name.png" -quality 90 "$repo/docs/$name.webp"
		echo "docs/$name.webp"
	else
		cat "$work/$name.log" >&2
		echo "docs-shots: no picture for $name" >&2
	fi
}

shot dark dark ''
shot light light 'wtype -k F2; sleep 0.3'
shot sidebar sidebar ''
shot layout layout 'wtype secret; sleep 0.2'
