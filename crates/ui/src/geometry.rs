//! Rectangles and colors. Layout works in logical pixels; only painting turns
//! them into buffer pixels, snapped to whole ones.

/// A rectangle: its top left corner and its size.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Rect {
    pub x: f32,
    pub y: f32,
    pub w: f32,
    pub h: f32,
}

impl Rect {
    pub const fn new(x: f32, y: f32, w: f32, h: f32) -> Self {
        Self { x, y, w, h }
    }

    pub fn right(&self) -> f32 {
        self.x + self.w
    }

    pub fn bottom(&self) -> f32 {
        self.y + self.h
    }

    pub fn is_empty(&self) -> bool {
        self.w <= 0.0 || self.h <= 0.0
    }

    pub fn contains(&self, x: f32, y: f32) -> bool {
        x >= self.x && x < self.right() && y >= self.y && y < self.bottom()
    }

    /// The smallest rectangle holding both; an empty one counts for nothing.
    pub fn union(&self, other: &Rect) -> Rect {
        if self.is_empty() {
            return *other;
        }
        if other.is_empty() {
            return *self;
        }
        let x = self.x.min(other.x);
        let y = self.y.min(other.y);
        Rect::new(
            x,
            y,
            self.right().max(other.right()) - x,
            self.bottom().max(other.bottom()) - y,
        )
    }

    /// Where both are; empty where they don't meet.
    pub fn intersect(&self, other: &Rect) -> Rect {
        let x = self.x.max(other.x);
        let y = self.y.max(other.y);
        let w = self.right().min(other.right()) - x;
        let h = self.bottom().min(other.bottom()) - y;
        if w <= 0.0 || h <= 0.0 {
            Rect::default()
        } else {
            Rect::new(x, y, w, h)
        }
    }

    /// `by` pixels larger on every side; smaller with a negative `by`.
    pub fn grow(&self, by: f32) -> Rect {
        Rect::new(self.x - by, self.y - by, self.w + 2.0 * by, self.h + 2.0 * by)
    }

    /// In buffer pixels at `scale`, its edges on whole pixels: two rects that
    /// touch in logical pixels touch in buffer ones too, with no gap.
    pub fn snapped(&self, scale: f32) -> Rect {
        let x = (self.x * scale).round();
        let y = (self.y * scale).round();
        Rect::new(
            x,
            y,
            (self.right() * scale).round() - x,
            (self.bottom() * scale).round() - y,
        )
    }

    /// The same at `scale`, outward to whole pixels: for damage, which must
    /// never cut a pixel in half.
    pub fn covering(&self, scale: f32) -> Rect {
        let x = (self.x * scale).floor();
        let y = (self.y * scale).floor();
        Rect::new(
            x,
            y,
            (self.right() * scale).ceil() - x,
            (self.bottom() * scale).ceil() - y,
        )
    }
}

/// A color, straight (not premultiplied) RGBA, each 0 to 1.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Color(pub [f32; 4]);

impl Color {
    pub const TRANSPARENT: Color = Color([0.0; 4]);
    pub const BLACK: Color = Color([0.0, 0.0, 0.0, 1.0]);
    pub const WHITE: Color = Color([1.0; 4]);

    pub const fn rgb(r: f32, g: f32, b: f32) -> Self {
        Self([r, g, b, 1.0])
    }

    /// "#rrggbb" or "#rrggbbaa".
    pub fn hex(text: &str) -> Option<Self> {
        let digits = text.strip_prefix('#')?;
        if !matches!(digits.len(), 6 | 8) || !digits.bytes().all(|b| b.is_ascii_hexdigit()) {
            return None;
        }
        let channel = |at: usize| {
            u8::from_str_radix(&digits[at..at + 2], 16)
                .ok()
                .map(|v| v as f32 / 255.0)
        };
        let alpha = if digits.len() == 8 { channel(6)? } else { 1.0 };
        Some(Self([channel(0)?, channel(2)?, channel(4)?, alpha]))
    }

    pub fn alpha(self, alpha: f32) -> Self {
        let [r, g, b, a] = self.0;
        Self([r, g, b, a * alpha])
    }

    /// Between `self` and `other`, `at` of the way.
    pub fn mix(self, other: Color, at: f32) -> Self {
        let mut out = [0.0; 4];
        for (index, value) in out.iter_mut().enumerate() {
            *value = self.0[index] + (other.0[index] - self.0[index]) * at;
        }
        Self(out)
    }

    /// Premultiplied, as the GPU blends it.
    pub fn premultiplied(self) -> [f32; 4] {
        let [r, g, b, a] = self.0;
        [r * a, g * a, b * a, a]
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn touching_rects_stay_touching_at_a_fractional_scale() {
        let left = Rect::new(0.0, 0.0, 10.3, 5.0).snapped(1.25);
        let right = Rect::new(10.3, 0.0, 7.0, 5.0).snapped(1.25);
        assert_eq!(left.right(), right.x);
    }

    #[test]
    fn covering_never_cuts_a_pixel() {
        let r = Rect::new(0.4, 0.4, 1.2, 1.2).covering(1.5);
        assert_eq!((r.x, r.y, r.right(), r.bottom()), (0.0, 0.0, 3.0, 3.0));
    }

    #[test]
    fn union_skips_empty_and_intersect_of_apart_is_empty() {
        let a = Rect::new(0.0, 0.0, 2.0, 2.0);
        let b = Rect::new(5.0, 5.0, 1.0, 1.0);
        assert_eq!(a.union(&Rect::default()), a);
        assert_eq!(a.union(&b), Rect::new(0.0, 0.0, 6.0, 6.0));
        assert!(a.intersect(&b).is_empty());
    }

    #[test]
    fn hex_reads_both_lengths_and_refuses_the_rest() {
        assert_eq!(Color::hex("#ff0000"), Some(Color([1.0, 0.0, 0.0, 1.0])));
        assert_eq!(Color::hex("#00000000"), Some(Color::TRANSPARENT));
        assert_eq!(Color::hex("ff0000"), None);
        assert_eq!(Color::hex("#ff00"), None);
        assert_eq!(Color::hex("#gg0000"), None);
    }
}
