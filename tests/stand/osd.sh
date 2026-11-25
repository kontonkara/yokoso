# The OSD: nothing as the sound is first heard; a change of volume, of the
# output, of the brightness shows at the bottom middle, and goes a moment
# after the last change, its surface with it; what the config hides doesn't.
dump() { "$binary" msg dump; }
osd() { dump | jq -c '.osd'; }
batch() {
	printf '[{"id":40,"type":"PipeWire:Interface:Metadata","props":{"metadata.name":"default"},"metadata":[{"subject":0,"key":"default.audio.sink","type":"Spa:String:JSON","value":{"name":"%s"}}]},{"id":65,"type":"PipeWire:Interface:Node","info":{"props":{"media.class":"Audio/Sink","node.name":"speakers","node.description":"Speakers"},"params":{"Props":[{"channelVolumes":[%s,%s],"mute":false}]}}},{"id":70,"type":"PipeWire:Interface:Node","info":{"props":{"media.class":"Audio/Sink","node.name":"phones","node.description":"Headphones"},"params":{"Props":[{"channelVolumes":[1,1],"mute":false}]}}}]\n' "$1" "$2" "$2" >>"$out/pw.json"
}
until_says() {
	for _ in $(seq 40); do
		[ "$(osd)" = "$1" ] && return 0
		sleep 0.1
	done
	fail "the OSD is $(osd), not $1"
}
sleep 0.3
batch speakers 0.125
echo 0.5 >&4
sleep 0.5
[ "$(osd)" = null ] || fail "the OSD showed as things were first heard: $(osd)"
# Louder: the volume, at the bottom middle.
batch speakers 0.216
until_says '{"leaving":false,"shows":"Volume { level: 0.6, muted: false }"}'
sleep 0.3
shot volume
[ "$(dump | jq '[.surfaces[] | select(.key | test("Osd"))] | length')" = 1 ] || fail "no surface for the OSD"
[ "$(pixel 700 820)" != "$(pixel 100 820)" ] || fail "nothing at the bottom middle"
# Gone a moment after the last change, its surface with it.
until_says null
# The output moves to the headphones; the brightness changes.
batch phones 0.216
until_says '{"leaving":false,"shows":"Output { name: \"Headphones\" }"}'
echo 0.8 >&4
until_says '{"leaving":false,"shows":"Brightness { level: 0.8 }"}'
until_says null
# Hidden by the config, the volume shows no more.
printf '[osd]\nhidden = ["volume"]\n' >"$XDG_CONFIG_HOME/yokoso/config.toml"
sleep 0.5
batch phones 0.125
sleep 0.5
[ "$(osd)" = null ] || fail "a hidden OSD showed: $(osd)"
