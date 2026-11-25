//! Pictures: small images of the world outside the shell's own icons, as a
//! tray item's or a theme's icon. Kept premultiplied RGBA, drawn through
//! the atlas, scaled to where they go.

use std::{
    path::Path,
    sync::{
        Arc,
        atomic::{AtomicU64, Ordering},
    },
};

/// Larger than this, a picture is made smaller as it is read: the atlas is
/// for small things.
pub const LARGEST: u32 = 256;

/// A picture in memory. Cheap to clone; each new one has its own id, by
/// which the atlas knows it.
#[derive(Clone, Debug)]
pub struct Picture {
    pub width: u32,
    pub height: u32,
    /// Premultiplied RGBA, a row after another.
    pub pixels: Arc<Vec<u8>>,
    pub id: u64,
}

impl PartialEq for Picture {
    fn eq(&self, other: &Self) -> bool {
        self.width == other.width && self.height == other.height && self.pixels == other.pixels
    }
}

fn next_id() -> u64 {
    static NEXT: AtomicU64 = AtomicU64::new(1);
    NEXT.fetch_add(1, Ordering::Relaxed)
}

impl Picture {
    /// From premultiplied RGBA.
    pub fn from_rgba(width: u32, height: u32, pixels: Vec<u8>) -> Option<Self> {
        if width == 0 || height == 0 || pixels.len() != (width * height * 4) as usize {
            return None;
        }
        Some(Self {
            width,
            height,
            pixels: Arc::new(pixels),
            id: next_id(),
        })
    }

    /// From ARGB of 32 bits a pixel, big-endian, straight alpha: as a tray
    /// item sends its pixmaps.
    pub fn from_argb32_be(width: u32, height: u32, bytes: &[u8]) -> Option<Self> {
        if bytes.len() != (width * height * 4) as usize {
            return None;
        }
        let pixels = bytes
            .as_chunks::<4>()
            .0
            .iter()
            .flat_map(|p| {
                let a = u32::from(p[0]);
                let premultiply = |c: u8| ((u32::from(c) * a + 127) / 255) as u8;
                [premultiply(p[1]), premultiply(p[2]), premultiply(p[3]), p[0]]
            })
            .collect();
        Self::from_rgba(width, height, pixels)
    }

    /// From a PNG file's bytes.
    pub fn from_png(bytes: &[u8]) -> Option<Self> {
        let pixmap = resvg::tiny_skia::Pixmap::decode_png(bytes).ok()?;
        Self::from_pixmap(pixmap)
    }

    /// From an SVG's text, drawn `size` pixels on its longer side.
    pub fn from_svg(text: &[u8], size: u32) -> Option<Self> {
        let tree = resvg::usvg::Tree::from_data(text, &resvg::usvg::Options::default()).ok()?;
        let given = tree.size();
        let scale = size as f32 / given.width().max(given.height()).max(1.0);
        let width = (given.width() * scale).round().max(1.0) as u32;
        let height = (given.height() * scale).round().max(1.0) as u32;
        let mut pixmap = resvg::tiny_skia::Pixmap::new(width, height)?;
        resvg::render(
            &tree,
            resvg::tiny_skia::Transform::from_scale(scale, scale),
            &mut pixmap.as_mut(),
        );
        Self::from_pixmap(pixmap)
    }

    /// From a PNG or an SVG file, an SVG drawn `size` pixels.
    pub fn load(path: &Path, size: u32) -> Option<Self> {
        let bytes = std::fs::read(path).ok()?;
        match path.extension().and_then(|e| e.to_str()) {
            Some("svg") => Self::from_svg(&bytes, size.min(LARGEST)),
            Some("png") => Self::from_png(&bytes),
            _ => None,
        }
    }

    /// The same picture `width` by `height`: halved by averaging while it is
    /// twice as large or more, then filtered to the size. A picture shrunk by
    /// the GPU at once loses its thin lines to the sampling.
    pub fn resized(&self, width: u32, height: u32) -> Option<Self> {
        use resvg::tiny_skia::{FilterQuality, Pixmap, PixmapPaint, Transform};
        if width == 0 || height == 0 {
            return None;
        }
        let (mut w, mut h) = (self.width, self.height);
        let mut pixels = self.pixels.to_vec();
        while w >= 2 * width && h >= 2 * height {
            pixels = halved(&pixels, w, h);
            w /= 2;
            h /= 2;
        }
        if (w, h) == (width, height) {
            return Self::from_rgba(w, h, pixels);
        }
        let source = Pixmap::from_vec(pixels, resvg::tiny_skia::IntSize::from_wh(w, h)?)?;
        let mut out = Pixmap::new(width, height)?;
        let paint = PixmapPaint {
            quality: FilterQuality::Bicubic,
            ..PixmapPaint::default()
        };
        let scale = Transform::from_scale(width as f32 / w as f32, height as f32 / h as f32);
        out.draw_pixmap(0, 0, source.as_ref(), &paint, scale, None);
        Self::from_rgba(width, height, out.take())
    }

    fn from_pixmap(pixmap: resvg::tiny_skia::Pixmap) -> Option<Self> {
        let (width, height) = (pixmap.width(), pixmap.height());
        if width > LARGEST || height > LARGEST {
            return None;
        }
        Self::from_rgba(width, height, pixmap.take())
    }
}

/// `pixels`, `w` by `h` RGBA, half as large each way: each pixel the mean
/// of the four it stands for.
fn halved(pixels: &[u8], w: u32, h: u32) -> Vec<u8> {
    let (half_w, half_h) = (w / 2, h / 2);
    let at = |x: u32, y: u32, c: usize| u32::from(pixels[((y * w + x) * 4) as usize + c]);
    let mut out = Vec::with_capacity((half_w * half_h * 4) as usize);
    for y in 0..half_h {
        for x in 0..half_w {
            for c in 0..4 {
                let sum = at(2 * x, 2 * y, c)
                    + at(2 * x + 1, 2 * y, c)
                    + at(2 * x, 2 * y + 1, c)
                    + at(2 * x + 1, 2 * y + 1, c);
                out.push(((sum + 2) / 4) as u8);
            }
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_picture_shrinks_by_halves_then_to_its_size() {
        // 4 by 4: a white left half, a clear right half.
        let pixels: Vec<u8> = (0..16)
            .flat_map(|i| if i % 4 < 2 { [255; 4] } else { [0; 4] })
            .collect();
        let picture = Picture::from_rgba(4, 4, pixels).unwrap();
        let half = picture.resized(2, 2).unwrap();
        assert_eq!(
            *half.pixels,
            [255, 255, 255, 255, 0, 0, 0, 0, 255, 255, 255, 255, 0, 0, 0, 0]
        );
        let odd = picture.resized(3, 3).unwrap();
        assert_eq!((odd.width, odd.height), (3, 3));
        assert!(picture.resized(0, 2).is_none());
    }

    #[test]
    fn a_tray_pixmap_turns_into_premultiplied_rgba() {
        // Two pixels: opaque red, half-transparent white.
        let picture = Picture::from_argb32_be(2, 1, &[255, 255, 0, 0, 128, 255, 255, 255]).unwrap();
        assert_eq!(*picture.pixels, vec![255, 0, 0, 255, 128, 128, 128, 128]);
        assert!(Picture::from_argb32_be(2, 2, &[0; 8]).is_none(), "too few bytes");
    }

    #[test]
    fn an_svg_is_drawn_at_the_size_asked() {
        let svg = br##"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 16 8"><rect width="16" height="8" fill="#fff"/></svg>"##;
        let picture = Picture::from_svg(svg, 32).unwrap();
        assert_eq!((picture.width, picture.height), (32, 16));
        assert_eq!(picture.pixels[3], 255);
        // Each picture its own id; the same pixels are the same picture.
        let again = Picture::from_svg(svg, 32).unwrap();
        assert_ne!(picture.id, again.id);
        assert_eq!(picture, again);
    }
}
