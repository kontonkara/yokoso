# Who records, as PipeWire says it: nothing shows while nothing records; an
# app on a microphone and a running camera show, and go as they stop.
dump() { "$binary" msg dump; }
node() {
	printf '{"id":%s,"type":"PipeWire:Interface:Node","info":{"state":"%s","props":%s}}' "$1" "$2" "$3"
}
sleep 0.3
printf '[%s]\n' "$(node 1 running '{"media.class":"Stream/Input/Audio","application.name":"Mixer","stream.monitor":true}')" >>"$out/pw.json"
sleep 0.3
[ "$(dump | jq -c '.bars[0].privacy')" = null ] || fail "a level meter counts as recording"
printf '[%s,%s]\n' "$(node 2 running '{"media.class":"Stream/Input/Audio","application.name":"Recorder"}')" \
	"$(node 3 running '{"media.class":"Video/Source","device.api":"v4l2","node.description":"Webcam"}')" >>"$out/pw.json"
sleep 0.3
[ "$(dump | jq -c .using)" = '["Microphone(\"Recorder\")","Camera(\"Webcam\")"]' ] || fail "the bar heard $(dump | jq -c .using)"
[ "$(dump | jq -c '.bars[0].privacy')" != null ] || fail "no part for what records"
shot recording
printf '[{"id":2,"info":null},{"id":3,"info":null}]\n' >>"$out/pw.json"
sleep 0.3
[ "$(dump | jq -c '.bars[0].privacy')" = null ] || fail "the part stayed after they stopped"
