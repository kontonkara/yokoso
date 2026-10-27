import QtQuick
import Quickshell
import yokoso

// What's drawn over all the screens, in a window (cage's, spread over all
// of them) or, locked, once on each screen's lock surface, moved so that
// the surface shows its part: the look, the pointer and the keys that
// aren't the look's.
Rectangle {
	id: stage

	required property var core
	// Locked: the screen whose surface this is, its form's while the keys
	// are on it.
	property var lockScreen: null
	// The keys are here: the shortcuts are this one's (on one surface only,
	// else Qt finds them ambiguous).
	readonly property bool active: lockScreen === null || Window.active

	color: core.colors.background
	onActiveChanged: if (active && lockScreen)
		core.choose(lockScreen)

	// The pointer, for idle and for output "cursor".
	HoverHandler {
		onPointChanged: stage.core.pointAt(point.position)
	}

	// A click on a screen without the form brings it there (locked, the
	// keys going there do).
	TapHandler {
		enabled: stage.lockScreen === null
		onTapped: point => {
			if (!stage.core.windowIsScreen)
				stage.core.choose(stage.core.screenAt(point.position));
		}
	}

	// F4 takes the form to the next screen, whatever the look does with the
	// keys.
	Shortcut {
		sequence: "F4"
		context: Qt.ApplicationShortcut
		enabled: stage.active && stage.lockScreen === null
		onActivated: stage.core.nextScreen()
	}

	// Safe mode, whatever the look does with the keys.
	Shortcut {
		sequence: stage.core.safeKey
		context: Qt.ApplicationShortcut
		enabled: stage.active
		onActivated: stage.core.safeMode()
	}

	// What's on screen, given the core as `greeter`: the theme's look, or
	// the built-in one (Look.qml) in safe mode, when there's none or when it
	// can't be loaded.
	Loader {
		id: looks

		property bool failed: false
		readonly property bool builtIn: !stage.core.config.look || stage.core.safe || failed

		function load() {
			setSource(builtIn ? "Look.qml" : stage.core.fileUrl(stage.core.config.look), { greeter: stage.core });
		}

		anchors.fill: parent
		focus: true
		active: stage.visible
		onBuiltInChanged: Qt.callLater(load)
		Component.onCompleted: {
			// The parts reach the core through Greeter.
			Greeter.core = stage.core;
			load();
		}
		onStatusChanged: {
			if (status !== Loader.Error || builtIn)
				return;
			console.warn(`yokoso: can't load the look ${stage.core.config.look}, using the built-in one`);
			failed = true;
		}
	}
}
