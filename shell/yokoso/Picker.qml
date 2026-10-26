import QtQuick

// The list `form` has open (Form.picking), dropping in under `under` (the
// UserName or the SessionPill it belongs to), from its left edge, as wide
// as it or minWidth; up and down, Enter, Escape and the pointer pick. A
// click anywhere else closes it. Fill the look with it.
Item {
	id: root

	required property Form form
	property Item under: null
	property real minWidth: Greeter.u(240)

	// A click anywhere else closes the list.
	MouseArea {
		anchors.fill: parent
		visible: root.form.picking !== ""
		onClicked: root.form.closePicker()
	}

	Rectangle {
		id: popup

		readonly property point at: root.under && root.form.picking ? root.under.mapToItem(root, 0, root.under.height) : Qt.point(0, 0)

		x: Math.max(0, Math.min(at.x, root.width - width))
		y: at.y + Greeter.u(8)
		width: root.under ? Math.max(root.under.width, root.minWidth) : 0
		height: Math.min(list.count, 8) * Greeter.u(40) + Greeter.u(12)
		visible: root.form.picking !== ""
		radius: Greeter.radius < 0 ? Greeter.u(16) : Greeter.radius
		color: Greeter.colors.field
		border.color: Greeter.outline(0.1)
		border.width: Greeter.outlined ? Math.max(1, Greeter.u(2)) : 1
		transform: Translate {
			id: shift
		}

		ListView {
			id: list

			anchors.fill: parent
			anchors.margins: Greeter.u(6)
			clip: true
			boundsBehavior: Flickable.StopAtBounds
			model: root.form.pickList
			Component.onCompleted: root.form.list = list
			onCurrentIndexChanged: positionViewAtIndex(currentIndex, ListView.Contain)
			// Under the entry the keys or the pointer are on, sliding from
			// one to the next.
			highlight: Rectangle {
				radius: Math.max(0, popup.radius - Greeter.u(6))
				color: Qt.alpha(Greeter.colors.accent, 0.15)
				border.width: Greeter.outlined ? Math.max(1, Greeter.u(2)) : 0
				border.color: Greeter.colors.accent
			}
			highlightMoveDuration: Greeter.animate ? 120 : 0
			highlightMoveVelocity: -1
			highlightResizeDuration: 0
			Keys.onPressed: event => root.form.keyPressed(event)

			delegate: Item {
				required property var modelData
				required property int index

				width: ListView.view.width
				height: Greeter.u(40)

				// A dot by the one picked now.
				Rectangle {
					anchors.verticalCenter: parent.verticalCenter
					x: Greeter.u(8)
					width: Greeter.u(6)
					height: width
					radius: width / 2
					visible: parent.modelData.current
					color: Greeter.colors.accent
				}

				Text {
					anchors.left: parent.left
					anchors.right: detail.left
					anchors.leftMargin: Greeter.u(22)
					anchors.rightMargin: Greeter.u(10)
					anchors.verticalCenter: parent.verticalCenter
					text: parent.modelData.label
					color: parent.modelData.other ? Greeter.colors.subtext : Greeter.colors.text
					font.pixelSize: Greeter.u(15)
					font.weight: parent.modelData.current ? Font.DemiBold : Font.Normal
					font.family: Greeter.font
					elide: Text.ElideRight
				}

				Text {
					id: detail

					anchors.right: parent.right
					anchors.rightMargin: Greeter.u(14)
					anchors.verticalCenter: parent.verticalCenter
					width: Math.min(implicitWidth, parent.width / 2)
					text: parent.modelData.detail
					color: Greeter.colors.dim
					font.pixelSize: Greeter.u(13)
					font.family: Greeter.font
					elide: Text.ElideRight
				}

				MouseArea {
					anchors.fill: parent
					hoverEnabled: true
					onEntered: list.currentIndex = parent.index
					onClicked: root.form.pick(parent.index)
				}
			}
		}
	}

	// It drops in as it opens.
	Connections {
		target: root.form

		function onPickingChanged() {
			if (root.form.picking && Greeter.animate)
				dropIn.restart();
		}
	}

	ParallelAnimation {
		id: dropIn

		NumberAnimation {
			target: popup
			property: "opacity"
			from: 0
			to: 1
			duration: 150
		}

		NumberAnimation {
			target: shift
			property: "y"
			from: -Greeter.u(6)
			to: 0
			duration: 150
			easing.type: Easing.OutCubic
		}
	}
}
