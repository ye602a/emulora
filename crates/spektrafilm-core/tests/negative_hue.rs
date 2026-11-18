use spektrafilm_core::{params::RuntimeParams, pipeline::Pipeline, profile::load_profile_by_name};
use spektrafilm_gpu::{
    FilmRenderOptions, SourceIdentity, cpu_backend::CpuBackend, wgpu_backend::WgpuBackend,
};
use spektrafilm_math::{
    image::ImageBuf,
    precision::{from_f64, srgb_decode},
};
use std::path::Path;

fn recipe() -> RuntimeParams {
    let mut p = RuntimeParams::default();
    p.io.input_color_space = "sRGB".into();
    p.io.output_cctf_encoding = false;
    p.io.output_gamut_compress.algorithm = "off".into();
    p.camera.auto_exposure = false;
    p.film_render.grain.active = false;
    p.film_render.halation.active = false;
    p.film_render.halation.boost_ev = 0.0;
    p.film_render.dir_couplers.active = false;
    p.print_render.glare.active = false;
    p.scanner.unsharp_mask = [0.0; 2];
    p
}

fn build(film: &str, paper: &str, p: RuntimeParams) -> Result<Pipeline, String> {
    let d = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../data");
    Pipeline::new_with_spectral(
        load_profile_by_name(&d, film).unwrap(),
        load_profile_by_name(&d, paper).unwrap(),
        p,
        &d,
    )
}

fn patches() -> ImageBuf {
    let codes = [
        [115, 62, 179],
        [92, 40, 115],
        [140, 85, 242],
        [242, 195, 85],
        [118, 118, 118],
    ];
    ImageBuf::from_data(
        5,
        1,
        codes
            .into_iter()
            .flatten()
            .map(|c| srgb_decode(from_f64(c as f64 / 255.0)))
            .collect(),
    )
}

#[test]
fn reprinting_preserves_the_resident_negative() {
    let Some(gpu) = WgpuBackend::new() else {
        assert!(std::env::var_os("SPEKTRAFILM_REQUIRE_GPU").is_none());
        return;
    };
    gpu.configure_negative_cache(true);
    let mut p = recipe();
    let mut pipe = build("kodak_portra_400", "kodak_portra_endura", p.clone()).unwrap();
    let input = patches();
    let mut optical = None;
    for (i, hue) in [false, true, false, true].into_iter().enumerate() {
        p.print_render.negative_hue = hue;
        pipe = pipe.refreshed(p.clone()).unwrap();
        let cpu = pipe.try_process(input.clone(), &CpuBackend).unwrap();
        let gpu_result = pipe
            .process_resident_with_options(
                &input,
                &gpu,
                FilmRenderOptions {
                    source: Some(SourceIdentity(700)),
                    ..Default::default()
                },
            )
            .unwrap()
            .unwrap();
        let stats = gpu.negative_cache_stats();
        assert_eq!((stats.misses, stats.hits, stats.stores), (1, i as u64, 1));
        assert!(
            gpu_result
                .data
                .iter()
                .zip(&cpu.data)
                .all(|(a, b)| (a - b).abs() < 0.001)
        );
        if i == 0 {
            optical = Some(cpu.data.clone());
        }
        if hue {
            // Held-out violet RGB (115,62,179) on the neutral Portra/Endura print.
            let expected = [0.1713, 0.1000, 0.3295];
            for (v, e) in cpu.data.iter().zip(expected) {
                assert!((*v as f64 - e).abs() < 0.001, "violet channel: {v} vs {e}");
            }
            assert_eq!(&cpu.data[12..], &optical.as_ref().unwrap()[12..]);
        } else {
            assert_eq!(&cpu.data, optical.as_ref().unwrap());
        }
    }
    // The unexposed rebate and sprocket holes have no scene hue to recover.
    let border = spektrafilm_core::border_135::Border135 {
        overscan: [20; 4],
        edge_exposure_centi_ev: Some(-100),
        ..Default::default()
    };
    p.border_135 = Some(border);
    let large = ImageBuf::from_data(
        64,
        42,
        [from_f64(0.16), from_f64(0.04), from_f64(0.5)].repeat(64 * 42),
    );
    let layout = border
        .layout([64, 42], p.camera.film_format_mm)
        .unwrap()
        .unwrap();
    let mut prints = Vec::new();
    for hue in [false, true] {
        p.print_render.negative_hue = hue;
        pipe = pipe.refreshed(p.clone()).unwrap();
        prints.push(
            pipe.process_resident_with_options(
                &large,
                &gpu,
                FilmRenderOptions {
                    source: Some(SourceIdentity(701)),
                    ..Default::default()
                },
            )
            .unwrap()
            .unwrap(),
        );
    }
    let mut gate_changed = 0;
    for y in 0..prints[0].height {
        for x in 0..prints[0].width {
            let a = prints[0].get(x, y);
            let b = prints[1].get(x, y);
            if layout.sample([x, y]).gate_open {
                gate_changed += usize::from((a[0] - b[0]).abs() > 0.005);
            } else {
                assert_eq!(a, b);
            }
        }
    }
    assert!(gate_changed > 100);
    let stats = gpu.negative_cache_stats();
    assert_eq!((stats.misses, stats.hits, stats.stores), (2, 4, 2));
}

#[test]
fn calibration_follows_local_dir_and_bypasses_non_colour_prints() {
    let mut p = recipe();
    p.print_render.negative_hue = true;
    let pipe = build(
        "kodak_gold_200",
        "fujifilm_crystal_archive_typeii",
        p.clone(),
    )
    .unwrap();
    p.film_render.dir_couplers.active = true;
    let cold = build(
        "kodak_gold_200",
        "fujifilm_crystal_archive_typeii",
        p.clone(),
    )
    .unwrap();
    let input = patches();
    let render = |pipe: &Pipeline| pipe.try_process(input.clone(), &CpuBackend).unwrap().data;
    assert_eq!(render(&pipe.refreshed(p.clone()).unwrap()), render(&cold));
    p.enlarger.y_filter_shift = 10.0;
    assert!(cold.refreshed(p.clone()).is_none());
    assert!(
        build(
            "kodak_gold_200",
            "fujifilm_crystal_archive_typeii",
            p.clone()
        )
        .is_err()
    );
    for (film, paper, scan) in [
        ("kodak_portra_400", "kodak_portra_endura", true),
        ("fujifilm_provia_100f", "kodak_portra_endura", false),
        ("ilford_hp5_plus_400", "ilford_multigrade_iv_rc", false),
    ] {
        p.io.scan_film = scan;
        p.print_render.negative_hue = true;
        let on = build(film, paper, p.clone()).unwrap();
        p.print_render.negative_hue = false;
        assert_eq!(render(&on), render(&build(film, paper, p.clone()).unwrap()));
    }
}

#[test]
fn manual_filtration_reprints_the_same_negative_and_rejects_hue_reconstruction() {
    let Some(gpu) = WgpuBackend::new() else {
        assert!(std::env::var_os("SPEKTRAFILM_REQUIRE_GPU").is_none());
        return;
    };
    gpu.configure_negative_cache(true);
    let input = patches();
    let mut p = recipe();
    let mut first_print = None;
    for (i, (y, m)) in [
        (0.0, 0.0),
        (5.0, 0.0),
        (0.0, -5.0),
        (10.0, -10.0),
        (0.0, 0.0),
    ]
    .into_iter()
    .enumerate()
    {
        p.enlarger.y_filter_shift = y;
        p.enlarger.m_filter_shift = m;
        p.print_render.negative_hue = true;
        let build_print = |p| build("kodak_ektar_100", "kodak_endura_premier", p);
        if y != 0.0 || m != 0.0 {
            let error = build_print(p.clone()).err().expect("trim must be rejected");
            assert!(error.contains("zero enlarger Y/M shifts"), "{error}");
            p.print_render.negative_hue = false;
        }
        let pipe = build_print(p.clone()).unwrap();
        let cpu = pipe.try_process(input.clone(), &CpuBackend).unwrap();
        let resident = pipe
            .process_resident_with_options(
                &input,
                &gpu,
                FilmRenderOptions {
                    source: Some(SourceIdentity(804)),
                    ..Default::default()
                },
            )
            .unwrap()
            .unwrap();
        assert!(
            resident
                .data
                .iter()
                .zip(&cpu.data)
                .all(|(a, b)| (a - b).abs() < 0.001)
        );
        if i == 0 {
            first_print = Some(resident.data);
        } else if y == 0.0 && m == 0.0 {
            assert_eq!(Some(resident.data), first_print);
        } else {
            // The neutral gray patch must show the actual optical trim.
            let gray = &resident.data[12..];
            assert!(gray.iter().any(|v| (v - gray[1]).abs() > 0.005));
        }
        let stats = gpu.negative_cache_stats();
        assert_eq!((stats.misses, stats.hits, stats.stores), (1, i as u64, 1));
    }
}
