import QtQuick

// Dark while the screen is idle (the core dims the backlight), with the
// pointer hidden; a click on it only wakes the screen. Fill the look with
// it, over everything else.
Rectangle {
	id: cover

	visible: opacity > 0
	color: "black"
	opacity: 0
	states: State {
		name: "asleep"
		when: Greeter.core.idle

		PropertyChanges {
			cover.opacity: 1
		}
	}
	transitions: [
		Transition {
			to: "asleep"
			enabled: Greeter.animate

			NumberAnimation {
				property: "opacity"
				duration: 1000
			}
		},
		Transition {
			from: "asleep"
			enabled: Greeter.animate

			NumberAnimation {
				property: "opacity"
				duration: 150
			}
		}
	]

	MouseArea {
		anchors.fill: parent
		enabled: Greeter.core.idle
		hoverEnabled: true
		cursorShape: Qt.BlankCursor
		onPressed: Greeter.core.wake()
	}
}
