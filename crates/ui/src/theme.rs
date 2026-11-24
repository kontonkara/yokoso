//! The theme: Material 3 color roles made from a palette, and the tokens for
//! sizes, spacing, radii and type. Elements take every color and size from
//! here and hold none of their own.

use material_colors::{
    color::Rgb,
    dynamic_color::{DynamicScheme, MaterialDynamicColors as Roles},
    hct::Hct,
    scheme::variant::{
        SchemeContent, SchemeExpressive, SchemeFidelity, SchemeMonochrome, SchemeNeutral, SchemeTonalSpot,
        SchemeVibrant,
    },
};

use crate::{geometry::Color, motion::Motion};

/// How Material 3 turns a seed color into a scheme.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Variant {
    #[default]
    TonalSpot,
    Vibrant,
    Expressive,
    Neutral,
    Monochrome,
    Fidelity,
    Content,
}

impl Variant {
    pub const ALL: [Variant; 7] = [
        Variant::TonalSpot,
        Variant::Vibrant,
        Variant::Expressive,
        Variant::Neutral,
        Variant::Monochrome,
        Variant::Fidelity,
        Variant::Content,
    ];

    pub fn name(self) -> &'static str {
        match self {
            Variant::TonalSpot => "tonal spot",
            Variant::Vibrant => "vibrant",
            Variant::Expressive => "expressive",
            Variant::Neutral => "neutral",
            Variant::Monochrome => "monochrome",
            Variant::Fidelity => "fidelity",
            Variant::Content => "content",
        }
    }
}

/// Where the colors come from.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Palette {
    pub seed: Color,
    pub variant: Variant,
    pub dark: bool,
}

impl Default for Palette {
    /// The peach chosen on the mockup, dark.
    fn default() -> Self {
        Self {
            seed: Color::hex("#f3a17a").expect("a color"),
            variant: Variant::TonalSpot,
            dark: true,
        }
    }
}

/// Material 3's color roles.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Colors {
    pub surface: Color,
    pub surface_container_lowest: Color,
    pub surface_container_low: Color,
    pub surface_container: Color,
    pub surface_container_high: Color,
    pub surface_container_highest: Color,
    pub on_surface: Color,
    pub on_surface_variant: Color,
    pub primary: Color,
    pub on_primary: Color,
    pub primary_container: Color,
    pub on_primary_container: Color,
    pub secondary: Color,
    pub on_secondary: Color,
    pub secondary_container: Color,
    pub on_secondary_container: Color,
    pub tertiary: Color,
    pub on_tertiary: Color,
    pub tertiary_container: Color,
    pub on_tertiary_container: Color,
    pub error: Color,
    pub on_error: Color,
    pub error_container: Color,
    pub on_error_container: Color,
    pub outline: Color,
    pub outline_variant: Color,
    pub inverse_surface: Color,
    pub inverse_on_surface: Color,
    pub inverse_primary: Color,
    pub shadow: Color,
    pub scrim: Color,
}

impl Colors {
    pub fn from(palette: &Palette) -> Self {
        let [r, g, b, _] = palette.seed.0;
        let byte = |v: f32| (v.clamp(0.0, 1.0) * 255.0).round() as u8;
        let hct = Hct::new(Rgb {
            red: byte(r),
            green: byte(g),
            blue: byte(b),
        });
        let dark = palette.dark;
        let scheme: DynamicScheme = match palette.variant {
            Variant::TonalSpot => SchemeTonalSpot::new(hct, dark, None).scheme,
            Variant::Vibrant => SchemeVibrant::new(hct, dark, None).scheme,
            Variant::Expressive => SchemeExpressive::new(hct, dark, None).scheme,
            Variant::Neutral => SchemeNeutral::new(hct, dark, None).scheme,
            Variant::Monochrome => SchemeMonochrome::new(hct, dark, None).scheme,
            Variant::Fidelity => SchemeFidelity::new(hct, dark, None).scheme,
            Variant::Content => SchemeContent::new(hct, dark, None).scheme,
        };
        let c = |role| {
            let rgb = scheme.get_rgb(role);
            Color([
                f32::from(rgb.red) / 255.0,
                f32::from(rgb.green) / 255.0,
                f32::from(rgb.blue) / 255.0,
                1.0,
            ])
        };
        Self {
            surface: c(Roles::surface()),
            surface_container_lowest: c(Roles::surface_container_lowest()),
            surface_container_low: c(Roles::surface_container_low()),
            surface_container: c(Roles::surface_container()),
            surface_container_high: c(Roles::surface_container_high()),
            surface_container_highest: c(Roles::surface_container_highest()),
            on_surface: c(Roles::on_surface()),
            on_surface_variant: c(Roles::on_surface_variant()),
            primary: c(Roles::primary()),
            on_primary: c(Roles::on_primary()),
            primary_container: c(Roles::primary_container()),
            on_primary_container: c(Roles::on_primary_container()),
            secondary: c(Roles::secondary()),
            on_secondary: c(Roles::on_secondary()),
            secondary_container: c(Roles::secondary_container()),
            on_secondary_container: c(Roles::on_secondary_container()),
            tertiary: c(Roles::tertiary()),
            on_tertiary: c(Roles::on_tertiary()),
            tertiary_container: c(Roles::tertiary_container()),
            on_tertiary_container: c(Roles::on_tertiary_container()),
            error: c(Roles::error()),
            on_error: c(Roles::on_error()),
            error_container: c(Roles::error_container()),
            on_error_container: c(Roles::on_error_container()),
            outline: c(Roles::outline()),
            outline_variant: c(Roles::outline_variant()),
            inverse_surface: c(Roles::inverse_surface()),
            inverse_on_surface: c(Roles::inverse_on_surface()),
            inverse_primary: c(Roles::inverse_primary()),
            shadow: c(Roles::shadow()),
            scrim: c(Roles::scrim()),
        }
    }
}

/// How much space things take.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Density {
    Compact,
    #[default]
    Normal,
    Comfortable,
}

/// Sizes, in logical pixels.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Sizes {
    /// Cards, panels, the bar: as niri's windows by default.
    pub radius: f32,
    /// What sits inside them: rows, tiles, fields.
    pub radius_inner: f32,
    /// Between things side by side.
    pub gap: f32,
    /// Inside a card or a panel, at its edge.
    pub pad: f32,
    /// A button's, a field's, a row's height.
    pub control: f32,
    pub icon: f32,
    pub focus_ring: f32,
    /// Type.
    pub body: f32,
    pub label: f32,
    pub small: f32,
    pub title: f32,
    pub headline: f32,
}

impl Sizes {
    pub fn new(density: Density) -> Self {
        let (gap, pad, control) = match density {
            Density::Compact => (8.0, 12.0, 32.0),
            Density::Normal => (12.0, 16.0, 40.0),
            Density::Comfortable => (16.0, 22.0, 48.0),
        };
        Self {
            radius: 8.0,
            radius_inner: 4.0,
            gap,
            pad,
            control,
            icon: 20.0,
            focus_ring: 2.0,
            body: 14.0,
            label: 13.0,
            small: 12.0,
            title: 16.0,
            headline: 22.0,
        }
    }
}

/// How strongly a state shows on a color, as Material 3 has it.
pub mod state {
    pub const HOVER: f32 = 0.08;
    pub const FOCUS: f32 = 0.10;
    pub const PRESS: f32 = 0.10;
    pub const DRAG: f32 = 0.16;
    /// A disabled element's content, and its container.
    pub const DISABLED_CONTENT: f32 = 0.38;
    pub const DISABLED_CONTAINER: f32 = 0.12;
}

#[derive(Clone, Debug, PartialEq)]
pub struct Theme {
    pub palette: Palette,
    pub colors: Colors,
    pub density: Density,
    pub sizes: Sizes,
    pub motion: Motion,
}

impl Default for Theme {
    fn default() -> Self {
        Self::new(Palette::default(), Density::default(), Motion::default())
    }
}

impl Theme {
    pub fn new(palette: Palette, density: Density, motion: Motion) -> Self {
        Self {
            colors: Colors::from(&palette),
            sizes: Sizes::new(density),
            palette,
            density,
            motion,
        }
    }

    /// `layer` over `base`, `amount` of it: hover and press on a color.
    pub fn over(base: Color, layer: Color, amount: f32) -> Color {
        let mixed = base.mix(Color([layer.0[0], layer.0[1], layer.0[2], base.0[3]]), amount);
        Color([mixed.0[0], mixed.0[1], mixed.0[2], base.0[3]])
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn luminance(color: Color) -> f32 {
        let channel = |c: f32| {
            if c <= 0.04045 {
                c / 12.92
            } else {
                ((c + 0.055) / 1.055).powf(2.4)
            }
        };
        let [r, g, b, _] = color.0;
        0.2126 * channel(r) + 0.7152 * channel(g) + 0.0722 * channel(b)
    }

    fn contrast(a: Color, b: Color) -> f32 {
        let (a, b) = (luminance(a), luminance(b));
        (a.max(b) + 0.05) / (a.min(b) + 0.05)
    }

    #[test]
    fn dark_is_dark_and_light_is_light() {
        let dark = Colors::from(&Palette::default());
        let light = Colors::from(&Palette {
            dark: false,
            ..Palette::default()
        });
        assert!(luminance(dark.surface) < 0.05);
        assert!(luminance(light.surface) > 0.8);
    }

    #[test]
    fn what_is_written_on_a_role_reads_on_it() {
        for variant in Variant::ALL {
            for dark in [true, false] {
                let colors = Colors::from(&Palette {
                    variant,
                    dark,
                    ..Palette::default()
                });
                for (on, under) in [
                    (colors.on_surface, colors.surface),
                    (colors.on_primary, colors.primary),
                    (colors.on_primary_container, colors.primary_container),
                    (colors.on_secondary_container, colors.secondary_container),
                ] {
                    assert!(contrast(on, under) >= 4.5, "{variant:?} dark={dark}");
                }
            }
        }
    }

    #[test]
    fn density_changes_spacing_not_radii() {
        let compact = Sizes::new(Density::Compact);
        let comfy = Sizes::new(Density::Comfortable);
        assert!(compact.gap < comfy.gap && compact.control < comfy.control);
        assert_eq!(compact.radius, comfy.radius);
    }
}
