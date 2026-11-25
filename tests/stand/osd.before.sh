# PipeWire as in the sound test: pw-dump a file the test writes to, wpctl
# writing down what it is asked; the backlight a pipe the test writes levels
# to, held open here.
mkdir -p "$out/fake"
: >"$out/pw.json"
: >"$out/wpctl.log"
printf '#!/bin/sh\nexec tail -n +1 -f "%s"\n' "$out/pw.json" >"$out/fake/pw-dump"
printf '#!/bin/sh\necho "$*" >>"%s"\n' "$out/wpctl.log" >"$out/fake/wpctl"
chmod +x "$out/fake/pw-dump" "$out/fake/wpctl"
export YOKOSO_PW_DUMP="$out/fake/pw-dump" YOKOSO_WPCTL="$out/fake/wpctl"
mkfifo "$out/backlight"
exec 4<>"$out/backlight"
export YOKOSO_BACKLIGHT="$out/backlight"
