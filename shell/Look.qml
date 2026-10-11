import QtQuick
import QtQuick.Shapes

// The built-in look: the clock, the form and the power buttons over the
// theme's background. Everything it shows and does comes from `greeter`,
// the core in shell.qml.
Item {
	id: look

	required property var greeter

	// Fades in from black and out to it as the session starts, a shake at
	// a wrong password, lists that drop in: config.animations.
	readonly property bool animate: !!greeter.config.animations
	property real shaken: 1

	// Line icons on a 24x24 grid, as SVG paths.
	readonly property var icons: ({
		poweroff: "M12 3v8 M17.66 6.34A8 8 0 1 1 6.34 6.34",
		reboot: "M20 12a8 8 0 1 1-2.34-5.66L20 8.5 M20 4v4.5h-4.5",
		suspend: "M12 3a6.5 6.5 0 0 0 9 9A9 9 0 1 1 12 3z",
		arrow: "M5 12h14 M13 6l6 6-6 6",
		chevron: "M6 9l6 6 6-6",
	})

	// The list open under the user's or the session's name: "user",
	// "session" or "".
	property string picking: ""
	readonly property var pickList: picking === "user"
		? greeter.users.map(u => ({ label: u.name, detail: u.realName !== u.name ? u.realName : "", current: u === greeter.user }))
		: picking === "session"
		? greeter.sessions.map(s => ({ label: greeter.sessionName(s), detail: "", current: s === greeter.session }))
		: []

	function togglePicker(which) {
		if (picking === which || !greeter.canPick(which)) {
			closePicker();
			return;
		}
		picking = which;
		list.currentIndex = pickList.findIndex(item => item.current);
		list.forceActiveFocus();
		if (animate)
			popupIn.restart();
	}

	function closePicker() {
		picking = "";
		input.forceActiveFocus();
	}

	function pick(index) {
		if (picking === "user")
			greeter.pickUser(greeter.users[index]);
		else if (picking === "session")
			greeter.pickSession(greeter.sessions[index]);
		closePicker();
	}

	function submit() {
		greeter.auth.submit(input.text);
		input.text = "";
	}

	// Every key press, before the field or the list gets it: it keeps the
	// screen awake, or wakes it and goes no further. F2 opens or closes
	// the user list, F3 the session list; in a list Enter picks and Escape
	// closes it, and Escape clears the field.
	function keyPressed(event) {
		event.accepted = true;
		if (greeter.wake())
			return;
		const enter = event.key === Qt.Key_Return || event.key === Qt.Key_Enter;
		if (event.key === Qt.Key_F2 || event.key === Qt.Key_F3)
			togglePicker(event.key === Qt.Key_F2 ? "user" : "session");
		else if (picking && enter)
			pick(list.currentIndex);
		else if (picking && event.key === Qt.Key_Escape)
			closePicker();
		else if (event.key === Qt.Key_Escape)
			input.text = "";
		else
			event.accepted = false;
	}

	Component.onCompleted: if (animate)
		fadeIn.start()

	Connections {
		target: greeter.auth

		function onPhaseChanged() {
			if (!look.animate)
				return;
			if (greeter.auth.phase === "failed")
				shake.restart();
			else if (greeter.auth.phase === "launching") {
				// A login without a password can come before the fade in
				// is over.
				fadeIn.stop();
				fadeOut.start();
			}
		}
	}

	// One of `icons`, stroked in `color`, `size` pixels square.
	component Icon: Shape {
		id: icon

		property string path
		property color color
		property real size: 20

		width: size
		height: size
		preferredRendererType: Shape.CurveRenderer

		ShapePath {
			strokeColor: icon.color
			strokeWidth: Math.max(1.5, icon.size / 10)
			fillColor: "transparent"
			capStyle: ShapePath.RoundCap
			joinStyle: ShapePath.RoundJoin
			scale: Qt.size(icon.size / 24, icon.size / 24)

			PathSvg {
				path: icon.path
			}
		}
	}

	// The background, decoded at each screen's size.
	Repeater {
		model: greeter.screens

		Image {
			required property var modelData

			x: modelData.rect.x
			y: modelData.rect.y
			width: modelData.rect.width
			height: modelData.rect.height
			source: greeter.backgroundFor(modelData.name)
			sourceSize: Qt.size(width * greeter.dpr, height * greeter.dpr)
			fillMode: Image.PreserveAspectCrop

			// So that text stays readable on a bright image.
			Rectangle {
				anchors.fill: parent
				visible: parent.status === Image.Ready
				color: greeter.colors.background
				opacity: greeter.config.backgroundDim
			}
		}
	}

	// The clock: large and light, with figures of one width so that it
	// doesn't shift as the minutes go.
	Column {
		x: greeter.area.x + (greeter.area.width - width) / 2
		y: form.y - height - greeter.u(72)
		spacing: greeter.u(2)

		Text {
			anchors.horizontalCenter: parent.horizontalCenter
			text: greeter.now.toLocaleTimeString(greeter.locale, Locale.ShortFormat)
			color: greeter.colors.text
			font.pixelSize: greeter.u(120)
			font.weight: Font.Light
			font.letterSpacing: -greeter.u(3)
			font.features: { "tnum": 1 }
			font.family: greeter.fontFamily
		}

		// The locale's long date, without the year.
		Text {
			anchors.horizontalCenter: parent.horizontalCenter
			text: greeter.now.toLocaleDateString(greeter.locale,
				greeter.locale.dateFormat(Locale.LongFormat).replace(/[\s,.]*y+[^dM]*/, "")).toUpperCase()
			color: greeter.colors.subtext
			font.pixelSize: greeter.u(15)
			font.weight: Font.Medium
			font.letterSpacing: greeter.u(3)
			font.family: greeter.fontFamily
		}
	}

	Column {
		id: form

		x: greeter.area.x + (greeter.area.width - width) / 2
		y: greeter.area.y + (greeter.area.height - height) / 2 + greeter.u(40)
		width: greeter.u(340)
		spacing: greeter.u(14)

		// The user's initial in a ring of the accent; a click picks
		// another user, like one on the name.
		Rectangle {
			anchors.horizontalCenter: parent.horizontalCenter
			width: greeter.u(92)
			height: width
			radius: width / 2
			color: Qt.alpha(greeter.colors.accent, 0.14)
			border.width: greeter.u(2)
			border.color: Qt.alpha(greeter.colors.accent, avatarMouse.containsMouse && greeter.canPick("user") ? 1 : 0.55)

			Text {
				anchors.centerIn: parent
				text: greeter.user ? greeter.user.name.charAt(0).toUpperCase() : ""
				color: greeter.colors.accent
				font.pixelSize: greeter.u(38)
				font.weight: Font.DemiBold
				font.family: greeter.fontFamily
			}

			MouseArea {
				id: avatarMouse

				anchors.fill: parent
				hoverEnabled: true
				cursorShape: greeter.canPick("user") ? Qt.PointingHandCursor : Qt.ArrowCursor
				onClicked: look.togglePicker("user")
			}
		}

		// The user, and the session (in a pill) when there's more than
		// one: a click (or F2, F3) opens the list to pick another from.
		Repeater {
			id: labels

			model: ["user", "session"]

			Item {
				id: label

				required property string modelData
				readonly property bool isUser: modelData === "user"
				readonly property bool hovered: mouse.containsMouse && greeter.canPick(modelData)

				width: parent.width
				height: isUser ? name.height : greeter.u(30)
				visible: isUser || greeter.sessions.length > 1

				Rectangle {
					anchors.centerIn: row
					width: row.width + greeter.u(24)
					height: greeter.u(30)
					radius: height / 2
					visible: !label.isUser
					color: Qt.alpha(greeter.colors.text, label.hovered ? 0.12 : 0.06)
				}

				Row {
					id: row

					anchors.centerIn: parent
					spacing: greeter.u(6)

					Text {
						id: name

						width: Math.min(implicitWidth, label.width - greeter.u(40))
						text: label.isUser ? (greeter.user ? greeter.user.name : "") : greeter.sessionName(greeter.session)
						color: label.hovered ? greeter.colors.accent : label.isUser ? greeter.colors.text : greeter.colors.subtext
						font.pixelSize: greeter.u(label.isUser ? 24 : 13)
						font.weight: label.isUser ? Font.DemiBold : Font.Medium
						font.family: greeter.fontFamily
						elide: Text.ElideRight
					}

					Icon {
						anchors.verticalCenter: name.verticalCenter
						visible: (label.isUser ? greeter.users : greeter.sessions).length > 1
						path: look.icons.chevron
						size: greeter.u(label.isUser ? 18 : 14)
						color: label.hovered ? greeter.colors.accent : greeter.colors.dim
					}
				}

				MouseArea {
					id: mouse

					anchors.fill: row
					anchors.margins: -greeter.u(8)
					hoverEnabled: true
					cursorShape: Qt.PointingHandCursor
					onClicked: look.togglePicker(label.modelData)
				}
			}
		}

		Item {
			width: 1
			height: greeter.u(4)
		}

		// The password field: its outline in the accent while it waits
		// for typing and in the error color under an error, and a button
		// at its end, an arrow that turns into a spinner while the
		// password is checked.
		Rectangle {
			id: field

			// The placeholder below shows an error.
			readonly property bool error: input.text === "" && (greeter.auth.messageIsError || greeter.problem !== "")
			readonly property bool busy: greeter.auth.phase === "verifying" || greeter.auth.phase === "launching"

			width: parent.width
			height: greeter.u(50)
			radius: height / 2
			color: greeter.colors.field
			border.width: greeter.u(2)
			border.color: error ? greeter.colors.error
				: greeter.auth.phase === "prompt" && input.activeFocus ? Qt.alpha(greeter.colors.accent, input.text ? 0.9 : 0.4)
				: Qt.alpha(greeter.colors.text, 0.08)
			// A shake, dying down as look.shaken goes from 0 to 1.
			transform: Translate {
				x: Math.sin(look.shaken * Math.PI * 6) * (1 - look.shaken) * greeter.u(10)
			}

			Behavior on border.color {
				enabled: look.animate

				ColorAnimation {
					duration: 150
				}
			}

			TextInput {
				id: input

				anchors.left: parent.left
				anchors.right: button.left
				anchors.leftMargin: greeter.u(22)
				anchors.rightMargin: greeter.u(10)
				anchors.verticalCenter: parent.verticalCenter
				opacity: greeter.auth.phase === "prompt" ? 1 : 0.5
				focus: true
				readOnly: greeter.auth.phase !== "prompt"
				echoMode: greeter.auth.secret ? TextInput.Password : TextInput.Normal
				passwordCharacter: "•"
				color: greeter.colors.text
				font.pixelSize: greeter.u(greeter.auth.secret && text ? 22 : 16)
				font.letterSpacing: greeter.auth.secret ? greeter.u(2) : 0
				font.family: greeter.fontFamily
				clip: true
				onAccepted: look.submit()
				Keys.onPressed: event => look.keyPressed(event)
			}

			// PAM's question, or its last message (a failed attempt
			// included) until something is typed.
			Text {
				anchors.fill: input
				verticalAlignment: Text.AlignVCenter
				visible: input.text === ""
				text: greeter.problem || greeter.auth.message || greeter.auth.prompt.replace(/:\s*$/, "")
				color: greeter.auth.messageIsError || greeter.problem ? greeter.colors.error : greeter.colors.dim
				font.pixelSize: greeter.u(16)
				font.family: greeter.fontFamily
				elide: Text.ElideRight
			}

			Rectangle {
				id: button

				readonly property bool ready: input.text !== "" && greeter.auth.phase === "prompt"

				anchors.right: parent.right
				anchors.rightMargin: greeter.u(7)
				anchors.verticalCenter: parent.verticalCenter
				width: greeter.u(36)
				height: width
				radius: width / 2
				color: ready ? greeter.colors.accent : Qt.alpha(greeter.colors.text, 0.06)

				Behavior on color {
					enabled: look.animate

					ColorAnimation {
						duration: 150
					}
				}

				Icon {
					anchors.centerIn: parent
					visible: !field.busy
					path: look.icons.arrow
					size: greeter.u(18)
					color: button.ready ? greeter.colors.background : greeter.colors.dim
				}

				// Three quarters of a ring, turning.
				Shape {
					anchors.centerIn: parent
					width: greeter.u(18)
					height: width
					visible: field.busy
					preferredRendererType: Shape.CurveRenderer

					ShapePath {
						strokeColor: greeter.colors.accent
						strokeWidth: Math.max(1.5, greeter.u(18) / 10)
						fillColor: "transparent"
						capStyle: ShapePath.RoundCap

						PathAngleArc {
							centerX: greeter.u(9)
							centerY: greeter.u(9)
							radiusX: greeter.u(8)
							radiusY: greeter.u(8)
							sweepAngle: 270
						}
					}

					RotationAnimator on rotation {
						running: field.busy && look.animate
						from: 0
						to: 360
						duration: 800
						loops: Animation.Infinite
					}
				}

				MouseArea {
					anchors.fill: parent
					cursorShape: button.ready ? Qt.PointingHandCursor : Qt.ArrowCursor
					onClicked: if (button.ready)
						look.submit()
				}
			}
		}

		// Hidden rather than gone, so the form doesn't move.
		Text {
			width: parent.width
			horizontalAlignment: Text.AlignHCenter
			text: "Caps Lock is on"
			color: greeter.colors.error
			font.pixelSize: greeter.u(13)
			font.weight: Font.Medium
			font.family: greeter.fontFamily
			opacity: greeter.capsLock ? 1 : 0
		}
	}

	// Power: round buttons with icons. Above them, what the one under the
	// pointer does, or that an armed one wants a second click.
	Column {
		x: greeter.area.x + (greeter.area.width - width) / 2
		y: greeter.area.y + greeter.area.height - height - greeter.u(40)
		spacing: greeter.u(10)

		Text {
			anchors.horizontalCenter: parent.horizontalCenter
			text: greeter.powerError || (greeter.armed ? `Click again to ${greeter.powerLabels[greeter.armed].toLowerCase()}`
				: greeter.powerLabels[powerRow.hovered] || "")
			color: greeter.powerError ? greeter.colors.error : greeter.armed ? greeter.colors.accent : greeter.colors.subtext
			font.pixelSize: greeter.u(13)
			font.weight: Font.Medium
			font.family: greeter.fontFamily
		}

		Row {
			id: powerRow

			// The action under the pointer.
			property string hovered: ""

			anchors.horizontalCenter: parent.horizontalCenter
			spacing: greeter.u(14)

			Repeater {
				model: greeter.power

				Rectangle {
					id: power

					required property string modelData
					readonly property bool armed: greeter.armed === modelData

					width: greeter.u(48)
					height: width
					radius: width / 2
					color: armed ? greeter.colors.accent : Qt.alpha(greeter.colors.text, powerMouse.containsMouse ? 0.1 : 0.04)

					Behavior on color {
						enabled: look.animate

						ColorAnimation {
							duration: 120
						}
					}

					Icon {
						anchors.centerIn: parent
						path: look.icons[power.modelData] || look.icons.poweroff
						size: greeter.u(20)
						color: power.armed ? greeter.colors.background : powerMouse.containsMouse ? greeter.colors.text : greeter.colors.subtext
					}

					MouseArea {
						id: powerMouse

						anchors.fill: parent
						hoverEnabled: true
						cursorShape: Qt.PointingHandCursor
						onContainsMouseChanged: {
							if (containsMouse)
								powerRow.hovered = power.modelData;
							else if (powerRow.hovered === power.modelData)
								powerRow.hovered = "";
						}
						onClicked: greeter.press(power.modelData)
					}
				}
			}
		}
	}

	// A click anywhere else closes the list.
	MouseArea {
		anchors.fill: parent
		visible: look.picking !== ""
		onClicked: look.closePicker()
	}

	Rectangle {
		id: popup

		readonly property var label: labels.itemAt(look.picking === "session" ? 1 : 0)

		x: form.x
		y: label ? form.y + label.y + label.height + greeter.u(8) : 0
		width: form.width
		height: Math.min(list.count, 8) * greeter.u(40) + greeter.u(12)
		visible: look.picking !== ""
		radius: greeter.u(16)
		color: greeter.colors.field
		border.color: Qt.alpha(greeter.colors.text, 0.1)
		border.width: 1
		transform: Translate {
			id: popupShift
		}

		ListView {
			id: list

			anchors.fill: parent
			anchors.margins: greeter.u(6)
			clip: true
			boundsBehavior: Flickable.StopAtBounds
			model: look.pickList
			onCurrentIndexChanged: positionViewAtIndex(currentIndex, ListView.Contain)
			Keys.onPressed: event => look.keyPressed(event)

			delegate: Item {
				required property var modelData
				required property int index

				width: ListView.view.width
				height: greeter.u(40)

				Rectangle {
					anchors.fill: parent
					radius: greeter.u(10)
					visible: list.currentIndex === parent.index
					color: Qt.alpha(greeter.colors.accent, 0.15)
				}

				// A dot by the one picked now.
				Rectangle {
					anchors.verticalCenter: parent.verticalCenter
					x: greeter.u(8)
					width: greeter.u(6)
					height: width
					radius: width / 2
					visible: parent.modelData.current
					color: greeter.colors.accent
				}

				Text {
					anchors.left: parent.left
					anchors.right: detail.left
					anchors.leftMargin: greeter.u(22)
					anchors.rightMargin: greeter.u(10)
					anchors.verticalCenter: parent.verticalCenter
					text: parent.modelData.label
					color: greeter.colors.text
					font.pixelSize: greeter.u(15)
					font.weight: parent.modelData.current ? Font.DemiBold : Font.Normal
					font.family: greeter.fontFamily
					elide: Text.ElideRight
				}

				Text {
					id: detail

					anchors.right: parent.right
					anchors.rightMargin: greeter.u(14)
					anchors.verticalCenter: parent.verticalCenter
					width: Math.min(implicitWidth, parent.width / 2)
					text: parent.modelData.detail
					color: greeter.colors.dim
					font.pixelSize: greeter.u(13)
					font.family: greeter.fontFamily
					elide: Text.ElideRight
				}

				MouseArea {
					anchors.fill: parent
					hoverEnabled: true
					onEntered: list.currentIndex = parent.index
					onClicked: look.pick(parent.index)
				}
			}
		}
	}

	// Dark while the screen is idle, with the pointer hidden; a click on
	// it only wakes the screen.
	Rectangle {
		id: dark

		anchors.fill: parent
		visible: opacity > 0
		color: "black"
		opacity: 0
		states: State {
			name: "asleep"
			when: greeter.idle

			PropertyChanges {
				dark.opacity: 1
			}
		}
		transitions: [
			Transition {
				to: "asleep"
				enabled: look.animate

				NumberAnimation {
					property: "opacity"
					duration: 1000
				}
			},
			Transition {
				from: "asleep"
				enabled: look.animate

				NumberAnimation {
					property: "opacity"
					duration: 150
				}
			}
		]

		MouseArea {
			anchors.fill: parent
			enabled: greeter.idle
			hoverEnabled: true
			cursorShape: Qt.BlankCursor
			onPressed: greeter.wake()
		}
	}

	// Black over everything, while the look fades in from cage's black
	// and out as the session starts.
	Rectangle {
		id: curtain

		anchors.fill: parent
		visible: opacity > 0
		color: "black"
		opacity: look.animate ? 1 : 0
	}

	NumberAnimation {
		id: fadeIn

		target: curtain
		property: "opacity"
		to: 0
		duration: 400
		easing.type: Easing.OutQuad
	}

	NumberAnimation {
		id: fadeOut

		target: curtain
		property: "opacity"
		to: 1
		duration: 300
		easing.type: Easing.InQuad
	}

	NumberAnimation {
		id: shake

		target: look
		property: "shaken"
		from: 0
		to: 1
		duration: 450
	}

	ParallelAnimation {
		id: popupIn

		NumberAnimation {
			target: popup
			property: "opacity"
			from: 0
			to: 1
			duration: 150
		}

		NumberAnimation {
			target: popupShift
			property: "y"
			from: -greeter.u(6)
			to: 0
			duration: 150
			easing.type: Easing.OutCubic
		}
	}
}
