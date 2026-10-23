import QtQuick

// The user's name, with a chevron when there are others to pick: a click
// (Form.userClicked()) opens the list, or without a user list types the
// name again. Hidden, keeping its place, while `form` asks for a name.
Item {
	id: label

	property Form form: null
	property var user: Greeter.core.user
	// The name's size, in pixels at scale 1.
	property real size: 24
	readonly property bool askingName: form !== null && form.askingName
	readonly property bool hovered: mouse.containsMouse
		&& (Greeter.core.userList ? Greeter.core.canPick("user") : Greeter.core.canEnterUser())

	implicitWidth: name.implicitWidth + (chevron.visible ? row.spacing + chevron.width : 0)
	implicitHeight: name.height

	Row {
		id: row

		anchors.centerIn: parent
		spacing: Greeter.u(6)
		opacity: label.askingName ? 0 : 1

		Text {
			id: name

			// Its own width, unless the label is narrower.
			width: Math.min(implicitWidth, label.width - (chevron.visible ? row.spacing + chevron.width : 0))
			text: label.user ? label.user.name : ""
			color: label.hovered ? Greeter.colors.accent : Greeter.colors.text
			font.pixelSize: Greeter.u(label.size)
			font.weight: Font.DemiBold
			font.family: Greeter.font
			elide: Text.ElideRight
		}

		Icon {
			id: chevron

			anchors.verticalCenter: name.verticalCenter
			visible: Greeter.core.users.length > 1
			path: Greeter.icons.chevron
			size: Greeter.u(label.size * 3 / 4)
			color: label.hovered ? Greeter.colors.accent : Greeter.colors.dim
		}
	}

	MouseArea {
		id: mouse

		anchors.fill: row
		anchors.margins: -Greeter.u(8)
		hoverEnabled: true
		cursorShape: Qt.PointingHandCursor
		enabled: !label.askingName
		onClicked: if (label.form)
			label.form.userClicked()
	}
}
