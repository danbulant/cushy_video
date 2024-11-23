use std::{
    collections::{btree_map::Entry, BTreeMap},
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc, Mutex,
    },
};

use cushy::{
    figures::{units::UPx, Rect},
    kludgine::{self, wgpu},
    RenderOperation,
};

#[repr(C)]
struct Uniforms {
    rect: [f32; 4],
}

struct VideoEntry {
    texture_y: wgpu::Texture,
    texture_uv: wgpu::Texture,
    uniforms: wgpu::Buffer,
    bg0: wgpu::BindGroup,
    alive: Arc<AtomicBool>,
}

struct VideoPipeline {
    pipeline: wgpu::RenderPipeline,
    bg0_layout: wgpu::BindGroupLayout,
    sampler: wgpu::Sampler,
    videos: BTreeMap<u64, VideoEntry>,
}

impl VideoPipeline {
    fn new(graphics: &mut kludgine::Graphics<'_>) -> Self {
        let device = graphics.device();
        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("iced_video_player shader"),
            source: wgpu::ShaderSource::Wgsl(include_str!("shader.wgsl").into()),
        });

        let bg0_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("iced_video_player bind group 0 layout"),
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
                    ty: wgpu::BindingType::Texture {
                        sample_type: wgpu::TextureSampleType::Float { filterable: true },
                        view_dimension: wgpu::TextureViewDimension::D2,
                        multisampled: false,
                    },
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 2,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 3,
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

        let layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("iced_video_player pipeline layout"),
            bind_group_layouts: &[&bg0_layout],
            push_constant_ranges: &[],
        });

        let pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("iced_video_player pipeline"),
            layout: Some(&layout),
            vertex: wgpu::VertexState {
                module: &shader,
                entry_point: Some("vs_main"),
                buffers: &[],
                compilation_options: Default::default(),
            },
            primitive: wgpu::PrimitiveState::default(),
            depth_stencil: None,
            multisample: graphics.multisample_state(),
            fragment: Some(wgpu::FragmentState {
                module: &shader,
                entry_point: Some("fs_main"),
                targets: &[Some(wgpu::ColorTargetState {
                    format: graphics.texture_format(),
                    blend: None,
                    write_mask: wgpu::ColorWrites::ALL,
                })],
                compilation_options: Default::default(),
            }),
            multiview: None,
            cache: None,
        });

        let sampler = device.create_sampler(&wgpu::SamplerDescriptor {
            label: Some("iced_video_player sampler"),
            address_mode_u: wgpu::AddressMode::ClampToEdge,
            address_mode_v: wgpu::AddressMode::ClampToEdge,
            address_mode_w: wgpu::AddressMode::ClampToEdge,
            mag_filter: wgpu::FilterMode::Linear,
            min_filter: wgpu::FilterMode::Linear,
            mipmap_filter: wgpu::FilterMode::Nearest,
            lod_min_clamp: 0.0,
            lod_max_clamp: 1.0,
            compare: None,
            anisotropy_clamp: 1,
            border_color: None,
        });

        VideoPipeline {
            pipeline,
            bg0_layout,
            sampler,
            videos: BTreeMap::new(),
        }
    }

    fn upload(
        &mut self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        video_id: u64,
        alive: &Arc<AtomicBool>,
        (width, height): (u32, u32),
        frame: &[u8],
    ) {
        if let Entry::Vacant(entry) = self.videos.entry(video_id) {
            let texture_y = device.create_texture(&wgpu::TextureDescriptor {
                label: Some("iced_video_player texture"),
                size: wgpu::Extent3d {
                    width,
                    height,
                    depth_or_array_layers: 1,
                },
                mip_level_count: 1,
                sample_count: 1,
                dimension: wgpu::TextureDimension::D2,
                format: wgpu::TextureFormat::R8Unorm,
                usage: wgpu::TextureUsages::COPY_DST | wgpu::TextureUsages::TEXTURE_BINDING,
                view_formats: &[],
            });

            let texture_uv = device.create_texture(&wgpu::TextureDescriptor {
                label: Some("iced_video_player texture"),
                size: wgpu::Extent3d {
                    width: width / 2,
                    height: height / 2,
                    depth_or_array_layers: 1,
                },
                mip_level_count: 1,
                sample_count: 1,
                dimension: wgpu::TextureDimension::D2,
                format: wgpu::TextureFormat::Rg8Unorm,
                usage: wgpu::TextureUsages::COPY_DST | wgpu::TextureUsages::TEXTURE_BINDING,
                view_formats: &[],
            });

            let view_y = texture_y.create_view(&wgpu::TextureViewDescriptor {
                label: Some("iced_video_player texture view"),
                format: None,
                dimension: None,
                aspect: wgpu::TextureAspect::All,
                base_mip_level: 0,
                mip_level_count: None,
                base_array_layer: 0,
                array_layer_count: None,
            });

            let view_uv = texture_uv.create_view(&wgpu::TextureViewDescriptor {
                label: Some("iced_video_player texture view"),
                format: None,
                dimension: None,
                aspect: wgpu::TextureAspect::All,
                base_mip_level: 0,
                mip_level_count: None,
                base_array_layer: 0,
                array_layer_count: None,
            });

            let buffer = device.create_buffer(&wgpu::BufferDescriptor {
                label: Some("iced_video_player uniform buffer"),
                size: std::mem::size_of::<Uniforms>() as _,
                usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::UNIFORM,
                mapped_at_creation: false,
            });

            let bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
                label: Some("iced_video_player bind group"),
                layout: &self.bg0_layout,
                entries: &[
                    wgpu::BindGroupEntry {
                        binding: 0,
                        resource: wgpu::BindingResource::TextureView(&view_y),
                    },
                    wgpu::BindGroupEntry {
                        binding: 1,
                        resource: wgpu::BindingResource::TextureView(&view_uv),
                    },
                    wgpu::BindGroupEntry {
                        binding: 2,
                        resource: wgpu::BindingResource::Sampler(&self.sampler),
                    },
                    wgpu::BindGroupEntry {
                        binding: 3,
                        resource: wgpu::BindingResource::Buffer(wgpu::BufferBinding {
                            buffer: &buffer,
                            offset: 0,
                            size: None,
                        }),
                    },
                ],
            });

            entry.insert(VideoEntry {
                texture_y,
                texture_uv,
                uniforms: buffer,
                bg0: bind_group,
                alive: Arc::clone(alive),
            });
        }

        let VideoEntry {
            texture_y,
            texture_uv,
            ..
        } = self.videos.get(&video_id).unwrap();

        queue.write_texture(
            wgpu::ImageCopyTexture {
                texture: texture_y,
                mip_level: 0,
                origin: wgpu::Origin3d::ZERO,
                aspect: wgpu::TextureAspect::All,
            },
            &frame[..(width * height) as usize],
            wgpu::ImageDataLayout {
                offset: 0,
                bytes_per_row: Some(width),
                rows_per_image: Some(height),
            },
            wgpu::Extent3d {
                width,
                height,
                depth_or_array_layers: 1,
            },
        );

        queue.write_texture(
            wgpu::ImageCopyTexture {
                texture: texture_uv,
                mip_level: 0,
                origin: wgpu::Origin3d::ZERO,
                aspect: wgpu::TextureAspect::All,
            },
            &frame[(width * height) as usize..],
            wgpu::ImageDataLayout {
                offset: 0,
                bytes_per_row: Some(width),
                rows_per_image: Some(height / 2),
            },
            wgpu::Extent3d {
                width: width / 2,
                height: height / 2,
                depth_or_array_layers: 1,
            },
        );
    }

    fn cleanup(&mut self) {
        let ids: Vec<_> = self
            .videos
            .iter()
            .filter_map(|(id, entry)| (!entry.alive.load(Ordering::SeqCst)).then_some(*id))
            .collect();
        for id in ids {
            if let Some(video) = self.videos.remove(&id) {
                video.texture_y.destroy();
                video.texture_uv.destroy();
                video.uniforms.destroy();
            }
        }
    }

    fn prepare(&mut self, queue: &wgpu::Queue, video_id: u64, bounds: Rect<UPx>) {
        if let Some(video) = self.videos.get(&video_id) {
            let uniforms = Uniforms {
                rect: [
                    bounds.origin.x.into(),
                    bounds.origin.y.into(),
                    (bounds.origin.x + bounds.size.width).into(),
                    (bounds.origin.y + bounds.size.height).into(),
                ],
            };
            queue.write_buffer(&video.uniforms, 0, unsafe {
                std::slice::from_raw_parts(
                    &uniforms as *const _ as *const u8,
                    std::mem::size_of::<Uniforms>(),
                )
            });
        }

        self.cleanup();
    }

    fn draw(&self, pass: &mut wgpu::RenderPass, viewport: Rect<UPx>, video_id: u64) {
        if let Some(video) = self.videos.get(&video_id) {
            // let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
            //     label: Some("iced_video_player render pass"),
            //     color_attachments: &[Some(wgpu::RenderPassColorAttachment {
            //         view: target,
            //         resolve_target: None,
            //         ops: wgpu::Operations {
            //             load: wgpu::LoadOp::Load,
            //             store: wgpu::StoreOp::Store,
            //         },
            //     })],
            //     depth_stencil_attachment: None,
            //     timestamp_writes: None,
            //     occlusion_query_set: None,
            // });

            pass.set_pipeline(&self.pipeline);
            pass.set_bind_group(0, &video.bg0, &[]);
            // pass.set_viewport(
            //     viewport.origin.x as _,
            //     viewport.origin.y as _,
            //     viewport.size.width as _,
            //     viewport.size.height as _,
            //     0.0,
            //     1.0,
            // );
            pass.draw(0..4, 0..1);
        }
    }
}

pub(crate) struct VideoRO {
    pipeline: Option<VideoPipeline>,
}

impl RenderOperation for VideoRO {
    type DrawInfo = VideoPrimitive;
    type Prepared = VideoPrimitive;

    fn new(graphics: &mut cushy::kludgine::Graphics<'_>) -> Self {
        VideoRO { pipeline: None }
    }

    fn prepare(
        &mut self,
        context: Self::DrawInfo,
        origin: cushy::figures::Point<cushy::figures::units::Px>,
        graphics: &mut cushy::kludgine::Graphics<'_>,
    ) -> Self::Prepared {
        let pipeline = self
            .pipeline
            .get_or_insert_with(|| VideoPipeline::new(graphics));

        if context.upload_frame {
            pipeline.upload(
                graphics.device(),
                graphics.queue(),
                context.video_id,
                &context.alive,
                context.size,
                context.frame.lock().expect("lock frame mutex").as_slice(),
            );
        }

        pipeline.prepare(graphics.queue(), context.video_id, graphics.clip_rect());
        context
    }

    fn render(
        &self,
        prepared: &Self::Prepared,
        origin: cushy::figures::Point<cushy::figures::units::Px>,
        opacity: f32,
        graphics: &mut cushy::kludgine::RenderingGraphics<'_, '_>,
    ) {
        let pipeline = self.pipeline.as_ref().expect("prepare sets pipeline");
        let rect = graphics.clip_rect();
        pipeline.draw(
            // target,
            graphics.pass_mut(),
            rect,
            prepared.video_id,
        );
    }
}

#[derive(Debug, Clone)]
pub(crate) struct VideoPrimitive {
    video_id: u64,
    alive: Arc<AtomicBool>,
    frame: Arc<Mutex<Vec<u8>>>,
    size: (u32, u32),
    upload_frame: bool,
}

impl VideoPrimitive {
    pub fn new(
        video_id: u64,
        alive: Arc<AtomicBool>,
        frame: Arc<Mutex<Vec<u8>>>,
        size: (u32, u32),
        upload_frame: bool,
    ) -> Self {
        VideoPrimitive {
            video_id,
            alive,
            frame,
            size,
            upload_frame,
        }
    }
}
