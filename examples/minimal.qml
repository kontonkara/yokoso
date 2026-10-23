import QtQuick

// A look of one's own, as small as a working one gets: the time, the user,
// the session, a password field, and dark while idle. The theme next to it
// (minimal.json) points at it with "look"; `greeter` is yokoso's core (see
// the README for what it has).
Item {
	id: look

	required property var greeter

	// Each key keeps the screen awake, or wakes it and goes no further.
	// F2 and F3 go on to the next user and the next session.
	function keyPressed(event) {
		event.accepted = true;
		if (greeter.wake())
			return;
		const next = (list, current) => list[(list.indexOf(current) + 1) % list.length];
		if (event.key === Qt.Key_F2 && greeter.canPick("user"))
			greeter.pickUser(next(greeter.users, greeter.user));
		else if (event.key === Qt.Key_F3 && greeter.canPick("session"))
			greeter.pickSession(next(greeter.sessions, greeter.session));
		else
			event.accepted = false;
	}

	Column {
		x: greeter.area.x + (greeter.area.width - width) / 2
		y: greeter.area.y + (greeter.area.height - height) / 2
		width: greeter.u(320)
		spacing: greeter.u(12)

		Text {
			width: parent.width
			horizontalAlignment: Text.AlignHCenter
			text: greeter.now.toLocaleTimeString(greeter.locale, Locale.ShortFormat)
			color: greeter.colors.text
			font.pixelSize: greeter.u(48)
			font.family: greeter.fontFamily
		}

		Text {
			width: parent.width
			horizontalAlignment: Text.AlignHCenter
			text: `${greeter.user ? greeter.user.name : ""} · ${greeter.sessionName(greeter.session)}`
			color: greeter.colors.subtext
			font.pixelSize: greeter.u(16)
			font.family: greeter.fontFamily
		}

		Rectangle {
			width: parent.width
			height: greeter.u(40)
			color: greeter.colors.field
			border.width: 1
			border.color: greeter.auth.messageIsError ? greeter.colors.error : greeter.colors.dim

			TextInput {
				id: input

				anchors.fill: parent
				anchors.margins: greeter.u(10)
				verticalAlignment: TextInput.AlignVCenter
				focus: true
				readOnly: !greeter.needsUser && greeter.auth.phase !== "prompt"
				echoMode: !greeter.needsUser && greeter.auth.secret ? TextInput.Password : TextInput.Normal
				color: greeter.colors.text
				font.pixelSize: greeter.u(16)
				font.family: greeter.fontFamily
				onAccepted: {
					if (greeter.needsUser)
						greeter.enterUser(text);
					else
						greeter.auth.submit(text);
					text = "";
				}
				Keys.onPressed: event => look.keyPressed(event)
			}
		}

		// PAM's question or its last message (or that a name is wanted),
		// and why a login can't start.
		Text {
			width: parent.width
			horizontalAlignment: Text.AlignHCenter
			text: greeter.problem || greeter.auth.message || (greeter.needsUser ? greeter.tr("Username") : greeter.auth.prompt)
			color: greeter.problem || greeter.auth.messageIsError ? greeter.colors.error : greeter.colors.dim
			font.pixelSize: greeter.u(14)
			font.family: greeter.fontFamily
			wrapMode: Text.Wrap
		}
	}

	// Idle: dark, and a click only wakes it.
	Rectangle {
		anchors.fill: parent
		visible: greeter.idle
		color: "black"

		MouseArea {
			anchors.fill: parent
			onPressed: greeter.wake()
		}
	}
}
