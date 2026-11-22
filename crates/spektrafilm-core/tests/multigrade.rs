use spektrafilm_core::{
    params::{CameraFilter, PaperGrade, RuntimeParams},
    pipeline::Pipeline,
    profile::load_profile_by_name,
};
use spektrafilm_gpu::{
    FilmRenderOptions, SourceIdentity, cpu_backend::CpuBackend, wgpu_backend::WgpuBackend,
};
use spektrafilm_math::{image::ImageBuf, precision::Scalar};
use std::path::Path;

const STOCKS: [&str; 4] = [
    "kodak_tri_x_400",
    "kodak_tmax_100",
    "fujifilm_neopan_acros_100_ii",
    "ilford_hp5_plus_400",
];
const PAPER: &str = "ilford_multigrade_iv_rc";

fn build(stock: &str, p: RuntimeParams) -> Pipeline {
    let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../data");
    let film = load_profile_by_name(&dir, stock).unwrap();
    assert_eq!(film.info.target_print.as_deref(), Some(PAPER));
    Pipeline::new_with_spectral(film, load_profile_by_name(&dir, PAPER).unwrap(), p, &dir).unwrap()
}

fn recipe() -> RuntimeParams {
    let mut p = RuntimeParams::default();
    p.camera.auto_exposure = false;
    p.camera.photographic_filter = CameraFilter::Clear;
    p.io.input_color_space = "sRGB".into();
    p.io.output_cctf_encoding = false;
    p.film_render.grain.active = false;
    p.film_render.halation.active = false;
    p.film_render.halation.boost_ev = 0.;
    p.film_render.dir_couplers.active = false;
    p.print_render.glare.active = false;
    p.scanner.unsharp_mask = [0.; 2];
    p.settings.use_enlarger_lut = false;
    p.settings.use_scanner_lut = false;
    p
}

fn chart() -> ImageBuf {
    let mut pixels = Vec::new();
    for ev in -3..=3 {
        pixels.extend([0.184 * (2.0 as Scalar).powi(ev); 3]);
    }
    pixels.extend([0.65, 0.06, 0.06, 0.06, 0.06, 0.65]);
    ImageBuf::from_data(9, 1, pixels)
}

#[test]
fn four_films_print_monotonic_wedges_and_filter_colors_on_graded_silver() {
    let input = chart();
    for stock in STOCKS {
        let mut prev = 0.;
        let mut p = recipe();
        for grade in PaperGrade::ALL {
            p.print_render.paper_grade = Some(grade);
            let pipe = build(stock, p.clone());
            let y = pipe.try_process(input.clone(), &CpuBackend).unwrap().data;
            let green: Vec<_> = y.as_chunks::<3>().0.iter().map(|rgb| rgb[1]).collect();
            let span = (green[6] / green[0]).log10();
            println!(
                "{stock} {} mid {:.5} span {:.5}",
                grade.label(),
                green[3],
                span
            );
            assert!(green[..7].windows(2).all(|w| w[1] > w[0]));
            assert!(span > prev, "{stock} {}: {span} vs {prev}", grade.label());
            assert!((green[3] - 0.184).abs() < 0.015);
            prev = span;
            if grade == PaperGrade::Two {
                let mut filtered = p.clone();
                filtered.camera.photographic_filter = CameraFilter::Wratten25;
                filtered.camera.compensate_filter = true;
                let red = build(stock, filtered)
                    .try_process(input.clone(), &CpuBackend)
                    .unwrap()
                    .data;
                assert!((red[10] - green[3]).abs() < 0.001);
                assert!(red[22] > green[7] + 0.02 && red[25] < green[8] - 0.02);
                assert!(
                    y.as_chunks::<3>()
                        .0
                        .iter()
                        .all(|v| (v[0] - v[1]).abs() < 0.002 && (v[2] - v[1]).abs() < 0.002)
                );
                let default = build(stock, recipe());
                assert_eq!(
                    default.print_exposure_factor(),
                    pipe.print_exposure_factor()
                );
                let mut trim = p.clone();
                trim.enlarger.print_exposure = 0.75;
                let warm = pipe.refreshed(trim.clone()).unwrap();
                let a = warm.try_process(input.clone(), &CpuBackend).unwrap();
                let b = build(stock, trim)
                    .try_process(input.clone(), &CpuBackend)
                    .unwrap();
                assert_eq!(a.data, b.data);
                assert_ne!(a.data, y);
            }
        }
    }
}

#[test]
fn resident_reprints_reuse_one_negative_and_match_cold_cpu() {
    let Some(gpu) = WgpuBackend::new() else {
        assert!(std::env::var_os("SPEKTRAFILM_REQUIRE_GPU").is_none());
        return;
    };
    gpu.configure_negative_cache(true);
    let input = chart();
    let mut p = recipe();
    for (i, grade) in PaperGrade::ALL.into_iter().enumerate() {
        p.print_render.paper_grade = Some(grade);
        let pipe = build(STOCKS[0], p.clone());
        let cpu = pipe.try_process(input.clone(), &CpuBackend).unwrap();
        let frame = pipe
            .process_resident_with_options(
                &input,
                &gpu,
                FilmRenderOptions {
                    source: Some(SourceIdentity(406)),
                    ..Default::default()
                },
            )
            .unwrap()
            .unwrap();
        assert!(
            frame
                .data
                .iter()
                .zip(cpu.data)
                .all(|(a, b)| (a - b).abs() < 0.001)
        );
        let stats = gpu.negative_cache_stats();
        assert_eq!((stats.hits, stats.misses), (i as u64, 1));
    }
}

#[test]
fn grade_controls_the_print_only_and_requires_a_matching_calibration() {
    let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../data");
    let film = load_profile_by_name(&dir, STOCKS[0]).unwrap();
    let paper = load_profile_by_name(&dir, PAPER).unwrap();
    let other = load_profile_by_name(&dir, "kodak_2302").unwrap();
    let mut p = recipe();
    let base = build(STOCKS[0], p.clone());
    p.print_render.paper_grade = Some(PaperGrade::Five);
    assert!(base.refreshed(p.clone()).is_none());
    assert!(
        Pipeline::new_with_spectral(film.clone(), other, p.clone(), &dir)
            .err()
            .unwrap()
            .contains("no grade calibration")
    );
    let simple = Pipeline::new(film.clone(), paper.clone(), p.clone());
    assert_eq!(
        simple.try_process(chart(), &CpuBackend).unwrap_err(),
        spektrafilm_gpu::OutputError::PaperGradeRequiresSpectral
    );
    p.enlarger.illuminant = "D65".into();
    assert!(
        Pipeline::new_with_spectral(film.clone(), paper, p.clone(), &dir)
            .err()
            .unwrap()
            .contains("TH-KG3")
    );
    p.io.scan_film = true;
    let scan = Pipeline::new_with_spectral(film.clone(), film, p.clone(), &dir).unwrap();
    p.print_render.paper_grade = Some(PaperGrade::DoubleZero);
    let refresh = scan.refreshed(p).unwrap();
    assert_eq!(
        scan.try_process(chart(), &CpuBackend).unwrap().data,
        refresh.try_process(chart(), &CpuBackend).unwrap().data
    );
}
