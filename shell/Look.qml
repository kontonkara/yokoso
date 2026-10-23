import QtQuick
import yokoso

// The built-in look, made of yokoso's parts (shell/yokoso): the clock, the
// form and the power buttons over the theme's background, laid out as the
// theme's layout says, coming in one after another and going as the
// session starts. Everything it shows and does comes from `greeter`, the
// core in shell.qml.
Item {
	id: look

	required property var greeter

	readonly property bool animate: greeter.animate
	readonly property var layout: greeter.config.layout || {}

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

	Form {
		id: loginForm
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
			if (look.animate && greeter.auth.phase === "launching") {
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
	}

	Background {
		anchors.fill: parent
	}

	// Above the form, or at the top of the screen (layout.clock).
	Clock {
		readonly property bool atTop: look.layout.clock === "top"

		x: atTop ? look.area.x + (look.area.width - width) / 2 : column.x + (column.width - width) / 2
		y: atTop ? look.area.y + greeter.u(72) : column.y - height - greeter.u(72)
		visible: look.layout.clock !== "none"
		opacity: look.fade(0)
		transform: Translate {
			y: look.lift(0)
		}
	}

	// In a top corner (layout.status).
	Status {
		x: look.layout.status === "top-left" ? look.area.x + greeter.u(28) : look.area.x + look.area.width - width - greeter.u(28)
		y: look.area.y + greeter.u(24)
		opacity: look.fade(0)
		transform: Translate {
			y: look.lift(0)
		}
	}

	// In the middle of the screen, or to one side (layout.form); a little
	// low when the clock is above it.
	Column {
		id: column

		readonly property real side: Math.max(greeter.u(64), look.area.width * 0.1)

		x: look.layout.form === "left" ? look.area.x + side
			: look.layout.form === "right" ? look.area.x + look.area.width - width - side
			: look.area.x + (look.area.width - width) / 2
		y: look.area.y + (look.area.height - height) / 2 + (look.layout.clock === "above" ? greeter.u(40) : 0)
		width: greeter.u(greeter.config.fieldWidth || 340)
		spacing: greeter.u(14)

		// None without a user list.
		Avatar {
			anchors.horizontalCenter: parent.horizontalCenter
			visible: greeter.userList
			form: loginForm
			user: look.shownUser
			opacity: look.fade(1) * look.userFade
			transform: Translate {
				x: look.userShift
				y: look.lift(1)
			}
		}

		UserName {
			id: userName

			width: parent.width
			form: loginForm
			user: look.shownUser
			opacity: look.fade(2) * look.userFade
			transform: Translate {
				x: look.userShift
				y: look.lift(2)
			}
		}

		SessionPill {
			id: sessionPill

			width: parent.width
			form: loginForm
			opacity: look.fade(2)
			transform: Translate {
				y: look.lift(2)
			}
		}

		Item {
			width: 1
			height: greeter.u(4)
		}

		PasswordField {
			width: parent.width
			form: loginForm
			opacity: look.fade(3)
			transform: Translate {
				y: look.lift(3)
			}
		}

		FieldNote {
			width: parent.width
			form: loginForm
			opacity: look.fade(3)
			transform: Translate {
				y: look.lift(3)
			}
		}
	}

	// At the bottom, or in its right corner (layout.power).
	PowerBar {
		readonly property bool corner: look.layout.power === "corner"

		x: corner ? look.area.x + look.area.width - width - greeter.u(32) : look.area.x + (look.area.width - width) / 2
		y: look.area.y + look.area.height - height - greeter.u(corner ? 32 : 40)
		visible: look.layout.power !== "none"
		opacity: look.fade(4)
		transform: Translate {
			y: look.lift(4)
		}
	}

	Picker {
		anchors.fill: parent
		form: loginForm
		under: loginForm.picking === "session" ? sessionPill : userName
	}

	IdleCover {
		anchors.fill: parent
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
}
