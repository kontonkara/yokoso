import QtQuick

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

	// F2 opens or closes the user list, F3 the session list.
	function pickerKey(event) {
		const which = event.key === Qt.Key_F2 ? "user" : event.key === Qt.Key_F3 ? "session" : "";
		if (which) {
			togglePicker(which);
			event.accepted = true;
		}
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

	Column {
		x: greeter.area.x + (greeter.area.width - width) / 2
		y: form.y - height - greeter.u(48)
		spacing: greeter.u(4)

		Text {
			anchors.horizontalCenter: parent.horizontalCenter
			text: greeter.now.toLocaleTimeString(greeter.locale, Locale.ShortFormat)
			color: greeter.colors.text
			font.pixelSize: greeter.u(56)
			font.family: greeter.fontFamily
		}

		// The locale's long date, without the year.
		Text {
			anchors.horizontalCenter: parent.horizontalCenter
			text: greeter.now.toLocaleDateString(greeter.locale,
				greeter.locale.dateFormat(Locale.LongFormat).replace(/[\s,.]*y+[^dM]*/, ""))
			color: greeter.colors.subtext
			font.pixelSize: greeter.u(16)
			font.family: greeter.fontFamily
		}
	}

	Column {
		id: form

		x: greeter.area.x + (greeter.area.width - width) / 2
		y: greeter.area.y + (greeter.area.height - height) / 2
		width: greeter.u(300)
		spacing: greeter.u(10)

		// The user, and the session when there's more than one: a
		// click (or F2, F3) opens the list to pick another from.
		Repeater {
			id: labels

			model: ["user", "session"]

			Item {
				id: label

				required property string modelData
				readonly property bool isUser: modelData === "user"

				width: parent.width
				height: name.height
				visible: isUser || greeter.sessions.length > 1

				Row {
					id: row

					anchors.horizontalCenter: parent.horizontalCenter
					spacing: greeter.u(6)

					Text {
						id: name

						width: Math.min(implicitWidth, label.width - (chevron.visible ? chevron.width + row.spacing : 0))
						text: label.isUser ? (greeter.user ? greeter.user.name : "") : greeter.sessionName(greeter.session)
						color: mouse.containsMouse && greeter.canPick(label.modelData) ? greeter.colors.accent
							: label.isUser ? greeter.colors.text : greeter.colors.subtext
						font.pixelSize: greeter.u(label.isUser ? 20 : 14)
						font.family: greeter.fontFamily
						elide: Text.ElideRight
					}

					Text {
						id: chevron

						anchors.verticalCenter: name.verticalCenter
						visible: (label.isUser ? greeter.users : greeter.sessions).length > 1
						text: "▾"
						color: mouse.containsMouse && greeter.canPick(label.modelData) ? greeter.colors.accent : greeter.colors.dim
						font.pixelSize: greeter.u(label.isUser ? 14 : 11)
						font.family: greeter.fontFamily
					}
				}

				MouseArea {
					id: mouse

					anchors.fill: row
					anchors.margins: -greeter.u(4)
					hoverEnabled: true
					cursorShape: Qt.PointingHandCursor
					onClicked: look.togglePicker(label.modelData)
				}
			}
		}

		// The password field, with a line under it: the accent while it
		// waits for typing, the error color under an error, and a bar
		// running along it while the password is checked.
		Item {
			id: field

			// The placeholder below shows an error.
			readonly property bool error: input.text === "" && (greeter.auth.messageIsError || greeter.problem !== "")

			width: parent.width
			height: greeter.u(36)
			// A shake, dying down as look.shaken goes from 0 to 1.
			transform: Translate {
				x: Math.sin(look.shaken * Math.PI * 6) * (1 - look.shaken) * greeter.u(10)
			}

			Rectangle {
				anchors.fill: parent
				color: greeter.colors.field
				opacity: greeter.auth.phase === "prompt" ? 1 : 0.5

				TextInput {
					id: input

					anchors.fill: parent
					anchors.leftMargin: greeter.u(10)
					anchors.rightMargin: greeter.u(10)
					verticalAlignment: TextInput.AlignVCenter
					focus: true
					readOnly: greeter.auth.phase !== "prompt"
					echoMode: greeter.auth.secret ? TextInput.Password : TextInput.Normal
					color: greeter.colors.text
					font.pixelSize: greeter.u(15)
					font.family: greeter.fontFamily
					clip: true
					onAccepted: {
						greeter.auth.submit(text);
						text = "";
					}
					Keys.onEscapePressed: text = ""
					Keys.onPressed: event => look.pickerKey(event)
				}

				// PAM's question, or its last message (a failed attempt
				// included) until something is typed.
				Text {
					anchors.fill: input
					verticalAlignment: Text.AlignVCenter
					visible: input.text === ""
					text: greeter.problem || greeter.auth.message || greeter.auth.prompt.replace(/:\s*$/, "")
					color: greeter.auth.messageIsError || greeter.problem ? greeter.colors.error : greeter.colors.dim
					font.pixelSize: greeter.u(15)
					font.family: greeter.fontFamily
					elide: Text.ElideRight
				}
			}

			Item {
				anchors.left: parent.left
				anchors.right: parent.right
				anchors.bottom: parent.bottom
				height: greeter.u(2)
				clip: true

				Rectangle {
					anchors.fill: parent
					visible: field.error || greeter.auth.phase === "prompt" && input.activeFocus
					color: field.error ? greeter.colors.error : greeter.colors.accent
				}

				Rectangle {
					id: bar

					width: parent.width / 3
					height: parent.height
					visible: look.animate && (greeter.auth.phase === "verifying" || greeter.auth.phase === "launching")
					color: greeter.colors.accent

					NumberAnimation on x {
						running: bar.visible
						loops: Animation.Infinite
						from: -bar.width
						to: field.width
						duration: 1100
						easing.type: Easing.InOutSine
					}
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
			font.family: greeter.fontFamily
			opacity: greeter.capsLock ? 1 : 0
		}
	}

	Column {
		x: greeter.area.x + (greeter.area.width - width) / 2
		y: greeter.area.y + greeter.area.height - height - greeter.u(32)
		spacing: greeter.u(12)

		Text {
			anchors.horizontalCenter: parent.horizontalCenter
			text: greeter.powerError || (greeter.armed ? `Click ${greeter.powerLabels[greeter.armed]} again` : "")
			color: greeter.powerError ? greeter.colors.error : greeter.colors.subtext
			font.pixelSize: greeter.u(13)
			font.family: greeter.fontFamily
		}

		Row {
			anchors.horizontalCenter: parent.horizontalCenter
			spacing: greeter.u(32)

			Repeater {
				model: greeter.power

				Text {
					required property string modelData

					text: greeter.powerLabels[modelData] || modelData
					color: greeter.armed === modelData || mouse.containsMouse ? greeter.colors.accent : greeter.colors.dim
					font.pixelSize: greeter.u(14)
					font.family: greeter.fontFamily

					MouseArea {
						id: mouse

						anchors.fill: parent
						anchors.margins: -greeter.u(8)
						hoverEnabled: true
						cursorShape: Qt.PointingHandCursor
						onClicked: greeter.press(parent.modelData)
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
		y: label ? form.y + label.y + label.height + greeter.u(6) : 0
		width: form.width
		height: Math.min(list.count, 8) * greeter.u(32) + 2
		visible: look.picking !== ""
		color: greeter.colors.field
		border.color: greeter.colors.dim
		border.width: 1
		transform: Translate {
			id: popupShift
		}

		ListView {
			id: list

			anchors.fill: parent
			anchors.margins: 1
			clip: true
			boundsBehavior: Flickable.StopAtBounds
			model: look.pickList
			onCurrentIndexChanged: positionViewAtIndex(currentIndex, ListView.Contain)
			Keys.onReturnPressed: look.pick(currentIndex)
			Keys.onEnterPressed: look.pick(currentIndex)
			Keys.onEscapePressed: look.closePicker()
			Keys.onPressed: event => look.pickerKey(event)

			delegate: Item {
				required property var modelData
				required property int index

				width: ListView.view.width
				height: greeter.u(32)

				Rectangle {
					anchors.fill: parent
					visible: list.currentIndex === parent.index
					color: greeter.colors.accent
					opacity: 0.15
				}

				// The one picked now.
				Rectangle {
					width: greeter.u(3)
					height: parent.height
					visible: parent.modelData.current
					color: greeter.colors.accent
				}

				Text {
					anchors.left: parent.left
					anchors.right: detail.left
					anchors.leftMargin: greeter.u(10)
					anchors.rightMargin: greeter.u(10)
					anchors.verticalCenter: parent.verticalCenter
					text: parent.modelData.label
					color: greeter.colors.text
					font.pixelSize: greeter.u(15)
					font.family: greeter.fontFamily
					elide: Text.ElideRight
				}

				Text {
					id: detail

					anchors.right: parent.right
					anchors.rightMargin: greeter.u(10)
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
