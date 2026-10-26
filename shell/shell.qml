import QtQml.Models
import QtQuick
import Quickshell
import Quickshell.Io
import yokoso

// The core: what yokoso knows (the config, screens, users and sessions)
// and does (the login, power, memory). Look.qml, or a theme's look,
// draws it from the properties and functions here.
ShellRoot {
	id: root

	// The version of what a look can use (docs/looks.md: the greeter API and
	// the parts in shell/yokoso), for a look made for a later one to tell.
	readonly property int api: 1

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

	// Safe mode: safeKey brings up the built-in look in the built-in dark
	// theme, for when a theme keeps anyone from logging in.
	readonly property string safeKey: "Ctrl+Alt+Shift+D"
	property bool safe: false

	function safeMode() {
		if (safe)
			return;
		console.warn("yokoso: safe mode: the built-in look and theme");
		safe = true;
		looks.builtIn = true;
		looks.setSource("Look.qml", { greeter: root });
	}

	// Every config key, with its default, over the theme the launcher put
	// together (themes/dark.json has its keys); colors and layout merge key
	// by key. The launcher reads the config file, leaving out one it can't.
	// In safe mode, the built-in dark theme and only the config's keys
	// that aren't the theme's.
	readonly property var configFile: safe
		? Object.keys(env.config || {}).filter(key => key in defaults && key !== "theme")
			.reduce((kept, key) => Object.assign(kept, { [key]: env.config[key] }), {})
		: env.config || {}
	readonly property var theme: (safe ? env.safeTheme : env.theme) || {}
	readonly property var defaults: ({
		user: "",
		// List the accounts to pick from; false: a name is typed every time.
		userList: true,
		session: "",
		// Sessions of one's own, before the desktop files' (the launcher
		// reads them): { name, exec, type ("wayland", "x11"), desktopNames
		// (["niri"]), env ({ VAR: "value" }) }.
		sessions: [],
		// Variables for every session, under a session's own env.
		env: {},
		// An eye in the field that shows the password as it's typed.
		reveal: true,
		// The power buttons, in order, those logind allows: "suspend",
		// "reboot", "poweroff", "firmware" (a reboot to the firmware's
		// setup) and { entry, label } for a reboot into a boot loader entry
		// ({ entry: "auto-windows" }; bootctl list shows the ids).
		power: ["suspend", "reboot", "poweroff"],
		// Start with the user and the session of the last login.
		remember: true,
		// "dark", "light", "high-contrast", a theme's name or a theme
		// file's path.
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

	// Whether things move: config.animations, true, false or "auto" for not
	// when everything is drawn in software, where each frame costs the CPU.
	readonly property bool animate: config.animations === "auto" ? !env.software : !!config.animations

	// The background for a screen, by its name ("" for the window), as a
	// URL: the user's own (backgrounds/<name> next to the config) on every
	// screen, else config.background's, one path for every screen or one
	// per output, "*" for the rest.
	function backgroundFor(name) {
		if (user && user.background)
			return fileUrl(user.background);
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

	// The screen with the form: the one chosen here (the pointer's last,
	// with output "cursor"; one clicked, or the next with F4), or
	// config.output; else a laptop's own panel, or with its lid closed an
	// external screen; else the first one. Never a closed lid's panel,
	// while there's another.
	property var chosenScreen: null

	function internal(s) {
		return /^(eDP|LVDS|DSI)-/.test(s.name);
	}

	function usable(s) {
		return !!s && Quickshell.screens.includes(s) && !(lidClosed && internal(s) && Quickshell.screens.length > 1);
	}

	readonly property var screen: {
		const screens = Quickshell.screens;
		return (usable(chosenScreen) ? chosenScreen : null)
			|| screens.find(s => s.name === config.output && usable(s))
			|| (lidClosed ? screens.find(s => !internal(s)) : screens.find(s => internal(s)))
			|| screens[0] || null;
	}

	// The window's top-left corner among the screens: cage spreads it over
	// all of them.
	readonly property point origin: Qt.point(Math.min(...Quickshell.screens.map(s => s.x)),
		Math.min(...Quickshell.screens.map(s => s.y)))

	// The screen at a point in the window.
	function screenAt(p) {
		return Quickshell.screens.find(s => p.x >= s.x - origin.x && p.x < s.x - origin.x + s.width
			&& p.y >= s.y - origin.y && p.y < s.y - origin.y + s.height) || null;
	}

	// The pointer's last position in the window. Where cage put it doesn't
	// count, only a move does.
	property point pointer: Qt.point(-1, -1)

	function pointAt(p) {
		const moved = pointer.x >= 0 && (p.x !== pointer.x || p.y !== pointer.y);
		pointer = Qt.point(p.x, p.y);
		if (!moved)
			return;
		wake();
		if (config.output === "cursor" && !windowIsScreen)
			choose(screenAt(p));
	}

	// The form goes to `s`, if it can, over config.output, until yokoso
	// starts again: a click on another screen, or F4, brings it to one that
	// can be seen when the one it's on can't (a screen that stays dark).
	function choose(s) {
		if (usable(s) && s !== screen)
			chosenScreen = s;
	}

	// F4: the next screen.
	function nextScreen() {
		if (wake() || windowIsScreen)
			return;
		const screens = Quickshell.screens.filter(s => usable(s));
		if (screens.length > 1)
			choose(screens[(screens.indexOf(screen) + 1) % screens.length]);
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

	// Whether the accounts are listed to pick from (config.userList); else
	// a name is typed, every time.
	readonly property bool userList: config.userList !== false

	// The users to pick from, config.user too when it isn't listed (LDAP
	// accounts often aren't); none without userList.
	readonly property var users: {
		const listed = env.users || [];
		const name = config.user;
		if (!userList)
			return [];
		return !name || listed.some(u => u.name === name) ? listed : [{ name, realName: name }].concat(listed);
	}
	readonly property var sessions: env.sessions || []

	// An account by its name: the listed one, else just the name (one in
	// a directory, typed in).
	function account(name) {
		return name ? (env.users || []).find(u => u.name === name) || { name, realName: name, icon: "" } : null;
	}

	// The one picked or typed here, else the last login's, else config's,
	// else the first; without userList, the one typed or config's. Picking
	// a user brings back their last session.
	property string pickedUser: ""
	property var pickedSession: null
	readonly property var user: {
		if (pickedUser)
			return users.find(u => u.name === pickedUser) || account(pickedUser);
		if (!userList)
			return account(config.user);
		return [remembered.user, config.user].map(name => users.find(u => u.name === name)).find(u => u)
			|| users[0] || null;
	}
	readonly property var session: {
		const last = user && remembered.sessions && remembered.sessions[user.name] || {};
		return pickedSession
			|| sessions.find(s => s.id === last.id && s.type === last.type)
			|| sessions.find(s => s.id === config.session || s.id === `${config.session}.desktop`)
			|| sessions[0] || null;
	}

	// No user yet: the look asks for a name (enterUser()).
	readonly property bool needsUser: !user

	// A session's name, with "(X11)" when a Wayland one has the same.
	function sessionName(s) {
		if (!s)
			return "";
		return s.type === "x11" && sessions.some(o => o !== s && o.name === s.name) ? `${s.name} (X11)` : s.name;
	}

	// "user" or "session": whether there's another to pick (any user,
	// with userList, as another can be typed). Not while a password is
	// checked (another user) or a session starts.
	function canPick(which) {
		if (which === "user")
			return userList && users.length > 0 && canEnterUser();
		return sessions.length > 1 && auth.phase !== "launching";
	}

	// Whether another name can be typed now.
	function canEnterUser() {
		return auth.phase !== "launching" && auth.phase !== "verifying";
	}

	// One of users: the login starts over for them.
	function pickUser(u) {
		if (u === user || !canPick("user"))
			return;
		enterUser(u.name);
	}

	// A typed name, listed or not: the login starts over for them.
	function enterUser(name) {
		name = (name || "").trim();
		if (!name || user && name === user.name || !canEnterUser())
			return;
		pickedUser = name;
		pickedSession = null;
		auth.restart();
	}

	// One of sessions.
	function pickSession(s) {
		if (canPick("session"))
			pickedSession = s;
	}

	// Why a login can't start, if it can't.
	readonly property string problem: !session ? tr("No session found") : ""

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

	// Caps Lock and Num Lock, from the keyboards' LEDs, read every 250 ms
	// while the screen is awake.
	readonly property bool capsLock: capsLeds.on
	readonly property bool numLock: numLeds.on

	component Leds: Instantiator {
		property bool on: false

		function read() {
			let lit = false;
			for (let i = 0; i < count; i++) {
				const led = objectAt(i);
				led.reload();
				lit = lit || Number(led.text()) > 0;
			}
			on = lit;
		}

		delegate: FileView {
			required property string modelData

			path: modelData
			blockLoading: true
			printErrors: false
		}
	}

	Leds {
		id: capsLeds

		model: root.env.capsLeds || []
	}

	Leds {
		id: numLeds

		model: root.env.numLeds || []
	}

	Timer {
		interval: 250
		running: capsLeds.count + numLeds.count > 0 && !root.idle
		repeat: true
		triggeredOnStart: true
		onTriggered: {
			capsLeds.read();
			numLeds.read();
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

	// Power: the actions of config.power that logind allows ("suspend",
	// "reboot", "poweroff", "firmware" for the firmware's setup, and
	// "entry:<id>" for a boot loader entry). A press() arms one, a second
	// within armedFor ms acts. Outside the greeter it only says what it
	// would run. powerLabels says what each does, powerAgain that an armed
	// one wants a second press.
	readonly property var power: env.power || []
	readonly property var powerLabels: powerWords(tr("Suspend"), tr("Reboot"), tr("Power off"),
		tr("Reboot to firmware setup"), tr("Reboot into %1"))
	readonly property var powerAgain: powerWords(tr("Press again to suspend"), tr("Press again to reboot"),
		tr("Press again to power off"), tr("Press again to reboot to firmware setup"), tr("Press again to reboot into %1"))

	function powerWords(suspend, reboot, poweroff, firmware, entry) {
		const words = { suspend, reboot, poweroff, firmware };
		for (const action of power) {
			if (action.startsWith("entry:"))
				words[action] = entry.arg(entryName(action.slice(6)));
		}
		return words;
	}

	// A boot loader entry's name: its label in config.power, a few of
	// systemd-boot's own, else its id.
	function entryName(id) {
		const own = (Array.isArray(config.power) ? config.power : []).find(a => a && a.entry === id && typeof a.label === "string");
		const known = { "auto-windows": "Windows", "auto-osx": "macOS", "auto-efi-shell": "EFI Shell" };
		return own ? own.label : known[id] || id.replace(/\.conf$/, "");
	}

	function powerCommand(action) {
		if (action === "firmware")
			return ["systemctl", "reboot", "--firmware-setup"];
		if (action.startsWith("entry:"))
			return ["systemctl", "reboot", `--boot-loader-entry=${action.slice(6)}`];
		return ["systemctl", action];
	}

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
			console.info(`yokoso: would run ${powerCommand(action).join(" ")}`);
			return;
		}
		systemctl.action = action;
		systemctl.command = powerCommand(action);
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
			root.powerError = root.tr({ suspend: "Suspend failed", poweroff: "Power off failed" }[action] || "Reboot failed");
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
		env: root.config.env
		launchDelay: root.animate ? 450 : 0
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

			// A click on a screen without the form brings it there.
			TapHandler {
				onTapped: point => {
					if (!root.windowIsScreen)
						root.choose(root.screenAt(point.position));
				}
			}

			// F4 takes the form to the next screen, whatever the look does
			// with the keys.
			Shortcut {
				sequence: "F4"
				context: Qt.ApplicationShortcut
				onActivated: root.nextScreen()
			}

			// Safe mode, whatever the look does with the keys.
			Shortcut {
				sequence: root.safeKey
				context: Qt.ApplicationShortcut
				onActivated: root.safeMode()
			}

			// What's on screen, given the core as `greeter`: the theme's
			// look, or the built-in one (Look.qml) when there's none or it
			// can't be loaded.
			Loader {
				id: looks

				property bool builtIn: !root.config.look

				anchors.fill: parent
				focus: true
				Component.onCompleted: {
					// The parts reach the core through Greeter.
					Greeter.core = root;
					setSource(builtIn ? "Look.qml" : root.fileUrl(root.config.look), { greeter: root });
				}
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
		if (config.look)
			console.info(`yokoso: the look is ${config.look}; ${safeKey} brings up the built-in one`);
		checkLastSession();
		if (!problem && user)
			auth.start();
	}
}
