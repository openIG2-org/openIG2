//! The depth-tested mesh pass (ADR 0006): world-space triangles of the colony screen's models (terrain,
//! buildings, units), lit by the scene's ambient and parallel lights and fogged towards the background, drawn
//! before the 2D quads of the same frame.

use wgpu::util::DeviceExt;

use crate::render::TexId;

const SHADER: &str = r#"
struct U {
    view: mat4x4<f32>,
    eye: vec4<f32>,
    light: vec4<f32>,
    ambient: vec4<f32>,
    sun: vec4<f32>,
    fog: vec4<f32>,
    fog_range: vec4<f32>,
};
@group(0) @binding(0) var<uniform> u: U;
@group(1) @binding(0) var tex: texture_2d<f32>;
@group(1) @binding(1) var smp: sampler;

struct VO {
    @builtin(position) pos: vec4<f32>,
    @location(0) n: vec3<f32>,
    @location(1) uv: vec2<f32>,
    @location(2) color: vec4<f32>,
    @location(3) dist: f32,
    @location(4) border: f32,
};

@vertex
fn vs(@location(0) p: vec3<f32>, @location(1) n: vec3<f32>, @location(2) uv: vec2<f32>, @location(3) c: vec4<f32>) -> VO {
    var o: VO;
    o.pos = u.view * vec4(p, 1.0);
    o.n = n;
    o.uv = uv;
    o.color = c;
    o.dist = distance(p, u.eye.xyz);
    // `SetBorderFog`: terrain vertices past 95 % of the map half-size get fog 0.5, past 99 % full fog.
    let a = max(abs(p.y), abs(p.z)) / u.fog_range.w;
    o.border = select(select(0.0, 0.5, a > 0.95), 1.0, a > 0.99);
    return o;
}

fn shade(i: VO) -> vec4<f32> {
    let t = textureSample(tex, smp, i.uv);
    let n = normalize(i.n);
    let lit = u.ambient.rgb + u.sun.rgb * abs(dot(n, u.light.xyz)) * u.light.w + u.sun.rgb * max(dot(n, u.light.xyz), 0.0) * (1.0 - u.light.w);
    var rgb = t.rgb * i.color.rgb * min(lit, vec3(1.3));
    let f = max(clamp((i.dist - u.fog_range.x) / max(u.fog_range.y - u.fog_range.x, 1.0), 0.0, 1.0) * u.fog_range.z, i.border) * i.color.a;
    rgb = mix(rgb, u.fog.rgb, f);
    return vec4(rgb, t.a);
}

@fragment
fn fs_opaque(i: VO) -> @location(0) vec4<f32> {
    let c = shade(i);
    if (c.a < 0.5) { discard; }
    return vec4(c.rgb, 1.0);
}

@fragment
fn fs_glass(i: VO) -> @location(0) vec4<f32> {
    let c = shade(i);
    return vec4(c.rgb, c.a * i.color.a);
}
"#;

/// A world-space vertex. `color.rgb` tints the texture; `color.a` is the opacity of glass parts (and scales
/// the fog for opaque ones, so the background dome can skip it).
#[repr(C)]
#[derive(Debug, Clone, Copy, bytemuck::Pod, bytemuck::Zeroable)]
pub struct MeshVertex {
    pub pos: [f32; 3],
    pub normal: [f32; 3],
    pub uv: [f32; 2],
    pub color: [f32; 4],
}

/// The per-frame scene constants.
#[repr(C)]
#[derive(Debug, Clone, Copy, bytemuck::Pod, bytemuck::Zeroable)]
pub struct SceneUniforms {
    /// World → clip.
    pub view: [[f32; 4]; 4],
    pub eye: [f32; 4],
    /// Direction towards the parallel light; `w` = 1 lights both sides (`abs`), 0 one side.
    pub light: [f32; 4],
    pub ambient: [f32; 4],
    pub sun: [f32; 4],
    pub fog: [f32; 4],
    /// Fog start, end, strength, the map's half-size (border fog).
    pub fog_range: [f32; 4],
}

/// A mesh buffer the renderer keeps across frames (terrain, buildings).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct MeshId(usize);

/// Which vertices of what to draw how.
#[derive(Debug, Clone)]
pub struct MeshDraw {
    /// `None` = the frame's own dynamic vertices.
    pub mesh: Option<MeshId>,
    pub range: std::ops::Range<u32>,
    pub tex: TexId,
    pub glass: bool,
}

/// The 3D part of a frame.
#[derive(Debug, Clone)]
pub struct Scene3d {
    pub uniforms: SceneUniforms,
    pub draws: Vec<MeshDraw>,
    pub dynamic: Vec<MeshVertex>,
}

pub(crate) struct Pipe3d {
    opaque: wgpu::RenderPipeline,
    glass: wgpu::RenderPipeline,
    uniform: wgpu::Buffer,
    uniform_bg: wgpu::BindGroup,
    meshes: Vec<wgpu::Buffer>,
    dynamic: Option<(wgpu::Buffer, u64)>,
    depth: Option<(wgpu::TextureView, (u32, u32))>,
}

pub(crate) const DEPTH: wgpu::TextureFormat = wgpu::TextureFormat::Depth32Float;

impl Pipe3d {
    pub(crate) fn new(d: &wgpu::Device, format: wgpu::TextureFormat, tex_layout: &wgpu::BindGroupLayout) -> Self {
        let shader = d.create_shader_module(wgpu::ShaderModuleDescriptor { label: Some("ig2-client meshes"), source: wgpu::ShaderSource::Wgsl(SHADER.into()) });
        let ubl = d.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: None,
            entries: &[wgpu::BindGroupLayoutEntry {
                binding: 0,
                visibility: wgpu::ShaderStages::VERTEX_FRAGMENT,
                ty: wgpu::BindingType::Buffer { ty: wgpu::BufferBindingType::Uniform, has_dynamic_offset: false, min_binding_size: None },
                count: None,
            }],
        });
        let layout = d.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor { label: None, bind_group_layouts: &[&ubl, tex_layout], push_constant_ranges: &[] });
        let attrs = wgpu::vertex_attr_array![0 => Float32x3, 1 => Float32x3, 2 => Float32x2, 3 => Float32x4];
        let pipeline = |fs: &str, blend: Option<wgpu::BlendState>, write: bool| {
            d.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
                label: Some("ig2-client meshes"),
                layout: Some(&layout),
                vertex: wgpu::VertexState {
                    module: &shader,
                    entry_point: Some("vs"),
                    compilation_options: Default::default(),
                    buffers: &[wgpu::VertexBufferLayout {
                        array_stride: std::mem::size_of::<MeshVertex>() as u64,
                        step_mode: wgpu::VertexStepMode::Vertex,
                        attributes: &attrs,
                    }],
                },
                primitive: wgpu::PrimitiveState { cull_mode: None, ..Default::default() },
                depth_stencil: Some(wgpu::DepthStencilState {
                    format: DEPTH,
                    depth_write_enabled: write,
                    depth_compare: wgpu::CompareFunction::LessEqual,
                    stencil: Default::default(),
                    bias: Default::default(),
                }),
                multisample: Default::default(),
                fragment: Some(wgpu::FragmentState {
                    module: &shader,
                    entry_point: Some(fs),
                    compilation_options: Default::default(),
                    targets: &[Some(wgpu::ColorTargetState { format, blend, write_mask: wgpu::ColorWrites::ALL })],
                }),
                multiview: None,
                cache: None,
            })
        };
        let opaque = pipeline("fs_opaque", None, true);
        let glass = pipeline("fs_glass", Some(wgpu::BlendState::ALPHA_BLENDING), false);
        let uniform = d.create_buffer(&wgpu::BufferDescriptor {
            label: None,
            size: std::mem::size_of::<SceneUniforms>() as u64,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        let uniform_bg = d.create_bind_group(&wgpu::BindGroupDescriptor {
            label: None,
            layout: &ubl,
            entries: &[wgpu::BindGroupEntry { binding: 0, resource: uniform.as_entire_binding() }],
        });
        Self { opaque, glass, uniform, uniform_bg, meshes: Vec::new(), dynamic: None, depth: None }
    }

    pub(crate) fn add_mesh(&mut self, d: &wgpu::Device, verts: &[MeshVertex]) -> MeshId {
        let contents: &[u8] = if verts.is_empty() { &[0u8; 48] } else { bytemuck::cast_slice(verts) };
        self.meshes.push(d.create_buffer_init(&wgpu::util::BufferInitDescriptor { label: Some("mesh"), contents, usage: wgpu::BufferUsages::VERTEX }));
        MeshId(self.meshes.len() - 1)
    }

    /// Forget every mesh (a new colony screen).
    pub(crate) fn clear_meshes(&mut self) {
        self.meshes.clear();
    }

    /// Record the 3D pass into `enc`: clears colour and depth, draws opaque parts then glass parts in order.
    #[allow(clippy::too_many_arguments)]
    pub(crate) fn encode(
        &mut self,
        d: &wgpu::Device,
        q: &wgpu::Queue,
        enc: &mut wgpu::CommandEncoder,
        target: &wgpu::TextureView,
        size: (u32, u32),
        clear: [f64; 4],
        scene: &Scene3d,
        textures: &[wgpu::BindGroup],
    ) {
        if self.depth.as_ref().is_none_or(|(_, s)| *s != size) {
            let t = d.create_texture(&wgpu::TextureDescriptor {
                label: Some("depth"),
                size: wgpu::Extent3d { width: size.0, height: size.1, depth_or_array_layers: 1 },
                mip_level_count: 1,
                sample_count: 1,
                dimension: wgpu::TextureDimension::D2,
                format: DEPTH,
                usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
                view_formats: &[],
            });
            self.depth = Some((t.create_view(&Default::default()), size));
        }
        q.write_buffer(&self.uniform, 0, bytemuck::bytes_of(&scene.uniforms));
        let bytes: &[u8] = bytemuck::cast_slice(&scene.dynamic);
        let need = (bytes.len() as u64).max(48);
        if self.dynamic.as_ref().is_none_or(|(_, cap)| *cap < need) {
            let cap = need.next_power_of_two();
            let buf = d.create_buffer(&wgpu::BufferDescriptor {
                label: Some("dynamic mesh"),
                size: cap,
                usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
                mapped_at_creation: false,
            });
            self.dynamic = Some((buf, cap));
        }
        let (dynbuf, _) = self.dynamic.as_ref().unwrap();
        if !bytes.is_empty() {
            q.write_buffer(dynbuf, 0, bytes);
        }
        let [r, g, b, a] = clear;
        let mut pass = enc.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: Some("meshes"),
            color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                view: target,
                resolve_target: None,
                ops: wgpu::Operations { load: wgpu::LoadOp::Clear(wgpu::Color { r, g, b, a }), store: wgpu::StoreOp::Store },
            })],
            depth_stencil_attachment: Some(wgpu::RenderPassDepthStencilAttachment {
                view: &self.depth.as_ref().unwrap().0,
                depth_ops: Some(wgpu::Operations { load: wgpu::LoadOp::Clear(1.0), store: wgpu::StoreOp::Discard }),
                stencil_ops: None,
            }),
            timestamp_writes: None,
            occlusion_query_set: None,
        });
        pass.set_bind_group(0, &self.uniform_bg, &[]);
        for glass in [false, true] {
            pass.set_pipeline(if glass { &self.glass } else { &self.opaque });
            for m in scene.draws.iter().filter(|m| m.glass == glass && !m.range.is_empty()) {
                let buf = match m.mesh {
                    Some(MeshId(i)) => &self.meshes[i],
                    None => dynbuf,
                };
                pass.set_vertex_buffer(0, buf.slice(..));
                pass.set_bind_group(1, &textures[m.tex.0], &[]);
                pass.draw(m.range.clone(), 0..1);
            }
        }
    }
}
