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
		id: configFile

		path: Quickshell.env("YOKOSO_CONFIG") || ""
		blockLoading: true
		printErrors: false
	}

	readonly property var env: parse(envFile.text())

	// Every config key, with its default.
	readonly property var config: Object.assign({
		user: "",
		session: "",
		// The output with the form ("DP-1"); "" picks one, see screen below.
		output: "",
		// 0: from the screen's physical size.
		scale: 0,
	}, parse(configFile.text()))

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

	// The screen with the form: config.output; else a laptop's own panel, or
	// with its lid closed an external screen; else the first one.
	readonly property var screen: {
		const screens = Quickshell.screens;
		const internal = s => /^(eDP|LVDS|DSI)-/.test(s.name);
		return screens.find(s => s.name === config.output)
			|| (lidClosed ? screens.find(s => !internal(s)) : screens.find(internal))
			|| screens[0] || null;
	}

	// Where that screen is in the window, which cage spreads over all of
	// them. In preview (and a one-screen shot) the window stands for it.
	readonly property bool windowIsScreen: mode === "preview" || (mode === "shot" && Quickshell.screens.length < 2)
	readonly property rect area: {
		if (windowIsScreen || !screen)
			return Qt.rect(0, 0, content.width, content.height);
		const screens = Quickshell.screens;
		const left = Math.min(...screens.map(s => s.x));
		const top = Math.min(...screens.map(s => s.y));
		return Qt.rect(screen.x - left, screen.y - top, screen.width, screen.height);
	}

	readonly property var users: env.users || []
	readonly property var user: users.find(u => u.name === config.user) || users[0] || null
	readonly property var sessions: env.sessions || []
	readonly property var session: sessions.find(s => s.id === config.session || s.id === `${config.session}.desktop`)
		|| sessions[0] || null

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

	Auth {
		id: auth

		// fakegreet only knows "user".
		username: root.mode === "greeter" ? (root.user ? root.user.name : "") : "user"
		session: root.session
	}

	FloatingWindow {
		id: window

		visible: true
		fullscreen: root.mode === "greeter"
		implicitWidth: 1280
		implicitHeight: 720
		title: "yokoso"
		color: "black"

		Rectangle {
			id: content

			width: root.mode === "shot" && root.windowIsScreen ? root.shotSize[0] : window.width
			height: root.mode === "shot" && root.windowIsScreen ? root.shotSize[1] : window.height
			color: "black"

			Column {
				x: root.area.x + (root.area.width - width) / 2
				y: root.area.y + (root.area.height - height) / 2
				width: root.u(300)
				spacing: root.u(10)

				Text {
					width: parent.width
					horizontalAlignment: Text.AlignHCenter
					text: root.user ? root.user.name : ""
					color: "white"
					font.pixelSize: root.u(20)
					elide: Text.ElideRight
				}

				Rectangle {
					width: parent.width
					height: root.u(36)
					color: "#222"
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
						color: "white"
						font.pixelSize: root.u(15)
						clip: true
						onAccepted: {
							auth.submit(text);
							text = "";
						}
						Keys.onEscapePressed: text = ""
					}

					// PAM's question, or its last message (a failed attempt
					// included) until something is typed.
					Text {
						anchors.fill: input
						verticalAlignment: Text.AlignVCenter
						visible: input.text === ""
						text: !root.session ? "No Wayland session found"
							: auth.message || auth.prompt.replace(/:\s*$/, "")
						color: auth.messageIsError || !root.session ? "#e66" : "#777"
						font.pixelSize: root.u(15)
						elide: Text.ElideRight
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
		if (session && auth.username)
			auth.start();
	}
}
