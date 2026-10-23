import QtQuick
import QtQuick.Effects
import QtQuick.Shapes

// The built-in look: the clock, the form and the power buttons over the
// theme's background. Everything it shows and does comes from `greeter`,
// the core in shell.qml.
Item {
	id: look

	required property var greeter

	// Everything that moves (fades, slides, the shake at a wrong password,
	// buttons answering the pointer): config.animations.
	readonly property bool animate: !!greeter.config.animations
	property real shaken: 1

	// Where the parts go, and how round they are (the theme's layout and
	// radius; see the README).
	readonly property var layout: greeter.config.layout || {}
	readonly property real radius: typeof greeter.config.radius === "number" ? greeter.u(greeter.config.radius) : -1

	// The corner radius for something `size` across: round, unless the
	// theme sets one; `inset`, how far inside a rounded thing it sits.
	function round(size, inset) {
		return radius < 0 ? size / 2 : Math.min(size / 2, Math.max(0, radius - (inset || 0)));
	}

	// The form's screen in the window. When the form goes to another
	// screen (the pointer's, with output "cursor"), it slides there.
	property rect area: greeter.area

	Behavior on area {
		enabled: look.animate && !greeter.windowIsScreen

		PropertyAnimation {
			duration: 500
			easing.type: Easing.InOutCubic
		}
	}

	// The parts (0 the clock, 1 the avatar, 2 the names, 3 the field, 4
	// the power buttons) come in one after another, rising, as `intro`
	// goes from 0 to 1, and go up and away as `outro` does.
	property real intro: animate ? 0 : 1
	property real outro: 0

	function entered(part) {
		const t = Math.min(1, Math.max(0, (intro * introAnim.duration - part * 70) / 450));
		return 1 - Math.pow(1 - t, 3);
	}

	function fade(part) {
		return entered(part) * (1 - outro);
	}

	function lift(part) {
		return (1 - entered(part)) * greeter.u(16) - outro * greeter.u(24);
	}

	// The user on show: the one before slides out, the new one in.
	property var shownUser: greeter.user
	property real userShift: 0
	property real userFade: 1

	// What's left of an armed power button's time, 1 to 0.
	property real armLeft: 0

	// Line icons on a 24x24 grid, as SVG paths.
	readonly property var icons: ({
		poweroff: "M12 3v8 M17.66 6.34A8 8 0 1 1 6.34 6.34",
		reboot: "M20 12a8 8 0 1 1-2.34-5.66L20 8.5 M20 4v4.5h-4.5",
		suspend: "M12 3a6.5 6.5 0 0 0 9 9A9 9 0 1 1 12 3z",
		arrow: "M5 12h14 M13 6l6 6-6 6",
		chevron: "M6 9l6 6 6-6",
		bolt: "M13 3L5 13h6l-1 8 8-10h-6l1-8z",
	})

	// The list open under the user's or the session's name: "user",
	// "session" or "".
	property string picking: ""
	readonly property var pickList: picking === "user"
		? greeter.users.map(u => ({ label: u.name, detail: u.realName !== u.name ? u.realName : "", current: u === greeter.user }))
			.concat([{ label: greeter.tr("Other…"), detail: "", current: false, other: true }])
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
		const other = picking === "user" && index === greeter.users.length;
		if (picking === "user" && !other)
			greeter.pickUser(greeter.users[index]);
		else if (picking === "session")
			greeter.pickSession(greeter.sessions[index]);
		closePicker();
		if (other)
			enterName("");
	}

	// A name typed into the field instead of the password: while there's
	// no user (without userList, until one is typed), and after "Other…"
	// or a click on a typed name.
	property bool enteringName: false
	readonly property bool askingName: enteringName || greeter.needsUser
	// Whether the field takes typing now, and whether what's typed is a
	// secret.
	readonly property bool typing: askingName || greeter.auth.phase === "prompt"
	readonly property bool secret: !askingName && greeter.auth.secret

	// Types a name in the field, starting from `name`.
	function enterName(name) {
		if (!greeter.canEnterUser())
			return;
		enteringName = true;
		input.text = name;
		input.selectAll();
		input.forceActiveFocus();
	}

	// A click on the user (or F2): the list, or without one the name to
	// type again.
	function userClicked() {
		if (greeter.userList)
			togglePicker("user");
		else if (!askingName)
			enterName(greeter.user ? greeter.user.name : "");
	}

	function submit() {
		if (askingName) {
			const name = input.text.trim();
			if (!name)
				return;
			enteringName = false;
			input.text = "";
			greeter.enterUser(name);
			return;
		}
		greeter.auth.submit(input.text);
		input.text = "";
	}

	// Every key press, before the field or the list gets it: it keeps the
	// screen awake, or wakes it and goes no further. F2 opens or closes
	// the user list (without one, types the name again), F3 the session
	// list; in a list Enter picks and Escape closes it, and Escape clears
	// the field (and goes back from typing a name to the user's password).
	function keyPressed(event) {
		event.accepted = true;
		if (greeter.wake())
			return;
		const enter = event.key === Qt.Key_Return || event.key === Qt.Key_Enter;
		if (event.key === Qt.Key_F2)
			userClicked();
		else if (event.key === Qt.Key_F3)
			togglePicker("session");
		else if (picking && enter)
			pick(list.currentIndex);
		else if (picking && event.key === Qt.Key_Escape)
			closePicker();
		else if (event.key === Qt.Key_Escape) {
			input.text = "";
			if (greeter.user)
				enteringName = false;
		} else
			event.accepted = false;
	}

	Component.onCompleted: {
		// From here on it changes only by userSwitch.
		shownUser = greeter.user;
		if (animate) {
			fadeIn.start();
			introAnim.start();
		}
	}

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
				outroAnim.start();
			}
		}
	}

	Connections {
		target: greeter

		function onUserChanged() {
			if (look.animate)
				userSwitch.restart();
			else
				look.shownUser = greeter.user;
		}

		function onArmedChanged() {
			if (greeter.armed && look.animate)
				armCountdown.restart();
		}
	}

	// A word under the field that a lock key is on, fading and dropping
	// in while `on`.
	component LockNote: Text {
		id: lockNote

		property bool on
		property real shown: on ? 1 : 0

		visible: shown > 0
		font.pixelSize: greeter.u(13)
		font.weight: Font.Medium
		font.family: greeter.fontFamily
		opacity: shown
		transform: Translate {
			y: (1 - lockNote.shown) * -greeter.u(4)
		}

		Behavior on shown {
			enabled: look.animate

			NumberAnimation {
				duration: 180
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
	// doesn't shift as the minutes go. Above the form, or at the top of
	// the screen (layout.clock).
	Column {
		readonly property bool atTop: look.layout.clock === "top"

		x: atTop ? look.area.x + (look.area.width - width) / 2 : form.x + (form.width - width) / 2
		y: atTop ? look.area.y + greeter.u(72) : form.y - height - greeter.u(72)
		visible: look.layout.clock !== "none"
		spacing: greeter.u(2)
		opacity: look.fade(0)
		transform: Translate {
			y: look.lift(0)
		}

		// The time, a character to an item, so that one that changes rolls
		// up: the old one fading out upwards, the new one in from below.
		Row {
			id: time

			readonly property string text: greeter.config.clockFormat ? greeter.now.toLocaleTimeString(greeter.locale, greeter.config.clockFormat)
				: greeter.now.toLocaleTimeString(greeter.locale, Locale.ShortFormat)

			anchors.horizontalCenter: parent.horizontalCenter
			spacing: -greeter.u(3)

			Repeater {
				model: time.text.length

				Item {
					id: glyph

					required property int index
					readonly property string character: time.text.charAt(index)
					property string was: ""
					property real roll: 0

					width: current.implicitWidth
					height: current.implicitHeight
					Component.onCompleted: was = character
					onCharacterChanged: {
						previous.text = was;
						was = character;
						if (look.animate)
							rollAnim.restart();
					}

					Text {
						id: current

						y: glyph.roll * glyph.height * 0.3
						opacity: 1 - glyph.roll
						text: glyph.character
						color: greeter.colors.text
						font.pixelSize: greeter.u(120)
						font.weight: Font.Light
						font.features: { "tnum": 1 }
						font.family: greeter.fontFamily
					}

					Text {
						id: previous

						y: (glyph.roll - 1) * glyph.height * 0.3
						opacity: glyph.roll
						visible: glyph.roll > 0
						color: current.color
						font: current.font
					}

					NumberAnimation {
						id: rollAnim

						target: glyph
						property: "roll"
						from: 1
						to: 0
						duration: 500
						easing.type: Easing.OutCubic
					}
				}
			}
		}

		// The theme's dateFormat, else the locale's long date without the
		// year.
		Text {
			anchors.horizontalCenter: parent.horizontalCenter
			text: greeter.now.toLocaleDateString(greeter.locale, greeter.config.dateFormat
				|| greeter.locale.dateFormat(Locale.LongFormat).replace(/[\s,.]*y+[^dM]*/, "")).toUpperCase()
			color: greeter.colors.subtext
			font.pixelSize: greeter.u(15)
			font.weight: Font.Medium
			font.letterSpacing: greeter.u(3)
			font.family: greeter.fontFamily
		}
	}

	// In a top corner (layout.status), what the theme's `status` lists:
	// "hostname", "battery".
	Row {
		readonly property var shows: greeter.config.status || []

		x: look.layout.status === "top-left" ? look.area.x + greeter.u(28) : look.area.x + look.area.width - width - greeter.u(28)
		y: look.area.y + greeter.u(24)
		spacing: greeter.u(18)
		opacity: look.fade(0)
		transform: Translate {
			y: look.lift(0)
		}

		Text {
			anchors.verticalCenter: parent.verticalCenter
			visible: parent.shows.includes("hostname") && text !== ""
			text: greeter.hostname
			color: greeter.colors.subtext
			font.pixelSize: greeter.u(13)
			font.weight: Font.Medium
			font.family: greeter.fontFamily
		}

		// The battery: a gauge, a bolt while it's plugged in, the level;
		// in the error color when low and not plugged in.
		Row {
			readonly property var battery: greeter.battery
			readonly property color tint: battery && battery.level <= 15 && !battery.plugged ? greeter.colors.error : greeter.colors.subtext

			anchors.verticalCenter: parent.verticalCenter
			visible: parent.shows.includes("battery") && battery !== null
			spacing: greeter.u(6)

			Icon {
				anchors.verticalCenter: parent.verticalCenter
				visible: parent.battery !== null && parent.battery.plugged
				path: look.icons.bolt
				size: greeter.u(14)
				color: parent.tint
			}

			Item {
				anchors.verticalCenter: parent.verticalCenter
				width: greeter.u(26)
				height: greeter.u(13)

				Rectangle {
					id: cell

					width: parent.width - greeter.u(3)
					height: parent.height
					radius: greeter.u(3)
					color: "transparent"
					border.width: Math.max(1, greeter.u(1.5))
					border.color: parent.parent.tint
				}

				Rectangle {
					x: cell.width + greeter.u(1)
					anchors.verticalCenter: parent.verticalCenter
					width: greeter.u(2)
					height: greeter.u(5)
					radius: greeter.u(1)
					color: parent.parent.tint
				}

				Rectangle {
					readonly property real inset: cell.border.width + greeter.u(1.5)

					x: inset
					y: inset
					width: Math.max(greeter.u(2), (cell.width - inset * 2) * (parent.parent.battery ? parent.parent.battery.level : 0) / 100)
					height: cell.height - inset * 2
					radius: greeter.u(1)
					color: parent.parent.tint
				}
			}

			Text {
				anchors.verticalCenter: parent.verticalCenter
				text: parent.battery ? `${parent.battery.level}%` : ""
				color: parent.tint
				font.pixelSize: greeter.u(13)
				font.weight: Font.Medium
				font.features: { "tnum": 1 }
				font.family: greeter.fontFamily
			}
		}
	}

	// In the middle of the screen, or to one side (layout.form); a little
	// low when the clock is above it.
	Column {
		id: form

		readonly property real side: Math.max(greeter.u(64), look.area.width * 0.1)

		x: look.layout.form === "left" ? look.area.x + side
			: look.layout.form === "right" ? look.area.x + look.area.width - width - side
			: look.area.x + (look.area.width - width) / 2
		y: look.area.y + (look.area.height - height) / 2 + (look.layout.clock === "above" ? greeter.u(40) : 0)
		width: greeter.u(greeter.config.fieldWidth || 340)
		spacing: greeter.u(14)

		// The user's picture, or else their initial, in a ring of the
		// accent (of the name typed, while one is); a click picks another
		// user, like one on the name. None without a user list.
		Rectangle {
			id: avatar

			readonly property bool hovered: avatarMouse.containsMouse && greeter.canPick("user")

			anchors.horizontalCenter: parent.horizontalCenter
			visible: greeter.userList
			width: greeter.u(92)
			height: width
			radius: look.round(width)
			color: Qt.alpha(greeter.colors.accent, 0.14)
			scale: hovered ? 1.04 : 1
			opacity: look.fade(1) * look.userFade
			transform: Translate {
				x: look.userShift
				y: look.lift(1)
			}

			Behavior on scale {
				enabled: look.animate

				NumberAnimation {
					duration: 150
					easing.type: Easing.OutCubic
				}
			}

			Image {
				id: picture

				anchors.fill: parent
				visible: false
				source: !look.askingName && look.shownUser && look.shownUser.icon ? "file://" + look.shownUser.icon.split("/").map(encodeURIComponent).join("/") : ""
				sourceSize: Qt.size(width * greeter.dpr, height * greeter.dpr)
				fillMode: Image.PreserveAspectCrop
				asynchronous: true
			}

			// The picture, cut round.
			MultiEffect {
				anchors.fill: parent
				visible: picture.status === Image.Ready
				source: picture
				maskEnabled: true
				maskSource: round
				maskThresholdMin: 0.5
				maskSpreadAtMin: 1
			}

			Item {
				id: round

				anchors.fill: parent
				visible: false
				layer.enabled: true

				Rectangle {
					anchors.fill: parent
					radius: look.round(width)
				}
			}

			Text {
				anchors.centerIn: parent
				visible: picture.status !== Image.Ready
				text: (look.askingName ? input.text.trim() : look.shownUser ? look.shownUser.name : "").charAt(0).toUpperCase()
				color: greeter.colors.accent
				font.pixelSize: greeter.u(38)
				font.weight: Font.DemiBold
				font.family: greeter.fontFamily
			}

			// The ring, over the picture's edge.
			Rectangle {
				anchors.fill: parent
				radius: look.round(width)
				color: "transparent"
				border.width: greeter.u(2)
				border.color: Qt.alpha(greeter.colors.accent, avatar.hovered ? 1 : 0.55)

				Behavior on border.color {
					enabled: look.animate

					ColorAnimation {
						duration: 150
					}
				}
			}

			MouseArea {
				id: avatarMouse

				anchors.fill: parent
				hoverEnabled: true
				cursorShape: greeter.canPick("user") ? Qt.PointingHandCursor : Qt.ArrowCursor
				onClicked: look.userClicked()
			}
		}

		// The user, and the session (in a pill) when there's more than
		// one: a click (or F2, F3) opens the list to pick another from, or
		// without a user list, types the name again. Hidden while a name is
		// typed.
		Repeater {
			id: labels

			model: ["user", "session"]

			Item {
				id: label

				required property string modelData
				readonly property bool isUser: modelData === "user"
				readonly property bool hovered: mouse.containsMouse && (isUser && !greeter.userList ? greeter.canEnterUser() : greeter.canPick(modelData))

				width: parent.width
				height: isUser ? name.height : greeter.u(30)
				visible: isUser || greeter.sessions.length > 1
				opacity: look.fade(2) * (isUser ? look.userFade * (look.askingName ? 0 : 1) : 1)
				transform: Translate {
					x: label.isUser ? look.userShift : 0
					y: look.lift(2)
				}

				Rectangle {
					anchors.centerIn: row
					width: row.width + greeter.u(24)
					height: greeter.u(30)
					radius: look.round(height)
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
						text: label.isUser ? (look.shownUser ? look.shownUser.name : "") : greeter.sessionName(greeter.session)
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
					enabled: !(label.isUser && look.askingName)
					onClicked: label.isUser ? look.userClicked() : look.togglePicker("session")
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
			readonly property bool error: input.text === "" && !look.enteringName && (greeter.auth.messageIsError || greeter.problem !== "")
			readonly property bool busy: greeter.auth.phase === "verifying" || greeter.auth.phase === "launching"

			width: parent.width
			height: greeter.u(50)
			radius: look.round(height)
			color: greeter.colors.field
			border.width: greeter.u(2)
			border.color: error ? greeter.colors.error
				: look.typing && input.activeFocus ? Qt.alpha(greeter.colors.accent, input.text ? 0.9 : 0.4)
				: Qt.alpha(greeter.colors.text, 0.08)
			opacity: look.fade(3)
			// A shake, dying down as look.shaken goes from 0 to 1.
			transform: Translate {
				x: Math.sin(look.shaken * Math.PI * 6) * (1 - look.shaken) * greeter.u(10)
				y: look.lift(3)
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
				opacity: look.typing ? 1 : 0.5
				focus: true
				readOnly: !look.typing
				echoMode: look.secret ? TextInput.Password : TextInput.Normal
				// A secret is drawn by `dots` below: its own echo, cursor
				// (in this color too) and selection stay hidden, and the
				// cursor stays at the end.
				color: look.secret ? "transparent" : greeter.colors.text
				selectionColor: look.secret ? "transparent" : Qt.alpha(greeter.colors.accent, 0.3)
				font.pixelSize: greeter.u(16)
				font.family: greeter.fontFamily
				clip: true
				onAccepted: look.submit()
				onCursorPositionChanged: if (look.secret && cursorPosition !== text.length)
					cursorPosition = text.length
				onTextChanged: {
					const count = look.secret ? text.length : 0;
					while (dotModel.count > count)
						dotModel.remove(dotModel.count - 1);
					while (dotModel.count < count)
						dotModel.append({});
				}
				Keys.onPressed: event => look.keyPressed(event)

				// One per character, kept as the text grows, so that only
				// a new one pops up.
				ListModel {
					id: dotModel
				}

				// The blink of the cursor after the dots.
				Timer {
					id: blink

					property bool on: true

					interval: 530
					running: input.activeFocus && look.secret && look.typing && !greeter.idle
					repeat: true
					onRunningChanged: on = true
					onTriggered: on = !on
				}
			}

			// The secret: a dot for each character, the last ones in view,
			// and the cursor after them.
			Item {
				anchors.fill: input
				visible: look.secret
				clip: true

				Row {
					id: dots

					x: Math.min(0, parent.width - width - greeter.u(6))
					anchors.verticalCenter: parent.verticalCenter
					spacing: greeter.u(7)

					Repeater {
						model: dotModel

						Rectangle {
							width: greeter.u(8)
							height: width
							radius: width / 2
							color: greeter.colors.text

							NumberAnimation on scale {
								running: look.animate
								from: 0.2
								to: 1
								duration: 180
								easing.type: Easing.OutBack
							}
						}
					}
				}

				Rectangle {
					x: dots.x + dots.width + (dotModel.count ? greeter.u(6) : 0)
					anchors.verticalCenter: parent.verticalCenter
					width: Math.max(1, greeter.u(1.5))
					height: greeter.u(20)
					visible: input.activeFocus && look.typing && blink.on
					color: greeter.colors.text
				}
			}

			// PAM's question ("Username" while a name is typed), or its last
			// message (a failed attempt included) until something is typed;
			// a message too long for the field goes under it (`note`).
			Text {
				id: placeholder

				readonly property bool messageFits: messageWidth.advanceWidth <= input.width
				// PAM's last message, unless the field takes a name after
				// "Other…".
				readonly property bool showsMessage: !greeter.problem && !look.enteringName && messageFits && greeter.auth.message !== ""

				anchors.fill: input
				verticalAlignment: Text.AlignVCenter
				visible: input.text === ""
				text: greeter.problem || (showsMessage ? greeter.auth.message : look.askingName ? greeter.tr("Username")
					: greeter.auth.prompt.replace(/:\s*$/, ""))
				color: greeter.problem || showsMessage && greeter.auth.messageIsError ? greeter.colors.error : greeter.colors.dim
				font.pixelSize: greeter.u(16)
				font.family: greeter.fontFamily
				elide: Text.ElideRight

				TextMetrics {
					id: messageWidth

					font: placeholder.font
					text: greeter.auth.message
				}
			}

			Rectangle {
				id: button

				readonly property bool ready: input.text !== "" && look.typing

				anchors.right: parent.right
				anchors.rightMargin: greeter.u(7)
				anchors.verticalCenter: parent.verticalCenter
				width: greeter.u(36)
				height: width
				radius: look.round(width, greeter.u(7))
				color: ready ? greeter.colors.accent : Qt.alpha(greeter.colors.text, 0.06)
				// It grows under the pointer and gives under a press, once
				// there's something to send.
				scale: !ready ? 1 : buttonMouse.pressed ? 0.9 : buttonMouse.containsMouse ? 1.08 : 1

				Behavior on color {
					enabled: look.animate

					ColorAnimation {
						duration: 150
					}
				}

				Behavior on scale {
					enabled: look.animate

					NumberAnimation {
						duration: 150
						easing.type: Easing.OutCubic
					}
				}

				// The arrow: it comes up as something is typed, leans
				// forward under the pointer, and gives way to the spinner.
				Icon {
					anchors.centerIn: parent
					path: look.icons.arrow
					size: greeter.u(18)
					color: button.ready ? greeter.colors.background : greeter.colors.dim
					opacity: field.busy ? 0 : 1
					scale: button.ready ? 1 : 0.8
					transform: Translate {
						x: button.ready && buttonMouse.containsMouse ? greeter.u(2) : 0

						Behavior on x {
							enabled: look.animate

							NumberAnimation {
								duration: 150
								easing.type: Easing.OutCubic
							}
						}
					}

					Behavior on color {
						enabled: look.animate

						ColorAnimation {
							duration: 150
						}
					}

					Behavior on opacity {
						enabled: look.animate

						NumberAnimation {
							duration: 150
						}
					}

					Behavior on scale {
						enabled: look.animate

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
					width: greeter.u(18)
					height: width
					visible: opacity > 0
					opacity: field.busy ? 1 : 0
					preferredRendererType: Shape.CurveRenderer

					Behavior on opacity {
						enabled: look.animate

						NumberAnimation {
							duration: 150
						}
					}

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
						running: spinner.visible && look.animate
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
						look.submit()
				}
			}
		}

		// A line under the field, kept so that the form doesn't move: a
		// message too long for the field (onto more lines below, if it
		// must), else Caps Lock's warning and Num Lock.
		Item {
			width: parent.width
			height: caps.implicitHeight
			opacity: look.fade(3)
			transform: Translate {
				y: look.lift(3)
			}

			Text {
				id: note

				readonly property bool shown: greeter.auth.message !== "" && !placeholder.messageFits

				width: parent.width
				horizontalAlignment: Text.AlignHCenter
				visible: shown
				text: shown ? greeter.auth.message : ""
				color: greeter.auth.messageIsError ? greeter.colors.error : greeter.colors.subtext
				font.pixelSize: greeter.u(13)
				font.weight: Font.Medium
				font.family: greeter.fontFamily
				wrapMode: Text.Wrap
			}

			// Caps Lock (a warning) and Num Lock, side by side, each
			// fading in and out on its own.
			Row {
				anchors.horizontalCenter: parent.horizontalCenter
				spacing: greeter.u(16)

				LockNote {
					id: caps

					on: greeter.capsLock && !note.shown
					text: greeter.tr("Caps Lock is on")
					color: greeter.colors.error
				}

				LockNote {
					on: greeter.numLock && !note.shown
					text: greeter.tr("Num Lock is on")
					color: greeter.colors.subtext
				}
			}
		}
	}

	// Power: round buttons with icons, at the bottom or in its right
	// corner (layout.power). Above them, what the one under the pointer
	// does, or that an armed one wants a second click.
	Column {
		readonly property bool corner: look.layout.power === "corner"

		x: corner ? look.area.x + look.area.width - width - greeter.u(32) : look.area.x + (look.area.width - width) / 2
		y: look.area.y + look.area.height - height - greeter.u(corner ? 32 : 40)
		visible: look.layout.power !== "none"
		spacing: greeter.u(10)
		opacity: look.fade(4)
		transform: Translate {
			y: look.lift(4)
		}

		// It fades in and out, and keeps its last words while it fades
		// out.
		Text {
			readonly property string says: greeter.powerError || (greeter.armed ? greeter.tr({ suspend: "Click again to suspend", reboot: "Click again to reboot", poweroff: "Click again to power off" }[greeter.armed] || greeter.powerLabels[greeter.armed])
				: greeter.powerLabels[powerRow.hovered] || "")
			property string said: ""

			anchors.horizontalCenter: parent.horizontalCenter
			text: says || said
			opacity: says ? 1 : 0
			color: greeter.powerError ? greeter.colors.error : greeter.armed ? greeter.colors.accent : greeter.colors.subtext
			font.pixelSize: greeter.u(13)
			font.weight: Font.Medium
			font.family: greeter.fontFamily
			onSaysChanged: if (says)
				said = says

			Behavior on opacity {
				enabled: look.animate

				NumberAnimation {
					duration: 150
				}
			}
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
					radius: look.round(width)
					color: armed ? greeter.colors.accent : Qt.alpha(greeter.colors.text, powerMouse.containsMouse ? 0.1 : 0.04)
					scale: powerMouse.pressed ? 0.92 : powerMouse.containsMouse ? 1.08 : 1

					Behavior on color {
						enabled: look.animate

						ColorAnimation {
							duration: 120
						}
					}

					Behavior on scale {
						enabled: look.animate

						NumberAnimation {
							duration: 150
							easing.type: Easing.OutCubic
						}
					}

					Icon {
						anchors.centerIn: parent
						path: look.icons[power.modelData] || look.icons.poweroff
						size: greeter.u(20)
						color: power.armed ? greeter.colors.background : powerMouse.containsMouse ? greeter.colors.text : greeter.colors.subtext
					}

					// Armed, a ring around it runs out as the time for the
					// second click does.
					Shape {
						id: countdown

						anchors.centerIn: parent
						width: parent.width + greeter.u(12)
						height: width
						visible: power.armed && look.animate
						preferredRendererType: Shape.CurveRenderer

						ShapePath {
							strokeColor: greeter.colors.accent
							strokeWidth: greeter.u(2)
							fillColor: "transparent"
							capStyle: ShapePath.RoundCap

							PathAngleArc {
								centerX: countdown.width / 2
								centerY: countdown.height / 2
								radiusX: countdown.width / 2 - greeter.u(1)
								radiusY: radiusX
								startAngle: -90
								sweepAngle: 360 * look.armLeft
							}
						}
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
		radius: look.radius < 0 ? greeter.u(16) : look.radius
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
			// Under the entry the keys or the pointer are on, sliding from
			// one to the next.
			highlight: Rectangle {
				radius: Math.max(0, popup.radius - greeter.u(6))
				color: Qt.alpha(greeter.colors.accent, 0.15)
			}
			highlightMoveDuration: look.animate ? 120 : 0
			highlightMoveVelocity: -1
			highlightResizeDuration: 0
			Keys.onPressed: event => look.keyPressed(event)

			delegate: Item {
				required property var modelData
				required property int index

				width: ListView.view.width
				height: greeter.u(40)


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
					color: parent.modelData.other ? greeter.colors.subtext : greeter.colors.text
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

	// After the form has begun to leave; the core gives it 450 ms.
	SequentialAnimation {
		id: fadeOut

		PauseAnimation {
			duration: 120
		}

		NumberAnimation {
			target: curtain
			property: "opacity"
			to: 1
			duration: 300
			easing.type: Easing.InQuad
		}
	}

	NumberAnimation {
		id: introAnim

		target: look
		property: "intro"
		from: 0
		to: 1
		duration: 4 * 70 + 450
	}

	NumberAnimation {
		id: outroAnim

		target: look
		property: "outro"
		to: 1
		duration: 350
		easing.type: Easing.InOutQuad
	}

	SequentialAnimation {
		id: userSwitch

		ParallelAnimation {
			NumberAnimation {
				target: look
				property: "userFade"
				to: 0
				duration: 120
			}

			NumberAnimation {
				target: look
				property: "userShift"
				to: -greeter.u(24)
				duration: 120
				easing.type: Easing.InCubic
			}
		}

		ScriptAction {
			script: {
				look.shownUser = greeter.user;
				look.userShift = greeter.u(24);
			}
		}

		ParallelAnimation {
			NumberAnimation {
				target: look
				property: "userFade"
				to: 1
				duration: 180
			}

			NumberAnimation {
				target: look
				property: "userShift"
				to: 0
				duration: 220
				easing.type: Easing.OutCubic
			}
		}
	}

	NumberAnimation {
		id: armCountdown

		target: look
		property: "armLeft"
		from: 1
		to: 0
		duration: greeter.armedFor
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
