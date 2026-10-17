//! Images from the user's install (ADR 0001, 0006): PVR textures decoded to RGBA8 on load, never cached on disk.

use ig2_formats::pvr::Pvr;
use ig2_formats::Install;

/// A decoded image, straight (not premultiplied) RGBA8.
#[derive(Debug, Clone)]
pub struct Image {
    pub width: u32,
    pub height: u32,
    pub rgba: Vec<u8>,
}

impl Image {
    pub fn load(install: &Install, path: &str) -> Result<Self, String> {
        let bytes = install.read(path).map_err(|e| format!("{path}: {e}"))?;
        let pvr = Pvr::parse(&bytes).map_err(|e| format!("{path}: {e}"))?;
        let img = pvr.decode_rgba8().map_err(|e| format!("{path}: {e}"))?;
        Ok(Self { width: img.width, height: img.height, rgba: img.pixels })
    }

    /// The texture's `uv_mul` (its PVR's `CNST` metadata, `ig2_vertex.shader` scales the mesh UVs by it), else (1, 1).
    pub fn uv_mul(install: &Install, path: &str) -> [f32; 2] {
        let c = install.read(path).ok().and_then(|b| Pvr::parse(&b).ok().and_then(|p| p.constant("uv_mul")));
        c.map_or([1.0; 2], |c| [c[0], c[1]])
    }

    /// A 1×1 white image (untextured shapes).
    pub fn white() -> Self {
        Self { width: 1, height: 1, rgba: vec![255; 4] }
    }
}

/// Write an RGBA8 image as PNG (snapshots; they hold game data, keep them out of the repo).
pub fn write_png(path: &std::path::Path, w: u32, h: u32, rgba: &[u8]) -> Result<(), String> {
    let file = std::fs::File::create(path).map_err(|e| format!("{}: {e}", path.display()))?;
    let mut enc = png::Encoder::new(std::io::BufWriter::new(file), w, h);
    enc.set_color(png::ColorType::Rgba);
    enc.set_depth(png::BitDepth::Eight);
    let mut wr = enc.write_header().map_err(|e| e.to_string())?;
    wr.write_image_data(rgba).map_err(|e| e.to_string())
}
