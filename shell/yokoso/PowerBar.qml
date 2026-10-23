import QtQuick
import QtQuick.Shapes

// The power actions logind allows, as round buttons with icons; a press
// arms one (a ring around it runs out as the time for the second press
// does), a second acts. Above them, what the one under the pointer does,
// or that an armed one wants a second click, or why one failed.
Column {
	id: root

	// The buttons' size, in pixels at scale 1.
	property real size: 48

	spacing: Greeter.u(10)

	// What's left of the armed button's time, 1 to 0.
	property real armLeft: 0

	NumberAnimation {
		id: countdown

		target: root
		property: "armLeft"
		from: 1
		to: 0
		duration: Greeter.core.armedFor
	}

	Connections {
		target: Greeter.core

		function onArmedChanged() {
			if (Greeter.core.armed && Greeter.animate)
				countdown.restart();
		}
	}

	// It fades in and out, and keeps its last words while it fades out.
	Text {
		readonly property string says: Greeter.core.powerError || (Greeter.core.armed
			? Greeter.core.tr({ suspend: "Click again to suspend", reboot: "Click again to reboot", poweroff: "Click again to power off" }[Greeter.core.armed]
				|| Greeter.core.powerLabels[Greeter.core.armed])
			: Greeter.core.powerLabels[buttons.hovered] || "")
		property string said: ""

		anchors.horizontalCenter: parent.horizontalCenter
		text: says || said
		opacity: says ? 1 : 0
		color: Greeter.core.powerError ? Greeter.colors.error : Greeter.core.armed ? Greeter.colors.accent : Greeter.colors.subtext
		font.pixelSize: Greeter.u(13)
		font.weight: Font.Medium
		font.family: Greeter.font
		onSaysChanged: if (says)
			said = says

		Behavior on opacity {
			enabled: Greeter.animate

			NumberAnimation {
				duration: 150
			}
		}
	}

	Row {
		id: buttons

		// The action under the pointer.
		property string hovered: ""

		anchors.horizontalCenter: parent.horizontalCenter
		spacing: Greeter.u(root.size * 14 / 48)

		Repeater {
			model: Greeter.core.power

			Rectangle {
				id: power

				required property string modelData
				readonly property bool armed: Greeter.core.armed === modelData

				width: Greeter.u(root.size)
				height: width
				radius: Greeter.round(width)
				color: armed ? Greeter.colors.accent : Qt.alpha(Greeter.colors.text, mouse.containsMouse ? 0.1 : 0.04)
				scale: mouse.pressed ? 0.92 : mouse.containsMouse ? 1.08 : 1

				Behavior on color {
					enabled: Greeter.animate

					ColorAnimation {
						duration: 120
					}
				}

				Behavior on scale {
					enabled: Greeter.animate

					NumberAnimation {
						duration: 150
						easing.type: Easing.OutCubic
					}
				}

				Icon {
					anchors.centerIn: parent
					path: Greeter.icons[power.modelData] || Greeter.icons.poweroff
					size: Greeter.u(root.size * 20 / 48)
					color: power.armed ? Greeter.colors.background : mouse.containsMouse ? Greeter.colors.text : Greeter.colors.subtext
				}

				// Armed, a ring around it runs out.
				Shape {
					id: ring

					anchors.centerIn: parent
					width: parent.width + Greeter.u(12)
					height: width
					visible: power.armed && Greeter.animate
					preferredRendererType: Shape.CurveRenderer

					ShapePath {
						strokeColor: Greeter.colors.accent
						strokeWidth: Greeter.u(2)
						fillColor: "transparent"
						capStyle: ShapePath.RoundCap

						PathAngleArc {
							centerX: ring.width / 2
							centerY: ring.height / 2
							radiusX: ring.width / 2 - Greeter.u(1)
							radiusY: radiusX
							startAngle: -90
							sweepAngle: 360 * root.armLeft
						}
					}
				}

				MouseArea {
					id: mouse

					anchors.fill: parent
					hoverEnabled: true
					cursorShape: Qt.PointingHandCursor
					onContainsMouseChanged: {
						if (containsMouse)
							buttons.hovered = power.modelData;
						else if (buttons.hovered === power.modelData)
							buttons.hovered = "";
					}
					onClicked: Greeter.core.press(power.modelData)
				}
			}
		}
	}
}
