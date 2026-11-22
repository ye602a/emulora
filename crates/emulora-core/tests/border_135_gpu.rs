use emulora_core::{
    border_135::{Border135, Layout135, ScanRegistration},
    params::RuntimeParams,
    pipeline::Pipeline,
    profile::load_profile_by_name,
};
use emulora_gpu::{
    FilmRenderOptions, SourceIdentity,
    output::{DisplayRegion, Rgb16Image},
    wgpu_backend::WgpuBackend,
};
use emulora_math::{image::ImageBuf, precision::from_f32};
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
        assert!(std::env::var_os("EMULORA_REQUIRE_GPU").is_none());
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
            ..Default::default()
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
        assert!(std::env::var_os("EMULORA_REQUIRE_GPU").is_none());
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

#[test]
fn explicit_roll_gates_match_the_exposure_mask_and_invalidate_density() {
    use emulora_core::border_135::{FilmGate, GateFormat, RollAxis};
    let Some(backend) = WgpuBackend::new() else {
        assert!(std::env::var_os("EMULORA_REQUIRE_GPU").is_none());
        return;
    };
    let mut params = options();
    let input = image(80, 60, [0.36, 0.45, 0.2]);
    let content = render(&backend, params.clone(), &input, None, None);
    let fog = render(
        &backend,
        params.clone(),
        &image(80, 60, [0.09; 3]),
        None,
        None,
    );
    backend.configure_negative_cache(true);
    let mut misses = backend.negative_cache_stats().misses;
    for format in [
        GateFormat::HalfFrame17,
        GateFormat::FullFrame36,
        GateFormat::Xpan65,
        GateFormat::Roll120 {
            along_tenth_mm: 415,
            across_tenth_mm: 560,
        },
        GateFormat::Roll120 {
            along_tenth_mm: 560,
            across_tenth_mm: 560,
        },
        GateFormat::Roll120 {
            along_tenth_mm: 700,
            across_tenth_mm: 560,
        },
        GateFormat::Roll120 {
            along_tenth_mm: 840,
            across_tenth_mm: 560,
        },
    ] {
        for axis in [RollAxis::Horizontal, RollAxis::Vertical] {
            let border = Border135 {
                gate: Some(FilmGate { format, axis }),
                overscan: [36; 4],
                edge_exposure_centi_ev: Some(-100),
                ..Default::default()
            };
            let layout = border.layout([80, 60], 35.).unwrap().unwrap();
            params.border_135 = Some(border);
            let frame = render(
                &backend,
                params.clone(),
                &input,
                None,
                Some(SourceIdentity(55)),
            );
            verify(&frame, layout, pixel(&content, 0, 0), pixel(&fog, 0, 0));
            misses += 1;
            assert_eq!(backend.negative_cache_stats().misses, misses);
            if !layout.perforated {
                for x in 0..layout.canvas[0] {
                    for y in 0..layout.canvas[1] {
                        assert_eq!(
                            layout.sample([x, y]).film_present,
                            layout.film_at([x, y])[1].abs() < 30.75
                        );
                    }
                }
            }
        }
    }
}

#[test]
fn latent_exposures_change_only_present_film_and_reuse_the_print_negative() {
    use emulora_core::border_135::{FilmImprint, LightLeak};
    let Some(backend) = WgpuBackend::new() else {
        assert!(std::env::var_os("EMULORA_REQUIRE_GPU").is_none());
        return;
    };
    backend.configure_negative_cache(true);
    let mut params = options();
    let input = image(180, 120, [0.18; 3]);
    let source = Some(SourceIdentity(777));
    let mut border = Border135 {
        overscan: [40; 4],
        ..Default::default()
    };
    let layout = border.layout([180, 120], 36.).unwrap().unwrap();
    params.border_135 = Some(border);
    let plain = render(&backend, params.clone(), &input, None, source);
    let mark = FilmImprint {
        text: "F-29".to_string().try_into().unwrap(),
        origin_tenth_mm: [-80, 130],
        height_tenth_mm: 30,
        exposure_milli: [1000; 3],
    };
    border.imprint = Some(mark);
    params.border_135 = Some(border);
    let marked = render(&backend, params.clone(), &input, None, source);
    let mut changed = 0;
    for y in 0..marked.height {
        for x in 0..marked.width {
            if pixel(&marked, x, y) != pixel(&plain, x, y) {
                let site = layout.sample([x, y]);
                assert!(site.film_present && !site.gate_open);
                let [s, t] = layout.film_at([x, y]);
                assert!((-8.0..1.6).contains(&s) && (13.0..16.0).contains(&t));
                changed += 1;
            }
        }
    }
    assert!(changed > 25);
    border.imprint = None;
    let mut leak = LightLeak {
        center_tenth_mm: [60, 0],
        sigma_tenth_mm: [20, 40],
        exposure_milli: [0; 3],
    };
    border.light_leak = Some(leak);
    params.border_135 = Some(border);
    assert_eq!(
        render(&backend, params.clone(), &input, None, source),
        plain
    );
    leak.exposure_milli = [500, 30, 0];
    border.light_leak = Some(leak);
    params.border_135 = Some(border);
    let right = render(&backend, params.clone(), &input, None, source);
    assert_ne!(right, plain);
    leak.center_tenth_mm[0] = -60;
    border.light_leak = Some(leak);
    params.border_135 = Some(border);
    let left = render(&backend, params.clone(), &input, None, source);
    for y in 0..left.height {
        for x in 0..left.width {
            assert_eq!(pixel(&left, x, y), pixel(&right, left.width - 1 - x, y));
            if !layout.sample([x, y]).film_present {
                assert_eq!(pixel(&left, x, y), pixel(&plain, x, y));
            }
        }
    }
    let stats = backend.negative_cache_stats();
    params.enlarger.print_exposure = 1.2;
    render(&backend, params, &input, None, source);
    assert_eq!(backend.negative_cache_stats().hits, stats.hits + 1);

    // In-aperture date on a frame with no shoulder pixels requested.
    let date = FilmImprint {
        text: "2026-11-19".to_owned().try_into().unwrap(),
        origin_tenth_mm: [-120, 40],
        ..mark
    };
    let mut date_params = options();
    date_params.border_135 = Some(Border135 {
        imprint: Some(date),
        ..Default::default()
    });
    let date_frame = render(&backend, date_params, &input, None, source);
    let reference = render(&backend, options(), &input, None, source);
    assert_eq!((date_frame.width, date_frame.height), (180, 120));
    assert_ne!(date_frame, reference);
    assert_eq!(pixel(&date_frame, 2, 2), pixel(&reference, 2, 2));
}
