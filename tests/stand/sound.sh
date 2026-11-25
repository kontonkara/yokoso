# The sound as PipeWire says it, and the wheel and the middle button asking
# WirePlumber for changes; what PipeWire says next shows at once.
dump() { "$binary" msg dump; }
batch() {
	printf '[{"id":40,"type":"PipeWire:Interface:Metadata","props":{"metadata.name":"default"},"metadata":[{"subject":0,"key":"default.audio.sink","type":"Spa:String:JSON","value":{"name":"speakers"}}]},{"id":65,"type":"PipeWire:Interface:Node","info":{"props":{"media.class":"Audio/Sink","node.name":"speakers","node.description":"Speakers"},"params":{"Props":[{"channelVolumes":[%s,%s],"mute":%s}]}}}]\n' "$1" "$1" "$2" >>"$out/pw.json"
}
sleep 0.3
[ "$(dump | jq -r .sound)" = Unknown ] || fail "nothing said, and still a sound: $(dump | jq -c .sound)"
batch 0.125 false
sleep 0.3
[ "$(dump | jq -c '[.sound.volume, .sound.muted]')" = "[0.5,false]" ] || fail "the bar heard $(dump | jq -c .sound)"
shot sound
read -r x y <<EOT
$(dump | jq -r '.bars[0].sound | "\(8 + .[0] + .[2] / 2 | floor) \(8 + .[1] + .[3] / 2 | floor)"')
EOT
move "$x" "$y"
wlrctl pointer scroll -40 0
sleep 0.3
grep -qx 'set-volume -l 1.00 @DEFAULT_AUDIO_SINK@ 5%+' "$out/wpctl.log" || fail "the wheel up asked: $(cat "$out/wpctl.log")"
click middle
grep -qx 'set-mute @DEFAULT_AUDIO_SINK@ toggle' "$out/wpctl.log" || fail "the middle button asked: $(cat "$out/wpctl.log")"
# What PipeWire says next: louder than full, then muted.
batch 1.728 false
sleep 0.3
[ "$(dump | jq -c .sound.volume)" = 1.2 ] || fail "the bar heard $(dump | jq -c .sound)"
shot loud
batch 1.728 true
sleep 0.3
[ "$(dump | jq -c .sound.muted)" = true ] || fail "the bar did not hear it muted"
