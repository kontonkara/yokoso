import QtQuick

// The session to start, in a pill, shown when there's more than one: a
// click (Form.togglePicker("session")) opens the list.
Item {
	id: pill

	property Form form: null
	readonly property bool hovered: mouse.containsMouse && Greeter.core.canPick("session")

	visible: Greeter.core.sessions.length > 1
	implicitWidth: name.implicitWidth + row.spacing + Greeter.u(14) + Greeter.u(24)
	implicitHeight: Greeter.u(30)

	// Tab comes here.
	Binding {
		when: pill.form !== null
		target: pill.form
		property: "sessionShown"
		value: pill.visible
	}

	Rectangle {
		id: background

		anchors.centerIn: row
		width: row.width + Greeter.u(24)
		height: Greeter.u(30)
		radius: Greeter.round(height)
		color: Qt.alpha(Greeter.colors.text, pill.hovered ? 0.12 : 0.06)
		border.width: Greeter.outlined ? Math.max(1, Greeter.u(2)) : 0
		border.color: Greeter.outline()
	}

	FocusRing {
		anchors.fill: background
		shown: pill.form !== null && pill.form.ringed("session")
		cornerRadius: background.radius
	}

	Row {
		id: row

		anchors.centerIn: parent
		spacing: Greeter.u(6)

		Text {
			id: name

			// Its own width, unless the pill is narrower.
			width: Math.min(implicitWidth, pill.width - pill.implicitWidth + implicitWidth)
			text: Greeter.core.sessionName(Greeter.core.session)
			color: pill.hovered ? Greeter.colors.accent : Greeter.colors.subtext
			font.pixelSize: Greeter.u(13)
			font.weight: Font.Medium
			font.family: Greeter.font
			elide: Text.ElideRight
		}

		Icon {
			anchors.verticalCenter: name.verticalCenter
			path: Greeter.icons.chevron
			size: Greeter.u(14)
			color: pill.hovered ? Greeter.colors.accent : Greeter.colors.dim
		}
	}

	MouseArea {
		id: mouse

		anchors.fill: row
		anchors.margins: -Greeter.u(8)
		hoverEnabled: true
		cursorShape: Qt.PointingHandCursor
		onClicked: if (pill.form)
			pill.form.togglePicker("session")
	}
}
