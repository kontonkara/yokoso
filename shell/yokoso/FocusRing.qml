import QtQuick

// The ring around what Tab took the keys to (Form.focused), a little out
// from its edges: fill the thing with it, and give it the thing's corner
// radius.
Rectangle {
	property bool shown: false
	property real cornerRadius: 0
	// How far out from its edges.
	property real gap: Greeter.u(4)

	anchors.margins: -gap - border.width
	radius: cornerRadius + gap + border.width
	visible: shown
	color: "transparent"
	border.width: Math.max(1, Greeter.u(2))
	border.color: Greeter.colors.accent
}
