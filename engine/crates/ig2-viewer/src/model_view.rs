//! `.xv` model viewer: flattens the scene tree on the CPU and renders it offscreen with wgpu
//! (depth-tested), then shows the result as an egui image.
//!
//! The child transform `p' = M·v + t` (M = 3×3 at child record +0x10, t = record +0x00..0x0C) is an
//! inference from data (re/formats/xv.md). The UI toggles exist to check the conventions visually.

use std::collections::HashMap;

use eframe::egui;
use eframe::egui_wgpu::{self, wgpu, wgpu::util::DeviceExt};
use glam::{Mat3, Mat4, Vec3};
use ig2_formats::pvr::Pvr;
use ig2_formats::xv::{Archive, ChildContent, Material, Node};
use ig2_formats::Install;

#[repr(C)]
#[derive(Clone, Copy, bytemuck::Pod, bytemuck::Zeroable)]
struct GpuVertex {
    pos: [f32; 3],
    normal: [f32; 3],
    uv: [f32; 2],
}

#[repr(C)]
#[derive(Clone, Copy, bytemuck::Pod, bytemuck::Zeroable)]
struct Uniforms {
    view_proj: [[f32; 4]; 4],
    light: [f32; 4],
    flags: [f32; 4],
}

const SHADER: &str = r#"
struct U { vp: mat4x4<f32>, light: vec4<f32>, flags: vec4<f32> };
@group(0) @binding(0) var<uniform> u: U;
@group(1) @binding(0) var tex: texture_2d<f32>;
@group(1) @binding(1) var smp: sampler;

struct VO { @builtin(position) pos: vec4<f32>, @location(0) n: vec3<f32>, @location(1) uv: vec2<f32> };

@vertex
fn vs(@location(0) p: vec3<f32>, @location(1) n: vec3<f32>, @location(2) uv: vec2<f32>) -> VO {
    var o: VO;
    o.pos = u.vp * vec4<f32>(p, 1.0);
    o.n = n;
    o.uv = vec2<f32>(uv.x, select(uv.y, 1.0 - uv.y, u.flags.x > 0.5));
    return o;
}

@fragment
fn fs(i: VO) -> @location(0) vec4<f32> {
    let c = textureSample(tex, smp, i.uv);
    if (c.a < 0.3) { discard; }
    var l = 1.0;
    if (u.flags.y < 0.5) {
        let nn = normalize(i.n + vec3<f32>(1e-6, 0.0, 0.0));
        l = abs(dot(nn, normalize(u.light.xyz))) * 0.75 + 0.25;
    }
    return vec4<f32>(c.rgb * l, 1.0);
}
"#;

const COLOR_FORMAT: wgpu::TextureFormat = wgpu::TextureFormat::Rgba8UnormSrgb;
const DEPTH_FORMAT: wgpu::TextureFormat = wgpu::TextureFormat::Depth32Float;

type EguiRenderer = std::sync::Arc<egui::mutex::RwLock<egui_wgpu::Renderer>>;

pub struct Gpu {
    device: wgpu::Device,
    queue: wgpu::Queue,
    /// None in headless snapshot mode.
    renderer: Option<EguiRenderer>,
    pipeline: wgpu::RenderPipeline,
    uniform_buf: wgpu::Buffer,
    uniform_bg: wgpu::BindGroup,
    tex_layout: wgpu::BindGroupLayout,
    sampler: wgpu::Sampler,
    white: wgpu::BindGroup,
    target: Option<Target>,
    textures: HashMap<String, Option<wgpu::BindGroup>>,
}

struct Target {
    size: [u32; 2],
    color: wgpu::Texture,
    color_view: wgpu::TextureView,
    depth_view: wgpu::TextureView,
    egui_id: egui::TextureId,
}

struct Batch {
    texture: Option<String>,
    vbuf: wgpu::Buffer,
    ibuf: wgpu::Buffer,
    count: u32,
}

struct Loaded {
    path: String,
    batches: Vec<Batch>,
    center: Vec3,
    radius: f32,
    stats: Stats,
    tree: Vec<String>,
}

#[derive(Default)]
struct Stats {
    meshes: usize,
    vertices: usize,
    triangles: usize,
    missing_children: usize,
    textures_ok: usize,
    textures_missing: Vec<String>,
}

#[derive(Clone, Copy, PartialEq)]
struct Conventions {
    transpose: bool,
    flip_v: bool,
}

pub struct ModelView {
    gpu: Option<Gpu>,
    loaded: Option<Result<Loaded, String>>,
    conv: Conventions,
    built_with: Option<Conventions>,
    unlit: bool,
    yaw: f32,
    pitch: f32,
    zoom: f32,
}

fn bind_texture(gpu_dev: &wgpu::Device, q: &wgpu::Queue, layout: &wgpu::BindGroupLayout, smp: &wgpu::Sampler, w: u32, h: u32, rgba: &[u8]) -> wgpu::BindGroup {
    let size = wgpu::Extent3d { width: w, height: h, depth_or_array_layers: 1 };
    let tex = gpu_dev.create_texture(&wgpu::TextureDescriptor {
        label: Some("model texture"),
        size,
        mip_level_count: 1,
        sample_count: 1,
        dimension: wgpu::TextureDimension::D2,
        format: wgpu::TextureFormat::Rgba8UnormSrgb,
        usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
        view_formats: &[],
    });
    q.write_texture(
        wgpu::TexelCopyTextureInfo { texture: &tex, mip_level: 0, origin: wgpu::Origin3d::ZERO, aspect: wgpu::TextureAspect::All },
        rgba,
        wgpu::TexelCopyBufferLayout { offset: 0, bytes_per_row: Some(4 * w), rows_per_image: Some(h) },
        size,
    );
    let view = tex.create_view(&Default::default());
    gpu_dev.create_bind_group(&wgpu::BindGroupDescriptor {
        label: Some("model texture bg"),
        layout,
        entries: &[
            wgpu::BindGroupEntry { binding: 0, resource: wgpu::BindingResource::TextureView(&view) },
            wgpu::BindGroupEntry { binding: 1, resource: wgpu::BindingResource::Sampler(smp) },
        ],
    })
}

impl Gpu {
    fn from_egui(rs: egui_wgpu::RenderState) -> Self {
        Self::new(rs.device.clone(), rs.queue.clone(), Some(rs.renderer.clone()))
    }

    /// A GPU context without a window, for `--snapshot`.
    pub fn headless() -> Result<Self, String> {
        let instance = wgpu::Instance::new(&wgpu::InstanceDescriptor::default());
        let adapter = pollster::block_on(instance.request_adapter(&wgpu::RequestAdapterOptions::default()))
            .map_err(|e| format!("no GPU adapter: {e}"))?;
        let (device, queue) =
            pollster::block_on(adapter.request_device(&wgpu::DeviceDescriptor::default())).map_err(|e| e.to_string())?;
        Ok(Self::new(device, queue, None))
    }

    fn new(device: wgpu::Device, queue: wgpu::Queue, renderer: Option<EguiRenderer>) -> Self {
        let d = &device;
        let shader = d.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("xv shader"),
            source: wgpu::ShaderSource::Wgsl(SHADER.into()),
        });
        let ubl = d.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("uniforms"),
            entries: &[wgpu::BindGroupLayoutEntry {
                binding: 0,
                visibility: wgpu::ShaderStages::VERTEX_FRAGMENT,
                ty: wgpu::BindingType::Buffer { ty: wgpu::BufferBindingType::Uniform, has_dynamic_offset: false, min_binding_size: None },
                count: None,
            }],
        });
        let tex_layout = d.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("texture"),
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
            label: Some("xv layout"),
            bind_group_layouts: &[&ubl, &tex_layout],
            push_constant_ranges: &[],
        });
        let attrs = wgpu::vertex_attr_array![0 => Float32x3, 1 => Float32x3, 2 => Float32x2];
        let pipeline = d.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("xv pipeline"),
            layout: Some(&layout),
            vertex: wgpu::VertexState {
                module: &shader,
                entry_point: Some("vs"),
                compilation_options: Default::default(),
                buffers: &[wgpu::VertexBufferLayout {
                    array_stride: std::mem::size_of::<GpuVertex>() as u64,
                    step_mode: wgpu::VertexStepMode::Vertex,
                    attributes: &attrs,
                }],
            },
            primitive: wgpu::PrimitiveState { cull_mode: None, ..Default::default() },
            depth_stencil: Some(wgpu::DepthStencilState {
                format: DEPTH_FORMAT,
                depth_write_enabled: true,
                depth_compare: wgpu::CompareFunction::Less,
                stencil: Default::default(),
                bias: Default::default(),
            }),
            multisample: Default::default(),
            fragment: Some(wgpu::FragmentState {
                module: &shader,
                entry_point: Some("fs"),
                compilation_options: Default::default(),
                targets: &[Some(wgpu::ColorTargetState { format: COLOR_FORMAT, blend: None, write_mask: wgpu::ColorWrites::ALL })],
            }),
            multiview: None,
            cache: None,
        });
        let uniform_buf = d.create_buffer(&wgpu::BufferDescriptor {
            label: Some("uniforms"),
            size: std::mem::size_of::<Uniforms>() as u64,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        let uniform_bg = d.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("uniforms bg"),
            layout: &ubl,
            entries: &[wgpu::BindGroupEntry { binding: 0, resource: uniform_buf.as_entire_binding() }],
        });
        let sampler = d.create_sampler(&wgpu::SamplerDescriptor {
            address_mode_u: wgpu::AddressMode::Repeat,
            address_mode_v: wgpu::AddressMode::Repeat,
            mag_filter: wgpu::FilterMode::Linear,
            min_filter: wgpu::FilterMode::Linear,
            ..Default::default()
        });
        let white = bind_texture(d, &queue, &tex_layout, &sampler, 1, 1, &[200, 200, 200, 255]);
        Self { device, queue, renderer, pipeline, uniform_buf, uniform_bg, tex_layout, sampler, white, target: None, textures: HashMap::new() }
    }

    fn ensure_target(&mut self, size: [u32; 2]) -> egui::TextureId {
        if let Some(t) = &self.target {
            if t.size == size {
                return t.egui_id;
            }
        }
        let d = &self.device;
        let ext = wgpu::Extent3d { width: size[0], height: size[1], depth_or_array_layers: 1 };
        let mk = |format, usage| {
            d.create_texture(&wgpu::TextureDescriptor {
                label: Some("xv target"),
                size: ext,
                mip_level_count: 1,
                sample_count: 1,
                dimension: wgpu::TextureDimension::D2,
                format,
                usage,
                view_formats: &[],
            })
        };
        let color = mk(
            COLOR_FORMAT,
            wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_SRC,
        );
        let depth = mk(DEPTH_FORMAT, wgpu::TextureUsages::RENDER_ATTACHMENT);
        let color_view = color.create_view(&Default::default());
        let depth_view = depth.create_view(&Default::default());
        let egui_id = match &self.renderer {
            None => egui::TextureId::default(),
            Some(r) => {
                let mut renderer = r.write();
                match &self.target {
                    Some(old) => {
                        renderer.update_egui_texture_from_wgpu_texture(d, &color_view, wgpu::FilterMode::Linear, old.egui_id);
                        old.egui_id
                    }
                    None => renderer.register_native_texture(d, &color_view, wgpu::FilterMode::Linear),
                }
            }
        };
        self.target = Some(Target { size, color, color_view, depth_view, egui_id });
        egui_id
    }

    fn texture(&mut self, install: &Install, path: &str) -> bool {
        if !self.textures.contains_key(path) {
            let bg = install.read(path).ok().and_then(|b| {
                let img = Pvr::parse(&b).ok()?.decode_rgba8().ok()?;
                Some(bind_texture(&self.device, &self.queue, &self.tex_layout, &self.sampler, img.width, img.height, &img.pixels))
            });
            self.textures.insert(path.to_owned(), bg);
        }
        self.textures[path].is_some()
    }

    fn render(&self, loaded: &Loaded, u: &Uniforms) {
        let Some(t) = &self.target else { return };
        self.queue.write_buffer(&self.uniform_buf, 0, bytemuck::bytes_of(u));
        let mut enc = self.device.create_command_encoder(&wgpu::CommandEncoderDescriptor { label: Some("xv") });
        {
            let mut pass = enc.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("xv pass"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: &t.color_view,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Clear(wgpu::Color { r: 0.02, g: 0.025, b: 0.05, a: 1.0 }),
                        store: wgpu::StoreOp::Store,
                    },
                })],
                depth_stencil_attachment: Some(wgpu::RenderPassDepthStencilAttachment {
                    view: &t.depth_view,
                    depth_ops: Some(wgpu::Operations { load: wgpu::LoadOp::Clear(1.0), store: wgpu::StoreOp::Discard }),
                    stencil_ops: None,
                }),
                timestamp_writes: None,
                occlusion_query_set: None,
            });
            pass.set_pipeline(&self.pipeline);
            pass.set_bind_group(0, &self.uniform_bg, &[]);
            for b in &loaded.batches {
                let bg = b.texture.as_ref().and_then(|p| self.textures.get(p)).and_then(Option::as_ref).unwrap_or(&self.white);
                pass.set_bind_group(1, bg, &[]);
                pass.set_vertex_buffer(0, b.vbuf.slice(..));
                pass.set_index_buffer(b.ibuf.slice(..), wgpu::IndexFormat::Uint32);
                pass.draw_indexed(0..b.count, 0, 0..1);
            }
        }
        self.queue.submit([enc.finish()]);
    }

    /// Copy the render target back to the CPU as tightly packed RGBA8.
    fn readback(&self) -> Option<(u32, u32, Vec<u8>)> {
        let t = self.target.as_ref()?;
        let [w, h] = t.size;
        let row = (4 * w).div_ceil(256) * 256;
        let buf = self.device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("readback"),
            size: (row * h) as u64,
            usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ,
            mapped_at_creation: false,
        });
        let mut enc = self.device.create_command_encoder(&Default::default());
        enc.copy_texture_to_buffer(
            wgpu::TexelCopyTextureInfo { texture: &t.color, mip_level: 0, origin: wgpu::Origin3d::ZERO, aspect: wgpu::TextureAspect::All },
            wgpu::TexelCopyBufferInfo {
                buffer: &buf,
                layout: wgpu::TexelCopyBufferLayout { offset: 0, bytes_per_row: Some(row), rows_per_image: Some(h) },
            },
            wgpu::Extent3d { width: w, height: h, depth_or_array_layers: 1 },
        );
        self.queue.submit([enc.finish()]);
        let slice = buf.slice(..);
        slice.map_async(wgpu::MapMode::Read, |_| {});
        self.device.poll(wgpu::PollType::Wait).ok()?;
        let data = slice.get_mapped_range();
        let mut out = Vec::with_capacity((4 * w * h) as usize);
        for y in 0..h as usize {
            out.extend_from_slice(&data[y * row as usize..y * row as usize + 4 * w as usize]);
        }
        Some((w, h, out))
    }
}

/// Right-handed view matrix.
fn look_at_rh(eye: Vec3, target: Vec3, up: Vec3) -> Mat4 {
    let f = (target - eye).normalize();
    let s = f.cross(up).normalize();
    let u = s.cross(f);
    Mat4::from_cols(
        glam::Vec4::new(s.x, u.x, -f.x, 0.0),
        glam::Vec4::new(s.y, u.y, -f.y, 0.0),
        glam::Vec4::new(s.z, u.z, -f.z, 0.0),
        glam::Vec4::new(-s.dot(eye), -u.dot(eye), f.dot(eye), 1.0),
    )
}

/// Right-handed perspective projection with depth in 0..1 (wgpu convention).
fn perspective_rh(fov_y: f32, aspect: f32, near: f32, far: f32) -> Mat4 {
    let h = 1.0 / (fov_y * 0.5).tan();
    let r = far / (near - far);
    Mat4::from_cols(
        glam::Vec4::new(h / aspect, 0.0, 0.0, 0.0),
        glam::Vec4::new(0.0, h, 0.0, 0.0),
        glam::Vec4::new(0.0, 0.0, r, -1.0),
        glam::Vec4::new(0.0, 0.0, r * near, 0.0),
    )
}

/// CPU-side flattening of the scene tree into per-texture triangle lists.
struct Flatten<'a, 'b> {
    archive: &'a Archive<'b>,
    install: &'a Install,
    screen: String,
    prefix: String,
    conv: Conventions,
    batches: HashMap<Option<String>, (Vec<GpuVertex>, Vec<u32>)>,
    stats: Stats,
    tree: Vec<String>,
}

impl Flatten<'_, '_> {
    fn material_texture(&mut self, name: &str) -> Option<String> {
        let m = self.archive.get(&format!("{}{name}.xvm", self.prefix))?;
        let mat = Material::parse(m.data).ok()?;
        if mat.texture.is_empty() {
            return None;
        }
        let r = self.install.resolve_model_texture(&self.screen, &mat.texture);
        if r.is_none() && !self.stats.textures_missing.contains(&mat.texture) {
            self.stats.textures_missing.push(mat.texture.clone());
        }
        r
    }

    fn node(&mut self, n: &Node, xf: Mat4, depth: usize) {
        for (c, content) in &n.children {
            let m = Mat3::from_cols_array(&c.matrix);
            let m = if self.conv.transpose { m.transpose() } else { m };
            let t: [f32; 3] = std::array::from_fn(|i| f32::from_le_bytes(c.head[4 * i..4 * i + 4].try_into().unwrap()));
            let local = Mat4::from_translation(Vec3::from(t).map(|v| if v.is_finite() { v } else { 0.0 })) * Mat4::from_mat3(m);
            let world = xf * local;
            match content {
                ChildContent::Node(sub) => {
                    self.tree.push(format!("{}▸ {} (node, {} children)", "  ".repeat(depth), c.name, sub.children.len()));
                    self.node(sub, world, depth + 1);
                }
                ChildContent::Mesh(mesh) => {
                    self.tree.push(format!(
                        "{}• {} ({} v, {} tri, mats {:?})",
                        "  ".repeat(depth),
                        c.name,
                        mesh.vertices.len(),
                        mesh.faces.len(),
                        mesh.materials
                    ));
                    self.stats.meshes += 1;
                    self.stats.vertices += mesh.vertices.len();
                    self.stats.triangles += mesh.faces.len();
                    let normal_m = Mat3::from_mat4(world).inverse().transpose();
                    let texs: Vec<Option<String>> = mesh.materials.clone().iter().map(|m| self.material_texture(m)).collect();
                    for f in &mesh.faces {
                        let key = texs.get(f.material as usize).cloned().flatten();
                        let (vs, is) = self.batches.entry(key).or_default();
                        for &i in &f.idx {
                            let v = mesh.vertices[i as usize];
                            is.push(vs.len() as u32);
                            vs.push(GpuVertex {
                                pos: world.transform_point3(Vec3::from(v.pos)).into(),
                                normal: (normal_m * Vec3::from(v.normal)).into(),
                                uv: v.uv,
                            });
                        }
                    }
                }
                ChildContent::Missing => {
                    self.tree.push(format!("{}✕ {} (missing)", "  ".repeat(depth), c.name));
                    self.stats.missing_children += 1;
                }
            }
        }
    }
}

impl ModelView {
    pub fn new(cc: &eframe::CreationContext<'_>) -> Self {
        Self {
            gpu: cc.wgpu_render_state.clone().map(Gpu::from_egui),
            loaded: None,
            conv: Conventions { transpose: false, flip_v: false },
            built_with: None,
            unlit: false,
            yaw: 0.6,
            pitch: 0.4,
            zoom: 1.0,
        }
    }

    fn load(&mut self, install: &Install, path: &str) -> Result<Loaded, String> {
        let gpu = self.gpu.as_mut().ok_or("wgpu backend unavailable")?;
        Self::load_model(gpu, install, path, self.conv)
    }

    /// Read, parse and flatten an archive and upload it to the GPU.
    fn load_model(gpu: &mut Gpu, install: &Install, path: &str, conv: Conventions) -> Result<Loaded, String> {
        let bytes = install.read(path).map_err(|e| e.to_string())?;
        let archive = Archive::parse(&bytes).map_err(|e| e.to_string())?;
        let stem = path.rsplit('/').next().unwrap_or(path).trim_end_matches(".xv");
        let scene = archive.scene(Some(stem)).map_err(|e| e.to_string())?.ok_or("archive has no root node (meshes only)")?;
        let root = archive.root(Some(stem)).map(|m| m.name.clone()).unwrap_or_default();
        let screen = path.strip_prefix("dataraw/vector/").and_then(|p| p.split('/').next()).unwrap_or("").to_owned();
        let mut fl = Flatten {
            archive: &archive,
            install,
            screen,
            prefix: format!("{}-", root.split('.').next().unwrap_or_default()),
            conv,
            batches: HashMap::new(),
            stats: Stats::default(),
            tree: vec![format!("{root}  origin {:?}  frames {}", scene.header.origin, scene.header.frames())],
        };
        fl.node(&scene, Mat4::IDENTITY, 0);
        let (mut lo, mut hi) = (Vec3::splat(f32::MAX), Vec3::splat(f32::MIN));
        for (vs, _) in fl.batches.values() {
            for v in vs {
                let p = Vec3::from(v.pos);
                if p.is_finite() {
                    lo = lo.min(p);
                    hi = hi.max(p);
                }
            }
        }
        let (center, radius) = if lo.x <= hi.x { ((lo + hi) * 0.5, ((hi - lo).length() * 0.5).max(1e-3)) } else { (Vec3::ZERO, 1.0) };
        let mut batches = Vec::new();
        let mut stats = std::mem::take(&mut fl.stats);
        let tree = std::mem::take(&mut fl.tree);
        for (tex, (vs, is)) in fl.batches {
            if let Some(t) = &tex {
                if gpu.texture(install, t) {
                    stats.textures_ok += 1;
                }
            }
            let d = &gpu.device;
            batches.push(Batch {
                texture: tex,
                vbuf: d.create_buffer_init(&wgpu::util::BufferInitDescriptor {
                    label: Some("xv vb"),
                    contents: bytemuck::cast_slice(&vs),
                    usage: wgpu::BufferUsages::VERTEX,
                }),
                ibuf: d.create_buffer_init(&wgpu::util::BufferInitDescriptor {
                    label: Some("xv ib"),
                    contents: bytemuck::cast_slice(&is),
                    usage: wgpu::BufferUsages::INDEX,
                }),
                count: is.len() as u32,
            });
        }
        Ok(Loaded { path: path.to_owned(), batches, center, radius, stats, tree })
    }

    pub fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame, install: &Install, path: &str) {
        let stale = match &self.loaded {
            Some(Ok(l)) => l.path != path,
            Some(Err(_)) => true,
            None => true,
        } || self.built_with != Some(self.conv);
        if stale {
            self.loaded = Some(self.load(install, path));
            self.built_with = Some(self.conv);
        }
        ui.horizontal(|ui| {
            ui.checkbox(&mut self.conv.transpose, "transpose child matrix");
            ui.checkbox(&mut self.conv.flip_v, "flip V");
            ui.checkbox(&mut self.unlit, "unlit");
            if ui.button("reset view").clicked() {
                (self.yaw, self.pitch, self.zoom) = (0.6, 0.4, 1.0);
            }
        });
        let loaded = match &self.loaded {
            Some(Ok(l)) => l,
            Some(Err(e)) => {
                ui.colored_label(egui::Color32::LIGHT_RED, e);
                return;
            }
            None => return,
        };
        let s = &loaded.stats;
        ui.label(format!(
            "{} meshes, {} vertices, {} triangles, {} batches; textures {} ok, missing {:?}; missing children {}",
            s.meshes,
            s.vertices,
            s.triangles,
            loaded.batches.len(),
            s.textures_ok,
            s.textures_missing,
            s.missing_children
        ));
        egui::SidePanel::right("xv_tree").default_width(300.0).show_inside(ui, |ui| {
            egui::ScrollArea::vertical().show(ui, |ui| {
                for l in &loaded.tree {
                    ui.monospace(l);
                }
            });
        });
        let avail = ui.available_size();
        let ppp = ui.ctx().pixels_per_point();
        let px = [((avail.x * ppp) as u32).max(16), ((avail.y * ppp) as u32).max(16)];
        let Some(gpu) = self.gpu.as_mut() else { return };
        let id = gpu.ensure_target(px);
        let resp = ui.add(egui::Image::new((id, avail)).sense(egui::Sense::click_and_drag()));
        if resp.dragged() {
            let d = resp.drag_delta();
            self.yaw -= d.x * 0.01;
            self.pitch = (self.pitch + d.y * 0.01).clamp(-1.5, 1.5);
        }
        if resp.hovered() {
            let scroll = ui.input(|i| i.smooth_scroll_delta.y);
            self.zoom = (self.zoom * (1.0 - scroll * 0.002)).clamp(0.05, 20.0);
        }
        let u = camera(loaded, self.yaw, self.pitch, self.zoom, px[0] as f32 / px[1] as f32, self.conv.flip_v, self.unlit);
        gpu.render(loaded, &u);
    }
}

fn camera(loaded: &Loaded, yaw: f32, pitch: f32, zoom: f32, aspect: f32, flip_v: bool, unlit: bool) -> Uniforms {
    let dist = loaded.radius * 2.6 * zoom;
    let eye = loaded.center + dist * Vec3::new(pitch.cos() * yaw.sin(), pitch.sin(), pitch.cos() * yaw.cos());
    let view = look_at_rh(eye, loaded.center, Vec3::Y);
    let proj = perspective_rh(0.8, aspect, dist * 0.01, dist * 10.0);
    Uniforms {
        view_proj: (proj * view).to_cols_array_2d(),
        light: [0.4, 0.8, 0.45, 0.0],
        flags: [flip_v as u32 as f32, unlit as u32 as f32, 0.0, 0.0],
    }
}

/// Headless: render a model from four angles into a 2×2 PNG contact sheet.
/// Returns a one-line summary (counts and missing textures).
pub fn snapshot(install: &Install, path: &str, out: &std::path::Path, transpose: bool, flip_v: bool) -> Result<String, String> {
    const TILE: u32 = 384;
    let mut gpu = Gpu::headless()?;
    let loaded = ModelView::load_model(&mut gpu, install, path, Conventions { transpose, flip_v })?;
    gpu.ensure_target([TILE, TILE]);
    let side = 2 * TILE as usize;
    let mut sheet = vec![0u8; side * side * 4];
    let views = [(0.6, 0.35), (2.4, 0.35), (0.0, 1.45), (std::f32::consts::PI, 0.0)];
    for (k, (yaw, pitch)) in views.into_iter().enumerate() {
        gpu.render(&loaded, &camera(&loaded, yaw, pitch, 1.0, 1.0, flip_v, false));
        let (w, h, px) = gpu.readback().ok_or("readback failed")?;
        let (ox, oy) = ((k % 2) * TILE as usize, (k / 2) * TILE as usize);
        for y in 0..h as usize {
            let dst = ((oy + y) * side + ox) * 4;
            sheet[dst..dst + 4 * w as usize].copy_from_slice(&px[y * 4 * w as usize..(y + 1) * 4 * w as usize]);
        }
    }
    crate::write_png(out, side as u32, side as u32, &sheet)?;
    let s = &loaded.stats;
    Ok(format!(
        "{path}: {} meshes, {} tris, {} batches, textures ok {}, missing {:?}, missing children {}",
        s.meshes,
        s.triangles,
        loaded.batches.len(),
        s.textures_ok,
        s.textures_missing,
        s.missing_children
    ))
}
