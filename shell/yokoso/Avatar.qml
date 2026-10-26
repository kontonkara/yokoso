import QtQuick
import QtQuick.Effects

// The user's picture, or else their initial, in a ring of the accent (the
// initial of the name typed, while `form` asks for one); a click picks
// another user (Form.userClicked()).
Rectangle {
	id: avatar

	// The form it belongs to, if any, and the user to show (the one
	// logging in, unless the look shows another while it switches).
	property Form form: null
	property var user: Greeter.core.user
	readonly property bool askingName: form !== null && form.askingName
	readonly property bool hovered: mouse.containsMouse && Greeter.core.canPick("user") || form !== null && form.ringed("user")

	width: Greeter.u(92)
	height: width
	radius: Greeter.round(width)
	color: Qt.alpha(Greeter.colors.accent, 0.14)
	scale: hovered ? 1.04 : 1

	Behavior on scale {
		enabled: Greeter.animate

		NumberAnimation {
			duration: 150
			easing.type: Easing.OutCubic
		}
	}

	Image {
		id: picture

		anchors.fill: parent
		visible: false
		source: !avatar.askingName && avatar.user && avatar.user.icon ? Greeter.core.fileUrl(avatar.user.icon) : ""
		sourceSize: Qt.size(width * Greeter.core.dpr, height * Greeter.core.dpr)
		fillMode: Image.PreserveAspectCrop
		asynchronous: true
	}

	// The picture, cut round.
	MultiEffect {
		anchors.fill: parent
		visible: picture.status === Image.Ready
		source: picture
		maskEnabled: true
		maskSource: shape
		maskThresholdMin: 0.5
		maskSpreadAtMin: 1
	}

	Item {
		id: shape

		anchors.fill: parent
		visible: false
		layer.enabled: true

		Rectangle {
			anchors.fill: parent
			radius: Greeter.round(width)
		}
	}

	Text {
		anchors.centerIn: parent
		visible: picture.status !== Image.Ready
		text: (avatar.askingName ? avatar.form.text.trim() : avatar.user ? avatar.user.name : "").charAt(0).toUpperCase()
		color: Greeter.colors.accent
		font.pixelSize: avatar.width * 38 / 92
		font.weight: Font.DemiBold
		font.family: Greeter.font
	}

	// The ring, over the picture's edge.
	Rectangle {
		anchors.fill: parent
		radius: Greeter.round(width)
		color: "transparent"
		border.width: Greeter.u(2)
		border.color: Qt.alpha(Greeter.colors.accent, avatar.hovered ? 1 : 0.55)

		Behavior on border.color {
			enabled: Greeter.animate

			ColorAnimation {
				duration: 150
			}
		}
	}

	MouseArea {
		id: mouse

		anchors.fill: parent
		hoverEnabled: true
		cursorShape: Greeter.core.canPick("user") ? Qt.PointingHandCursor : Qt.ArrowCursor
		onClicked: if (avatar.form)
			avatar.form.userClicked()
	}
}
