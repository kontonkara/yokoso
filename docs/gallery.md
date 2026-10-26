# Gallery

The built-in themes and the [examples](../examples), each with what puts
it on the screen. An example's files go to `/etc/yokoso/themes/` (on
NixOS, `services.yokoso.themes.<name>`, a look as `look = ./card.qml;`).

## dark

The default, over a picture of your own (`"background":
"/etc/yokoso/wallpaper.jpg"`).

<img src="images/dark.webp" width="100%" alt="The dark theme over a wallpaper">

## light

`"theme": "light"`, the user list open.

<img src="images/light.webp" width="100%" alt="The light theme, with the user list open">

## high-contrast

`"theme": "high-contrast"`: white on black, a yellow accent and solid
outlines; Tab's ring on the session.

<img src="images/contrast.webp" width="100%" alt="The high-contrast theme: white on black with yellow, a ring around the session">

## Catppuccin Mocha, laid out

[`catppuccin-mocha.json`](../examples/catppuccin-mocha.json), with the form
on the left, the clock at the top and the power buttons in a corner:
`"layout": {"form": "left", "clock": "top", "power": "corner", "status":
"top-left"}` and `"radius": 12`.

<img src="images/layout.webp" width="100%" alt="Catppuccin Mocha with the form on the left, the clock at the top and the power buttons in a corner">

## Card

[`card.json`](../examples/card.json) and its look
[`card.qml`](../examples/card.qml), made of yokoso's parts: Nord, the clock
in a corner, and the form on a card.

<img src="images/card.webp" width="100%" alt="Nord: a large clock in the top left corner, and the form on a card in the middle, the picture half out of its top edge">

## Sidebar

[`sidebar.json`](../examples/sidebar.json) and
[`sidebar.qml`](../examples/sidebar.qml), made of the parts too: a panel
down the left side, on Catppuccin Mocha.

<img src="images/sidebar.webp" width="100%" alt="The sidebar example: a panel down the left side with the clock, the user, the session, the field and the power buttons">

## Minimal

[`minimal.json`](../examples/minimal.json) and
[`minimal.qml`](../examples/minimal.qml): a look from scratch, as small as
a working one gets, without the parts.

<img src="images/minimal.webp" width="100%" alt="The minimal example: the time, the user and the session, and a plain password field on black">
