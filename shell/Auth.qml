import QtQuick
import Quickshell.Services.Greetd

// The login conversation with greetd for one user, ending in their session.
QtObject {
	id: auth

	required property string username
	// { id, exec, desktopNames, type, env } from the launcher's snapshot, or
	// null.
	required property var session
	// config.x11Wrapper: an X11 session's command goes after it, as greetd
	// starts only the command, not an X server.
	required property string x11Wrapper
	// config.env: variables for every session, under the session's env.
	property var env: ({})
	// What a failed attempt says, unless PAM said why: a wrong password; a
	// password to change, which greetd (0.10) can't do; an account PAM
	// turns away.
	property string failedText: "Wrong password"
	property string expiredText: "The password has expired: change it with passwd on a text console (Ctrl+Alt+F2)"
	property string deniedText: "This account can't log in now"
	// How long "launching" lasts before the session starts: the look's
	// time to fade out.
	property int launchDelay: 0

	// "starting", "prompt" (waiting for submit()), "verifying", "failed" or
	// "launching".
	property string phase: "starting"
	property string prompt: ""
	property bool secret: true
	property string message: ""
	property bool messageIsError: false

	// The session is greetd's: the greeter is to quit.
	signal launched

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
		messageIsError = false;
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
		messageIsError = false;
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

	property Timer launchTimer: Timer {
		interval: auth.launchDelay
		onTriggered: {
			const s = auth.session;
			const x11 = s.type === "x11";
			// The desktop's name: DesktopNames, else the desktop file's (one
			// of config.sessions has none, and its program says).
			const desktops = (s.desktopNames || "").split(";").filter(x => x);
			const name = s.id.endsWith(".desktop") ? s.id.replace(/\.desktop$/, "") : "";
			const vars = { XDG_SESSION_TYPE: x11 ? "x11" : "wayland" };
			if (desktops.length || name) {
				vars.XDG_CURRENT_DESKTOP = desktops.join(":") || name;
				vars.XDG_SESSION_DESKTOP = desktops[0] || name;
			}
			for (const more of [auth.env, s.env]) {
				if (more && typeof more === "object" && !Array.isArray(more))
					Object.assign(vars, more);
			}
			// The greeter quits itself, at launched().
			Greetd.launch(x11 ? [auth.x11Wrapper, s.exec] : [s.exec], Object.keys(vars).map(k => `${k}=${vars[k]}`), false);
		}
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
				// Info and error messages: Quickshell acknowledges them.
				auth.message = message;
				auth.messageIsError = error;
			}
		}

		// greetd's word for it: "pam_authenticate: AUTH_ERR",
		// "pam_acct_mgmt: NEW_AUTHTOK_REQD", ...
		function onAuthFailure(message) {
			if (message.includes("NEW_AUTHTOK_REQD"))
				auth.fail(auth.expiredText);
			else if (auth.messageIsError && auth.message)
				auth.fail(auth.message);
			else
				auth.fail(message.startsWith("pam_acct_mgmt") ? auth.deniedText : auth.failedText);
		}

		function onError(error) {
			// While a retry waits, an error is greetd's answer to the cancel
			// after a failure: by then (0.10) the session's worker is gone.
			if (!auth.retry)
				auth.fail(error);
		}

		function onReadyToLaunch() {
			auth.phase = "launching";
			auth.launchTimer.start();
		}

		// greetd has the session: it starts once the greeter is gone.
		function onLaunched() {
			auth.launched();
		}
	}
}
