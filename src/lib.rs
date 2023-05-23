use bytemuck::{cast_slice, Pod, Zeroable};
use framebuffer::{FrameBuffer, Pixel};
use raw_window_handle::{HasRawDisplayHandle, HasRawWindowHandle};
use std::{iter::once, mem::size_of};
use wgpu::{
    include_wgsl, Adapter, AddressMode, BindGroup, BindGroupDescriptor, BindGroupEntry,
    BindGroupLayout, BindGroupLayoutDescriptor, BindGroupLayoutEntry, BindingResource, BindingType,
    BlendState, Buffer, BufferDescriptor, BufferUsages, Color, ColorTargetState, ColorWrites,
    CompositeAlphaMode, CreateSurfaceError, Device, Extent3d, Face, FilterMode, FragmentState,
    FrontFace, ImageDataLayout, IndexFormat, Instance, LoadOp, MultisampleState, Operations,
    PipelineLayoutDescriptor, PolygonMode, PresentMode, PrimitiveState, PrimitiveTopology, Queue,
    RenderPassColorAttachment, RenderPassDescriptor, RenderPipeline, RenderPipelineDescriptor,
    Sampler, SamplerBindingType, SamplerDescriptor, ShaderStages, Surface, SurfaceConfiguration,
    SurfaceError, Texture, TextureDescriptor, TextureDimension, TextureFormat, TextureSampleType,
    TextureUsages, TextureViewDimension, VertexAttribute, VertexBufferLayout, VertexFormat,
    VertexState, VertexStepMode,
};

pub mod framebuffer;

const FRAMEBUFFER_TEXTURE_FORMAT: TextureFormat = TextureFormat::Rgba8UnormSrgb;

pub struct Pixely {
    framebuffer: FrameBuffer,
    framebuffer_changed: bool,

    surface: Surface,
    config: SurfaceConfiguration,
    surface_changed: bool,

    pipeline: RenderPipeline,
    texture: Option<Texture>,
    sampler: Sampler,
    bind_group_layout: BindGroupLayout,
    bind_group: Option<BindGroup>,
    vertex_buffer: Buffer,
    vertices_changed: bool,
    index_buffer: Buffer,
}
impl Pixely {
    pub fn new<W: HasRawWindowHandle + HasRawDisplayHandle>(
        desc: PixelyDesc<W>,
    ) -> Result<Self, CreateSurfaceError> {
        let surface = unsafe { desc.instance.create_surface(desc.window.window) }?;
        let surface_format = TextureFormat::Bgra8UnormSrgb;
        let config = SurfaceConfiguration {
            usage: TextureUsages::RENDER_ATTACHMENT,
            format: surface_format,
            width: desc.window.width as u32,
            height: desc.window.height as u32,
            present_mode: PresentMode::Fifo,
            alpha_mode: CompositeAlphaMode::Opaque,
            view_formats: [surface_format].into(),
        };
        let framebuffer = FrameBuffer::new(desc.buffer.width, desc.buffer.height);

        let shader_src = include_wgsl!("shader.wgsl");
        let shader_mod = desc.device.create_shader_module(shader_src);
        let bind_group_layout = desc
            .device
            .create_bind_group_layout(&BindGroupLayoutDescriptor {
                label: None,
                entries: &[
                    BindGroupLayoutEntry {
                        binding: 0,
                        visibility: ShaderStages::FRAGMENT,
                        ty: BindingType::Texture {
                            sample_type: TextureSampleType::Float { filterable: true },
                            view_dimension: TextureViewDimension::D2,
                            multisampled: false,
                        },
                        count: None,
                    },
                    BindGroupLayoutEntry {
                        binding: 1,
                        visibility: ShaderStages::FRAGMENT,
                        ty: BindingType::Sampler(SamplerBindingType::Filtering),
                        count: None,
                    },
                ],
            });
        let pipeline_layout = desc
            .device
            .create_pipeline_layout(&PipelineLayoutDescriptor {
                label: None,
                bind_group_layouts: &[&bind_group_layout],
                push_constant_ranges: &[],
            });
        let pipeline = desc
            .device
            .create_render_pipeline(&RenderPipelineDescriptor {
                label: None,
                layout: Some(&pipeline_layout),
                vertex: VertexState {
                    module: &shader_mod,
                    entry_point: "vertex_main",
                    buffers: &[VertexBufferLayout {
                        array_stride: size_of::<Vertex>() as u64,
                        step_mode: VertexStepMode::Vertex,
                        attributes: &[
                            VertexAttribute {
                                format: VertexFormat::Float32x2,
                                offset: 0,
                                shader_location: 0,
                            },
                            VertexAttribute {
                                format: VertexFormat::Float32x2,
                                offset: 2 * size_of::<f32>() as u64,
                                shader_location: 1,
                            },
                        ],
                    }],
                },
                primitive: PrimitiveState {
                    topology: PrimitiveTopology::TriangleList,
                    strip_index_format: None,
                    front_face: FrontFace::Cw,
                    cull_mode: Some(Face::Back),
                    unclipped_depth: false,
                    polygon_mode: PolygonMode::Fill,
                    conservative: false,
                },
                depth_stencil: None,
                multisample: MultisampleState {
                    count: 1,
                    mask: !0,
                    alpha_to_coverage_enabled: false,
                },
                fragment: Some(FragmentState {
                    module: &shader_mod,
                    entry_point: "fragment_main",
                    targets: &[Some(ColorTargetState {
                        format: surface_format,
                        blend: Some(BlendState::REPLACE),
                        write_mask: ColorWrites::ALL,
                    })],
                }),
                multiview: None,
            });
        let sampler = desc.device.create_sampler(&SamplerDescriptor {
            label: None,
            address_mode_u: AddressMode::Repeat,
            address_mode_v: AddressMode::Repeat,
            address_mode_w: AddressMode::Repeat,
            mag_filter: FilterMode::Nearest,
            min_filter: FilterMode::Linear,
            mipmap_filter: FilterMode::Nearest,
            ..Default::default()
        });

        let indices = [0u16, 1, 2, 1, 3, 2];
        let index_buffer = desc.device.create_buffer(&BufferDescriptor {
            label: None,
            size: 6 * size_of::<u16>() as u64,
            usage: BufferUsages::COPY_DST | BufferUsages::INDEX,
            mapped_at_creation: false,
        });
        desc.queue
            .write_buffer(&index_buffer, 0, cast_slice(&indices));

        let vertex_buffer = desc.device.create_buffer(&BufferDescriptor {
            label: None,
            size: 4 * size_of::<Vertex>() as u64,
            usage: BufferUsages::COPY_DST | BufferUsages::VERTEX,
            mapped_at_creation: false,
        });

        Ok(Self {
            framebuffer,
            framebuffer_changed: true,
            surface,
            config,
            surface_changed: true,
            pipeline,
            texture: None,
            sampler,
            bind_group_layout,
            bind_group: None,
            vertex_buffer,
            vertices_changed: true,
            index_buffer,
        })
    }

    fn recreate_texture(&mut self, device: &Device) {
        let texture = device.create_texture(&TextureDescriptor {
            label: None,
            size: Extent3d {
                width: self.framebuffer.width() as u32,
                height: self.framebuffer.height() as u32,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: TextureDimension::D2,
            format: FRAMEBUFFER_TEXTURE_FORMAT,
            usage: TextureUsages::COPY_DST | TextureUsages::TEXTURE_BINDING,
            view_formats: &[FRAMEBUFFER_TEXTURE_FORMAT],
        });
        let view = texture.create_view(&Default::default());

        let bind_group = device.create_bind_group(&BindGroupDescriptor {
            label: None,
            layout: &self.bind_group_layout,
            entries: &[
                BindGroupEntry {
                    binding: 0,
                    resource: BindingResource::TextureView(&view),
                },
                BindGroupEntry {
                    binding: 1,
                    resource: BindingResource::Sampler(&self.sampler),
                },
            ],
        });

        self.texture = Some(texture);
        self.bind_group = Some(bind_group);
    }
    fn reconfigure_surface(&mut self, device: &Device) {
        self.surface.configure(device, &self.config);
        self.surface_changed = false;
    }
    fn upload_texture(&mut self, queue: &Queue) {
        let texture = self.texture.as_ref().unwrap();
        let image_copy = texture.as_image_copy();
        let layout = ImageDataLayout {
            offset: 0,
            bytes_per_row: Some((self.framebuffer.width() * size_of::<Pixel>()) as u32),
            rows_per_image: Some(self.framebuffer.height() as u32),
        };
        let size = Extent3d {
            width: self.framebuffer.width() as u32,
            height: self.framebuffer.height() as u32,
            depth_or_array_layers: 1,
        };
        queue.write_texture(image_copy, self.framebuffer.as_bytes(), layout, size);
        self.framebuffer_changed = false;
    }
    fn update_vertex_buffer(&mut self, queue: &Queue) {
        let (width, height) = self.get_quad_size();
        let vertices = [
            vertex([-width, -height], [0.0, 1.0]),
            vertex([-width, height], [0.0, 0.0]),
            vertex([width, -height], [1.0, 1.0]),
            vertex([width, height], [1.0, 0.0]),
        ];

        queue.write_buffer(&self.vertex_buffer, 0, cast_slice(&vertices));
        self.vertices_changed = false;
    }
    fn get_quad_size(&self) -> (f32, f32) {
        let frame_aspect = self.framebuffer.height() as f32 / self.framebuffer.width() as f32;
        let width = self.config.width as f32;
        let height = self.config.height as f32;
        let height_of_width = width * frame_aspect;
        let width_of_height = height / frame_aspect;

        if height_of_width <= height {
            (1.0, height_of_width / height)
        } else {
            (width_of_height / width, 1.0)
        }
    }

    pub fn buffer_mut(&mut self) -> &mut FrameBuffer {
        self.framebuffer_changed = true;
        &mut self.framebuffer
    }
    pub fn resize_framebuffer(&mut self, width: usize, height: usize) {
        self.texture = None;
        self.bind_group = None;
        self.vertices_changed = true;
        self.framebuffer_changed = true;
        self.framebuffer = FrameBuffer::new(width, height);
    }
    pub fn resize_surface(&mut self, width: usize, height: usize) {
        self.vertices_changed = true;
        self.surface_changed = true;
        self.config.width = width as u32;
        self.config.height = height as u32;
    }
    pub fn render(&mut self, device: &Device, queue: &Queue) -> Result<(), SurfaceError> {
        if self.config.width == 0 || self.config.height == 0 {
            return Ok(());
        }
        let texture_recreated = self.texture.is_none();
        if texture_recreated {
            self.recreate_texture(device);
        }
        if texture_recreated || self.framebuffer_changed {
            self.upload_texture(queue);
        }
        if self.surface_changed {
            self.reconfigure_surface(device);
        }
        if self.vertices_changed {
            self.update_vertex_buffer(queue);
        }

        let texture = self.surface.get_current_texture()?;
        let view = texture.texture.create_view(&Default::default());
        let mut cmd = device.create_command_encoder(&Default::default());
        let mut pass = cmd.begin_render_pass(&RenderPassDescriptor {
            label: None,
            color_attachments: &[Some(RenderPassColorAttachment {
                view: &view,
                resolve_target: None,
                ops: Operations {
                    store: true,
                    load: LoadOp::Clear(Color::BLACK),
                },
            })],
            depth_stencil_attachment: None,
        });
        pass.set_vertex_buffer(0, self.vertex_buffer.slice(..));
        pass.set_index_buffer(self.index_buffer.slice(..), IndexFormat::Uint16);
        pass.set_pipeline(&self.pipeline);
        pass.set_bind_group(0, self.bind_group.as_ref().unwrap(), &[]);
        pass.draw_indexed(0..6, 0, 0..1);

        drop(pass);
        queue.submit(once(cmd.finish()));
        texture.present();
        Ok(())
    }
}

pub struct PixelyDesc<'a, W> {
    pub window: WindowDesc<'a, W>,
    pub buffer: FrameBufferDesc,
    pub instance: &'a Instance,
    pub adapter: &'a Adapter,
    pub device: &'a Device,
    pub queue: &'a Queue,
}
pub struct WindowDesc<'a, W> {
    pub window: &'a W,
    pub width: usize,
    pub height: usize,
}
pub struct FrameBufferDesc {
    pub width: usize,
    pub height: usize,
}

#[derive(Copy, Clone, Debug, PartialEq)]
#[repr(C)]
struct Vertex {
    position: [f32; 2],
    tex_coord: [f32; 2],
}
unsafe impl Pod for Vertex {}
unsafe impl Zeroable for Vertex {}
fn vertex(position: [f32; 2], tex_coord: [f32; 2]) -> Vertex {
    Vertex {
        position,
        tex_coord,
    }
}
