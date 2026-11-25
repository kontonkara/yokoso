//! The process's side of Wayland: the outputs, a surface for each thing the
//! shell wants shown, and frames. A surface draws only when it is dirty or
//! something on it moves, and only once the compositor has asked for a frame;
//! idle is zero frames.

use smithay_client_toolkit::{
    compositor::{CompositorHandler, CompositorState, FrameCallbackData},
    output::{OutputHandler, OutputState},
    presentation_time::{PresentTime, PresentationTimeHandler, PresentationTimeState},
    reexports::{
        calloop::{self, EventLoop, LoopHandle, channel},
        calloop_wayland_source::WaylandSource,
        client::{
            Connection, Proxy, QueueHandle, WEnum,
            globals::registry_queue_init,
            protocol::{wl_keyboard, wl_output, wl_pointer, wl_seat, wl_surface},
        },
        protocols::wp::presentation_time::client::wp_presentation_feedback,
    },
    registry::{ProvidesRegistryState, RegistryState},
    registry_handlers,
    seat::{
        Capability, SeatHandler, SeatState,
        keyboard::{KeyEvent, KeyboardHandler, Modifiers, RawModifiers},
        pointer::{PointerEvent, PointerEventKind, PointerHandler},
    },
    shell::{
        WaylandSurface,
        wlr_layer::{LayerShell, LayerShellHandler, LayerSurface, LayerSurfaceConfigure},
        xdg::{
            XdgShell,
            window::{Window, WindowConfigure, WindowDecorations, WindowHandler},
        },
    },
};

pub use smithay_client_toolkit::shell::wlr_layer::{Anchor, KeyboardInteractivity, Layer};

use crate::{
    geometry::{Color, Rect},
    gpu::{Gpu, Target},
    icons::Icons,
    input::{Input, Key, KeyState, Keysym, Mods, Scroll, ScrollSource},
    paint::Painter,
    scale::{Scaled, Scaling},
    text::{Paragraph, Text},
};

/// An output as the shell sees it.
#[derive(Clone, Debug, PartialEq)]
pub struct Output {
    pub id: OutputId,
    /// The connector, as "eDP-1".
    pub name: String,
    pub description: String,
    /// Its size in logical pixels.
    pub size: (i32, i32),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct OutputId(u32);

impl OutputId {
    /// An output that is not there: for tests of what goes where.
    #[doc(hidden)]
    pub fn for_tests(id: u32) -> Self {
        Self(id)
    }
}

/// What kind of surface the shell wants, and where.
#[derive(Clone, Debug, PartialEq)]
pub enum Spec {
    Layer(LayerSpec),
    /// A plain window, for trying a part without touching the session.
    Window(WindowSpec),
}

#[derive(Clone, Debug, PartialEq)]
pub struct WindowSpec {
    pub title: String,
    pub app_id: String,
    /// Logical pixels, until the compositor says otherwise.
    pub size: (u32, u32),
}

#[derive(Clone, Debug, PartialEq)]
pub struct LayerSpec {
    pub output: OutputId,
    pub layer: Layer,
    pub anchor: Anchor,
    /// Logical pixels; 0 for "as large as the anchors make it".
    pub size: (u32, u32),
    /// Top, right, bottom, left.
    pub margin: (i32, i32, i32, i32),
    pub exclusive_zone: i32,
    pub keyboard: KeyboardInteractivity,
    pub namespace: String,
    /// The pointer goes through it to what is beneath: it shows, and that is all.
    pub through: bool,
}

/// A surface's size and scale, for layout.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct View {
    /// Logical pixels.
    pub width: f32,
    pub height: f32,
    /// Buffer pixels to a logical one.
    pub scale: f32,
}

/// The time of the frame being made, in seconds of the monotonic clock.
#[derive(Clone, Copy, Debug)]
pub struct FrameTime {
    pub now: f64,
    /// Since the frame before, while something moves; 0 for the first frame
    /// after idle, so motion never jumps over time it did not show.
    pub dt: f64,
    /// How the surface has drawn so far, for a debug overlay: frames shown a
    /// second, the output's refresh in Hz (0 when unknown), the CPU time of
    /// its last frame in seconds, and its frames in all.
    pub fps: f64,
    pub refresh: f64,
    pub cpu: f64,
    pub frames: u64,
}

/// The shell: what it shows and how it answers. Its methods go in the order
/// of a frame: update, layout, render.
pub trait Shell: Sized + 'static {
    /// The shell's own name for each of its surfaces.
    type Key: Copy + Eq + std::hash::Hash + std::fmt::Debug + 'static;
    /// What its services send it.
    type Message: Send + 'static;

    /// The surfaces it wants, for these outputs.
    fn surfaces(&self, outputs: &[Output]) -> Vec<(Self::Key, Spec)>;

    fn message(&mut self, _ui: &mut Ui<Self>, _message: Self::Message) {}

    /// Input on `key`. It changes state and marks dirty; it never draws.
    fn input(&mut self, _ui: &mut Ui<Self>, _key: Self::Key, _input: &Input) {}

    /// Update: advances `key`'s state to `time`. True while something on it
    /// still moves.
    fn update(&mut self, _key: Self::Key, _time: &FrameTime) -> bool {
        false
    }

    /// Layout: sizes and positions for `view`; text is laid out here, at
    /// the view's scale.
    fn layout(&mut self, _key: Self::Key, _view: &View, _text: &mut Text) {}

    /// Where `key` changed since its last frame, in logical pixels; None for
    /// all of it. Asked after layout.
    fn damage(&mut self, _key: Self::Key) -> Option<Vec<Rect>> {
        None
    }

    /// Render: paints `key`. The state is only read here.
    fn render(&self, key: Self::Key, frame: &mut Frame);

    /// Where `key` takes the pointer, logical pixels, after its frame: None
    /// for all of it. What is outside goes to what is beneath, as the gaps
    /// between toasts.
    fn input_region(&self, _key: Self::Key) -> Option<Vec<Rect>> {
        None
    }

    /// After a frame of `key`: how long until it wants another though nothing
    /// moves, as a caret that blinks. None for never.
    fn wake(&self, _key: Self::Key) -> Option<std::time::Duration> {
        None
    }

    /// The user closed the window `key`. Quits, unless the shell says else.
    fn closed(&mut self, ui: &mut Ui<Self>, _key: Self::Key) {
        ui.quit();
    }
}

/// Painting a surface, in its logical pixels.
pub struct Frame<'a> {
    painter: &'a mut Painter,
    text: &'a mut Text,
    icons: &'a mut Icons,
    view: View,
    /// Added to everything painted, logical pixels: what comes in sliding.
    offset: (f32, f32),
    alpha: f32,
    /// What painting is kept inside now, logical pixels on the surface.
    clipped: Option<Rect>,
}

impl Frame<'_> {
    pub fn view(&self) -> View {
        self.view
    }

    fn snap(&self, rect: Rect) -> Rect {
        Rect::new(rect.x + self.offset.0, rect.y + self.offset.1, rect.w, rect.h).snapped(self.view.scale)
    }

    /// Everything painted from here on moved by `dx`, `dy`, until set again.
    pub fn set_offset(&mut self, dx: f32, dy: f32) {
        self.offset = (dx, dy);
    }

    pub fn offset(&self) -> (f32, f32) {
        self.offset
    }

    /// How much of what is painted shows now.
    pub fn current_alpha(&self) -> f32 {
        self.alpha
    }

    pub fn fill(&mut self, rect: Rect, radius: f32, color: Color) {
        let rect = self.snap(rect);
        self.painter.fill(rect, radius * self.view.scale, color);
    }

    /// A line `width` wide just inside `rect`'s edge; at least a buffer pixel.
    pub fn ring(&mut self, rect: Rect, radius: f32, width: f32, color: Color) {
        let rect = self.snap(rect);
        let width = (width * self.view.scale).round().max(1.0);
        self.painter.ring(rect, radius * self.view.scale, width, color);
    }

    pub fn shadow(&mut self, rect: Rect, radius: f32, blur: f32, color: Color) {
        let rect = self.snap(rect);
        let scale = self.view.scale;
        self.painter.shadow(rect, radius * scale, blur * scale, color);
    }

    /// `paragraph` with its top left at `x`, `y`. One laid out at another
    /// scale is skipped: layout runs again before the next frame.
    pub fn text(&mut self, paragraph: &Paragraph, x: f32, y: f32, color: Color) {
        if paragraph.scale() == self.view.scale {
            let scale = self.view.scale;
            let (x, y) = (x + self.offset.0, y + self.offset.1);
            self.text.draw(self.painter, paragraph, x * scale, y * scale, color);
        }
    }

    /// The icon `name` filling `rect`, in `color`.
    pub fn icon(&mut self, name: &str, rect: Rect, color: Color) {
        self.icon_scaled(name, rect, color, 1.0);
    }

    /// The same, `scale` of its size about its middle: an icon that pops.
    pub fn icon_scaled(&mut self, name: &str, rect: Rect, color: Color, scale: f32) {
        let rect = self.snap(rect);
        self.icons.paint(self.painter, name, rect, color, scale);
    }

    /// `picture` filling `rect`, scaled to it.
    pub fn picture(&mut self, picture: &crate::picture::Picture, rect: Rect) {
        let rect = self.snap(rect);
        self.icons.paint_picture(self.painter, picture, rect);
    }

    /// What is painted from here on shows `alpha` of itself: a part fading
    /// in or out as one.
    pub fn alpha(&mut self, alpha: f32) {
        self.alpha = alpha.clamp(0.0, 1.0);
        self.painter.set_alpha(self.alpha);
    }

    /// What is painted from here on shows only in `rect`, or everywhere.
    pub fn clip(&mut self, rect: Option<Rect>) {
        // Kept where it is on the surface, so an offset set after it moves
        // what is painted, not the clip.
        let (dx, dy) = self.offset;
        self.clipped = rect.map(|rect| Rect::new(rect.x + dx, rect.y + dy, rect.w, rect.h));
        let rect = rect.map(|rect| self.snap(rect));
        self.painter.clip(rect);
    }

    /// What painting is kept inside now, from where the offset is now.
    pub fn current_clip(&self) -> Option<Rect> {
        let (dx, dy) = self.offset;
        self.clipped
            .map(|rect| Rect::new(rect.x - dx, rect.y - dy, rect.w, rect.h))
    }

    /// Painting kept inside `rect` as well as what it is kept inside now.
    /// What it was before, to put back with `clip`.
    pub fn clip_within(&mut self, rect: Rect) -> Option<Rect> {
        let before = self.current_clip();
        self.clip(Some(before.map_or(rect, |outer| outer.intersect(&rect))));
        before
    }
}

/// A surface as it is now, for `msg dump`.
#[derive(Clone, Debug)]
pub struct SurfaceInfo {
    pub key: String,
    pub size: (u32, u32),
    pub scale: f32,
    pub frames: u64,
    pub waiting: bool,
    pub moving: bool,
    /// The output's refresh, Hz, as the last presented frame said.
    pub refresh: f64,
    /// Frames shown a second, as the last presented ones went.
    pub fps: f64,
}

/// Counts of what the process did, for the debug overlay and `msg stats`.
#[derive(Clone, Copy, Debug, Default)]
pub struct Stats {
    /// Times the event loop woke.
    pub wakeups: u64,
    pub frames: u64,
    /// CPU time of the last frame, in seconds.
    pub last_frame: f64,
    /// Frames that drew only what changed.
    pub partial: u64,
}

struct Surface<K> {
    key: K,
    spec: Spec,
    // Before the role: dropped before its surface is.
    scaled: Option<Scaled>,
    role: Role,
    target: Option<Target>,
    /// Logical size, as configured.
    size: (u32, u32),
    scale: f32,
    configured: bool,
    dirty: bool,
    /// A frame callback is pending: the compositor has not asked for the
    /// next frame yet.
    waiting: bool,
    moving: bool,
    last_time: Option<f64>,
    /// When the last frame was shown, and the refresh period then, seconds.
    presented: Option<f64>,
    refresh: Option<f64>,
    fps: f64,
    frames: u64,
    /// CPU time of the last frame, seconds.
    cpu: f64,
    /// Where it takes the pointer, as last set: None for all of it.
    input: Option<Vec<Rect>>,
    /// What each of the last few frames drew, buffer pixels, newest last.
    history: Vec<Rect>,
    /// The buffer size the last frame was drawn at.
    drawn: (i32, i32),
    /// When a wake is set for it, monotonic seconds.
    wake_at: Option<f64>,
}

enum Role {
    Layer(LayerSurface),
    Window(Window),
}

impl<K> Surface<K> {
    fn wl(&self) -> &wl_surface::WlSurface {
        match &self.role {
            Role::Layer(layer) => layer.wl_surface(),
            Role::Window(window) => window.wl_surface(),
        }
    }

    fn is_layer(&self, layer: &LayerSurface) -> bool {
        matches!(&self.role, Role::Layer(own) if own == layer)
    }

    fn is_window(&self, window: &Window) -> bool {
        matches!(&self.role, Role::Window(own) if own == window)
    }

    fn buffer_size(&self) -> (i32, i32) {
        let (w, h) = self.size;
        (
            (w as f32 * self.scale).round() as i32,
            (h as f32 * self.scale).round() as i32,
        )
    }
}

/// What the shell can ask of the process.
pub struct Ui<S: Shell> {
    pub(crate) handle: LoopHandle<'static, App<S>>,
    pub(crate) qh: QueueHandle<App<S>>,
    pub(crate) clipboard: crate::clipboard::Clipboard,
    /// The last input event's serial: what lets us set the clipboard.
    pub(crate) serial: u32,
    registry: RegistryState,
    output_state: OutputState,
    seat: SeatState,
    pointer: Option<wl_pointer::WlPointer>,
    keyboard: Option<wl_keyboard::WlKeyboard>,
    /// The surface with the keyboard.
    focus: Option<S::Key>,
    mods: Mods,
    compositor: CompositorState,
    layer_shell: Option<LayerShell>,
    xdg: Option<XdgShell>,
    scaling: Option<Scaling>,
    presentation: Option<PresentationTimeState>,
    gpu: Gpu,
    painter: Painter,
    text: Text,
    icons: Icons,
    /// The atlas generation the frames so far were drawn with.
    atlas_generation: u64,
    outputs: Vec<(Output, wl_output::WlOutput)>,
    surfaces: Vec<Surface<S::Key>>,
    /// Surfaces are to be matched with what the shell wants.
    refresh: bool,
    sender: channel::Sender<S::Message>,
    stats: Stats,
    /// How much larger than its own logical pixels everything is drawn.
    zoom: f32,
    quit: bool,
}

impl<S: Shell> Ui<S> {
    /// `key` has changed and needs a frame.
    pub fn dirty(&mut self, key: S::Key) {
        if let Some(surface) = self.surfaces.iter_mut().find(|s| s.key == key) {
            surface.dirty = true;
        }
    }

    pub fn dirty_all(&mut self) {
        for surface in &mut self.surfaces {
            surface.dirty = true;
        }
    }

    /// What the shell wants shown has changed: ask it again.
    pub fn refresh_surfaces(&mut self) {
        self.refresh = true;
    }

    pub fn outputs(&self) -> Vec<Output> {
        self.outputs.iter().map(|(output, _)| output.clone()).collect()
    }

    /// For services: a way to send the shell its messages from anywhere.
    pub fn sender(&self) -> channel::Sender<S::Message> {
        self.sender.clone()
    }

    /// For services that are loop sources: sockets, timers.
    pub fn handle(&self) -> LoopHandle<'static, App<S>> {
        self.handle.clone()
    }

    pub fn stats(&self) -> Stats {
        self.stats
    }

    pub fn surface_info(&self) -> Vec<SurfaceInfo> {
        self.surfaces
            .iter()
            .map(|s| SurfaceInfo {
                key: format!("{:?}", s.key),
                size: s.size,
                scale: s.scale,
                frames: s.frames,
                waiting: s.waiting,
                moving: s.moving,
                refresh: s.refresh.map_or(0.0, |r| 1.0 / r),
                fps: s.fps,
            })
            .collect()
    }

    /// Marks `key` dirty `after` this long: a delay with no frames in it,
    /// as a tooltip's before it shows.
    pub fn wake_after(&mut self, key: S::Key, after: std::time::Duration) {
        let timer = calloop::timer::Timer::from_duration(after);
        let _ = self.handle.insert_source(timer, move |_, _, app| {
            app.ui.dirty(key);
            calloop::timer::TimeoutAction::Drop
        });
    }

    /// Everything drawn `zoom` times larger (or smaller): layout sees a
    /// smaller (or larger) surface, painting a finer scale.
    pub fn set_zoom(&mut self, zoom: f32) {
        self.zoom = zoom.clamp(0.5, 3.0);
        self.dirty_all();
    }

    pub fn zoom(&self) -> f32 {
        self.zoom
    }

    pub fn quit(&mut self) {
        self.quit = true;
    }

    pub(crate) fn set_scale(&mut self, wl: &wl_surface::WlSurface, scale: f32) {
        if let Some(surface) = self.surfaces.iter_mut().find(|s| s.wl() == wl)
            && surface.scale != scale
        {
            tracing::info!(key = ?surface.key, scale, "scale");
            surface.scale = scale;
            surface.dirty = true;
        }
    }

    fn output_changed(&mut self, wl: &wl_output::WlOutput, gone: bool) {
        let id = OutputId(wl.id().protocol_id());
        let old = self
            .outputs
            .iter()
            .position(|(output, _)| output.id == id)
            .map(|at| self.outputs.remove(at).0);
        if !gone && let Some(info) = self.output_state.info(wl) {
            let output = Output {
                id,
                name: info.name.clone().unwrap_or_default(),
                description: info.description.clone().unwrap_or_default(),
                size: info.logical_size.unwrap_or((0, 0)),
            };
            if old.as_ref() != Some(&output) {
                tracing::info!(name = %output.name, size = ?output.size, "output");
                self.refresh = true;
            }
            self.outputs.push((output, wl.clone()));
        } else if gone {
            tracing::info!(?id, "output gone");
            self.refresh = true;
        }
    }

    /// Matches the surfaces with what the shell wants: new ones made, gone
    /// ones dropped, changed ones changed.
    fn reconcile(&mut self, shell: &S) {
        self.refresh = false;
        let outputs = self.outputs();
        let wanted = shell.surfaces(&outputs);
        let mut index = 0;
        while index < self.surfaces.len() {
            let surface = &mut self.surfaces[index];
            let changed_in_place = match (wanted.iter().find(|(key, _)| *key == surface.key), &surface.role) {
                (Some((_, spec)), _) if *spec == surface.spec => true,
                (Some((_, Spec::Layer(new))), Role::Layer(layer)) if same_place(&surface.spec, new) => {
                    apply_layer(layer, new);
                    layer.commit();
                    surface.spec = Spec::Layer(new.clone());
                    true
                }
                (Some((_, Spec::Window(new))), Role::Window(window)) => {
                    window.set_title(new.title.clone());
                    window.set_app_id(new.app_id.clone());
                    surface.spec = Spec::Window(new.clone());
                    true
                }
                _ => false,
            };
            if changed_in_place {
                index += 1;
            } else {
                let gone = self.surfaces.remove(index);
                self.drop_surface(gone);
            }
        }
        for (key, spec) in wanted {
            if self.surfaces.iter().any(|s| s.key == key) {
                continue;
            }
            match self.make(key, &spec) {
                Ok(surface) => self.surfaces.push(surface),
                Err(error) => tracing::warn!(?key, %error, "no surface"),
            }
        }
    }

    fn make(&self, key: S::Key, spec: &Spec) -> Result<Surface<S::Key>, String> {
        let wl = self.compositor.create_surface(&self.qh);
        let scaled = self.scaling.as_ref().map(|scaling| scaling.attach(&wl, &self.qh));
        let (role, size) = match spec {
            Spec::Layer(layer_spec) => {
                let layer_shell = self.layer_shell.as_ref().ok_or("the compositor has no layer shell")?;
                let output = self
                    .outputs
                    .iter()
                    .find(|(output, _)| output.id == layer_spec.output)
                    .map(|(_, wl)| wl)
                    .ok_or("no such output")?;
                let layer = layer_shell.create_layer_surface(
                    &self.qh,
                    wl,
                    layer_spec.layer,
                    Some(layer_spec.namespace.clone()),
                    Some(output),
                );
                apply_layer(&layer, layer_spec);
                if layer_spec.through {
                    // No input region at all; the protocol copies it, so it can go at once.
                    let none = smithay_client_toolkit::compositor::Region::new(&self.compositor)
                        .map_err(|error| error.to_string())?;
                    layer.wl_surface().set_input_region(Some(none.wl_region()));
                }
                layer.commit();
                (Role::Layer(layer), layer_spec.size)
            }
            Spec::Window(window_spec) => {
                let xdg = self.xdg.as_ref().ok_or("the compositor has no xdg shell")?;
                let window = xdg.create_window(wl, WindowDecorations::RequestServer, &self.qh);
                window.set_title(window_spec.title.clone());
                window.set_app_id(window_spec.app_id.clone());
                window.commit();
                (Role::Window(window), window_spec.size)
            }
        };
        Ok(Surface {
            key,
            spec: spec.clone(),
            scaled,
            role,
            target: None,
            size,
            scale: 1.0,
            configured: false,
            dirty: true,
            waiting: false,
            moving: false,
            last_time: None,
            presented: None,
            refresh: None,
            fps: 0.0,
            input: None,
            history: Vec::new(),
            drawn: (0, 0),
            wake_at: None,
            frames: 0,
            cpu: 0.0,
        })
    }

    fn drop_surface(&mut self, surface: Surface<S::Key>) {
        if let Some(target) = surface.target {
            self.gpu.drop_target(target);
        }
        tracing::debug!(key = ?surface.key, "surface dropped");
    }

    /// Draws every surface that needs it and may: dirty or moving, and not
    /// waiting for the compositor.
    fn render(&mut self, shell: &mut S) {
        if self.refresh {
            self.reconcile(shell);
        }
        for index in 0..self.surfaces.len() {
            let surface = &self.surfaces[index];
            if !surface.configured || surface.waiting || !(surface.dirty || surface.moving) {
                continue;
            }
            if let Err(error) = self.draw(shell, index) {
                tracing::warn!(key = ?self.surfaces[index].key, %error, "frame failed");
            }
        }
        // The atlas started over: what any buffer shows of it is gone, so
        // every surface draws all of itself again.
        if self.painter.atlas.generation != self.atlas_generation {
            self.atlas_generation = self.painter.atlas.generation;
            tracing::info!("the atlas started over");
            for surface in &mut self.surfaces {
                surface.dirty = true;
                surface.drawn = (0, 0);
            }
        }
    }

    fn draw(&mut self, shell: &mut S, index: usize) -> Result<(), String> {
        let started = std::time::Instant::now();
        let surface = &mut self.surfaces[index];
        let now = frame_time(monotonic(), surface.presented, surface.refresh);
        let key = surface.key;
        // Update.
        let dt = match (surface.moving, surface.last_time) {
            (true, Some(last)) => (now - last).clamp(0.0, 0.1),
            _ => 0.0,
        };
        let time = FrameTime {
            now,
            dt,
            fps: surface.fps,
            refresh: surface.refresh.map_or(0.0, |period| 1.0 / period),
            cpu: surface.cpu,
            frames: surface.frames,
        };
        surface.moving = shell.update(key, &time);
        surface.last_time = Some(now);
        // Layout.
        let view = View {
            width: surface.size.0 as f32 / self.zoom,
            height: surface.size.1 as f32 / self.zoom,
            scale: surface.scale * self.zoom,
        };
        shell.layout(key, &view, &mut self.text);
        let asked = shell.damage(key);
        // Render.
        let (width, height) = surface.buffer_size();
        match &surface.target {
            Some(target) => self.gpu.resize(target, width, height),
            None => surface.target = Some(self.gpu.target(surface.wl().id(), width, height)?),
        }
        let target = surface.target.as_ref().expect("made above");
        self.gpu.bind(target)?;
        // Only what changed, where the shell says what that is and the buffer
        // is one whose age is known: this frame's damage and that of the
        // frames the buffer missed. All of it after a resize.
        let whole = Rect::new(0.0, 0.0, width as f32, height as f32);
        let changed = match asked {
            Some(rects) if surface.drawn == (width, height) => Some(
                rects
                    .iter()
                    .fold(Rect::default(), |all, r| all.union(&r.covering(view.scale))),
            ),
            _ => None,
        };
        let age = self.gpu.buffer_age(target) as usize;
        let region = changed
            .filter(|_| age >= 1 && age - 1 <= surface.history.len())
            .map(|changed| {
                surface
                    .history
                    .iter()
                    .rev()
                    .take(age - 1)
                    .fold(changed, |all, r| all.union(r))
            });
        surface.history.push(changed.unwrap_or(whole));
        if surface.history.len() > 4 {
            surface.history.remove(0);
        }
        surface.drawn = (width, height);
        let mut frame = Frame {
            painter: &mut self.painter,
            text: &mut self.text,
            icons: &mut self.icons,
            view,
            offset: (0.0, 0.0),
            alpha: 1.0,
            clipped: None,
        };
        shell.render(key, &mut frame);
        self.painter.flush(&self.gpu.gl, width, height, region);
        // Where it takes the pointer, if the shell says, set before the frame
        // commits it.
        let input = shell.input_region(key);
        if input != surface.input && (input.is_some() || surface.input.is_some()) {
            match &input {
                Some(rects) => {
                    if let Ok(region) = smithay_client_toolkit::compositor::Region::new(&self.compositor) {
                        for rect in rects {
                            let zoomed = |v: f32| (v * self.zoom).round() as i32;
                            region.add(zoomed(rect.x), zoomed(rect.y), zoomed(rect.w), zoomed(rect.h));
                        }
                        surface.wl().set_input_region(Some(region.wl_region()));
                    }
                }
                None => surface.wl().set_input_region(None),
            }
            surface.input = input;
        }
        if let Some(scaled) = &surface.scaled {
            scaled
                .viewport
                .set_destination(surface.size.0 as i32, surface.size.1 as i32);
        }
        if let Some(presentation) = &self.presentation {
            let _ = presentation.feedback(surface.wl(), &self.qh);
        }
        // The next frame is the compositor's to ask for.
        surface.wl().frame(&self.qh, FrameCallbackData(surface.wl().clone()));
        surface.waiting = true;
        surface.dirty = false;
        let region: Vec<Rect> = region.into_iter().filter(|r| !r.is_empty()).collect();
        if !region.is_empty() {
            self.stats.partial += 1;
        }
        self.gpu.present(target, &region, height)?;
        if surface.frames == 0 {
            tracing::info!(key = ?key, width, height, scale = surface.scale, "first frame");
        }
        surface.frames += 1;
        self.stats.frames += 1;
        // A wake it asks for, unless an earlier one is set already.
        if let Some(after) = shell.wake(key) {
            let at = monotonic() + after.as_secs_f64();
            if surface.wake_at.is_none_or(|set| set > at + 0.002) {
                surface.wake_at = Some(at);
                let timer = calloop::timer::Timer::from_duration(after);
                let _ = self.handle.insert_source(timer, move |_, _, app| {
                    if let Some(surface) = app.ui.surfaces.iter_mut().find(|s| s.key == key) {
                        surface.wake_at = None;
                        surface.dirty = true;
                    }
                    calloop::timer::TimeoutAction::Drop
                });
            }
        }
        surface.cpu = started.elapsed().as_secs_f64();
        self.stats.last_frame = surface.cpu;
        Ok(())
    }
}

/// Whether a layer can change into `new` in place: same output and layer.
fn same_place(old: &Spec, new: &LayerSpec) -> bool {
    matches!(old, Spec::Layer(old) if old.output == new.output && old.layer == new.layer && old.namespace == new.namespace)
}

fn apply_layer(layer: &LayerSurface, spec: &LayerSpec) {
    layer.set_anchor(spec.anchor);
    layer.set_size(spec.size.0, spec.size.1);
    let (top, right, bottom, left) = spec.margin;
    layer.set_margin(top, right, bottom, left);
    layer.set_exclusive_zone(spec.exclusive_zone);
    layer.set_keyboard_interactivity(spec.keyboard);
}

/// The time a frame made `now` will likely be shown: the first vblank after
/// now, counted from the last presented frame. Motion stepped by these moves
/// in even steps, whatever the CPU was doing. Plain `now` where nothing was
/// presented lately.
fn frame_time(now: f64, presented: Option<f64>, refresh: Option<f64>) -> f64 {
    match (presented, refresh) {
        (Some(shown), Some(period)) if period > 0.0 && now >= shown && now - shown < 1.0 => {
            shown + ((now - shown) / period).ceil().max(1.0) * period
        }
        _ => now,
    }
}

/// Seconds of CLOCK_MONOTONIC, the clock the compositor's timestamps use.
pub fn monotonic() -> f64 {
    let now = rustix::time::clock_gettime(rustix::time::ClockId::Monotonic);
    now.tv_sec as f64 + now.tv_nsec as f64 * 1e-9
}

/// The shell and the process's side of Wayland, together.
pub struct App<S: Shell> {
    pub shell: S,
    pub ui: Ui<S>,
}

/// Runs `shell` until it quits. `start` runs once everything is up, to
/// start the shell's services.
pub fn run<S: Shell>(shell: S, start: impl FnOnce(&mut App<S>) -> Result<(), String>) -> Result<(), String> {
    let conn = Connection::connect_to_env().map_err(|error| format!("no Wayland: {error}"))?;
    let (globals, queue) = registry_queue_init::<App<S>>(&conn).map_err(|error| error.to_string())?;
    let qh = queue.handle();
    let mut event_loop: EventLoop<'static, App<S>> = EventLoop::try_new().map_err(|error| error.to_string())?;
    WaylandSource::new(conn.clone(), queue)
        .insert(event_loop.handle())
        .map_err(|error| error.to_string())?;
    // SAFETY: the connection outlives the GPU, both owned until the end here.
    let gpu = unsafe { Gpu::new(conn.backend().display_ptr().cast())? };
    tracing::info!(renderer = %gpu.renderer(), "drawing");
    let painter = Painter::new(&gpu.gl)?;
    let (sender, messages) = channel::channel::<S::Message>();
    event_loop
        .handle()
        .insert_source(messages, |event, _, app| {
            if let channel::Event::Msg(message) = event {
                app.shell.message(&mut app.ui, message);
            }
        })
        .map_err(|error| error.to_string())?;
    let ui = Ui {
        handle: event_loop.handle(),
        clipboard: crate::clipboard::Clipboard::new(
            smithay_client_toolkit::data_device_manager::DataDeviceManagerState::bind(&globals, &qh).ok(),
        ),
        serial: 0,
        registry: RegistryState::new(&globals),
        output_state: OutputState::new(&globals, &qh),
        seat: SeatState::new(&globals, &qh),
        pointer: None,
        keyboard: None,
        focus: None,
        mods: Mods::default(),
        compositor: CompositorState::bind(&globals, &qh).map_err(|error| error.to_string())?,
        layer_shell: LayerShell::bind(&globals, &qh).ok(),
        xdg: XdgShell::bind(&globals, &qh).ok(),
        scaling: Scaling::bind(&globals, &qh),
        presentation: Some(PresentationTimeState::bind(&globals, &qh)),
        qh,
        gpu,
        painter,
        text: Text::new(),
        icons: Icons::default(),
        atlas_generation: 0,
        outputs: Vec::new(),
        surfaces: Vec::new(),
        refresh: true,
        sender,
        stats: Stats::default(),
        zoom: 1.0,
        quit: false,
    };
    let mut app = App { shell, ui };
    start(&mut app)?;
    let signal = event_loop.get_signal();
    event_loop
        .run(None, &mut app, |app| {
            app.ui.stats.wakeups += 1;
            app.ui.render(&mut app.shell);
            if app.ui.quit {
                signal.stop();
            }
        })
        .map_err(|error| error.to_string())
}

impl<S: Shell> CompositorHandler for App<S> {
    fn scale_factor_changed(&mut self, _: &Connection, _: &QueueHandle<Self>, wl: &wl_surface::WlSurface, factor: i32) {
        // Only where nothing finer says it.
        if let Some(surface) = self.ui.surfaces.iter_mut().find(|s| s.wl() == wl && s.scaled.is_none()) {
            surface.scale = factor.max(1) as f32;
            wl.set_buffer_scale(factor.max(1));
            surface.dirty = true;
        }
    }

    fn transform_changed(
        &mut self,
        _: &Connection,
        _: &QueueHandle<Self>,
        _: &wl_surface::WlSurface,
        _: wl_output::Transform,
    ) {
    }

    fn frame(&mut self, _: &Connection, _: &QueueHandle<Self>, wl: &wl_surface::WlSurface, _: u32) {
        if let Some(surface) = self.ui.surfaces.iter_mut().find(|s| s.wl() == wl) {
            surface.waiting = false;
        }
    }

    fn surface_enter(
        &mut self,
        _: &Connection,
        _: &QueueHandle<Self>,
        _: &wl_surface::WlSurface,
        _: &wl_output::WlOutput,
    ) {
    }

    fn surface_leave(
        &mut self,
        _: &Connection,
        _: &QueueHandle<Self>,
        _: &wl_surface::WlSurface,
        _: &wl_output::WlOutput,
    ) {
    }
}

impl<S: Shell> OutputHandler for App<S> {
    fn output_state(&mut self) -> &mut OutputState {
        &mut self.ui.output_state
    }

    fn new_output(&mut self, _: &Connection, _: &QueueHandle<Self>, output: wl_output::WlOutput) {
        self.ui.output_changed(&output, false);
    }

    fn update_output(&mut self, _: &Connection, _: &QueueHandle<Self>, output: wl_output::WlOutput) {
        self.ui.output_changed(&output, false);
    }

    fn output_destroyed(&mut self, _: &Connection, _: &QueueHandle<Self>, output: wl_output::WlOutput) {
        self.ui.output_changed(&output, true);
    }
}

impl<S: Shell> LayerShellHandler for App<S> {
    fn closed(&mut self, _: &Connection, _: &QueueHandle<Self>, layer: &LayerSurface) {
        if let Some(index) = self.ui.surfaces.iter().position(|s| s.is_layer(layer)) {
            let gone = self.ui.surfaces.remove(index);
            tracing::info!(key = ?gone.key, "closed by the compositor");
            self.ui.drop_surface(gone);
        }
    }

    fn configure(
        &mut self,
        _: &Connection,
        _: &QueueHandle<Self>,
        layer: &LayerSurface,
        configure: LayerSurfaceConfigure,
        _: u32,
    ) {
        if let Some(surface) = self.ui.surfaces.iter_mut().find(|s| s.is_layer(layer)) {
            let (w, h) = configure.new_size;
            if let Spec::Layer(spec) = &surface.spec {
                surface.size = (
                    if w == 0 { spec.size.0 } else { w },
                    if h == 0 { spec.size.1 } else { h },
                );
            }
            surface.configured = true;
            surface.dirty = true;
        }
    }
}

impl<S: Shell> ProvidesRegistryState for App<S> {
    fn registry(&mut self) -> &mut RegistryState {
        &mut self.ui.registry
    }
    registry_handlers![OutputState, SeatState];
}

smithay_client_toolkit::delegate_registry!(@<S: Shell> App<S>);
smithay_client_toolkit::delegate_dispatch2!(@<S: Shell> App<S>);

impl<S: Shell> PresentationTimeHandler for App<S> {
    fn presentation_time_state(&mut self) -> &mut PresentationTimeState {
        self.ui.presentation.as_mut().expect("bound at start")
    }

    fn presented(
        &mut self,
        _: &Connection,
        _: &QueueHandle<Self>,
        _: &wp_presentation_feedback::WpPresentationFeedback,
        wl: &wl_surface::WlSurface,
        _: Vec<wl_output::WlOutput>,
        time: PresentTime,
        refresh: u32,
        _: u64,
        _: WEnum<wp_presentation_feedback::Kind>,
    ) {
        if let Some(surface) = self.ui.surfaces.iter_mut().find(|s| s.wl() == wl) {
            let shown = time.tv_sec as f64 + time.tv_nsec as f64 * 1e-9;
            if let Some(before) = surface.presented {
                let gap = shown - before;
                if gap > 0.0 {
                    // Smoothed a little: one late frame should not swing it.
                    let rate = 1.0 / gap;
                    surface.fps = if surface.fps == 0.0 {
                        rate
                    } else {
                        surface.fps * 0.8 + rate * 0.2
                    };
                }
            }
            surface.presented = Some(shown);
            surface.refresh = (refresh > 0).then_some(refresh as f64 * 1e-9);
        }
    }

    fn discarded(
        &mut self,
        _: &Connection,
        _: &QueueHandle<Self>,
        _: &wp_presentation_feedback::WpPresentationFeedback,
        _: &wl_surface::WlSurface,
    ) {
    }
}

impl<S: Shell> WindowHandler for App<S> {
    fn request_close(&mut self, _: &Connection, _: &QueueHandle<Self>, window: &Window) {
        if let Some(key) = self.ui.surfaces.iter().find(|s| s.is_window(window)).map(|s| s.key) {
            self.shell.closed(&mut self.ui, key);
        }
    }

    fn configure(
        &mut self,
        _: &Connection,
        _: &QueueHandle<Self>,
        window: &Window,
        configure: WindowConfigure,
        _: u32,
    ) {
        if let Some(surface) = self.ui.surfaces.iter_mut().find(|s| s.is_window(window)) {
            let (w, h) = configure.new_size;
            surface.size = (
                w.map_or(surface.size.0, |w| w.get()),
                h.map_or(surface.size.1, |h| h.get()),
            );
            surface.configured = true;
            surface.dirty = true;
        }
    }
}

impl<S: Shell> SeatHandler for App<S> {
    fn seat_state(&mut self) -> &mut SeatState {
        &mut self.ui.seat
    }

    fn new_seat(&mut self, _: &Connection, _: &QueueHandle<Self>, _: wl_seat::WlSeat) {}

    fn new_capability(
        &mut self,
        _: &Connection,
        qh: &QueueHandle<Self>,
        seat: wl_seat::WlSeat,
        capability: Capability,
    ) {
        if capability == Capability::Pointer && self.ui.pointer.is_none() {
            self.ui.pointer = self.ui.seat.get_pointer(qh, &seat).ok();
        }
        self.ui.clipboard.seat(&seat, qh);
        if capability == Capability::Keyboard && self.ui.keyboard.is_none() {
            // Repeats at the compositor's rate and delay, timed on the loop.
            let repeat = Box::new(|app: &mut App<S>, _: &wl_keyboard::WlKeyboard, event: KeyEvent| {
                app.key(event, KeyState::Repeated);
            });
            self.ui.keyboard = self
                .ui
                .seat
                .get_keyboard_with_repeat(qh, &seat, None, self.ui.handle.clone(), repeat)
                .ok();
        }
    }

    fn remove_capability(&mut self, _: &Connection, _: &QueueHandle<Self>, _: wl_seat::WlSeat, capability: Capability) {
        if capability == Capability::Pointer
            && let Some(pointer) = self.ui.pointer.take()
        {
            pointer.release();
        }
        if capability == Capability::Keyboard
            && let Some(keyboard) = self.ui.keyboard.take()
        {
            keyboard.release();
        }
    }

    fn remove_seat(&mut self, _: &Connection, _: &QueueHandle<Self>, _: wl_seat::WlSeat) {}
}

impl<S: Shell> PointerHandler for App<S> {
    fn pointer_frame(
        &mut self,
        _: &Connection,
        _: &QueueHandle<Self>,
        _: &wl_pointer::WlPointer,
        events: &[PointerEvent],
    ) {
        for event in events {
            let Some(key) = self
                .ui
                .surfaces
                .iter()
                .find(|s| s.wl() == &event.surface)
                .map(|s| s.key)
            else {
                continue;
            };
            let zoom = self.ui.zoom;
            let (x, y) = (event.position.0 as f32 / zoom, event.position.1 as f32 / zoom);
            let input = match event.kind {
                PointerEventKind::Enter { .. } => Input::PointerEnter { x, y },
                PointerEventKind::Leave { .. } => Input::PointerLeave,
                PointerEventKind::Motion { .. } => Input::PointerMove { x, y },
                PointerEventKind::Press { button, serial, .. } => {
                    self.ui.serial = serial;
                    Input::Press { button, x, y }
                }
                PointerEventKind::Release { button, .. } => Input::Release { button, x, y },
                // A frame with neither motion nor fingers lifted carries nothing.
                PointerEventKind::Axis {
                    horizontal, vertical, ..
                } if horizontal.is_none() && vertical.is_none() => continue,
                PointerEventKind::Axis {
                    horizontal,
                    vertical,
                    source,
                    ..
                } => {
                    let source = match source {
                        Some(wl_pointer::AxisSource::Finger) => ScrollSource::Finger,
                        Some(wl_pointer::AxisSource::Continuous) => ScrollSource::Continuous,
                        Some(wl_pointer::AxisSource::WheelTilt) => ScrollSource::WheelTilt,
                        _ => ScrollSource::Wheel,
                    };
                    Input::Scroll(Scroll {
                        x: horizontal.absolute as f32,
                        y: vertical.absolute as f32,
                        steps_x: steps(horizontal.value120, horizontal.discrete),
                        steps_y: steps(vertical.value120, vertical.discrete),
                        source,
                        stop: horizontal.stop || vertical.stop,
                    })
                }
            };
            self.shell.input(&mut self.ui, key, &input);
        }
    }
}

/// Wheel steps from `value120`, or from the older whole `discrete` ones.
fn steps(value120: i32, discrete: i32) -> f32 {
    if value120 != 0 {
        value120 as f32 / 120.0
    } else {
        discrete as f32
    }
}

impl<S: Shell> KeyboardHandler for App<S> {
    #[allow(clippy::too_many_arguments)]
    fn enter(
        &mut self,
        _: &Connection,
        _: &QueueHandle<Self>,
        _: &wl_keyboard::WlKeyboard,
        wl: &wl_surface::WlSurface,
        _: u32,
        _: &[u32],
        _: &[Keysym],
    ) {
        self.ui.focus = self.ui.surfaces.iter().find(|s| s.wl() == wl).map(|s| s.key);
        if let Some(key) = self.ui.focus {
            self.shell.input(&mut self.ui, key, &Input::KeyboardEnter);
        }
    }

    fn leave(
        &mut self,
        _: &Connection,
        _: &QueueHandle<Self>,
        _: &wl_keyboard::WlKeyboard,
        _: &wl_surface::WlSurface,
        _: u32,
    ) {
        if let Some(key) = self.ui.focus.take() {
            self.shell.input(&mut self.ui, key, &Input::KeyboardLeave);
        }
    }

    fn press_key(
        &mut self,
        _: &Connection,
        _: &QueueHandle<Self>,
        _: &wl_keyboard::WlKeyboard,
        serial: u32,
        event: KeyEvent,
    ) {
        self.ui.serial = serial;
        self.key(event, KeyState::Pressed);
    }

    fn repeat_key(
        &mut self,
        _: &Connection,
        _: &QueueHandle<Self>,
        _: &wl_keyboard::WlKeyboard,
        _: u32,
        event: KeyEvent,
    ) {
        self.key(event, KeyState::Repeated);
    }

    fn release_key(
        &mut self,
        _: &Connection,
        _: &QueueHandle<Self>,
        _: &wl_keyboard::WlKeyboard,
        _: u32,
        event: KeyEvent,
    ) {
        self.key(event, KeyState::Released);
    }

    #[allow(clippy::too_many_arguments)]
    fn update_modifiers(
        &mut self,
        _: &Connection,
        _: &QueueHandle<Self>,
        _: &wl_keyboard::WlKeyboard,
        _: u32,
        modifiers: Modifiers,
        _: RawModifiers,
        _: u32,
    ) {
        self.ui.mods = Mods {
            ctrl: modifiers.ctrl,
            alt: modifiers.alt,
            shift: modifiers.shift,
            logo: modifiers.logo,
            caps_lock: modifiers.caps_lock,
            num_lock: modifiers.num_lock,
        };
    }
}

impl<S: Shell> App<S> {
    fn key(&mut self, event: KeyEvent, state: KeyState) {
        let Some(key) = self.ui.focus else { return };
        let input = Input::Key(Key {
            keysym: crate::input::plain(event.keysym),
            code: event.raw_code,
            text: event
                .utf8
                .filter(|text| !text.is_empty() && !text.chars().any(char::is_control)),
            state,
            mods: self.ui.mods,
        });
        self.shell.input(&mut self.ui, key, &input);
    }
}

#[cfg(test)]
mod tests {
    use super::frame_time;

    #[test]
    fn frames_land_on_the_next_vblank() {
        let period = 1.0 / 240.0;
        // Just after a frame was shown: the next one, one period on.
        let t = frame_time(10.0 + period * 0.1, Some(10.0), Some(period));
        assert!((t - (10.0 + period)).abs() < 1e-9);
        // Late by two and a half periods: the third vblank.
        let t = frame_time(10.0 + period * 2.5, Some(10.0), Some(period));
        assert!((t - (10.0 + 3.0 * period)).abs() < 1e-9);
        // Long idle: plain now.
        assert_eq!(frame_time(20.0, Some(10.0), Some(period)), 20.0);
        assert_eq!(frame_time(5.0, None, None), 5.0);
    }
}
