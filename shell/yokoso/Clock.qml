import QtQuick

// The time, large and light, with figures of one width so that it doesn't
// shift as the minutes go, a figure that changes rolling over; under it
// the date, small and spaced. The theme's clockFormat and dateFormat, else
// the locale's.
Column {
	id: clock

	// The time's size, in pixels at scale 1.
	property real size: 120

	spacing: Greeter.u(2)

	// The time, a character to an item, so that one that changes rolls
	// up: the old one fading out upwards, the new one in from below.
	Row {
		id: time

		readonly property string text: Greeter.core.config.clockFormat
			? Greeter.core.now.toLocaleTimeString(Greeter.core.locale, Greeter.core.config.clockFormat)
			: Greeter.core.now.toLocaleTimeString(Greeter.core.locale, Locale.ShortFormat)

		anchors.horizontalCenter: parent.horizontalCenter
		spacing: -Greeter.u(clock.size / 40)

		Repeater {
			model: time.text.length

			Item {
				id: glyph

				required property int index
				readonly property string character: time.text.charAt(index)
				property string was: ""
				property real roll: 0

				width: current.implicitWidth
				height: current.implicitHeight
				Component.onCompleted: was = character
				onCharacterChanged: {
					previous.text = was;
					was = character;
					if (Greeter.animate)
						rollAnim.restart();
				}

				Text {
					id: current

					y: glyph.roll * glyph.height * 0.3
					opacity: 1 - glyph.roll
					text: glyph.character
					color: Greeter.colors.text
					font.pixelSize: Greeter.u(clock.size)
					font.weight: Font.Light
					font.features: { "tnum": 1 }
					font.family: Greeter.font
				}

				Text {
					id: previous

					y: (glyph.roll - 1) * glyph.height * 0.3
					opacity: glyph.roll
					visible: glyph.roll > 0
					color: current.color
					font: current.font
				}

				NumberAnimation {
					id: rollAnim

					target: glyph
					property: "roll"
					from: 1
					to: 0
					duration: 500
					easing.type: Easing.OutCubic
				}
			}
		}
	}

	// The theme's dateFormat, else the locale's long date without the
	// year.
	Text {
		anchors.horizontalCenter: parent.horizontalCenter
		text: Greeter.core.now.toLocaleDateString(Greeter.core.locale, Greeter.core.config.dateFormat
			|| Greeter.core.locale.dateFormat(Locale.LongFormat).replace(/[\s,.]*y+[^dM]*/, "")).toUpperCase()
		color: Greeter.colors.subtext
		font.pixelSize: Greeter.u(clock.size / 8)
		font.weight: Font.Medium
		font.letterSpacing: Greeter.u(clock.size / 40)
		font.family: Greeter.font
	}
}
