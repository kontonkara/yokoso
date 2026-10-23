import QtQuick

// A word that a lock key is on, fading and dropping in while `on`.
Text {
	id: note

	property bool on
	property real shown: on ? 1 : 0

	visible: shown > 0
	font.pixelSize: Greeter.u(13)
	font.weight: Font.Medium
	font.family: Greeter.font
	opacity: shown
	transform: Translate {
		y: (1 - note.shown) * -Greeter.u(4)
	}

	Behavior on shown {
		enabled: Greeter.animate

		NumberAnimation {
			duration: 180
		}
	}
}
