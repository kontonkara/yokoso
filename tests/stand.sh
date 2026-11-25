#!/bin/sh
# Runs yokoso in a niri of its own, inside a headless sway on the integrated
# GPU, then <script> with these at hand:
#
#   shot NAME       a picture of the 1400x900 screen, <out>/NAME.png
#   pixel X Y       the color there in the last shot, as #rrggbb
#   move X Y        the pointer to X Y, in niri's logical pixels
#   click [BUTTON]  left, right or middle
#   rss             the shell's resident memory, KiB
#   log             the shell's log so far
#   own PID         a process the script started, stopped with the stand
#
# A <script>.before.sh beside it runs before the shell starts: it may set
# shell_args (the words yokoso runs with, "shell" if unset), or on_sway=1
# to run it on the outer sway instead of niri.
#   fail WHY        stops with an error
#
# Nothing here outlives it: every process it starts is stopped by its PID.
#
#   nix develop -c tests/stand.sh <binary> <out> <script>
set -eu
[ "$#" -eq 3 ] || {
	echo "usage: stand.sh <binary> <out> <script>" >&2
	exit 1
}
binary=$(readlink -f "$1")
out=$(mkdir -p "$2" && readlink -f "$2")
script=$(readlink -f "$3")

real_run=${XDG_RUNTIME_DIR:-/run/user/$(id -u)}
work=$(mktemp -d "$real_run/yokoso-stand.XXXXXX")
export XDG_RUNTIME_DIR=$work/run XDG_CONFIG_HOME=$work/config XDG_STATE_HOME=$work/state XDG_CACHE_HOME=$work/cache
mkdir -m 700 -p "$XDG_RUNTIME_DIR" "$XDG_CONFIG_HOME/yokoso"
# The integrated GPU only, through Mesa: the discrete one stays asleep.
export WLR_BACKENDS=headless WLR_LIBINPUT_NO_DEVICES=1 WLR_RENDERER=gles2 WLR_HEADLESS_OUTPUTS=1
export WLR_RENDER_DRM_DEVICE=/dev/dri/renderD128
export __EGL_VENDOR_LIBRARY_FILENAMES=/run/opengl-driver/share/glvnd/egl_vendor.d/50_mesa.json

stop() {
	trap - EXIT INT TERM
	for pid in ${others:-} ${shell_pid:-} ${niri_pid:-} ${sway_pid:-}; do
		kill "$pid" 2>/dev/null || true
	done
	sleep 0.3
	for pid in ${others:-} ${shell_pid:-} ${niri_pid:-} ${sway_pid:-}; do
		kill -9 "$pid" 2>/dev/null || true
		wait "$pid" 2>/dev/null || true
	done
	rm -rf "$work"
}
trap stop EXIT
trap 'exit 1' INT TERM

printf 'xwayland disable\noutput * resolution 1400x900\ndefault_border none\n' >"$work/sway.conf"
sway --unsupported-gpu -c "$work/sway.conf" >"$work/sway.log" 2>&1 &
sway_pid=$!
socket() {
	(cd "$XDG_RUNTIME_DIR" && ls wayland-* 2>/dev/null | grep -v lock | sort | sed -n "$1p") || true
}
for _ in $(seq 100); do
	outer=$(socket 1)
	[ -n "$outer" ] && break
	sleep 0.05
done
[ -n "$outer" ] || { cat "$work/sway.log" >&2; exit 1; }
for _ in $(seq 50); do
	SWAYSOCK=$(ls "$XDG_RUNTIME_DIR"/sway-ipc.*.sock 2>/dev/null | head -n 1 || true)
	[ -n "$SWAYSOCK" ] && break
	sleep 0.05
done
export SWAYSOCK

printf 'hotkey-overlay {\n\tskip-at-startup\n}\ngestures {\n\thot-corners {\n\t\toff\n\t}\n}\nprefer-no-csd\nanimations {\n\toff\n}\n' >"$work/niri.kdl"
WAYLAND_DISPLAY=$outer niri -c "$work/niri.kdl" >"$work/niri.log" 2>&1 &
niri_pid=$!
for _ in $(seq 100); do
	NIRI_SOCKET=$(ls "$XDG_RUNTIME_DIR"/niri.*.sock 2>/dev/null | head -n 1 || true)
	inner=$(socket 2)
	[ -n "$NIRI_SOCKET" ] && [ -n "$inner" ] && break
	sleep 0.1
done
[ -n "${NIRI_SOCKET:-}" ] || { cat "$work/niri.log" >&2; exit 1; }
export NIRI_SOCKET WAYLAND_DISPLAY=$inner
export OUTER_DISPLAY=$outer

# What a script wants before the shell starts, as `shell_args`, or
# `on_sway=1` to run it on the outer sway, whose outputs can come and go:
# <script>.before.sh.
before=${script%.sh}.before.sh
[ ! -f "$before" ] || . "$before"
[ -z "${on_sway:-}" ] || export WAYLAND_DISPLAY=$outer

# yokoso finds Mesa by itself: none of the vendor list above for it.
# shellcheck disable=SC2086 # the words of shell_args are meant to split
env -u __EGL_VENDOR_LIBRARY_FILENAMES YOKOSO_LOG=${YOKOSO_LOG:-info} "$binary" ${shell_args:-shell} >"$out/shell.log" 2>&1 &
shell_pid=$!
for _ in $(seq 100); do
	grep -q 'first frame' "$out/shell.log" && break
	kill -0 "$shell_pid" 2>/dev/null || { cat "$out/shell.log" >&2; exit 1; }
	sleep 0.1
done
grep -q 'first frame' "$out/shell.log" || { cat "$out/shell.log" >&2; exit 1; }
sleep 0.3

last_shot=
shot() {
	WAYLAND_DISPLAY=$outer grim "$out/$1.png"
	last_shot=$out/$1.png
}
pixel() { magick "$last_shot" -crop "1x1+$1+$2" -depth 8 txt:- | grep -o '#[0-9A-F]\{6\}' | head -n 1; }
move() {
	wlrctl pointer move -3000 -3000
	wlrctl pointer move "$1" "$2"
	sleep 0.1
}
click() { wlrctl pointer click "${1:-left}"; sleep 0.2; }
rss() { awk '/^VmRSS/ { print $2 }' "/proc/$shell_pid/status"; }
log() { cat "$out/shell.log"; }
fail() { echo "stand: $*" >&2; exit 1; }
# What a script starts in the background, stopped with the rest: `own $!`.
# A script never sets a trap of its own: that would take the stand's away.
own() { others="${others:-} $1"; }

# The discrete GPU stays out of it: no NVIDIA library loaded, no other render
# node open; as the shell starts, and after the script if it still runs.
kept_off() {
	kill -0 "$shell_pid" 2>/dev/null || return 0
	if grep -q libnvidia "/proc/$shell_pid/maps" || ls -l "/proc/$shell_pid/fd" 2>/dev/null | grep -qE 'nvidia|renderD12[^8]'; then
		fail "the shell touched the discrete GPU"
	fi
}
kept_off

. "$script"

kept_off
[ -n "${may_quit:-}" ] || kill -0 "$shell_pid" 2>/dev/null || fail "the shell died"
grep 'drawing' "$out/shell.log"
