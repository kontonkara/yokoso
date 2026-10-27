import QtQuick
import QtMultimedia

// A video as the background: muted, round and round, filling the screen,
// and still while the screen is dark. Background loads it by its file, as
// Qt Multimedia is only there with yokoso's video (services.yokoso.video).
Video {
	id: video

	// Playing: Background fades it in.
	property bool ready: false

	fillMode: VideoOutput.PreserveAspectCrop
	muted: true
	loops: MediaPlayer.Infinite
	onPlaying: ready = true
	onErrorOccurred: (error, errorString) => console.warn(`yokoso: ${source}: ${errorString}`)
	onSourceChanged: if (source.toString() && !Greeter.core.idle)
		play()

	Connections {
		target: Greeter.core

		function onIdleChanged() {
			if (Greeter.core.idle)
				video.pause();
			else
				video.play();
		}
	}
}
