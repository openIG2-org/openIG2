//! Images from the user's install (ADR 0001, 0006): PVR textures decoded to RGBA8 on load, never cached on disk.

use ig2_formats::pvr::Pvr;
use ig2_formats::Install;

/// A decoded image, straight (not premultiplied) RGBA8.
#[derive(Debug, Clone)]
pub struct Image {
    pub width: u32,
    pub height: u32,
    pub rgba: Vec<u8>,
    /// The smaller levels the PVR holds, largest first (empty: the renderer filters its own).
    pub mips: Vec<Vec<u8>>,
}

impl Image {
    pub fn load(install: &Install, path: &str) -> Result<Self, String> {
        let bytes = install.read(path).map_err(|e| format!("{path}: {e}"))?;
        let pvr = Pvr::parse(&bytes).map_err(|e| format!("{path}: {e}"))?;
        let mut levels = pvr.decode_levels().map_err(|e| format!("{path}: {e}"))?.into_iter().map(|l| l.pixels);
        let rgba = levels.next().ok_or(format!("{path}: no pixels"))?;
        Ok(Self { width: pvr.width, height: pvr.height, rgba, mips: levels.collect() })
    }

    /// The texture's `uv_mul` (its PVR's `CNST` metadata, `ig2_vertex.shader` scales the mesh UVs by it), else (1, 1).
    pub fn uv_mul(install: &Install, path: &str) -> [f32; 2] {
        let c = install.read(path).ok().and_then(|b| Pvr::parse(&b).ok().and_then(|p| p.constant("uv_mul")));
        c.map_or([1.0; 2], |c| [c[0], c[1]])
    }

    /// A 1×1 white image (untextured shapes).
    pub fn white() -> Self {
        Self { width: 1, height: 1, rgba: vec![255; 4], mips: Vec::new() }
    }
}

/// Write an RGBA8 image as PNG (snapshots; they hold game data, keep them out of the repo).
/// A snapshot's band (`o.size`) written as the window `o.window` shows it ([`crate::kit::layout::compose`]).
pub fn write_snapshot(path: &std::path::Path, o: &crate::Options, rgba: &[u8]) -> Result<(), String> {
    let l = crate::kit::Layout::new(o.window.0 as i32, o.window.1 as i32);
    if o.window == o.size && !(crate::kit::layout::letterbox() && l.frame[2] < l.size[0]) {
        return write_png(path, o.size.0, o.size.1, rgba);
    }
    write_png(path, o.window.0, o.window.1, &crate::kit::layout::compose(&l, rgba))
}

pub fn write_png(path: &std::path::Path, w: u32, h: u32, rgba: &[u8]) -> Result<(), String> {
    let file = std::fs::File::create(path).map_err(|e| format!("{}: {e}", path.display()))?;
    let mut enc = png::Encoder::new(std::io::BufWriter::new(file), w, h);
    enc.set_color(png::ColorType::Rgba);
    enc.set_depth(png::BitDepth::Eight);
    let mut wr = enc.write_header().map_err(|e| e.to_string())?;
    wr.write_image_data(rgba).map_err(|e| e.to_string())
}
