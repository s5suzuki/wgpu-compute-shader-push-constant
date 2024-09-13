use bytemuck::NoUninit;
use rand::Rng;
use std::borrow::Cow;
use wgpu::util::DeviceExt;

#[derive(NoUninit, Clone, Copy)]
#[repr(C)]
struct Pc {
    offset: f32,
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let instance = wgpu::Instance::default();

    let adapter = instance
        .request_adapter(&wgpu::RequestAdapterOptions::default())
        .await
        .unwrap();

    let (device, queue) = adapter
        .request_device(
            &wgpu::DeviceDescriptor {
                label: None,
                required_features: wgpu::Features::PUSH_CONSTANTS, // required for push constants
                required_limits: wgpu::Limits {
                    max_push_constant_size: std::mem::size_of::<Pc>() as u32, // required for push constants
                    ..wgpu::Limits::downlevel_defaults()
                },
                memory_hints: wgpu::MemoryHints::MemoryUsage,
            },
            None,
        )
        .await?;

    let cs_module = device.create_shader_module(wgpu::ShaderModuleDescriptor {
        label: None,
        source: wgpu::ShaderSource::Wgsl(Cow::Borrowed(include_str!("shader.wgsl"))),
    });

    let mut rng = rand::thread_rng();

    let a = (0..10).map(|_| rng.gen()).collect::<Vec<f32>>();
    let b = (0..10).map(|_| rng.gen()).collect::<Vec<f32>>();

    let size = (a.len() * size_of::<f32>()) as wgpu::BufferAddress;

    let dst_buffer = device.create_buffer(&wgpu::BufferDescriptor {
        label: None,
        size,
        usage: wgpu::BufferUsages::MAP_READ | wgpu::BufferUsages::COPY_DST,
        mapped_at_creation: false,
    });

    let a_storage_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
        label: None,
        contents: bytemuck::cast_slice(&a),
        usage: wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_SRC,
    });
    let b_storage_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
        label: None,
        contents: bytemuck::cast_slice(&b),
        usage: wgpu::BufferUsages::STORAGE
            | wgpu::BufferUsages::COPY_DST
            | wgpu::BufferUsages::COPY_SRC,
    });

    let bind_group_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
        label: None,
        entries: &[
            wgpu::BindGroupLayoutEntry {
                binding: 0,
                visibility: wgpu::ShaderStages::COMPUTE,
                ty: wgpu::BindingType::Buffer {
                    ty: wgpu::BufferBindingType::Storage { read_only: true },
                    has_dynamic_offset: false,
                    min_binding_size: None,
                },
                count: None,
            },
            wgpu::BindGroupLayoutEntry {
                binding: 1,
                visibility: wgpu::ShaderStages::COMPUTE,
                ty: wgpu::BindingType::Buffer {
                    ty: wgpu::BufferBindingType::Storage { read_only: false },
                    has_dynamic_offset: false,
                    min_binding_size: None,
                },
                count: None,
            },
        ],
    });

    let bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
        label: None,
        layout: &bind_group_layout,
        entries: &[
            wgpu::BindGroupEntry {
                binding: 0,
                resource: a_storage_buffer.as_entire_binding(),
            },
            wgpu::BindGroupEntry {
                binding: 1,
                resource: b_storage_buffer.as_entire_binding(),
            },
        ],
    });

    // Create the pipeline layout with push constant range
    let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
        label: None,
        bind_group_layouts: &[&bind_group_layout],
        push_constant_ranges: &[wgpu::PushConstantRange {
            stages: wgpu::ShaderStages::COMPUTE,
            range: 0..4,
        }],
    });

    let compute_pipeline = device.create_compute_pipeline(&wgpu::ComputePipelineDescriptor {
        label: None,
        layout: Some(&pipeline_layout),
        module: &cs_module,
        entry_point: "main",
        compilation_options: Default::default(),
        cache: None,
    });

    let pc = Pc { offset: rng.gen() };

    let mut encoder =
        device.create_command_encoder(&wgpu::CommandEncoderDescriptor { label: None });
    {
        let mut cpass = encoder.begin_compute_pass(&wgpu::ComputePassDescriptor {
            label: None,
            timestamp_writes: None,
        });
        cpass.set_pipeline(&compute_pipeline);
        cpass.set_bind_group(0, &bind_group, &[]);
        cpass.set_push_constants(0, bytemuck::bytes_of(&pc)); // set push constants
        cpass.dispatch_workgroups(a.len() as u32, 1, 1);
    }
    encoder.copy_buffer_to_buffer(&b_storage_buffer, 0, &dst_buffer, 0, size);

    queue.submit(Some(encoder.finish()));

    let buffer_slice = dst_buffer.slice(..);
    buffer_slice.map_async(wgpu::MapMode::Read, |_| {});
    device.poll(wgpu::Maintain::wait()).panic_on_timeout();
    let data = buffer_slice.get_mapped_range();
    let res: Vec<f32> = bytemuck::cast_slice(&data).to_vec();
    drop(data);
    dst_buffer.unmap();

    assert_eq!(
        a.iter()
            .zip(b.iter())
            .map(|(a, b)| a + b + pc.offset)
            .collect::<Vec<f32>>(),
        res
    );

    Ok(())
}
