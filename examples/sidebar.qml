import QtQuick
import yokoso

// A look made of yokoso's parts: a panel down the left side of the screen
// with a smaller clock, the user, the session, the field and the power
// buttons, over the theme's background. With sidebar.json, which puts it
// on Catppuccin Mocha.
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

	Rectangle {
		id: panel

		x: greeter.area.x
		y: greeter.area.y
		width: greeter.u(420)
		height: greeter.area.height
		color: Qt.alpha(greeter.colors.field, 0.85)

		Clock {
			anchors.horizontalCenter: parent.horizontalCenter
			y: greeter.u(80)
			size: 72
		}

		Column {
			anchors.centerIn: parent
			width: parent.width - greeter.u(80)
			spacing: greeter.u(12)

			Row {
				spacing: greeter.u(16)

				Avatar {
					width: greeter.u(56)
					form: loginForm
				}

				UserName {
					id: userName

					anchors.verticalCenter: parent.verticalCenter
					size: 20
					form: loginForm
				}
			}

			SessionPill {
				id: sessionPill

				form: loginForm
			}

			PasswordField {
				width: parent.width
				form: loginForm
			}

			FieldNote {
				width: parent.width
				form: loginForm
			}
		}

		PowerBar {
			anchors.horizontalCenter: parent.horizontalCenter
			anchors.bottom: parent.bottom
			anchors.bottomMargin: greeter.u(40)
			size: 40
		}

		Status {
			anchors.horizontalCenter: parent.horizontalCenter
			y: greeter.u(24)
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
