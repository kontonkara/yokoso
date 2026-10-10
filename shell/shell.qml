import QtQml.Models
import QtQuick
import Quickshell
import Quickshell.Io

ShellRoot {
	id: root

	readonly property string mode: Quickshell.env("YOKOSO_MODE") || "greeter"

	function parse(text) {
		try {
			return JSON.parse(text) || {};
		} catch (e) {
			return {};
		}
	}

	FileView {
		id: envFile

		path: Quickshell.env("YOKOSO_ENV") || ""
		blockLoading: true
	}

	FileView {
		id: configFileView

		path: Quickshell.env("YOKOSO_CONFIG") || ""
		blockLoading: true
		printErrors: false
	}

	readonly property var env: parse(envFile.text())

	// Every config key, with its default, over the theme the launcher put
	// together (themes/dark.json has its keys); colors merge key by key.
	readonly property var configFile: parse(configFileView.text())
	readonly property var theme: env.theme || {}
	readonly property var config: Object.assign({
		user: "",
		session: "",
		// Start with the user and the session of the last login.
		remember: true,
		// "dark", "light", a theme's name or a theme file's path.
		theme: "dark",
		// The output with the form ("DP-1"), or "cursor" for the one the
		// pointer is on; "" picks one, see screen below.
		output: "",
		// 0: from the screen's physical size.
		scale: 0,
		// What starts an X server for an X11 session, with the session's
		// command after it.
		x11Wrapper: "startx /usr/bin/env",
	}, theme, configFile, {
		colors: Object.assign({}, theme.colors, configFile.colors),
	})
	readonly property var colors: config.colors

	// config.background's image for a screen (null: the window), as a URL:
	// one path for every screen, or one per output, "*" for the rest.
	function backgroundFor(screen) {
		const bg = config.background;
		const path = typeof bg === "string" ? bg : bg && (screen && bg[screen.name] || bg["*"]) || "";
		return path ? "file://" + path.split("/").map(encodeURIComponent).join("/") : "";
	}
	readonly property string fontFamily: config.font || Qt.application.font.family

	// The lid, read every second: closing it moves the form to another screen.
	property bool lidClosed: false

	FileView {
		id: lidFile

		path: root.env.lid || ""
		blockLoading: true
		printErrors: false
		onLoaded: root.lidClosed = text().indexOf("closed") >= 0
	}

	Timer {
		interval: 1000
		running: lidFile.path !== ""
		repeat: true
		onTriggered: lidFile.reload()
	}

	// The screen with the form: the one the pointer last moved onto (with
	// output "cursor", and never a closed lid's panel), or config.output;
	// else a laptop's own panel, or with its lid closed an external screen;
	// else the first one.
	property var pointerScreen: null

	readonly property var screen: {
		const screens = Quickshell.screens;
		const internal = s => /^(eDP|LVDS|DSI)-/.test(s.name);
		const pointed = screens.includes(pointerScreen) && !(lidClosed && internal(pointerScreen));
		return (pointed ? pointerScreen : null)
			|| screens.find(s => s.name === config.output)
			|| (lidClosed ? screens.find(s => !internal(s)) : screens.find(internal))
			|| screens[0] || null;
	}

	// The window's top-left corner among the screens: cage spreads it over
	// all of them.
	readonly property point origin: Qt.point(Math.min(...Quickshell.screens.map(s => s.x)),
		Math.min(...Quickshell.screens.map(s => s.y)))

	// Where cage put the pointer doesn't count, only a move does.
	property var pointerStart: null

	function pointAt(p) {
		if (!pointerStart)
			pointerStart = Qt.point(p.x, p.y);
		if (p.x === pointerStart.x && p.y === pointerStart.y)
			return;
		const s = Quickshell.screens.find(s => p.x >= s.x - origin.x && p.x < s.x - origin.x + s.width
			&& p.y >= s.y - origin.y && p.y < s.y - origin.y + s.height);
		if (s && s !== pointerScreen)
			pointerScreen = s;
	}

	// Where the form's screen is in the window. In preview (and a one-screen
	// shot) the window stands for it.
	readonly property bool windowIsScreen: mode === "preview" || (mode === "shot" && Quickshell.screens.length < 2)
	readonly property rect area: {
		if (windowIsScreen || !screen)
			return Qt.rect(0, 0, content.width, content.height);
		return Qt.rect(screen.x - origin.x, screen.y - origin.y, screen.width, screen.height);
	}

	// Who logged in last, and each user's last session, as { user, sessions:
	// { <user>: { id, type } } }: written at a login, in /var/lib/yokoso.
	FileView {
		id: stateFile

		path: root.config.remember ? Quickshell.env("YOKOSO_STATE") || "" : ""
		blockLoading: true
		blockWrites: true
		printErrors: false
		onSaveFailed: console.warn(`yokoso: can't write ${path}`)
	}

	readonly property var remembered: parse(stateFile.text())

	function remember() {
		if (!stateFile.path || !user || !session)
			return;
		const last = Object.assign({}, remembered.sessions, {
			[user.name]: { id: session.id, type: session.type },
		});
		stateFile.setText(JSON.stringify({ user: user.name, sessions: last }) + "\n");
	}

	// The users to pick from, config.user too when it isn't listed (LDAP
	// accounts often aren't).
	readonly property var users: {
		const listed = env.users || [];
		const name = config.user;
		return !name || listed.some(u => u.name === name) ? listed : [{ name, realName: name }].concat(listed);
	}
	readonly property var sessions: env.sessions || []

	// The ones picked here, else the last login's, else config's, else the
	// first. Picking a user brings back their last session.
	property string pickedUser: ""
	property var pickedSession: null
	readonly property var user: [pickedUser, remembered.user, config.user]
		.map(name => users.find(u => u.name === name)).find(u => u) || users[0] || null
	readonly property var session: {
		const last = user && remembered.sessions && remembered.sessions[user.name] || {};
		return pickedSession
			|| sessions.find(s => s.id === last.id && s.type === last.type)
			|| sessions.find(s => s.id === config.session || s.id === `${config.session}.desktop`)
			|| sessions[0] || null;
	}

	// A session's name, with "(X11)" when a Wayland one has the same.
	function sessionName(s) {
		if (!s)
			return "";
		return s.type === "x11" && sessions.some(o => o !== s && o.name === s.name) ? `${s.name} (X11)` : s.name;
	}

	// The list open under the user's or the session's name: "user",
	// "session" or "".
	property string picking: ""
	readonly property var pickList: picking === "user"
		? users.map(u => ({ label: u.name, detail: u.realName !== u.name ? u.realName : "", current: u === user }))
		: picking === "session"
		? sessions.map(s => ({ label: sessionName(s), detail: "", current: s === session }))
		: []

	// Not while a password is checked (another user) or a session starts.
	function canPick(which) {
		return (which === "user" ? users : sessions).length > 1 && auth.phase !== "launching"
			&& !(which === "user" && auth.phase === "verifying");
	}

	function togglePicker(which) {
		if (picking === which || !canPick(which)) {
			closePicker();
			return;
		}
		picking = which;
		list.currentIndex = pickList.findIndex(item => item.current);
		list.forceActiveFocus();
	}

	function closePicker() {
		picking = "";
		input.forceActiveFocus();
	}

	function pick(index) {
		if (picking === "user" && users[index] !== user) {
			pickedUser = users[index].name;
			pickedSession = null;
			auth.restart();
		} else if (picking === "session") {
			pickedSession = sessions[index];
		}
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

	// Why a login can't start, if it can't.
	readonly property string problem: !user ? "No user found" : !session ? "No session found" : ""

	// The UI scale: config.scale, else the screen's DPI / 96 rounded to 0.05
	// (1 when its size is unknown), over what the compositor already scales.
	readonly property var shotSize: (Quickshell.env("YOKOSO_SHOT_SIZE") || "2560x1440").split("x").map(Number)
	readonly property real dpr: mode === "shot" || !screen ? 1 : screen.devicePixelRatio
	readonly property real uiScale: {
		if (config.scale > 0)
			return config.scale / dpr;
		const known = env.screens || [];
		const info = known.find(s => screen && s.name === screen.name) || (mode !== "greeter" ? known[0] : null);
		const pixels = mode === "shot" ? shotSize[0] : (screen ? screen.width * dpr : 0);
		if (!info || !info.widthMm || !pixels)
			return 1;
		const scale = Math.round(pixels / (info.widthMm / 25.4) / 96 / 0.05) * 0.05;
		return Math.min(3, Math.max(1, scale)) / dpr;
	}

	function u(n) {
		return Math.round(n * uiScale);
	}

	// The clock, in the system's locale for dates.
	readonly property var locale: Qt.locale((Quickshell.env("LC_ALL") || Quickshell.env("LC_TIME")
		|| Quickshell.env("LANG") || "C").replace(/[.@].*/, ""))

	SystemClock {
		id: clock

		precision: SystemClock.Minutes
	}

	// Caps Lock, from the keyboards' LEDs, read every 250 ms.
	property bool capsLock: false

	Instantiator {
		id: capsLeds

		model: root.env.capsLeds || []
		delegate: FileView {
			required property string modelData

			path: modelData
			blockLoading: true
			printErrors: false
		}
	}

	Timer {
		interval: 250
		running: capsLeds.count > 0
		repeat: true
		onTriggered: {
			let on = false;
			for (let i = 0; i < capsLeds.count; i++) {
				const led = capsLeds.objectAt(i);
				led.reload();
				on = on || Number(led.text()) > 0;
			}
			root.capsLock = on;
		}
	}

	// Power: a button's first click arms it, a second one within 3 s acts.
	// Outside the greeter it only says what it would run.
	readonly property var powerLabels: ({ suspend: "Suspend", reboot: "Reboot", poweroff: "Power off" })
	property string armed: ""
	property string powerError: ""

	function press(action) {
		powerError = "";
		if (armed !== action) {
			armed = action;
			disarm.restart();
			return;
		}
		armed = "";
		if (mode !== "greeter") {
			console.info(`yokoso: would run systemctl ${action}`);
			return;
		}
		power.action = action;
		power.command = ["systemctl", action];
		power.running = true;
	}

	Timer {
		id: disarm

		interval: 3000
		onTriggered: {
			root.armed = "";
			root.powerError = "";
		}
	}

	Process {
		id: power

		property string action

		stderr: StdioCollector {
			onStreamFinished: if (text)
				console.warn(`yokoso: ${text.trim()}`)
		}
		onExited: code => {
			if (code === 0)
				return;
			root.powerError = `${root.powerLabels[action]} failed`;
			disarm.restart();
		}
	}

	Auth {
		id: auth

		// fakegreet only knows "user".
		username: root.mode === "greeter" ? (root.user ? root.user.name : "") : "user"
		session: root.session
		x11Wrapper: root.config.x11Wrapper
	}

	Connections {
		target: auth

		function onPhaseChanged() {
			if (auth.phase === "launching")
				root.remember();
		}
	}

	FloatingWindow {
		id: window

		visible: true
		fullscreen: root.mode === "greeter"
		implicitWidth: 1280
		implicitHeight: 720
		title: "yokoso"
		color: root.colors.background

		Rectangle {
			id: content

			width: root.mode === "shot" && root.windowIsScreen ? root.shotSize[0] : window.width
			height: root.mode === "shot" && root.windowIsScreen ? root.shotSize[1] : window.height
			color: root.colors.background

			HoverHandler {
				enabled: root.config.output === "cursor" && !root.windowIsScreen
				onPointChanged: root.pointAt(point.position)
			}

			// The background, decoded at each screen's size.
			Repeater {
				model: root.windowIsScreen ? [null] : Quickshell.screens

				Image {
					required property var modelData

					x: modelData ? modelData.x - root.origin.x : 0
					y: modelData ? modelData.y - root.origin.y : 0
					width: modelData ? modelData.width : content.width
					height: modelData ? modelData.height : content.height
					source: root.backgroundFor(modelData)
					sourceSize: Qt.size(width * root.dpr, height * root.dpr)
					fillMode: Image.PreserveAspectCrop

					// So that text stays readable on a bright image.
					Rectangle {
						anchors.fill: parent
						visible: parent.status === Image.Ready
						color: root.colors.background
						opacity: root.config.backgroundDim
					}
				}
			}

			Column {
				x: root.area.x + (root.area.width - width) / 2
				y: form.y - height - root.u(48)
				spacing: root.u(4)

				Text {
					anchors.horizontalCenter: parent.horizontalCenter
					text: clock.date.toLocaleTimeString(root.locale, Locale.ShortFormat)
					color: root.colors.text
					font.pixelSize: root.u(56)
					font.family: root.fontFamily
				}

				// The locale's long date, without the year.
				Text {
					anchors.horizontalCenter: parent.horizontalCenter
					text: clock.date.toLocaleDateString(root.locale,
						root.locale.dateFormat(Locale.LongFormat).replace(/[\s,.]*y+[^dM]*/, ""))
					color: root.colors.subtext
					font.pixelSize: root.u(16)
					font.family: root.fontFamily
				}
			}

			Column {
				id: form

				x: root.area.x + (root.area.width - width) / 2
				y: root.area.y + (root.area.height - height) / 2
				width: root.u(300)
				spacing: root.u(10)

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
						visible: isUser || root.sessions.length > 1

						Row {
							id: row

							anchors.horizontalCenter: parent.horizontalCenter
							spacing: root.u(6)

							Text {
								id: name

								width: Math.min(implicitWidth, label.width - (chevron.visible ? chevron.width + row.spacing : 0))
								text: label.isUser ? (root.user ? root.user.name : "") : root.sessionName(root.session)
								color: mouse.containsMouse && root.canPick(label.modelData) ? root.colors.accent
									: label.isUser ? root.colors.text : root.colors.subtext
								font.pixelSize: root.u(label.isUser ? 20 : 14)
								font.family: root.fontFamily
								elide: Text.ElideRight
							}

							Text {
								id: chevron

								anchors.verticalCenter: name.verticalCenter
								visible: (label.isUser ? root.users : root.sessions).length > 1
								text: "▾"
								color: mouse.containsMouse && root.canPick(label.modelData) ? root.colors.accent : root.colors.dim
								font.pixelSize: root.u(label.isUser ? 14 : 11)
								font.family: root.fontFamily
							}
						}

						MouseArea {
							id: mouse

							anchors.fill: row
							anchors.margins: -root.u(4)
							hoverEnabled: true
							cursorShape: Qt.PointingHandCursor
							onClicked: root.togglePicker(label.modelData)
						}
					}
				}

				Rectangle {
					width: parent.width
					height: root.u(36)
					color: root.colors.field
					opacity: auth.phase === "prompt" ? 1 : 0.5

					TextInput {
						id: input

						anchors.fill: parent
						anchors.leftMargin: root.u(10)
						anchors.rightMargin: root.u(10)
						verticalAlignment: TextInput.AlignVCenter
						focus: true
						readOnly: auth.phase !== "prompt"
						echoMode: auth.secret ? TextInput.Password : TextInput.Normal
						color: root.colors.text
						font.pixelSize: root.u(15)
						font.family: root.fontFamily
						clip: true
						onAccepted: {
							auth.submit(text);
							text = "";
						}
						Keys.onEscapePressed: text = ""
						Keys.onPressed: event => root.pickerKey(event)
					}

					// PAM's question, or its last message (a failed attempt
					// included) until something is typed.
					Text {
						anchors.fill: input
						verticalAlignment: Text.AlignVCenter
						visible: input.text === ""
						text: root.problem || auth.message || auth.prompt.replace(/:\s*$/, "")
						color: auth.messageIsError || root.problem ? root.colors.error : root.colors.dim
						font.pixelSize: root.u(15)
						font.family: root.fontFamily
						elide: Text.ElideRight
					}
				}

				// Hidden rather than gone, so the form doesn't move.
				Text {
					width: parent.width
					horizontalAlignment: Text.AlignHCenter
					text: "Caps Lock is on"
					color: root.colors.error
					font.pixelSize: root.u(13)
					font.family: root.fontFamily
					opacity: root.capsLock ? 1 : 0
				}
			}

			Column {
				x: root.area.x + (root.area.width - width) / 2
				y: root.area.y + root.area.height - height - root.u(32)
				spacing: root.u(12)

				Text {
					anchors.horizontalCenter: parent.horizontalCenter
					text: root.powerError || (root.armed ? `Click ${root.powerLabels[root.armed]} again` : "")
					color: root.powerError ? root.colors.error : root.colors.subtext
					font.pixelSize: root.u(13)
					font.family: root.fontFamily
				}

				Row {
					anchors.horizontalCenter: parent.horizontalCenter
					spacing: root.u(32)

					Repeater {
						model: root.env.power || []

						Text {
							required property string modelData

							text: root.powerLabels[modelData] || modelData
							color: root.armed === modelData || mouse.containsMouse ? root.colors.accent : root.colors.dim
							font.pixelSize: root.u(14)
							font.family: root.fontFamily

							MouseArea {
								id: mouse

								anchors.fill: parent
								anchors.margins: -root.u(8)
								hoverEnabled: true
								cursorShape: Qt.PointingHandCursor
								onClicked: root.press(parent.modelData)
							}
						}
					}
				}
			}

			// A click anywhere else closes the list.
			MouseArea {
				anchors.fill: parent
				visible: root.picking !== ""
				onClicked: root.closePicker()
			}

			Rectangle {
				readonly property var label: labels.itemAt(root.picking === "session" ? 1 : 0)

				x: form.x
				y: label ? form.y + label.y + label.height + root.u(6) : 0
				width: form.width
				height: Math.min(list.count, 8) * root.u(32) + 2
				visible: root.picking !== ""
				color: root.colors.field
				border.color: root.colors.dim
				border.width: 1

				ListView {
					id: list

					anchors.fill: parent
					anchors.margins: 1
					clip: true
					boundsBehavior: Flickable.StopAtBounds
					model: root.pickList
					onCurrentIndexChanged: positionViewAtIndex(currentIndex, ListView.Contain)
					Keys.onReturnPressed: root.pick(currentIndex)
					Keys.onEnterPressed: root.pick(currentIndex)
					Keys.onEscapePressed: root.closePicker()
					Keys.onPressed: event => root.pickerKey(event)

					delegate: Item {
						required property var modelData
						required property int index

						width: ListView.view.width
						height: root.u(32)

						Rectangle {
							anchors.fill: parent
							visible: list.currentIndex === parent.index
							color: root.colors.accent
							opacity: 0.15
						}

						// The one picked now.
						Rectangle {
							width: root.u(3)
							height: parent.height
							visible: parent.modelData.current
							color: root.colors.accent
						}

						Text {
							anchors.left: parent.left
							anchors.right: detail.left
							anchors.leftMargin: root.u(10)
							anchors.rightMargin: root.u(10)
							anchors.verticalCenter: parent.verticalCenter
							text: parent.modelData.label
							color: root.colors.text
							font.pixelSize: root.u(15)
							font.family: root.fontFamily
							elide: Text.ElideRight
						}

						Text {
							id: detail

							anchors.right: parent.right
							anchors.rightMargin: root.u(10)
							anchors.verticalCenter: parent.verticalCenter
							width: Math.min(implicitWidth, parent.width / 2)
							text: parent.modelData.detail
							color: root.colors.dim
							font.pixelSize: root.u(13)
							font.family: root.fontFamily
							elide: Text.ElideRight
						}

						MouseArea {
							anchors.fill: parent
							hoverEnabled: true
							onEntered: list.currentIndex = parent.index
							onClicked: root.pick(parent.index)
						}
					}
				}
			}
		}
	}

	// Development: YOKOSO_TEST_ANSWERS answers prompts (comma separated),
	// YOKOSO_SCREENSHOT saves a frame and quits.
	property var answers: mode === "greeter" ? [] : (Quickshell.env("YOKOSO_TEST_ANSWERS") || "").split(",").filter(x => x)
	readonly property string shotFile: Quickshell.env("YOKOSO_SCREENSHOT") || ""

	Connections {
		target: auth
		enabled: root.mode !== "greeter"

		function onPhaseChanged() {
			console.info(`yokoso: ${auth.phase}`);
			if (auth.phase === "prompt" && root.answers.length)
				Qt.callLater(() => auth.submit(root.answers.shift()));
		}
	}

	Timer {
		running: root.shotFile !== ""
		interval: Number(Quickshell.env("YOKOSO_SHOT_DELAY") || 500)
		onTriggered: {
			const screens = Quickshell.screens.map(s => `${s.name} ${s.width}x${s.height}`);
			console.info(`yokoso: window ${window.width}x${window.height}, screens ${screens.join(", ")}, form on ${root.screen ? root.screen.name : "-"} at ${root.area}, lid "${lidFile.text().trim()}"`);
			content.grabToImage(result => {
				result.saveToFile(root.shotFile);
				if (!root.answers.length)
					Qt.quit();
			});
		}
	}

	Component.onCompleted: {
		Quickshell.watchFiles = mode !== "greeter";
		if (!problem)
			auth.start();
	}
}
