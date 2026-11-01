//! The depth-tested mesh pass (ADR 0006): world-space triangles of the 3D screens' models, lit by the scene's
//! ambient and parallel lights (or per vertex on the CPU) and fogged towards the background, drawn before the 2D quads
//! of the same frame. With [`Scene3d::gamma`] it is the GOG remaster's own render layer (re/screens/render.md): its
//! `ig2_fragment.shader` on the stored gamma-space values, the shadow map (`BIT2`) and the `VIDEO` material.

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
    flash: vec4<f32>,
    flash_dir: vec4<f32>,
};
@group(0) @binding(0) var<uniform> u: U;
@group(1) @binding(0) var tex: texture_2d<f32>;
@group(1) @binding(1) var smp: sampler;

// The remaster's shadow map (re/screens/render.md §4): light-space rows (map = rows·p/32 + 128 on a 256-unit map).
struct S {
    a: vec4<f32>,
    b: vec4<f32>,
    // The frame's height in pixels (`sg3d_window.y`), `sg3d_time.y` (`(acttime % 1000)/1000`), 1 with "Soft
    // shadows" (ADR 0055).
    c: vec4<f32>,
};
@group(2) @binding(0) var<uniform> sh: S;
@group(2) @binding(1) var shadow_tex: texture_2d<f32>;
@group(2) @binding(2) var shadow_smp: sampler;

struct VO {
    @builtin(position) pos: vec4<f32>,
    @location(0) n: vec3<f32>,
    @location(1) uv: vec2<f32>,
    @location(2) color: vec4<f32>,
    @location(3) dist: f32,
    @location(4) border: f32,
    @location(5) suv: vec2<f32>,
};

// `Kockarecshadow` (ELF 0x5fcae0): a receiver vertex's shadow texcoords `(rows·p·shadowzoom + shadowkp)/256`; the
// map was drawn with GL's bottom-up rows, so v is flipped for wgpu's top-down texture.
fn shadow_uv(p: vec3<f32>) -> vec2<f32> {
    let m = vec2(dot(sh.a.xyz, p), dot(sh.b.xyz, p)) / 32.0 + vec2(128.0);
    return vec2(m.x / 256.0, 1.0 - m.y / 256.0);
}

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
    o.suv = shadow_uv(p);
    return o;
}

// The light-space pass (`rendershadowmap` ELF 0x604f50 → `Kockashadow` 0x5fc430): casters' triangles at
// `rows·p/32 + 128` on the 256-unit map, colour 0x80808080 without texture, blend or depth, on white.
@vertex
fn vs_shadow(@location(0) p: vec3<f32>) -> @builtin(position) vec4<f32> {
    let m = vec2(dot(sh.a.xyz, p), dot(sh.b.xyz, p)) / 32.0 + vec2(128.0);
    return vec4(m / 128.0 - vec2(1.0), 0.5, 1.0);
}

@fragment
fn fs_shadow() -> @location(0) vec4<f32> {
    return vec4(0.5, 0.5, 0.5, 0.5);
}

// `ig2_fragment.shader`'s `BIT2` on the desktop: four taps a quarter and three quarters of a 2048-texel map apart.
fn shadow_at(uv: vec2<f32>) -> f32 {
    let k = 1.0 / 2048.0;
    if (sh.c.z > 0.5) {
        return shadow_pcf(uv);
    }
    var s = textureSample(shadow_tex, shadow_smp, uv + vec2(-0.25, 0.75) * k).y;
    s += textureSample(shadow_tex, shadow_smp, uv + vec2(0.75, 0.25) * k).y;
    s += textureSample(shadow_tex, shadow_smp, uv + vec2(-0.75, -0.25) * k).y;
    s += textureSample(shadow_tex, shadow_smp, uv + vec2(0.25, -0.75) * k).y;
    return s * 0.25;
}

// "Soft shadows" (ADR 0055): a 4 × 4 grid of bilinear taps two texels apart, about six texels of penumbra.
fn shadow_pcf(uv: vec2<f32>) -> f32 {
    let k = 2.0 / 2048.0;
    var s = 0.0;
    for (var y = 0; y < 4; y++) {
        for (var x = 0; x < 4; x++) {
            s += textureSampleLevel(shadow_tex, shadow_smp, uv + (vec2(f32(x), f32(y)) - vec2(1.5)) * k, 0.0).y;
        }
    }
    return s / 16.0;
}

fn shade(i: VO) -> vec4<f32> {
    return shade_cap(i, 1.3, false, 1.0);
}

// `fenyekszamolasa`'s parallel light (re/screens/research.md §2.1): `shade[float2int(|n − L|²·2047)]` of the table
// `SetLights(2, 0.15)`, `sqrt(1 − i/2896)·255`, times the colour `>> 8`; for unit vectors `|n − L|² = 2 − 2·n·L`.
fn engine_shade(c: f32) -> f32 {
    let i = round((2.0 - 2.0 * c) * 2047.0);
    return select(0.0, floor(sqrt(1.0 - i / 2896.0) * 255.0) / 256.0, i < 2896.0);
}

fn shade_cap(i: VO, cap: f32, engine: bool, shadow: f32) -> vec4<f32> {
    let t = textureSample(tex, smp, i.uv);
    // A zero normal: lit on the CPU (`color.rgb` holds the light).
    let prelit = dot(i.n, i.n) < 1e-8;
    let n = normalize(select(i.n, vec3(1.0, 0.0, 0.0), prelit));
    let d = dot(n, u.light.xyz);
    let lambert = u.ambient.rgb + u.sun.rgb * abs(d) * u.light.w + u.sun.rgb * max(d, 0.0) * (1.0 - u.light.w);
    // The engine path's second parallel light (the colony's lightning, `villamid`; black elsewhere).
    let flash = u.flash.rgb * engine_shade(dot(n, u.flash_dir.xyz));
    // A normal longer than 1.5: the plain Lambert term in the engine path (the battle's ships lit per pixel, ADR 0055).
    let s = select(engine_shade(d), max(d, 0.0), dot(i.n, i.n) > 2.25);
    let lit = select(select(lambert, u.ambient.rgb + u.sun.rgb * s + flash, engine), vec3(1.0), prelit);
    var rgb = t.rgb * shadow * i.color.rgb * min(lit, vec3(cap));
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

// The remaster's `ig2_fragment.shader` on the raw (gamma-space) views: `texture × vertex colour` (the engine's light,
// clamped to 1 like its colour bytes), mixed towards the fog colour; `BIT0` (an alpha texture) discards below 0.5.
@fragment
fn fs_opaque_g(i: VO) -> @location(0) vec4<f32> {
    let c = shade_cap(i, 1.0, true, 1.0);
    if (c.a < 0.5) { discard; }
    return vec4(c.rgb, 1.0);
}

// A blob shadow: the texel times the vertex colour, which the blend takes away from what lies below.
@fragment
fn fs_blob_g(i: VO) -> @location(0) vec4<f32> {
    return vec4(textureSample(tex, smp, i.uv).rgb * i.color.rgb, 1.0);
}

@fragment
fn fs_glass_g(i: VO) -> @location(0) vec4<f32> {
    let c = shade_cap(i, 1.0, true, 1.0);
    return vec4(c.rgb, c.a * i.color.a);
}

// The same for shadow receivers (`BIT2`: the texel's colour times the map before the vertex colour and the fog).
@fragment
fn fs_opaque_gr(i: VO) -> @location(0) vec4<f32> {
    let c = shade_cap(i, 1.0, true, shadow_at(i.suv));
    if (c.a < 0.5) { discard; }
    return vec4(c.rgb, 1.0);
}

@fragment
fn fs_glass_gr(i: VO) -> @location(0) vec4<f32> {
    let c = shade_cap(i, 1.0, true, shadow_at(i.suv));
    return vec4(c.rgb, c.a * i.color.a);
}

// The remaster's `VIDEO` material (`ig2_fragment_video.shader`, textures named `xx_bmf`: the head videos played onto
// a disc by `PlayAnimVector`): four taps half a 256-texel step apart diagonally, scan lines `|fract(y·256/H + 4t) −
// 0.5|·1.5 + 0.5` over GL's bottom-up window rows, times the vertex colour; no fog.
@fragment
fn fs_video_g(i: VO) -> @location(0) vec4<f32> {
    let k = 0.5 / 256.0;
    var c = textureSample(tex, smp, i.uv + vec2(-k, -k));
    c += textureSample(tex, smp, i.uv + vec2(k, -k));
    c += textureSample(tex, smp, i.uv + vec2(k, k));
    c += textureSample(tex, smp, i.uv + vec2(-k, k));
    c *= 0.25;
    let y = sh.c.x - i.pos.y;
    let rgb = c.rgb * (abs(fract(y * 256.0 / sh.c.x + sh.c.y * 4.0) - 0.5) * 1.5 + 0.5);
    return vec4(rgb * i.color.rgb, 1.0);
}

// GLSL's smoothstep, also for edge0 > edge1.
fn ss(e0: f32, e1: f32, x: vec2<f32>) -> vec2<f32> {
    let t = clamp((x - vec2(e0)) / (e1 - e0), vec2(0.0), vec2(1.0));
    return t * t * (3.0 - 2.0 * t);
}

// The GOG remaster's CLOUD material (render/ig2_fragment_cloud.shader, CLOUD_REPEAT 0.5, CLOUD_COVERAGE 0.4) on its
// noise texture, `sg3d_time.x` in `eye.w`; drawn in gamma space (`color.rgb` = the vertex light).
@fragment
fn fs_cloud(i: VO) -> @location(0) vec4<f32> {
    var uv = i.uv;
    let em0 = ss(0.0, 0.2, uv);
    let em1 = ss(1.0, 0.8, uv);
    uv.y = uv.y * 0.5;
    uv = uv * 0.5 + vec2(u.eye.w * 0.01);
    var f = 0.0;
    var ampli = 1.0;
    for (var k = 0; k < 4; k++) {
        f += (textureSample(tex, smp, uv).x - 0.4) * ampli;
        ampli *= 0.5;
        uv *= 3.3;
    }
    f = f * em0.x * em0.y * em1.x * em1.y;
    let cloud = ss(0.0, 0.4, vec2(f)).x;
    return vec4(vec3(min(cloud * 2.0, 1.0)) * i.color.rgb, cloud);
}
"#;

/// A world-space vertex. `color.rgb` tints the texture; `color.a` is the opacity of glass parts (and scales
/// the fog for opaque ones, so the background dome can skip it). A zero normal marks a vertex lit on the CPU:
/// `color.rgb` is then its light and the scene's lights are skipped. A normal four units long takes the engine path's
/// parallel light as the plain Lambert term `max(n·L, 0)` instead of the engine's table (ADR 0055: the space battle's
/// ships lit per pixel as `spacewar_screen` lights their vertices).
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
    /// A second parallel light of the engine path (`gamma`): its colour, then the direction towards it (the colony's
    /// lightning flash, `UpdateRain`); zero lights nothing.
    pub flash: [[f32; 4]; 2],
}

/// A part's role in the remaster's shadow map (re/screens/render.md §4): property 0x400000 casts (drawn into the
/// map), 0x800000 receives (`Kocka` draws it with `shadowkirak` and the `BIT2` variation).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Shadow {
    #[default]
    None,
    Cast,
    Receive,
    /// A blob shadow under a unit (`WF_Tank::DrawShadow`'s `polygons3d(v, 3, 2)`: blend ZERO / ONE_MINUS_SRC_COLOR,
    /// so what lies below is multiplied by `1 − texel·colour`; depth tested, not written; no fog). Gamma pass, glass.
    Blob,
    /// Drawn into the shadow map only: `Kockashadow` (ELF 0x5fc430) draws a building under construction whole while
    /// the scene cuts it (re/screens/colony.md §3.4).
    CastOnly,
}

/// The light-space rows of `rendershadowmap`'s camera (`CameraRotate(0, light)`: `a`, `−b` of its basis).
#[derive(Debug, Clone, Copy)]
pub struct ShadowMap {
    pub rows: [[f32; 3]; 2],
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
    /// The remaster's `CLOUD` material (`tex` = its noise): drawn last, in gamma space, over what is below without
    /// the depth test (the starmap's planet clouds).
    pub cloud: bool,
    /// Its role in the shadow map (only with [`Scene3d::shadow`]).
    pub shadow: Shadow,
    /// The remaster's `VIDEO` material (a head video's disc; gamma pass only, opaque).
    pub video: bool,
    /// The engine's additive objects (property 8 off: `ONE`/`ONE`, re/screens/render.md §2.3): the colour times the
    /// opacity added to what lies below, the coverage kept (gamma pass, with the glass). Without `glass`: the same
    /// blend writing depth under a strict depth test, with the opaque meshes (the engine's `Lines`, drawn nearest
    /// first: where lines crowd under one pixel only the first counts, so they don't pile up).
    pub additive: bool,
    /// Glass that keeps writing depth: the engine's transparency only turns blending on (`renderstateallitas` ELF
    /// 0x5fa410) and leaves the object's depth write, so its meshes hide one another as when opaque (the design
    /// screen's hull at `SetObjTransparency(0.01)`, re/screens/design.md §12c).
    pub depth_write: bool,
}

/// The 3D part of a frame.
#[derive(Debug, Clone)]
pub struct Scene3d {
    pub uniforms: SceneUniforms,
    /// Drawn like the remaster (re/screens/render.md §2): on the frame's raw view with the textures' raw views, so
    /// texels, vertex colours and blends work on the stored gamma-space values. Else in linear light (sRGB views).
    pub gamma: bool,
    /// The remaster's shadow map (`IG2_USE_SHADOW`, the colony with shadows on), drawn from the casters first.
    pub shadow: Option<ShadowMap>,
    pub draws: Vec<MeshDraw>,
    pub dynamic: Vec<MeshVertex>,
}

/// The modern lighting options (ADR 0055), each off by default: off, the mesh pass is the remaster's.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct Lighting {
    /// "Anti-aliasing": the meshes drawn 4× multisampled.
    pub msaa: bool,
    /// "Per-pixel lighting": the space battle's ships and pieces lit per fragment.
    pub per_pixel: bool,
    /// "Soft shadows": the shadow map read percentage-closer filtered.
    pub soft_shadows: bool,
    /// "Bloom": the additive translucent parts' glow.
    pub bloom: bool,
}

static LIGHTING: std::sync::atomic::AtomicU8 = std::sync::atomic::AtomicU8::new(0);

impl Lighting {
    fn bits(self) -> u8 {
        self.msaa as u8 | (self.per_pixel as u8) << 1 | (self.soft_shadows as u8) << 2 | (self.bloom as u8) << 3
    }

    fn from_bits(b: u8) -> Self {
        Lighting { msaa: b & 1 != 0, per_pixel: b & 2 != 0, soft_shadows: b & 4 != 0, bloom: b & 8 != 0 }
    }
}

pub fn set_lighting(l: Lighting) {
    LIGHTING.store(l.bits(), std::sync::atomic::Ordering::Relaxed);
}

pub fn lighting() -> Lighting {
    Lighting::from_bits(LIGHTING.load(std::sync::atomic::Ordering::Relaxed))
}

/// The full-screen passes of the modern lighting (ADR 0055): an anti-aliased pass laid over the frame, the bloom's
/// blur and its sum.
const POST: &str = r#"
@group(0) @binding(0) var src: texture_2d<f32>;
@group(0) @binding(1) var smp: sampler;

@vertex
fn vs(@builtin(vertex_index) i: u32) -> @builtin(position) vec4<f32> {
    let p = vec2(f32((i << 1u) & 2u), f32(i & 2u));
    return vec4(p * 2.0 - vec2(1.0), 0.0, 1.0);
}

@fragment
fn fs_copy(@builtin(position) p: vec4<f32>) -> @location(0) vec4<f32> {
    return textureLoad(src, vec2<i32>(p.xy), 0);
}

fn blur(p: vec2<f32>, dir: vec2<f32>) -> vec4<f32> {
    var w = array<f32, 5>(0.227027, 0.1945946, 0.1216216, 0.054054, 0.016216);
    let px = 1.0 / vec2<f32>(textureDimensions(src));
    let uv = p * px;
    var c = textureSampleLevel(src, smp, uv, 0.0) * w[0];
    for (var k = 1; k < 5; k++) {
        let o = dir * px * f32(k);
        c += (textureSampleLevel(src, smp, uv + o, 0.0) + textureSampleLevel(src, smp, uv - o, 0.0)) * w[k];
    }
    return c;
}

@fragment
fn fs_blur_h(@builtin(position) p: vec4<f32>) -> @location(0) vec4<f32> {
    return blur(p.xy, vec2(1.0, 0.0));
}

@fragment
fn fs_blur_v(@builtin(position) p: vec4<f32>) -> @location(0) vec4<f32> {
    return blur(p.xy, vec2(0.0, 1.0));
}

// The half-size glow at half strength, added to the frame.
@fragment
fn fs_bloom(@builtin(position) p: vec4<f32>) -> @location(0) vec4<f32> {
    let uv = p.xy * 0.5 / vec2<f32>(textureDimensions(src));
    return vec4(textureSampleLevel(src, smp, uv, 0.0).rgb * 0.5, 0.0);
}
"#;

/// The glow's format (gamma-space bytes, like the pass it is taken from).
const GLOW: wgpu::TextureFormat = wgpu::TextureFormat::Rgba8Unorm;

/// The mesh pipelines at one sample count.
struct Pipes {
    opaque: wgpu::RenderPipeline,
    glass: wgpu::RenderPipeline,
    opaque_g: wgpu::RenderPipeline,
    glass_g: wgpu::RenderPipeline,
    opaque_gr: wgpu::RenderPipeline,
    glass_gr: wgpu::RenderPipeline,
    glass_w: wgpu::RenderPipeline,
    glass_gw: wgpu::RenderPipeline,
    video_g: wgpu::RenderPipeline,
    add_g: wgpu::RenderPipeline,
    add_gw: wgpu::RenderPipeline,
    blob_g: wgpu::RenderPipeline,
}

fn attrs() -> [wgpu::VertexAttribute; 4] {
    wgpu::vertex_attr_array![0 => Float32x3, 1 => Float32x3, 2 => Float32x2, 3 => Float32x4]
}

/// A mesh pipeline: `depth` = (write, compare), none for the glow.
#[allow(clippy::too_many_arguments)]
fn mesh_pipeline(
    d: &wgpu::Device,
    shader: &wgpu::ShaderModule,
    layout: &wgpu::PipelineLayout,
    fs: &str,
    blend: Option<wgpu::BlendState>,
    depth: Option<(bool, wgpu::CompareFunction)>,
    format: wgpu::TextureFormat,
    samples: u32,
) -> wgpu::RenderPipeline {
    let attrs = attrs();
    d.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
        label: Some("ig2-client meshes"),
        layout: Some(layout),
        vertex: wgpu::VertexState {
            module: shader,
            entry_point: Some("vs"),
            compilation_options: Default::default(),
            buffers: &[wgpu::VertexBufferLayout { array_stride: std::mem::size_of::<MeshVertex>() as u64, step_mode: wgpu::VertexStepMode::Vertex, attributes: &attrs }],
        },
        primitive: wgpu::PrimitiveState { cull_mode: None, ..Default::default() },
        depth_stencil: depth.map(|(write, depth_compare)| wgpu::DepthStencilState { format: DEPTH, depth_write_enabled: write, depth_compare, stencil: Default::default(), bias: Default::default() }),
        multisample: wgpu::MultisampleState { count: samples, ..Default::default() },
        fragment: Some(wgpu::FragmentState {
            module: shader,
            entry_point: Some(fs),
            compilation_options: Default::default(),
            targets: &[Some(wgpu::ColorTargetState { format, blend, write_mask: wgpu::ColorWrites::ALL })],
        }),
        multiview: None,
        cache: None,
    })
}

/// The engine's additive blend: the colour times the opacity added, the coverage kept.
const ADD: wgpu::BlendState = wgpu::BlendState {
    color: wgpu::BlendComponent { src_factor: wgpu::BlendFactor::SrcAlpha, dst_factor: wgpu::BlendFactor::One, operation: wgpu::BlendOperation::Add },
    alpha: wgpu::BlendComponent { src_factor: wgpu::BlendFactor::Zero, dst_factor: wgpu::BlendFactor::One, operation: wgpu::BlendOperation::Add },
};

impl Pipes {
    fn new(d: &wgpu::Device, shader: &wgpu::ShaderModule, layout: &wgpu::PipelineLayout, format: wgpu::TextureFormat, samples: u32) -> Self {
        let pipeline_cmp = |fs: &str, blend: Option<wgpu::BlendState>, write: bool, format: wgpu::TextureFormat, cmp: wgpu::CompareFunction| mesh_pipeline(d, shader, layout, fs, blend, Some((write, cmp)), format, samples);
        let pipeline = |fs: &str, blend: Option<wgpu::BlendState>, write: bool, format: wgpu::TextureFormat| pipeline_cmp(fs, blend, write, format, wgpu::CompareFunction::LessEqual);
        let raw = format.remove_srgb_suffix();
        // The gamma pass's glass: colour `src·a + dst·(1 − a)`, coverage `a + dst·(1 − a)`, so a scene drawn into a
        // cleared target and laid over the 2D premultiplied ([`crate::render::Blend::GammaPremul`]) lets `1 − a` of
        // what lies under it through, as the exe's glass over its earlier faders (re/screens/info.md §6.6).
        let over = wgpu::BlendState {
            color: wgpu::BlendComponent { src_factor: wgpu::BlendFactor::SrcAlpha, dst_factor: wgpu::BlendFactor::OneMinusSrcAlpha, operation: wgpu::BlendOperation::Add },
            alpha: wgpu::BlendComponent { src_factor: wgpu::BlendFactor::One, dst_factor: wgpu::BlendFactor::OneMinusSrcAlpha, operation: wgpu::BlendOperation::Add },
        };
        let blob = wgpu::BlendState {
            color: wgpu::BlendComponent { src_factor: wgpu::BlendFactor::Zero, dst_factor: wgpu::BlendFactor::OneMinusSrc, operation: wgpu::BlendOperation::Add },
            alpha: wgpu::BlendComponent { src_factor: wgpu::BlendFactor::Zero, dst_factor: wgpu::BlendFactor::One, operation: wgpu::BlendOperation::Add },
        };
        Self {
            opaque: pipeline("fs_opaque", None, true, format),
            glass: pipeline("fs_glass", Some(wgpu::BlendState::ALPHA_BLENDING), false, format),
            opaque_g: pipeline("fs_opaque_g", None, true, raw),
            glass_g: pipeline("fs_glass_g", Some(over), false, raw),
            opaque_gr: pipeline("fs_opaque_gr", None, true, raw),
            glass_gr: pipeline("fs_glass_gr", Some(over), false, raw),
            glass_w: pipeline("fs_glass", Some(wgpu::BlendState::ALPHA_BLENDING), true, format),
            glass_gw: pipeline("fs_glass_g", Some(over), true, raw),
            video_g: pipeline("fs_video_g", None, true, raw),
            add_g: pipeline("fs_glass_g", Some(ADD), false, raw),
            add_gw: pipeline_cmp("fs_glass_g", Some(ADD), true, raw, wgpu::CompareFunction::Less),
            blob_g: pipeline("fs_blob_g", Some(blob), false, raw),
        }
    }
}

/// The modern lighting's targets for one frame size (ADR 0055), made when first needed.
struct Targets {
    size: (u32, u32),
    /// The 4× colour (the frame's format, its raw view allowed) and depth.
    msaa: Option<(wgpu::Texture, wgpu::TextureView)>,
    /// What an anti-aliased pass that draws over the frame resolves into, and its raw view's bind group.
    over: Option<(wgpu::Texture, wgpu::BindGroup)>,
    /// The half-size glow and its blur, each with its bind group.
    glow: Option<[(wgpu::TextureView, wgpu::BindGroup); 2]>,
}

/// The full-screen passes' pipelines.
struct Post {
    layout: wgpu::BindGroupLayout,
    sampler: wgpu::Sampler,
    /// The anti-aliased pass over the frame, premultiplied.
    over: wgpu::RenderPipeline,
    blur_h: wgpu::RenderPipeline,
    blur_v: wgpu::RenderPipeline,
    /// The glow added to the frame (its colour; the coverage kept).
    bloom: wgpu::RenderPipeline,
    /// The additive parts drawn into the glow (no depth).
    glow: wgpu::RenderPipeline,
}

impl Post {
    fn new(d: &wgpu::Device, shader: &wgpu::ShaderModule, mesh_layout: &wgpu::PipelineLayout, format: wgpu::TextureFormat) -> Self {
        let module = d.create_shader_module(wgpu::ShaderModuleDescriptor { label: Some("ig2-client post"), source: wgpu::ShaderSource::Wgsl(POST.into()) });
        let layout = d.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: None,
            entries: &[
                wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Texture { sample_type: wgpu::TextureSampleType::Float { filterable: true }, view_dimension: wgpu::TextureViewDimension::D2, multisampled: false },
                    count: None,
                },
                wgpu::BindGroupLayoutEntry { binding: 1, visibility: wgpu::ShaderStages::FRAGMENT, ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering), count: None },
            ],
        });
        let pl = d.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor { label: None, bind_group_layouts: &[&layout], push_constant_ranges: &[] });
        let pipe = |fs: &str, format: wgpu::TextureFormat, blend: Option<wgpu::BlendState>, write_mask: wgpu::ColorWrites| {
            d.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
                label: Some("ig2-client post"),
                layout: Some(&pl),
                vertex: wgpu::VertexState { module: &module, entry_point: Some("vs"), compilation_options: Default::default(), buffers: &[] },
                primitive: Default::default(),
                depth_stencil: None,
                multisample: Default::default(),
                fragment: Some(wgpu::FragmentState { module: &module, entry_point: Some(fs), compilation_options: Default::default(), targets: &[Some(wgpu::ColorTargetState { format, blend, write_mask })] }),
                multiview: None,
                cache: None,
            })
        };
        let raw = format.remove_srgb_suffix();
        let add = wgpu::BlendState { color: wgpu::BlendComponent { src_factor: wgpu::BlendFactor::One, dst_factor: wgpu::BlendFactor::One, operation: wgpu::BlendOperation::Add }, alpha: wgpu::BlendComponent::REPLACE };
        Self {
            over: pipe("fs_copy", raw, Some(wgpu::BlendState::PREMULTIPLIED_ALPHA_BLENDING), wgpu::ColorWrites::ALL),
            blur_h: pipe("fs_blur_h", GLOW, None, wgpu::ColorWrites::ALL),
            blur_v: pipe("fs_blur_v", GLOW, None, wgpu::ColorWrites::ALL),
            bloom: pipe("fs_bloom", raw, Some(add), wgpu::ColorWrites::COLOR),
            glow: mesh_pipeline(d, shader, mesh_layout, "fs_glass_g", Some(ADD), None, GLOW, 1),
            sampler: d.create_sampler(&wgpu::SamplerDescriptor { mag_filter: wgpu::FilterMode::Linear, min_filter: wgpu::FilterMode::Linear, ..Default::default() }),
            layout,
        }
    }

    fn bind(&self, d: &wgpu::Device, view: &wgpu::TextureView) -> wgpu::BindGroup {
        d.create_bind_group(&wgpu::BindGroupDescriptor {
            label: None,
            layout: &self.layout,
            entries: &[wgpu::BindGroupEntry { binding: 0, resource: wgpu::BindingResource::TextureView(view) }, wgpu::BindGroupEntry { binding: 1, resource: wgpu::BindingResource::Sampler(&self.sampler) }],
        })
    }

    /// One full-screen pass of `pipe` from `src` into `view`.
    fn run(enc: &mut wgpu::CommandEncoder, pipe: &wgpu::RenderPipeline, src: &wgpu::BindGroup, view: &wgpu::TextureView, load: wgpu::LoadOp<wgpu::Color>) {
        let mut pass = enc.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: Some("post"),
            color_attachments: &[Some(wgpu::RenderPassColorAttachment { view, resolve_target: None, ops: wgpu::Operations { load, store: wgpu::StoreOp::Store } })],
            depth_stencil_attachment: None,
            timestamp_writes: None,
            occlusion_query_set: None,
        });
        pass.set_pipeline(pipe);
        pass.set_bind_group(0, src, &[]);
        pass.draw(0..3, 0..1);
    }
}

pub(crate) struct Pipe3d {
    pipes: Pipes,
    /// The same at 4× (anti-aliasing), made when first used.
    msaa: Option<Pipes>,
    shader: wgpu::ShaderModule,
    layout: wgpu::PipelineLayout,
    format: wgpu::TextureFormat,
    post: Post,
    targets: Vec<Targets>,
    cloud: wgpu::RenderPipeline,
    shadow: wgpu::RenderPipeline,
    uniform: wgpu::Buffer,
    uniform_bg: wgpu::BindGroup,
    shadow_uniform: wgpu::Buffer,
    shadow_view: wgpu::TextureView,
    shadow_bg: wgpu::BindGroup,
    /// Group 2 for the light-space pass itself (a 1 × 1 stand-in: the map can't be sampled while drawn).
    shadow_pass_bg: wgpu::BindGroup,
    meshes: Vec<wgpu::Buffer>,
    dynamic: Option<(wgpu::Buffer, u64)>,
    /// The depth buffers by target size, the last used first (a frame draws targets of a few sizes: the window, a
    /// screen's models, the overlay's backdrop).
    depth: Vec<(wgpu::TextureView, (u32, u32))>,
}

pub(crate) const DEPTH: wgpu::TextureFormat = wgpu::TextureFormat::Depth32Float;

/// The shadow map's size (`ig2_fragment.shader`'s desktop `shadow_size`).
const SHADOW_SIZE: u32 = 2048;

/// The anti-aliasing's sample count.
const SAMPLES: u32 = 4;

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
        let entry = |binding: u32, ty: wgpu::BindingType| wgpu::BindGroupLayoutEntry { binding, visibility: wgpu::ShaderStages::VERTEX_FRAGMENT, ty, count: None };
        let sbl = d.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: None,
            entries: &[
                entry(0, wgpu::BindingType::Buffer { ty: wgpu::BufferBindingType::Uniform, has_dynamic_offset: false, min_binding_size: None }),
                entry(1, wgpu::BindingType::Texture { sample_type: wgpu::TextureSampleType::Float { filterable: true }, view_dimension: wgpu::TextureViewDimension::D2, multisampled: false }),
                entry(2, wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering)),
            ],
        });
        let layout = d.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor { label: None, bind_group_layouts: &[&ubl, tex_layout, &sbl], push_constant_ranges: &[] });
        let attrs = attrs();
        let pipes = Pipes::new(d, &shader, &layout, format, 1);
        let shadow = d.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("ig2-client shadow map"),
            layout: Some(&layout),
            vertex: wgpu::VertexState {
                module: &shader,
                entry_point: Some("vs_shadow"),
                compilation_options: Default::default(),
                buffers: &[wgpu::VertexBufferLayout { array_stride: std::mem::size_of::<MeshVertex>() as u64, step_mode: wgpu::VertexStepMode::Vertex, attributes: &attrs }],
            },
            primitive: wgpu::PrimitiveState { cull_mode: None, ..Default::default() },
            depth_stencil: None,
            multisample: Default::default(),
            fragment: Some(wgpu::FragmentState {
                module: &shader,
                entry_point: Some("fs_shadow"),
                compilation_options: Default::default(),
                targets: &[Some(wgpu::ColorTargetState { format: wgpu::TextureFormat::Rgba8Unorm, blend: None, write_mask: wgpu::ColorWrites::ALL })],
            }),
            multiview: None,
            cache: None,
        });
        let shadow_uniform = d.create_buffer(&wgpu::BufferDescriptor { label: None, size: 48, usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST, mapped_at_creation: false });
        let shadow_view = d
            .create_texture(&wgpu::TextureDescriptor {
                label: Some("shadow map"),
                size: wgpu::Extent3d { width: SHADOW_SIZE, height: SHADOW_SIZE, depth_or_array_layers: 1 },
                mip_level_count: 1,
                sample_count: 1,
                dimension: wgpu::TextureDimension::D2,
                format: wgpu::TextureFormat::Rgba8Unorm,
                usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::TEXTURE_BINDING,
                view_formats: &[],
            })
            .create_view(&Default::default());
        // Past the map's edge receivers read its border texels (clamped).
        let shadow_smp = d.create_sampler(&wgpu::SamplerDescriptor { mag_filter: wgpu::FilterMode::Linear, min_filter: wgpu::FilterMode::Linear, ..Default::default() });
        let shadow_bg = d.create_bind_group(&wgpu::BindGroupDescriptor {
            label: None,
            layout: &sbl,
            entries: &[
                wgpu::BindGroupEntry { binding: 0, resource: shadow_uniform.as_entire_binding() },
                wgpu::BindGroupEntry { binding: 1, resource: wgpu::BindingResource::TextureView(&shadow_view) },
                wgpu::BindGroupEntry { binding: 2, resource: wgpu::BindingResource::Sampler(&shadow_smp) },
            ],
        });
        let stand_in = d
            .create_texture(&wgpu::TextureDescriptor {
                label: None,
                size: wgpu::Extent3d { width: 1, height: 1, depth_or_array_layers: 1 },
                mip_level_count: 1,
                sample_count: 1,
                dimension: wgpu::TextureDimension::D2,
                format: wgpu::TextureFormat::Rgba8Unorm,
                usage: wgpu::TextureUsages::TEXTURE_BINDING,
                view_formats: &[],
            })
            .create_view(&Default::default());
        let shadow_pass_bg = d.create_bind_group(&wgpu::BindGroupDescriptor {
            label: None,
            layout: &sbl,
            entries: &[
                wgpu::BindGroupEntry { binding: 0, resource: shadow_uniform.as_entire_binding() },
                wgpu::BindGroupEntry { binding: 1, resource: wgpu::BindingResource::TextureView(&stand_in) },
                wgpu::BindGroupEntry { binding: 2, resource: wgpu::BindingResource::Sampler(&shadow_smp) },
            ],
        });
        // `video.state`: no depth test, SRC_ALPHA / ONE_MINUS_SRC_ALPHA on the frame's raw (gamma) view.
        let cloud = d.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("ig2-client clouds"),
            layout: Some(&layout),
            vertex: wgpu::VertexState {
                module: &shader,
                entry_point: Some("vs"),
                compilation_options: Default::default(),
                buffers: &[wgpu::VertexBufferLayout { array_stride: std::mem::size_of::<MeshVertex>() as u64, step_mode: wgpu::VertexStepMode::Vertex, attributes: &attrs }],
            },
            primitive: wgpu::PrimitiveState { cull_mode: None, ..Default::default() },
            depth_stencil: None,
            multisample: Default::default(),
            fragment: Some(wgpu::FragmentState {
                module: &shader,
                entry_point: Some("fs_cloud"),
                compilation_options: Default::default(),
                targets: &[Some(wgpu::ColorTargetState { format: format.remove_srgb_suffix(), blend: Some(wgpu::BlendState::ALPHA_BLENDING), write_mask: wgpu::ColorWrites::ALL })],
            }),
            multiview: None,
            cache: None,
        });
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
        Self {
            pipes,
            msaa: None,
            post: Post::new(d, &shader, &layout, format),
            shader,
            layout,
            format,
            targets: Vec::new(),
            cloud,
            shadow,
            uniform,
            uniform_bg,
            shadow_uniform,
            shadow_view,
            shadow_bg,
            shadow_pass_bg,
            meshes: Vec::new(),
            dynamic: None,
            depth: Vec::new(),
        }
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

    /// The modern lighting's targets for `size` first in [`Pipe3d::targets`], with what `l` needs made.
    fn prepare(&mut self, d: &wgpu::Device, size: (u32, u32), l: Lighting) {
        if l.msaa && self.msaa.is_none() {
            self.msaa = Some(Pipes::new(d, &self.shader, &self.layout, self.format, SAMPLES));
        }
        match self.targets.iter().position(|t| t.size == size) {
            Some(i) => {
                let t = self.targets.remove(i);
                self.targets.insert(0, t);
            }
            None => {
                // The battle draws five sizes a frame (the window, its column, the info model, the console's).
                self.targets.insert(0, Targets { size, msaa: None, over: None, glow: None });
                self.targets.truncate(6);
            }
        }
        let tex = |label: &str, size: (u32, u32), samples: u32, format: wgpu::TextureFormat, usage: wgpu::TextureUsages| {
            d.create_texture(&wgpu::TextureDescriptor {
                label: Some(label),
                size: wgpu::Extent3d { width: size.0.max(1), height: size.1.max(1), depth_or_array_layers: 1 },
                mip_level_count: 1,
                sample_count: samples,
                dimension: wgpu::TextureDimension::D2,
                format,
                usage,
                view_formats: &crate::render::Renderer::view_formats(format),
            })
        };
        let (post, format) = (&self.post, self.format);
        let t = &mut self.targets[0];
        if l.msaa && t.msaa.is_none() {
            let depth = tex("depth 4x", size, SAMPLES, DEPTH, wgpu::TextureUsages::RENDER_ATTACHMENT).create_view(&Default::default());
            t.msaa = Some((tex("meshes 4x", size, SAMPLES, format, wgpu::TextureUsages::RENDER_ATTACHMENT), depth));
            let over = tex("meshes resolved", size, 1, format, wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::TEXTURE_BINDING);
            let bg = post.bind(d, &raw_view(&over));
            t.over = Some((over, bg));
        }
        if l.bloom && t.glow.is_none() {
            let half = (size.0.div_ceil(2), size.1.div_ceil(2));
            t.glow = Some(std::array::from_fn(|_| {
                let v = tex("glow", half, 1, GLOW, wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::TEXTURE_BINDING).create_view(&Default::default());
                let bg = post.bind(d, &v);
                (v, bg)
            }));
        }
    }

    /// Record the 3D pass into `enc`: on the `first` pass the scene's buffers go up (the others reuse them, for
    /// [`crate::render::Frame::late`]); clears colour with `clear` (`None` keeps it: the late parts, a backdrop) and depth, draws `draws`' opaque parts then glass parts in order, then
    /// the cloud parts on `raw`, the target's view without the sRGB conversion. The modern lighting (ADR 0055)
    /// draws the meshes multisampled and adds the additive parts' glow after them.
    #[allow(clippy::too_many_arguments)]
    pub(crate) fn encode(
        &mut self,
        d: &wgpu::Device,
        q: &wgpu::Queue,
        enc: &mut wgpu::CommandEncoder,
        (target, raw): (&wgpu::TextureView, &wgpu::TextureView),
        size: (u32, u32),
        (first, clear): (bool, Option<[f64; 4]>),
        scene: &Scene3d,
        draws: &[MeshDraw],
        (textures, textures_raw): (&[wgpu::BindGroup], &[wgpu::BindGroup]),
    ) {
        let l = lighting();
        let glows = |m: &&MeshDraw| m.additive && m.glass && m.shadow != Shadow::Blob && !m.cloud && !m.video && !m.range.is_empty();
        let bloom = l.bloom && draws.iter().any(|m| glows(&m));
        if l.msaa || bloom {
            self.prepare(d, size, Lighting { bloom, ..l });
        }
        if let Some(i) = self.depth.iter().position(|(_, s)| *s == size) {
            let hit = self.depth.remove(i);
            self.depth.insert(0, hit);
        } else {
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
            self.depth.insert(0, (t.create_view(&Default::default()), size));
            self.depth.truncate(4);
        }
        if first {
            q.write_buffer(&self.uniform, 0, bytemuck::bytes_of(&scene.uniforms));
        }
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
        if !bytes.is_empty() && first {
            q.write_buffer(dynbuf, 0, bytes);
        }
        // With `gamma` the clear colour is stored as given (`glClearColor` on the remaster's non-sRGB frame).
        let load = clear.map_or(wgpu::LoadOp::Load, |[r, g, b, a]| wgpu::LoadOp::Clear(wgpu::Color { r, g, b, a }));
        if first {
            let [a, b] = scene.shadow.map_or([[0.0; 3]; 2], |m| m.rows);
            let t = scene.uniforms.eye[3];
            let soft = if l.soft_shadows { 1.0 } else { 0.0 };
            q.write_buffer(&self.shadow_uniform, 0, bytemuck::cast_slice(&[a[0], a[1], a[2], 0.0, b[0], b[1], b[2], 0.0, size.1 as f32, t - t.floor(), soft, 0.0]));
        }
        if let (Some(_), true) = (scene.shadow, first) {
            // `command_glSetTarget(1)` (ELF 0x427c00): the map cleared to white, then the casters.
            let mut pass = enc.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("shadow map"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: &self.shadow_view,
                    resolve_target: None,
                    ops: wgpu::Operations { load: wgpu::LoadOp::Clear(wgpu::Color::WHITE), store: wgpu::StoreOp::Store },
                })],
                depth_stencil_attachment: None,
                timestamp_writes: None,
                occlusion_query_set: None,
            });
            pass.set_pipeline(&self.shadow);
            pass.set_bind_group(0, &self.uniform_bg, &[]);
            pass.set_bind_group(1, &textures[0], &[]);
            pass.set_bind_group(2, &self.shadow_pass_bg, &[]);
            for m in draws.iter().filter(|m| matches!(m.shadow, Shadow::Cast | Shadow::CastOnly) && !m.range.is_empty()) {
                pass.set_vertex_buffer(0, match m.mesh {
                    Some(MeshId(i)) => self.meshes[i].slice(..),
                    None => dynbuf.slice(..),
                });
                pass.draw(m.range.clone(), 0..1);
            }
        }
        let (target, textures) = if scene.gamma { (raw, textures_raw) } else { (target, textures) };
        // Anti-aliasing: the 4× target, resolved into the frame when the pass clears it, else into `over` (cleared
        // transparent) and laid over the frame after.
        let view_format = if scene.gamma { self.format.remove_srgb_suffix() } else { self.format };
        let ms = l.msaa.then(|| &self.targets[0]).and_then(|t| Some((t.msaa.as_ref()?, t.over.as_ref()?)));
        let ms_view = ms.map(|((c, _), _)| c.create_view(&wgpu::TextureViewDescriptor { format: Some(view_format), ..Default::default() }));
        let over_view = ms.filter(|_| clear.is_none()).map(|(_, (o, _))| o.create_view(&wgpu::TextureViewDescriptor { format: Some(view_format), ..Default::default() }));
        let (view, resolve_target, load, store) = match &ms_view {
            Some(v) => (v, Some(over_view.as_ref().unwrap_or(target)), if clear.is_some() { load } else { wgpu::LoadOp::Clear(wgpu::Color::TRANSPARENT) }, wgpu::StoreOp::Discard),
            None => (target, None, load, wgpu::StoreOp::Store),
        };
        let pipes = if ms.is_some() { self.msaa.as_ref().unwrap() } else { &self.pipes };
        let mut pass = enc.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: Some("meshes"),
            color_attachments: &[Some(wgpu::RenderPassColorAttachment { view, resolve_target, ops: wgpu::Operations { load, store } })],
            depth_stencil_attachment: Some(wgpu::RenderPassDepthStencilAttachment {
                view: ms.map_or(&self.depth[0].0, |((_, depth), _)| depth),
                depth_ops: Some(wgpu::Operations { load: wgpu::LoadOp::Clear(1.0), store: wgpu::StoreOp::Discard }),
                stencil_ops: None,
            }),
            timestamp_writes: None,
            occlusion_query_set: None,
        });
        pass.set_bind_group(0, &self.uniform_bg, &[]);
        pass.set_bind_group(2, &self.shadow_bg, &[]);
        // `renderscene`: `Kocka` (the opaque objects, then the translucent ones) before `Lines` (re/screens/spacewar.md
        // §3 step 11; the colony's alike): the engine's lines (additive, not glass) come last, so a translucent object
        // behind one isn't cut by the depth it writes (the shield rings under the battle's grid).
        for phase in 0..3 {
            let glass = phase == 1;
            for m in draws.iter().filter(|m| !m.cloud && m.shadow != Shadow::CastOnly && !m.range.is_empty() && m.glass == glass && (phase == 1 || (phase == 2) == (m.additive && !m.video))) {
                let receives = scene.shadow.is_some() && m.shadow == Shadow::Receive;
                pass.set_pipeline(match (scene.gamma, glass, receives) {
                    (true, false, _) if m.video => &pipes.video_g,
                    (true, true, _) if m.shadow == Shadow::Blob => &pipes.blob_g,
                    (true, true, _) if m.additive => &pipes.add_g,
                    (true, false, _) if m.additive => &pipes.add_gw,
                    (false, true, _) if m.depth_write => &pipes.glass_w,
                    (true, true, false) if m.depth_write => &pipes.glass_gw,
                    (false, false, _) => &pipes.opaque,
                    (false, true, _) => &pipes.glass,
                    (true, false, false) => &pipes.opaque_g,
                    (true, true, false) => &pipes.glass_g,
                    (true, false, true) => &pipes.opaque_gr,
                    (true, true, true) => &pipes.glass_gr,
                });
                let buf = match m.mesh {
                    Some(MeshId(i)) => &self.meshes[i],
                    None => dynbuf,
                };
                pass.set_vertex_buffer(0, buf.slice(..));
                pass.set_bind_group(1, &textures[m.tex.0], &[]);
                pass.draw(m.range.clone(), 0..1);
            }
        }
        drop(pass);
        if let (Some(_), Some((_, (_, over)))) = (&over_view, ms) {
            Post::run(enc, &self.post.over, over, raw, wgpu::LoadOp::Load);
        }
        if let Some([(a, a_bg), (b, b_bg)]) = self.targets.first().filter(|_| bloom).and_then(|t| t.glow.as_ref()) {
            // Bloom: the additive translucent parts again into the half-size glow, blurred both ways, added at half.
            let mut pass = enc.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("glow"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment { view: a, resolve_target: None, ops: wgpu::Operations { load: wgpu::LoadOp::Clear(wgpu::Color::TRANSPARENT), store: wgpu::StoreOp::Store } })],
                depth_stencil_attachment: None,
                timestamp_writes: None,
                occlusion_query_set: None,
            });
            pass.set_pipeline(&self.post.glow);
            pass.set_bind_group(0, &self.uniform_bg, &[]);
            pass.set_bind_group(2, &self.shadow_bg, &[]);
            for m in draws.iter().filter(glows) {
                pass.set_vertex_buffer(0, match m.mesh {
                    Some(MeshId(i)) => self.meshes[i].slice(..),
                    None => dynbuf.slice(..),
                });
                pass.set_bind_group(1, &textures[m.tex.0], &[]);
                pass.draw(m.range.clone(), 0..1);
            }
            drop(pass);
            Post::run(enc, &self.post.blur_h, a_bg, b, wgpu::LoadOp::Clear(wgpu::Color::TRANSPARENT));
            Post::run(enc, &self.post.blur_v, b_bg, a, wgpu::LoadOp::Clear(wgpu::Color::TRANSPARENT));
            Post::run(enc, &self.post.bloom, a_bg, raw, wgpu::LoadOp::Load);
        }
        if !draws.iter().any(|m| m.cloud && !m.range.is_empty()) {
            return;
        }
        let mut pass = enc.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: Some("clouds"),
            color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                view: raw,
                resolve_target: None,
                ops: wgpu::Operations { load: wgpu::LoadOp::Load, store: wgpu::StoreOp::Store },
            })],
            depth_stencil_attachment: None,
            timestamp_writes: None,
            occlusion_query_set: None,
        });
        pass.set_bind_group(0, &self.uniform_bg, &[]);
        pass.set_bind_group(2, &self.shadow_bg, &[]);
        pass.set_pipeline(&self.cloud);
        pass.set_vertex_buffer(0, dynbuf.slice(..));
        for m in draws.iter().filter(|m| m.cloud && m.mesh.is_none() && !m.range.is_empty()) {
            pass.set_bind_group(1, &textures[m.tex.0], &[]);
            pass.draw(m.range.clone(), 0..1);
        }
    }
}

/// `tex` viewed without its sRGB transfer.
fn raw_view(tex: &wgpu::Texture) -> wgpu::TextureView {
    tex.create_view(&wgpu::TextureViewDescriptor { format: Some(tex.format().remove_srgb_suffix()), ..Default::default() })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn lighting_options_round_trip() {
        assert_eq!(Lighting::default().bits(), 0);
        for b in 0..16 {
            assert_eq!(Lighting::from_bits(b).bits(), b);
        }
    }
}
