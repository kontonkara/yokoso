# Architecture

How yokoso is put together, and the rules every part keeps. A change that
breaks a rule here changes this file first.

## Processes

One binary, `yokoso`, with modes:

- `yokoso shell` — the desktop shell: bar, panels, notifications, launcher.
- `yokoso lock` and `yokoso greeter` — later stages, each its own process, so
  a crash in the bar never takes the lock screen with it. Authentication runs
  in a helper process of its own.
- `yokoso msg …` — talks to a running shell (see IPC).
- `yokoso validate [file]` — checks a config without running anything.

## Crates

- `yokoso-ui` — the toolkit: Wayland surfaces, the GPU, text, input, motion.
  It knows nothing of bars or Wi-Fi.
- `yokoso` — the binary: modes, config, IPC, services and the shell's parts.

## The loop

- One event loop (calloop) on the main thread owns all UI state.
- Services (D-Bus, files, sockets) run apart, on threads of their own or as
  loop sources, and reach the UI only through typed messages on a channel.
  They never touch UI state.
- Wayland handlers record what happened — a new output, a scale, input — and
  mark the surfaces it concerns dirty. They never draw.

## The frame

Every frame of a surface goes through three phases, in order:

1. **Update** — apply pending messages and input; advance motion to the
   frame's time.
2. **Layout** — sizes and positions from the state.
3. **Render** — paint into the buffer and commit.

The state is not changed while rendering; what render needs is computed
before it.

When a surface draws:

- Only when it is dirty or something on it moves.
- When it turns dirty with no frame callback pending, one draw is queued on
  the loop's idle: many changes in one dispatch make one frame.
- Each commit asks for a frame callback. When it comes, the surface draws
  again only if it is still dirty or moving. Otherwise nothing happens:
  **idle is zero frames and no timers.**
- Frame time comes from presentation feedback where the compositor has it,
  else from the callback. Motion that starts after idle starts at its first
  frame, so it never jumps over time it did not show.
- EGL never waits for vblank (swap interval 0): the compositor's frame
  callbacks pace everything, at the output's own rate.

## Drawing

- One EGL display and one GLES 3 context for the whole process; a surface
  owns only its EGL window surface.
- The GPU is the one the compositor draws on, through Mesa. NVIDIA's EGL is
  never loaded: loading it alone wakes a sleeping discrete GPU.
- Shapes are signed distance fields in one shader: rounded rectangles,
  borders, shadows, all crisp at any scale. Text and icons come from atlases,
  one per scale.
- A frame repaints only what changed (buffer age) and tells the compositor
  only that region.
- Images decode on a worker thread, never in a frame.

## Text

- Parley lays text out; swash rasterizes its glyphs into the atlas. Fonts
  and their fallbacks are fontconfig's, opened at run time.
- Chosen over cosmic-text by a probe in stage 0: Parley follows the
  system's font settings by itself and shapes about twice as fast; what it
  lacks, an ellipsis, is a few lines here.
- Text is laid out in layout, at the surface's scale, so glyphs sit on its
  pixels; render only draws it.
- Emoji get a family of their own, in color: left to fallbacks they go to
  the first font that has them, often in plain outlines.

## Scale

- Buffers are drawn at the surface's fractional scale
  (`wp_fractional_scale_v1`) and shown at logical size through
  `wp_viewporter`. Without them, the output's whole scale.
- Layout is in logical pixels; edges snap to physical pixels when painting,
  so 1 px lines stay 1 px and nothing leaves a gap.

## Input

- Pointer events are collected until `wl_pointer.frame` and handled as one.
- A wheel scrolls in `value120` steps; fingers scroll continuously, and
  `axis_stop` hands over to kinetic scrolling. The two are never mixed.
- The keymap comes from the compositor (xkbcommon), with compose (dead keys)
  and the compositor's repeat rate and delay.

## Config

- `config.toml` — written by Nix or by hand; yokoso only reads it.
- `overrides.toml` — what the settings window changes, laid over the first.
- `state.toml` — what yokoso remembers by itself: history, counters. It
  comes with the first part that has something to remember.
- Files are watched and reloaded on change. A broken file is reported with
  its line and column, and the last good config stays in force.

## IPC

- A Unix socket, `$XDG_RUNTIME_DIR/yokoso-<WAYLAND_DISPLAY>.sock`.
- One JSON request per line in, one JSON reply per line out.
- `yokoso msg` is its client; niri binds call it.

## Logs

- `tracing`, to stderr (the journal, under systemd). `YOKOSO_LOG` sets the
  filter, as `RUST_LOG` does.

## Rules, short

1. Handlers mark dirty; only the frame draws.
2. Update, Layout, Render — never back.
3. No frame callback asked for without a reason; idle is zero frames.
4. Services talk in messages, not in shared state.
5. The integrated GPU only, through Mesa.
6. Logical pixels in layout, physical ones only in paint.
7. Every part works in a plain window too, for trying it without breaking
   the session.
