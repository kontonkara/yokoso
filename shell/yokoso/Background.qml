import QtQuick

// The theme's background on every screen, decoded at each screen's size
// away from the GUI thread (a large picture takes it half a second),
// fading in when it's there, under colors.background at backgroundDim.
// Fill the look with it.
Item {
	Repeater {
		model: Greeter.core.screens

		Image {
			required property var modelData

			x: modelData.rect.x
			y: modelData.rect.y
			width: modelData.rect.width
			height: modelData.rect.height
			source: Greeter.core.backgroundFor(modelData.name)
			sourceSize: Qt.size(width * Greeter.core.dpr, height * Greeter.core.dpr)
			fillMode: Image.PreserveAspectCrop
			asynchronous: true
			opacity: status === Image.Ready ? 1 : 0

			Behavior on opacity {
				enabled: Greeter.animate

				NumberAnimation {
					duration: 300
				}
			}

			// So that text stays readable on a bright image.
			Rectangle {
				anchors.fill: parent
				color: Greeter.colors.background
				opacity: Greeter.core.config.backgroundDim
			}
		}
	}
}
