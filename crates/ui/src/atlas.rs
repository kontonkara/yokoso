//! Glyphs and icons in one RGBA texture, packed on shelves. A mask keeps its
//! coverage in alpha over white; a colored glyph keeps its own colors,
//! premultiplied. Nothing of it stays on the CPU: what is added waits only
//! for the next upload.
//!
//! When it is full it starts over: everything in it is forgotten at once and
//! drawn into it again as it is asked for.

use glow::HasContext;

pub const SIZE: i32 = 1024;

/// Where a picture went, as texture coordinates: left, top, right, bottom.
pub type Uv = [f32; 4];

pub struct Atlas {
    pub texture: glow::Texture,
    shelves: Shelves,
    // Added since the last upload: where, how large, and the pixels.
    pending: Vec<(i32, i32, i32, i32, Vec<u8>)>,
    /// Counts the times it started over: what was placed under another
    /// count is gone.
    pub generation: u64,
}

impl Atlas {
    pub fn new(gl: &glow::Context) -> Result<Self, String> {
        unsafe {
            let texture = gl.create_texture()?;
            gl.bind_texture(glow::TEXTURE_2D, Some(texture));
            for (name, value) in [
                (glow::TEXTURE_MIN_FILTER, glow::NEAREST),
                (glow::TEXTURE_MAG_FILTER, glow::NEAREST),
                (glow::TEXTURE_WRAP_S, glow::CLAMP_TO_EDGE),
                (glow::TEXTURE_WRAP_T, glow::CLAMP_TO_EDGE),
            ] {
                gl.tex_parameter_i32(glow::TEXTURE_2D, name, value as i32);
            }
            gl.tex_storage_2d(glow::TEXTURE_2D, 1, glow::RGBA8, SIZE, SIZE);
            Ok(Self {
                texture,
                shelves: Shelves::default(),
                pending: Vec::new(),
                generation: 0,
            })
        }
    }

    /// Puts premultiplied RGBA `data`, `width` by `height`, in. None when it
    /// doesn't fit: then the atlas has started over, and the caller forgets
    /// what it placed before.
    pub fn add(&mut self, width: i32, height: i32, data: Vec<u8>) -> Option<Uv> {
        let (x, y) = match self.shelves.place(width, height) {
            Some(at) => at,
            None => {
                self.shelves = Shelves::default();
                self.pending.clear();
                self.generation += 1;
                return None;
            }
        };
        self.pending.push((x, y, width, height, data));
        let size = SIZE as f32;
        Some([
            x as f32 / size,
            y as f32 / size,
            (x + width) as f32 / size,
            (y + height) as f32 / size,
        ])
    }

    pub fn upload(&mut self, gl: &glow::Context) {
        if self.pending.is_empty() {
            return;
        }
        unsafe {
            gl.bind_texture(glow::TEXTURE_2D, Some(self.texture));
            gl.pixel_store_i32(glow::UNPACK_ALIGNMENT, 1);
            for (x, y, width, height, data) in self.pending.drain(..) {
                gl.tex_sub_image_2d(
                    glow::TEXTURE_2D,
                    0,
                    x,
                    y,
                    width,
                    height,
                    glow::RGBA,
                    glow::UNSIGNED_BYTE,
                    glow::PixelUnpackData::Slice(Some(&data)),
                );
            }
        }
    }
}

/// Rows of pictures of about one height each, with a pixel between any two.
#[derive(Default)]
struct Shelves {
    // Each: its top, its height and how far along it is filled.
    rows: Vec<(i32, i32, i32)>,
}

impl Shelves {
    fn place(&mut self, width: i32, height: i32) -> Option<(i32, i32)> {
        let (w, h) = (width + 1, height + 1);
        if w > SIZE || h > SIZE {
            return None;
        }
        // A row no more than a third taller than the picture: else small
        // glyphs waste the tall rows of large ones.
        for row in self.rows.iter_mut() {
            if h <= row.1 && h * 4 >= row.1 * 3 && row.2 + w <= SIZE {
                let x = row.2;
                row.2 += w;
                return Some((x, row.0));
            }
        }
        let top = self.rows.last().map_or(0, |row| row.0 + row.1);
        if top + h > SIZE {
            return None;
        }
        self.rows.push((top, h, w));
        Some((0, top))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pictures_of_one_height_share_a_row_and_never_overlap() {
        let mut shelves = Shelves::default();
        let a = shelves.place(10, 12).unwrap();
        let b = shelves.place(10, 12).unwrap();
        assert_eq!(a, (0, 0));
        assert_eq!(b, (11, 0));
        // Much smaller: a row of its own.
        let c = shelves.place(4, 4).unwrap();
        assert_eq!(c, (0, 13));
    }

    #[test]
    fn a_full_atlas_says_so() {
        let mut shelves = Shelves::default();
        assert!(shelves.place(SIZE + 1, 1).is_none());
        while shelves.place(SIZE - 1, 100).is_some() {}
        assert!(shelves.place(SIZE - 1, 100).is_none());
    }
}
