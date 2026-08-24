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
use tiny_skia::{FillRule, Paint, PathBuilder, PixmapMut, Stroke, Transform};
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
    Upsert { id: u32, node: OverlayNode },
    Remove { id: u32 },
    Clear,
    Show,
    Hide,
    Exit,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(tag = "kind", rename_all = "camelCase")]
enum OverlayNode {
    Text {
        x: f32,
        y: f32,
        text: String,
        size: f32,
        #[serde(default)]
        color: Option<String>,
    },
    Rect {
        x: f32,
        y: f32,
        width: f32,
        height: f32,
        radius: f32,
        #[serde(default)]
        color: Option<String>,
    },
    Line {
        x1: f32,
        y1: f32,
        x2: f32,
        y2: f32,
        width: f32,
        #[serde(default)]
        color: Option<String>,
    },
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
}

#[repr(C)]
#[derive(Clone, Copy, Pod, Zeroable)]
struct UvScale {
    value: [f32; 2],
    padding: [f32; 2],
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
    frame: Vec<u8>,
    font: Option<Font>,
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
        let font = load_system_font();
        let (texture, bind_group, padded_width, frame) =
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
            frame,
            font,
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
        let (texture, bind_group, padded_width, frame) =
            create_frame_resources(&self.device, &bind_group_layout, self.width, self.height);
        self.texture = texture;
        self.bind_group = bind_group;
        self.padded_width = padded_width;
        self.frame = frame;
    }

    fn render(&mut self, nodes: &BTreeMap<u32, OverlayNode>) -> Result<(), String> {
        self.frame.fill(0);
        let mut pixmap = PixmapMut::from_bytes(&mut self.frame, self.padded_width, self.height)
            .ok_or_else(|| "overlay frame dimensions are invalid".to_owned())?;
        for node in nodes.values() {
            draw_node(&mut pixmap, self.font.as_ref(), node);
        }
        self.queue.write_texture(
            wgpu::ImageCopyTexture {
                texture: &self.texture,
                mip_level: 0,
                origin: wgpu::Origin3d::ZERO,
                aspect: wgpu::TextureAspect::All,
            },
            &self.frame,
            wgpu::ImageDataLayout {
                offset: 0,
                bytes_per_row: Some(self.padded_width * 4),
                rows_per_image: Some(self.height),
            },
            wgpu::Extent3d {
                width: self.padded_width,
                height: self.height,
                depth_or_array_layers: 1,
            },
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
    let frame_len = usize::try_from(padded_width)
        .unwrap_or_default()
        .saturating_mul(usize::try_from(height).unwrap_or_default())
        .saturating_mul(4);
    (texture, bind_group, padded_width, vec![0; frame_len])
}

fn draw_node(pixmap: &mut PixmapMut<'_>, font: Option<&Font>, node: &OverlayNode) {
    match node {
        OverlayNode::Rect { x, y, width, height, radius, color } => {
            let Some(path) = rounded_rect(*x, *y, *width, *height, *radius) else { return };
            let paint = Color::parse(color.as_deref(), Color::PANEL).paint();
            pixmap.fill_path(&path, &paint, FillRule::Winding, Transform::identity(), None);
        }
        OverlayNode::Line { x1, y1, x2, y2, width, color } => {
            if *width <= 0.0 {
                return;
            }
            let mut builder = PathBuilder::new();
            builder.move_to(*x1, *y1);
            builder.line_to(*x2, *y2);
            let Some(path) = builder.finish() else { return };
            let paint = Color::parse(color.as_deref(), Color::WHITE).paint();
            pixmap.stroke_path(
                &path,
                &paint,
                &Stroke { width: *width, ..Stroke::default() },
                Transform::identity(),
                None,
            );
        }
        OverlayNode::Text { x, y, text, size, color } => {
            if let Some(font) = font {
                draw_text(
                    pixmap,
                    font,
                    *x,
                    *y,
                    text,
                    *size,
                    Color::parse(color.as_deref(), Color::WHITE),
                );
            }
        }
    }
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

fn draw_text(
    pixmap: &mut PixmapMut<'_>,
    font: &Font,
    x: f32,
    y: f32,
    text: &str,
    size: f32,
    color: Color,
) {
    if size <= 0.0 {
        return;
    }
    let mut pen_x = x;
    for character in text.chars() {
        let (metrics, bitmap) = font.rasterize(character, size);
        let glyph_x = rounded_i32(pen_x).saturating_add(metrics.xmin);
        let glyph_height = i32::try_from(metrics.height).unwrap_or(i32::MAX);
        let glyph_y = rounded_i32(y).saturating_add(rounded_i32(size).saturating_sub(glyph_height));
        blend_glyph(pixmap, glyph_x, glyph_y, metrics.width, metrics.height, &bitmap, color);
        pen_x += metrics.advance_width;
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

fn load_system_font() -> Option<Font> {
    #[cfg(target_os = "macos")]
    const CANDIDATES: &[&str] =
        &["/System/Library/Fonts/SFNS.ttf", "/System/Library/Fonts/Supplemental/Arial.ttf"];
    #[cfg(target_os = "windows")]
    const CANDIDATES: &[&str] = &[r"C:\Windows\Fonts\segoeui.ttf", r"C:\Windows\Fonts\arial.ttf"];
    #[cfg(target_os = "linux")]
    const CANDIDATES: &[&str] = &[
        "/usr/share/fonts/truetype/dejavu/DejaVuSans.ttf",
        "/usr/share/fonts/truetype/noto/NotoSans-Regular.ttf",
    ];
    CANDIDATES.iter().find_map(|path| {
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
    let mut nodes = BTreeMap::new();
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
        "{{\"event\":\"ready\",\"width\":{},\"height\":{},\"alphaMode\":\"{:?}\"}}",
        size.width, size.height, renderer.config.alpha_mode
    );
    let window_id = window.id();
    event_loop.run(move |event, target| {
        target.set_control_flow(ControlFlow::Wait);
        match event {
            Event::UserEvent(command) => {
                let redraw = match command {
                    Command::Upsert { id, node } => {
                        nodes.insert(id, node);
                        true
                    }
                    Command::Remove { id } => nodes.remove(&id).is_some(),
                    Command::Clear => {
                        nodes.clear();
                        true
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
                        window.request_redraw();
                    }
                    WindowEvent::RedrawRequested => {
                        if let Err(error) = renderer.render(&nodes) {
                            eprintln!("overlay render failed: {error}");
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
            r##"{"op":"upsert","id":1,"node":{"kind":"text","x":4,"y":8,"text":"ok","size":16,"color":"#ffffff"}}"##,
        )
        .unwrap();
        assert!(matches!(command, Command::Upsert { id: 1, .. }));
    }
}
