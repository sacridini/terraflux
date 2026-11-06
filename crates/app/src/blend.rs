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

/// A tile of a layer of another coordinate system drawn in a blend mode by the GPU: the tile's mesh on the view
/// ([`crate::reproject::mesh`]: each vertex a screen point and a place on the texture) instead of a quad. Its texture
/// is the one of a [`Tile`] with the same `id`.
pub struct MeshTile {
    /// What tells its texture from the others (the tile in a colouring), and the frame it is drawn in.
    pub id: u64,
    pub frame: u64,
    pub values: Arc<Vec<f32>>,
    pub coloring: Arc<Coloring>,
    pub side: usize,
    pub opacity: f32,
    pub mode: Blend,
    /// The screen rectangle the callback is given (the vertices are points of the screen).
    pub view: egui::Rect,
    /// Screen x and y, then the texture's u and v, of each vertex.
    pub vertices: Vec<[f32; 4]>,
    pub indices: Vec<u32>,
    /// What tells this mesh from the others in the frame.
    slot: u64,
}

impl MeshTile {
    pub fn new(
        (id, frame, mode, opacity): (u64, u64, Blend, f32),
        (values, coloring, side): (Arc<Vec<f32>>, Arc<Coloring>, usize),
        view: egui::Rect,
        mesh: &egui::Mesh,
    ) -> Self {
        use std::hash::{Hash, Hasher};
        let vertices: Vec<[f32; 4]> = mesh.vertices.iter().map(|v| [v.pos.x, v.pos.y, v.uv.x, v.uv.y]).collect();
        let mut h = std::hash::DefaultHasher::new();
        (id, frame).hash(&mut h);
        vertices.iter().flatten().for_each(|v| v.to_bits().hash(&mut h));
        MeshTile {
            id,
            frame,
            values,
            coloring,
            side,
            opacity,
            mode,
            view,
            vertices,
            indices: mesh.indices.clone(),
            slot: h.finish(),
        }
    }

    /// What tells this draw's bind group from the others: the texture, the opacity, the mode and the rectangle.
    fn draw_id(&self) -> u64 {
        use std::hash::{Hash, Hasher};
        let mut h = std::hash::DefaultHasher::new();
        (self.id, "mesh").hash(&mut h);
        self.opacity.to_bits().hash(&mut h);
        self.mode.hash(&mut h);
        [self.view.min.x, self.view.min.y, self.view.width(), self.view.height()]
            .iter()
            .for_each(|v| v.to_bits().hash(&mut h));
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
    /// A mesh's rectangle on the screen: x, y, width, height (nothing for a quad).
    view: [f32; 4],
}

/// A draw made ready: its bind group (uniforms, texture, sampler).
struct Draw {
    bind_group: wgpu::BindGroup,
    mode: Blend,
    frame: u64,
}

/// A mesh's buffers, made for the frame.
struct MeshBuffers {
    vertices: wgpu::Buffer,
    indices: wgpu::Buffer,
    count: u32,
    frame: u64,
}

struct Resources {
    multiply: wgpu::RenderPipeline,
    screen: wgpu::RenderPipeline,
    multiply_mesh: wgpu::RenderPipeline,
    screen_mesh: wgpu::RenderPipeline,
    layout: wgpu::BindGroupLayout,
    sampler: wgpu::Sampler,
    linear: bool,
    textures: HashMap<u64, (wgpu::TextureView, u64)>,
    draws: HashMap<u64, Draw>,
    meshes: HashMap<u64, MeshBuffers>,
}

/// The colour part of the GPU's blend state of a mode (the pipelines of the quads and of the meshes alike).
fn blend_color(mode: Blend) -> wgpu::BlendComponent {
    match mode {
        Blend::Multiply => wgpu::BlendComponent {
            src_factor: wgpu::BlendFactor::Dst,
            dst_factor: wgpu::BlendFactor::Zero,
            operation: wgpu::BlendOperation::Add,
        },
        _ => wgpu::BlendComponent {
            src_factor: wgpu::BlendFactor::One,
            dst_factor: wgpu::BlendFactor::OneMinusSrc,
            operation: wgpu::BlendOperation::Add,
        },
    }
}

/// A mesh vertex: the screen point, then the place on the texture.
const MESH_ATTRS: [wgpu::VertexAttribute; 2] = wgpu::vertex_attr_array![0 => Float32x2, 1 => Float32x2];

/// The most textures and draws kept (those not drawn in the last frames are let go beyond them).
const KEPT: usize = 600;

const SHADER: &str = r#"
struct Uniforms {
    uv: vec4<f32>,
    opacity: f32,
    linear: u32,
    mode: u32,
    pad: u32,
    view: vec4<f32>,
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

struct MeshIn {
    @location(0) pos: vec2<f32>,
    @location(1) uv: vec2<f32>,
};

// a mesh: the vertices are points of the screen, within the rectangle the callback is given
@vertex
fn vs_mesh(in: MeshIn) -> VsOut {
    let unit = (in.pos - u.view.xy) / u.view.zw;
    var out: VsOut;
    out.pos = vec4<f32>(unit.x * 2.0 - 1.0, 1.0 - unit.y * 2.0, 0.0, 1.0);
    out.uv = in.uv;
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
    let mesh_buffer = Some(wgpu::VertexBufferLayout {
        array_stride: 16,
        step_mode: wgpu::VertexStepMode::Vertex,
        attributes: &MESH_ATTRS,
    });
    // (a quad: no buffers, a strip; a mesh: its vertices, a list of triangles)
    let pipeline = |label: &str, mode: Blend, mesh: bool| {
        let color = blend_color(mode);
        device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some(label),
            layout: Some(&pipeline_layout),
            vertex: wgpu::VertexState {
                module: &module,
                entry_point: Some(if mesh { "vs_mesh" } else { "vs" }),
                compilation_options: wgpu::PipelineCompilationOptions::default(),
                buffers: if mesh { std::slice::from_ref(&mesh_buffer) } else { &[] },
            },
            primitive: wgpu::PrimitiveState {
                topology: if mesh {
                    wgpu::PrimitiveTopology::TriangleList
                } else {
                    wgpu::PrimitiveTopology::TriangleStrip
                },
                ..Default::default()
            },
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
    let multiply = pipeline("terraflux multiply", Blend::Multiply, false);
    let screen = pipeline("terraflux screen", Blend::Screen, false);
    let multiply_mesh = pipeline("terraflux multiply mesh", Blend::Multiply, true);
    let screen_mesh = pipeline("terraflux screen mesh", Blend::Screen, true);
    let sampler = device.create_sampler(&wgpu::SamplerDescriptor::default());
    rs.renderer.write().callback_resources.insert(Resources {
        multiply,
        screen,
        multiply_mesh,
        screen_mesh,
        layout,
        sampler,
        linear: rs.target_format.is_srgb(),
        textures: HashMap::new(),
        draws: HashMap::new(),
        meshes: HashMap::new(),
    });
}

/// The texture of a tile in its colouring (made once, kept while drawn).
fn tile_view(
    r: &mut Resources,
    (device, queue): (&wgpu::Device, &wgpu::Queue),
    (id, frame): (u64, u64),
    (values, coloring, side): (&[f32], &Coloring, usize),
) -> wgpu::TextureView {
    use wgpu::util::DeviceExt;
    // (what was not drawn in the last frames goes when there are many)
    if r.textures.len() > KEPT || r.draws.len() > KEPT * 4 {
        r.textures.retain(|_, (_, f)| *f + 2 >= frame);
        r.draws.retain(|_, d| d.frame + 2 >= frame);
    }
    // (a mesh's buffers are of its frame only)
    r.meshes.retain(|_, m| m.frame + 1 >= frame);
    let (view, seen) = r.textures.entry(id).or_insert_with(|| {
        let img = image_f32(values, side, side, coloring);
        let bytes: Vec<u8> = img.pixels.iter().flat_map(|p| p.to_array()).collect();
        let texture = device.create_texture_with_data(
            queue,
            &wgpu::TextureDescriptor {
                label: Some("terraflux blend tile"),
                size: wgpu::Extent3d { width: side as u32, height: side as u32, depth_or_array_layers: 1 },
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
        (texture.create_view(&wgpu::TextureViewDescriptor::default()), frame)
    });
    *seen = frame;
    view.clone()
}

/// A draw's bind group (uniforms, texture, sampler), made once per draw.
fn draw_of<'a>(
    r: &'a mut Resources,
    device: &wgpu::Device,
    (id, mode, frame): (u64, Blend, u64),
    view: &wgpu::TextureView,
    u: &Uniforms,
) -> &'a mut Draw {
    use wgpu::util::DeviceExt;
    let (layout, sampler) = (&r.layout, &r.sampler);
    let draw = r.draws.entry(id).or_insert_with(|| {
        let uniforms = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("terraflux blend uniforms"),
            contents: bytemuck::bytes_of(u),
            usage: wgpu::BufferUsages::UNIFORM,
        });
        let bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("terraflux blend"),
            layout,
            entries: &[
                wgpu::BindGroupEntry { binding: 0, resource: uniforms.as_entire_binding() },
                wgpu::BindGroupEntry { binding: 1, resource: wgpu::BindingResource::TextureView(view) },
                wgpu::BindGroupEntry { binding: 2, resource: wgpu::BindingResource::Sampler(sampler) },
            ],
        });
        Draw { bind_group, mode, frame }
    });
    draw.frame = frame;
    draw
}

impl CallbackTrait for MeshTile {
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
        let view = tile_view(r, (device, queue), (self.id, self.frame), (&self.values, &self.coloring, self.side));
        let u = Uniforms {
            uv: [0.0; 4],
            opacity: self.opacity,
            linear: u32::from(r.linear),
            mode: if self.mode == Blend::Multiply { 1 } else { 2 },
            pad: 0,
            view: [self.view.min.x, self.view.min.y, self.view.width().max(1.0), self.view.height().max(1.0)],
        };
        draw_of(r, device, (self.draw_id(), self.mode, self.frame), &view, &u);
        if !self.indices.is_empty() {
            let vertices = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
                label: Some("terraflux blend mesh"),
                contents: bytemuck::cast_slice(&self.vertices),
                usage: wgpu::BufferUsages::VERTEX,
            });
            let indices = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
                label: Some("terraflux blend mesh"),
                contents: bytemuck::cast_slice(&self.indices),
                usage: wgpu::BufferUsages::INDEX,
            });
            let count = self.indices.len() as u32;
            r.meshes.insert(self.slot, MeshBuffers { vertices, indices, count, frame: self.frame });
        }
        Vec::new()
    }

    fn paint(
        &self,
        _info: egui::PaintCallbackInfo,
        pass: &mut wgpu::RenderPass<'static>,
        resources: &CallbackResources,
    ) {
        let Some(r) = resources.get::<Resources>() else { return };
        let (Some(d), Some(m)) = (r.draws.get(&self.draw_id()), r.meshes.get(&self.slot)) else { return };
        pass.set_pipeline(if d.mode == Blend::Multiply { &r.multiply_mesh } else { &r.screen_mesh });
        pass.set_bind_group(0, &d.bind_group, &[]);
        pass.set_vertex_buffer(0, m.vertices.slice(..));
        pass.set_index_buffer(m.indices.slice(..), wgpu::IndexFormat::Uint32);
        pass.draw_indexed(0..m.count, 0, 0..1);
    }
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
        let Some(r) = resources.get_mut::<Resources>() else { return Vec::new() };
        let view = tile_view(r, (device, queue), (self.id, self.frame), (&self.values, &self.coloring, self.side));
        let u = Uniforms {
            uv: self.uv,
            opacity: self.opacity,
            linear: u32::from(r.linear),
            mode: if self.mode == Blend::Multiply { 1 } else { 2 },
            pad: 0,
            view: [0.0; 4],
        };
        draw_of(r, device, (self.draw_id(), self.mode, self.frame), &view, &u);
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
    fn a_mesh_tile_keeps_the_meshs_points_and_places() {
        let mut m = egui::Mesh::default();
        for (x, y) in [(0.0, 0.0), (8.0, 0.0), (0.0, 8.0)] {
            m.vertices.push(egui::epaint::Vertex {
                pos: egui::pos2(x, y),
                uv: egui::pos2(x / 8.0, y / 8.0),
                color: Color32::WHITE,
            });
        }
        m.indices.extend([0, 1, 2]);
        let view = egui::Rect::from_min_size(egui::pos2(0.0, 0.0), egui::vec2(100.0, 50.0));
        let (values, coloring) = (Arc::new(vec![0.0; 4]), Arc::new(Coloring::Packed));
        let make = |frame: u64, mode: Blend| {
            MeshTile::new((7, frame, mode, 1.0), (values.clone(), coloring.clone(), 2), view, &m)
        };
        let t = make(1, Blend::Multiply);
        assert_eq!(t.vertices, vec![[0.0, 0.0, 0.0, 0.0], [8.0, 0.0, 1.0, 0.0], [0.0, 8.0, 0.0, 1.0]]);
        assert_eq!(t.indices, vec![0, 1, 2]);
        // (a mode, a frame, or other vertices: another draw or slot)
        assert_ne!(t.draw_id(), make(1, Blend::Screen).draw_id());
        assert_ne!(t.slot, make(2, Blend::Multiply).slot);
        assert_eq!(t.slot, make(1, Blend::Screen).slot);
        // the quads' and the meshes' pipelines blend the same way
        assert_eq!(blend_color(Blend::Multiply).src_factor, wgpu::BlendFactor::Dst);
        assert_eq!(blend_color(Blend::Screen).dst_factor, wgpu::BlendFactor::OneMinusSrc);
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
