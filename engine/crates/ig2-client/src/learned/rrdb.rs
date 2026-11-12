//! The pack tool's quality upscaler (ADR 0089 §3, row 570): Real-ESRGAN x4plus, `RealESRGAN_x4plus.pth` (RRDBNet:
//! a 3×3 convolution to 64 features, 23 residual-in-residual dense blocks of three dense blocks each (five 3×3
//! convolutions growing by 32 features, leaky ReLU 0.2, the block's output scaled by 0.2 and added to its input),
//! a convolution added back to the first features, two nearest-neighbour ×2 steps each followed by a convolution,
//! and two more convolutions to RGB; BSD-3-Clause, bundled in the release beside the compact model). Run on the
//! graphics card as a list of convolutions over one buffer, in tiles that overlap by [`OVERLAP`] texels.

use std::io::Cursor;
use std::path::{Path, PathBuf};

use ig2_formats::zipfs::Zip;
use sha2::{Digest, Sha256};

/// The weights' file, as published in Real-ESRGAN's v0.1.0 release.
pub const MODEL_FILE: &str = "RealESRGAN_x4plus.pth";
const MODEL_SHA256: &str = "4fa0d38905f75ac06eb49a7951b426670021be3018265fd191d2125df9d682f1";
const FEAT: usize = 64;
const GROW: usize = 32;
const BLOCKS: usize = 23;
/// Texels two neighbouring tiles share, blended across (ADR 0089 §3).
pub const OVERLAP: usize = 32;
/// The buffer one tile needs: 2048 floats a texel of the tile (see [`program`]).
const FLOATS_PER_TEXEL: u64 = 2048;

/// Where the tool looks for the weights: `models/` beside the executable, else `IG2_MODEL_DIR`.
pub fn default_path() -> Option<PathBuf> {
    super::model_path(MODEL_FILE)
}

/// One 3×3 convolution: weights `[cout][cin][3][3]` and its bias.
struct Conv {
    cin: usize,
    cout: usize,
    w: Vec<f32>,
    b: Vec<f32>,
}

pub struct Model {
    convs: Vec<Conv>,
}

/// The convolutions in the network's order: (name in the file, inputs, outputs).
fn layers() -> Vec<(String, usize, usize)> {
    let mut v = vec![("conv_first".to_string(), 3, FEAT)];
    for b in 0..BLOCKS {
        for r in 1..=3 {
            for c in 1..=5 {
                v.push((format!("body.{b}.rdb{r}.conv{c}"), FEAT + GROW * (c - 1), if c == 5 { FEAT } else { GROW }));
            }
        }
    }
    v.extend(["conv_body", "conv_up1", "conv_up2", "conv_hr"].map(|n| (n.to_string(), FEAT, FEAT)));
    v.push(("conv_last".to_string(), FEAT, 3));
    v
}

/// The strings of a protocol-2 pickle in order (`BINUNICODE`): enough to pair a state dict's tensor names with their
/// storages. Refuses an opcode it does not know rather than guess its length.
fn pickle_strings(b: &[u8]) -> Result<Vec<String>, String> {
    let mut out = Vec::new();
    let mut i = 0;
    let take = |i: &mut usize, n: usize| -> Result<&[u8], String> {
        let s = b.get(*i..*i + n).ok_or("data.pkl ends early")?;
        *i += n;
        Ok(s)
    };
    let len4 = |s: &[u8]| u32::from_le_bytes([s[0], s[1], s[2], s[3]]) as usize;
    while i < b.len() {
        let op = b[i];
        i += 1;
        match op {
            b'.' => return Ok(out),
            b'}' | b')' | b'R' | b'(' | b't' | b'Q' | 0x88 | 0x89 | 0x85 | 0x86 | 0x87 | b's' | b'u' | b'b' | b']' | b'e' | b'a' | b'N' => {}
            0x80 | b'q' | b'h' | b'K' => i += 1,
            b'M' => i += 2,
            b'r' | b'j' | b'J' => i += 4,
            b'G' => i += 8,
            0x8a | b'U' => {
                let n = take(&mut i, 1)?[0] as usize;
                take(&mut i, n)?;
            }
            b'X' | b'T' => {
                let n = len4(take(&mut i, 4)?);
                let s = take(&mut i, n)?;
                if op == b'X' {
                    out.push(String::from_utf8_lossy(s).into_owned());
                }
            }
            b'c' => {
                for _ in 0..2 {
                    let n = b[i..].iter().position(|&c| c == b'\n').ok_or("data.pkl ends early")?;
                    i += n + 1;
                }
            }
            _ => return Err(format!("data.pkl: opcode {op:#04x} not read")),
        }
    }
    Err("data.pkl has no end".into())
}

impl Model {
    /// The published file, checked by its SHA-256. A PyTorch zip whose `data.pkl` holds `params_ema`, each tensor a
    /// storage `data/<key>` (little-endian `f32`); the keys are paired with the names through the pickle's strings
    /// (each `<name>.weight`/`.bias` is followed by its storage's key).
    pub fn load(path: &Path) -> Result<Self, String> {
        let bytes = std::fs::read(path).map_err(|e| format!("{}: {e}", path.display()))?;
        let sum: String = Sha256::digest(&bytes).iter().map(|b| format!("{b:02x}")).collect();
        if sum != MODEL_SHA256 {
            return Err(format!("{}: not the published {MODEL_FILE} (SHA-256 {sum})", path.display()));
        }
        let zip = Zip::open(&mut Cursor::new(&bytes))?;
        let strings = pickle_strings(&zip.read(&mut Cursor::new(&bytes), "data.pkl")?)?;
        let key = |name: &str| -> Result<&str, String> {
            let at = strings.iter().position(|s| s == name).ok_or(format!("{MODEL_FILE}: no {name}"))?;
            let k = strings[at + 1..].iter().find(|s| !s.is_empty() && s.bytes().all(|c| c.is_ascii_digit()));
            k.map(String::as_str).ok_or(format!("{MODEL_FILE}: no storage for {name}"))
        };
        let tensor = |name: String, len: usize| -> Result<Vec<f32>, String> {
            let file = format!("data/{}", key(&name)?);
            let raw = zip.read(&mut Cursor::new(&bytes), &file)?;
            if raw.len() != len * 4 {
                return Err(format!("{MODEL_FILE}: {name} holds {} bytes, not {}", raw.len(), len * 4));
            }
            Ok(raw.chunks_exact(4).map(|c| f32::from_le_bytes([c[0], c[1], c[2], c[3]])).collect())
        };
        let mut convs = Vec::new();
        for (name, cin, cout) in layers() {
            let w = tensor(format!("{name}.weight"), cout * cin * 9)?;
            let b = tensor(format!("{name}.bias"), cout)?;
            convs.push(Conv { cin, cout, w, b });
        }
        Ok(Self { convs })
    }
}

/// One convolution of the network over the buffer: `dst = a·act(conv(src)) + b·res1 + c·res2`, all offsets in
/// floats, `[channel][h][w]` planes; the output is `w` × `h`, the input `w/up` × `h/up` read by nearest neighbour.
#[derive(Clone, Copy, Debug)]
struct Op {
    layer: usize,
    w: usize,
    h: usize,
    up: usize,
    src: usize,
    dst: usize,
    lrelu: bool,
    a: f32,
    res1: (usize, f32),
    res2: (usize, f32),
}

/// The network for a `w` × `h` input as convolutions over one buffer, with where the input goes, where the output
/// is and the buffer's length. With `p = w·h`: the first features at 0, three 192-channel dense buffers at 64p,
/// 256p and 448p (the input in the third); a residual-in-residual block's input stays in the first dense buffer's
/// first 64 channels and its three dense blocks pass along the three buffers. The ×2 steps then reuse the space:
/// 2w × 2h at 640p, 4w × 4h at 1024p, again at 0, and the RGB output at 1024p (2048p floats in all).
fn program(w: usize, h: usize) -> (Vec<Op>, usize, usize, usize) {
    let p = w * h;
    let dense = [64 * p, 256 * p, 448 * p];
    let op = |layer, src, dst| Op { layer, w, h, up: 1, src, dst, lrelu: false, a: 1.0, res1: (0, 0.0), res2: (0, 0.0) };
    let mut ops = vec![op(0, dense[2], 0), op(0, dense[2], dense[0])];
    let mut l = 1;
    for _ in 0..BLOCKS {
        for r in 0..3 {
            let (d, next) = (dense[r], dense[(r + 1) % 3]);
            for k in 0..4 {
                ops.push(Op { lrelu: true, ..op(l, d, d + (FEAT + GROW * k) * p) });
                l += 1;
            }
            // The dense block's output: 0.2·conv5 + its input; the third's also closes the outer block
            // (0.2·that + the outer block's input, which is where it is written).
            ops.push(if r < 2 {
                Op { a: 0.2, res1: (d, 1.0), ..op(l, d, next) }
            } else {
                Op { a: 0.04, res1: (d, 0.2), res2: (next, 1.0), ..op(l, d, next) }
            });
            l += 1;
        }
    }
    ops.push(Op { res1: (0, 1.0), ..op(l, dense[0], dense[1]) });
    let (u1, u2) = (640 * p, 1024 * p);
    ops.push(Op { w: 2 * w, h: 2 * h, up: 2, lrelu: true, ..op(l + 1, dense[1], u1) });
    ops.push(Op { w: 4 * w, h: 4 * h, up: 2, lrelu: true, ..op(l + 2, u1, u2) });
    ops.push(Op { w: 4 * w, h: 4 * h, lrelu: true, ..op(l + 3, u2, 0) });
    ops.push(Op { w: 4 * w, h: 4 * h, ..op(l + 4, 0, u2) });
    (ops, dense[2], u2, 2048 * p)
}

/// The program on the CPU (the GPU's reference in the tests): `rgb` planes `[3][h][w]` in 0..1 → `[3][4h][4w]`.
#[cfg(test)]
fn run_cpu(m: &Model, rgb: &[f32], w: usize, h: usize) -> Vec<f32> {
    let (ops, input, output, len) = program(w, h);
    let mut mem = vec![0.0f32; len];
    mem[input..input + rgb.len()].copy_from_slice(rgb);
    for o in &ops {
        let c = &m.convs[o.layer];
        let (sw, sh, plane) = (o.w / o.up, o.h / o.up, o.w * o.h);
        let mut y = vec![0.0f32; c.cout * plane];
        for (oc, out) in y.chunks_exact_mut(plane).enumerate() {
            for py in 0..o.h {
                for px in 0..o.w {
                    let mut acc = c.b[oc];
                    for i in 0..c.cin {
                        for ky in 0..3 {
                            let Some(yy) = (py + ky).checked_sub(1).filter(|&v| v < o.h) else { continue };
                            for kx in 0..3 {
                                let Some(xx) = (px + kx).checked_sub(1).filter(|&v| v < o.w) else { continue };
                                acc += mem[o.src + i * sw * sh + yy / o.up * sw + xx / o.up] * c.w[((oc * c.cin + i) * 3 + ky) * 3 + kx];
                            }
                        }
                    }
                    out[py * o.w + px] = if o.lrelu && acc < 0.0 { acc * 0.2 } else { acc };
                }
            }
        }
        for (e, v) in y.into_iter().enumerate() {
            mem[o.dst + e] = o.a * v + o.res1.1 * mem[o.res1.0 + e] + o.res2.1 * mem[o.res2.0 + e];
        }
    }
    mem[output..output + 3 * 16 * w * h].to_vec()
}

const SHADER: &str = r#"
struct P { w: u32, h: u32, cin: u32, cout: u32, src: u32, dst: u32, wofs: u32, bofs: u32,
           act: u32, up: u32, res1: u32, res2: u32, a: f32, b: f32, c: f32, pad: u32 }
@group(0) @binding(0) var<uniform> p: P;
@group(0) @binding(1) var<storage, read> weights: array<f32>;
@group(0) @binding(2) var<storage, read_write> mem: array<f32>;

// Each invocation: one texel, 16 output channels (the weights are padded to a multiple of 16 outputs).
@compute @workgroup_size(8, 8, 1)
fn main(@builtin(global_invocation_id) g: vec3<u32>) {
    let x = g.x;
    let y = g.y;
    let o0 = g.z * 16u;
    if (x >= p.w || y >= p.h) { return; }
    let sw = p.w / p.up;
    let splane = sw * (p.h / p.up);
    var acc: array<f32, 16>;
    for (var o = 0u; o < 16u; o++) { acc[o] = weights[p.bofs + o0 + o]; }
    let stride = p.cin * 9u;
    for (var i = 0u; i < p.cin; i++) {
        for (var ky = 0u; ky < 3u; ky++) {
            let yy = i32(y) + i32(ky) - 1;
            if (yy < 0 || yy >= i32(p.h)) { continue; }
            let row = p.src + i * splane + (u32(yy) / p.up) * sw;
            for (var kx = 0u; kx < 3u; kx++) {
                let xx = i32(x) + i32(kx) - 1;
                if (xx < 0 || xx >= i32(p.w)) { continue; }
                let v = mem[row + u32(xx) / p.up];
                let k = p.wofs + (o0 * p.cin + i) * 9u + ky * 3u + kx;
                for (var o = 0u; o < 16u; o++) { acc[o] += v * weights[k + o * stride]; }
            }
        }
    }
    let plane = p.w * p.h;
    for (var o = 0u; o < 16u; o++) {
        if (o0 + o >= p.cout) { break; }
        var r = acc[o];
        if (p.act == 1u && r < 0.0) { r *= 0.2; }
        let e = (o0 + o) * plane + y * p.w + x;
        r = p.a * r;
        if (p.b != 0.0) { r += p.b * mem[p.res1 + e]; }
        if (p.c != 0.0) { r += p.c * mem[p.res2 + e]; }
        mem[p.dst + e] = r;
    }
}
"#;

/// The model on the graphics card, its buffer sized for one tile of [`Gpu::tile`] texels a side.
pub struct Gpu {
    device: wgpu::Device,
    queue: wgpu::Queue,
    pipeline: wgpu::ComputePipeline,
    /// One uniform and bind group per convolution of [`program`].
    params: Vec<wgpu::Buffer>,
    groups: Vec<wgpu::BindGroup>,
    mem: wgpu::Buffer,
    read: wgpu::Buffer,
    /// Each layer's (weights, bias) offsets in the weights buffer.
    offsets: Vec<(u32, u32)>,
    cins: Vec<usize>,
    couts: Vec<usize>,
    tile: usize,
}

impl Gpu {
    pub fn new(model: &Model) -> Result<Self, String> {
        use wgpu::util::DeviceExt;
        let instance = wgpu::Instance::new(&wgpu::InstanceDescriptor::default());
        let adapter = crate::block_on(instance.request_adapter(&wgpu::RequestAdapterOptions {
            power_preference: wgpu::PowerPreference::HighPerformance,
            ..Default::default()
        }))
        .map_err(|e| e.to_string())?;
        // The largest tile the card's buffers hold (2048 floats a texel); the overlap needs room inside it.
        let limits = adapter.limits();
        let most = (limits.max_storage_buffer_binding_size as u64).min(limits.max_buffer_size);
        let tile = [192usize, 160, 128, 96].into_iter().find(|&t| (t * t) as u64 * FLOATS_PER_TEXEL * 4 <= most).ok_or("the graphics card's buffers are too small")?;
        let desc = wgpu::DeviceDescriptor { required_limits: limits, ..Default::default() };
        let (device, queue) = crate::block_on(adapter.request_device(&desc)).map_err(|e| e.to_string())?;
        // Every layer's weights and bias in one buffer, each padded to a multiple of 16 outputs.
        let mut all = Vec::new();
        let mut offsets = Vec::new();
        for c in &model.convs {
            let pad = c.cout.next_multiple_of(16);
            let wofs = all.len();
            all.extend_from_slice(&c.w);
            all.resize(wofs + pad * c.cin * 9, 0.0);
            let bofs = all.len();
            all.extend_from_slice(&c.b);
            all.resize(bofs + pad, 0.0);
            offsets.push((wofs as u32, bofs as u32));
        }
        let weights = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("x4plus weights"),
            contents: bytemuck::cast_slice(&all),
            usage: wgpu::BufferUsages::STORAGE,
        });
        let texels = (tile * tile) as u64;
        let mem = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("x4plus tile"),
            size: texels * FLOATS_PER_TEXEL * 4,
            usage: wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_SRC | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        let read = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("x4plus read"),
            size: texels * 48 * 4,
            usage: wgpu::BufferUsages::MAP_READ | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        let module = device.create_shader_module(wgpu::ShaderModuleDescriptor { label: Some("x4plus"), source: wgpu::ShaderSource::Wgsl(SHADER.into()) });
        let pipeline = device.create_compute_pipeline(&wgpu::ComputePipelineDescriptor {
            label: Some("x4plus"),
            layout: None,
            module: &module,
            entry_point: Some("main"),
            compilation_options: Default::default(),
            cache: None,
        });
        let layout = pipeline.get_bind_group_layout(0);
        let (mut params, mut groups) = (Vec::new(), Vec::new());
        for _ in 0..program(1, 1).0.len() {
            let buf = device.create_buffer(&wgpu::BufferDescriptor {
                label: Some("x4plus op"),
                size: 64,
                usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
                mapped_at_creation: false,
            });
            groups.push(device.create_bind_group(&wgpu::BindGroupDescriptor {
                label: None,
                layout: &layout,
                entries: &[
                    wgpu::BindGroupEntry { binding: 0, resource: buf.as_entire_binding() },
                    wgpu::BindGroupEntry { binding: 1, resource: weights.as_entire_binding() },
                    wgpu::BindGroupEntry { binding: 2, resource: mem.as_entire_binding() },
                ],
            }));
            params.push(buf);
        }
        let (cins, couts) = model.convs.iter().map(|c| (c.cin, c.cout)).unzip();
        Ok(Self { device, queue, pipeline, params, groups, mem, read, offsets, cins, couts, tile })
    }

    /// The tile's side in texels (the input's; its output is 4 times that).
    pub fn tile(&self) -> usize {
        self.tile
    }

    /// `rgb` planes `[3][h][w]` (at most a tile a side) → `[3][4h][4w]`.
    fn run(&self, rgb: &[f32], w: usize, h: usize) -> Vec<f32> {
        let (ops, input, output, _) = program(w, h);
        self.queue.write_buffer(&self.mem, input as u64 * 4, bytemuck::cast_slice(rgb));
        let n = 48 * w * h;
        for (i, o) in ops.iter().enumerate() {
            let (wofs, bofs) = self.offsets[o.layer];
            let u = |v: usize| v as u32;
            let p: [u32; 16] = [
                u(o.w), u(o.h), u(self.cins[o.layer]), u(self.couts[o.layer]), u(o.src), u(o.dst), wofs, bofs,
                o.lrelu as u32, u(o.up), u(o.res1.0), u(o.res2.0), o.a.to_bits(), o.res1.1.to_bits(), o.res2.1.to_bits(), 0,
            ];
            self.queue.write_buffer(&self.params[i], 0, bytemuck::cast_slice(&p));
            let mut enc = self.device.create_command_encoder(&Default::default());
            {
                let mut pass = enc.begin_compute_pass(&Default::default());
                pass.set_pipeline(&self.pipeline);
                pass.set_bind_group(0, &self.groups[i], &[]);
                pass.dispatch_workgroups(o.w.div_ceil(8) as u32, o.h.div_ceil(8) as u32, self.couts[o.layer].div_ceil(16) as u32);
            }
            if i + 1 == ops.len() {
                enc.copy_buffer_to_buffer(&self.mem, output as u64 * 4, &self.read, 0, n as u64 * 4);
            }
            // One convolution a submission: a long one could trip the system's GPU watchdog.
            self.queue.submit([enc.finish()]);
        }
        let slice = self.read.slice(..n as u64 * 4);
        slice.map_async(wgpu::MapMode::Read, |_| ());
        let _ = self.device.poll(wgpu::PollType::Wait);
        let v = bytemuck::cast_slice::<u8, f32>(&slice.get_mapped_range()).to_vec();
        self.read.unmap();
        v
    }
}

impl super::Upscaler for Gpu {
    fn upscale(&self, rgba: &[u8], w: usize, h: usize, alpha: &[u8]) -> Vec<u8> {
        let rgb = super::tiled(w, h, self.tile, OVERLAP, |x0, y0, tw, th| self.run(&super::planes(rgba, w, (x0, y0, tw, th)), tw, th));
        super::to_rgba(&rgb, w * 4, h * 4, alpha)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn model() -> Option<Model> {
        let Some(p) = default_path() else {
            eprintln!("no {MODEL_FILE} (IG2_MODEL_DIR); skipping");
            return None;
        };
        Some(Model::load(&p).unwrap())
    }

    #[test]
    fn the_program_fits_its_buffer() {
        let (ops, input, output, len) = program(3, 2);
        assert_eq!(ops.len(), 2 + BLOCKS * 15 + 5);
        assert_eq!(layers().len(), 1 + BLOCKS * 15 + 5);
        assert!(input + 3 * 6 <= len && output + 48 * 6 <= len);
        for o in &ops {
            let c = layers()[o.layer].2;
            assert!(o.dst + c * o.w * o.h <= len, "{o:?}");
        }
        // A pickle's strings in order, past the opcodes a state dict uses.
        let pkl = [&[0x80, 2, b'}', b'q', 0, b'X', 3, 0, 0, 0][..], b"abc", &[b'K', 7, b'M', 1, 2, b'X', 1, 0, 0, 0, b'9', b'.']].concat();
        assert_eq!(pickle_strings(&pkl).unwrap(), ["abc", "9"]);
        assert!(pickle_strings(&[0x80, 2, 0xff]).is_err());
    }

    /// The GPU equals the CPU on a tiny picture, and a flat grey stays nearly flat (the weights are paired with
    /// the right layers).
    #[test]
    fn gpu_matches_cpu() {
        let Some(m) = model() else { return };
        let Ok(gpu) = Gpu::new(&m) else {
            eprintln!("no GPU; skipping");
            return;
        };
        let (w, h) = (6, 5);
        let rgba: Vec<u8> = (0..w * h).flat_map(|i| [(i * 37 % 256) as u8, (i * 91 % 256) as u8, (i * 5 % 256) as u8, 255]).collect();
        let rgb = super::super::planes(&rgba, w, (0, 0, w, h));
        let (got, want) = (gpu.run(&rgb, w, h), run_cpu(&m, &rgb, w, h));
        let worst = got.iter().zip(&want).map(|(a, b)| (a - b).abs()).fold(0.0, f32::max);
        assert!(worst * 255.0 <= 1.0, "largest difference {}", worst * 255.0);

        use super::super::Upscaler;
        let grey = [128u8, 128, 128, 255].repeat(16 * 16);
        let up = gpu.upscale(&grey, 16, 16, &vec![255; 16 * 16 * 16 * 4]);
        let worst = up.chunks_exact(4).map(|p| p[0].abs_diff(128).max(p[1].abs_diff(128)).max(p[2].abs_diff(128))).max().unwrap();
        assert!(worst <= 8, "a flat grey came out {worst} off");
    }

    #[test]
    fn a_wrong_file_is_refused() {
        let p = std::env::temp_dir().join(format!("ig2-x4plus-{}.pth", std::process::id()));
        std::fs::write(&p, b"not a model").unwrap();
        assert!(Model::load(&p).err().unwrap().contains("not the published"));
        std::fs::remove_file(&p).unwrap();
    }
}
