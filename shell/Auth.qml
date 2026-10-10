import QtQuick
import Quickshell.Services.Greetd

// The login conversation with greetd for one user, ending in their session.
QtObject {
	id: auth

	required property string username
	// { id, exec, desktopNames } from the launcher's snapshot, or null.
	required property var session
	property string failedText: "Wrong password"

	// "starting", "prompt" (waiting for submit()), "verifying", "failed" or
	// "launching".
	property string phase: "starting"
	property string prompt: ""
	property bool secret: true
	property string message: ""
	property bool messageIsError: false

	// After a failure greetd answers the cancel_session Quickshell sends, and
	// Quickshell (0.3.1) doesn't wait for that answer: a createSession sent
	// before it arrives would get it as its own reply. So a new attempt
	// waits for Inactive, then a little longer.
	property bool retry: false

	function start() {
		retry = false;
		prompt = "";
		phase = "starting";
		Greetd.createSession(username);
	}

	function submit(text) {
		if (phase !== "prompt")
			return;
		phase = "verifying";
		message = "";
		Greetd.respond(text);
	}

	function fail(text) {
		message = text;
		messageIsError = true;
		phase = "failed";
		retry = true;
		if (Greetd.state === GreetdState.Inactive)
			retryTimer.restart();
	}

	property Timer retryTimer: Timer {
		interval: 300
		onTriggered: auth.start()
	}

	property Connections greetd: Connections {
		target: Greetd

		function onStateChanged() {
			if (Greetd.state === GreetdState.Inactive && auth.retry)
				auth.retryTimer.restart();
		}

		function onAuthMessage(message, error, responseRequired, echoResponse) {
			if (responseRequired) {
				auth.prompt = message;
				auth.secret = !echoResponse;
				auth.phase = "prompt";
			} else {
				// Info and error messages are acknowledged with an empty
				// response, or the conversation stalls.
				auth.message = message;
				auth.messageIsError = error;
				Greetd.respond("");
			}
		}

		function onAuthFailure(message) {
			auth.fail(auth.failedText);
		}

		function onError(error) {
			auth.fail(error);
		}

		function onReadyToLaunch() {
			const s = auth.session;
			const desktops = (s.desktopNames || "").split(";").filter(x => x);
			const name = s.id.replace(/\.desktop$/, "");
			auth.phase = "launching";
			Greetd.launch([s.exec], [
				"XDG_SESSION_TYPE=wayland",
				`XDG_CURRENT_DESKTOP=${desktops.join(":") || name}`,
				`XDG_SESSION_DESKTOP=${desktops[0] || name}`,
			], true);
		}
	}
}
