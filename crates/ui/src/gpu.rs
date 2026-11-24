//! One EGL display and one GLES 3 context for every surface of the process:
//! a surface costs only its own buffers, never a context of its own.

use std::ffi::c_void;

use khronos_egl as egl;

use crate::geometry::Rect;

type Egl = egl::DynamicInstance<egl::EGL1_5>;

const PLATFORM_WAYLAND: egl::Enum = 0x31D8;
const BUFFER_AGE: egl::Int = 0x313D;

type SwapWithDamage = unsafe extern "C" fn(egl::EGLDisplay, egl::EGLSurface, *const egl::Int, egl::Int) -> egl::Boolean;

pub struct Gpu {
    egl: Egl,
    display: egl::Display,
    config: egl::Config,
    context: egl::Context,
    pub gl: glow::Context,
    /// Whether a buffer tells how many frames old its contents are.
    buffer_age: bool,
    swap_with_damage: Option<SwapWithDamage>,
}

/// A surface's own part: its EGL window surface over a `wl_egl_window`.
pub struct Target {
    surface: egl::Surface,
    window: wayland_egl::WlEglSurface,
}

/// Has libglvnd offer only Mesa's EGL. Mesa draws on the GPU the compositor
/// draws on, while NVIDIA's EGL, loaded first otherwise, wakes a sleeping
/// discrete GPU only by being loaded. A vendor list of one's own, or
/// `YOKOSO_GPU=any`, is left alone. Mesa's own threads go too: one context
/// draws little.
///
/// Called first thing in `main`, before any thread exists, as it changes the
/// environment.
pub fn prefer_mesa() {
    let set = |name: &str, value: &str| {
        if std::env::var_os(name).is_none() {
            // SAFETY: called before the process has any other thread.
            unsafe { std::env::set_var(name, value) };
        }
    };
    set("mesa_glthread", "false");
    set("GALLIUM_THREAD", "0");
    if std::env::var("YOKOSO_GPU").as_deref() == Ok("any") {
        return;
    }
    let dirs = std::env::var("__EGL_VENDOR_LIBRARY_DIRS")
        .map(|dirs| dirs.split(':').map(str::to_owned).collect::<Vec<_>>())
        .unwrap_or_default()
        .into_iter()
        .chain(
            [
                "/run/opengl-driver/share/glvnd/egl_vendor.d",
                "/etc/glvnd/egl_vendor.d",
                "/usr/share/glvnd/egl_vendor.d",
            ]
            .map(str::to_owned),
        );
    for dir in dirs {
        let Ok(entries) = std::fs::read_dir(&dir) else { continue };
        let mut found: Vec<_> = entries
            .flatten()
            .map(|entry| entry.path())
            .filter(|path| {
                path.file_name()
                    .and_then(|name| name.to_str())
                    .is_some_and(|name| name.contains("mesa") && name.ends_with(".json"))
            })
            .collect();
        found.sort();
        if let Some(path) = found.first() {
            set("__EGL_VENDOR_LIBRARY_FILENAMES", &path.to_string_lossy());
            return;
        }
    }
}

impl Gpu {
    /// On the Wayland display `display`, a `wl_display *`.
    ///
    /// # Safety
    /// `display` must be a live `wl_display` for as long as this lives.
    pub unsafe fn new(display: *mut c_void) -> Result<Self, String> {
        let egl = unsafe { Egl::load_required() }.map_err(|error| format!("no EGL: {error}"))?;
        let display = unsafe { egl.get_platform_display(PLATFORM_WAYLAND, display, &[egl::ATTRIB_NONE]) }
            .map_err(|error| format!("no EGL display: {error}"))?;
        egl.initialize(display)
            .map_err(|error| format!("EGL won't start: {error}"))?;
        egl.bind_api(egl::OPENGL_ES_API).map_err(|error| error.to_string())?;
        let wanted = [
            egl::RED_SIZE,
            8,
            egl::GREEN_SIZE,
            8,
            egl::BLUE_SIZE,
            8,
            egl::ALPHA_SIZE,
            8,
            egl::SURFACE_TYPE,
            egl::WINDOW_BIT,
            egl::RENDERABLE_TYPE,
            egl::OPENGL_ES3_BIT,
            egl::NONE,
        ];
        let config = egl
            .choose_first_config(display, &wanted)
            .map_err(|error| error.to_string())?
            .ok_or("no EGL config with alpha")?;
        let context = egl
            .create_context(
                display,
                config,
                None,
                &[egl::CONTEXT_MAJOR_VERSION, 3, egl::CONTEXT_MINOR_VERSION, 0, egl::NONE],
            )
            .map_err(|error| format!("no GLES 3 context: {error}"))?;
        // No surface yet: Mesa has EGL_KHR_surfaceless_context.
        egl.make_current(display, None, None, Some(context))
            .map_err(|error| error.to_string())?;
        let gl = unsafe {
            glow::Context::from_loader_function(|name| {
                egl.get_proc_address(name)
                    .map_or(std::ptr::null(), |pointer| pointer as *const c_void)
            })
        };
        let extensions = egl
            .query_string(Some(display), egl::EXTENSIONS)
            .map(|found| found.to_string_lossy().into_owned())
            .unwrap_or_default();
        let has = |name: &str| extensions.split_whitespace().any(|found| found == name);
        let swap_with_damage = ["EGL_KHR_swap_buffers_with_damage", "EGL_EXT_swap_buffers_with_damage"]
            .into_iter()
            .find(|name| has(name))
            .and_then(|name| egl.get_proc_address(&format!("eglSwapBuffersWithDamage{}", &name[4..7])))
            // SAFETY: the extension names this function with this signature.
            .map(|found| unsafe { std::mem::transmute::<extern "system" fn(), SwapWithDamage>(found) });
        Ok(Self {
            buffer_age: has("EGL_EXT_buffer_age"),
            swap_with_damage,
            egl,
            display,
            config,
            context,
            gl,
        })
    }

    /// What draws, as GL_RENDERER names it: for the log.
    pub fn renderer(&self) -> String {
        unsafe { glow::HasContext::get_parameter_string(&self.gl, glow::RENDERER) }
    }

    /// A target for the `wl_surface` with `id`, `width` by `height` buffer pixels.
    pub fn target(&self, id: wayland_client::backend::ObjectId, width: i32, height: i32) -> Result<Target, String> {
        let window =
            wayland_egl::WlEglSurface::new(id, width.max(1), height.max(1)).map_err(|error| error.to_string())?;
        let surface = unsafe {
            self.egl
                .create_window_surface(self.display, self.config, window.ptr() as egl::NativeWindowType, None)
        }
        .map_err(|error| format!("no EGL surface: {error}"))?;
        Ok(Target { surface, window })
    }

    pub fn resize(&self, target: &Target, width: i32, height: i32) {
        target.window.resize(width.max(1), height.max(1), 0, 0);
    }

    /// Draws into `target` from here on. Nothing waits for vblank: frames go
    /// by the compositor's frame callbacks.
    pub fn bind(&self, target: &Target) -> Result<(), String> {
        self.egl
            .make_current(
                self.display,
                Some(target.surface),
                Some(target.surface),
                Some(self.context),
            )
            .map_err(|error| error.to_string())?;
        self.egl
            .swap_interval(self.display, 0)
            .map_err(|error| error.to_string())
    }

    /// How many frames old the bound buffer's contents are: 0 for unknown,
    /// when everything has to be drawn.
    pub fn buffer_age(&self, target: &Target) -> u32 {
        if !self.buffer_age {
            return 0;
        }
        self.egl
            .query_surface(self.display, target.surface, BUFFER_AGE)
            .map_or(0, |age| age.max(0) as u32)
    }

    /// Shows the frame, telling the compositor that only `damage` changed,
    /// in buffer pixels from the top left of a buffer `height` high; all of it
    /// with an empty `damage`.
    pub fn present(&self, target: &Target, damage: &[Rect], height: i32) -> Result<(), String> {
        if let (Some(swap), false) = (self.swap_with_damage, damage.is_empty()) {
            // EGL counts from the bottom left.
            let rects: Vec<egl::Int> = damage
                .iter()
                .flat_map(|r| [r.x as i32, height - r.bottom() as i32, r.w as i32, r.h as i32])
                .collect();
            let done = unsafe {
                swap(
                    self.display.as_ptr(),
                    target.surface.as_ptr(),
                    rects.as_ptr(),
                    damage.len() as egl::Int,
                )
            };
            return if done == egl::TRUE {
                Ok(())
            } else {
                Err("swap with damage failed".into())
            };
        }
        self.egl
            .swap_buffers(self.display, target.surface)
            .map_err(|error| error.to_string())
    }

    pub fn drop_target(&self, target: Target) {
        let _ = self.egl.make_current(self.display, None, None, Some(self.context));
        let _ = self.egl.destroy_surface(self.display, target.surface);
    }
}
