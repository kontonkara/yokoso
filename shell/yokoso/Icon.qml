import QtQuick
import QtQuick.Shapes

// A line icon, `path` (one of Greeter.icons, or any SVG path on a 24x24
// grid), stroked in `color`, `size` pixels square.
Shape {
	id: icon

	property string path
	property color color
	property real size: 20

	width: size
	height: size
	preferredRendererType: Shape.CurveRenderer

	ShapePath {
		strokeColor: icon.color
		strokeWidth: Math.max(1.5, icon.size / 10)
		fillColor: "transparent"
		capStyle: ShapePath.RoundCap
		joinStyle: ShapePath.RoundJoin
		scale: Qt.size(icon.size / 24, icon.size / 24)

		PathSvg {
			path: icon.path
		}
	}
}
