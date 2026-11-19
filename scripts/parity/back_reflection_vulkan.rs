#[cfg(test)]
#[test]
#[ignore = "offline 5 um film-base study on Vulkan hardware"]
fn backing_mixture_probe() {
    let gpu = WgpuBackend::new().unwrap();
    let dir = std::path::PathBuf::from(std::env::var_os("BACKING_PROBE_DIR").unwrap());
    let (w, h) = (513u32, 513u32);
    let bytes = std::fs::read(dir.join("input.f32")).unwrap();
    assert_eq!(bytes.len(), (w * h * 12) as usize);
    let src = gpu
        .device
        .create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("linear exposure"),
            contents: &bytes,
            usage: wgpu::BufferUsages::STORAGE,
        });
    let plane = |usage| {
        gpu.device.create_buffer(&wgpu::BufferDescriptor {
            label: None,
            size: bytes.len() as u64,
            usage,
            mapped_at_creation: false,
        })
    };
    let mid = plane(wgpu::BufferUsages::STORAGE);
    let blurred = plane(wgpu::BufferUsages::STORAGE);
    let sum = plane(
        wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_SRC | wgpu::BufferUsages::COPY_DST,
    );
    let staging = plane(wgpu::BufferUsages::MAP_READ | wgpu::BufferUsages::COPY_DST);
    let shader = gpu.cached_pipeline(
        r#"
        @group(0) @binding(0) var<uniform> weight: vec4<f32>;
        @group(0) @binding(1) var<storage,read> src: array<f32>;
        @group(0) @binding(2) var<storage,read_write> sum: array<f32>;
        @compute @workgroup_size(256) fn main(@builtin(global_invocation_id) id: vec3<u32>) {
            if (id.x < arrayLength(&sum)) { sum[id.x] += weight[id.x%3u] * src[id.x]; }
        }"#,
        &[
            wgpu::BufferBindingType::Uniform,
            wgpu::BufferBindingType::Storage { read_only: true },
            wgpu::BufferBindingType::Storage { read_only: false },
        ],
    );
    let mut jobs = Vec::new();
    for (ratio, weight) in [
        (
            [0.184586, 0.178469, 0.170237],
            [0.041510, 0.106789, 0.202791, 0.0f32],
        ),
        ([1.121207, 1.000078, 0.546805], [-0.5, -0.5, 0.156578, 0.0]),
        (
            [1.651048, 1.581389, 0.808738],
            [1.034500, 1.384389, -0.5, 0.0],
        ),
        (
            [4.677567, 9.448921, 1.340598],
            [0.423990, 0.008822, 1.140631, 0.0],
        ),
    ] {
        let sigmas = ratio.map(|r| r * 24.0);
        append_pass(&mut jobs, &gpu, w, h, sigmas, 1, &src, &mid);
        append_pass(&mut jobs, &gpu, w, h, sigmas, 0, &mid, &blurred);
        let weights = gpu
            .device
            .create_buffer_init(&wgpu::util::BufferInitDescriptor {
                label: None,
                contents: bytemuck::cast_slice(&weight),
                usage: wgpu::BufferUsages::UNIFORM,
            });
        let entries = [&weights, &blurred, &sum].map(|buf| buf.as_entire_binding());
        let bg = gpu.device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: None,
            layout: &shader.layout,
            entries: &std::array::from_fn::<_, 3, _>(|i| wgpu::BindGroupEntry {
                binding: i as u32,
                resource: entries[i].clone(),
            }),
        });
        jobs.push((
            DispatchJob {
                _params_buf: weights,
                pipeline: CachedPipelineRef {
                    layout: shader.layout.clone(),
                    pipeline: shader.pipeline.clone(),
                },
                bg,
            },
            ((w * h * 3).div_ceil(256), 1),
        ));
    }
    for iteration in 0..4 {
        let start = std::time::Instant::now();
        let mut enc = gpu.device.create_command_encoder(&Default::default());
        enc.clear_buffer(&sum, 0, None);
        for (job, groups) in &jobs {
            let mut pass = enc.begin_compute_pass(&Default::default());
            pass.set_pipeline(&job.pipeline.pipeline);
            pass.set_bind_group(0, &job.bg, &[]);
            pass.dispatch_workgroups(groups.0, groups.1, 1);
        }
        gpu.queue.submit(Some(enc.finish()));
        gpu.device
            .poll(wgpu::PollType::Wait {
                submission_index: None,
                timeout: None,
            })
            .unwrap();
        eprintln!("mixture {iteration}: {:?}", start.elapsed());
    }
    let mut enc = gpu.device.create_command_encoder(&Default::default());
    enc.copy_buffer_to_buffer(&sum, 0, &staging, 0, bytes.len() as u64);
    gpu.queue.submit(Some(enc.finish()));
    let slice = staging.slice(..);
    let (tx, rx) = std::sync::mpsc::channel();
    slice.map_async(wgpu::MapMode::Read, move |r| tx.send(r).unwrap());
    gpu.device
        .poll(wgpu::PollType::Wait {
            submission_index: None,
            timeout: None,
        })
        .unwrap();
    rx.recv().unwrap().unwrap();
    std::fs::write(dir.join("mixture.f32"), &*slice.get_mapped_range().unwrap()).unwrap();
}
