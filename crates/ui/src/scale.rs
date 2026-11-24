//! Scales between whole numbers, as 1.25 or 1.5: the compositor says which
//! one a surface is on, its buffer is drawn that many pixels to a logical one,
//! and the viewport shows it at its logical size. Where the compositor has
//! neither protocol, the surface goes by its output's whole scale.

use smithay_client_toolkit::{
    dispatch2::Dispatch2,
    reexports::{
        client::{Connection, Proxy, QueueHandle, globals::GlobalList, protocol::wl_surface::WlSurface},
        protocols::wp::{
            fractional_scale::v1::client::{
                wp_fractional_scale_manager_v1::WpFractionalScaleManagerV1,
                wp_fractional_scale_v1::{self, WpFractionalScaleV1},
            },
            viewporter::client::{wp_viewport::WpViewport, wp_viewporter::WpViewporter},
        },
    },
};

use crate::app::{App, Shell};

/// The two globals, where the compositor has them.
pub struct Scaling {
    manager: WpFractionalScaleManagerV1,
    viewporter: WpViewporter,
}

/// What a surface has of them, dropped with it.
pub struct Scaled {
    fraction: WpFractionalScaleV1,
    pub viewport: WpViewport,
}

impl Drop for Scaled {
    fn drop(&mut self) {
        self.fraction.destroy();
        self.viewport.destroy();
    }
}

/// For objects with no events.
pub struct Silent;

/// For a surface's fractional scale: which surface it is.
pub struct ScaleOf(WlSurface);

impl Scaling {
    pub fn bind<S: Shell>(globals: &GlobalList, qh: &QueueHandle<App<S>>) -> Option<Self> {
        let manager = globals.bind(qh, 1..=1, Silent).ok()?;
        let viewporter = globals.bind(qh, 1..=1, Silent).ok()?;
        Some(Self { manager, viewporter })
    }

    pub fn attach<S: Shell>(&self, surface: &WlSurface, qh: &QueueHandle<App<S>>) -> Scaled {
        Scaled {
            fraction: self.manager.get_fractional_scale(surface, qh, ScaleOf(surface.clone())),
            viewport: self.viewporter.get_viewport(surface, qh, Silent),
        }
    }
}

macro_rules! silent {
    ($($interface:ty),*) => {$(
        impl<S: Shell> Dispatch2<$interface, App<S>> for Silent {
            fn event(
                &self,
                _: &mut App<S>,
                _: &$interface,
                _: <$interface as Proxy>::Event,
                _: &Connection,
                _: &QueueHandle<App<S>>,
            ) {
            }
        }
    )*};
}

silent!(WpFractionalScaleManagerV1, WpViewporter, WpViewport);

impl<S: Shell> Dispatch2<WpFractionalScaleV1, App<S>> for ScaleOf {
    fn event(
        &self,
        app: &mut App<S>,
        _: &WpFractionalScaleV1,
        event: wp_fractional_scale_v1::Event,
        _: &Connection,
        _: &QueueHandle<App<S>>,
    ) {
        if let wp_fractional_scale_v1::Event::PreferredScale { scale } = event {
            app.ui.set_scale(&self.0, scale as f32 / 120.0);
        }
    }
}
