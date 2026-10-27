import QtQuick
import QtQuick.VirtualKeyboard
import yokoso

// The on-screen keyboard (config.keyboard), for a screen without one: at
// the bottom of the form's screen, no wider than a keyboard would be, up
// while the field takes typing, the form above it. The launcher turns its
// input method on.
InputPanel {
	id: panel

	readonly property rect area: Greeter.core.screenArea

	x: area.x + (area.width - width) / 2
	y: active ? area.y + area.height - height : area.y + area.height
	width: Math.min(area.width, Greeter.u(800))

	// The form makes room.
	Binding {
		target: Greeter.core
		property: "keyboardHeight"
		value: panel.active ? panel.height : 0
	}

	Behavior on y {
		enabled: Greeter.animate

		NumberAnimation {
			duration: 200
			easing.type: Easing.OutCubic
		}
	}
}
