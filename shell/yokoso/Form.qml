import QtQuick

// The login form's state, shared by the parts it's made of (Avatar,
// UserName, SessionPill, PasswordField, FieldNote, Picker): which list is
// open, whether a name is typed instead of the password, and what keys
// and clicks do. A look makes one and gives it to them as `form`.
QtObject {
	id: form

	readonly property var core: Greeter.core

	// Filled in by the parts: PasswordField's text input, Picker's list,
	// and whether PAM's last message fits in the field (else FieldNote
	// shows it).
	property Item input: null
	property Item list: null
	property bool messageFits: true

	// The list open: "user", "session" or "".
	property string picking: ""
	readonly property var pickList: picking === "user"
		? core.users.map(u => ({ label: u.name, detail: u.realName !== u.name ? u.realName : "", current: u === core.user }))
			.concat([{ label: core.tr("Other…"), detail: "", current: false, other: true }])
		: picking === "session"
		? core.sessions.map(s => ({ label: core.sessionName(s), detail: "", current: s === core.session }))
		: []

	// A name typed into the field instead of the password: while there's
	// no user (without userList, until one is typed), and after "Other…"
	// or a click on a typed name.
	property bool enteringName: false
	readonly property bool askingName: enteringName || core.needsUser
	// Whether the field takes typing now, and whether what's typed is a
	// secret.
	readonly property bool typing: askingName || core.auth.phase === "prompt"
	readonly property bool secret: !askingName && core.auth.secret
	// What's typed.
	readonly property string text: input ? input.text : ""

	function togglePicker(which) {
		if (picking === which || !core.canPick(which)) {
			closePicker();
			return;
		}
		picking = which;
		if (list) {
			list.currentIndex = pickList.findIndex(item => item.current);
			list.forceActiveFocus();
		}
	}

	function closePicker() {
		picking = "";
		if (input)
			input.forceActiveFocus();
	}

	// The entry at `index` of pickList ("Other…" types a name).
	function pick(index) {
		const other = picking === "user" && index === core.users.length;
		if (picking === "user" && !other)
			core.pickUser(core.users[index]);
		else if (picking === "session")
			core.pickSession(core.sessions[index]);
		closePicker();
		if (other)
			enterName("");
	}

	// Types a name in the field, starting from `name`.
	function enterName(name) {
		if (!core.canEnterUser() || !input)
			return;
		enteringName = true;
		input.text = name;
		input.selectAll();
		input.forceActiveFocus();
	}

	// A click on the user (or F2): the list, or without one the name to
	// type again.
	function userClicked() {
		if (core.userList)
			togglePicker("user");
		else if (!askingName)
			enterName(core.user ? core.user.name : "");
	}

	// What's typed goes: the name, or the answer to PAM.
	function submit() {
		if (!input)
			return;
		if (askingName) {
			const name = input.text.trim();
			if (!name)
				return;
			enteringName = false;
			input.text = "";
			core.enterUser(name);
			return;
		}
		core.auth.submit(input.text);
		input.text = "";
	}

	// Every key press, before the field or the list gets it: it keeps the
	// screen awake, or wakes it and goes no further. F2 opens or closes
	// the user list (without one, types the name again), F3 the session
	// list; in a list Enter picks and Escape closes it, and Escape clears
	// the field (and goes back from typing a name to the user's password).
	function keyPressed(event) {
		event.accepted = true;
		if (core.wake())
			return;
		const enter = event.key === Qt.Key_Return || event.key === Qt.Key_Enter;
		if (event.key === Qt.Key_F2)
			userClicked();
		else if (event.key === Qt.Key_F3)
			togglePicker("session");
		else if (picking && enter && list)
			pick(list.currentIndex);
		else if (picking && event.key === Qt.Key_Escape)
			closePicker();
		else if (event.key === Qt.Key_Escape && input) {
			input.text = "";
			if (core.user)
				enteringName = false;
		} else
			event.accepted = false;
	}
}
