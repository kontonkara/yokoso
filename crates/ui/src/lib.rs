//! The toolkit yokoso draws itself with: Wayland surfaces, the GPU, text,
//! input and motion. It knows nothing of bars or Wi-Fi.

pub mod app;
pub mod atlas;
pub mod geometry;
pub mod gpu;
pub mod paint;
pub mod scale;

pub use geometry::{Color, Rect};
