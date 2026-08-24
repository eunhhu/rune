use std::{
    collections::BTreeMap,
    fs,
    io::{self, BufRead},
    sync::Arc,
    thread,
    time::Duration,
};

use bytemuck::{Pod, Zeroable};
use fontdue::{Font, FontSettings};
use serde::Deserialize;
use tiny_skia::{FillRule, Paint, Path, PathBuilder, PixmapMut, Rect, Stroke, Transform};
use wgpu::util::DeviceExt;
#[cfg(target_os = "macos")]
use winit::platform::macos::{ActivationPolicy, EventLoopBuilderExtMacOS};
use winit::{
    dpi::{PhysicalPosition, PhysicalSize},
    event::{Event, WindowEvent},
    event_loop::{ControlFlow, EventLoopBuilder, EventLoopProxy},
    window::{Window, WindowBuilder, WindowLevel},
};

const TEXTURE_ALIGNMENT_PIXELS: u32 = wgpu::COPY_BYTES_PER_ROW_ALIGNMENT / 4;
const SHADER: &str = r"
struct UvScale { value: vec2<f32>, padding: vec2<f32> };
@group(0) @binding(0) var frame_texture: texture_2d<f32>;
@group(0) @binding(1) var frame_sampler: sampler;
@group(0) @binding(2) var<uniform> uv_scale: UvScale;

struct VertexOutput {
    @builtin(position) position: vec4<f32>,
    @location(0) uv: vec2<f32>,
};

@vertex
fn vs_main(@builtin(vertex_index) index: u32) -> VertexOutput {
    var positions = array<vec2<f32>, 3>(
        vec2<f32>(-1.0, -3.0),
        vec2<f32>(3.0, 1.0),
        vec2<f32>(-1.0, 1.0),
    );
    var coordinates = array<vec2<f32>, 3>(
        vec2<f32>(0.0, 2.0),
        vec2<f32>(2.0, 0.0),
        vec2<f32>(0.0, 0.0),
    );
    var output: VertexOutput;
    output.position = vec4<f32>(positions[index], 0.0, 1.0);
    output.uv = coordinates[index] * uv_scale.value;
    return output;
}

@fragment
fn fs_main(input: VertexOutput) -> @location(0) vec4<f32> {
    return textureSample(frame_texture, frame_sampler, input.uv);
}
";

#[derive(Debug, Deserialize)]
#[serde(tag = "op", rename_all = "camelCase")]
enum Command {
    Batch { mutations: Vec<OverlayMutation> },
    Clear,
    Show,
    Hide,
    Exit,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct OverlayMutation {
    id: u32,
    #[serde(default)]
    node: Option<OverlayNode>,
    #[serde(default)]
    remove: bool,
}

#[derive(Debug, Clone, Deserialize)]
struct StrokeStyle {
    fill: String,
    width: f32,
}

#[derive(Debug, Clone, Deserialize)]
struct ShadowStyle {
    fill: String,
    #[serde(default)]
    x: f32,
    #[serde(default)]
    y: f32,
    #[serde(default)]
    blur: f32,
    #[serde(default)]
    spread: f32,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
struct FontStyle {
    #[serde(default)]
    family: Option<String>,
    #[serde(default)]
    weight: Option<u16>,
    #[serde(default)]
    line_height: Option<f32>,
    #[serde(default)]
    letter_spacing: Option<f32>,
    #[serde(default)]
    align: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(tag = "kind", rename_all = "camelCase")]
enum OverlayNode {
    Text {
        x: f32,
        y: f32,
        #[serde(default)]
        width: Option<f32>,
        #[serde(default)]
        height: Option<f32>,
        text: String,
        size: f32,
        #[serde(default)]
        color: Option<String>,
        #[serde(default)]
        fill: Option<String>,
        #[serde(default = "default_opacity")]
        opacity: f32,
        #[serde(default)]
        font: Option<FontStyle>,
        #[serde(default)]
        z: i32,
    },
    Rect {
        x: f32,
        y: f32,
        width: f32,
        height: f32,
        radius: f32,
        #[serde(default)]
        color: Option<String>,
        #[serde(default)]
        fill: Option<String>,
        #[serde(default)]
        stroke: Option<StrokeStyle>,
        #[serde(default)]
        shadow: Option<ShadowStyle>,
        #[serde(default = "default_opacity")]
        opacity: f32,
        #[serde(default)]
        z: i32,
    },
    Ellipse {
        x: f32,
        y: f32,
        width: f32,
        height: f32,
        #[serde(default)]
        color: Option<String>,
        #[serde(default)]
        fill: Option<String>,
        #[serde(default)]
        stroke: Option<StrokeStyle>,
        #[serde(default)]
        shadow: Option<ShadowStyle>,
        #[serde(default = "default_opacity")]
        opacity: f32,
        #[serde(default)]
        z: i32,
    },
    Line {
        x1: f32,
        y1: f32,
        x2: f32,
        y2: f32,
        width: f32,
        #[serde(default)]
        color: Option<String>,
        #[serde(default)]
        fill: Option<String>,
        #[serde(default = "default_opacity")]
        opacity: f32,
        #[serde(default)]
        z: i32,
    },
}

const fn default_opacity() -> f32 {
    1.0
}

#[derive(Clone, Copy)]
struct Color {
    red: u8,
    green: u8,
    blue: u8,
    alpha: u8,
}

impl Color {
    const WHITE: Self = Self { red: 255, green: 255, blue: 255, alpha: 255 };
    const PANEL: Self = Self { red: 18, green: 18, blue: 22, alpha: 190 };

    fn parse(value: Option<&str>, fallback: Self) -> Self {
        let Some(hex) = value.and_then(|value| value.strip_prefix('#')) else {
            return fallback;
        };
        if hex.len() != 6 && hex.len() != 8 {
            return fallback;
        }
        let parse = |range| u8::from_str_radix(&hex[range], 16).ok();
        let Some(red) = parse(0..2) else { return fallback };
        let Some(green) = parse(2..4) else { return fallback };
        let Some(blue) = parse(4..6) else { return fallback };
        let alpha = if hex.len() == 8 {
            let Some(alpha) = parse(6..8) else { return fallback };
            alpha
        } else {
            255
        };
        Self { red, green, blue, alpha }
    }

    fn paint(self) -> Paint<'static> {
        let mut paint = Paint::default();
        paint.set_color_rgba8(self.red, self.green, self.blue, self.alpha);
        paint.anti_alias = true;
        paint
    }

    fn with_opacity(self, opacity: f32) -> Self {
        #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
        let alpha = (f32::from(self.alpha) * opacity.clamp(0.0, 1.0)).round() as u8;
        Self { alpha, ..self }
    }
}

impl OverlayNode {
    const fn z(&self) -> i32 {
        match self {
            Self::Text { z, .. }
            | Self::Rect { z, .. }
            | Self::Ellipse { z, .. }
            | Self::Line { z, .. } => *z,
        }
    }

    fn bounds(&self) -> Bounds {
        match self {
            Self::Text { x, y, width, height, text, size, font, .. } => {
                let letter_spacing =
                    font.as_ref().and_then(|font| font.letter_spacing).unwrap_or(0.0);
                // Dirty bounds must cover glyph overflow even when the caller's layout width is
                // narrower than the selected platform font. The conservative factor avoids stale
                // pixels without needing font rasterization on the control path.
                let character_count = text.chars().fold(0.0_f32, |count, _| count + 1.0);
                let estimated_width = character_count * *size * 1.2
                    + (character_count - 1.0).max(0.0) * letter_spacing.max(0.0);
                let line_height =
                    font.as_ref().and_then(|font| font.line_height).unwrap_or(*size * 1.2);
                Bounds::new(
                    *x - 2.0,
                    *y - 2.0,
                    width.unwrap_or(estimated_width).max(estimated_width) + 4.0,
                    height.unwrap_or(line_height) + 4.0,
                )
            }
            Self::Rect { x, y, width, height, stroke, shadow, .. }
            | Self::Ellipse { x, y, width, height, stroke, shadow, .. } => {
                shape_bounds(*x, *y, *width, *height, stroke.as_ref(), shadow.as_ref())
            }
            Self::Line { x1, y1, x2, y2, width, .. } => {
                let half = width.max(0.0) / 2.0 + 2.0;
                Bounds::from_edges(
                    x1.min(*x2) - half,
                    y1.min(*y2) - half,
                    x1.max(*x2) + half,
                    y1.max(*y2) + half,
                )
            }
        }
    }
}

#[derive(Clone, Copy, Debug)]
struct Bounds {
    left: f32,
    top: f32,
    right: f32,
    bottom: f32,
}

impl Bounds {
    fn new(x: f32, y: f32, width: f32, height: f32) -> Self {
        Self::from_edges(x, y, x + width.max(0.0), y + height.max(0.0))
    }

    fn from_edges(left: f32, top: f32, right: f32, bottom: f32) -> Self {
        Self { left, top, right, bottom }
    }

    fn union(self, other: Self) -> Self {
        Self {
            left: self.left.min(other.left),
            top: self.top.min(other.top),
            right: self.right.max(other.right),
            bottom: self.bottom.max(other.bottom),
        }
    }

    fn intersects(self, other: Self) -> bool {
        self.left < other.right
            && self.right > other.left
            && self.top < other.bottom
            && self.bottom > other.top
    }
}

fn shape_bounds(
    x: f32,
    y: f32,
    width: f32,
    height: f32,
    stroke: Option<&StrokeStyle>,
    shadow: Option<&ShadowStyle>,
) -> Bounds {
    let stroke_expansion = stroke.map_or(0.0, |stroke| stroke.width.max(0.0) / 2.0);
    let mut bounds = Bounds::new(
        x - stroke_expansion,
        y - stroke_expansion,
        width + stroke_expansion * 2.0,
        height + stroke_expansion * 2.0,
    );
    if let Some(shadow) = shadow {
        let expansion = shadow.blur.clamp(0.0, 64.0) + shadow.spread.max(0.0);
        bounds = bounds.union(Bounds::new(
            x + shadow.x - expansion,
            y + shadow.y - expansion,
            width + expansion * 2.0,
            height + expansion * 2.0,
        ));
    }
    bounds
}

fn union_bounds(target: &mut Option<Bounds>, bounds: Bounds) {
    *target = Some(target.map_or(bounds, |current| current.union(bounds)));
}

#[derive(Default)]
struct FontBook {
    system: Option<Font>,
    system_bold: Option<Font>,
    monospace: Option<Font>,
    monospace_bold: Option<Font>,
}

impl FontBook {
    fn select(&self, style: Option<&FontStyle>) -> Option<&Font> {
        let monospace = style.and_then(|style| style.family.as_deref()) == Some("monospace");
        let bold = style.and_then(|style| style.weight).unwrap_or(400) >= 600;
        match (monospace, bold) {
            (true, true) => self.monospace_bold.as_ref().or(self.monospace.as_ref()),
            (true, false) => self.monospace.as_ref(),
            (false, true) => self.system_bold.as_ref().or(self.system.as_ref()),
            (false, false) => self.system.as_ref(),
        }
    }
}

#[repr(C)]
#[derive(Clone, Copy, Pod, Zeroable)]
struct UvScale {
    value: [f32; 2],
    padding: [f32; 2],
}

#[derive(Clone, Copy)]
struct PixelRegion {
    x: u32,
    y: u32,
    width: u32,
    height: u32,
}

impl PixelRegion {
    #[allow(clippy::cast_possible_truncation, clippy::cast_precision_loss, clippy::cast_sign_loss)]
    fn from_bounds(bounds: Bounds, width: u32, padded_width: u32, height: u32) -> Option<Self> {
        let alignment = TEXTURE_ALIGNMENT_PIXELS;
        let left = bounds.left.floor().max(0.0).min(width as f32) as u32;
        let top = bounds.top.floor().max(0.0).min(height as f32) as u32;
        let right = bounds.right.ceil().max(0.0).min(width as f32) as u32;
        let bottom = bounds.bottom.ceil().max(0.0).min(height as f32) as u32;
        if left >= right || top >= bottom {
            return None;
        }
        let aligned_left = left / alignment * alignment;
        let aligned_right = right.div_ceil(alignment).saturating_mul(alignment).min(padded_width);
        Some(Self {
            x: aligned_left,
            y: top,
            width: aligned_right - aligned_left,
            height: bottom - top,
        })
    }

    fn bounds(self) -> Bounds {
        #[allow(clippy::cast_precision_loss)]
        Bounds::new(self.x as f32, self.y as f32, self.width as f32, self.height as f32)
    }
}

struct Renderer {
    surface: wgpu::Surface<'static>,
    device: wgpu::Device,
    queue: wgpu::Queue,
    config: wgpu::SurfaceConfiguration,
    texture: wgpu::Texture,
    bind_group: wgpu::BindGroup,
    pipeline: wgpu::RenderPipeline,
    width: u32,
    padded_width: u32,
    height: u32,
    scratch: Vec<u8>,
    fonts: FontBook,
}

impl Renderer {
    // GPU surface and pipeline setup is intentionally kept in one fallible constructor so a
    // partially initialized renderer can never escape.
    #[allow(clippy::too_many_lines)]
    fn new(window: Arc<Window>, size: PhysicalSize<u32>) -> Result<Self, String> {
        let instance = wgpu::Instance::default();
        let surface = instance.create_surface(window).map_err(|error| error.to_string())?;
        let adapter = pollster::block_on(instance.request_adapter(&wgpu::RequestAdapterOptions {
            power_preference: wgpu::PowerPreference::LowPower,
            force_fallback_adapter: false,
            compatible_surface: Some(&surface),
        }))
        .ok_or_else(|| "no graphics adapter supports the overlay surface".to_owned())?;
        let (device, queue) = pollster::block_on(adapter.request_device(
            &wgpu::DeviceDescriptor {
                label: Some("Spellwire overlay device"),
                required_features: wgpu::Features::empty(),
                required_limits:
                    wgpu::Limits::downlevel_defaults().using_resolution(adapter.limits()),
            },
            None,
        ))
        .map_err(|error| error.to_string())?;
        let capabilities = surface.get_capabilities(&adapter);
        let format = capabilities
            .formats
            .iter()
            .copied()
            .find(wgpu::TextureFormat::is_srgb)
            .or_else(|| capabilities.formats.first().copied())
            .ok_or_else(|| "overlay surface exposes no texture format".to_owned())?;
        let alpha_mode = capabilities
            .alpha_modes
            .iter()
            .copied()
            .find(|mode| *mode == wgpu::CompositeAlphaMode::PreMultiplied)
            .or_else(|| {
                capabilities
                    .alpha_modes
                    .iter()
                    .copied()
                    .find(|mode| *mode == wgpu::CompositeAlphaMode::PostMultiplied)
            })
            .or_else(|| capabilities.alpha_modes.first().copied())
            .ok_or_else(|| "overlay surface exposes no alpha mode".to_owned())?;
        let config = wgpu::SurfaceConfiguration {
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
            format,
            width: size.width.max(1),
            height: size.height.max(1),
            present_mode: wgpu::PresentMode::AutoNoVsync,
            alpha_mode,
            view_formats: vec![],
            desired_maximum_frame_latency: 1,
        };
        surface.configure(&device, &config);

        let bind_group_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("Spellwire overlay bind group layout"),
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
                wgpu::BindGroupLayoutEntry {
                    binding: 2,
                    visibility: wgpu::ShaderStages::VERTEX,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Uniform,
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                },
            ],
        });
        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("Spellwire overlay shader"),
            source: wgpu::ShaderSource::Wgsl(SHADER.into()),
        });
        let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("Spellwire overlay pipeline layout"),
            bind_group_layouts: &[&bind_group_layout],
            push_constant_ranges: &[],
        });
        let pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("Spellwire overlay pipeline"),
            layout: Some(&pipeline_layout),
            vertex: wgpu::VertexState { module: &shader, entry_point: "vs_main", buffers: &[] },
            fragment: Some(wgpu::FragmentState {
                module: &shader,
                entry_point: "fs_main",
                targets: &[Some(wgpu::ColorTargetState {
                    format,
                    blend: None,
                    write_mask: wgpu::ColorWrites::ALL,
                })],
            }),
            primitive: wgpu::PrimitiveState::default(),
            depth_stencil: None,
            multisample: wgpu::MultisampleState::default(),
            multiview: None,
        });
        let fonts = load_fonts();
        let (texture, bind_group, padded_width, scratch) =
            create_frame_resources(&device, &bind_group_layout, config.width, config.height);
        Ok(Self {
            surface,
            device,
            queue,
            config,
            texture,
            bind_group,
            pipeline,
            width: size.width.max(1),
            padded_width,
            height: size.height.max(1),
            scratch,
            fonts,
        })
    }

    fn resize(&mut self, size: PhysicalSize<u32>) {
        if size.width == 0 || size.height == 0 {
            return;
        }
        self.width = size.width;
        self.height = size.height;
        self.config.width = size.width;
        self.config.height = size.height;
        self.surface.configure(&self.device, &self.config);
        let bind_group_layout = self.pipeline.get_bind_group_layout(0);
        let (texture, bind_group, padded_width, scratch) =
            create_frame_resources(&self.device, &bind_group_layout, self.width, self.height);
        self.texture = texture;
        self.bind_group = bind_group;
        self.padded_width = padded_width;
        self.scratch = scratch;
    }

    #[allow(clippy::cast_precision_loss)]
    fn full_bounds(&self) -> Bounds {
        Bounds::new(0.0, 0.0, self.width as f32, self.height as f32)
    }

    fn render(&mut self, nodes: &BTreeMap<u32, OverlayNode>, dirty: Bounds) -> Result<(), String> {
        let Some(region) =
            PixelRegion::from_bounds(dirty, self.width, self.padded_width, self.height)
        else {
            return Ok(());
        };
        let scratch_len = usize::try_from(region.width)
            .unwrap_or_default()
            .saturating_mul(usize::try_from(region.height).unwrap_or_default())
            .saturating_mul(4);
        self.scratch.resize(scratch_len, 0);
        self.scratch.fill(0);
        let mut pixmap = PixmapMut::from_bytes(&mut self.scratch, region.width, region.height)
            .ok_or_else(|| "overlay frame dimensions are invalid".to_owned())?;
        let mut ordered = nodes.iter().collect::<Vec<_>>();
        ordered.sort_by_key(|(id, node)| (node.z(), **id));
        for (_, node) in ordered {
            if node.bounds().intersects(region.bounds()) {
                #[allow(clippy::cast_precision_loss)]
                draw_node(&mut pixmap, &self.fonts, node, region.x as f32, region.y as f32);
            }
        }
        self.queue.write_texture(
            wgpu::ImageCopyTexture {
                texture: &self.texture,
                mip_level: 0,
                origin: wgpu::Origin3d { x: region.x, y: region.y, z: 0 },
                aspect: wgpu::TextureAspect::All,
            },
            &self.scratch,
            wgpu::ImageDataLayout {
                offset: 0,
                bytes_per_row: Some(region.width * 4),
                rows_per_image: Some(region.height),
            },
            wgpu::Extent3d { width: region.width, height: region.height, depth_or_array_layers: 1 },
        );
        let output = self.surface.get_current_texture().map_err(|error| error.to_string())?;
        let view = output.texture.create_view(&wgpu::TextureViewDescriptor::default());
        let mut encoder = self.device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
            label: Some("Spellwire overlay encoder"),
        });
        {
            let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("Spellwire overlay pass"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: &view,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Clear(wgpu::Color::TRANSPARENT),
                        store: wgpu::StoreOp::Store,
                    },
                })],
                depth_stencil_attachment: None,
                timestamp_writes: None,
                occlusion_query_set: None,
            });
            pass.set_pipeline(&self.pipeline);
            pass.set_bind_group(0, &self.bind_group, &[]);
            pass.draw(0..3, 0..1);
        }
        self.queue.submit(Some(encoder.finish()));
        output.present();
        Ok(())
    }
}

fn create_frame_resources(
    device: &wgpu::Device,
    layout: &wgpu::BindGroupLayout,
    width: u32,
    height: u32,
) -> (wgpu::Texture, wgpu::BindGroup, u32, Vec<u8>) {
    let padded_width = width.div_ceil(TEXTURE_ALIGNMENT_PIXELS) * TEXTURE_ALIGNMENT_PIXELS;
    let texture = device.create_texture(&wgpu::TextureDescriptor {
        label: Some("Spellwire overlay frame"),
        size: wgpu::Extent3d { width: padded_width, height, depth_or_array_layers: 1 },
        mip_level_count: 1,
        sample_count: 1,
        dimension: wgpu::TextureDimension::D2,
        format: wgpu::TextureFormat::Rgba8UnormSrgb,
        usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
        view_formats: &[],
    });
    let view = texture.create_view(&wgpu::TextureViewDescriptor::default());
    let sampler = device.create_sampler(&wgpu::SamplerDescriptor {
        label: Some("Spellwire overlay sampler"),
        mag_filter: wgpu::FilterMode::Nearest,
        min_filter: wgpu::FilterMode::Nearest,
        ..wgpu::SamplerDescriptor::default()
    });
    // Window dimensions are bounded far below f32's exact-integer range on supported desktops.
    #[allow(clippy::cast_precision_loss)]
    let uv_scale = UvScale { value: [width as f32 / padded_width as f32, 1.0], padding: [0.0; 2] };
    let uniform = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
        label: Some("Spellwire overlay UV scale"),
        contents: bytemuck::bytes_of(&uv_scale),
        usage: wgpu::BufferUsages::UNIFORM,
    });
    let bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
        label: Some("Spellwire overlay bind group"),
        layout,
        entries: &[
            wgpu::BindGroupEntry {
                binding: 0,
                resource: wgpu::BindingResource::TextureView(&view),
            },
            wgpu::BindGroupEntry { binding: 1, resource: wgpu::BindingResource::Sampler(&sampler) },
            wgpu::BindGroupEntry { binding: 2, resource: uniform.as_entire_binding() },
        ],
    });
    (texture, bind_group, padded_width, Vec::new())
}

fn draw_node(
    pixmap: &mut PixmapMut<'_>,
    fonts: &FontBook,
    node: &OverlayNode,
    offset_x: f32,
    offset_y: f32,
) {
    match node {
        OverlayNode::Rect {
            x,
            y,
            width,
            height,
            radius,
            color,
            fill,
            stroke,
            shadow,
            opacity,
            ..
        } => {
            if let Some(shadow) = shadow {
                draw_rect_shadow(
                    pixmap,
                    *x - offset_x,
                    *y - offset_y,
                    *width,
                    *height,
                    *radius,
                    shadow,
                    *opacity,
                );
            }
            let Some(path) = rounded_rect(*x - offset_x, *y - offset_y, *width, *height, *radius)
            else {
                return;
            };
            let paint = Color::parse(fill.as_deref().or(color.as_deref()), Color::PANEL)
                .with_opacity(*opacity)
                .paint();
            pixmap.fill_path(&path, &paint, FillRule::Winding, Transform::identity(), None);
            draw_stroke(pixmap, &path, stroke.as_ref(), *opacity);
        }
        OverlayNode::Ellipse {
            x, y, width, height, color, fill, stroke, shadow, opacity, ..
        } => {
            if let Some(shadow) = shadow {
                draw_ellipse_shadow(
                    pixmap,
                    *x - offset_x,
                    *y - offset_y,
                    *width,
                    *height,
                    shadow,
                    *opacity,
                );
            }
            let Some(path) = ellipse_path(*x - offset_x, *y - offset_y, *width, *height) else {
                return;
            };
            let paint = Color::parse(fill.as_deref().or(color.as_deref()), Color::WHITE)
                .with_opacity(*opacity)
                .paint();
            pixmap.fill_path(&path, &paint, FillRule::Winding, Transform::identity(), None);
            draw_stroke(pixmap, &path, stroke.as_ref(), *opacity);
        }
        OverlayNode::Line { x1, y1, x2, y2, width, color, fill, opacity, .. } => {
            if *width <= 0.0 {
                return;
            }
            let mut builder = PathBuilder::new();
            builder.move_to(*x1 - offset_x, *y1 - offset_y);
            builder.line_to(*x2 - offset_x, *y2 - offset_y);
            let Some(path) = builder.finish() else { return };
            let paint = Color::parse(fill.as_deref().or(color.as_deref()), Color::WHITE)
                .with_opacity(*opacity)
                .paint();
            pixmap.stroke_path(
                &path,
                &paint,
                &Stroke { width: *width, ..Stroke::default() },
                Transform::identity(),
                None,
            );
        }
        OverlayNode::Text {
            x, y, width, height, text, size, color, fill, opacity, font, ..
        } => {
            if let Some(selected) = fonts.select(font.as_ref()) {
                draw_text(
                    pixmap,
                    selected,
                    *x - offset_x,
                    *y - offset_y,
                    *width,
                    *height,
                    text,
                    *size,
                    font.as_ref(),
                    Color::parse(fill.as_deref().or(color.as_deref()), Color::WHITE)
                        .with_opacity(*opacity),
                );
            }
        }
    }
}

fn draw_stroke(
    pixmap: &mut PixmapMut<'_>,
    path: &Path,
    stroke: Option<&StrokeStyle>,
    opacity: f32,
) {
    let Some(stroke) = stroke.filter(|stroke| stroke.width > 0.0) else { return };
    let paint = Color::parse(Some(&stroke.fill), Color::WHITE).with_opacity(opacity).paint();
    pixmap.stroke_path(
        path,
        &paint,
        &Stroke { width: stroke.width, ..Stroke::default() },
        Transform::identity(),
        None,
    );
}

#[allow(clippy::cast_precision_loss, clippy::too_many_arguments)]
fn draw_rect_shadow(
    pixmap: &mut PixmapMut<'_>,
    x: f32,
    y: f32,
    width: f32,
    height: f32,
    radius: f32,
    shadow: &ShadowStyle,
    opacity: f32,
) {
    let blur = shadow.blur.clamp(0.0, 64.0);
    let layers = rounded_i32(blur).clamp(1, 24);
    for layer in (1..=layers).rev() {
        let progress = layer as f32 / layers as f32;
        let expansion = shadow.spread + blur * progress;
        let Some(path) = rounded_rect(
            x + shadow.x - expansion,
            y + shadow.y - expansion,
            width + expansion * 2.0,
            height + expansion * 2.0,
            radius + expansion,
        ) else {
            continue;
        };
        let layer_opacity = opacity * (1.0 - progress * 0.72) / layers as f32;
        let paint =
            Color::parse(Some(&shadow.fill), Color::PANEL).with_opacity(layer_opacity).paint();
        pixmap.fill_path(&path, &paint, FillRule::Winding, Transform::identity(), None);
    }
}

#[allow(clippy::cast_precision_loss)]
fn draw_ellipse_shadow(
    pixmap: &mut PixmapMut<'_>,
    x: f32,
    y: f32,
    width: f32,
    height: f32,
    shadow: &ShadowStyle,
    opacity: f32,
) {
    let blur = shadow.blur.clamp(0.0, 64.0);
    let layers = rounded_i32(blur).clamp(1, 24);
    for layer in (1..=layers).rev() {
        let progress = layer as f32 / layers as f32;
        let expansion = shadow.spread + blur * progress;
        let Some(path) = ellipse_path(
            x + shadow.x - expansion,
            y + shadow.y - expansion,
            width + expansion * 2.0,
            height + expansion * 2.0,
        ) else {
            continue;
        };
        let layer_opacity = opacity * (1.0 - progress * 0.72) / layers as f32;
        let paint =
            Color::parse(Some(&shadow.fill), Color::PANEL).with_opacity(layer_opacity).paint();
        pixmap.fill_path(&path, &paint, FillRule::Winding, Transform::identity(), None);
    }
}

fn ellipse_path(x: f32, y: f32, width: f32, height: f32) -> Option<Path> {
    PathBuilder::from_oval(Rect::from_xywh(x, y, width, height)?)
}

fn rounded_rect(x: f32, y: f32, width: f32, height: f32, radius: f32) -> Option<tiny_skia::Path> {
    if width <= 0.0 || height <= 0.0 {
        return None;
    }
    let radius = radius.max(0.0).min(width / 2.0).min(height / 2.0);
    let mut path = PathBuilder::new();
    path.move_to(x + radius, y);
    path.line_to(x + width - radius, y);
    path.quad_to(x + width, y, x + width, y + radius);
    path.line_to(x + width, y + height - radius);
    path.quad_to(x + width, y + height, x + width - radius, y + height);
    path.line_to(x + radius, y + height);
    path.quad_to(x, y + height, x, y + height - radius);
    path.line_to(x, y + radius);
    path.quad_to(x, y, x + radius, y);
    path.close();
    path.finish()
}

#[allow(clippy::too_many_arguments)]
fn draw_text(
    pixmap: &mut PixmapMut<'_>,
    font: &Font,
    x: f32,
    y: f32,
    width: Option<f32>,
    height: Option<f32>,
    text: &str,
    size: f32,
    style: Option<&FontStyle>,
    color: Color,
) {
    if size <= 0.0 {
        return;
    }
    let letter_spacing = style.and_then(|style| style.letter_spacing).unwrap_or(0.0);
    let line_height = style.and_then(|style| style.line_height).unwrap_or(size * 1.2).max(0.0);
    let text_width = text
        .chars()
        .map(|character| font.metrics(character, size).advance_width + letter_spacing)
        .sum::<f32>()
        - if text.is_empty() { 0.0 } else { letter_spacing };
    let available_width = width.unwrap_or(text_width).max(0.0);
    let align = style.and_then(|style| style.align.as_deref()).unwrap_or("left");
    let mut pen_x = match align {
        "center" => x + (available_width - text_width) / 2.0,
        "right" => x + available_width - text_width,
        _ => x,
    };
    let available_height = height.unwrap_or(line_height).max(0.0);
    let top = y + (available_height - line_height) / 2.0 + (line_height - size) / 2.0;
    for character in text.chars() {
        let (metrics, bitmap) = font.rasterize(character, size);
        let glyph_x = rounded_i32(pen_x).saturating_add(metrics.xmin);
        let glyph_height = i32::try_from(metrics.height).unwrap_or(i32::MAX);
        let glyph_y =
            rounded_i32(top).saturating_add(rounded_i32(size).saturating_sub(glyph_height));
        blend_glyph(pixmap, glyph_x, glyph_y, metrics.width, metrics.height, &bitmap, color);
        pen_x += metrics.advance_width + letter_spacing;
    }
}

#[allow(clippy::cast_possible_truncation, clippy::cast_precision_loss)]
fn rounded_i32(value: f32) -> i32 {
    if !value.is_finite() {
        return 0;
    }
    // Bounds are checked before the intentional conversion from raster coordinates.
    value.round().clamp(i32::MIN as f32, i32::MAX as f32) as i32
}

#[allow(clippy::too_many_arguments)]
fn blend_glyph(
    pixmap: &mut PixmapMut<'_>,
    x: i32,
    y: i32,
    width: usize,
    height: usize,
    bitmap: &[u8],
    color: Color,
) {
    let target_width = i32::try_from(pixmap.width()).unwrap_or(i32::MAX);
    let target_height = i32::try_from(pixmap.height()).unwrap_or(i32::MAX);
    let data = pixmap.data_mut();
    for row in 0..height {
        for column in 0..width {
            let target_x = x + i32::try_from(column).unwrap_or_default();
            let target_y = y + i32::try_from(row).unwrap_or_default();
            if target_x < 0 || target_y < 0 || target_x >= target_width || target_y >= target_height
            {
                continue;
            }
            let coverage = u16::from(bitmap[row * width + column]);
            let alpha = u16::from(color.alpha) * coverage / 255;
            let inverse = 255 - alpha;
            let offset = (usize::try_from(target_y).unwrap_or_default()
                * usize::try_from(target_width).unwrap_or_default()
                + usize::try_from(target_x).unwrap_or_default())
                * 4;
            let source = [color.red, color.green, color.blue];
            for channel in 0..3 {
                let premultiplied = u16::from(source[channel]) * alpha / 255;
                data[offset + channel] =
                    u8::try_from(premultiplied + u16::from(data[offset + channel]) * inverse / 255)
                        .unwrap_or(255);
            }
            data[offset + 3] =
                u8::try_from(alpha + u16::from(data[offset + 3]) * inverse / 255).unwrap_or(255);
        }
    }
}

fn load_fonts() -> FontBook {
    #[cfg(target_os = "macos")]
    const SYSTEM: &[&str] =
        &["/System/Library/Fonts/SFNS.ttf", "/System/Library/Fonts/Supplemental/Arial.ttf"];
    #[cfg(target_os = "macos")]
    const SYSTEM_BOLD: &[&str] =
        &["/System/Library/Fonts/SFNS.ttf", "/System/Library/Fonts/Supplemental/Arial Bold.ttf"];
    #[cfg(target_os = "macos")]
    const MONOSPACE: &[&str] =
        &["/System/Library/Fonts/SFNSMono.ttf", "/System/Library/Fonts/Monaco.ttf"];
    #[cfg(target_os = "macos")]
    const MONOSPACE_BOLD: &[&str] =
        &["/System/Library/Fonts/SFNSMono.ttf", "/System/Library/Fonts/Monaco.ttf"];
    #[cfg(target_os = "windows")]
    const SYSTEM: &[&str] = &[r"C:\Windows\Fonts\segoeui.ttf", r"C:\Windows\Fonts\arial.ttf"];
    #[cfg(target_os = "windows")]
    const SYSTEM_BOLD: &[&str] =
        &[r"C:\Windows\Fonts\segoeuib.ttf", r"C:\Windows\Fonts\arialbd.ttf"];
    #[cfg(target_os = "windows")]
    const MONOSPACE: &[&str] = &[r"C:\Windows\Fonts\consola.ttf", r"C:\Windows\Fonts\cour.ttf"];
    #[cfg(target_os = "windows")]
    const MONOSPACE_BOLD: &[&str] =
        &[r"C:\Windows\Fonts\consolab.ttf", r"C:\Windows\Fonts\courbd.ttf"];
    #[cfg(target_os = "linux")]
    const SYSTEM: &[&str] = &[
        "/usr/share/fonts/truetype/dejavu/DejaVuSans.ttf",
        "/usr/share/fonts/truetype/noto/NotoSans-Regular.ttf",
    ];
    #[cfg(target_os = "linux")]
    const SYSTEM_BOLD: &[&str] = &[
        "/usr/share/fonts/truetype/dejavu/DejaVuSans-Bold.ttf",
        "/usr/share/fonts/truetype/noto/NotoSans-Bold.ttf",
    ];
    #[cfg(target_os = "linux")]
    const MONOSPACE: &[&str] = &["/usr/share/fonts/truetype/dejavu/DejaVuSansMono.ttf"];
    #[cfg(target_os = "linux")]
    const MONOSPACE_BOLD: &[&str] = &["/usr/share/fonts/truetype/dejavu/DejaVuSansMono-Bold.ttf"];
    FontBook {
        system: load_first_font(SYSTEM),
        system_bold: load_first_font(SYSTEM_BOLD),
        monospace: load_first_font(MONOSPACE),
        monospace_bold: load_first_font(MONOSPACE_BOLD),
    }
}

fn load_first_font(candidates: &[&str]) -> Option<Font> {
    candidates.iter().find_map(|path| {
        let bytes = fs::read(path).ok()?;
        Font::from_bytes(bytes, FontSettings::default()).ok()
    })
}

fn reader_thread(proxy: &EventLoopProxy<Command>) {
    for line in io::stdin().lock().lines() {
        let Ok(line) = line else { break };
        if line.trim().is_empty() {
            continue;
        }
        match serde_json::from_str::<Command>(&line) {
            Ok(command) => {
                let exit = matches!(command, Command::Exit);
                if proxy.send_event(command).is_err() || exit {
                    return;
                }
            }
            Err(error) => eprintln!("invalid overlay command: {error}"),
        }
    }
    let _ = proxy.send_event(Command::Exit);
}

#[allow(clippy::too_many_lines)]
fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut event_loop_builder = EventLoopBuilder::<Command>::with_user_event();
    #[cfg(target_os = "macos")]
    event_loop_builder
        .with_activation_policy(ActivationPolicy::Accessory)
        .with_default_menu(false)
        .with_activate_ignoring_other_apps(false);
    let event_loop = event_loop_builder.build()?;
    let monitor = event_loop.primary_monitor();
    let size =
        monitor.as_ref().map_or(PhysicalSize::new(1280, 720), winit::monitor::MonitorHandle::size);
    let position = monitor
        .as_ref()
        .map_or(PhysicalPosition::new(0, 0), winit::monitor::MonitorHandle::position);
    let window = Arc::new(
        WindowBuilder::new()
            .with_title("Spellwire Overlay")
            .with_inner_size(size)
            .with_position(position)
            .with_decorations(false)
            .with_resizable(false)
            .with_transparent(true)
            .with_window_level(WindowLevel::AlwaysOnTop)
            .with_visible(false)
            .build(&event_loop)?,
    );
    window.set_cursor_hittest(false)?;
    let mut renderer = Renderer::new(Arc::clone(&window), size).map_err(io::Error::other)?;
    let mut nodes: BTreeMap<u32, OverlayNode> = BTreeMap::new();
    let proxy = event_loop.create_proxy();
    thread::Builder::new()
        .name("spellwire-overlay-control".into())
        .spawn(move || reader_thread(&proxy))?;
    if std::env::args().any(|argument| argument == "--smoke") {
        let proxy = event_loop.create_proxy();
        thread::spawn(move || {
            thread::sleep(Duration::from_millis(350));
            let _ = proxy.send_event(Command::Exit);
        });
    }
    window.set_visible(true);
    println!(
        "{{\"event\":\"ready\",\"width\":{},\"height\":{},\"scaleFactor\":{},\"alphaMode\":\"{:?}\"}}",
        size.width,
        size.height,
        window.scale_factor(),
        renderer.config.alpha_mode
    );
    let window_id = window.id();
    let mut dirty = Some(renderer.full_bounds());
    window.request_redraw();
    event_loop.run(move |event, target| {
        target.set_control_flow(ControlFlow::Wait);
        match event {
            Event::UserEvent(command) => {
                let redraw = match command {
                    Command::Batch { mutations } => {
                        let mut changed = None;
                        for mutation in mutations {
                            if mutation.remove {
                                if let Some(previous) = nodes.remove(&mutation.id) {
                                    union_bounds(&mut changed, previous.bounds());
                                }
                            } else if let Some(node) = mutation.node {
                                if let Some(previous) = nodes.get(&mutation.id) {
                                    union_bounds(&mut changed, previous.bounds());
                                }
                                union_bounds(&mut changed, node.bounds());
                                nodes.insert(mutation.id, node);
                            }
                        }
                        if let Some(changed) = changed {
                            union_bounds(&mut dirty, changed);
                            true
                        } else {
                            false
                        }
                    }
                    Command::Clear => {
                        let changed = !nodes.is_empty();
                        nodes.clear();
                        if changed {
                            dirty = Some(renderer.full_bounds());
                        }
                        changed
                    }
                    Command::Show => {
                        window.set_visible(true);
                        false
                    }
                    Command::Hide => {
                        window.set_visible(false);
                        false
                    }
                    Command::Exit => {
                        target.exit();
                        false
                    }
                };
                if redraw {
                    window.request_redraw();
                }
            }
            Event::WindowEvent { window_id: event_window, event } if event_window == window_id => {
                match event {
                    WindowEvent::Resized(size) => {
                        renderer.resize(size);
                        dirty = Some(renderer.full_bounds());
                        window.request_redraw();
                    }
                    WindowEvent::RedrawRequested => {
                        if let Some(region) = dirty.take() {
                            if let Err(error) = renderer.render(&nodes, region) {
                                eprintln!("overlay render failed: {error}");
                                dirty = Some(renderer.full_bounds());
                            }
                        }
                    }
                    WindowEvent::CloseRequested => target.exit(),
                    _ => {}
                }
            }
            _ => {}
        }
    })?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_colors_and_rounds_rectangles() {
        let color = Color::parse(Some("#11223344"), Color::WHITE);
        assert_eq!((color.red, color.green, color.blue, color.alpha), (17, 34, 51, 68));
        assert!(rounded_rect(1.0, 2.0, 30.0, 20.0, 100.0).is_some());
        assert!(rounded_rect(0.0, 0.0, -1.0, 10.0, 0.0).is_none());
    }

    #[test]
    fn command_protocol_accepts_overlay_scene_nodes() {
        let command: Command = serde_json::from_str(
            r##"{"op":"batch","mutations":[{"id":1,"node":{"kind":"text","x":4,"y":8,"text":"ok","size":16,"fill":"#ffffff","font":{"family":"system","weight":600}}}]}"##,
        )
        .unwrap();
        assert!(matches!(command, Command::Batch { mutations } if mutations.len() == 1));
    }

    #[test]
    fn dirty_regions_align_uploads_and_include_effects() {
        let region =
            PixelRegion::from_bounds(Bounds::new(70.0, 11.0, 20.0, 13.0), 1920, 1920, 1080)
                .unwrap();
        assert_eq!((region.x, region.y, region.width, region.height), (64, 11, 64, 13));

        let bounds = shape_bounds(
            100.0,
            100.0,
            80.0,
            40.0,
            Some(&StrokeStyle { fill: "#ffffff".into(), width: 2.0 }),
            Some(&ShadowStyle { fill: "#000000".into(), x: 0.0, y: 8.0, blur: 16.0, spread: 0.0 }),
        );
        assert!(bounds.left <= 84.0 && bounds.bottom >= 164.0);

        let text = OverlayNode::Text {
            x: 0.0,
            y: 0.0,
            width: Some(4.0),
            height: Some(16.0),
            text: "WW".into(),
            size: 16.0,
            color: None,
            fill: None,
            opacity: 1.0,
            font: None,
            z: 0,
        };
        assert!(text.bounds().right >= 40.0);
    }
}
