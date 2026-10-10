import QtQuick
import Quickshell.Services.Greetd

// The login conversation with greetd for one user, ending in their session.
QtObject {
	id: auth

	required property string username
	// { id, exec, desktopNames, type } from the launcher's snapshot, or null.
	required property var session
	// config.x11Wrapper: an X11 session's command goes after it, as greetd
	// starts only the command, not an X server.
	required property string x11Wrapper
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

	// Starts over, for another user: the conversation going on is cancelled,
	// and the new one waits as after a failure.
	function restart() {
		prompt = "";
		message = "";
		phase = "starting";
		retry = true;
		if (Greetd.state === GreetdState.Inactive)
			retryTimer.restart();
		else
			Greetd.cancelSession();
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
			// While a retry waits, an error is greetd's answer to the cancel
			// after a failure: by then (0.10) the session's worker is gone.
			if (!auth.retry)
				auth.fail(error);
		}

		function onReadyToLaunch() {
			const s = auth.session;
			const x11 = s.type === "x11";
			const desktops = (s.desktopNames || "").split(";").filter(x => x);
			const name = s.id.replace(/\.desktop$/, "");
			auth.phase = "launching";
			Greetd.launch(x11 ? [auth.x11Wrapper, s.exec] : [s.exec], [
				`XDG_SESSION_TYPE=${x11 ? "x11" : "wayland"}`,
				`XDG_CURRENT_DESKTOP=${desktops.join(":") || name}`,
				`XDG_SESSION_DESKTOP=${desktops[0] || name}`,
			], true);
		}
	}
}
