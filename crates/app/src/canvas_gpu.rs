//! The canvas of the model builder drawn by the GPU: the boxes and ports of the nodes as rounded rectangles and
//! the connections as curves, each kind in one instanced draw whatever the number of nodes. The instances are
//! in the graph's coordinates and uploaded when the graph changes; moving and zooming the view only changes a
//! uniform. The shapes' edges are smoothed in the fragment shader (signed distances), in screen pixels.
//!
//! The pipelines are made once ([`install`]) in egui's renderer; a frame hands its instances over in an egui
//! paint callback ([`Canvas`]), drawn in egui's own render pass under the texts egui draws on top.

use std::sync::Arc;

use eframe::egui_wgpu::{self, CallbackResources, CallbackTrait, wgpu};
use egui::Color32;

/// A rounded rectangle (a node's box, a port, a light): its corner, size and corner radius in the graph's
/// units, its fill and border (premultiplied sRGB, as egui's colours), the border's width in points.
#[repr(C)]
#[derive(Clone, Copy, Debug, PartialEq, bytemuck::Pod, bytemuck::Zeroable)]
pub struct RectInstance {
    pub min: [f32; 2],
    pub size: [f32; 2],
    pub fill: [u8; 4],
    pub border: [u8; 4],
    pub radius: f32,
    pub border_width: f32,
}

impl RectInstance {
    pub fn new(min: [f32; 2], size: [f32; 2], radius: f32, fill: Color32, border: Color32, width: f32) -> Self {
        RectInstance { min, size, fill: fill.to_array(), border: border.to_array(), radius, border_width: width }
    }
}

/// A connection: a cubic curve from an output port to an input port (leaving and arriving horizontally), its
/// colour and width in points.
#[repr(C)]
#[derive(Clone, Copy, Debug, PartialEq, bytemuck::Pod, bytemuck::Zeroable)]
pub struct EdgeInstance {
    pub from: [f32; 2],
    pub to: [f32; 2],
    pub color: [u8; 4],
    pub width: f32,
}

/// What a canvas draws: changed (a new `generation`) only when the graph changes.
#[derive(Debug, Default, Clone)]
pub struct Shapes {
    pub generation: u64,
    pub rects: Vec<RectInstance>,
    pub edges: Vec<EdgeInstance>,
}

#[repr(C)]
#[derive(Clone, Copy, Debug, PartialEq, bytemuck::Pod, bytemuck::Zeroable)]
struct Uniforms {
    /// the size of the canvas, points
    size: [f32; 2],
    /// where the graph's origin is on the canvas, points
    origin: [f32; 2],
    /// points per unit of the graph
    zoom: f32,
    pixels_per_point: f32,
    /// whether the target converts to sRGB itself (colours given linear then)
    linear: u32,
    /// segments of a curve
    segments: u32,
}

/// A frame's drawing of a canvas.
pub struct Canvas {
    pub shapes: Arc<Shapes>,
    /// the canvas's size and where the graph's origin is on it (points, from its corner), the zoom
    pub size: [f32; 2],
    pub origin: [f32; 2],
    pub zoom: f32,
}

struct Buffer {
    buffer: wgpu::Buffer,
    capacity: u64,
}

struct Resources {
    rects: wgpu::RenderPipeline,
    edges: wgpu::RenderPipeline,
    uniforms: wgpu::Buffer,
    bind_group: wgpu::BindGroup,
    linear: bool,
    rect_buffer: Option<Buffer>,
    edge_buffer: Option<Buffer>,
    rect_count: u32,
    edge_count: u32,
    generation: u64,
    segments: u32,
}

const SHADER: &str = r#"
struct Uniforms {
    size: vec2<f32>,
    origin: vec2<f32>,
    zoom: f32,
    ppp: f32,
    linear: u32,
    segments: u32,
};
@group(0) @binding(0) var<uniform> u: Uniforms;

fn to_clip(p: vec2<f32>) -> vec4<f32> {
    return vec4<f32>(p.x / u.size.x * 2.0 - 1.0, 1.0 - p.y / u.size.y * 2.0, 0.0, 1.0);
}

fn screen(world: vec2<f32>) -> vec2<f32> {
    return u.origin + world * u.zoom;
}

fn linear_from_gamma(c: vec3<f32>) -> vec3<f32> {
    let lower = c / 12.92;
    let higher = pow((c + 0.055) / 1.055, vec3<f32>(2.4));
    return select(higher, lower, c < vec3<f32>(0.04045));
}

fn out_colour(c: vec4<f32>) -> vec4<f32> {
    if (u.linear == 1u) {
        return vec4<f32>(linear_from_gamma(c.rgb), c.a);
    }
    return c;
}

// ---- rounded rectangles

struct RectOut {
    @builtin(position) pos: vec4<f32>,
    // the pixel from the rectangle's centre, its half size and radius (pixels)
    @location(0) local: vec2<f32>,
    @location(1) half: vec2<f32>,
    @location(2) radius: f32,
    @location(3) border_width: f32,
    @location(4) fill: vec4<f32>,
    @location(5) border: vec4<f32>,
};

@vertex
fn vs_rect(
    @builtin(vertex_index) v: u32,
    @location(0) corner: vec2<f32>,
    @location(1) extent: vec2<f32>,
    @location(2) fill: vec4<f32>,
    @location(3) border: vec4<f32>,
    @location(4) radius: f32,
    @location(5) border_width: f32,
) -> RectOut {
    let unit = vec2<f32>(f32(v & 1u), f32(v >> 1u));
    let half_points = extent * u.zoom * 0.5;
    let centre = screen(corner) + half_points;
    // a pixel more around, for the smoothed edge
    let pad = 1.0 / u.ppp;
    let p = centre + (unit * 2.0 - 1.0) * (half_points + pad);
    var out: RectOut;
    out.pos = to_clip(p);
    out.local = (p - centre) * u.ppp;
    out.half = half_points * u.ppp;
    out.radius = min(radius * u.zoom * u.ppp, min(out.half.x, out.half.y));
    out.border_width = border_width * u.ppp;
    out.fill = fill;
    out.border = border;
    return out;
}

@fragment
fn fs_rect(in: RectOut) -> @location(0) vec4<f32> {
    let q = abs(in.local) - in.half + in.radius;
    let d = length(max(q, vec2<f32>(0.0))) + min(max(q.x, q.y), 0.0) - in.radius;
    let outside = clamp(0.5 - d, 0.0, 1.0);
    var c = in.fill;
    if (in.border_width > 0.0) {
        let inside_border = clamp(0.5 - (d + in.border_width), 0.0, 1.0);
        c = mix(in.border, in.fill, inside_border);
    }
    return out_colour(c * outside);
}

// ---- connections

struct EdgeOut {
    @builtin(position) pos: vec4<f32>,
    // the distance across the curve and its half width (pixels)
    @location(0) across: f32,
    @location(1) half: f32,
    @location(2) color: vec4<f32>,
};

@vertex
fn vs_edge(
    @builtin(vertex_index) v: u32,
    @location(0) start: vec2<f32>,
    @location(1) end: vec2<f32>,
    @location(2) color: vec4<f32>,
    @location(3) width: f32,
) -> EdgeOut {
    let t = f32(v / 2u) / f32(u.segments);
    let side = select(-1.0, 1.0, (v & 1u) == 1u);
    let p0 = screen(start);
    let p3 = screen(end);
    let dx = max(abs(p3.x - p0.x) * 0.5, 30.0 * u.zoom);
    let p1 = p0 + vec2<f32>(dx, 0.0);
    let p2 = p3 - vec2<f32>(dx, 0.0);
    let s = 1.0 - t;
    let p = s * s * s * p0 + 3.0 * s * s * t * p1 + 3.0 * s * t * t * p2 + t * t * t * p3;
    let d = 3.0 * s * s * (p1 - p0) + 6.0 * s * t * (p2 - p1) + 3.0 * t * t * (p3 - p2);
    var n = vec2<f32>(0.0, 1.0);
    if (length(d) > 1e-6) {
        let k = normalize(d);
        n = vec2<f32>(-k.y, k.x);
    }
    let half = max(width * min(u.zoom, 1.5), 0.6) * 0.5;
    let reach = half + 1.0 / u.ppp;
    var out: EdgeOut;
    out.pos = to_clip(p + n * side * reach);
    out.across = side * reach * u.ppp;
    out.half = half * u.ppp;
    out.color = color;
    return out;
}

@fragment
fn fs_edge(in: EdgeOut) -> @location(0) vec4<f32> {
    let a = clamp(in.half + 0.5 - abs(in.across), 0.0, 1.0);
    return out_colour(in.color * a);
}
"#;

/// Make the canvas's pipelines in egui's renderer (once, at the start).
pub fn install(rs: &egui_wgpu::RenderState) {
    let device = &rs.device;
    let format = rs.target_format;
    let module = device.create_shader_module(wgpu::ShaderModuleDescriptor {
        label: Some("terraflux canvas"),
        source: wgpu::ShaderSource::Wgsl(SHADER.into()),
    });
    let layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
        label: Some("terraflux canvas"),
        entries: &[wgpu::BindGroupLayoutEntry {
            binding: 0,
            visibility: wgpu::ShaderStages::VERTEX | wgpu::ShaderStages::FRAGMENT,
            ty: wgpu::BindingType::Buffer {
                ty: wgpu::BufferBindingType::Uniform,
                has_dynamic_offset: false,
                min_binding_size: None,
            },
            count: None,
        }],
    });
    let uniforms = device.create_buffer(&wgpu::BufferDescriptor {
        label: Some("terraflux canvas uniforms"),
        size: std::mem::size_of::<Uniforms>() as u64,
        usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
        mapped_at_creation: false,
    });
    let bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
        label: Some("terraflux canvas"),
        layout: &layout,
        entries: &[wgpu::BindGroupEntry { binding: 0, resource: uniforms.as_entire_binding() }],
    });
    let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
        label: Some("terraflux canvas"),
        bind_group_layouts: &[Some(&layout)],
        immediate_size: 0,
    });
    let target = [Some(wgpu::ColorTargetState {
        format,
        // premultiplied alpha, as egui
        blend: Some(wgpu::BlendState {
            color: wgpu::BlendComponent {
                src_factor: wgpu::BlendFactor::One,
                dst_factor: wgpu::BlendFactor::OneMinusSrcAlpha,
                operation: wgpu::BlendOperation::Add,
            },
            alpha: wgpu::BlendComponent {
                src_factor: wgpu::BlendFactor::OneMinusDstAlpha,
                dst_factor: wgpu::BlendFactor::One,
                operation: wgpu::BlendOperation::Add,
            },
        }),
        write_mask: wgpu::ColorWrites::ALL,
    })];
    let pipeline = |label: &str, vs: &str, fs: &str, attributes: &[wgpu::VertexAttribute], stride: u64| {
        device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some(label),
            layout: Some(&pipeline_layout),
            vertex: wgpu::VertexState {
                module: &module,
                entry_point: Some(vs),
                compilation_options: wgpu::PipelineCompilationOptions::default(),
                buffers: &[Some(wgpu::VertexBufferLayout {
                    array_stride: stride,
                    step_mode: wgpu::VertexStepMode::Instance,
                    attributes,
                })],
            },
            primitive: wgpu::PrimitiveState { topology: wgpu::PrimitiveTopology::TriangleStrip, ..Default::default() },
            depth_stencil: None,
            multisample: wgpu::MultisampleState::default(),
            fragment: Some(wgpu::FragmentState {
                module: &module,
                entry_point: Some(fs),
                compilation_options: wgpu::PipelineCompilationOptions::default(),
                targets: &target,
            }),
            multiview_mask: None,
            cache: None,
        })
    };
    let rects = pipeline(
        "terraflux canvas rects",
        "vs_rect",
        "fs_rect",
        &wgpu::vertex_attr_array![0 => Float32x2, 1 => Float32x2, 2 => Unorm8x4, 3 => Unorm8x4, 4 => Float32, 5 => Float32],
        std::mem::size_of::<RectInstance>() as u64,
    );
    let edges = pipeline(
        "terraflux canvas edges",
        "vs_edge",
        "fs_edge",
        &wgpu::vertex_attr_array![0 => Float32x2, 1 => Float32x2, 2 => Unorm8x4, 3 => Float32],
        std::mem::size_of::<EdgeInstance>() as u64,
    );
    rs.renderer.write().callback_resources.insert(Resources {
        rects,
        edges,
        uniforms,
        bind_group,
        linear: format.is_srgb(),
        rect_buffer: None,
        edge_buffer: None,
        rect_count: 0,
        edge_count: 0,
        generation: u64::MAX,
        segments: 24,
    });
}

/// Write instances into a buffer, made larger when they do not fit.
fn upload<T: bytemuck::Pod>(device: &wgpu::Device, queue: &wgpu::Queue, slot: &mut Option<Buffer>, items: &[T]) {
    let bytes: &[u8] = bytemuck::cast_slice(items);
    if bytes.is_empty() {
        return;
    }
    let need = bytes.len() as u64;
    if slot.as_ref().is_none_or(|b| b.capacity < need) {
        let capacity = need.next_power_of_two().max(4096);
        let buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("terraflux canvas instances"),
            size: capacity,
            usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        *slot = Some(Buffer { buffer, capacity });
    }
    queue.write_buffer(&slot.as_ref().unwrap().buffer, 0, bytes);
}

impl CallbackTrait for Canvas {
    fn prepare(
        &self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        screen: &egui_wgpu::ScreenDescriptor,
        _encoder: &mut wgpu::CommandEncoder,
        resources: &mut CallbackResources,
    ) -> Vec<wgpu::CommandBuffer> {
        let Some(r) = resources.get_mut::<Resources>() else { return Vec::new() };
        if r.generation != self.shapes.generation {
            upload(device, queue, &mut r.rect_buffer, &self.shapes.rects);
            upload(device, queue, &mut r.edge_buffer, &self.shapes.edges);
            r.rect_count = self.shapes.rects.len() as u32;
            r.edge_count = self.shapes.edges.len() as u32;
            r.generation = self.shapes.generation;
        }
        // fewer segments per curve when they are small on the screen
        r.segments = if self.zoom < 0.3 {
            6
        } else if self.zoom < 0.7 {
            12
        } else {
            24
        };
        let u = Uniforms {
            size: self.size,
            origin: self.origin,
            zoom: self.zoom,
            pixels_per_point: screen.pixels_per_point,
            linear: u32::from(r.linear),
            segments: r.segments,
        };
        queue.write_buffer(&r.uniforms, 0, bytemuck::bytes_of(&u));
        Vec::new()
    }

    fn paint(
        &self,
        _info: egui::PaintCallbackInfo,
        pass: &mut wgpu::RenderPass<'static>,
        resources: &CallbackResources,
    ) {
        let Some(r) = resources.get::<Resources>() else { return };
        pass.set_bind_group(0, &r.bind_group, &[]);
        if let (Some(b), true) = (&r.edge_buffer, r.edge_count > 0) {
            pass.set_pipeline(&r.edges);
            pass.set_vertex_buffer(0, b.buffer.slice(..));
            pass.draw(0..(r.segments + 1) * 2, 0..r.edge_count);
        }
        if let (Some(b), true) = (&r.rect_buffer, r.rect_count > 0) {
            pass.set_pipeline(&r.rects);
            pass.set_vertex_buffer(0, b.buffer.slice(..));
            pass.draw(0..4, 0..r.rect_count);
        }
    }
}
