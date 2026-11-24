//! The toolkit yokoso draws itself with: Wayland surfaces, the GPU, text,
//! input and motion. It knows nothing of bars or Wi-Fi.

pub mod app;
pub mod atlas;
pub mod geometry;
pub mod gpu;
pub mod input;
pub mod paint;
pub mod scale;
pub mod text;

pub use geometry::{Color, Rect};

pub use smithay_client_toolkit::reexports::calloop;
