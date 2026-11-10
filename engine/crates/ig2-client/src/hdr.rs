//! The floating-point scene a mod's `visuals.toml` switches on (ADR 0088 §8): the scene's passes draw their usual
//! gamma-space values into an `Rgba16Float` picture, unclamped; its bright parts are blurred into a glow and added;
//! a tone curve that leaves every value below the threshold as it is brings the result back onto the frame.

/// The scene picture's format.
pub(crate) const FORMAT: wgpu::TextureFormat = wgpu::TextureFormat::Rgba16Float;

const SHADER: &str = r#"
// x: the bright threshold, y: the blur's step in texels.
@group(0) @binding(0) var src: texture_2d<f32>;
@group(0) @binding(1) var smp: sampler;
@group(0) @binding(2) var<uniform> p: vec4<f32>;

@vertex
fn vs(@builtin(vertex_index) i: u32) -> @builtin(position) vec4<f32> {
    let q = vec2(f32((i << 1u) & 2u), f32(i & 2u));
    return vec4(q * 2.0 - vec2(1.0), 0.0, 1.0);
}

// The scene at half size: a colour whose strongest channel is over the threshold, in full at 1 and above.
@fragment
fn fs_bright(@builtin(position) at: vec4<f32>) -> @location(0) vec4<f32> {
    let c = max(textureSampleLevel(src, smp, at.xy * 2.0 / vec2<f32>(textureDimensions(src)), 0.0).rgb, vec3(0.0));
    let m = max(c.r, max(c.g, c.b));
    return vec4(c * clamp((m - p.x) / max(1.0 - p.x, 1e-4), 0.0, 1.0), 1.0);
}

fn blur(at: vec2<f32>, dir: vec2<f32>) -> vec4<f32> {
    var w = array<f32, 5>(0.227027, 0.1945946, 0.1216216, 0.054054, 0.016216);
    let px = 1.0 / vec2<f32>(textureDimensions(src));
    let uv = at * px;
    var c = textureSampleLevel(src, smp, uv, 0.0) * w[0];
    for (var k = 1; k < 5; k++) {
        let o = dir * px * f32(k) * p.y;
        c += (textureSampleLevel(src, smp, uv + o, 0.0) + textureSampleLevel(src, smp, uv - o, 0.0)) * w[k];
    }
    return c;
}

@fragment
fn fs_blur_h(@builtin(position) at: vec4<f32>) -> @location(0) vec4<f32> {
    return blur(at.xy, vec2(1.0, 0.0));
}

@fragment
fn fs_blur_v(@builtin(position) at: vec4<f32>) -> @location(0) vec4<f32> {
    return blur(at.xy, vec2(0.0, 1.0));
}

// The half-size glow over the scene (the blend takes its strength).
@fragment
fn fs_glow(@builtin(position) at: vec4<f32>) -> @location(0) vec4<f32> {
    return vec4(textureSampleLevel(src, smp, at.xy * 0.5 / vec2<f32>(textureDimensions(src)), 0.0).rgb, 0.0);
}

// The tone curve: a value up to the threshold as it is, the rest eased towards 1 (slope 1 at the threshold).
@fragment
fn fs_tone(@builtin(position) at: vec4<f32>) -> @location(0) vec4<f32> {
    let c = textureLoad(src, vec2<i32>(at.xy), 0);
    let k = max(1.0 - p.x, 1e-4);
    let x = max(c.rgb, vec3(0.0));
    return vec4(select(p.x + k * (1.0 - exp((p.x - x) / k)), x, x <= vec3(p.x)), c.a);
}
"#;

/// The tone curve of `fs_tone` (its test pins the identity below the threshold).
#[cfg(test)]
fn tone(x: f32, threshold: f32) -> f32 {
    let k = (1.0 - threshold).max(1e-4);
    if x <= threshold { x } else { threshold + k * (1.0 - ((threshold - x) / k).exp()) }
}

/// The pictures for one frame size: the scene, and the half-size glow and its blur, each with its bind group.
struct Targets {
    size: (u32, u32),
    scene: (wgpu::TextureView, wgpu::BindGroup),
    glow: [(wgpu::TextureView, wgpu::BindGroup); 2],
}

pub(crate) struct Hdr {
    layout: wgpu::BindGroupLayout,
    sampler: wgpu::Sampler,
    uniform: wgpu::Buffer,
    bright: wgpu::RenderPipeline,
    blur_h: wgpu::RenderPipeline,
    blur_v: wgpu::RenderPipeline,
    glow: wgpu::RenderPipeline,
    tone: wgpu::RenderPipeline,
    /// By size, the last used first.
    targets: Vec<Targets>,
}

impl Hdr {
    /// `out`: the format of the frame's view the tone curve writes (gamma-space values: the raw view).
    pub(crate) fn new(d: &wgpu::Device, out: wgpu::TextureFormat) -> Self {
        let module = d.create_shader_module(wgpu::ShaderModuleDescriptor { label: Some("ig2-client hdr"), source: wgpu::ShaderSource::Wgsl(SHADER.into()) });
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
                wgpu::BindGroupLayoutEntry {
                    binding: 2,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Buffer { ty: wgpu::BufferBindingType::Uniform, has_dynamic_offset: false, min_binding_size: None },
                    count: None,
                },
            ],
        });
        let pl = d.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor { label: None, bind_group_layouts: &[&layout], push_constant_ranges: &[] });
        let pipe = |fs: &str, format: wgpu::TextureFormat, blend: Option<wgpu::BlendState>| {
            d.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
                label: Some("ig2-client hdr"),
                layout: Some(&pl),
                vertex: wgpu::VertexState { module: &module, entry_point: Some("vs"), compilation_options: Default::default(), buffers: &[] },
                primitive: Default::default(),
                depth_stencil: None,
                multisample: Default::default(),
                fragment: Some(wgpu::FragmentState { module: &module, entry_point: Some(fs), compilation_options: Default::default(), targets: &[Some(wgpu::ColorTargetState { format, blend, write_mask: wgpu::ColorWrites::ALL })] }),
                multiview: None,
                cache: None,
            })
        };
        // The glow times the pass's blend constant (its strength) added; the scene's coverage kept.
        let add = wgpu::BlendState {
            color: wgpu::BlendComponent { src_factor: wgpu::BlendFactor::Constant, dst_factor: wgpu::BlendFactor::One, operation: wgpu::BlendOperation::Add },
            alpha: wgpu::BlendComponent { src_factor: wgpu::BlendFactor::Zero, dst_factor: wgpu::BlendFactor::One, operation: wgpu::BlendOperation::Add },
        };
        Self {
            bright: pipe("fs_bright", FORMAT, None),
            blur_h: pipe("fs_blur_h", FORMAT, None),
            blur_v: pipe("fs_blur_v", FORMAT, None),
            glow: pipe("fs_glow", FORMAT, Some(add)),
            tone: pipe("fs_tone", out, None),
            sampler: d.create_sampler(&wgpu::SamplerDescriptor { mag_filter: wgpu::FilterMode::Linear, min_filter: wgpu::FilterMode::Linear, ..Default::default() }),
            uniform: d.create_buffer(&wgpu::BufferDescriptor { label: Some("hdr"), size: 16, usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST, mapped_at_creation: false }),
            layout,
            targets: Vec::new(),
        }
    }

    /// The scene picture for a frame of `size`, to draw into.
    pub(crate) fn scene(&mut self, d: &wgpu::Device, size: (u32, u32)) -> wgpu::TextureView {
        match self.targets.iter().position(|t| t.size == size) {
            Some(i) => {
                let t = self.targets.remove(i);
                self.targets.insert(0, t);
            }
            None => {
                let picture = |label: &str, (w, h): (u32, u32)| {
                    let view = d
                        .create_texture(&wgpu::TextureDescriptor {
                            label: Some(label),
                            size: wgpu::Extent3d { width: w.max(1), height: h.max(1), depth_or_array_layers: 1 },
                            mip_level_count: 1,
                            sample_count: 1,
                            dimension: wgpu::TextureDimension::D2,
                            format: FORMAT,
                            usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::TEXTURE_BINDING,
                            view_formats: &[],
                        })
                        .create_view(&Default::default());
                    let bg = d.create_bind_group(&wgpu::BindGroupDescriptor {
                        label: None,
                        layout: &self.layout,
                        entries: &[
                            wgpu::BindGroupEntry { binding: 0, resource: wgpu::BindingResource::TextureView(&view) },
                            wgpu::BindGroupEntry { binding: 1, resource: wgpu::BindingResource::Sampler(&self.sampler) },
                            wgpu::BindGroupEntry { binding: 2, resource: self.uniform.as_entire_binding() },
                        ],
                    });
                    (view, bg)
                };
                let half = (size.0.div_ceil(2), size.1.div_ceil(2));
                let t = Targets { size, scene: picture("hdr scene", size), glow: [picture("hdr glow", half), picture("hdr glow", half)] };
                self.targets.insert(0, t);
                self.targets.truncate(3);
            }
        }
        self.targets[0].scene.0.clone()
    }

    /// The glow added to the scene picture last handed out by [`Hdr::scene`], and the picture tone mapped into `out`.
    pub(crate) fn finish(&self, q: &wgpu::Queue, enc: &mut wgpu::CommandEncoder, out: &wgpu::TextureView, (threshold, strength): (f32, f32)) {
        let Some(t) = self.targets.first() else { return };
        // The blur reaches as far on a taller frame (two rounds of ±4 steps at half size).
        let step = (t.size.1 as f32 / 1080.0).max(1.0);
        q.write_buffer(&self.uniform, 0, bytemuck::cast_slice(&[threshold, step, 0.0, 0.0]));
        let s = strength as f64;
        let mut run = |pipe: &wgpu::RenderPipeline, src: &wgpu::BindGroup, view: &wgpu::TextureView, load: wgpu::LoadOp<wgpu::Color>| {
            let mut pass = enc.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("hdr"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment { view, resolve_target: None, ops: wgpu::Operations { load, store: wgpu::StoreOp::Store } })],
                depth_stencil_attachment: None,
                timestamp_writes: None,
                occlusion_query_set: None,
            });
            pass.set_pipeline(pipe);
            pass.set_blend_constant(wgpu::Color { r: s, g: s, b: s, a: s });
            pass.set_bind_group(0, src, &[]);
            pass.draw(0..3, 0..1);
        };
        let clear = wgpu::LoadOp::Clear(wgpu::Color::TRANSPARENT);
        let [(a, a_bg), (b, b_bg)] = &t.glow;
        if strength > 0.0 {
            run(&self.bright, &t.scene.1, a, clear);
            for _ in 0..2 {
                run(&self.blur_h, a_bg, b, clear);
                run(&self.blur_v, b_bg, a, clear);
            }
            run(&self.glow, a_bg, &t.scene.0, wgpu::LoadOp::Load);
        }
        run(&self.tone, &t.scene.1, out, wgpu::LoadOp::Load);
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn the_tone_curve_keeps_what_is_below_the_threshold() {
        for t in [0.0, 0.5, 0.8, 1.0] {
            for i in 0..=20 {
                let x = i as f32 / 20.0 * t;
                assert_eq!(super::tone(x, t), x);
            }
            // Above: rising, never over 1 (but for rounding and the floor of `k`), joined without a step.
            let (mut last, mut x) = (t, t);
            while x < 8.0 {
                x += 0.01;
                let y = super::tone(x, t);
                assert!(y >= last - 1e-6 && y <= 1.0002, "{t} {x} {y}");
                last = y;
            }
            assert!((super::tone(t + 1e-4, t) - t).abs() < 2e-4);
        }
    }
}
