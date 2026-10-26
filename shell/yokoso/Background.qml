import QtQuick

// The background on every screen (the user's own, or the theme's), decoded
// at each screen's size away from the GUI thread (a large picture takes it
// half a second), fading in when it's there, under colors.background at
// backgroundDim. Another one, for another user, fades in over the one
// before. Fill the look with it.
Item {
	component Layer: Image {
		id: layer

		required property Item owner

		anchors.fill: parent
		sourceSize: Qt.size(width * Greeter.core.dpr, height * Greeter.core.dpr)
		fillMode: Image.PreserveAspectCrop
		asynchronous: true
		opacity: 0
		onStatusChanged: if (status === Image.Ready && source.toString() === owner.wanted.toString())
			fade(1)

		function fade(to) {
			fading.to = to;
			fading.duration = Greeter.animate ? 300 : 0;
			fading.restart();
		}

		function drop() {
			fading.stop();
			opacity = 0;
			source = "";
		}

		NumberAnimation {
			id: fading

			target: layer
			property: "opacity"
			onFinished: layer.owner.settle(layer)
		}

		// So that text stays readable on a bright image.
		Rectangle {
			anchors.fill: parent
			color: Greeter.colors.background
			opacity: Greeter.core.config.backgroundDim
		}
	}

	Repeater {
		model: Greeter.core.screens

		Item {
			id: place

			required property var modelData
			readonly property url wanted: Greeter.core.backgroundFor(modelData.name)
			// The layer that's in, and the other one, for the next picture.
			property Item shown: one

			x: modelData.rect.x
			y: modelData.rect.y
			width: modelData.rect.width
			height: modelData.rect.height
			onWantedChanged: show()
			// Unless the user came first.
			Component.onCompleted: if (one.source.toString() === "" && two.source.toString() === "")
				show()

			// The wanted picture in the other layer, over this one (or this
			// one fading out, for none).
			function show() {
				const next = shown === one ? two : one;
				next.z = 1;
				shown.z = 0;
				next.source = wanted;
				if (wanted.toString() === "")
					shown.fade(0);
			}

			// Once the next one is in, it's the one shown, and the one under
			// it lets its picture go.
			function settle(layer) {
				if (layer.opacity === 0) {
					layer.drop();
					return;
				}
				shown = layer;
				(layer === one ? two : one).drop();
			}

			Layer {
				id: one

				owner: place
			}

			Layer {
				id: two

				owner: place
			}
		}
	}
}
