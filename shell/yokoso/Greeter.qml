pragma Singleton

import QtQuick

// yokoso's core (shell.qml) for the parts, set before a look loads, and
// what the parts share: the UI scale, the theme's colors, font and
// corners, whether things move, and the icons.
QtObject {
	// The core: what a look's `greeter` is.
	property var core: null

	// (While the core goes, at the end, its properties are gone first.)
	readonly property var colors: core && core.colors || ({})
	readonly property string font: core && core.fontFamily || ""
	readonly property bool animate: !!(core && core.animate)

	// colors.outline, around the field, the session, the power buttons
	// and the lists, or "": `faint` of colors.text for the field and the
	// lists, nothing for the rest.
	readonly property bool outlined: !!colors.outline

	function outline(faint) {
		return outlined ? colors.outline : Qt.alpha(colors.text, faint || 0);
	}

	// `n` pixels at the UI scale.
	function u(n) {
		return core && core.u ? core.u(n) : n;
	}

	// The theme's corner radius at the UI scale, or -1 for round.
	readonly property real radius: core && core.config && typeof core.config.radius === "number" ? u(core.config.radius) : -1

	// The corner radius for something `size` across: round, unless the
	// theme sets one; `inset`, how far inside a rounded thing it sits.
	function round(size, inset) {
		return radius < 0 ? size / 2 : Math.min(size / 2, Math.max(0, radius - (inset || 0)));
	}

	// Line icons on a 24x24 grid, as SVG paths, for Icon.
	readonly property var icons: ({
		poweroff: "M12 3v8 M17.66 6.34A8 8 0 1 1 6.34 6.34",
		reboot: "M20 12a8 8 0 1 1-2.34-5.66L20 8.5 M20 4v4.5h-4.5",
		suspend: "M12 3a6.5 6.5 0 0 0 9 9A9 9 0 1 1 12 3z",
		// A chip, and an arrow going in.
		firmware: "M7 5h10a2 2 0 0 1 2 2v10a2 2 0 0 1-2 2H7a2 2 0 0 1-2-2V7a2 2 0 0 1 2-2z M10 2v3 M14 2v3 M10 19v3 M14 19v3 M2 10h3 M2 14h3 M19 10h3 M19 14h3",
		entry: "M15 3h4a2 2 0 0 1 2 2v14a2 2 0 0 1-2 2h-4 M10 17l5-5-5-5 M15 12H3",
		arrow: "M5 12h14 M13 6l6 6-6 6",
		chevron: "M6 9l6 6 6-6",
		bolt: "M13 3L5 13h6l-1 8 8-10h-6l1-8z",
		eye: "M2 12s3.6-7 10-7 10 7 10 7-3.6 7-10 7S2 12 2 12z M12 9a3 3 0 1 0 0 6 3 3 0 1 0 0-6z",
		eyeOff: "M2 12s3.6-7 10-7 10 7 10 7-3.6 7-10 7S2 12 2 12z M12 9a3 3 0 1 0 0 6 3 3 0 1 0 0-6z M4 4l16 16",
	})
}
