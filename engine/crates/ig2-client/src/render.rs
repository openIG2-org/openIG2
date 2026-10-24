//! The batched quad renderer (ADR 0006 §6): one instanced pipeline drawing sprites, lit spheres, rings, markers and
//! lines in physical pixels, batched per texture and blend mode in submission order. It draws a [`Frame`] into any
//! texture view, so the window's swap chain and the headless snapshot share every line of it.

use wgpu::util::DeviceExt;

use crate::assets::Image;

const SHADER: &str = r#"
struct U { screen: vec2<f32>, time: f32, _pad: f32 };
@group(0) @binding(0) var<uniform> u: U;
@group(1) @binding(0) var tex: texture_2d<f32>;
@group(1) @binding(1) var smp: sampler;

struct I {
    @location(0) center: vec2<f32>,
    @location(1) half: vec2<f32>,
    @location(2) rot: f32,
    @location(3) kind: u32,
    @location(4) param: vec2<f32>,
    @location(5) uv: vec4<f32>,
    @location(6) color: vec4<f32>,
};
struct VO {
    @builtin(position) pos: vec4<f32>,
    @location(0) q: vec2<f32>,
    @location(1) @interpolate(flat) kind: u32,
    @location(2) param: vec2<f32>,
    @location(3) uv: vec4<f32>,
    @location(4) color: vec4<f32>,
    @location(5) half: vec2<f32>,
};

@vertex
fn vs(@builtin(vertex_index) vi: u32, i: I) -> VO {
    var corners = array<vec2<f32>, 6>(vec2(-1.0, -1.0), vec2(1.0, -1.0), vec2(1.0, 1.0), vec2(-1.0, -1.0), vec2(1.0, 1.0), vec2(-1.0, 1.0));
    let q = corners[vi];
    let c = cos(i.rot);
    let s = sin(i.rot);
    let d = q * i.half;
    var p = i.center + vec2(d.x * c - d.y * s, d.x * s + d.y * c);
    // A triangle: its corners are `center`, `half` and `param`; the second half of the quad collapses.
    if i.kind == 6u {
        var tri = array<vec2<f32>, 6>(i.center, i.half, i.param, i.center, i.center, i.center);
        p = tri[vi];
    }
    var o: VO;
    o.pos = vec4(p.x / u.screen.x * 2.0 - 1.0, 1.0 - p.y / u.screen.y * 2.0, 0.0, 1.0);
    o.q = q;
    o.kind = i.kind;
    o.param = i.param;
    o.uv = i.uv;
    o.color = i.color;
    o.half = i.half;
    return o;
}

const PI: f32 = 3.14159265;

// Coverage of the edge of a shape whose signed distance (in quad units, < 0 inside) is `d`.
fn cover(d: f32) -> f32 {
    let w = max(fwidth(d), 1e-4);
    return clamp(0.5 - d / w, 0.0, 1.0);
}

@fragment
fn fs(v: VO) -> @location(0) vec4<f32> {
    let q = v.q;
    switch v.kind {
        // Textured sprite.
        case 0u: {
            let t = v.uv.xy + (q * 0.5 + 0.5) * v.uv.zw;
            return textureSample(tex, smp, t) * v.color;
        }
        // Lit sphere sampling an equirectangular map: param.x = longitude turn (0..1), uv = the map's rect.
        case 1u: {
            let r2 = dot(q, q);
            let a = cover(sqrt(r2) - 1.0);
            let z = sqrt(max(1.0 - r2, 0.0));
            let n = vec3(q.x, -q.y, z);
            let lon = atan2(n.x, n.z) / (2.0 * PI) + 0.5 + v.param.x;
            let lat = asin(clamp(n.y, -1.0, 1.0)) / PI;
            let t = v.uv.xy + vec2(fract(lon), 0.5 - lat) * v.uv.zw;
            let dims = vec2<f32>(textureDimensions(tex));
            let lod = log2(max(dims.x * v.uv.z / (PI * 2.0 * v.half.x), 1.0));
            let albedo = textureSampleLevel(tex, smp, t, lod).rgb;
            let l = normalize(vec3(-0.55, 0.45, 0.7));
            let diffuse = 0.12 + 0.88 * max(dot(n, l), 0.0);
            let rim = pow(1.0 - z, 3.0) * v.param.y;
            let rgb = albedo * diffuse * v.color.rgb + rim * vec3(0.45, 0.6, 1.0);
            return vec4(rgb, a * v.color.a);
        }
        // Ring: param.x = inner radius (quad units).
        case 2u: {
            let r = length(q);
            let a = cover(r - 1.0) * (1.0 - cover(r - v.param.x));
            return vec4(v.color.rgb, v.color.a * a);
        }
        // Fleet marker: a triangle pointing along +x, (1, 0) (-0.7, ±0.75), with a dark outline.
        case 3u: {
            let p = vec2(q.x, abs(q.y));
            let d = max(dot(p - vec2(1.0, 0.0), vec2(0.4036, 0.9149)), -q.x - 0.7);
            let a = cover(d);
            let inner = cover(d + 0.18);
            let rgb = mix(vec3(0.0), v.color.rgb, inner);
            return vec4(rgb, a * v.color.a);
        }
        // Solid quad with soft long edges (lines); param.x > 0.5: hard edges, every pixel whose centre the quad
        // covers at full colour (the exe's aliased GL lines, `Instance::hard_line`).
        case 4u: {
            if v.param.x > 0.5 {
                return v.color;
            }
            let a = cover(abs(q.y) - 1.0 + 0.5 * fwidth(q.y));
            return vec4(v.color.rgb, v.color.a * a);
        }
        // Solid triangle (kind 6).
        case 6u: {
            return v.color;
        }
        // A `KiirStr` glyph: a sprite whose uv is clamped to its cell inset by half a texel (`ig2_fragment_font`).
        case 7u: {
            let h = 0.5 / vec2<f32>(textureDimensions(tex));
            let t = clamp(v.uv.xy + (q * 0.5 + 0.5) * v.uv.zw, v.uv.xy + h, v.uv.xy + v.uv.zw - h);
            return textureSample(tex, smp, t) * v.color;
        }
        // Soft round glow (procedural stars).
        default: {
            let r = length(q);
            let g = clamp(1.0 - r, 0.0, 1.0);
            return vec4(v.color.rgb, v.color.a * g * g * g);
        }
    }
}
"#;

/// Instance kinds (`I.kind`).
pub mod kind {
    pub const SPRITE: u32 = 0;
    pub const SPHERE: u32 = 1;
    pub const RING: u32 = 2;
    pub const MARKER: u32 = 3;
    pub const LINE: u32 = 4;
    pub const GLOW: u32 = 5;
    pub const TRI: u32 = 6;
    pub const GLYPH: u32 = 7;
}

/// One quad, in physical pixels (origin top left).
#[repr(C)]
#[derive(Debug, Clone, Copy, bytemuck::Pod, bytemuck::Zeroable)]
pub struct Instance {
    pub center: [f32; 2],
    pub half: [f32; 2],
    pub rot: f32,
    pub kind: u32,
    pub param: [f32; 2],
    /// Texture rect: offset, size (normalised).
    pub uv: [f32; 4],
    /// Linear RGBA multiplier.
    pub color: [f32; 4],
}

impl Instance {
    pub fn new(kind: u32, center: [f32; 2], half: [f32; 2], color: [f32; 4]) -> Self {
        Self { center, half, rot: 0.0, kind, param: [0.0; 2], uv: [0.0, 0.0, 1.0, 1.0], color }
    }
    /// A solid triangle with the corners `a`, `b`, `c` (kind [`kind::TRI`]).
    pub fn tri(a: [f32; 2], b: [f32; 2], c: [f32; 2], color: [f32; 4]) -> Self {
        Self { param: c, ..Self::new(kind::TRI, a, b, color) }
    }
    /// A line from `a` to `b`, `width` pixels wide.
    pub fn line(a: [f32; 2], b: [f32; 2], width: f32, color: [f32; 4]) -> Self {
        let (dx, dy) = (b[0] - a[0], b[1] - a[1]);
        let len = (dx * dx + dy * dy).sqrt();
        Self {
            rot: dy.atan2(dx),
            ..Self::new(kind::LINE, [(a[0] + b[0]) / 2.0, (a[1] + b[1]) / 2.0], [len / 2.0, width / 2.0], color)
        }
    }

    /// A line as the engine's GL lines draw it (`glBegin(GL_LINES)`, one output pixel wide, not smoothed): the
    /// pixels whose centres lie within half a pixel of it at full colour, no soft edge.
    pub fn hard_line(a: [f32; 2], b: [f32; 2], color: [f32; 4]) -> Self {
        Self { param: [1.0, 0.0], ..Self::line(a, b, 1.0, color) }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Blend {
    Alpha,
    Additive,
    /// The exe's 2D pass (`DDBrushToScreen3d`, `KiirStr`; re/screens/hud.md §1.1): its GL textures and frame have no
    /// sRGB formats, so texels, tints and blending stay in gamma space, and its PVRs have one level, sampled
    /// bilinearly without mips. Drawn through the frame's non-sRGB view with the textures' raw views.
    GammaAlpha,
    GammaAdditive,
    /// A frame of the gamma-space mesh pass ([`crate::render3d::Scene3d::gamma`]) drawn into a cleared target, laid
    /// over the frame as the remaster draws its meshes straight onto it: the target holds `c·a` and the coverage
    /// `a` (premultiplied), so the result is `c·a + frame·(1 − a)` in gamma space.
    GammaPremul,
    /// A union of shapes added once (the exe's coplanar, depth-tested radar discs, re/screens/starmap.md §6.3),
    /// through the frame's alpha channel, in gamma space: `MaskClear` (a full-frame quad) sets alpha to 0,
    /// `MaskMax` (the shapes) raises it to the shapes' alpha, `MaskAdd` (a full-frame quad) adds its colour ×
    /// that alpha, `MaskRestore` (a full-frame quad) sets alpha back to 1. Colours are left alone except by `MaskAdd`.
    MaskClear,
    MaskMax,
    MaskAdd,
    MaskRestore,
}

impl Blend {
    fn gamma(self) -> bool {
        !matches!(self, Blend::Alpha | Blend::Additive)
    }
}

/// A texture frames are drawn into ([`Renderer::target`]).
pub struct Target {
    pub tex: TexId,
    texture: wgpu::Texture,
    pub size: (u32, u32),
}

/// A texture registered with the renderer.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TexId(pub(crate) usize);

#[derive(Debug, Clone)]
struct Batch {
    tex: TexId,
    blend: Blend,
    range: std::ops::Range<u32>,
}

/// What to draw this frame, in order.
#[derive(Debug, Clone, Default)]
pub struct Frame {
    pub clear: [f64; 4],
    /// Meshes drawn before the quads (the colony screen).
    pub scene: Option<crate::render3d::Scene3d>,
    /// Parts of `scene` (its vertices) drawn after the quads pushed before [`Frame::mark_late`], over them: the
    /// starmap's planets over its background particles.
    pub late: Vec<crate::render3d::MeshDraw>,
    /// A picture over the whole target under everything else, the scene too (`Copy3hatterkep`: the game behind an
    /// overlay screen, re/screens/overlay-background.md): its texture and its gamma-space grey.
    pub backdrop: Option<(TexId, f32)>,
    late_at: usize,
    instances: Vec<Instance>,
    batches: Vec<Batch>,
}

impl Frame {
    pub fn new(clear: [f64; 4]) -> Self {
        Self { clear, ..Default::default() }
    }
    pub fn push(&mut self, tex: TexId, blend: Blend, i: Instance) {
        let n = self.instances.len() as u32;
        self.instances.push(i);
        match self.batches.last_mut() {
            Some(b) if b.tex == tex && b.blend == blend => b.range.end = n + 1,
            _ => self.batches.push(Batch { tex, blend, range: n..n + 1 }),
        }
    }
    pub fn len(&self) -> usize {
        self.instances.len()
    }
    /// Push `o`'s quads after these (its clear colour, scene and late parts are dropped).
    pub fn append(&mut self, o: Frame) {
        for b in &o.batches {
            for i in b.range.clone() {
                self.push(b.tex, b.blend, o.instances[i as usize]);
            }
        }
    }
    /// The quads pushed so far go below [`Frame::late`].
    pub fn mark_late(&mut self) {
        self.late_at = self.batches.len();
    }
}

#[repr(C)]
#[derive(Debug, Clone, Copy, bytemuck::Pod, bytemuck::Zeroable)]
struct Uniforms {
    screen: [f32; 2],
    time: f32,
    _pad: f32,
}

pub struct Renderer {
    pub device: wgpu::Device,
    pub queue: wgpu::Queue,
    alpha: wgpu::RenderPipeline,
    additive: wgpu::RenderPipeline,
    /// [`Blend::GammaAlpha`] / [`Blend::GammaAdditive`], drawing into the frame's non-sRGB view.
    gamma_alpha: wgpu::RenderPipeline,
    gamma_additive: wgpu::RenderPipeline,
    gamma_premul: wgpu::RenderPipeline,
    /// [`Blend::MaskClear`], [`Blend::MaskMax`], [`Blend::MaskAdd`], [`Blend::MaskRestore`].
    mask: [wgpu::RenderPipeline; 4],
    uniform: wgpu::Buffer,
    uniform_bg: wgpu::BindGroup,
    tex_layout: wgpu::BindGroupLayout,
    sampler: wgpu::Sampler,
    textures: Vec<wgpu::BindGroup>,
    /// The same textures with a repeating sampler (meshes).
    textures_rep: Vec<wgpu::BindGroup>,
    sampler_rep: wgpu::Sampler,
    /// The same textures' raw (non-sRGB) views with the exe's 2D sampling (the gamma blends).
    textures_raw: Vec<wgpu::BindGroup>,
    sampler_raw: wgpu::Sampler,
    /// The raw views with the mesh sampler (`TXNV` 11: repeat, trilinear with anisotropy): the remaster's gamma-space
    /// mesh pass (re/screens/render.md).
    textures_raw_rep: Vec<wgpu::BindGroup>,
    instances: Option<(wgpu::Buffer, u64)>,
    pipe3d: crate::render3d::Pipe3d,
    /// The colour format the pipelines draw into.
    format: wgpu::TextureFormat,
    /// The video frames' textures (one level, rewritten in place each frame), by texture id.
    videos: Vec<(TexId, wgpu::Texture)>,
    /// `--snapshot --backdrop`: the picture under a snapshot frame that has none (crate::backdrop).
    pub snapshot_backdrop: Option<(TexId, f32)>,
}

impl Renderer {
    pub fn new(device: wgpu::Device, queue: wgpu::Queue, format: wgpu::TextureFormat) -> Self {
        let d = &device;
        let shader = d.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("ig2-client quads"),
            source: wgpu::ShaderSource::Wgsl(SHADER.into()),
        });
        let ubl = d.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: None,
            entries: &[wgpu::BindGroupLayoutEntry {
                binding: 0,
                visibility: wgpu::ShaderStages::VERTEX_FRAGMENT,
                ty: wgpu::BindingType::Buffer { ty: wgpu::BufferBindingType::Uniform, has_dynamic_offset: false, min_binding_size: None },
                count: None,
            }],
        });
        let tex_layout = d.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: None,
            entries: &[
                wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Texture {
                        sample_type: wgpu::TextureSampleType::Float { filterable: true },
                        view_dimension: wgpu::TextureViewDimension::D2,
                        multisampled: false,
                    },
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 1,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
                    count: None,
                },
            ],
        });
        let layout = d.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: None,
            bind_group_layouts: &[&ubl, &tex_layout],
            push_constant_ranges: &[],
        });
        let attrs = wgpu::vertex_attr_array![0 => Float32x2, 1 => Float32x2, 2 => Float32, 3 => Uint32, 4 => Float32x2, 5 => Float32x4, 6 => Float32x4];
        let pipeline = |blend: wgpu::BlendState, format: wgpu::TextureFormat| {
            d.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
                label: Some("ig2-client quads"),
                layout: Some(&layout),
                vertex: wgpu::VertexState {
                    module: &shader,
                    entry_point: Some("vs"),
                    compilation_options: Default::default(),
                    buffers: &[wgpu::VertexBufferLayout {
                        array_stride: std::mem::size_of::<Instance>() as u64,
                        step_mode: wgpu::VertexStepMode::Instance,
                        attributes: &attrs,
                    }],
                },
                primitive: wgpu::PrimitiveState { cull_mode: None, ..Default::default() },
                depth_stencil: None,
                multisample: Default::default(),
                fragment: Some(wgpu::FragmentState {
                    module: &shader,
                    entry_point: Some("fs"),
                    compilation_options: Default::default(),
                    targets: &[Some(wgpu::ColorTargetState { format, blend: Some(blend), write_mask: wgpu::ColorWrites::ALL })],
                }),
                multiview: None,
                cache: None,
            })
        };
        let alpha = pipeline(wgpu::BlendState::ALPHA_BLENDING, format);
        let add = wgpu::BlendComponent { src_factor: wgpu::BlendFactor::SrcAlpha, dst_factor: wgpu::BlendFactor::One, operation: wgpu::BlendOperation::Add };
        let additive = pipeline(wgpu::BlendState { color: add, alpha: add }, format);
        let gamma_alpha = pipeline(wgpu::BlendState::ALPHA_BLENDING, format.remove_srgb_suffix());
        let gamma_additive = pipeline(wgpu::BlendState { color: add, alpha: add }, format.remove_srgb_suffix());
        let gamma_premul = pipeline(wgpu::BlendState::PREMULTIPLIED_ALPHA_BLENDING, format.remove_srgb_suffix());
        let comp = |src_factor, dst_factor, operation| wgpu::BlendComponent { src_factor, dst_factor, operation };
        use wgpu::{BlendFactor as F, BlendOperation as O};
        let keep = comp(F::Zero, F::One, O::Add);
        let mask = [
            wgpu::BlendState { color: keep, alpha: comp(F::Zero, F::Zero, O::Add) },
            wgpu::BlendState { color: keep, alpha: comp(F::One, F::One, O::Max) },
            wgpu::BlendState { color: comp(F::DstAlpha, F::One, O::Add), alpha: keep },
            wgpu::BlendState { color: keep, alpha: comp(F::One, F::Zero, O::Add) },
        ]
        .map(|b| pipeline(b, format.remove_srgb_suffix()));
        let uniform = d.create_buffer(&wgpu::BufferDescriptor {
            label: None,
            size: std::mem::size_of::<Uniforms>() as u64,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        let uniform_bg = d.create_bind_group(&wgpu::BindGroupDescriptor {
            label: None,
            layout: &ubl,
            entries: &[wgpu::BindGroupEntry { binding: 0, resource: uniform.as_entire_binding() }],
        });
        let sampler = d.create_sampler(&wgpu::SamplerDescriptor {
            address_mode_u: wgpu::AddressMode::Repeat,
            address_mode_v: wgpu::AddressMode::ClampToEdge,
            mag_filter: wgpu::FilterMode::Linear,
            min_filter: wgpu::FilterMode::Linear,
            mipmap_filter: wgpu::FilterMode::Linear,
            ..Default::default()
        });
        let sampler_rep = d.create_sampler(&wgpu::SamplerDescriptor {
            address_mode_u: wgpu::AddressMode::Repeat,
            address_mode_v: wgpu::AddressMode::Repeat,
            mag_filter: wgpu::FilterMode::Linear,
            min_filter: wgpu::FilterMode::Linear,
            mipmap_filter: wgpu::FilterMode::Linear,
            anisotropy_clamp: 8,
            ..Default::default()
        });
        // `sg3d::texture_t::load_pvr` (ELF 0x86db50): the screen pictures' PVRs have one level and a `TXNV` word
        // of 11 (repeat in s and t; trilinear only with mips): GL_LINEAR both ways on level 0.
        let sampler_raw = d.create_sampler(&wgpu::SamplerDescriptor {
            address_mode_u: wgpu::AddressMode::Repeat,
            address_mode_v: wgpu::AddressMode::Repeat,
            mag_filter: wgpu::FilterMode::Linear,
            min_filter: wgpu::FilterMode::Linear,
            mipmap_filter: wgpu::FilterMode::Nearest,
            lod_max_clamp: 0.0,
            ..Default::default()
        });
        let pipe3d = crate::render3d::Pipe3d::new(d, format, &tex_layout);
        Self {
            mask,
            device,
            queue,
            alpha,
            additive,
            gamma_alpha,
            gamma_additive,
            gamma_premul,
            uniform,
            uniform_bg,
            tex_layout,
            sampler,
            textures: Vec::new(),
            textures_rep: Vec::new(),
            sampler_rep,
            textures_raw: Vec::new(),
            textures_raw_rep: Vec::new(),
            sampler_raw,
            instances: None,
            pipe3d,
            format,
            videos: Vec::new(),
            snapshot_backdrop: None,
        }
    }

    /// A headless renderer (no surface): `--snapshot`.
    pub fn headless(format: wgpu::TextureFormat) -> Result<Self, String> {
        let instance = wgpu::Instance::new(&wgpu::InstanceDescriptor::default());
        let adapter = pollster::block_on(instance.request_adapter(&wgpu::RequestAdapterOptions::default())).map_err(|e| e.to_string())?;
        let (device, queue) = pollster::block_on(adapter.request_device(&wgpu::DeviceDescriptor::default())).map_err(|e| e.to_string())?;
        Ok(Self::new(device, queue, format))
    }

    /// Upload an image with a full mip chain (box-filtered on the CPU).
    pub fn add_texture(&mut self, img: &Image) -> TexId {
        self.upload(img, wgpu::TextureFormat::Rgba8UnormSrgb)
    }

    /// [`Renderer::add_texture`] for data, sampled as stored (no sRGB decoding): the cloud material's noise.
    pub fn add_texture_linear(&mut self, img: &Image) -> TexId {
        self.upload(img, wgpu::TextureFormat::Rgba8Unorm)
    }

    fn upload(&mut self, img: &Image, format: wgpu::TextureFormat) -> TexId {
        crate::prof::count("tex", 1);
        crate::prof::count("tex_kb", img.rgba.len() as u64 / 1024);
        let mips = mip_chain(img);
        let size = wgpu::Extent3d { width: img.width, height: img.height, depth_or_array_layers: 1 };
        let tex = self.device.create_texture_with_data(
            &self.queue,
            &wgpu::TextureDescriptor {
                label: None,
                size,
                mip_level_count: mips.len() as u32,
                sample_count: 1,
                dimension: wgpu::TextureDimension::D2,
                format,
                usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
                view_formats: &[wgpu::TextureFormat::Rgba8Unorm],
            },
            wgpu::util::TextureDataOrder::MipMajor,
            &mips.concat(),
        );
        let [a, b, c, e] = self.groups(&tex);
        self.textures.push(a);
        self.textures_rep.push(b);
        self.textures_raw.push(c);
        self.textures_raw_rep.push(e);
        TexId(self.textures.len() - 1)
    }

    /// The bind groups of `tex`: its sRGB view with the clamping and the repeating sampler, its raw view with the
    /// exe's 2D sampling and with the repeating sampler.
    fn groups(&self, tex: &wgpu::Texture) -> [wgpu::BindGroup; 4] {
        let view = tex.create_view(&Default::default());
        let raw = raw_view(tex);
        let group = |view: &wgpu::TextureView, sampler: &wgpu::Sampler| {
            self.device.create_bind_group(&wgpu::BindGroupDescriptor {
                label: None,
                layout: &self.tex_layout,
                entries: &[
                    wgpu::BindGroupEntry { binding: 0, resource: wgpu::BindingResource::TextureView(view) },
                    wgpu::BindGroupEntry { binding: 1, resource: wgpu::BindingResource::Sampler(sampler) },
                ],
            })
        };
        [group(&view, &self.sampler), group(&view, &self.sampler_rep), group(&raw, &self.sampler_raw), group(&raw, &self.sampler_rep)]
    }

    /// A texture for a video's frames (re/screens/video.md §6): one level, bilinear like the remaster's video
    /// texture, rewritten by [`Renderer::write_video`]; draw it with [`Blend::GammaAlpha`] (the shader's output goes
    /// to the frame as it is).
    pub fn video_texture(&mut self, width: u32, height: u32) -> TexId {
        let tex = self.device.create_texture(&wgpu::TextureDescriptor {
            label: Some("video"),
            size: wgpu::Extent3d { width, height, depth_or_array_layers: 1 },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::Rgba8UnormSrgb,
            usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
            view_formats: &[wgpu::TextureFormat::Rgba8Unorm],
        });
        let [a, b, c, e] = self.groups(&tex);
        self.textures.push(a);
        self.textures_rep.push(b);
        self.textures_raw.push(c);
        self.textures_raw_rep.push(e);
        let id = TexId(self.textures.len() - 1);
        self.videos.push((id, tex));
        id
    }

    /// Writes a frame into a [`Renderer::video_texture`] of the same size.
    pub fn write_video(&mut self, id: TexId, img: &Image) {
        let Some((_, tex)) = self.videos.iter().find(|(t, _)| *t == id) else { return };
        let size = tex.size();
        if (size.width, size.height) != (img.width, img.height) {
            return;
        }
        self.queue.write_texture(
            tex.as_image_copy(),
            &img.rgba,
            wgpu::TexelCopyBufferLayout { offset: 0, bytes_per_row: Some(img.width * 4), rows_per_image: Some(img.height) },
            size,
        );
    }

    /// Upload world-space triangles the renderer keeps until [`Renderer::clear_meshes`].
    /// Upload `img` in place of texture `id` (a video frame; the old texture is freed).
    pub fn replace_texture(&mut self, id: TexId, img: &Image) {
        let new = self.add_texture(img);
        for list in [&mut self.textures, &mut self.textures_rep, &mut self.textures_raw, &mut self.textures_raw_rep] {
            let g = list.swap_remove(new.0);
            if let Some(slot) = list.get_mut(id.0) {
                *slot = g;
            }
        }
    }

    pub fn add_mesh(&mut self, verts: &[crate::render3d::MeshVertex]) -> crate::render3d::MeshId {
        crate::prof::count("mesh", 1);
        self.pipe3d.add_mesh(&self.device, verts)
    }

    pub fn clear_meshes(&mut self) {
        self.pipe3d.clear_meshes();
    }

    /// Draw `frame` into `target` (`size` in physical pixels; made with [`Renderer::view_formats`]).
    pub fn draw(&mut self, frame: &Frame, target: &wgpu::Texture, size: (u32, u32), time: f32) {
        crate::prof::count("passes", 1);
        crate::prof::count("quads", frame.instances.len() as u64);
        crate::prof::count("batches", frame.batches.len() as u64);
        let u = Uniforms { screen: [size.0 as f32, size.1 as f32], time, _pad: 0.0 };
        self.queue.write_buffer(&self.uniform, 0, bytemuck::bytes_of(&u));
        // The backdrop's quad goes after the frame's own in the buffer.
        let backdrop = frame.backdrop.map(|(tex, c)| {
            let (w, h) = (size.0 as f32 / 2.0, size.1 as f32 / 2.0);
            (tex, Instance::new(kind::SPRITE, [w, h], [w, h], [c, c, c, 1.0]))
        });
        let mut all;
        let bytes: &[u8] = match backdrop {
            Some((_, i)) => {
                all = frame.instances.clone();
                all.push(i);
                bytemuck::cast_slice(&all)
            }
            None => bytemuck::cast_slice(&frame.instances),
        };
        let need = (bytes.len() as u64).max(std::mem::size_of::<Instance>() as u64);
        if self.instances.as_ref().is_none_or(|(_, cap)| *cap < need) {
            let cap = need.next_power_of_two();
            let buf = self.device.create_buffer(&wgpu::BufferDescriptor {
                label: Some("instances"),
                size: cap,
                usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
                mapped_at_creation: false,
            });
            self.instances = Some((buf, cap));
        }
        let (buf, _) = self.instances.as_ref().unwrap();
        if !bytes.is_empty() {
            self.queue.write_buffer(buf, 0, bytes);
        }
        let (view, raw) = (target.create_view(&Default::default()), raw_view(target));
        let mut enc = self.device.create_command_encoder(&Default::default());
        if let Some((tex, _)) = backdrop {
            // The exe's 2D pass: the picture's bytes times the grey, in gamma space, over the cleared target.
            let [r, g, b, a] = frame.clear;
            let mut pass = enc.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("backdrop"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: &raw,
                    resolve_target: None,
                    ops: wgpu::Operations { load: wgpu::LoadOp::Clear(wgpu::Color { r, g, b, a }), store: wgpu::StoreOp::Store },
                })],
                depth_stencil_attachment: None,
                timestamp_writes: None,
                occlusion_query_set: None,
            });
            pass.set_bind_group(0, &self.uniform_bg, &[]);
            pass.set_vertex_buffer(0, buf.slice(..));
            pass.set_pipeline(&self.gamma_alpha);
            pass.set_bind_group(1, &self.textures_raw[tex.0], &[]);
            let n = frame.instances.len() as u32;
            pass.draw(0..6, n..n + 1);
        }
        if let Some(scene) = &frame.scene {
            self.pipe3d.encode(&self.device, &self.queue, &mut enc, (&view, &raw), size, (true, backdrop.is_none().then_some(frame.clear)), scene, &scene.draws, (&self.textures_rep, &self.textures_raw_rep));
        }
        let late = frame.scene.as_ref().filter(|_| !frame.late.is_empty());
        let mut late_done = late.is_none();
        // One pass per run of batches in the same space: the gamma blends draw through the raw view.
        let mut start = 0;
        let mut first = true;
        while first || start < frame.batches.len() {
            if !late_done && start >= frame.late_at {
                if let Some(scene) = late {
                    self.pipe3d.encode(&self.device, &self.queue, &mut enc, (&view, &raw), size, (false, None), scene, &frame.late, (&self.textures_rep, &self.textures_raw_rep));
                }
                late_done = true;
            }
            let gamma = frame.batches.get(start).is_some_and(|b| b.blend.gamma());
            let mut end = (start..frame.batches.len()).find(|&i| frame.batches[i].blend.gamma() != gamma).unwrap_or(frame.batches.len());
            if !late_done && start < frame.late_at {
                end = end.min(frame.late_at);
            }
            let [r, g, b, a] = frame.clear;
            let load = if !first || frame.scene.is_some() || backdrop.is_some() { wgpu::LoadOp::Load } else { wgpu::LoadOp::Clear(wgpu::Color { r, g, b, a }) };
            let mut pass = enc.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: None,
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: if gamma { &raw } else { &view },
                    resolve_target: None,
                    ops: wgpu::Operations { load, store: wgpu::StoreOp::Store },
                })],
                depth_stencil_attachment: None,
                timestamp_writes: None,
                occlusion_query_set: None,
            });
            pass.set_bind_group(0, &self.uniform_bg, &[]);
            pass.set_vertex_buffer(0, buf.slice(..));
            for b in &frame.batches[start..end] {
                let (pipeline, textures) = match b.blend {
                    Blend::Alpha => (&self.alpha, &self.textures),
                    Blend::Additive => (&self.additive, &self.textures),
                    Blend::GammaAlpha => (&self.gamma_alpha, &self.textures_raw),
                    Blend::GammaAdditive => (&self.gamma_additive, &self.textures_raw),
                    Blend::GammaPremul => (&self.gamma_premul, &self.textures_raw),
                    Blend::MaskClear => (&self.mask[0], &self.textures_raw),
                    Blend::MaskMax => (&self.mask[1], &self.textures_raw),
                    Blend::MaskAdd => (&self.mask[2], &self.textures_raw),
                    Blend::MaskRestore => (&self.mask[3], &self.textures_raw),
                };
                pass.set_pipeline(pipeline);
                pass.set_bind_group(1, &textures[b.tex.0], &[]);
                pass.draw(0..6, b.range.clone());
            }
            (start, first) = (end, false);
        }
        if let (false, Some(scene)) = (late_done, late) {
            self.pipe3d.encode(&self.device, &self.queue, &mut enc, (&view, &raw), size, (false, None), scene, &frame.late, (&self.textures_rep, &self.textures_raw_rep));
        }
        self.queue.submit([enc.finish()]);
    }

    /// Draw `frame` into a new texture the quads can sample (the colony radar's picture of the terrain).
    pub fn render_texture(&mut self, frame: &Frame, size: (u32, u32)) -> TexId {
        let t = self.target(size);
        self.draw(frame, &t.texture, size, 0.0);
        t.tex
    }

    /// A texture the quads can sample that frames can be drawn into again and again ([`Renderer::draw_into`]).
    pub fn target(&mut self, size: (u32, u32)) -> Target {
        let (texture, [a, b, c, e]) = self.target_texture(size);
        self.textures.push(a);
        self.textures_rep.push(b);
        self.textures_raw.push(c);
        self.textures_raw_rep.push(e);
        Target { tex: TexId(self.textures.len() - 1), texture, size }
    }

    /// Draw `frame` into `t`, resized to `size` first (same [`TexId`]).
    pub fn draw_into(&mut self, frame: &Frame, t: &mut Target, size: (u32, u32), time: f32) {
        if t.size != size {
            let (texture, [a, b, c, e]) = self.target_texture(size);
            (self.textures[t.tex.0], self.textures_rep[t.tex.0], self.textures_raw[t.tex.0], self.textures_raw_rep[t.tex.0]) = (a, b, c, e);
            (t.texture, t.size) = (texture, size);
        }
        let texture = t.texture.clone();
        self.draw(frame, &texture, size, time);
    }

    fn target_texture(&mut self, size: (u32, u32)) -> (wgpu::Texture, [wgpu::BindGroup; 4]) {
        crate::prof::count("target", 1);
        let size = (size.0.max(1), size.1.max(1));
        let tex = self.device.create_texture(&wgpu::TextureDescriptor {
            label: Some("rendered"),
            size: wgpu::Extent3d { width: size.0, height: size.1, depth_or_array_layers: 1 },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: self.format,
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::TEXTURE_BINDING,
            view_formats: &Self::view_formats(self.format),
        });
        let groups = self.groups(&tex);
        (tex, groups)
    }

    /// The view formats a frame of `format` needs: its non-sRGB twin for the gamma blends.
    pub fn view_formats(format: wgpu::TextureFormat) -> Vec<wgpu::TextureFormat> {
        if format.remove_srgb_suffix() == format { vec![] } else { vec![format.remove_srgb_suffix()] }
    }

    /// Draw into an offscreen texture and read it back as RGBA8 (sRGB).
    pub fn draw_to_image(&mut self, frame: &Frame, size: (u32, u32), time: f32) -> Vec<u8> {
        let (w, h) = size;
        let tex = self.device.create_texture(&wgpu::TextureDescriptor {
            label: Some("snapshot"),
            size: wgpu::Extent3d { width: w, height: h, depth_or_array_layers: 1 },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::Rgba8UnormSrgb,
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::COPY_SRC,
            view_formats: &[wgpu::TextureFormat::Rgba8Unorm],
        });
        match self.snapshot_backdrop.filter(|_| frame.backdrop.is_none()) {
            Some(b) => self.draw(&Frame { backdrop: Some(b), ..frame.clone() }, &tex, size, time),
            None => self.draw(frame, &tex, size, time),
        }
        let row = (4 * w).div_ceil(256) * 256;
        let buf = self.device.create_buffer(&wgpu::BufferDescriptor {
            label: None,
            size: (row * h) as u64,
            usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ,
            mapped_at_creation: false,
        });
        let mut enc = self.device.create_command_encoder(&Default::default());
        enc.copy_texture_to_buffer(
            wgpu::TexelCopyTextureInfo { texture: &tex, mip_level: 0, origin: wgpu::Origin3d::ZERO, aspect: wgpu::TextureAspect::All },
            wgpu::TexelCopyBufferInfo { buffer: &buf, layout: wgpu::TexelCopyBufferLayout { offset: 0, bytes_per_row: Some(row), rows_per_image: Some(h) } },
            wgpu::Extent3d { width: w, height: h, depth_or_array_layers: 1 },
        );
        self.queue.submit([enc.finish()]);
        let slice = buf.slice(..);
        slice.map_async(wgpu::MapMode::Read, |_| ());
        let _ = self.device.poll(wgpu::PollType::Wait);
        let data = slice.get_mapped_range();
        let mut out = Vec::with_capacity((4 * w * h) as usize);
        for y in 0..h as usize {
            out.extend_from_slice(&data[y * row as usize..][..4 * w as usize]);
        }
        out
    }
}

/// `tex` viewed without its sRGB transfer (the same format when it has none).
fn raw_view(tex: &wgpu::Texture) -> wgpu::TextureView {
    tex.create_view(&wgpu::TextureViewDescriptor { format: Some(tex.format().remove_srgb_suffix()), ..Default::default() })
}

/// The image and its box-filtered mips down to 1×1 (in sRGB space; good enough for these assets).
fn mip_chain(img: &Image) -> Vec<Vec<u8>> {
    let mut out = vec![img.rgba.clone()];
    let (mut w, mut h) = (img.width as usize, img.height as usize);
    while w > 1 || h > 1 {
        let (nw, nh) = ((w / 2).max(1), (h / 2).max(1));
        let src = out.last().unwrap();
        let mut dst = vec![0u8; nw * nh * 4];
        for y in 0..nh {
            for x in 0..nw {
                for c in 0..4 {
                    let mut sum = 0u32;
                    for (dx, dy) in [(0, 0), (1, 0), (0, 1), (1, 1)] {
                        let (sx, sy) = ((2 * x + dx).min(w - 1), (2 * y + dy).min(h - 1));
                        sum += src[(sy * w + sx) * 4 + c] as u32;
                    }
                    dst[(y * nw + x) * 4 + c] = (sum / 4) as u8;
                }
            }
        }
        out.push(dst);
        (w, h) = (nw, nh);
    }
    out
}

/// `--snapshot --screen diplomacy`: the diplomacy screen at rest (re/screens/diplomacy.md), or with `--anim-frame F`
/// its opening held at frame F after 1000 frames (run_screendiff.py's wait, §3.1).
fn diplomacy_snapshot(o: &crate::Options, out: &std::path::Path, l: &crate::Loaded, world: &crate::world::World, r: &mut Renderer, hud: &mut crate::ui::Hud) -> Result<(), String> {
    let state = &world.game.state;
    let mut d = crate::diplomacy::Open::new(r, &l.install, &l.tables, &hud.texts, state);
    d.scene.acttime = o.acttime;
    let size = [o.size.0 as f32, o.size.1 as f32];
    let t = world.curr.tick as f32 * 0.05;
    match o.spy_frame {
        Some(f) => {
            d.screen.hold(f, 1000);
            let _ = d.input(state, hud, &l.tables, &crate::diplomacy::Input::default());
        }
        None => {
            d.screen.settle();
            let _ = d.input(state, hud, &l.tables, &crate::diplomacy::Input::default());
            d.screen.settle();
        }
    }
    let mut frame = Frame::new([0.0, 0.0, 0.0, 1.0]);
    d.draw(&mut frame, r, &l.install, &l.tables, hud, state, size, t);
    hud.draw(&mut frame, size, &world.ui, &l.tables, crate::ui::Screen::Diplomacy, t);
    hud.snapshot_pointer(&mut frame, size, o.mouse);
    let rgba = r.draw_to_image(&frame, o.size, t);
    crate::assets::write_snapshot(out, o, &rgba)?;
    let v = crate::diplomacy::View::read(state);
    println!("{}: diplomacy | selected {} | contacts {:?} | menus {:?}", out.display(), d.screen.selected, v.slots.iter().map(|s| (s.empire, s.waiting, s.answered)).collect::<Vec<_>>(), d.screen.shown);
    Ok(())
}

/// `--snapshot --screen diplomacy --script S`: the diplomacy screen opened animated (`Diplomacy_Belep` with
/// `KELLKINYITASDEFAULT`), run tick by tick with the script's input (re/screens/sound.md §3): one drawn frame a tick
/// (`Diplomacy3DUpdate`, then `FRISSIT_Diplomacy2`, then the 2D passes with the input of the events before the
/// tick), commands applied at once, F2 (or the client's D) `Diplomacy_Kilep`; grab t is drawn after t ticks.
fn diplomacy_sequence(o: &crate::Options, out: &std::path::Path, script: &crate::script::Script, l: &crate::Loaded, world: &mut crate::world::World, r: &mut Renderer, hud: &mut crate::ui::Hud) -> Result<(), String> {
    use crate::script::Input;
    let mut d = crate::diplomacy::Open::new(r, &l.install, &l.tables, &hud.texts, &world.game.state);
    d.enter(&l.tables, &hud.texts, &world.game.state, true);
    d.scene.acttime = o.acttime;
    let size = [o.size.0 as f32, o.size.1 as f32];
    let mut inp = crate::diplomacy::Input::default();
    let mut heard: Vec<(Option<u32>, crate::audio::SoundId)> = d.screen.sounds.drain(..).map(|id| (None, id)).collect();
    // F2 (`AktKepKell`) waits for the frame drawn after it: here a tick's `frame3d` stands for the draw before it, so
    // the request goes in after the next one.
    let mut leave = false;
    for tick in 0..=script.end() {
        if tick > 0 {
            d.screen.frame3d();
            if std::mem::take(&mut leave) {
                d.screen.leave(false);
            }
            d.screen.ui_tick();
            let cmds = d.input(&world.game.state, hud, &l.tables, &inp);
            inp.pressed = false;
            inp.released = false;
            heard.extend(d.screen.sounds.drain(..).map(|id| (Some(tick - 1), id)));
            if !cmds.is_empty() {
                for c in cmds {
                    world.send(c);
                }
                world.pump();
            }
        }
        if script.grabs.contains(&tick) {
            let t = tick as f32 * 0.01;
            let mut frame = Frame::new([0.0, 0.0, 0.0, 1.0]);
            d.paint(&mut frame, r, &l.install, &l.tables, hud, &world.game.state, size, t);
            hud.draw(&mut frame, size, &world.ui, &l.tables, crate::ui::Screen::Diplomacy, t);
            let rgba = r.draw_to_image(&frame, o.size, t);
            let path = crate::script::grab_path(out, tick);
            crate::assets::write_snapshot(&path, o, &rgba)?;
            println!("{}: t {tick} | dipframe {} open {} closing {} | selected {} menus {:?}", path.display(), d.screen.dipframe, d.screen.opened, d.screen.closing, d.screen.selected, d.screen.shown);
        }
        for e in script.at(tick) {
            match &e.input {
                Input::Move(at) => inp.at = *at,
                Input::Button { button: 1, down: true, at } => {
                    (inp.at, inp.pressed, inp.held) = (*at, true, true);
                }
                Input::Button { button: 1, down: false, at } => {
                    (inp.at, inp.released, inp.held) = (*at, true, false);
                }
                Input::Key { name, down: true } if name == "f2" || name == "d" => leave = true,
                _ => {}
            }
            heard.extend(d.screen.sounds.drain(..).map(|id| (Some(tick), id)));
        }
    }
    let path = crate::script::write_sounds(out, &heard)?;
    println!("{}: {} sounds", path.display(), heard.len());
    Ok(())
}

/// `--snapshot --colony P`: planet `P`'s colony screen.
fn colony_snapshot(o: &crate::Options, out: &std::path::Path, l: &crate::Loaded, world: &crate::world::World, r: &mut Renderer, hud: &mut crate::ui::Hud, p: usize) -> Result<(), String> {
    use crate::colony::{ColonyScene, ColonySnap, View};
    let curr = ColonySnap::read(&world.game.state, p).ok_or(format!("no planet {p}"))?;
    let prev = world.colony_prev.clone().filter(|s| s.planet == p).unwrap_or_else(|| curr.clone());
    let mut scene = ColonyScene::load(r, &l.install, &l.tables, &l.rules.ground_maps, &curr)?;
    let mut view = View::new(&curr, o.size, o.scale, &scene);
    if let Some([x, y, zoom, pitch, rotate]) = o.colony_cam {
        view.camera = crate::colony::Camera { x, y, zoom, pitch, rotate, ..view.camera };
    } else if o.colony_live {
        // The colony screen's own globals, saved as chunks (re/formats/save.md §4: `ColonyXStart/YStart/Rotate/Pitch/Zoom`).
        let f = |n: &str| crate::save_f32(l.save.as_ref(), n);
        view.camera = crate::colony::Camera { x: f("ColonyXStart"), y: f("ColonyYStart"), zoom: f("ColonyZoom"), pitch: f("ColonyPitch"), rotate: f("ColonyRotate"), ..view.camera };
    }
    let t = curr.tick as f32 * 0.05 + o.alpha * 0.05;
    // The screen's input state as entering leaves it, a second of UI frames on (the selection marker settled).
    let own = curr.owner as i32 == world.game.state.mem.i32(ig2_sim::save::va::EN) && !curr.at_war;
    let mut ui = crate::colony_ui::ColonyUi::enter(&curr, own, &world.game.state);
    if o.colony_live && own {
        // `AktEpulet` isn't saved: entering selected the first building (`SelectBuildingRelative(1)`) [inf].
        ui.selected = curr.buildings.first().map_or(0, |b| b.slot);
    }
    // `--colony-ui war[:N]`: the units the exe's save has selected (flag 8, colony.md §11), `SpecWeaponMode` N.
    if let Some(m) = o.colony_ui.as_deref().and_then(|m| m.strip_prefix("war")) {
        ui.war.seed(curr.units.iter().filter(|u| u.flags & 8 != 0).map(|u| u.slot));
        ui.war.armed = m.strip_prefix(':').and_then(|n| n.parse().ok()).unwrap_or(0);
        // `run_screen.py --grwar` calls `StartGrWar` just before the grab: `TamadasSzovegCounter` 900.
        ui.war.banner = 900;
    }
    for _ in 0..100 {
        ui.ui_frame(&curr, world.game.state.mem.i32(ig2_sim::save::va::EN), &mut view.camera);
    }
    let mut cursor = [320.0, 240.0];
    let mut now_ms = 0;
    if let Some(mode) = o.colony_ui.as_deref() {
        let texts = &hud.texts;
        let names = |t: usize| hud.tech_name(t);
        let c = crate::colony_ui::Ctx { state: &world.game.state, snap: &curr, rules: &l.tables, sim: &l.rules, texts, tech_name: &names };
        match mode {
            "pick" | "build" => {
                ui.press(&c, [100.0, 80.0], true, 0);
                cursor = [150.0, 90.0];
                ui.drag(&c, cursor);
                now_ms = 1000;
                if mode == "build" {
                    ui.release(&c, cursor, true, now_ms);
                    let gm = &l.rules.ground_maps[curr.map];
                    let px = [o.size.0 as f32 * 0.5, o.size.1 as f32 * 0.55];
                    ui.hover(&c, crate::colony_ui::cell_under(&view, gm, px), crate::colony_ui::cursor_cell_under(&view, gm, px));
                }
            }
            "demolish" => {
                ui.press(&c, [220.0, 450.0], true, 0);
                cursor = [250.0, 440.0];
            }
            m if m.starts_with("war") => {}
            // The planet info mode (colony.md §12), with `:KIND` its menu opened by a press on the hot-spot's centre.
            m if m == "info" || m.starts_with("info:") => {
                ui.key(&c, crate::colony_ui::Key::Info);
                if let Some(kind) = m.strip_prefix("info:") {
                    let at = match kind {
                        "tax" => [450.0, 297.0],
                        "autobuild" => [450.0, 319.0],
                        "priority" => [450.0, 341.0],
                        k => return Err(format!("--colony-ui info:{k}: tax, autobuild or priority")),
                    };
                    ui.press(&c, at, true, 0);
                    cursor = at;
                    if let Some(m) = o.mouse {
                        ui.drag(&c, m);
                        ui.menu_frame();
                    }
                }
            }
            m => return Err(format!("--colony-ui {m}: pick, demolish, build, war[:N] or info[:KIND]")),
        }
    }
    view.overlay = ui.overlay(&curr, world.game.state.mem.i32(ig2_sim::save::va::EN));
    if let Some(gm) = l.rules.ground_maps.get(curr.map) {
        ui.war.project(&view, gm, &curr, &[]);
    }
    // `--mouse X,Y`: the game's pointer there (colony.md §11.10).
    if let Some(m) = o.mouse {
        cursor = m;
        if let Some(gm) = l.rules.ground_maps.get(curr.map) {
            let px = [m[0] * o.size.0 as f32 / 640.0, m[1] * o.size.1 as f32 / 480.0];
            ui.cell = crate::colony_ui::cell_under(&view, gm, px);
            ui.war.pointer(ui.cell);
        }
    }
    let size = [o.size.0 as f32, o.size.1 as f32];
    let mut panel_model = crate::colony_ui::PanelBuilding::default();
    let panel = panel_model.render(r, &l.install, &l.tables, ui.kind, crate::colony::landing_unit(&world.game.state, &curr), ui.spin(), size);
    let en = world.game.state.mem.i32(ig2_sim::save::va::EN);
    let info_model = crate::colony_ui::render_kis_info(r, &l.install, &l.tables, &ui, &mut scene, &mut panel_model, &curr, en, size);
    let mut frame = view.build(r, &l.install, &l.tables, &l.rules.ground_maps, &mut scene, &prev, &curr, o.alpha, t);
    // The drawn units' `GetObjCenter`s for the labels.
    if let Some(gm) = l.rules.ground_maps.get(curr.map) {
        ui.war.project(&view, gm, &curr, &scene.unit_centres);
    }
    let input = crate::ui::ColonyInput { ui: &ui, sim: &l.rules, panel, cursor, pointer: false, info_model, now_ms, time: t, names: true, rename: None };
    hud.draw_colony(&mut frame, &world.game.state, &l.tables, &l.rules.ground_maps, &view, &scene, &curr, &input);
    hud.draw(&mut frame, [o.size.0 as f32, o.size.1 as f32], &world.ui, &l.tables, crate::ui::Screen::Colony, t);
    // `UpdateFrame`'s last pass at `--mouse` (re/screens/input.md §8): the hot-spot's tooltip (the pointer at rest),
    // then the pointer of its type (colony.md §11.10).
    if let Some(m) = o.mouse {
        let names = |t: usize| hud.tech_name(t);
        let c = crate::colony_ui::Ctx { state: &world.game.state, snap: &curr, rules: &l.tables, sim: &l.rules, texts: &hud.texts, tech_name: &names };
        let tip = ui.tip(&c, m).map(|(_, t)| t);
        hud.snapshot_tip_pointer(&mut frame, [o.size.0 as f32, o.size.1 as f32], m, tip.as_deref(), ui.cursor(&c, m), ui.mouse_phase().max(0) as u64);
    }
    let rgba = r.draw_to_image(&frame, o.size, t);
    crate::assets::write_snapshot(out, o, &rgba)?;
    let gm = &l.rules.ground_maps[curr.map];
    let e = view.camera.eye(gm);
    println!(
        "{}: planet {p} map {} surface {} owner {} | {} buildings, {} units, {} shots | camera {:?} eye {:?} target {:?}",
        out.display(),
        curr.map,
        curr.surface,
        curr.owner,
        curr.buildings.len(),
        curr.units.len(),
        curr.shots.len(),
        curr.camera,
        e.eye,
        e.target
    );
    let basis = view.camera.basis(gm);
    println!("  weather {} background {} fog {:?}", curr.weather, curr.background, scene.fog());
    for (b, (slot, at)) in curr.buildings.iter().zip(&scene.building_at) {
        println!("  building {slot:3} kind {:2} grid ({}, {}) work {} at {at:?} -> {:?}", b.kind, b.x, b.y, b.work_left, view.camera.project(&basis, *at));
    }
    for u in &curr.units {
        println!(
            "  unit {:3} owner {:2} hull {} flags {:08x} gun {:?} design {} at {:?} -> {:?}",
            u.slot,
            u.owner,
            u.hull,
            u.flags,
            u.gun,
            u.design,
            u.pos,
            view.camera.project(&basis, u.pos)
        );
    }
    Ok(())
}

/// `--snapshot --screen research`: the research screen on the save's selection after `--ui-frames` 10 ms frames,
/// open (or with `--anim-frame F [--anim-closing]` held at frame F of its opening or closing), the pointer resting at
/// `--mouse X,Y` (a hot-spot's tooltip).
fn research_snapshot(o: &crate::Options, out: &std::path::Path, l: &crate::Loaded, world: &crate::world::World, r: &mut Renderer, hud: &mut crate::ui::Hud) -> Result<(), String> {
    let state = &world.game.state;
    // Animated like the exe (`KELLKINYITASDEFAULT` 1): the info box and the wire sphere stay where `MoveObj` puts
    // them also at rest.
    let mut s = crate::research::Research::open(state, &l.tables, true);
    match o.spy_frame {
        Some(f) => s.hold(f, o.spy_closing),
        None => s.hold_open(),
    }
    if let Some(m) = o.mouse {
        s.tip.mouse = m;
    }
    let infos = crate::research::tech_infos(&l.install);
    let tips = crate::research::tips(&hud.texts);
    let mut models = crate::research::ResearchModels::new(r, &l.install);
    for _ in 0..o.ui_frames.max(1) {
        s.step(state);
        s.tooltip(state, &models.mask, &tips, s.frames as f64 * 10.0);
    }
    let size = [o.size.0 as f32, o.size.1 as f32];
    let (tex, lines) = models.render(r, &l.install, &l.tables, &s, state, size);
    let t = world.curr.tick as f32 * 0.05;
    let mut frame = Frame::new([0.0, 0.0, 0.0, 1.0]);
    hud.draw_research(&mut frame, &mut s, &infos, &tips, state, &l.tables, (tex, models.glow, models.arrow), lines, size, t);
    hud.draw(&mut frame, size, &world.ui, &l.tables, crate::ui::Screen::Research, t);
    hud.snapshot_pointer(&mut frame, size, o.mouse);
    let rgba = r.draw_to_image(&frame, o.size, t);
    crate::assets::write_snapshot(out, o, &rgba)?;
    println!("{}: research area {} field {} tech {} | models at {:?}", out.display(), s.area, s.field, s.tech, models.centres);
    Ok(())
}

/// What the design screen reads ([`crate::design_edit::Ctx`]).
fn design_ctx<'a>(state: &'a ig2_sim::save::SaveState, techs: &'a ig2_sim::colony_side::TechTable, l: &'a crate::Loaded, world: &'a crate::world::World, hud: &'a crate::ui::Hud) -> crate::design_edit::Ctx<'a> {
    crate::design_edit::Ctx {
        install: &l.install,
        state,
        techs,
        rules: &l.tables,
        sim: &world.game.rules.ai,
        debug: state.mem.i32(ig2_sim::save::va::DESIGN_DEBUG) != 0,
        texts: Some(&hud.texts),
    }
}

/// `--snapshot --screen design`: the design screen on the save's chosen design after `--ui-frames` 10 ms frames,
/// open (the hull slid in and turning), the pointer resting at `--mouse X,Y` (a hot-spot's tooltip).
fn design_snapshot(o: &crate::Options, out: &std::path::Path, l: &crate::Loaded, world: &crate::world::World, r: &mut Renderer, hud: &mut crate::ui::Hud) -> Result<(), String> {
    let state = &world.game.state;
    let techs = state.tech();
    let mut s = crate::design::Design::open(state, true);
    s.hold_open();
    if let Some(m) = o.mouse {
        s.tip.mouse = m;
    }
    let tips = crate::design::tips(&hud.texts);
    let mut scene = crate::design_scene::DesignScene::new(r, &l.install);
    {
        let c = design_ctx(state, &techs, l, world, hud);
        for _ in 0..o.ui_frames.max(1) {
            s.step(&c, &scene.mask);
            s.tooltip(&c, &scene.mask, &tips, s.frames as f64 * 10.0);
        }
    }
    let size = [o.size.0 as f32, o.size.1 as f32];
    let t = world.curr.tick as f32 * 0.05;
    let en = state.mem.i32(ig2_sim::save::va::EN).clamp(0, 18) as usize;
    let mut frame = Frame::new([0.0, 0.0, 0.0, 1.0]);
    scene.draw(&mut frame, r, &l.install, &l.tables, &mut s, state, &techs, en, size);
    hud.draw_design(&mut frame, &mut s, state, &techs, &l.tables, &tips, size, t);
    hud.draw(&mut frame, size, &world.ui, &l.tables, crate::ui::Screen::Design, t);
    hud.snapshot_pointer(&mut frame, size, o.mouse);
    let rgba = r.draw_to_image(&frame, o.size, t);
    crate::assets::write_snapshot(out, o, &rgba)?;
    println!("{}: design {} (rank {} of {}) model {} zoom {}", out.display(), s.current(), s.pointer, s.count(), s.loaded, s.add_zoom);
    Ok(())
}

/// `--snapshot --screen design --script S`: the design screen opened animated (`Design_Belep` with
/// `KELLKINYITASDEFAULT`), run tick by tick with the script's input (re/screens/sound.md §3): the pointer, the left
/// button as `Design_Main`'s press and release, the right one turning the hull, up and down
/// (`DesignKeyboardHandler`), r and Enter (redesign, modify; Enter ends the name input), Esc
/// (`DesignKeyboardEscPressed`), a typed name (keys of one character), F2 (`Design_Kilep`); the orders are applied at
/// once as the exe's `MULTIGET_Design` and their events go back to the screen; grab t is drawn after t ticks.
fn design_sequence(o: &crate::Options, out: &std::path::Path, script: &crate::script::Script, l: &crate::Loaded, world: &mut crate::world::World, r: &mut Renderer, hud: &mut crate::ui::Hud) -> Result<(), String> {
    use crate::design::Key;
    use crate::design_edit::EntryKey;
    use crate::script::Input;
    let mut s = crate::design::Design::open(&world.game.state, true);
    s.names = crate::design_edit::Names::read(&hud.texts);
    s.tip.moved([320.0, 240.0]);
    let tips = crate::design::tips(&hud.texts);
    let mut scene = crate::design_scene::DesignScene::new(r, &l.install);
    let size = [o.size.0 as f32, o.size.1 as f32];
    let t = world.curr.tick as f32 * 0.05;
    let en = world.game.state.mem.i32(ig2_sim::save::va::EN).clamp(0, 18) as usize;
    let mut heard: Vec<(Option<u32>, crate::audio::SoundId)> = s.sounds.drain(..).map(|id| (None, id)).collect();
    let mut sent = Vec::new();
    for tick in 0..=script.end() {
        if tick > 0 {
            let techs = world.game.state.tech();
            let c = design_ctx(&world.game.state, &techs, l, world, hud);
            s.step(&c, &scene.mask);
            s.tooltip(&c, &scene.mask, &tips, s.frames as f64 * 10.0);
            heard.extend(s.sounds.drain(..).map(|id| (Some(tick - 1), id)));
            // The tick's `Design_Main` sent them: `MULTIGET_Design` at the next frame's start.
            for o in std::mem::take(&mut s.orders) {
                sent.push(format!("t {}: {o:?}", tick - 1));
                world.send(ig2_net::Command::from(o));
            }
            world.pump();
            for ev in std::mem::take(&mut world.order_events) {
                s.event(&ev);
            }
        }
        if script.grabs.contains(&tick) {
            let state = &world.game.state;
            let techs = state.tech();
            let mut frame = Frame::new([0.0, 0.0, 0.0, 1.0]);
            scene.draw(&mut frame, r, &l.install, &l.tables, &mut s, state, &techs, en, size);
            hud.draw_design(&mut frame, &mut s, state, &techs, &l.tables, &tips, size, t);
            hud.draw(&mut frame, size, &world.ui, &l.tables, crate::ui::Screen::Design, t);
            // `UpdateFrame`'s pointer, the hand over a pickable pod or a model (phase 0: deviation 277).
            hud.snapshot_tip_pointer(&mut frame, size, s.mouse, None, (s.tip.under != 0 || s.pods.hand) as i32, 0);
            let rgba = r.draw_to_image(&frame, o.size, t);
            let path = crate::script::grab_path(out, tick);
            crate::assets::write_snapshot(&path, o, &rgba)?;
            println!("{}: t {tick} | design {} rank {} first {} | mode {} pod {} | desframe {} open {} model {} zoom {}", path.display(), s.current(), s.pointer, s.first, s.mode, s.pod, s.frame, s.opened, s.loaded, s.add_zoom);
        }
        for e in script.at(tick) {
            let mut rename = None;
            {
                let techs = world.game.state.tech();
                let c = design_ctx(&world.game.state, &techs, l, world, hud);
                match &e.input {
                    Input::Move(at) => s.moved(*at),
                    Input::Button { button: 1, down: true, at } => s.press(&c, &scene.mask, *at),
                    Input::Button { button: 3, down: true, at } => s.press_right(*at),
                    Input::Button { button: 1, down: false, .. } => s.release(&c),
                    Input::Button { down: false, .. } => s.release_right(),
                    Input::Key { name, down: true } if s.entry.is_some() && name != "escape" => {
                        let k = match name.as_str() {
                            "enter" => {
                                s.entry = None;
                                None
                            }
                            "back" | "backspace" => Some(EntryKey::Back),
                            "delete" => Some(EntryKey::Delete),
                            "space" => Some(EntryKey::Char(b' ')),
                            n if n.len() == 1 => Some(EntryKey::Char(n.as_bytes()[0])),
                            _ => None,
                        };
                        rename = k.and_then(|k| s.entry_key(k));
                    }
                    Input::Key { name, down: true } => match name.as_str() {
                        "up" => s.key(Key::Up),
                        "down" => s.key(Key::Down),
                        "r" => s.edit_key(&c, true),
                        "enter" => s.edit_key(&c, false),
                        "escape" => {
                            s.escape(&c);
                        }
                        // F2 asks for the starmap (the closing runs).
                        "f2" => {
                            s.leave();
                        }
                        _ => {}
                    },
                    _ => {}
                }
            }
            if let Some((k, name)) = rename {
                if let Some(rec) = world.game.state.designs.get_mut(en).and_then(|t| t.get_mut(k)) {
                    let n = name.len().min(0x15);
                    rec[..n].copy_from_slice(&name[..n]);
                    rec[n] = 0;
                }
            }
            for o in std::mem::take(&mut s.orders) {
                sent.push(format!("t {tick}: {o:?}"));
                world.send(ig2_net::Command::from(o));
            }
            world.pump();
            for ev in std::mem::take(&mut world.order_events) {
                s.event(&ev);
            }
            heard.extend(s.sounds.drain(..).map(|id| (Some(tick), id)));
        }
    }
    for line in &sent {
        println!("order {line}");
    }
    let path = crate::script::write_sounds(out, &heard)?;
    println!("{}: {} sounds", path.display(), heard.len());
    Ok(())
}

/// `--snapshot --screen trade`: the trade screen after `--ui-frames` 10 ms frames, open, the pointer resting at
/// `--mouse X,Y` (a hot-spot's tooltip).
fn trade_snapshot(o: &crate::Options, out: &std::path::Path, l: &crate::Loaded, world: &mut crate::world::World, r: &mut Renderer, hud: &mut crate::ui::Hud) -> Result<(), String> {
    if o.trade_cheat {
        world.game.state.mem.set_u8(ig2_sim::save::va::TRADE_CHEAT, 1);
    }
    if let Some(d) = o.trade_eta {
        world.game.state.mem.set_i32(ig2_sim::save::va::TRADE_ETA_DAYS, d);
    }
    let state = &world.game.state;
    let advance = hud.font.as_ref().map_or(6.0, |f| f.advance(2));
    let mut s = crate::trade::Trade::open(state, true, 2, crate::trade::ticker_lines(&hud.texts), advance);
    if let Some([m, c, n]) = o.dkm {
        s.list_frame = crate::kit::DesignFrame::with_phase(m, c, n);
    }
    s.hold_open();
    if let Some(m) = o.mouse {
        s.moved(m);
    }
    let tips = crate::trade::tips(&hud.texts);
    let mut scene = crate::trade_scene::TradeScene::new(r);
    for _ in 0..o.ui_frames.max(1) {
        s.step(state);
        s.tooltip(&tips, s.frames as f64 * 10.0);
    }
    let size = [o.size.0 as f32, o.size.1 as f32];
    let t = world.curr.tick as f32 * 0.05;
    let mut frame = Frame::new([0.0, 0.0, 0.0, 1.0]);
    scene.draw(&mut frame, r, &l.install, &l.tables, &s, size);
    hud.draw_trade(&mut frame, &s, state, &l.tables, &tips, size, t);
    hud.draw(&mut frame, size, &world.ui, &l.tables, crate::ui::Screen::Trade, t);
    hud.snapshot_pointer(&mut frame, size, o.mouse);
    let rgba = r.draw_to_image(&frame, o.size, t);
    crate::assets::write_snapshot(out, o, &rgba)?;
    println!("{}: trade | ports {:?} | which {} kind {} qty {} planet {} | traframe {} open {}", out.display(), crate::trade::ports(state), s.which, s.kind, s.qty, s.planet, s.frame, s.opened);
    Ok(())
}

/// `--snapshot --screen trade --script S`: the trade screen opened animated (`Trade_Belep` with
/// `KELLKINYITASDEFAULT`), run tick by tick with the script's input (re/screens/sound.md §3): events at t go in before
/// tick t + 1 (the pointer, both buttons as `Trade_Main`'s input, F2 `Trade_Kilep`); OK's message 0x51 is applied at
/// once as the exe's `MULTIGET_Trade`; grab t is drawn after t ticks.
fn trade_sequence(o: &crate::Options, out: &std::path::Path, script: &crate::script::Script, l: &crate::Loaded, world: &mut crate::world::World, r: &mut Renderer, hud: &mut crate::ui::Hud) -> Result<(), String> {
    use crate::script::Input;
    if o.trade_cheat {
        world.game.state.mem.set_u8(ig2_sim::save::va::TRADE_CHEAT, 1);
    }
    if let Some(d) = o.trade_eta {
        world.game.state.mem.set_i32(ig2_sim::save::va::TRADE_ETA_DAYS, d);
    }
    let advance = hud.font.as_ref().map_or(6.0, |f| f.advance(2));
    let mut s = crate::trade::Trade::open(&world.game.state, true, 2, crate::trade::ticker_lines(&hud.texts), advance);
    if let Some([m, c, n]) = o.dkm {
        s.list_frame = crate::kit::DesignFrame::with_phase(m, c, n);
    }
    s.tip.moved([320.0, 240.0]);
    let tips = crate::trade::tips(&hud.texts);
    let mut scene = crate::trade_scene::TradeScene::new(r);
    let size = [o.size.0 as f32, o.size.1 as f32];
    let t = world.curr.tick as f32 * 0.05;
    let mut heard: Vec<(Option<u32>, crate::audio::SoundId)> = s.sounds.drain(..).map(|id| (None, id)).collect();
    for tick in 0..=script.end() {
        if tick > 0 {
            s.step(&world.game.state);
            s.tooltip(&tips, s.frames as f64 * 10.0);
            heard.extend(s.sounds.drain(..).map(|id| (Some(tick - 1), id)));
            let orders = std::mem::take(&mut s.orders);
            if !orders.is_empty() {
                for delivery in orders {
                    world.send(ig2_net::Command::Buy { delivery });
                }
                world.pump();
            }
        }
        if script.grabs.contains(&tick) {
            let mut frame = Frame::new([0.0, 0.0, 0.0, 1.0]);
            scene.draw(&mut frame, r, &l.install, &l.tables, &s, size);
            hud.draw_trade(&mut frame, &s, &world.game.state, &l.tables, &tips, size, t);
            hud.draw(&mut frame, size, &world.ui, &l.tables, crate::ui::Screen::Trade, t);
            let rgba = r.draw_to_image(&frame, o.size, t);
            let path = crate::script::grab_path(out, tick);
            crate::assets::write_snapshot(&path, o, &rgba)?;
            println!("{}: t {tick} | which {} kind {} qty {} planet {} | traframe {} open {}", path.display(), s.which, s.kind, s.qty, s.planet, s.frame, s.opened);
        }
        for e in script.at(tick) {
            match &e.input {
                Input::Move(at) => s.moved(*at),
                Input::Button { button, down: true, at } => s.press(*button == 1, *at),
                Input::Button { button, down: false, at } => s.release(*button == 1, *at),
                Input::Key { name, down: true } if name == "f2" => {
                    s.leave(2);
                }
                _ => {}
            }
            heard.extend(s.sounds.drain(..).map(|id| (Some(tick), id)));
        }
    }
    let path = crate::script::write_sounds(out, &heard)?;
    println!("{}: {} sounds", path.display(), heard.len());
    Ok(())
}

/// `--snapshot --screen research --script S`: the research screen opened animated (as `Research_Belep` with
/// `KELLKINYITASDEFAULT`), run tick by tick with the script's input (re/screens/sound.md §3): events at t go in before
/// tick t + 1 (the pointer, the left button as `Research_Main`'s press, `ResearchKeyboardHandler`'s keys), their
/// commands applied at once as the exe's `MULTIGET_Research`; grab t is drawn after t ticks. Esc ends the sequence:
/// `ESC_Pressed` puts up the options screen at once, without the closing (re/screens/options.md §5.1).
fn research_sequence(o: &crate::Options, out: &std::path::Path, script: &crate::script::Script, l: &crate::Loaded, world: &mut crate::world::World, r: &mut Renderer, hud: &mut crate::ui::Hud) -> Result<(), String> {
    use crate::research::{Act, Key};
    use crate::script::Input;
    let mut s = crate::research::Research::open(&world.game.state, &l.tables, true);
    s.tip.moved([320.0, 240.0]);
    let infos = crate::research::tech_infos(&l.install);
    let tips = crate::research::tips(&hud.texts);
    let mut models = crate::research::ResearchModels::new(r, &l.install);
    let size = [o.size.0 as f32, o.size.1 as f32];
    let t = world.curr.tick as f32 * 0.05;
    let mut rendered = None;
    // The sounds by the tracer's `t` (the ticks done when the frame began): tick k's own sounds at k − 1, an event
    // at t's at t (it goes in at the frame before tick t + 1).
    let mut heard: Vec<(Option<u32>, crate::audio::SoundId)> = s.sounds.drain(..).map(|id| (None, id)).collect();
    // Per tick: the tick, the grab (the frame drawn after it), then the events that go in before the next tick.
    'ticks: for tick in 0..=script.end() {
        if tick > 0 {
            s.step(&world.game.state);
            s.tooltip(&world.game.state, &models.mask, &tips, s.frames as f64 * 10.0);
            rendered = None;
            heard.extend(s.sounds.drain(..).map(|id| (Some(tick - 1), id)));
        }
        if script.grabs.contains(&tick) {
            let (tex, lines) = models.render(r, &l.install, &l.tables, &s, &world.game.state, size);
            let mut frame = Frame::new([0.0, 0.0, 0.0, 1.0]);
            hud.draw_research(&mut frame, &mut s, &infos, &tips, &world.game.state, &l.tables, (tex, models.glow, models.arrow), lines, size, t);
            hud.draw(&mut frame, size, &world.ui, &l.tables, crate::ui::Screen::Research, t);
            // `UpdateFrame`'s pointer at the script's point, the hand over a row or a model (phase 0: deviation 277).
            let m = s.tip.mouse;
            hud.snapshot_tip_pointer(&mut frame, size, m, None, (s.tip.under != 0 || s.hand(&world.game.state, m, &models.centres)) as i32, 0);
            let rgba = r.draw_to_image(&frame, o.size, t);
            let path = crate::script::grab_path(out, tick);
            crate::assets::write_snapshot(&path, o, &rgba)?;
            println!("{}: t {tick} | research area {} field {} tech {} | frames {} resframe {} open {}", path.display(), s.area, s.field, s.tech, s.frames, s.frame, s.opened);
        }
        for e in script.at(tick) {
            let acts = match &e.input {
                Input::Move(at) => {
                    s.tip.moved(*at);
                    Vec::new()
                }
                // The top bar's spots (re/screens/input.md §6: 2..5 at x 0..73, 7 at 270..593, the drop-downs 8 and 6 at
                // 598..640) are over the screen's own; the drop-downs aren't run here.
                Input::Button { button: 1, down: true, at } if at[1] < 20.0 && (at[0] < 73.0 || (270.0..593.0).contains(&at[0]) || at[0] >= 598.0) => Vec::new(),
                Input::Button { button: 3, down, .. } => {
                    s.buttons = if *down { s.buttons | 2 } else { s.buttons & !2 };
                    Vec::new()
                }
                Input::Button { button: 1, down: true, at } => {
                    s.held = true;
                    s.buttons |= 1;
                    if s.closing {
                        Vec::new()
                    } else {
                        // The models' projected centres of the last drawn frame (`CheckIfTalalmanyokFolottVan`).
                        if rendered.is_none() {
                            rendered = Some(models.render(r, &l.install, &l.tables, &s, &world.game.state, size));
                        }
                        s.press(&world.game.state, &l.tables, &models.mask, *at, &models.centres)
                    }
                }
                Input::Button { button: 1, down: false, .. } => {
                    s.held = false;
                    s.buttons &= !1;
                    Vec::new()
                }
                Input::Key { name, down: true } => {
                    let k = match name.as_str() {
                        "down" => Some(Key::Down),
                        "up" => Some(Key::Up),
                        "right" => Some(Key::Right),
                        "left" => Some(Key::Left),
                        "enter" => Some(Key::Enter),
                        // `remapKeyCodes` swaps them (the window's `research_key`).
                        "pageup" => Some(Key::PageDown),
                        "pagedown" => Some(Key::PageUp),
                        "escape" => {
                            println!("t {tick}: Esc, the options screen (AktKep 13)");
                            break 'ticks;
                        }
                        // The other key downs `remapKeyCodes` queues reach the handler (`oldEnterlenyomva`); letters don't.
                        "space" | "1" | "2" | "3" | "tab" => {
                            s.key_seen(true);
                            None
                        }
                        _ => None,
                    };
                    k.map_or_else(Vec::new, |k| s.key(&world.game.state, k))
                }
                Input::Key { down: false, .. } => {
                    s.key_seen(false);
                    Vec::new()
                }
                _ => Vec::new(),
            };
            for a in acts {
                let Act::Send(c) = a;
                world.send(c);
            }
            world.pump();
            heard.extend(s.sounds.drain(..).map(|id| (Some(tick), id)));
        }
    }
    let path = crate::script::write_sounds(out, &heard)?;
    println!("{}: {} sounds", path.display(), heard.len());
    Ok(())
}

/// `--snapshot --screen messages --script`: the screen entered at tick 0 (`Messages_Belep`, the opening from
/// `mesframe` 0, re/screens/messages.md §4.1), each tick one drawn frame as in the exe's script runs: the UI tick, then
/// the render (`Messages_Kilep`'s loop test, `Messages3DUpdate`), the grab, then the events before the next tick. Black
/// under the frame (a bare jump); the talking-head clip isn't played.
fn messages_sequence(o: &crate::Options, out: &std::path::Path, script: &crate::script::Script, l: &crate::Loaded, world: &mut crate::world::World, r: &mut Renderer, hud: &mut crate::ui::Hud) -> Result<(), String> {
    use crate::audio::SoundId;
    use crate::messages_screen::Press;
    use crate::script::Input;
    use ig2_rules::sounds::Table;
    let size = [o.size.0 as f32, o.size.1 as f32];
    let t = world.curr.tick as f32 * 0.05;
    let mut models = crate::screen_frames::FrameModels::new(r);
    hud.messages.debounce = 5;
    hud.messages.enter(true);
    let mut heard: Vec<(Option<u32>, SoundId)> = vec![(None, SoundId::new(Table::Mes, 0))];
    let mut mouse = [320.0, 240.0];
    for tick in 0..=script.end() {
        if tick > 0 {
            let m = &mut hud.messages;
            m.ui_frame();
            m.tick();
            m.answer();
            let typing = m.text_up();
            if let Some(d) = m.open.as_mut().filter(|_| typing) {
                d.advance(10.0);
            }
        }
        if hud.messages.render_step() {
            println!("t {tick}: the closing has run out, the screen goes");
            break;
        }
        if script.grabs.contains(&tick) {
            hud.frame_tex = models.render_messages(r, &l.install, hud.messages.frame, size);
            let mut frame = Frame::new([0.0, 0.0, 0.0, 1.0]);
            hud.draw(&mut frame, size, &world.ui, &l.tables, crate::ui::Screen::Messages, t);
            hud.snapshot_pointer(&mut frame, size, Some(mouse));
            let rgba = r.draw_to_image(&frame, o.size, t);
            let path = crate::script::grab_path(out, tick);
            crate::assets::write_snapshot(&path, o, &rgba)?;
            let m = &hud.messages;
            println!("{}: t {tick} | mesframe {} open {} closing {} message {}", path.display(), m.frame, m.opened, m.closing, m.open.is_some());
        }
        if tick > 0 {
            // The input of the frame before, after this frame's ticks and render: `IG2_gameUpdate` and `Messages_Main`
            // come last in the exe's frame (an Esc or a press at t is taken after the render of t + 1; the closing's
            // first step is tick t + 2).
            for e in script.at(tick - 1) {
                match &e.input {
                    Input::Move(at) => {
                        mouse = *at;
                        hud.messages.hover(*at);
                    }
                    Input::Button { button: 1 | 3, down: true, at } => {
                        mouse = *at;
                        match hud.messages.press(*at, world.ui.inbox.len()) {
                            Press::Open(i) => hud.open_message(&l.install, &world.ui, i),
                            Press::Leave => {
                                hud.messages.leave();
                            }
                            Press::None => {}
                        }
                    }
                    Input::Key { name, down: true } if name == "escape" => {
                        let idle = hud.messages.open.as_ref().is_none_or(|d| d.answers.is_empty() && d.typed_all);
                        if idle && !hud.messages.closing {
                            hud.messages.leave();
                        }
                    }
                    Input::Key { name, down: true } => hud.messages.key(name),
                    _ => {}
                }
                let mut orders = std::mem::take(&mut hud.messages.orders);
                if let Some(d) = &mut hud.messages.open {
                    orders.append(&mut d.orders);
                }
                for c in orders {
                    world.send(c.into());
                }
                world.pump();
            }
            if std::mem::take(&mut hud.messages.kilep) {
                heard.push((Some(tick), SoundId::new(Table::Mes, 1)));
            }
        }
    }
    let path = crate::script::write_sounds(out, &heard)?;
    println!("{}: {} sounds", path.display(), heard.len());
    Ok(())
}

/// `--snapshot --backdrop`: the starmap as a running game's exit leaves it in `pic` (crate::backdrop): without its
/// HUD (`SaveScreen(1)`), or with it (`SaveScreen(2)`, messages and options).
pub fn starmap_picture(o: &crate::Options, l: &crate::Loaded, world: &crate::world::World<'_>, r: &mut Renderer, hud: &mut crate::ui::Hud, pic: crate::backdrop::Pic) -> TexId {
    let assets = crate::starmap::Assets::load(r, &l.install);
    let view = snapshot_view(o, world);
    let t = world.curr.tick as f32 * 0.05 + o.alpha * 0.05;
    let mut frame = view.build(&assets, &world.prev, &world.curr, o.alpha, t);
    if pic == crate::backdrop::Pic::Before {
        let lv = world.game.local_view();
        let sel = crate::orders::Selection { mode: lv.starmap_mode, planet: lv.planet.max(0) as usize, fleet: lv.fleet.max(0) as usize };
        let mut panels = crate::starmap_panels::Panels::default();
        panels.ui_tick();
        if let Some(f) = &o.faders {
            panels.faders = f.clone();
        }
        hud.draw_starmap_names(&mut frame, &view, &assets, &world.prev, &world.curr, o.alpha, t);
        let mut models = crate::starmap_panels::PanelModels::new(r);
        let globe = models.render_globe(r, &l.install, &world.game.state, &sel, &view.camera.matrix(), [o.size.0 as f32, o.size.1 as f32], o.ui_frames as f32);
        hud.draw_starmap_hud(&mut frame, &world.game.state, &l.tables, &view, &world.prev, &world.curr, &sel, o.alpha, t, &panels.faders.starf, false, globe);
        hud.draw(&mut frame, [o.size.0 as f32, o.size.1 as f32], &world.ui, &l.tables, crate::ui::Screen::Starmap, t);
    }
    r.render_texture(&frame, o.size)
}

/// The starmap's view of a snapshot: the save's camera and the exe's clocks at the grab.
fn snapshot_view(o: &crate::Options, world: &crate::world::World<'_>) -> crate::starmap::View {
    let mut view = crate::starmap::View::new(&world.curr, &world.view, o.size, o.scale);
    view.reveal = o.reveal;
    view.set_clouds(o.clouds);
    view.futofeny = o.futofeny;
    view.acttime = o.acttime;
    view
}

/// `--snapshot`: run the requested ticks headlessly and write one frame as PNG.
/// Render a snapshot of the window `o.size`: the screens draw in its band ([`crate::kit::Layout::band`]).
pub fn snapshot(o: &crate::Options, out: &std::path::Path) -> Result<(), String> {
    if let Some(name) = o.screen.as_deref().and_then(|s| s.strip_prefix("video:")) {
        return crate::video::playback::snapshot(&crate::Options { window: o.size, ..o.clone() }, name, out);
    }
    let [_, _, w, h] = crate::kit::Layout::new(o.size.0 as i32, o.size.1 as i32).band();
    let o = crate::Options { size: (w as u32, h as u32), window: o.size, ..o.clone() };
    snapshot_band(&o, out)
}

fn snapshot_band(o: &crate::Options, out: &std::path::Path) -> Result<(), String> {
    // An input script on the starmap or a colony screen runs the window's code headless.
    if let (None | Some("starmap"), Some(path)) = (o.screen.as_deref(), &o.script) {
        return crate::window::script_run(o, out, &crate::script::Script::load(path)?);
    }
    if o.screen.as_deref() == Some("main") {
        return crate::mainscr_scene::snapshot(o, out);
    }
    if o.screen.as_deref() == Some("options") {
        return crate::options::snapshot(o, out);
    }
    if o.screen.as_deref() == Some("help") {
        return crate::help::snapshot(o, out);
    }
    let l = crate::load(o)?;
    let mut world = crate::start_world(o, &l.install, l.save.as_ref(), &l.rules, &l.tech_defaults, &l.volatile)?;
    world.set_speed(o.speed);
    world.show_colony(o.colony);
    for _ in 0..o.ticks {
        world.tick();
    }
    let mut r = Renderer::headless(wgpu::TextureFormat::Rgba8UnormSrgb)?;
    let mut hud = crate::ui::Hud::load(&mut r, &l.install, &l.tables);
    hud.set_one_language(&l.install, o.texts == Some(true));
    hud.info = crate::ui::InfoScreen::enter(world.ui.human);
    hud.info.chart = o.chart.min(9);
    hud.info.at_turn(o.info_turn);
    if let Some((p, h, c)) = o.info_list {
        (hud.info.pointer, hud.info.problem_step, hud.info.counter) = (p, h, c);
        hud.info.problem_counter = c;
    }
    for (g, v) in hud.info.globes.iter_mut().zip(&o.info_globes) {
        g.spin = v[0];
        g.planet = std::array::from_fn(|k| v[1 + k]);
        g.fleet = std::array::from_fn(|k| v[10 + k]);
        if v.len() >= 34 {
            let at = |i: usize| [v[19 + 3 * i], v[20 + 3 * i], v[21 + 3 * i]];
            (g.pos, g.home, g.start, g.target, g.step) = (at(0), at(1), at(2), at(3), at(4));
        }
    }
    // The detail mode, the star log (re/screens/info.md §6.6, §6.8).
    if let Some((e, s, leaving, p)) = o.info_detail {
        let i = &mut hud.info;
        (i.detail, i.leaving, i.sel_entry, i.sel_slot, i.designs_ptr) = (true, leaving, e, s, p);
        i.sel_pick = world.ui.info.list(i.selected, world.ui.human).get(e).copied();
    }
    if let Some(t) = o.info_texts {
        hud.info.texts = t;
    }
    if let Some([k, x, y, item]) = o.info_select.filter(|s| (26..=28).contains(&s[0])) {
        (hud.info.select, hud.info.select_at, hud.info.select_item) = (Some(k as u32), [x, y], item);
    }
    if let Some(fr) = o.info_star {
        (hud.info.star_log, hud.info.frame) = (true, fr);
    }
    if let Some(path) = &o.info_log {
        let text = std::fs::read_to_string(path).map_err(|e| format!("--info-log {path}: {e}"))?;
        let mut lines = text.lines();
        let head: Vec<usize> = lines.next().unwrap_or("").split_whitespace().filter_map(|x| x.parse().ok()).collect();
        if let [p, n] = head[..] {
            (hud.info.log_pointer, hud.info.log_filled) = (p, n);
        }
        let hex = |h: &str| (0..h.len() / 2).filter_map(|k| u8::from_str_radix(&h[2 * k..2 * k + 2], 16).ok()).collect::<Vec<u8>>();
        hud.info.log = lines.map(|l| l.split_once(' ').map_or((0, Vec::new()), |(c, t)| (c.parse().unwrap_or(0), hex(t.trim())))).collect();
    }
    hud.info.settle(&world.ui.info, world.ui.human);
    hud.speed = crate::hud::speed_index(world.game.state.mem.i32(ig2_sim::save::va::IDO_SPEED));
    hud.paused = o.paused;
    // `--backdrop`: the starmap a running game's exit left under the overlay screens (crate::backdrop): without its
    // HUD (`SaveScreen(1)`), or with it for the messages screen (`SaveScreen(2)`).
    if let Some((pic, grey)) = crate::backdrop::snapshot_pic(o).filter(|_| o.backdrop) {
        r.snapshot_backdrop = Some((starmap_picture(o, &l, &world, &mut r, &mut hud, pic), grey));
    }
    // The message line at the UI's pace (10 ms frames).
    for _ in 0..o.ui_frames {
        hud.scroller.step(&mut || world.pop_message());
    }
    if o.screen.as_deref() == Some("spacewar") {
        return crate::spacewar_screen::snapshot(o, out, &l, &mut world, &mut r, &mut hud);
    }
    if o.screen.as_deref() == Some("spy") {
        return crate::spy_screen::snapshot(o, out, &l, &world, &mut r, &mut hud);
    }
    if let (Some("research"), Some(path)) = (o.screen.as_deref(), &o.script) {
        let script = crate::script::Script::load(path)?;
        return research_sequence(o, out, &script, &l, &mut world, &mut r, &mut hud);
    }
    if let (Some("design"), Some(path)) = (o.screen.as_deref(), &o.script) {
        let script = crate::script::Script::load(path)?;
        return design_sequence(o, out, &script, &l, &mut world, &mut r, &mut hud);
    }
    if o.screen.as_deref() == Some("design") {
        return design_snapshot(o, out, &l, &world, &mut r, &mut hud);
    }
    if let (Some("trade"), Some(path)) = (o.screen.as_deref(), &o.script) {
        let script = crate::script::Script::load(path)?;
        return trade_sequence(o, out, &script, &l, &mut world, &mut r, &mut hud);
    }
    if o.screen.as_deref() == Some("trade") {
        return trade_snapshot(o, out, &l, &mut world, &mut r, &mut hud);
    }
    if let (Some("diplomacy"), Some(path)) = (o.screen.as_deref(), &o.script) {
        let script = crate::script::Script::load(path)?;
        return diplomacy_sequence(o, out, &script, &l, &mut world, &mut r, &mut hud);
    }
    if o.screen.as_deref() == Some("research") {
        return research_snapshot(o, out, &l, &world, &mut r, &mut hud);
    }
    if let (Some("messages"), Some(path)) = (o.screen.as_deref(), &o.script) {
        let script = crate::script::Script::load(path)?;
        return messages_sequence(o, out, &script, &l, &mut world, &mut r, &mut hud);
    }
    let screen = match o.screen.as_deref() {
        Some("info") => Some(crate::ui::Screen::Info),
        Some("messages") => Some(crate::ui::Screen::Messages),
        Some("diplomacy") => return diplomacy_snapshot(o, out, &l, &world, &mut r, &mut hud),
        _ => None,
    };
    if let Some(screen) = screen {
        let t = world.curr.tick as f32 * 0.05;
        let size = [o.size.0 as f32, o.size.1 as f32];
        let mut models = crate::screen_frames::FrameModels::new(&mut r);
        if screen == crate::ui::Screen::Messages {
            if let Some(id) = o.show_message {
                hud.show_message(&l.install, id);
            }
            if let Some(i) = o.open_message {
                hud.open_message(&l.install, &world.ui, i);
            }
            if o.open_message.is_some() || o.show_message.is_some() {
                // Typed out as after `--ui-frames` frames of 10 ms.
                if let Some(d) = &mut hud.messages.open {
                    d.advance(o.ui_frames as f32 * 10.0);
                }
            }
        }
        // The screen at rest: the list (`mesframe` 15), or the message open (30).
        hud.messages.hold_open();
        hud.frame_tex = match screen {
            crate::ui::Screen::Info => models.render_info(&mut r, &l.install, &hud.info.frame_view(&world.ui), size),
            _ => models.render_messages(&mut r, &l.install, hud.messages.frame, size),
        };
        // Black under the frame: the exe's `Copy3hatterkep(0x34)` picture (`SaveScreen(1)`, the screen before) is
        // black in the captures (re/screens/info.md §1.1).
        let mut frame = Frame::new([0.0, 0.0, 0.0, 1.0]);
        hud.draw(&mut frame, size, &world.ui, &l.tables, screen, t);
        // The info screen's hot-spot tooltip at `--mouse` (`Info_MouseInit`'s texts), as the window's `frame_overlay`.
        match (screen, o.mouse) {
            (crate::ui::Screen::Info, Some(m)) => {
                let tip = hud.info.spot_at(&l.tables.race_name_x, m, &world.ui).map(|id| hud.info.tip_text(id, &hud.texts, &hud.empire_names));
                hud.snapshot_tip_pointer(&mut frame, size, m, tip.as_deref(), 0, 0);
            }
            _ => hud.snapshot_pointer(&mut frame, size, o.mouse),
        }
        let rgba = r.draw_to_image(&frame, o.size, t);
        crate::assets::write_snapshot(out, o, &rgba)?;
        let s = &world.ui;
        println!(
            "{}: {screen:?} chart {} | known {:?} | inbox {} | line {:?} queue {} history {}",
            out.display(),
            hud.info.chart,
            (1..=8).filter(|&e| s.known[e]).collect::<Vec<_>>(),
            s.inbox.len(),
            String::from_utf8_lossy(&hud.scroller.text),
            s.queue.len(),
            s.history.len()
        );
        return Ok(());
    }
    if let Some(p) = o.colony {
        return colony_snapshot(o, out, &l, &world, &mut r, &mut hud, p);
    }
    let assets = crate::starmap::Assets::load(&mut r, &l.install);
    let mut view = snapshot_view(o, &world);
    let lv = world.game.local_view();
    let saved = match lv.starmap_mode {
        1 => Some(crate::starmap::Pick::Planet(lv.planet.max(0) as usize)),
        2 => Some(crate::starmap::Pick::Fleet(lv.fleet.max(0) as usize)),
        _ => None,
    };
    view.selected = o.select_fleet.map(crate::starmap::Pick::Fleet).or(saved);
    view.hover = o.hover.and_then(|(x, y)| view.pick(&world.prev, &world.curr, o.alpha, [x, y]));
    let t = world.curr.tick as f32 * 0.05 + o.alpha * 0.05;
    // The long info shrinks the planet's star to nothing (re/screens/starmap.md §15.1).
    let info = o.planet_info || o.menu.as_deref().is_some_and(|m| !m.starts_with("planets"));
    if info && lv.starmap_mode == 1 {
        // The pin to the orbiting planet at zoom 599 (a save's zoom is clamped to 580) and no selection ring.
        let p = lv.planet.max(0) as usize;
        view.star_scale = Some((p, 0.0));
        view.selected = None;
        if let Some(q) = world.curr.planets.iter().find(|q| q.index == p) {
            let clock = world.prev.orbit + (world.curr.orbit - world.prev.orbit) * o.alpha.clamp(0.0, 1.0);
            let [x, y] = crate::starmap_menus::info_pivot(crate::starmap::orbit_position(q, clock), view.camera.rotate);
            (view.camera.x, view.camera.y, view.camera.zoom) = (x, y, 599.0);
        }
    }
    // The transfer panel opens zoomed in to 590 (`StarmapBeZoomol`, re/screens/starmap.md §13.1); a save's zoom is
    // clamped to 580.
    if o.panel.as_deref().is_some_and(|p| p.starts_with("transfer") || p == "fleet") {
        view.camera.zoom = 590.0;
    }
    let mut frame = view.build(&assets, &world.prev, &world.curr, o.alpha, t);
    hud.draw_starmap_names(&mut frame, &view, &assets, &world.prev, &world.curr, o.alpha, t);
    let sel = crate::orders::Selection { mode: lv.starmap_mode, planet: lv.planet.max(0) as usize, fleet: lv.fleet.max(0) as usize };
    let mut panels = match &o.panel {
        Some(panel) => crate::starmap_panels::Panels::opened(panel)?,
        None => crate::starmap_panels::Panels::default(),
    };
    // `--ui-frames` of `FRISSIT_Starmap2` since `Starmap_Belep`: the faders' shine and drift.
    for _ in 0..o.ui_frames.max(1) {
        panels.ui_tick();
    }
    // `--faders`: the exe's own `fifo` / `starf` at the grab (their `rand()`-driven shine and drift, deviation 208).
    if let Some(f) = &o.faders {
        panels.faders = f.clone();
    }
    // The staged long info is drawn at zoom 599 at least: its box is up.
    let mut models = crate::starmap_panels::PanelModels::new(&mut r);
    let globe = models.render_globe(&mut r, &l.install, &world.game.state, &sel, &view.camera.matrix(), [o.size.0 as f32, o.size.1 as f32], o.ui_frames as f32);
    hud.draw_starmap_hud(&mut frame, &world.game.state, &l.tables, &view, &world.prev, &world.curr, &sel, o.alpha, t, &panels.faders.starf, o.planet_info, globe);
    // An info panel at rest: its selection frame morphed in (re/screens/starmap.md §19.4).
    if let Some(i) = &mut panels.info {
        i.advance(&world.game.state, &sel, t, view.camera.zoom, 0);
        i.advance(&world.game.state, &sel, t, view.camera.zoom, 200);
        i.turn = o.info_alap;
    }
    if o.panel.is_some() {
        let size = [o.size.0 as f32, o.size.1 as f32];
        let tex = models.render(&mut r, &l.install, &l.tables, &panels, &world.game.state, &sel, &view.camera.matrix(), size);
        hud.draw_starmap_panels(&mut frame, &panels, &world.game.state, &l.tables, &sel, tex, size, t);
    }
    if o.planet_info || o.rop_menu.is_some() || o.menu.is_some() {
        let c = &view.camera;
        let (menus, _) = crate::starmap_menus::staged(&world.game.state, &sel, &panels, [c.x, c.y, c.zoom, c.rotate], o.planet_info, o.rop_menu, o.menu.as_deref(), o.mouse, t)?;
        hud.draw_starmap_menus(&mut frame, &menus, &panels, &world.game.state, &l.tables, [o.size.0 as f32, o.size.1 as f32], view.camera.zoom.max(if menus.info_on() { 599.0 } else { 0.0 }), t);
    }
    if let Some(spec) = &o.top_menu {
        crate::topbar_menus::stage(&mut hud, &mut world, &l.tables, spec, o.top_mouse.unwrap_or([0.0; 2]))?;
    }
    hud.draw(&mut frame, [o.size.0 as f32, o.size.1 as f32], &world.ui, &l.tables, crate::ui::Screen::Starmap, t);
    // `--tutorial-window FILE`: the tutorial's window as the exe's probe logged it (re/rules/tutorial.md §2).
    if let Some(path) = &o.tutorial_window {
        let line = std::fs::read_to_string(path).map_err(|e| format!("{}: {e}", path.display()))?;
        let tu = crate::tutorial_ui::TutorialUi::from_record(line.lines().next().unwrap_or_default())?;
        hud.draw_tutorial(&mut frame, &tu, [o.size.0 as f32, o.size.1 as f32], 2);
    }
    // `UpdateFrame`'s last pass at `--mouse` (re/screens/input.md §8), not over a staged menu: the hot-spot's tooltip
    // (the pointer at rest), then the pointer (over the map view hot-spot 0x35 its types 2/5/6/7, `orders::view_pointer`).
    if let (Some(m), None, None, None) = (o.mouse, &o.menu, o.rop_menu, &o.top_menu) {
        let menus = crate::starmap_menus::Menus::default();
        let console = (!panels.minimap_off() && !o.planet_info)
            .then(|| l.install.read("dataraw/screen/star_m.raw").ok())
            .flatten()
            .and_then(|b| b.get(800 + m[1].clamp(0.0, 479.0) as usize * 640 + m[0].clamp(0.0, 639.0) as usize).copied())
            .and_then(crate::orders::console_spot);
        let c = crate::starmap_tip::Ctx { texts: &hud.texts, state: &world.game.state, sel, menus: &menus, panels: &panels, console, locked: false, human: world.curr.human };
        let tip = crate::starmap_tip::tip(&c, m);
        let view_spot = panels.info.is_none() && panels.transfer.is_none() && tip.as_ref().is_none_or(|t| t.id == 0x35);
        let fleet_5c = world.game.state.fleets.get(sel.fleet).map_or(0, |f| i32::from_le_bytes([f[0x5c], f[0x5d], f[0x5e], f[0x5f]]));
        let vc = crate::orders::ViewCtx { sel, production: panels.production_open(), redirect: false, alt: false, fleet_5c };
        let kind = if view_spot { crate::orders::view_pointer(&world.curr, &view.camera, m, &vc).unwrap_or(0) } else { 0 };
        hud.snapshot_tip_pointer(&mut frame, [o.size.0 as f32, o.size.1 as f32], m, tip.map(|t| t.text).as_deref(), kind, 0);
    }
    let rgba = r.draw_to_image(&frame, o.size, t);
    crate::assets::write_snapshot(out, o, &rgba)?;
    let (y, m, d) = world.curr.date();
    let shown = world.curr.fleets.iter().filter(|f| view.reveal || f.visible).count();
    println!(
        "{}: tick {} ({y}-{m:02}-{d:02}) alpha {} | {} planets, {shown}/{} fleets drawn, {} quads | hover {:?} selected {:?}",
        out.display(),
        world.curr.tick,
        o.alpha,
        world.curr.planets.len(),
        world.curr.fleets.len(),
        frame.len(),
        view.hover,
        view.selected
    );
    for f in world.curr.fleets_at(&world.prev, o.alpha).iter().filter(|f| view.reveal || f.visible) {
        let [x, y] = view.camera.to_screen(f.pos).unwrap_or([f32::NAN; 2]);
        println!("  fleet {:3} owner {:2} at ({:8.3}, {:8.3}) -> px ({x:7.1}, {y:7.1})", f.slot, f.owner, f.pos[0], f.pos[1]);
    }
    Ok(())
}
