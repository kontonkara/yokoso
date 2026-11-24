#!/bin/sh
# Everything: format, lints, unit tests, then each stand script, a stand of
# its own each, one after another. Only the scripts named, where names are
# given.
#
#   nix develop -c tests/all.sh [script ...]
set -u
cd "$(dirname "$0")/.."
out=${OUT:-${XDG_RUNTIME_DIR:-/tmp}/yokoso-tests}
rm -rf "$out"
failed=
cargo fmt --all --check || failed="$failed format"
cargo clippy -q --workspace --all-targets -- -D warnings || failed="$failed lints"
cargo test -q --workspace 2>&1 | grep -E 'test result|panicked|FAILED' || true
cargo test -q --workspace >/dev/null 2>&1 || failed="$failed unit"
cargo build -q --release || exit 1
for script in tests/stand/*.sh; do
	[ -e "$script" ] || continue
	name=${script##*/}
	name=${name%.sh}
	[ "$#" -eq 0 ] || echo " $* " | grep -q " $name " || continue
	start=$(date +%s)
	if tests/stand.sh target/release/yokoso "$out/$name" "$script" >"$out-$name.log" 2>&1; then
		echo "passed: $name ($(($(date +%s) - start)) s)"
	else
		echo "FAILED: $name, see $out-$name.log"
		tail -n 3 "$out-$name.log" | sed 's/^/  /'
		failed="$failed $name"
	fi
done
[ -z "$failed" ] && echo "all passed" || { echo "failed:$failed"; exit 1; }
