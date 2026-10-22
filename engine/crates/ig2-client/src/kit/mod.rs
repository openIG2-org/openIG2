//! The shared screen kit: what every screen of the client draws and reads with. The layout of the virtual 640 × 480
//! screen in the window (the remaster's widescreen rule, [`layout`]), hot-spot tests, the faders, the hot-spot
//! tooltip and the animated row frame.

pub mod design_frame;
pub mod fader;
pub mod layout;
pub mod tooltip;

pub use design_frame::DesignFrame;
pub use layout::{Layout, Virtual, Wide};
pub use tooltip::Tooltip;

/// A hot-spot `[x, y, w, h]` (virtual pixels) holds the pointer `at`: `x <= at.x < x + w`, `y <= at.y < y + h`.
pub fn inside(r: [i32; 4], at: [f32; 2]) -> bool {
    at[0] >= r[0] as f32 && at[0] < (r[0] + r[2]) as f32 && at[1] >= r[1] as f32 && at[1] < (r[1] + r[3]) as f32
}

/// [`inside`] for a rectangle in floats.
pub fn inside_f(r: [f32; 4], p: [f32; 2]) -> bool {
    p[0] >= r[0] && p[0] < r[0] + r[2] && p[1] >= r[1] && p[1] < r[1] + r[3]
}
