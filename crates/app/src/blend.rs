//! How a map layer is mixed with the ones under it: `normal` (the layer over them, at its opacity), `multiply` (the
//! layer's colours darken what is under it: a hillshade, a shadow) or `screen` (they lighten it: a glow, a haze).
//!
//! The picture made on the CPU (the figure, the export, the animation, `terraflux plot`) mixes each pixel with
//! [`Blend::mix`]. The live view is drawn by egui's painter, which only knows alpha blending, so a layer that is not
//! `normal` is drawn by a paint callback ([`Tile`]) in the GPU's own blend state (multiply: the colour times what is
//! there; screen: the colour plus what is there times one minus it), from a texture of its own made once per tile in
//! the colouring. The two give the same colours (the GPU's, when the target is sRGB, blends in linear light).

use std::collections::HashMap;
use std::sync::Arc;

use eframe::egui_wgpu::{self, CallbackResources, CallbackTrait, wgpu};
use egui::Color32;

use crate::palette::{Coloring, image_f32};

/// How a layer is mixed with the ones under it.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub enum Blend {
    /// The layer over the others, at its opacity.
    #[default]
    Normal,
    /// The colours multiplied: white leaves what is under it, black makes it black.
    Multiply,
    /// The colours' complements multiplied: black leaves what is under it, white makes it white.
    Screen,
}

impl Blend {
    pub const ALL: [Blend; 3] = [Blend::Normal, Blend::Multiply, Blend::Screen];

    /// Its name in a Plot and in the interface.
    pub fn key(self) -> &'static str {
        match self {
            Blend::Normal => "normal",
            Blend::Multiply => "multiply",
            Blend::Screen => "screen",
        }
    }

    /// The mode a name is (any case); none for what is not one.
    pub fn named(name: &str) -> Option<Blend> {
        Blend::ALL.into_iter().find(|b| b.key().eq_ignore_ascii_case(name.trim()))
    }

    /// What it does, for the hover text.
    pub fn about(self) -> &'static str {
        match self {
            Blend::Normal => "the layer over the ones under it, at its opacity",
            Blend::Multiply => "its colours multiplied with what is under it: a hillshade or a shadow darkens it",
            Blend::Screen => "its colours screened with what is under it: lighter, never darker",
        }
    }

    /// As a Plot keeps it: nothing for `normal`.
    pub fn to_plot(self) -> Option<String> {
        (self != Blend::Normal).then(|| self.key().to_string())
    }

    /// From what a Plot keeps (anything else is `normal`).
    pub fn of_plot(p: Option<&str>) -> Blend {
        p.and_then(Blend::named).unwrap_or_default()
    }

    /// `top` (premultiplied, its opacity already in it) over `dst` (premultiplied) in this mode: with the colours as
    /// 0 to 1 and `P`, `D` premultiplied, `ta`, `da` their alphas, the result is `P (1 - da) + D (1 - ta) + ta da B`
    /// where `B` is the mode's mix of the two unpremultiplied colours (`normal`: the top's; multiply: their product;
    /// screen: one minus the product of their complements), so no division is needed.
    pub fn mix(self, dst: Color32, top: Color32) -> Color32 {
        let f = |c: u8| f32::from(c) / 255.0;
        let (ta, da) = (f(top.a()), f(dst.a()));
        let one = |p: u8, d: u8| {
            let (p, d) = (f(p), f(d));
            let v = match self {
                Blend::Normal => p + d * (1.0 - ta),
                Blend::Multiply => p * (1.0 - da) + d * (1.0 - ta) + p * d,
                Blend::Screen => p * (1.0 - da) + d * (1.0 - ta) + p * da + d * ta - p * d,
            };
            (v * 255.0).round().clamp(0.0, 255.0) as u8
        };
        let a = (ta + da - ta * da) * 255.0;
        Color32::from_rgba_premultiplied(
            one(top.r(), dst.r()),
            one(top.g(), dst.g()),
            one(top.b(), dst.b()),
            a.round().clamp(0.0, 255.0) as u8,
        )
    }
}

/// A tile of a layer drawn in a blend mode by the GPU: the cells' values (its texture is made from them in the
/// colouring, once), where on the texture (`uv`: the corners, 0 to 1) and how opaque.
pub struct Tile {
    /// What tells its texture from the others (the tile in a colouring), and the frame it is drawn in.
    pub id: u64,
    pub frame: u64,
    pub values: Arc<Vec<f32>>,
    pub coloring: Arc<Coloring>,
    pub side: usize,
    pub uv: [f32; 4],
    pub opacity: f32,
    pub mode: Blend,
}

impl Tile {
    /// What tells this draw from the others: the texture, the part of it, the opacity and the mode.
    fn draw_id(&self) -> u64 {
        use std::hash::{Hash, Hasher};
        let mut h = std::hash::DefaultHasher::new();
        self.id.hash(&mut h);
        self.uv.iter().for_each(|v| v.to_bits().hash(&mut h));
        self.opacity.to_bits().hash(&mut h);
        self.mode.hash(&mut h);
        h.finish()
    }
}

/// The uniforms of a draw.
#[repr(C)]
#[derive(Clone, Copy, bytemuck::Pod, bytemuck::Zeroable)]
struct Uniforms {
    uv: [f32; 4],
    opacity: f32,
    linear: u32,
    mode: u32,
    pad: u32,
}

/// A draw made ready: its bind group (uniforms, texture, sampler).
struct Draw {
    bind_group: wgpu::BindGroup,
    mode: Blend,
    frame: u64,
}

struct Resources {
    multiply: wgpu::RenderPipeline,
    screen: wgpu::RenderPipeline,
    layout: wgpu::BindGroupLayout,
    sampler: wgpu::Sampler,
    linear: bool,
    textures: HashMap<u64, (wgpu::TextureView, u64)>,
    draws: HashMap<u64, Draw>,
}

/// The most textures and draws kept (those not drawn in the last frames are let go beyond them).
const KEPT: usize = 600;

const SHADER: &str = r#"
struct Uniforms {
    uv: vec4<f32>,
    opacity: f32,
    linear: u32,
    mode: u32,
    pad: u32,
};
@group(0) @binding(0) var<uniform> u: Uniforms;
@group(0) @binding(1) var tex: texture_2d<f32>;
@group(0) @binding(2) var samp: sampler;

struct VsOut {
    @builtin(position) pos: vec4<f32>,
    @location(0) uv: vec2<f32>,
};

// the quad is the whole viewport (egui sets it to the callback's rectangle)
@vertex
fn vs(@builtin(vertex_index) v: u32) -> VsOut {
    let unit = vec2<f32>(f32(v & 1u), f32(v >> 1u));
    var out: VsOut;
    out.pos = vec4<f32>(unit.x * 2.0 - 1.0, 1.0 - unit.y * 2.0, 0.0, 1.0);
    out.uv = mix(u.uv.xy, u.uv.zw, unit);
    return out;
}

fn linear_from_gamma(c: vec3<f32>) -> vec3<f32> {
    let lower = c / 12.92;
    let higher = pow((c + 0.055) / 1.055, vec3<f32>(2.4));
    return select(higher, lower, c < vec3<f32>(0.04045));
}

fn out_colour(c: vec3<f32>) -> vec3<f32> {
    if (u.linear == 1u) {
        return linear_from_gamma(c);
    }
    return c;
}

// the texture holds premultiplied colours
@fragment
fn fs(in: VsOut) -> @location(0) vec4<f32> {
    let c = textureSample(tex, samp, in.uv);
    if (u.mode == 1u) {
        // multiply: what is there times (1 - o (a - P)); the blend state multiplies
        return vec4<f32>(out_colour(vec3<f32>(1.0) - u.opacity * (vec3<f32>(c.a) - c.rgb)), 1.0);
    }
    // screen: S + what is there times (1 - S), S the colour at its opacity
    return vec4<f32>(out_colour(u.opacity * c.rgb), 0.0);
}
"#;

/// Make the pipelines in egui's renderer (once, at the start).
pub fn install(rs: &egui_wgpu::RenderState) {
    let device = &rs.device;
    let module = device.create_shader_module(wgpu::ShaderModuleDescriptor {
        label: Some("terraflux blend"),
        source: wgpu::ShaderSource::Wgsl(SHADER.into()),
    });
    let entry = |binding: u32, ty: wgpu::BindingType| wgpu::BindGroupLayoutEntry {
        binding,
        visibility: wgpu::ShaderStages::VERTEX | wgpu::ShaderStages::FRAGMENT,
        ty,
        count: None,
    };
    let layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
        label: Some("terraflux blend"),
        entries: &[
            entry(
                0,
                wgpu::BindingType::Buffer {
                    ty: wgpu::BufferBindingType::Uniform,
                    has_dynamic_offset: false,
                    min_binding_size: None,
                },
            ),
            entry(
                1,
                wgpu::BindingType::Texture {
                    sample_type: wgpu::TextureSampleType::Float { filterable: true },
                    view_dimension: wgpu::TextureViewDimension::D2,
                    multisampled: false,
                },
            ),
            entry(2, wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering)),
        ],
    });
    let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
        label: Some("terraflux blend"),
        bind_group_layouts: &[Some(&layout)],
        immediate_size: 0,
    });
    let pipeline = |label: &str, color: wgpu::BlendComponent| {
        device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some(label),
            layout: Some(&pipeline_layout),
            vertex: wgpu::VertexState {
                module: &module,
                entry_point: Some("vs"),
                compilation_options: wgpu::PipelineCompilationOptions::default(),
                buffers: &[],
            },
            primitive: wgpu::PrimitiveState { topology: wgpu::PrimitiveTopology::TriangleStrip, ..Default::default() },
            depth_stencil: None,
            multisample: wgpu::MultisampleState::default(),
            fragment: Some(wgpu::FragmentState {
                module: &module,
                entry_point: Some("fs"),
                compilation_options: wgpu::PipelineCompilationOptions::default(),
                targets: &[Some(wgpu::ColorTargetState {
                    format: rs.target_format,
                    // (the alpha is left as it is: the layer is mixed into the colours under it)
                    blend: Some(wgpu::BlendState {
                        color,
                        alpha: wgpu::BlendComponent {
                            src_factor: wgpu::BlendFactor::Zero,
                            dst_factor: wgpu::BlendFactor::One,
                            operation: wgpu::BlendOperation::Add,
                        },
                    }),
                    write_mask: wgpu::ColorWrites::ALL,
                })],
            }),
            multiview_mask: None,
            cache: None,
        })
    };
    let multiply = pipeline(
        "terraflux multiply",
        wgpu::BlendComponent {
            src_factor: wgpu::BlendFactor::Dst,
            dst_factor: wgpu::BlendFactor::Zero,
            operation: wgpu::BlendOperation::Add,
        },
    );
    let screen = pipeline(
        "terraflux screen",
        wgpu::BlendComponent {
            src_factor: wgpu::BlendFactor::One,
            dst_factor: wgpu::BlendFactor::OneMinusSrc,
            operation: wgpu::BlendOperation::Add,
        },
    );
    let sampler = device.create_sampler(&wgpu::SamplerDescriptor::default());
    rs.renderer.write().callback_resources.insert(Resources {
        multiply,
        screen,
        layout,
        sampler,
        linear: rs.target_format.is_srgb(),
        textures: HashMap::new(),
        draws: HashMap::new(),
    });
}

impl CallbackTrait for Tile {
    fn prepare(
        &self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        _screen: &egui_wgpu::ScreenDescriptor,
        _encoder: &mut wgpu::CommandEncoder,
        resources: &mut CallbackResources,
    ) -> Vec<wgpu::CommandBuffer> {
        use wgpu::util::DeviceExt;
        let Some(r) = resources.get_mut::<Resources>() else { return Vec::new() };
        // (what was not drawn in the last frames goes when there are many)
        if r.textures.len() > KEPT || r.draws.len() > KEPT * 4 {
            r.textures.retain(|_, (_, f)| *f + 2 >= self.frame);
            r.draws.retain(|_, d| d.frame + 2 >= self.frame);
        }
        let side = self.side as u32;
        let (view, seen) = r.textures.entry(self.id).or_insert_with(|| {
            let img = image_f32(&self.values, self.side, self.side, &self.coloring);
            let bytes: Vec<u8> = img.pixels.iter().flat_map(|p| p.to_array()).collect();
            let texture = device.create_texture_with_data(
                queue,
                &wgpu::TextureDescriptor {
                    label: Some("terraflux blend tile"),
                    size: wgpu::Extent3d { width: side, height: side, depth_or_array_layers: 1 },
                    mip_level_count: 1,
                    sample_count: 1,
                    dimension: wgpu::TextureDimension::D2,
                    format: wgpu::TextureFormat::Rgba8Unorm,
                    usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
                    view_formats: &[],
                },
                wgpu::util::TextureDataOrder::LayerMajor,
                &bytes,
            );
            (texture.create_view(&wgpu::TextureViewDescriptor::default()), self.frame)
        });
        *seen = self.frame;
        let view = view.clone();
        let id = self.draw_id();
        let linear = r.linear;
        let (layout, sampler) = (&r.layout, &r.sampler);
        let draw = r.draws.entry(id).or_insert_with(|| {
            let u = Uniforms {
                uv: self.uv,
                opacity: self.opacity,
                linear: u32::from(linear),
                mode: if self.mode == Blend::Multiply { 1 } else { 2 },
                pad: 0,
            };
            let uniforms = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
                label: Some("terraflux blend uniforms"),
                contents: bytemuck::bytes_of(&u),
                usage: wgpu::BufferUsages::UNIFORM,
            });
            let bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
                label: Some("terraflux blend"),
                layout,
                entries: &[
                    wgpu::BindGroupEntry { binding: 0, resource: uniforms.as_entire_binding() },
                    wgpu::BindGroupEntry { binding: 1, resource: wgpu::BindingResource::TextureView(&view) },
                    wgpu::BindGroupEntry { binding: 2, resource: wgpu::BindingResource::Sampler(sampler) },
                ],
            });
            Draw { bind_group, mode: self.mode, frame: self.frame }
        });
        draw.frame = self.frame;
        Vec::new()
    }

    fn paint(
        &self,
        _info: egui::PaintCallbackInfo,
        pass: &mut wgpu::RenderPass<'static>,
        resources: &CallbackResources,
    ) {
        let Some(r) = resources.get::<Resources>() else { return };
        let Some(d) = r.draws.get(&self.draw_id()) else { return };
        pass.set_pipeline(if d.mode == Blend::Multiply { &r.multiply } else { &r.screen });
        pass.set_bind_group(0, &d.bind_group, &[]);
        pass.draw(0..4, 0..1);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn px(r: u8, g: u8, b: u8, a: u8) -> Color32 {
        Color32::from_rgba_premultiplied(r, g, b, a)
    }

    #[test]
    fn the_names_of_the_modes() {
        for b in Blend::ALL {
            assert_eq!(Blend::named(b.key()), Some(b));
        }
        assert_eq!(Blend::named(" Multiply "), Some(Blend::Multiply));
        assert_eq!(Blend::named("overlay"), None);
        // (a Plot leaves `normal` out and reads what it does not know as `normal`)
        assert_eq!(Blend::Normal.to_plot(), None);
        assert_eq!(Blend::of_plot(Blend::Screen.to_plot().as_deref()), Blend::Screen);
        assert_eq!(Blend::of_plot(Some("overlay")), Blend::Normal);
        assert_eq!(Blend::of_plot(None), Blend::Normal);
    }

    #[test]
    fn normal_is_the_top_over_the_bottom() {
        // an opaque top hides the bottom; a half transparent one lets half of it through
        let dst = px(200, 100, 50, 255);
        assert_eq!(Blend::Normal.mix(dst, px(10, 20, 30, 255)), px(10, 20, 30, 255));
        assert_eq!(Blend::Normal.mix(dst, Color32::TRANSPARENT), dst);
        let half = Blend::Normal.mix(dst, px(50, 50, 50, 128));
        assert_eq!(half, px(50 + (200.0 * 127.0 / 255.0f32).round() as u8, 50 + 50, 50 + 25, 255));
    }

    #[test]
    fn multiply_darkens_by_the_colours() {
        let dst = px(200, 100, 50, 255);
        // white leaves it, black makes it black, a grey of half takes half of each channel
        assert_eq!(Blend::Multiply.mix(dst, px(255, 255, 255, 255)), dst);
        assert_eq!(Blend::Multiply.mix(dst, px(0, 0, 0, 255)), px(0, 0, 0, 255));
        assert_eq!(Blend::Multiply.mix(dst, px(102, 102, 102, 255)), px(80, 40, 20, 255));
        // nothing on top leaves it as it is, and a half transparent black darkens it by half
        assert_eq!(Blend::Multiply.mix(dst, Color32::TRANSPARENT), dst);
        let half = Blend::Multiply.mix(dst, px(0, 0, 0, 128));
        let expect = |c: f32| (c * (1.0 - 128.0 / 255.0)).round() as u8;
        assert_eq!(half, px(expect(200.0), expect(100.0), expect(50.0), 255));
        // (never lighter than the bottom)
        let m = Blend::Multiply.mix(dst, px(100, 200, 150, 255));
        assert!(m.r() <= dst.r() && m.g() <= dst.g() && m.b() <= dst.b());
    }

    #[test]
    fn screen_lightens_by_the_colours() {
        let dst = px(200, 100, 50, 255);
        // black leaves it, white makes it white, a grey of half takes half of what is missing
        assert_eq!(Blend::Screen.mix(dst, px(0, 0, 0, 255)), dst);
        assert_eq!(Blend::Screen.mix(dst, px(255, 255, 255, 255)), px(255, 255, 255, 255));
        assert_eq!(Blend::Screen.mix(dst, px(102, 102, 102, 255)), px(222, 162, 132, 255));
        assert_eq!(Blend::Screen.mix(dst, Color32::TRANSPARENT), dst);
        let m = Blend::Screen.mix(dst, px(100, 20, 150, 255));
        assert!(m.r() >= dst.r() && m.g() >= dst.g() && m.b() >= dst.b());
    }

    #[test]
    fn a_layer_over_nothing_is_itself_in_every_mode() {
        let top = px(30, 60, 90, 200);
        for b in Blend::ALL {
            assert_eq!(b.mix(Color32::TRANSPARENT, top), top, "{b:?}");
        }
    }
}
