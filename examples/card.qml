import QtQuick
import yokoso

// A look made of yokoso's parts: the clock large in the top left corner,
// and the form on a card in the middle, the picture half out of its top
// edge and the power buttons along its bottom. With card.json, which puts
// it on Nord.
Item {
	id: look

	required property var greeter

	// What the parts that make up the form share.
	Form {
		id: loginForm
	}

	Background {
		anchors.fill: parent
	}

	Clock {
		x: greeter.area.x + greeter.u(64)
		y: greeter.area.y + greeter.u(48)
		size: 96
	}

	Status {
		x: greeter.area.x + greeter.area.width - width - greeter.u(32)
		y: greeter.area.y + greeter.u(28)
	}

	Rectangle {
		id: card

		x: greeter.area.x + (greeter.area.width - width) / 2
		y: greeter.area.y + (greeter.area.height - height) / 2 + greeter.u(32)
		width: greeter.u(400)
		height: column.y + column.height + greeter.u(28)
		radius: Greeter.round(greeter.u(56))
		color: Qt.alpha(greeter.colors.field, 0.8)
		border.width: Math.max(1, greeter.u(1))
		border.color: Greeter.outline(0.08)

		Avatar {
			anchors.horizontalCenter: parent.horizontalCenter
			y: -width / 2
			width: greeter.u(96)
			form: loginForm
		}

		Column {
			id: column

			anchors.horizontalCenter: parent.horizontalCenter
			y: greeter.u(64)
			width: parent.width - greeter.u(56)
			spacing: greeter.u(12)

			UserName {
				id: userName

				anchors.horizontalCenter: parent.horizontalCenter
				size: 22
				form: loginForm
			}

			SessionPill {
				id: sessionPill

				anchors.horizontalCenter: parent.horizontalCenter
				form: loginForm
			}

			PasswordField {
				width: parent.width
				size: 46
				form: loginForm
			}

			FieldNote {
				width: parent.width
				form: loginForm
			}

			PowerBar {
				anchors.horizontalCenter: parent.horizontalCenter
				size: 36
				form: loginForm
			}
		}
	}

	Picker {
		anchors.fill: parent
		form: loginForm
		under: loginForm.picking === "session" ? sessionPill : userName
	}

	IdleCover {
		anchors.fill: parent
	}
}
