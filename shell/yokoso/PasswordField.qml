import QtQuick
import QtQuick.Shapes

// The field: it takes PAM's answers (a password as dots, the last ones in
// view) or, while `form` asks for one, a name. Its outline is in the
// accent while it waits for typing and in the error color under an error;
// a wrong password shakes it. PAM's question or last message is its
// placeholder. The button at its end is an arrow that turns into a
// spinner while the password is checked.
Item {
	id: root

	required property Form form
	// The field's height, in pixels at scale 1.
	property real size: 50

	implicitHeight: Greeter.u(size)

	// A shake, dying down as it goes from 0 to 1.
	property real shaken: 1

	NumberAnimation {
		id: shake

		target: root
		property: "shaken"
		from: 0
		to: 1
		duration: 450
	}

	Connections {
		target: Greeter.core.auth

		function onPhaseChanged() {
			if (Greeter.core.auth.phase === "failed" && Greeter.animate)
				shake.restart();
		}
	}

	Rectangle {
		id: field

		// The placeholder shows an error.
		readonly property bool error: input.text === "" && !root.form.enteringName
			&& (Greeter.core.auth.messageIsError || Greeter.core.problem !== "")
		readonly property bool busy: Greeter.core.auth.phase === "verifying" || Greeter.core.auth.phase === "launching"

		width: parent.width
		height: parent.height
		radius: Greeter.round(height)
		color: Greeter.colors.field
		border.width: Greeter.u(2)
		border.color: error ? Greeter.colors.error
			: root.form.typing && input.activeFocus ? Qt.alpha(Greeter.colors.accent, input.text ? 0.9 : 0.4)
			: Qt.alpha(Greeter.colors.text, 0.08)
		transform: Translate {
			x: Math.sin(root.shaken * Math.PI * 6) * (1 - root.shaken) * Greeter.u(10)
		}

		Behavior on border.color {
			enabled: Greeter.animate

			ColorAnimation {
				duration: 150
			}
		}

		TextInput {
			id: input

			anchors.left: parent.left
			anchors.right: button.left
			anchors.leftMargin: Greeter.u(22)
			anchors.rightMargin: Greeter.u(10)
			anchors.verticalCenter: parent.verticalCenter
			opacity: root.form.typing ? 1 : 0.5
			focus: true
			readOnly: !root.form.typing
			echoMode: root.form.secret ? TextInput.Password : TextInput.Normal
			// A secret is drawn by `dots` below: its own echo, cursor (in
			// this color too) and selection stay hidden, and the cursor
			// stays at the end.
			color: root.form.secret ? "transparent" : Greeter.colors.text
			selectionColor: root.form.secret ? "transparent" : Qt.alpha(Greeter.colors.accent, 0.3)
			font.pixelSize: Greeter.u(16)
			font.family: Greeter.font
			clip: true
			Component.onCompleted: root.form.input = input
			onAccepted: root.form.submit()
			onCursorPositionChanged: if (root.form.secret && cursorPosition !== text.length)
				cursorPosition = text.length
			onTextChanged: {
				const count = root.form.secret ? text.length : 0;
				while (dotModel.count > count)
					dotModel.remove(dotModel.count - 1);
				while (dotModel.count < count)
					dotModel.append({});
			}
			Keys.onPressed: event => root.form.keyPressed(event)

			// One per character, kept as the text grows, so that only a
			// new one pops up.
			ListModel {
				id: dotModel
			}

			// The blink of the cursor after the dots.
			Timer {
				id: blink

				property bool on: true

				interval: 530
				running: input.activeFocus && root.form.secret && root.form.typing && !Greeter.core.idle
				repeat: true
				onRunningChanged: on = true
				onTriggered: on = !on
			}
		}

		// The secret: a dot for each character, the last ones in view, and
		// the cursor after them.
		Item {
			anchors.fill: input
			visible: root.form.secret
			clip: true

			Row {
				id: dots

				x: Math.min(0, parent.width - width - Greeter.u(6))
				anchors.verticalCenter: parent.verticalCenter
				spacing: Greeter.u(7)

				Repeater {
					model: dotModel

					Rectangle {
						width: Greeter.u(8)
						height: width
						radius: width / 2
						color: Greeter.colors.text

						NumberAnimation on scale {
							running: Greeter.animate
							from: 0.2
							to: 1
							duration: 180
							easing.type: Easing.OutBack
						}
					}
				}
			}

			Rectangle {
				x: dots.x + dots.width + (dotModel.count ? Greeter.u(6) : 0)
				anchors.verticalCenter: parent.verticalCenter
				width: Math.max(1, Greeter.u(1.5))
				height: Greeter.u(20)
				visible: input.activeFocus && root.form.typing && blink.on
				color: Greeter.colors.text
			}
		}

		// PAM's question ("Username" while a name is typed), or its last
		// message (a failed attempt included) until something is typed; a
		// message too long for the field goes to FieldNote.
		Text {
			id: placeholder

			readonly property bool messageFits: messageWidth.advanceWidth <= input.width
			// PAM's last message, unless the field takes a name after
			// "Other…".
			readonly property bool showsMessage: !Greeter.core.problem && !root.form.enteringName && messageFits
				&& Greeter.core.auth.message !== ""

			anchors.fill: input
			verticalAlignment: Text.AlignVCenter
			visible: input.text === ""
			text: Greeter.core.problem || (showsMessage ? Greeter.core.auth.message
				: root.form.askingName ? Greeter.core.tr("Username") : Greeter.core.auth.prompt.replace(/:\s*$/, ""))
			color: Greeter.core.problem || showsMessage && Greeter.core.auth.messageIsError ? Greeter.colors.error : Greeter.colors.dim
			font.pixelSize: Greeter.u(16)
			font.family: Greeter.font
			elide: Text.ElideRight

			Binding {
				target: root.form
				property: "messageFits"
				value: placeholder.messageFits
			}

			TextMetrics {
				id: messageWidth

				font: placeholder.font
				text: Greeter.core.auth.message
			}
		}

		Rectangle {
			id: button

			readonly property bool ready: input.text !== "" && root.form.typing

			anchors.right: parent.right
			anchors.rightMargin: Greeter.u(7)
			anchors.verticalCenter: parent.verticalCenter
			width: parent.height - Greeter.u(14)
			height: width
			radius: Greeter.round(width, Greeter.u(7))
			color: ready ? Greeter.colors.accent : Qt.alpha(Greeter.colors.text, 0.06)
			// It grows under the pointer and gives under a press, once
			// there's something to send.
			scale: !ready ? 1 : buttonMouse.pressed ? 0.9 : buttonMouse.containsMouse ? 1.08 : 1

			Behavior on color {
				enabled: Greeter.animate

				ColorAnimation {
					duration: 150
				}
			}

			Behavior on scale {
				enabled: Greeter.animate

				NumberAnimation {
					duration: 150
					easing.type: Easing.OutCubic
				}
			}

			// The arrow: it comes up as something is typed, leans forward
			// under the pointer, and gives way to the spinner.
			Icon {
				anchors.centerIn: parent
				path: Greeter.icons.arrow
				size: Greeter.u(18)
				color: button.ready ? Greeter.colors.background : Greeter.colors.dim
				opacity: field.busy ? 0 : 1
				scale: button.ready ? 1 : 0.8
				transform: Translate {
					x: button.ready && buttonMouse.containsMouse ? Greeter.u(2) : 0

					Behavior on x {
						enabled: Greeter.animate

						NumberAnimation {
							duration: 150
							easing.type: Easing.OutCubic
						}
					}
				}

				Behavior on color {
					enabled: Greeter.animate

					ColorAnimation {
						duration: 150
					}
				}

				Behavior on opacity {
					enabled: Greeter.animate

					NumberAnimation {
						duration: 150
					}
				}

				Behavior on scale {
					enabled: Greeter.animate

					NumberAnimation {
						duration: 200
						easing.type: Easing.OutBack
					}
				}
			}

			// Three quarters of a ring, turning.
			Shape {
				id: spinner

				anchors.centerIn: parent
				width: Greeter.u(18)
				height: width
				visible: opacity > 0
				opacity: field.busy ? 1 : 0
				preferredRendererType: Shape.CurveRenderer

				Behavior on opacity {
					enabled: Greeter.animate

					NumberAnimation {
						duration: 150
					}
				}

				ShapePath {
					strokeColor: Greeter.colors.accent
					strokeWidth: Math.max(1.5, Greeter.u(18) / 10)
					fillColor: "transparent"
					capStyle: ShapePath.RoundCap

					PathAngleArc {
						centerX: Greeter.u(9)
						centerY: Greeter.u(9)
						radiusX: Greeter.u(8)
						radiusY: Greeter.u(8)
						sweepAngle: 270
					}
				}

				RotationAnimator on rotation {
					running: spinner.visible && Greeter.animate
					from: 0
					to: 360
					duration: 800
					loops: Animation.Infinite
				}
			}

			MouseArea {
				id: buttonMouse

				anchors.fill: parent
				hoverEnabled: true
				cursorShape: button.ready ? Qt.PointingHandCursor : Qt.ArrowCursor
				onClicked: if (button.ready)
					root.form.submit()
			}
		}
	}
}
