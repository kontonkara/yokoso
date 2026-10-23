import QtQuick

// The line under the field, one line high so that the form doesn't move: a
// message of PAM's too long for the field (onto more lines below, if it
// must), else Caps Lock's warning and that Num Lock is on, side by side.
Item {
	id: root

	property Form form: null

	implicitHeight: caps.implicitHeight

	Text {
		id: message

		readonly property bool shown: Greeter.core.auth.message !== "" && root.form !== null && !root.form.messageFits

		width: parent.width
		horizontalAlignment: Text.AlignHCenter
		visible: shown
		text: shown ? Greeter.core.auth.message : ""
		color: Greeter.core.auth.messageIsError ? Greeter.colors.error : Greeter.colors.subtext
		font.pixelSize: Greeter.u(13)
		font.weight: Font.Medium
		font.family: Greeter.font
		wrapMode: Text.Wrap
	}

	Row {
		anchors.horizontalCenter: parent.horizontalCenter
		spacing: Greeter.u(16)

		LockNote {
			id: caps

			on: Greeter.core.capsLock && !message.shown
			text: Greeter.core.tr("Caps Lock is on")
			color: Greeter.colors.error
		}

		LockNote {
			on: Greeter.core.numLock && !message.shown
			text: Greeter.core.tr("Num Lock is on")
			color: Greeter.colors.subtext
		}
	}
}
