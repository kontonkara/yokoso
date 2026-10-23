import QtQuick

// What the theme's `status` lists, in a row: "hostname", the machine's
// name, and "battery", a laptop's (a gauge, a bolt while it's plugged in,
// the level; in the error color when low and not plugged in).
Row {
	readonly property var shows: Greeter.core.config.status || []

	spacing: Greeter.u(18)

	Text {
		anchors.verticalCenter: parent.verticalCenter
		visible: parent.shows.includes("hostname") && text !== ""
		text: Greeter.core.hostname
		color: Greeter.colors.subtext
		font.pixelSize: Greeter.u(13)
		font.weight: Font.Medium
		font.family: Greeter.font
	}

	Row {
		id: gauge

		readonly property var battery: Greeter.core.battery
		readonly property color tint: battery && battery.level <= 15 && !battery.plugged ? Greeter.colors.error : Greeter.colors.subtext

		anchors.verticalCenter: parent.verticalCenter
		visible: parent.shows.includes("battery") && battery !== null
		spacing: Greeter.u(6)

		Icon {
			anchors.verticalCenter: parent.verticalCenter
			visible: gauge.battery !== null && gauge.battery.plugged
			path: Greeter.icons.bolt
			size: Greeter.u(14)
			color: gauge.tint
		}

		Item {
			anchors.verticalCenter: parent.verticalCenter
			width: Greeter.u(26)
			height: Greeter.u(13)

			Rectangle {
				id: cell

				width: parent.width - Greeter.u(3)
				height: parent.height
				radius: Greeter.u(3)
				color: "transparent"
				border.width: Math.max(1, Greeter.u(1.5))
				border.color: gauge.tint
			}

			Rectangle {
				x: cell.width + Greeter.u(1)
				anchors.verticalCenter: parent.verticalCenter
				width: Greeter.u(2)
				height: Greeter.u(5)
				radius: Greeter.u(1)
				color: gauge.tint
			}

			Rectangle {
				readonly property real inset: cell.border.width + Greeter.u(1.5)

				x: inset
				y: inset
				width: Math.max(Greeter.u(2), (cell.width - inset * 2) * (gauge.battery ? gauge.battery.level : 0) / 100)
				height: cell.height - inset * 2
				radius: Greeter.u(1)
				color: gauge.tint
			}
		}

		Text {
			anchors.verticalCenter: parent.verticalCenter
			text: gauge.battery ? `${gauge.battery.level}%` : ""
			color: gauge.tint
			font.pixelSize: Greeter.u(13)
			font.weight: Font.Medium
			font.features: { "tnum": 1 }
			font.family: Greeter.font
		}
	}
}
