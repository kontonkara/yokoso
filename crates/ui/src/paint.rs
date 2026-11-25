//! Everything on screen is a quad: a rounded rectangle, filled or as a ring,
//! or its soft shadow, all by one distance field; or a glyph out of the
//! atlas. A frame is one batch of them, drawn in as few calls as its clips
//! allow. Everything here is in buffer pixels.

use glow::HasContext;

use crate::{
    atlas::{Atlas, Uv},
    geometry::{Color, Rect},
};

const FILL: f32 = 0.0;
const RING: f32 = 1.0;
const MASK: f32 = 2.0;
const IMAGE: f32 = 3.0;
const SHADOW: f32 = 4.0;

/// 16 floats a quad: its rect, its color, then radius, ring width or blur,
/// kind and how far it reaches past its rect, then the atlas rect.
const FLOATS: usize = 16;

const VERTEX: &str = r#"#version 300 es
layout(location = 0) in vec4 rect;
layout(location = 1) in vec4 color;
layout(location = 2) in vec4 shape;
layout(location = 3) in vec4 uv;
uniform vec2 screen;
out vec2 local;
out vec2 extent;
out vec4 tint;
out vec4 form;
out vec2 at;
void main() {
    vec2 corner = vec2(float(gl_VertexID & 1), float((gl_VertexID >> 1) & 1));
    // How far past its rect it reaches: a pixel for an edge's softness, a
    // shadow's blur twice over, nothing for a glyph.
    float grow = shape.w;
    vec2 size = rect.zw + 2.0 * grow;
    vec2 position = rect.xy - grow + corner * size;
    local = corner * size - grow - rect.zw * 0.5;
    extent = rect.zw * 0.5;
    tint = color;
    form = shape;
    at = mix(uv.xy, uv.zw, corner);
    gl_Position = vec4(position / screen * vec2(2.0, -2.0) + vec2(-1.0, 1.0), 0.0, 1.0);
}
"#;

const FRAGMENT: &str = r#"#version 300 es
precision highp float;
in vec2 local;
in vec2 extent;
in vec4 tint;
in vec4 form;
in vec2 at;
uniform sampler2D atlas;
out vec4 color;
float rounded(vec2 p, vec2 b, float r) {
    vec2 q = abs(p) - b + r;
    return length(max(q, 0.0)) + min(max(q.x, q.y), 0.0) - r;
}
void main() {
    float kind = form.z;
    float radius = min(form.x, min(extent.x, extent.y));
    if (kind > 3.5) {
        // A shadow: the edge spread over the blur, roughly as a Gaussian is.
        float d = rounded(local, extent, radius);
        float cover = 1.0 - smoothstep(-form.y, form.y, d);
        color = tint * cover * cover;
    } else if (kind < 1.5) {
        float d = rounded(local, extent, radius);
        float cover = clamp(0.5 - d, 0.0, 1.0);
        if (kind > 0.5)
            cover *= 1.0 - clamp(0.5 - (d + form.y), 0.0, 1.0);
        color = tint * cover;
    } else if (kind < 2.5) {
        color = tint * texture(atlas, at).a;
    } else {
        color = texture(atlas, at) * tint.a;
    }
}
"#;

pub struct Painter {
    program: glow::Program,
    vao: glow::VertexArray,
    instances: glow::Buffer,
    screen: glow::UniformLocation,
    batch: Vec<f32>,
    // Where in the batch each clip starts, and the rect it keeps what follows
    // in: None for none.
    clips: Vec<(usize, Option<Rect>)>,
    /// How much of what is pushed shows: everything fades together.
    alpha: f32,
    pub atlas: Atlas,
}

impl Painter {
    pub fn new(gl: &glow::Context) -> Result<Self, String> {
        unsafe {
            let program = gl.create_program()?;
            let mut shaders = Vec::new();
            for (kind, source) in [(glow::VERTEX_SHADER, VERTEX), (glow::FRAGMENT_SHADER, FRAGMENT)] {
                let shader = gl.create_shader(kind)?;
                gl.shader_source(shader, source);
                gl.compile_shader(shader);
                if !gl.get_shader_compile_status(shader) {
                    return Err(gl.get_shader_info_log(shader));
                }
                gl.attach_shader(program, shader);
                shaders.push(shader);
            }
            gl.link_program(program);
            if !gl.get_program_link_status(program) {
                return Err(gl.get_program_info_log(program));
            }
            for shader in shaders {
                gl.delete_shader(shader);
            }
            let vao = gl.create_vertex_array()?;
            gl.bind_vertex_array(Some(vao));
            let instances = gl.create_buffer()?;
            gl.bind_buffer(glow::ARRAY_BUFFER, Some(instances));
            for location in 0..4u32 {
                gl.enable_vertex_attrib_array(location);
                gl.vertex_attrib_divisor(location, 1);
            }
            let screen = gl.get_uniform_location(program, "screen").ok_or("no screen uniform")?;
            Ok(Self {
                program,
                vao,
                instances,
                screen,
                batch: Vec::new(),
                clips: Vec::new(),
                alpha: 1.0,
                atlas: Atlas::new(gl)?,
            })
        }
    }

    fn push(&mut self, rect: Rect, color: Color, shape: [f32; 4], uv: Uv) {
        let [r, g, b, a] = color.alpha(self.alpha).premultiplied();
        self.batch
            .extend_from_slice(&[rect.x, rect.y, rect.w, rect.h, r, g, b, a]);
        self.batch.extend_from_slice(&shape);
        self.batch.extend_from_slice(&uv);
    }

    pub fn fill(&mut self, rect: Rect, radius: f32, color: Color) {
        if color.0[3] > 0.0 && !rect.is_empty() {
            self.push(rect, color, [radius, 0.0, FILL, 1.0], [0.0; 4]);
        }
    }

    /// A line `width` wide just inside `rect`'s edge.
    pub fn ring(&mut self, rect: Rect, radius: f32, width: f32, color: Color) {
        if color.0[3] > 0.0 && width > 0.0 && !rect.is_empty() {
            self.push(rect, color, [radius, width, RING, 1.0], [0.0; 4]);
        }
    }

    /// The soft shadow of a rounded `rect`, `blur` pixels across its edge.
    pub fn shadow(&mut self, rect: Rect, radius: f32, blur: f32, color: Color) {
        if color.0[3] > 0.0 && blur > 0.0 {
            self.push(rect, color, [radius, blur, SHADOW, blur * 2.0], [0.0; 4]);
        }
    }

    /// A glyph's mask in `color`, or with `colored` its own colors, from the
    /// atlas at `uv`.
    pub fn glyph(&mut self, rect: Rect, uv: Uv, color: Color, colored: bool) {
        let kind = if colored { IMAGE } else { MASK };
        self.push(rect, color, [0.0, 0.0, kind, 0.0], uv);
    }

    /// What is pushed from here on shows `alpha` of itself, until set again.
    pub fn set_alpha(&mut self, alpha: f32) {
        self.alpha = alpha.clamp(0.0, 1.0);
    }

    /// What is pushed from here on shows only in `rect`, or everywhere with
    /// None.
    pub fn clip(&mut self, rect: Option<Rect>) {
        let at = self.batch.len() / FLOATS;
        if self.clips.last().is_some_and(|(start, _)| *start == at) {
            self.clips.pop();
        }
        if self.clips.last().map(|(_, r)| *r) != Some(rect) {
            self.clips.push((at, rect));
        }
    }

    /// Draws what was pushed onto a `width` by `height` buffer and starts the
    /// next batch. Only `damage` is cleared and drawn, where given; all of
    /// the buffer otherwise.
    pub fn flush(&mut self, gl: &glow::Context, width: i32, height: i32, damage: Option<Rect>) {
        let whole = Rect::new(0.0, 0.0, width as f32, height as f32);
        let damage = damage.map_or(whole, |damage| damage.intersect(&whole));
        let scissor = |rect: Rect| unsafe {
            gl.scissor(
                rect.x as i32,
                height - rect.bottom() as i32,
                rect.w.max(0.0) as i32,
                rect.h.max(0.0) as i32,
            )
        };
        unsafe {
            self.atlas.upload(gl);
            gl.viewport(0, 0, width, height);
            gl.enable(glow::SCISSOR_TEST);
            scissor(damage);
            gl.clear_color(0.0, 0.0, 0.0, 0.0);
            gl.clear(glow::COLOR_BUFFER_BIT);
            gl.enable(glow::BLEND);
            gl.blend_func(glow::ONE, glow::ONE_MINUS_SRC_ALPHA);
            gl.use_program(Some(self.program));
            gl.uniform_2_f32(Some(&self.screen), width as f32, height as f32);
            gl.bind_vertex_array(Some(self.vao));
            gl.bind_buffer(glow::ARRAY_BUFFER, Some(self.instances));
            gl.buffer_data_u8_slice(glow::ARRAY_BUFFER, floats_as_bytes(&self.batch), glow::STREAM_DRAW);
            gl.active_texture(glow::TEXTURE0);
            gl.bind_texture(glow::TEXTURE_2D, Some(self.atlas.texture));
            // A draw for each stretch of the batch under one clip, its
            // attributes from where that stretch starts.
            let total = self.batch.len() / FLOATS;
            let mut stretches = self.clips.clone();
            if stretches.first().is_none_or(|(start, _)| *start > 0) {
                stretches.insert(0, (0, None));
            }
            for (index, (start, clip)) in stretches.iter().enumerate() {
                let end = stretches.get(index + 1).map_or(total, |(next, _)| *next);
                if end <= *start {
                    continue;
                }
                let area = clip.map_or(damage, |clip| clip.intersect(&damage));
                if area.is_empty() {
                    continue;
                }
                scissor(area);
                for location in 0..4u32 {
                    gl.vertex_attrib_pointer_f32(
                        location,
                        4,
                        glow::FLOAT,
                        false,
                        (FLOATS * 4) as i32,
                        (start * FLOATS * 4) as i32 + (location * 16) as i32,
                    );
                }
                gl.draw_arrays_instanced(glow::TRIANGLE_STRIP, 0, 4, (end - start) as i32);
            }
            gl.disable(glow::SCISSOR_TEST);
        }
        self.batch.clear();
        self.clips.clear();
        self.alpha = 1.0;
    }
}

fn floats_as_bytes(floats: &[f32]) -> &[u8] {
    // SAFETY: any f32 is four valid bytes, and u8 has no alignment to keep.
    unsafe { std::slice::from_raw_parts(floats.as_ptr().cast(), std::mem::size_of_val(floats)) }
}
