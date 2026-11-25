# PipeWire as the stand has it: what pw-dump says is a file the test writes
# to, and wpctl only writes down what it was asked. The real one is never
# touched.
mkdir -p "$out/fake"
: >"$out/pw.json"
: >"$out/wpctl.log"
printf '#!/bin/sh\nexec tail -n +1 -f "%s"\n' "$out/pw.json" >"$out/fake/pw-dump"
printf '#!/bin/sh\necho "$*" >>"%s"\n' "$out/wpctl.log" >"$out/fake/wpctl"
chmod +x "$out/fake/pw-dump" "$out/fake/wpctl"
export YOKOSO_PW_DUMP="$out/fake/pw-dump" YOKOSO_WPCTL="$out/fake/wpctl"
