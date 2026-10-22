import QtQml.Models
import QtQuick
import Quickshell
import Quickshell.Io

// The core: what yokoso knows (the config, screens, users and sessions)
// and does (the login, power, memory). Look.qml, or a theme's look,
// draws it from the properties and functions here.
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

	readonly property var env: parse(envFile.text())

	// yokoso's own words (English ones) in the config's language, else the
	// locale's, when shell/i18n has it.
	function tr(text) {
		return env.strings && env.strings[text] || text;
	}

	// Every config key, with its default, over the theme the launcher put
	// together (themes/dark.json has its keys); colors and layout merge key
	// by key. The launcher reads the config file, leaving out one it can't.
	readonly property var configFile: env.config || {}
	readonly property var theme: env.theme || {}
	readonly property var defaults: ({
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
		// The language of yokoso's own words ("ru"); "" for the locale's.
		language: "",
		// Seconds without input before the screen goes dark; 0 for never.
		idle: 60,
		// What starts an X server for an X11 session, with the session's
		// command after it.
		x11Wrapper: "startx /usr/bin/env",
	})
	readonly property var config: Object.assign({}, defaults, theme, configFile, {
		colors: Object.assign({}, theme.colors, configFile.colors),
		layout: Object.assign({}, theme.layout, configFile.layout),
	})

	// The config's keys that nothing reads, a misspelt one ("thme") most
	// likely.
	function checkConfig() {
		const unknown = Object.keys(configFile).filter(k => !(k in defaults) && !(k in theme) && k !== "look");
		for (const group of ["colors", "layout"]) {
			if (configFile[group] && typeof configFile[group] === "object")
				unknown.push(...Object.keys(configFile[group]).filter(k => !(k in (theme[group] || {}))).map(k => `${group}.${k}`));
		}
		if (unknown.length)
			console.warn(`yokoso: unknown keys in ${Quickshell.env("YOKOSO_CONFIG")}: ${unknown.join(", ")}`);
	}

	readonly property var colors: config.colors

	// config.background's image for a screen, by its name ("" for the window),
	// as a URL: one path for every screen, or one per output, "*" for the rest.
	function backgroundFor(name) {
		const bg = config.background;
		return fileUrl(typeof bg === "string" ? bg : bg && (bg[name] || bg["*"]) || "");
	}

	// A file's path as a URL, "" for "".
	function fileUrl(path) {
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

	// The pointer's last position in the window. Where cage put it doesn't
	// count, only a move does.
	property point pointer: Qt.point(-1, -1)

	function pointAt(p) {
		const moved = pointer.x >= 0 && (p.x !== pointer.x || p.y !== pointer.y);
		pointer = Qt.point(p.x, p.y);
		if (!moved)
			return;
		wake();
		if (config.output !== "cursor" || windowIsScreen)
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

	// Every screen in the window, as { name, rect }, for what goes on each.
	readonly property var screens: windowIsScreen ? [{ name: "", rect: Qt.rect(0, 0, content.width, content.height) }]
		: Quickshell.screens.map(s => ({ name: s.name, rect: Qt.rect(s.x - origin.x, s.y - origin.y, s.width, s.height) }))

	// Who logged in last, and each user's last session, as { user, sessions:
	// { <user>: { id, type } } } (with remember), and the session started
	// last as started: { name, at } (ms): written at a login, in
	// /var/lib/yokoso.
	FileView {
		id: stateFile

		path: Quickshell.env("YOKOSO_STATE") || ""
		blockLoading: true
		// In the background, so that the fade out doesn't wait for the
		// disk; Qt finishes the write before it quits.
		blockWrites: false
		printErrors: false
		onSaveFailed: console.warn(`yokoso: can't write ${path}`)
	}

	readonly property var lastLogin: parse(stateFile.text())
	readonly property var remembered: config.remember ? lastLogin : {}

	function remember() {
		if (!stateFile.path || !user || !session)
			return;
		const started = { name: sessionName(session), at: Date.now() };
		if (!config.remember) {
			stateFile.setText(JSON.stringify({ started }) + "\n");
			return;
		}
		const last = Object.assign({}, remembered.sessions, {
			[user.name]: { id: session.id, type: session.type },
		});
		stateFile.setText(JSON.stringify({ user: user.name, sessions: last, started }) + "\n");
	}

	// A session that ended within a few seconds of its start (a broken
	// profile, a missing program, a compositor that can't start): greetd
	// brings the greeter back without a word, so the field says it.
	function checkLastSession() {
		const started = lastLogin.started;
		const after = started ? Date.now() - started.at : -1;
		if (after < 0 || after > 10000)
			return;
		console.info(`yokoso: the session ${started.name} ended ${Math.round(after / 1000)} s after it started`);
		auth.message = tr("%1 ended right after it started (journalctl -b)").arg(started.name);
		auth.messageIsError = true;
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

	// "user" or "session": whether there's another to pick. Not while a
	// password is checked (another user) or a session starts.
	function canPick(which) {
		return (which === "user" ? users : sessions).length > 1 && auth.phase !== "launching"
			&& !(which === "user" && auth.phase === "verifying");
	}

	// One of users: the login starts over for them.
	function pickUser(u) {
		if (u === user || !canPick("user"))
			return;
		pickedUser = u.name;
		pickedSession = null;
		auth.restart();
	}

	// One of sessions.
	function pickSession(s) {
		if (canPick("session"))
			pickedSession = s;
	}

	// Why a login can't start, if it can't.
	readonly property string problem: !user ? tr("No user found") : !session ? tr("No session found") : ""

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

	readonly property var now: clock.date

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

	// The machine's name, and its battery as { level (0 to 100), plugged },
	// or null without one, read every 10 s.
	readonly property string hostname: env.hostname || ""
	property var battery: null

	FileView {
		id: batteryLevel

		path: root.env.battery ? `${root.env.battery}/capacity` : ""
		blockLoading: true
		printErrors: false
	}

	FileView {
		id: batteryStatus

		path: root.env.battery ? `${root.env.battery}/status` : ""
		blockLoading: true
		printErrors: false
	}

	Timer {
		interval: 10000
		running: !!root.env.battery
		repeat: true
		triggeredOnStart: true
		onTriggered: {
			batteryLevel.reload();
			batteryStatus.reload();
			const level = parseInt(batteryLevel.text());
			root.battery = isNaN(level) ? null : {
				level: Math.max(0, Math.min(100, level)),
				plugged: batteryStatus.text().trim() !== "Discharging",
			};
		}
	}

	// Idle: after config.idle seconds without input the screen goes dark
	// and its backlights low (logind lets the session's owner set them),
	// until any input wakes it. The pointer is watched here; the look
	// reports keys with wake().
	property bool idle: false
	// The backlights' levels from before, while dim; in YOKOSO_BACKLIGHT
	// too, for the launcher to set back if yokoso stops before waking.
	property var dimmed: ({})
	onDimmedChanged: if (backlightFile.path)
		backlightFile.setText(JSON.stringify(dimmed) + "\n")

	// Returns whether the screen was asleep, so that the key or the click
	// that wakes it can be dropped.
	function wake() {
		if (!idle) {
			if (idleTimer.running)
				idleTimer.restart();
			return false;
		}
		idle = false;
		setBacklights(dimmed);
		dimmed = {};
		return true;
	}

	function sleep() {
		idle = true;
		const levels = {};
		const low = {};
		for (let i = 0; i < backlights.count; i++) {
			const b = backlights.objectAt(i);
			b.reload();
			const level = Number(b.text());
			const least = Math.max(1, Math.round(b.modelData.max / 100));
			if (level > least) {
				levels[b.modelData.name] = level;
				low[b.modelData.name] = least;
			}
		}
		dimmed = levels;
		setBacklights(low);
	}

	Timer {
		id: idleTimer

		interval: root.config.idle * 1000
		running: root.config.idle > 0 && !root.idle && root.auth.phase !== "launching"
		onTriggered: root.sleep()
	}

	Instantiator {
		id: backlights

		model: root.env.backlights || []
		delegate: FileView {
			required property var modelData

			path: `/sys/class/backlight/${modelData.name}/brightness`
			blockLoading: true
			printErrors: false
		}
	}

	FileView {
		id: backlightFile

		path: Quickshell.env("YOKOSO_BACKLIGHT") || ""
		blockWrites: true
		printErrors: false
	}

	// { <name>: level } through logind, one call after another: a wake
	// right after the dimming waits for it. Outside the greeter it only
	// says what it would set.
	property var nextBacklights: null

	function setBacklights(levels) {
		const args = [];
		for (const name in levels)
			args.push(name, String(levels[name]));
		if (!args.length)
			return;
		if (mode !== "greeter") {
			console.info(`yokoso: would set the backlights to ${JSON.stringify(levels)}`);
			return;
		}
		if (brightness.running) {
			nextBacklights = levels;
			return;
		}
		brightness.command = ["sh", "-c", `while [ $# -gt 0 ]; do
			busctl call org.freedesktop.login1 /org/freedesktop/login1/session/auto \\
				org.freedesktop.login1.Session SetBrightness ssu backlight "$1" "$2"
			shift 2
		done`, "sh", ...args];
		brightness.running = true;
	}

	Process {
		id: brightness

		stderr: StdioCollector {
			onStreamFinished: if (text)
				console.warn(`yokoso: ${text.trim()}`)
		}
		onExited: if (root.nextBacklights) {
			const levels = root.nextBacklights;
			root.nextBacklights = null;
			Qt.callLater(() => root.setBacklights(levels));
		}
	}

	// Power: the actions logind allows ("suspend", "reboot", "poweroff"). A
	// press() arms one, a second within armedFor ms acts. Outside the greeter
	// it only says what it would run.
	readonly property var power: env.power || []
	readonly property var powerLabels: ({ suspend: tr("Suspend"), reboot: tr("Reboot"), poweroff: tr("Power off") })
	property string armed: ""
	readonly property int armedFor: 3000
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
		systemctl.action = action;
		systemctl.command = ["systemctl", action];
		systemctl.running = true;
	}

	Timer {
		id: disarm

		interval: root.armedFor
		onTriggered: {
			root.armed = "";
			root.powerError = "";
		}
	}

	Process {
		id: systemctl

		property string action

		stderr: StdioCollector {
			onStreamFinished: if (text)
				console.warn(`yokoso: ${text.trim()}`)
		}
		onExited: code => {
			if (code === 0)
				return;
			root.powerError = root.tr({ suspend: "Suspend failed", reboot: "Reboot failed", poweroff: "Power off failed" }[action] || `${action} failed`);
			disarm.restart();
		}
	}

	// The login with greetd: phase, prompt, secret, message, messageIsError,
	// submit() (Auth.qml).
	readonly property Auth auth: Auth {
		// fakegreet only knows "user".
		username: root.mode === "greeter" ? (root.user ? root.user.name : "") : "user"
		session: root.session
		x11Wrapper: root.config.x11Wrapper
		launchDelay: root.config.animations ? 450 : 0
		failedText: root.tr("Wrong password")
		expiredText: root.tr("The password has expired: change it with passwd on a text console (Ctrl+Alt+F2)")
		deniedText: root.tr("This account can't log in now")
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

			// The pointer, for idle and for output "cursor".
			HoverHandler {
				onPointChanged: root.pointAt(point.position)
			}

			// What's on screen, given the core as `greeter`: the theme's
			// look, or the built-in one (Look.qml) when there's none or it
			// can't be loaded.
			Loader {
				property bool builtIn: !root.config.look

				anchors.fill: parent
				focus: true
				Component.onCompleted: setSource(builtIn ? "Look.qml" : root.fileUrl(root.config.look), { greeter: root })
				onStatusChanged: {
					if (status !== Loader.Error || builtIn)
						return;
					console.warn(`yokoso: can't load the look ${root.config.look}, using the built-in one`);
					builtIn = true;
					Qt.callLater(() => setSource("Look.qml", { greeter: root }));
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
			const outputs = Quickshell.screens.map(s => `${s.name} ${s.width}x${s.height}`);
			console.info(`yokoso: window ${window.width}x${window.height}, screens ${outputs.join(", ")}, form on ${root.screen ? root.screen.name : "-"} at ${root.area}, lid "${lidFile.text().trim()}"`);
			content.grabToImage(result => {
				result.saveToFile(root.shotFile);
				if (!root.answers.length)
					Qt.quit();
			});
		}
	}

	Component.onCompleted: {
		Quickshell.watchFiles = mode !== "greeter";
		checkConfig();
		checkLastSession();
		if (!problem)
			auth.start();
	}
}
