use std::{sync::mpsc::channel, u32};

use bytemuck;
use pollster::block_on;
use wgpu::util::DeviceExt;

const N_PRIMECHECK: u32 = u32::MAX / 64;
const WORKGROUP_SZ: u32 = 256;
const MAX_DISPATCH_SZ: u32 = 65535;

#[repr(C)]
#[derive(Copy, Clone, bytemuck::Pod, bytemuck::Zeroable)]
struct SieveElement {
    data: u32,
}

fn main() {
    block_on(run());
}

async fn run() {
    // We'll start off on the cpu - search for primes up to 65536.
    let mut primes: Vec<u32> = (2..u16::MAX as u32)
        .filter(|v| {
            for i in 2..(f32::sqrt(*v as f32) as u32 + 1) {
                if v % i == 0 {
                    return false;
                }
            }
            true
        })
        .collect();

    let mut prime_starter = primes.clone();
    println!("n starter primes {}", prime_starter.len());
    // Pad the primes so they fit in a Vec<Vec4<u32>> for the uniform buffer
    prime_starter.resize(prime_starter.len().div_ceil(4) * 4, 0);

    // Calculate some values for running the sieve.
    const MIN_BUF_SZ: u32 = N_PRIMECHECK.div_ceil(32);
    const REQUESTED_DISPATCH_GROUP_SZ: u32 = MIN_BUF_SZ.div_ceil(WORKGROUP_SZ);
    const BUF_SZ: u32 = REQUESTED_DISPATCH_GROUP_SZ * WORKGROUP_SZ;
    const DISPATCH_SZ: u32 = if REQUESTED_DISPATCH_GROUP_SZ < MAX_DISPATCH_SZ {
        REQUESTED_DISPATCH_GROUP_SZ
    } else {
        MAX_DISPATCH_SZ
    };

    const BYTES_TO_COVER: u32 = BUF_SZ.div_ceil(WORKGROUP_SZ * DISPATCH_SZ);

    println!(
        "Requested buf size: {}\n Bytes to cover per workgroup-dispatch: {}\n Buf size: {}\n",
        BUF_SZ,
        BYTES_TO_COVER,
        BYTES_TO_COVER * WORKGROUP_SZ * DISPATCH_SZ
    );

    let instance = wgpu::Instance::new(wgpu::InstanceDescriptor {
        backends: wgpu::Backends::all(),
        dx12_shader_compiler: Default::default(),
        ..Default::default()
    });

    let adapter = instance
        .request_adapter(&wgpu::RequestAdapterOptions {
            power_preference: wgpu::PowerPreference::HighPerformance,
            compatible_surface: None,
            force_fallback_adapter: false,
        })
        .await
        .unwrap();

    let (device, queue) = adapter
        .request_device(
            &wgpu::DeviceDescriptor {
                required_features: wgpu::Features::empty(),
                required_limits: wgpu::Limits::default(),
                label: None,
                ..Default::default()
            },
            None,
        )
        .await
        .unwrap();

    let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
        label: Some("Prime Sieve Shader"),
        source: wgpu::ShaderSource::Wgsl(include_str!("primesieve.wgsl").into()),
    });

    let sieve: Vec<u32> = vec![0; (BYTES_TO_COVER * WORKGROUP_SZ * DISPATCH_SZ) as usize];

    let sieve_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
        label: Some("Sieve Buffer"),
        contents: bytemuck::cast_slice(&sieve),
        usage: wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_SRC,
    });

    let bytes_cover_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
        label: Some("Bytes workgroup must cover"),
        contents: bytemuck::cast_slice(&[BYTES_TO_COVER]),
        usage: wgpu::BufferUsages::UNIFORM,
    });

    let starter_primes_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
        label: Some("Primes required to calculate up to 2**32"),
        contents: bytemuck::cast_slice(&prime_starter),
        usage: wgpu::BufferUsages::UNIFORM,
    });

    let output_buffer = device.create_buffer(&wgpu::BufferDescriptor {
        label: Some("Output Buffer"),
        size: (sieve.len() * std::mem::size_of::<u32>()) as u64,
        usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ,
        mapped_at_creation: false,
    });

    let start_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
        label: Some("Starting number"),
        contents: bytemuck::cast_slice(&[0]),
        usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
    });

    let bind_group_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
        entries: &[
            wgpu::BindGroupLayoutEntry {
                binding: 0,
                visibility: wgpu::ShaderStages::COMPUTE,
                ty: wgpu::BindingType::Buffer {
                    ty: wgpu::BufferBindingType::Uniform,
                    has_dynamic_offset: false,
                    min_binding_size: None,
                },
                count: None,
            },
            wgpu::BindGroupLayoutEntry {
                binding: 1,
                visibility: wgpu::ShaderStages::COMPUTE,
                ty: wgpu::BindingType::Buffer {
                    ty: wgpu::BufferBindingType::Uniform,
                    has_dynamic_offset: false,
                    min_binding_size: None,
                },
                count: None,
            },
            wgpu::BindGroupLayoutEntry {
                binding: 2,
                visibility: wgpu::ShaderStages::COMPUTE,
                ty: wgpu::BindingType::Buffer {
                    ty: wgpu::BufferBindingType::Storage { read_only: false },
                    has_dynamic_offset: false,
                    min_binding_size: None,
                },
                count: None,
            },
        ],
        label: None,
    });

    let start_bind_group_layout =
        device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            entries: &[wgpu::BindGroupLayoutEntry {
                binding: 0,
                visibility: wgpu::ShaderStages::COMPUTE,
                ty: wgpu::BindingType::Buffer {
                    ty: wgpu::BufferBindingType::Uniform,
                    has_dynamic_offset: false,
                    min_binding_size: None,
                },
                count: None,
            }],
            label: None,
        });

    let bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
        layout: &bind_group_layout,
        entries: &[
            wgpu::BindGroupEntry {
                binding: 0,
                resource: bytes_cover_buffer.as_entire_binding(),
            },
            wgpu::BindGroupEntry {
                binding: 1,
                resource: starter_primes_buffer.as_entire_binding(),
            },
            wgpu::BindGroupEntry {
                binding: 2,
                resource: sieve_buffer.as_entire_binding(),
            },
        ],
        label: None,
    });

    let start_bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
        layout: &start_bind_group_layout,
        entries: &[wgpu::BindGroupEntry {
            binding: 0,
            resource: start_buffer.as_entire_binding(),
        }],
        label: None,
    });

    let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
        label: None,
        bind_group_layouts: &[&bind_group_layout, &start_bind_group_layout],
        push_constant_ranges: &[],
    });

    let compute_pipeline = device.create_compute_pipeline(&wgpu::ComputePipelineDescriptor {
        label: Some("Compute Pipeline"),
        layout: Some(&pipeline_layout),
        module: &shader,
        entry_point: "main",
        compilation_options: Default::default(),
        cache: Default::default(),
    });

    let mut result: Vec<u32> = Vec::new();

    println!("Calculating primes in batches of 64...");
    for i in 0..64 {
        println!("{}", i);
        let mut encoder =
            device.create_command_encoder(&wgpu::CommandEncoderDescriptor { label: None });
        queue.write_buffer(&start_buffer, 0, bytemuck::cast_slice(&[i * 2u32.pow(26)]));

        {
            let mut compute_pass =
                encoder.begin_compute_pass(&wgpu::ComputePassDescriptor::default());
            compute_pass.set_pipeline(&compute_pipeline);
            compute_pass.set_bind_group(0, &bind_group, &[]);
            compute_pass.set_bind_group(1, &start_bind_group, &[]);
            compute_pass.dispatch_workgroups(DISPATCH_SZ, 1, 1);
        }

        encoder.copy_buffer_to_buffer(
            &sieve_buffer,
            0,
            &output_buffer,
            0,
            (sieve.len() * std::mem::size_of::<u32>()) as u64,
        );

        queue.submit(Some(encoder.finish()));

        let buffer_slice = output_buffer.slice(..);

        let (sender, _receiver) = channel();
        let _buffer_future =
            buffer_slice.map_async(wgpu::MapMode::Read, move |v| sender.send(v).unwrap());
        device.poll(wgpu::Maintain::Wait);

        let data = buffer_slice.get_mapped_range();
        result.append(&mut bytemuck::cast_slice(&data).to_vec());

        drop(data);
        output_buffer.unmap();
    }

    for (i, v) in result.iter().enumerate() {
        for j in 0..32 {
            if (v & (1 << j)) == 0 && i as u32 * 32 + j > 1 {
                primes.push(i as u32 * 32 + j);
            }
        }
    }

    println!("last: {}", primes.last().expect("odear"));
    println!("number of primes up to 2^32-1: {}", primes.len());
}
