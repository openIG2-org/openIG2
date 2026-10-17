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
    let p = i.center + vec2(d.x * c - d.y * s, d.x * s + d.y * c);
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
        // Solid quad with soft long edges (lines).
        case 4u: {
            let a = cover(abs(q.y) - 1.0 + 0.5 * fwidth(q.y));
            return vec4(v.color.rgb, v.color.a * a);
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
    /// A line from `a` to `b`, `width` pixels wide.
    pub fn line(a: [f32; 2], b: [f32; 2], width: f32, color: [f32; 4]) -> Self {
        let (dx, dy) = (b[0] - a[0], b[1] - a[1]);
        let len = (dx * dx + dy * dy).sqrt();
        Self {
            rot: dy.atan2(dx),
            ..Self::new(kind::LINE, [(a[0] + b[0]) / 2.0, (a[1] + b[1]) / 2.0], [len / 2.0, width / 2.0], color)
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Blend {
    Alpha,
    Additive,
}

/// A texture frames are drawn into ([`Renderer::target`]).
pub struct Target {
    pub tex: TexId,
    view: wgpu::TextureView,
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
    uniform: wgpu::Buffer,
    uniform_bg: wgpu::BindGroup,
    tex_layout: wgpu::BindGroupLayout,
    sampler: wgpu::Sampler,
    textures: Vec<wgpu::BindGroup>,
    /// The same textures with a repeating sampler (meshes).
    textures_rep: Vec<wgpu::BindGroup>,
    sampler_rep: wgpu::Sampler,
    instances: Option<(wgpu::Buffer, u64)>,
    pipe3d: crate::render3d::Pipe3d,
    /// The colour format the pipelines draw into.
    format: wgpu::TextureFormat,
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
        let pipeline = |blend: wgpu::BlendState| {
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
        let alpha = pipeline(wgpu::BlendState::ALPHA_BLENDING);
        let add = wgpu::BlendComponent { src_factor: wgpu::BlendFactor::SrcAlpha, dst_factor: wgpu::BlendFactor::One, operation: wgpu::BlendOperation::Add };
        let additive = pipeline(wgpu::BlendState { color: add, alpha: add });
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
        let pipe3d = crate::render3d::Pipe3d::new(d, format, &tex_layout);
        Self {
            device,
            queue,
            alpha,
            additive,
            uniform,
            uniform_bg,
            tex_layout,
            sampler,
            textures: Vec::new(),
            textures_rep: Vec::new(),
            sampler_rep,
            instances: None,
            pipe3d,
            format,
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
                format: wgpu::TextureFormat::Rgba8UnormSrgb,
                usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
                view_formats: &[],
            },
            wgpu::util::TextureDataOrder::MipMajor,
            &mips.concat(),
        );
        let view = tex.create_view(&Default::default());
        for (list, sampler) in [(&mut self.textures, &self.sampler), (&mut self.textures_rep, &self.sampler_rep)] {
            list.push(self.device.create_bind_group(&wgpu::BindGroupDescriptor {
                label: None,
                layout: &self.tex_layout,
                entries: &[
                    wgpu::BindGroupEntry { binding: 0, resource: wgpu::BindingResource::TextureView(&view) },
                    wgpu::BindGroupEntry { binding: 1, resource: wgpu::BindingResource::Sampler(sampler) },
                ],
            }));
        }
        TexId(self.textures.len() - 1)
    }

    /// Upload world-space triangles the renderer keeps until [`Renderer::clear_meshes`].
    /// Upload `img` in place of texture `id` (a video frame; the old texture is freed).
    pub fn replace_texture(&mut self, id: TexId, img: &Image) {
        let new = self.add_texture(img);
        for list in [&mut self.textures, &mut self.textures_rep] {
            let g = list.swap_remove(new.0);
            if let Some(slot) = list.get_mut(id.0) {
                *slot = g;
            }
        }
    }

    pub fn add_mesh(&mut self, verts: &[crate::render3d::MeshVertex]) -> crate::render3d::MeshId {
        self.pipe3d.add_mesh(&self.device, verts)
    }

    pub fn clear_meshes(&mut self) {
        self.pipe3d.clear_meshes();
    }

    /// Draw `frame` into `target` (`size` in physical pixels).
    pub fn draw(&mut self, frame: &Frame, target: &wgpu::TextureView, size: (u32, u32), time: f32) {
        let u = Uniforms { screen: [size.0 as f32, size.1 as f32], time, _pad: 0.0 };
        self.queue.write_buffer(&self.uniform, 0, bytemuck::bytes_of(&u));
        let bytes: &[u8] = bytemuck::cast_slice(&frame.instances);
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
        let mut enc = self.device.create_command_encoder(&Default::default());
        if let Some(scene) = &frame.scene {
            self.pipe3d.encode(&self.device, &self.queue, &mut enc, target, size, frame.clear, scene, &self.textures_rep);
        }
        {
            let [r, g, b, a] = frame.clear;
            let load = if frame.scene.is_some() { wgpu::LoadOp::Load } else { wgpu::LoadOp::Clear(wgpu::Color { r, g, b, a }) };
            let mut pass = enc.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: None,
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: target,
                    resolve_target: None,
                    ops: wgpu::Operations { load, store: wgpu::StoreOp::Store },
                })],
                depth_stencil_attachment: None,
                timestamp_writes: None,
                occlusion_query_set: None,
            });
            pass.set_bind_group(0, &self.uniform_bg, &[]);
            pass.set_vertex_buffer(0, buf.slice(..));
            for b in &frame.batches {
                pass.set_pipeline(match b.blend {
                    Blend::Alpha => &self.alpha,
                    Blend::Additive => &self.additive,
                });
                pass.set_bind_group(1, &self.textures[b.tex.0], &[]);
                pass.draw(0..6, b.range.clone());
            }
        }
        self.queue.submit([enc.finish()]);
    }

    /// Draw `frame` into a new texture the quads can sample (the colony radar's picture of the terrain).
    pub fn render_texture(&mut self, frame: &Frame, size: (u32, u32)) -> TexId {
        let t = self.target(size);
        self.draw(frame, &t.view, size, 0.0);
        t.tex
    }

    /// A texture the quads can sample that frames can be drawn into again and again ([`Renderer::draw_into`]).
    pub fn target(&mut self, size: (u32, u32)) -> Target {
        let (view, groups) = self.target_view(size);
        self.textures.push(groups[0].clone());
        self.textures_rep.push(groups[1].clone());
        Target { tex: TexId(self.textures.len() - 1), view, size }
    }

    /// Draw `frame` into `t`, resized to `size` first (same [`TexId`]).
    pub fn draw_into(&mut self, frame: &Frame, t: &mut Target, size: (u32, u32), time: f32) {
        if t.size != size {
            let (view, groups) = self.target_view(size);
            self.textures[t.tex.0] = groups[0].clone();
            self.textures_rep[t.tex.0] = groups[1].clone();
            (t.view, t.size) = (view, size);
        }
        let view = t.view.clone();
        self.draw(frame, &view, size, time);
    }

    fn target_view(&mut self, size: (u32, u32)) -> (wgpu::TextureView, [wgpu::BindGroup; 2]) {
        let size = (size.0.max(1), size.1.max(1));
        let tex = self.device.create_texture(&wgpu::TextureDescriptor {
            label: Some("rendered"),
            size: wgpu::Extent3d { width: size.0, height: size.1, depth_or_array_layers: 1 },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: self.format,
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::TEXTURE_BINDING,
            view_formats: &[],
        });
        let view = tex.create_view(&Default::default());
        let group = |sampler: &wgpu::Sampler| {
            self.device.create_bind_group(&wgpu::BindGroupDescriptor {
                label: None,
                layout: &self.tex_layout,
                entries: &[
                    wgpu::BindGroupEntry { binding: 0, resource: wgpu::BindingResource::TextureView(&view) },
                    wgpu::BindGroupEntry { binding: 1, resource: wgpu::BindingResource::Sampler(sampler) },
                ],
            })
        };
        let groups = [group(&self.sampler), group(&self.sampler_rep)];
        (view, groups)
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
            view_formats: &[],
        });
        self.draw(frame, &tex.create_view(&Default::default()), size, time);
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

/// `--snapshot --screen diplomacy`: the diplomacy screen at rest (re/screens/diplomacy.md).
fn diplomacy_snapshot(o: &crate::Options, out: &std::path::Path, l: &crate::Loaded, world: &crate::world::World, r: &mut Renderer, hud: &mut crate::ui::Hud) -> Result<(), String> {
    let state = &world.game.state;
    let mut d = crate::diplomacy::Open::new(r, &l.install, &l.tables, &hud.texts, state);
    let size = [o.size.0 as f32, o.size.1 as f32];
    let t = world.curr.tick as f32 * 0.05;
    d.screen.settle();
    let _ = d.input(state, hud, &l.tables, &crate::diplomacy::Input::default());
    d.screen.settle();
    let mut frame = Frame::new([0.0, 0.0, 0.0, 1.0]);
    d.draw(&mut frame, r, &l.install, &l.tables, hud, state, size, t);
    hud.draw(&mut frame, size, &world.ui, &l.tables, crate::ui::Screen::Diplomacy, t);
    let rgba = r.draw_to_image(&frame, o.size, t);
    crate::assets::write_png(out, o.size.0, o.size.1, &rgba)?;
    let v = crate::diplomacy::View::read(state);
    println!("{}: diplomacy | selected {} | contacts {:?} | menus {:?}", out.display(), d.screen.selected, v.slots.iter().map(|s| (s.empire, s.waiting, s.answered)).collect::<Vec<_>>(), d.screen.shown);
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
        let f = |n: &str| l.save.chunk(n).and_then(|b| b.get(..4)).map_or(0.0, |b| f32::from_le_bytes(b.try_into().unwrap()));
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
    for _ in 0..100 {
        ui.ui_frame(&curr, &mut view.camera);
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
                    ui.hover(&c, crate::colony_ui::cell_under(&view, gm, px));
                }
            }
            "demolish" => {
                ui.press(&c, [220.0, 450.0], true, 0);
                cursor = [250.0, 440.0];
            }
            m => return Err(format!("--colony-ui {m}: pick, demolish or build")),
        }
    }
    view.overlay = ui.overlay();
    let size = [o.size.0 as f32, o.size.1 as f32];
    let mut panel_model = crate::colony_ui::PanelBuilding::default();
    let panel = panel_model.render(r, &l.install, &l.tables, ui.kind, ui.spin(), size);
    let mut frame = view.build(r, &l.install, &l.tables, &l.rules.ground_maps, &mut scene, &prev, &curr, o.alpha, t);
    let input = crate::ui::ColonyInput { ui: &ui, sim: &l.rules, panel, cursor, now_ms, time: t };
    hud.draw_colony(&mut frame, &world.game.state, &l.tables, &l.rules.ground_maps, &view, &scene, &curr, &input);
    hud.draw(&mut frame, [o.size.0 as f32, o.size.1 as f32], &world.ui, &l.tables, crate::ui::Screen::Colony, t);
    let rgba = r.draw_to_image(&frame, o.size, t);
    crate::assets::write_png(out, o.size.0, o.size.1, &rgba)?;
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

/// `--snapshot --screen research`: the research screen on the save's selection after `--ui-frames` 10 ms frames.
fn research_snapshot(o: &crate::Options, out: &std::path::Path, l: &crate::Loaded, world: &crate::world::World, r: &mut Renderer, hud: &mut crate::ui::Hud) -> Result<(), String> {
    let state = &world.game.state;
    let mut s = crate::research::Research::open(state, &l.tables);
    for _ in 0..o.ui_frames.max(1) {
        s.step(state);
    }
    let infos = crate::research::tech_infos(&l.install);
    let mut models = crate::research::ResearchModels::new(r, &l.install);
    let size = [o.size.0 as f32, o.size.1 as f32];
    let (tex, lines) = models.render(r, &l.install, &l.tables, &s, state, size);
    let t = world.curr.tick as f32 * 0.05;
    let mut frame = Frame::new([0.0, 0.0, 0.0, 1.0]);
    hud.draw_research(&mut frame, &s, &infos, state, &l.tables, tex, lines, size, t);
    hud.draw(&mut frame, size, &world.ui, &l.tables, crate::ui::Screen::Research, t);
    let rgba = r.draw_to_image(&frame, o.size, t);
    crate::assets::write_png(out, o.size.0, o.size.1, &rgba)?;
    println!("{}: research area {} field {} tech {} | models at {:?}", out.display(), s.area, s.field, s.tech, models.centres);
    Ok(())
}

/// `--snapshot`: run the requested ticks headlessly and write one frame as PNG.
pub fn snapshot(o: &crate::Options, out: &std::path::Path) -> Result<(), String> {
    let l = crate::load(o)?;
    let mut world = crate::world::World::load(&l.save, &l.rules, &l.tech_defaults, &l.volatile);
    world.set_speed(o.speed);
    world.show_colony(o.colony);
    for _ in 0..o.ticks {
        world.tick();
    }
    let mut r = Renderer::headless(wgpu::TextureFormat::Rgba8UnormSrgb)?;
    let mut hud = crate::ui::Hud::load(&mut r, &l.install, &l.tables);
    hud.info = crate::ui::InfoScreen::enter(world.ui.human);
    hud.info.chart = o.chart.min(9);
    hud.speed = crate::hud::speed_index(world.game.state.mem.i32(ig2_sim::save::va::IDO_SPEED));
    hud.paused = o.paused;
    // The message line at the UI's pace (10 ms frames).
    for _ in 0..o.ui_frames {
        hud.scroller.step(&mut || world.pop_message());
    }
    if o.screen.as_deref() == Some("spy") {
        return crate::spy_screen::snapshot(o, out, &l, &world, &mut r, &mut hud);
    }
    if o.screen.as_deref() == Some("research") {
        return research_snapshot(o, out, &l, &world, &mut r, &mut hud);
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
        hud.frame_tex = match screen {
            crate::ui::Screen::Info => models.render_info(&mut r, &l.install, &hud.info.frame_view(&world.ui), size),
            _ => models.render_messages(&mut r, &l.install, if hud.messages.open.is_some() { 30 } else { 15 }, size),
        };
        // Black under the frame: the exe's `Copy3hatterkep(0x34)` picture (`SaveScreen(1)`, the screen before) is
        // black in the captures (re/screens/info.md §1.1).
        let mut frame = Frame::new([0.0, 0.0, 0.0, 1.0]);
        hud.draw(&mut frame, size, &world.ui, &l.tables, screen, t);
        let rgba = r.draw_to_image(&frame, o.size, t);
        crate::assets::write_png(out, o.size.0, o.size.1, &rgba)?;
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
    let mut view = crate::starmap::View::new(&world.curr, &world.view, o.size, o.scale);
    view.reveal = o.reveal;
    let lv = world.game.local_view();
    let saved = match lv.starmap_mode {
        1 => Some(crate::starmap::Pick::Planet(lv.planet.max(0) as usize)),
        2 => Some(crate::starmap::Pick::Fleet(lv.fleet.max(0) as usize)),
        _ => None,
    };
    view.selected = o.select_fleet.map(crate::starmap::Pick::Fleet).or(saved);
    view.hover = o.hover.and_then(|(x, y)| view.pick(&world.prev, &world.curr, o.alpha, [x, y]));
    let t = world.curr.tick as f32 * 0.05 + o.alpha * 0.05;
    let mut frame = view.build(&assets, &world.prev, &world.curr, o.alpha, t);
    hud.draw_starmap_names(&mut frame, &view, &assets, &world.prev, &world.curr, o.alpha, t);
    let sel = crate::orders::Selection { mode: lv.starmap_mode, planet: lv.planet.max(0) as usize, fleet: lv.fleet.max(0) as usize };
    hud.draw_starmap_hud(&mut frame, &world.game.state, &l.tables, &view, &world.prev, &world.curr, &sel, o.alpha, t);
    if let Some(panel) = &o.panel {
        let p = crate::starmap_panels::Panels::opened(panel)?;
        let mut models = crate::starmap_panels::PanelModels::new(&mut r);
        let size = [o.size.0 as f32, o.size.1 as f32];
        let tex = models.render(&mut r, &l.install, &l.tables, &p, &world.game.state, &sel, size);
        hud.draw_starmap_panels(&mut frame, &p, &world.game.state, &sel, tex, size, t);
    }
    hud.draw(&mut frame, [o.size.0 as f32, o.size.1 as f32], &world.ui, &l.tables, crate::ui::Screen::Starmap, t);
    let rgba = r.draw_to_image(&frame, o.size, t);
    crate::assets::write_png(out, o.size.0, o.size.1, &rgba)?;
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
