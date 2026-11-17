use spektrafilm_core::{
    border_135::{Border135, Layout135, ScanRegistration},
    params::RuntimeParams,
    pipeline::Pipeline,
    profile::load_profile_by_name,
};
use spektrafilm_gpu::{
    FilmRenderOptions, SourceIdentity,
    output::{DisplayRegion, Rgb16Image},
    wgpu_backend::WgpuBackend,
};
use spektrafilm_math::{image::ImageBuf, precision::from_f32};
use std::path::Path;

fn image(width: u32, height: u32, rgb: [f32; 3]) -> ImageBuf {
    ImageBuf::from_data(
        width,
        height,
        rgb.map(from_f32).repeat((width * height) as usize),
    )
}

fn pixel(image: &Rgb16Image, x: u32, y: u32) -> &[u16] {
    let i = ((y * image.width + x) * 3) as usize;
    &image.data[i..i + 3]
}

fn render(
    backend: &WgpuBackend,
    params: RuntimeParams,
    input: &ImageBuf,
    region: Option<DisplayRegion>,
    identity: Option<SourceIdentity>,
) -> Rgb16Image {
    let data = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../data");
    let film = load_profile_by_name(&data, "kodak_portra_400").unwrap();
    let paper = load_profile_by_name(&data, "kodak_portra_endura").unwrap();
    Pipeline::new_with_spectral(film, paper, params, &data)
        .unwrap()
        .try_with_resident_params(input, |prepared| {
            backend.run_film_chain_rgb16(
                prepared,
                FilmRenderOptions {
                    source: identity,
                    ..Default::default()
                },
                region,
            )
        })
        .unwrap()
        .unwrap()
        .unwrap()
}

fn options() -> RuntimeParams {
    let mut params = RuntimeParams::default();
    params.camera.film_format_mm = 36.0;
    params.camera.auto_exposure = false;
    params.film_render.halation.active = false;
    params.film_render.dir_couplers.active = false;
    params.film_render.grain.active = false;
    params.film_render.glare.active = false;
    params.print_render.glare.active = false;
    params.scanner.unsharp_mask = [0.0, 0.0];
    params
}

fn verify(rendered: &Rgb16Image, layout: Layout135, content_code: &[u16], shoulder_code: &[u16]) {
    assert_eq!([rendered.width, rendered.height], layout.canvas);
    assert_ne!(content_code, shoulder_code);
    let [left, top, width, height] = layout.content;
    let mut absent_code = None;
    let mut counts = [0; 3];
    for y in 0..rendered.height {
        for x in 0..rendered.width {
            let actual = pixel(rendered, x, y);
            let site = layout.sample([x, y]);
            if !site.film_present {
                assert_eq!(actual, *absent_code.get_or_insert(actual), "{x},{y}");
                counts[0] += 1;
            } else if site.gate_open
                && (left..left + width).contains(&x)
                && (top..top + height).contains(&y)
            {
                assert_eq!(actual, content_code, "{x},{y}");
                counts[1] += 1;
            } else {
                assert_eq!(actual, shoulder_code, "{x},{y}");
                counts[2] += 1;
            }
        }
    }
    assert!(counts.iter().all(|&n| n > 0));
    assert_ne!(absent_code.unwrap(), shoulder_code);
}

#[test]
fn exposure_and_transmission_follow_cpu_layout_in_print_and_scan() {
    let Some(backend) = WgpuBackend::new() else {
        assert!(std::env::var_os("SPEKTRAFILM_REQUIRE_GPU").is_none());
        return;
    };
    for (scan, width, height, registration) in [
        (false, 80, 53, ScanRegistration::default()),
        (
            true,
            49,
            73,
            ScanRegistration {
                offset_px: [3, -2],
                rotation_centi_degrees: 475,
            },
        ),
    ] {
        let mut params = options();
        params.io.scan_film = scan;
        let input = image(width, height, [0.3, 0.63, 0.17]);
        let reference = render(&backend, params.clone(), &input, None, None);
        let fog = render(
            &backend,
            params.clone(),
            &image(width, height, [0.09; 3]),
            None,
            None,
        );
        let border = Border135 {
            overscan: [21, 19, 22, 25],
            registration,
            edge_exposure_centi_ev: Some(-100),
        };
        let layout = border.layout([width, height], 36.0).unwrap().unwrap();
        params.border_135 = Some(border);
        let frame = render(&backend, params.clone(), &input, None, None);
        verify(&frame, layout, pixel(&reference, 0, 0), pixel(&fog, 0, 0));
        if !scan {
            let crop = render(
                &backend,
                params,
                &input,
                Some(layout.content_region()),
                None,
            );
            assert_eq!(crop, reference);
        }
    }
}

#[test]
fn negative_identity_tracks_border_and_reuses_layout_for_output_changes() {
    let Some(backend) = WgpuBackend::new() else {
        assert!(std::env::var_os("SPEKTRAFILM_REQUIRE_GPU").is_none());
        return;
    };
    let mut params = options();
    let input = image(63, 49, [0.5, 0.34, 0.18]);
    params.border_135 = Some(Border135 {
        overscan: [18; 4],
        ..Default::default()
    });
    let source = Some(SourceIdentity(135));
    backend.configure_negative_cache(true);
    let original = render(&backend, params.clone(), &input, None, source);
    let layout = params
        .border_135
        .unwrap()
        .layout([63, 49], 36.0)
        .unwrap()
        .unwrap();
    render(
        &backend,
        params.clone(),
        &input,
        Some(layout.content_region()),
        source,
    );
    params.enlarger.print_exposure = 1.2;
    render(&backend, params.clone(), &input, None, source);
    let stats = backend.negative_cache_stats();
    assert_eq!((stats.hits, stats.misses), (2, 1));

    params.border_135.as_mut().unwrap().edge_exposure_centi_ev = Some(0);
    let bright = render(&backend, params.clone(), &input, None, source);
    assert_ne!(bright, original);
    params.border_135.as_mut().unwrap().registration.offset_px = [0, 2];
    assert_ne!(render(&backend, params, &input, None, source), bright);
    assert_eq!(backend.negative_cache_stats().misses, 3);
}
