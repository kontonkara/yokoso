//! The process's side of Wayland: the outputs, a surface for each thing the
//! shell wants shown, and frames. A surface draws only when it is dirty or
//! something on it moves, and only once the compositor has asked for a frame;
//! idle is zero frames.

use smithay_client_toolkit::{
    compositor::{CompositorHandler, CompositorState, FrameCallbackData},
    output::{OutputHandler, OutputState},
    reexports::{
        calloop::{EventLoop, LoopHandle, channel},
        calloop_wayland_source::WaylandSource,
        client::{
            Connection, Proxy, QueueHandle,
            globals::registry_queue_init,
            protocol::{wl_output, wl_surface},
        },
    },
    registry::{ProvidesRegistryState, RegistryState},
    registry_handlers,
    shell::{
        WaylandSurface,
        wlr_layer::{LayerShell, LayerShellHandler, LayerSurface, LayerSurfaceConfigure},
    },
};

pub use smithay_client_toolkit::shell::wlr_layer::{Anchor, KeyboardInteractivity, Layer};

use crate::{
    geometry::{Color, Rect},
    gpu::{Gpu, Target},
    paint::Painter,
    scale::{Scaled, Scaling},
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

/// What kind of surface the shell wants, and where.
#[derive(Clone, Debug, PartialEq)]
pub enum Spec {
    Layer(LayerSpec),
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

    /// Update: advances `key`'s state to `time`. True while something on it
    /// still moves.
    fn update(&mut self, _key: Self::Key, _time: &FrameTime) -> bool {
        false
    }

    /// Layout: sizes and positions for `view`.
    fn layout(&mut self, _key: Self::Key, _view: &View) {}

    /// Render: paints `key`. The state is only read here.
    fn render(&self, key: Self::Key, frame: &mut Frame);
}

/// Painting a surface, in its logical pixels.
pub struct Frame<'a> {
    painter: &'a mut Painter,
    view: View,
}

impl Frame<'_> {
    pub fn view(&self) -> View {
        self.view
    }

    fn snap(&self, rect: Rect) -> Rect {
        rect.snapped(self.view.scale)
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

    /// What is painted from here on shows only in `rect`, or everywhere.
    pub fn clip(&mut self, rect: Option<Rect>) {
        let rect = rect.map(|rect| self.snap(rect));
        self.painter.clip(rect);
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
}

/// Counts of what the process did, for the debug overlay and `msg stats`.
#[derive(Clone, Copy, Debug, Default)]
pub struct Stats {
    /// Times the event loop woke.
    pub wakeups: u64,
    pub frames: u64,
    /// CPU time of the last frame, in seconds.
    pub last_frame: f64,
}

struct Surface<K> {
    key: K,
    spec: Spec,
    // Before the layer: dropped before its surface is.
    scaled: Option<Scaled>,
    layer: LayerSurface,
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
    frames: u64,
}

impl<K> Surface<K> {
    fn wl(&self) -> &wl_surface::WlSurface {
        self.layer.wl_surface()
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
    handle: LoopHandle<'static, App<S>>,
    qh: QueueHandle<App<S>>,
    registry: RegistryState,
    output_state: OutputState,
    compositor: CompositorState,
    layer_shell: Option<LayerShell>,
    scaling: Option<Scaling>,
    gpu: Gpu,
    painter: Painter,
    outputs: Vec<(Output, wl_output::WlOutput)>,
    surfaces: Vec<Surface<S::Key>>,
    /// Surfaces are to be matched with what the shell wants.
    refresh: bool,
    sender: channel::Sender<S::Message>,
    stats: Stats,
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
            })
            .collect()
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
        self.outputs.retain(|(output, _)| output.id != id);
        if !gone && let Some(info) = self.output_state.info(wl) {
            let output = Output {
                id,
                name: info.name.clone().unwrap_or_default(),
                description: info.description.clone().unwrap_or_default(),
                size: info.logical_size.unwrap_or((0, 0)),
            };
            tracing::info!(name = %output.name, size = ?output.size, "output");
            self.outputs.push((output, wl.clone()));
        } else if gone {
            tracing::info!(?id, "output gone");
        }
        self.refresh = true;
    }

    /// Matches the surfaces with what the shell wants: new ones made, gone
    /// ones dropped, changed ones changed.
    fn reconcile(&mut self, shell: &S) {
        self.refresh = false;
        let outputs = self.outputs();
        let wanted = shell.surfaces(&outputs);
        let mut index = 0;
        while index < self.surfaces.len() {
            let surface = &self.surfaces[index];
            match wanted.iter().find(|(key, _)| *key == surface.key) {
                Some((_, spec)) if *spec == surface.spec => index += 1,
                Some((_, Spec::Layer(new))) if same_place(&surface.spec, new) => {
                    let surface = &mut self.surfaces[index];
                    apply_layer(&surface.layer, new);
                    surface.spec = Spec::Layer(new.clone());
                    surface.layer.commit();
                    index += 1;
                }
                _ => {
                    let gone = self.surfaces.remove(index);
                    self.drop_surface(gone);
                }
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
        let Spec::Layer(layer_spec) = spec;
        let layer_shell = self.layer_shell.as_ref().ok_or("the compositor has no layer shell")?;
        let output = self
            .outputs
            .iter()
            .find(|(output, _)| output.id == layer_spec.output)
            .map(|(_, wl)| wl)
            .ok_or("no such output")?;
        let wl = self.compositor.create_surface(&self.qh);
        let layer = layer_shell.create_layer_surface(
            &self.qh,
            wl,
            layer_spec.layer,
            Some(layer_spec.namespace.clone()),
            Some(output),
        );
        apply_layer(&layer, layer_spec);
        let scaled = self
            .scaling
            .as_ref()
            .map(|scaling| scaling.attach(layer.wl_surface(), &self.qh));
        layer.commit();
        Ok(Surface {
            key,
            spec: spec.clone(),
            scaled,
            layer,
            target: None,
            size: layer_spec.size,
            scale: 1.0,
            configured: false,
            dirty: true,
            waiting: false,
            moving: false,
            last_time: None,
            frames: 0,
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
    }

    fn draw(&mut self, shell: &mut S, index: usize) -> Result<(), String> {
        let started = std::time::Instant::now();
        let now = monotonic();
        let surface = &mut self.surfaces[index];
        let key = surface.key;
        // Update.
        let dt = match (surface.moving, surface.last_time) {
            (true, Some(last)) => (now - last).clamp(0.0, 0.1),
            _ => 0.0,
        };
        surface.moving = shell.update(key, &FrameTime { now, dt });
        surface.last_time = Some(now);
        // Layout.
        let view = View {
            width: surface.size.0 as f32,
            height: surface.size.1 as f32,
            scale: surface.scale,
        };
        shell.layout(key, &view);
        // Render.
        let (width, height) = surface.buffer_size();
        match &surface.target {
            Some(target) => self.gpu.resize(target, width, height),
            None => surface.target = Some(self.gpu.target(surface.wl().id(), width, height)?),
        }
        let target = surface.target.as_ref().expect("made above");
        self.gpu.bind(target)?;
        let mut frame = Frame {
            painter: &mut self.painter,
            view,
        };
        shell.render(key, &mut frame);
        self.painter.flush(&self.gpu.gl, width, height, None);
        if let Some(scaled) = &surface.scaled {
            scaled
                .viewport
                .set_destination(surface.size.0 as i32, surface.size.1 as i32);
        }
        // The next frame is the compositor's to ask for.
        surface.wl().frame(&self.qh, FrameCallbackData(surface.wl().clone()));
        surface.waiting = true;
        surface.dirty = false;
        self.gpu.present(target, &[], height)?;
        if surface.frames == 0 {
            tracing::info!(key = ?key, width, height, scale = surface.scale, "first frame");
        }
        surface.frames += 1;
        self.stats.frames += 1;
        self.stats.last_frame = started.elapsed().as_secs_f64();
        Ok(())
    }
}

/// Whether a layer can change into `new` in place: same output and layer.
fn same_place(old: &Spec, new: &LayerSpec) -> bool {
    let Spec::Layer(old) = old;
    old.output == new.output && old.layer == new.layer && old.namespace == new.namespace
}

fn apply_layer(layer: &LayerSurface, spec: &LayerSpec) {
    layer.set_anchor(spec.anchor);
    layer.set_size(spec.size.0, spec.size.1);
    let (top, right, bottom, left) = spec.margin;
    layer.set_margin(top, right, bottom, left);
    layer.set_exclusive_zone(spec.exclusive_zone);
    layer.set_keyboard_interactivity(spec.keyboard);
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
        registry: RegistryState::new(&globals),
        output_state: OutputState::new(&globals, &qh),
        compositor: CompositorState::bind(&globals, &qh).map_err(|error| error.to_string())?,
        layer_shell: LayerShell::bind(&globals, &qh).ok(),
        scaling: Scaling::bind(&globals, &qh),
        qh,
        gpu,
        painter,
        outputs: Vec::new(),
        surfaces: Vec::new(),
        refresh: true,
        sender,
        stats: Stats::default(),
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
        if let Some(index) = self.ui.surfaces.iter().position(|s| &s.layer == layer) {
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
        if let Some(surface) = self.ui.surfaces.iter_mut().find(|s| &s.layer == layer) {
            let (w, h) = configure.new_size;
            let Spec::Layer(spec) = &surface.spec;
            surface.size = (
                if w == 0 { spec.size.0 } else { w },
                if h == 0 { spec.size.1 } else { h },
            );
            surface.configured = true;
            surface.dirty = true;
        }
    }
}

impl<S: Shell> ProvidesRegistryState for App<S> {
    fn registry(&mut self) -> &mut RegistryState {
        &mut self.ui.registry
    }
    registry_handlers![OutputState];
}

smithay_client_toolkit::delegate_registry!(@<S: Shell> App<S>);
smithay_client_toolkit::delegate_dispatch2!(@<S: Shell> App<S>);
