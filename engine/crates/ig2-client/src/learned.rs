//! The pack tool's learned upscaler (ADR 0054 §8, row 567): Real-ESRGAN's compact model `realesr-general-x4v3`
//! (SRVGGNetCompact: 34 3×3 convolutions of 64 features with PReLU, the last one to 48, a 4× pixel shuffle, plus the
//! input scaled by nearest neighbour; BSD-3-Clause, bundled in the release beside the binaries) run on the graphics
//! card by a wgpu compute shader, in tiles.

use std::io::Cursor;
use std::path::{Path, PathBuf};

use ig2_formats::zipfs::Zip;
use sha2::{Digest, Sha256};

/// The weights' file, as published in Real-ESRGAN's v0.2.5.0 release.
pub const MODEL_FILE: &str = "realesr-general-x4v3.pth";
const MODEL_SHA256: &str = "8dc7edb9ac80ccdc30c3a5dca6616509367f05fbc184ad95b731f05bece96292";
/// The model's scale.
pub const SCALE: usize = 4;
const FEATURES: usize = 64;
const CONVS: usize = 32;
const OUT: usize = 3 * SCALE * SCALE;
/// Texels of a tile's side the GPU computes exactly; each side reads [`HALO`] more (one per convolution).
const TILE: usize = 256;
const HALO: usize = CONVS + 2;

/// One 3×3 convolution: weights `[cout][cin][3][3]`, its bias and, but for the last, its PReLU slopes.
struct Conv {
    cin: usize,
    cout: usize,
    w: Vec<f32>,
    b: Vec<f32>,
    prelu: Option<Vec<f32>>,
}

pub struct Model {
    convs: Vec<Conv>,
}

/// Where the tool looks for the weights: `models/` beside the executable (the release's layout), else `IG2_MODEL_DIR`.
pub fn default_path() -> Option<PathBuf> {
    let beside = std::env::current_exe().ok()?.parent()?.join("models").join(MODEL_FILE);
    if beside.is_file() {
        return Some(beside);
    }
    std::env::var_os("IG2_MODEL_DIR").map(|d| PathBuf::from(d).join(MODEL_FILE)).filter(|p| p.is_file())
}

impl Model {
    /// The published file, checked by its SHA-256. A PyTorch state dict: a zip whose `data/<k>` are the tensors, in
    /// the dict's order (body.0.weight, body.0.bias, body.1.weight (PReLU), body.2.weight, …), little-endian `f32`.
    pub fn load(path: &Path) -> Result<Self, String> {
        let bytes = std::fs::read(path).map_err(|e| format!("{}: {e}", path.display()))?;
        let sum: String = Sha256::digest(&bytes).iter().map(|b| format!("{b:02x}")).collect();
        if sum != MODEL_SHA256 {
            return Err(format!("{}: not the published {MODEL_FILE} (SHA-256 {sum})", path.display()));
        }
        let zip = Zip::open(&mut Cursor::new(&bytes))?;
        let mut k = 0;
        let mut tensor = |len: usize| -> Result<Vec<f32>, String> {
            let name = format!("data/{k}");
            k += 1;
            let raw = zip.read(&mut Cursor::new(&bytes), &name)?;
            if raw.len() != len * 4 {
                return Err(format!("{MODEL_FILE}: {name} holds {} bytes, not {}", raw.len(), len * 4));
            }
            Ok(raw.chunks_exact(4).map(|c| f32::from_le_bytes([c[0], c[1], c[2], c[3]])).collect())
        };
        let mut convs = Vec::new();
        for i in 0..CONVS + 2 {
            let (cin, cout) = (if i == 0 { 3 } else { FEATURES }, if i == CONVS + 1 { OUT } else { FEATURES });
            let (w, b) = (tensor(cout * cin * 9)?, tensor(cout)?);
            let prelu = if i <= CONVS { Some(tensor(cout)?) } else { None };
            convs.push(Conv { cin, cout, w, b, prelu });
        }
        Ok(Self { convs })
    }

    /// The network on the CPU (the GPU's reference in the tests): `rgb` planes `[3][h][w]` in 0..1 → the 48 planes.
    #[cfg(test)]
    fn features_cpu(&self, rgb: &[f32], w: usize, h: usize) -> Vec<f32> {
        let mut x = rgb.to_vec();
        for c in &self.convs {
            let mut y = vec![0.0; c.cout * w * h];
            for o in 0..c.cout {
                for py in 0..h {
                    for px in 0..w {
                        let mut acc = c.b[o];
                        for i in 0..c.cin {
                            for ky in 0..3 {
                                let Some(yy) = (py + ky).checked_sub(1).filter(|&v| v < h) else { continue };
                                for kx in 0..3 {
                                    let Some(xx) = (px + kx).checked_sub(1).filter(|&v| v < w) else { continue };
                                    acc += x[(i * h + yy) * w + xx] * c.w[((o * c.cin + i) * 3 + ky) * 3 + kx];
                                }
                            }
                        }
                        if let Some(a) = &c.prelu {
                            if acc < 0.0 {
                                acc *= a[o];
                            }
                        }
                        y[(o * h + py) * w + px] = acc;
                    }
                }
            }
            x = y;
        }
        x
    }
}

/// RGB planes `[3][h][w]` in 0..1 of the RGBA rectangle (`x0`, `y0`, `w`, `h`) of a `stride`-wide picture.
fn planes(rgba: &[u8], stride: usize, (x0, y0, w, h): (usize, usize, usize, usize)) -> Vec<f32> {
    let mut v = vec![0.0; 3 * w * h];
    for y in 0..h {
        for x in 0..w {
            let p = &rgba[((y0 + y) * stride + x0 + x) * 4..][..3];
            for c in 0..3 {
                v[(c * h + y) * w + x] = p[c] as f32 / 255.0;
            }
        }
    }
    v
}

/// The 48 planes (`fw` × `fh`) shuffled into 4× RGB plus the input by nearest neighbour, for the feature texels
/// (`ox`, `oy`, `w`, `h`) into `out`'s RGB at (`dx`, `dy`) × 4; `out` is `stride` texels wide (RGBA).
#[allow(clippy::too_many_arguments)]
fn shuffle(feat: &[f32], fw: usize, fh: usize, rgb: &[f32], (ox, oy, w, h): (usize, usize, usize, usize), out: &mut [u8], stride: usize, (dx, dy): (usize, usize)) {
    for y in 0..h {
        for x in 0..w {
            let (fx, fy) = (ox + x, oy + y);
            for c in 0..3 {
                let base = rgb[(c * fh + fy) * fw + fx];
                for i in 0..SCALE {
                    for j in 0..SCALE {
                        let f = feat[((c * SCALE * SCALE + i * SCALE + j) * fh + fy) * fw + fx];
                        let v = ((f + base).clamp(0.0, 1.0) * 255.0).round() as u8;
                        out[(((dy + y) * SCALE + i) * stride + (dx + x) * SCALE + j) * 4 + c] = v;
                    }
                }
            }
        }
    }
}

/// The model on the graphics card, its buffers sized for one tile.
pub struct Gpu {
    device: wgpu::Device,
    queue: wgpu::Queue,
    pipeline: wgpu::ComputePipeline,
    params: Vec<wgpu::Buffer>,
    groups: Vec<wgpu::BindGroup>,
    ping: wgpu::Buffer,
    read: wgpu::Buffer,
    /// Each layer's output channels.
    layers: Vec<usize>,
}

const SHADER: &str = r#"
struct P { w: u32, h: u32, cin: u32, cout: u32, wofs: u32, bofs: u32, aofs: u32, act: u32 }
@group(0) @binding(0) var<uniform> p: P;
@group(0) @binding(1) var<storage, read> weights: array<f32>;
@group(0) @binding(2) var<storage, read> src: array<f32>;
@group(0) @binding(3) var<storage, read_write> dst: array<f32>;

// Each invocation: one texel, 16 output channels.
@compute @workgroup_size(8, 8, 1)
fn main(@builtin(global_invocation_id) g: vec3<u32>) {
    let x = g.x;
    let y = g.y;
    let o0 = g.z * 16u;
    if (x >= p.w || y >= p.h || o0 >= p.cout) { return; }
    var acc: array<f32, 16>;
    for (var o = 0u; o < 16u; o++) { acc[o] = weights[p.bofs + o0 + o]; }
    let plane = p.w * p.h;
    let stride = p.cin * 9u;
    for (var i = 0u; i < p.cin; i++) {
        for (var ky = 0u; ky < 3u; ky++) {
            let yy = i32(y) + i32(ky) - 1;
            if (yy < 0 || yy >= i32(p.h)) { continue; }
            for (var kx = 0u; kx < 3u; kx++) {
                let xx = i32(x) + i32(kx) - 1;
                if (xx < 0 || xx >= i32(p.w)) { continue; }
                let v = src[i * plane + u32(yy) * p.w + u32(xx)];
                let k = p.wofs + (o0 * p.cin + i) * 9u + ky * 3u + kx;
                for (var o = 0u; o < 16u; o++) { acc[o] += v * weights[k + o * stride]; }
            }
        }
    }
    for (var o = 0u; o < 16u; o++) {
        var r = acc[o];
        if (p.act == 1u && r < 0.0) { r *= weights[p.aofs + o0 + o]; }
        dst[(o0 + o) * plane + y * p.w + x] = r;
    }
}
"#;

impl Gpu {
    pub fn new(model: &Model) -> Result<Self, String> {
        use wgpu::util::DeviceExt;
        let instance = wgpu::Instance::new(&wgpu::InstanceDescriptor::default());
        let adapter = crate::block_on(instance.request_adapter(&wgpu::RequestAdapterOptions {
            power_preference: wgpu::PowerPreference::HighPerformance,
            ..Default::default()
        }))
        .map_err(|e| e.to_string())?;
        let (device, queue) = crate::block_on(adapter.request_device(&wgpu::DeviceDescriptor::default())).map_err(|e| e.to_string())?;
        // Every layer's weights, bias and slopes in one buffer; each layer's offsets in its own uniform.
        let mut all = Vec::new();
        let mut layers = Vec::new();
        let mut offsets = Vec::new();
        for c in &model.convs {
            let (wofs, _) = (all.len(), all.extend_from_slice(&c.w));
            let (bofs, _) = (all.len(), all.extend_from_slice(&c.b));
            let aofs = all.len();
            if let Some(a) = &c.prelu {
                all.extend_from_slice(a);
            }
            layers.push(c.cout);
            offsets.push([wofs, bofs, aofs, c.prelu.is_some() as usize].map(|v| v as u32));
        }
        let weights = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("learned weights"),
            contents: bytemuck::cast_slice(&all),
            usage: wgpu::BufferUsages::STORAGE,
        });
        let side = (TILE + 2 * HALO) as u64;
        let size = side * side * FEATURES as u64 * 4;
        let storage = |label| {
            device.create_buffer(&wgpu::BufferDescriptor {
                label: Some(label),
                size,
                usage: wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_SRC | wgpu::BufferUsages::COPY_DST,
                mapped_at_creation: false,
            })
        };
        let (ping, pong) = (storage("learned ping"), storage("learned pong"));
        let read = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("learned read"),
            size: side * side * OUT as u64 * 4,
            usage: wgpu::BufferUsages::MAP_READ | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        let module = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("learned"),
            source: wgpu::ShaderSource::Wgsl(SHADER.into()),
        });
        let pipeline = device.create_compute_pipeline(&wgpu::ComputePipelineDescriptor {
            label: Some("learned"),
            layout: None,
            module: &module,
            entry_point: Some("main"),
            compilation_options: Default::default(),
            cache: None,
        });
        let layout = pipeline.get_bind_group_layout(0);
        let mut params = Vec::new();
        let mut groups = Vec::new();
        for (i, o) in offsets.iter().enumerate() {
            let buf = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
                label: Some("learned layer"),
                contents: bytemuck::cast_slice(&[0, 0, model.convs[i].cin as u32, layers[i] as u32, o[0], o[1], o[2], o[3]]),
                usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            });
            // Layer 0 reads `ping`, layer 1 reads `pong`, …: the last (the 34th) writes `ping`.
            let (src, dst) = if i % 2 == 0 { (&ping, &pong) } else { (&pong, &ping) };
            groups.push(device.create_bind_group(&wgpu::BindGroupDescriptor {
                label: None,
                layout: &layout,
                entries: &[
                    wgpu::BindGroupEntry { binding: 0, resource: buf.as_entire_binding() },
                    wgpu::BindGroupEntry { binding: 1, resource: weights.as_entire_binding() },
                    wgpu::BindGroupEntry { binding: 2, resource: src.as_entire_binding() },
                    wgpu::BindGroupEntry { binding: 3, resource: dst.as_entire_binding() },
                ],
            }));
            params.push(buf);
        }
        Ok(Self { device, queue, pipeline, params, groups, ping, read, layers })
    }

    /// The 48 feature planes of `rgb` (`[3][h][w]`, at most a tile and its halo a side).
    fn features(&self, rgb: &[f32], w: usize, h: usize) -> Vec<f32> {
        self.queue.write_buffer(&self.ping, 0, bytemuck::cast_slice(rgb));
        for (i, &cout) in self.layers.iter().enumerate() {
            self.queue.write_buffer(&self.params[i], 0, bytemuck::cast_slice(&[w as u32, h as u32]));
            let mut enc = self.device.create_command_encoder(&Default::default());
            {
                let mut pass = enc.begin_compute_pass(&Default::default());
                pass.set_pipeline(&self.pipeline);
                pass.set_bind_group(0, &self.groups[i], &[]);
                pass.dispatch_workgroups(w.div_ceil(8) as u32, h.div_ceil(8) as u32, (cout / 16) as u32);
            }
            if i + 1 == self.layers.len() {
                enc.copy_buffer_to_buffer(&self.ping, 0, &self.read, 0, (OUT * w * h * 4) as u64);
            }
            // One layer a submission: a long one could trip the system's GPU watchdog.
            self.queue.submit([enc.finish()]);
        }
        let slice = self.read.slice(..(OUT * w * h * 4) as u64);
        slice.map_async(wgpu::MapMode::Read, |_| ());
        let _ = self.device.poll(wgpu::PollType::Wait);
        let v = bytemuck::cast_slice::<u8, f32>(&slice.get_mapped_range()).to_vec();
        self.read.unmap();
        v
    }

    /// `rgba` (`w` × `h`) at 4×: RGB from the model, alpha from `alpha` (the caller's smooth 4× copy, RGBA).
    pub fn upscale(&self, rgba: &[u8], w: usize, h: usize, alpha: &[u8]) -> Vec<u8> {
        let stride = w * SCALE;
        let mut out = alpha.to_vec();
        for ty in (0..h).step_by(TILE) {
            for tx in (0..w).step_by(TILE) {
                let (x0, y0) = (tx.saturating_sub(HALO), ty.saturating_sub(HALO));
                let (x1, y1) = ((tx + TILE + HALO).min(w), (ty + TILE + HALO).min(h));
                let (fw, fh) = (x1 - x0, y1 - y0);
                let rgb = planes(rgba, w, (x0, y0, fw, fh));
                let feat = self.features(&rgb, fw, fh);
                let inner = (tx - x0, ty - y0, TILE.min(w - tx), TILE.min(h - ty));
                shuffle(&feat, fw, fh, &rgb, inner, &mut out, stride, (tx, ty));
            }
        }
        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn model() -> Option<Model> {
        let Some(p) = default_path() else {
            eprintln!("IG2_MODEL_DIR not set; skipping");
            return None;
        };
        Some(Model::load(&p).unwrap())
    }

    /// The GPU's tiles equal the CPU's network over the whole picture (the halo makes a tile's inside exact), and a
    /// flat grey stays nearly flat (the weights are read in the right order).
    #[test]
    fn gpu_matches_cpu() {
        let Some(m) = model() else { return };
        // Two tiles across: the second's halo reaches into the first.
        let (w, h) = (TILE + HALO + 10, 5);
        let rgba: Vec<u8> = (0..w * h).flat_map(|i| [(i * 7 % 256) as u8, (i * 13 % 256) as u8, (i / 3 % 256) as u8, 255]).collect();
        let Ok(gpu) = Gpu::new(&m) else {
            eprintln!("no GPU; skipping");
            return;
        };
        let got = gpu.upscale(&rgba, w, h, &vec![255; w * h * 16 * 4]);
        let rgb = planes(&rgba, w, (0, 0, w, h));
        let mut want = vec![255; w * h * 16 * 4];
        shuffle(&m.features_cpu(&rgb, w, h), w, h, &rgb, (0, 0, w, h), &mut want, w * SCALE, (0, 0));
        let worst = got.iter().zip(&want).map(|(a, b)| a.abs_diff(*b)).max().unwrap();
        assert!(worst <= 1, "largest difference {worst}");

        let grey = [128u8, 128, 128, 255].repeat(12 * 12);
        let up = gpu.upscale(&grey, 12, 12, &vec![255; 12 * 12 * 16 * 4]);
        let worst = up.chunks_exact(4).map(|p| p[0].abs_diff(128).max(p[1].abs_diff(128)).max(p[2].abs_diff(128))).max().unwrap();
        assert!(worst <= 8, "a flat grey came out {worst} off");
    }

    #[test]
    fn a_wrong_file_is_refused() {
        let p = std::env::temp_dir().join(format!("ig2-model-{}.pth", std::process::id()));
        std::fs::write(&p, b"not a model").unwrap();
        assert!(Model::load(&p).err().unwrap().contains("not the published"));
        std::fs::remove_file(&p).unwrap();
    }
}
