import QtQuick

// The background on every screen (the user's own, or the theme's; a
// picture, a video, or a directory's in turn), a picture decoded at each
// screen's size away from the GUI thread (a large picture takes it half a
// second), fading in when it's there, under colors.background at
// backgroundDim. The next one, another user's or a directory's, fades in
// over the one before. Fill the look with it.
Item {
	// A picture, or a video (VideoLayer.qml, with Qt Multimedia).
	component Layer: Item {
		id: layer

		required property Item owner
		property url source: ""
		readonly property bool video: Greeter.core.isVideo(source)
		readonly property bool ready: video ? movie.status === Loader.Ready && movie.item.ready
			: picture.status === Image.Ready

		anchors.fill: parent
		opacity: 0
		onReadyChanged: if (ready && source.toString() === owner.wanted.toString())
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

		Image {
			id: picture

			anchors.fill: parent
			source: layer.video ? "" : layer.source
			sourceSize: Qt.size(width * Greeter.core.dpr, height * Greeter.core.dpr)
			fillMode: Image.PreserveAspectCrop
			asynchronous: true
		}

		Loader {
			id: movie

			anchors.fill: parent
			active: layer.video
			source: "VideoLayer.qml"
			onStatusChanged: if (status === Loader.Error)
				console.warn("yokoso: a video background takes Qt Multimedia (services.yokoso.video)")
		}

		Binding {
			target: movie.item
			property: "source"
			value: layer.source
			when: movie.status === Loader.Ready
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
