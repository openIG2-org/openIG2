//! The shared screen kit: what every screen of the client draws and reads with. The layout of the virtual 640 × 480
//! screen in the window (the remaster's widescreen rule, [`layout`]), hot-spot tests, the faders, the hot-spot
//! tooltip and the animated row frame.

pub mod design_frame;
pub mod fader;
pub mod fader_design;
pub mod laser;
pub mod layout;
pub mod tooltip;

pub use design_frame::DesignFrame;
pub use fader_design::FaderDesign;
pub use laser::Lasers;
pub use layout::{Layout, Virtual, Wide};
pub use tooltip::Tooltip;

/// A hot-spot `[x, y, w, h]` (virtual pixels) holds the pointer `at`: `x <= at.x < x + w`, `y <= at.y < y + h`.
pub fn inside(r: [i32; 4], at: [f32; 2]) -> bool {
    at[0] >= r[0] as f32 && at[0] < (r[0] + r[2]) as f32 && at[1] >= r[1] as f32 && at[1] < (r[1] + r[3]) as f32
}

/// The texture of the picture at `path` (cached; `None` or a picture that fails to load gives `white`).
pub fn texture(r: &mut crate::render::Renderer, install: &ig2_formats::Install, textures: &mut std::collections::HashMap<String, crate::render::TexId>, white: crate::render::TexId, path: Option<&str>) -> crate::render::TexId {
    let Some(path) = path else { return white };
    if let Some(&t) = textures.get(path) {
        return t;
    }
    let t = match crate::assets::Image::load(install, path) {
        Ok(img) => r.add_texture(&img),
        Err(e) => {
            eprintln!("ig2-client: {e}");
            white
        }
    };
    textures.insert(path.to_owned(), t);
    t
}

/// [`inside`] for a rectangle in floats.
pub fn inside_f(r: [f32; 4], p: [f32; 2]) -> bool {
    p[0] >= r[0] && p[0] < r[0] + r[2] && p[1] >= r[1] && p[1] < r[1] + r[3]
}
