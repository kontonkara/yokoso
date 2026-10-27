import QtQuick
import Quickshell
import Quickshell.Services.Pam

// The conversation with PAM that unlocks the screen, for the user who
// locked it: Auth's phases, with "launching" for unlocked (the look's time
// to fade out, then unlocked()).
QtObject {
	id: auth

	required property string username
	// PAM's service, /etc/pam.d/<config>.
	property string config: "yokoso-lock"
	property string failedText: "Wrong password"
	property int launchDelay: 0

	signal unlocked

	// "starting", "prompt" (waiting for submit()), "verifying", "failed" or
	// "launching".
	property string phase: "starting"
	property string prompt: ""
	property bool secret: true
	property string message: ""
	property bool messageIsError: false

	function start() {
		prompt = "";
		phase = "starting";
		if (!pam.start())
			fail(`can't start PAM's ${config}`);
	}

	// The user doesn't change.
	function restart() {
	}

	function submit(text) {
		if (phase !== "prompt")
			return;
		phase = "verifying";
		message = "";
		messageIsError = false;
		pam.respond(text);
	}

	function fail(text) {
		message = text;
		messageIsError = true;
		phase = "failed";
		retryTimer.restart();
	}

	property Timer retryTimer: Timer {
		interval: 300
		onTriggered: auth.start()
	}

	property Timer launchTimer: Timer {
		interval: auth.launchDelay
		onTriggered: auth.unlocked()
	}

	property PamContext pam: PamContext {
		config: auth.config
		// Development: a directory of PAM services of its own.
		configDirectory: Quickshell.env("YOKOSO_PAM_DIR") || "/etc/pam.d"
		user: auth.username

		onPamMessage: {
			if (responseRequired) {
				auth.prompt = message;
				auth.secret = !responseVisible;
				auth.phase = "prompt";
			} else {
				// A fingerprint reader's word, or why it failed.
				auth.message = message;
				auth.messageIsError = messageIsError;
			}
		}

		onCompleted: result => {
			if (result === PamResult.Success) {
				auth.phase = "launching";
				auth.launchTimer.start();
			} else {
				auth.fail(auth.messageIsError && auth.message ? auth.message : auth.failedText);
			}
		}

		onError: error => auth.fail(PamError.toString(error))
	}
}
