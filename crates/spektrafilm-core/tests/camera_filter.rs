use spektrafilm_core::{
    border_135::Border135,
    params::{CameraFilter, RuntimeParams},
    pipeline::Pipeline,
    profile::load_profile_by_name,
};
use spektrafilm_gpu::{
    FilmRenderOptions, SourceIdentity, cpu_backend::CpuBackend, wgpu_backend::WgpuBackend,
};
use spektrafilm_math::{image::ImageBuf, precision::Scalar};
use std::path::Path;

fn build(stock: &str, params: RuntimeParams) -> Result<Pipeline, String> {
    let data = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../data");
    Pipeline::new_with_spectral(
        load_profile_by_name(&data, stock).unwrap(),
        load_profile_by_name(&data, "kodak_2302").unwrap(),
        params,
        &data,
    )
}

fn recipe() -> RuntimeParams {
    let mut p = RuntimeParams::default();
    p.io.input_color_space = "sRGB".into();
    p.io.output_cctf_encoding = false;
    p.camera.auto_exposure = false;
    p.camera.photographic_filter = CameraFilter::Clear;
    p.film_render.grain.active = false;
    p.film_render.halation.active = false;
    p.film_render.halation.boost_ev = 0.;
    p.film_render.dir_couplers.active = false;
    p.film_render.glare.active = false;
    p.print_render.glare.active = false;
    p.scanner.unsharp_mask = [0.; 2];
    p.settings.use_enlarger_lut = false;
    p.settings.use_scanner_lut = false;
    p
}

fn chart() -> ImageBuf {
    ImageBuf::from_data(
        3,
        1,
        vec![0.184, 0.184, 0.184, 0.65, 0.06, 0.06, 0.06, 0.06, 0.65],
    )
}

#[test]
fn compensated_red_filters_preserve_gray_and_separate_red_and_blue_in_print() {
    for method in ["hanatos2025", "arctic2026beta04", "mallett2019"] {
        let mut p = recipe();
        p.settings.rgb_to_raw_method = method.into();
        let bare = build("kodak_doublex", p.clone()).unwrap();
        let render = |pipe: &Pipeline| pipe.try_process(chart(), &CpuBackend).unwrap().data;
        let reference = render(&bare);
        // Kodak H-1-5222 daylight factors. These are independent of the
        // LUTs and catch treating null log samples as unit sensitivity.
        for (filter, daylight) in [
            (CameraFilter::Wratten25, 8.0_f64),
            (CameraFilter::Wratten29, 20.0),
        ] {
            p.camera.photographic_filter = filter;
            p.camera.compensate_filter = false;
            assert!(bare.refreshed(p.clone()).is_none());
            let glass = build("kodak_doublex", p.clone()).unwrap();
            let loss = glass.filter_factor_ev();
            assert!(
                (loss - daylight.log2()).abs() < 0.55,
                "{method} {filter:?}: {loss} EV"
            );
            let physical = render(&glass);
            p.camera.compensate_filter = true;
            assert!(glass.refreshed(p.clone()).is_none());
            let compensated = build("kodak_doublex", p.clone()).unwrap();
            assert_eq!(
                bare.print_exposure_factor(),
                compensated.print_exposure_factor()
            );
            let corrected = render(&compensated);
            println!(
                "{method} {filter:?} EV={loss:.3} gray/red/blue bare={:?} filtered={:?} comp={:?}",
                [reference[1], reference[4], reference[7]],
                [physical[1], physical[4], physical[7]],
                [corrected[1], corrected[4], corrected[7]]
            );
            assert!((reference[1] - corrected[1]).abs() < 0.001);
            assert!(physical[1] < corrected[1] - 0.01);
            assert!(corrected[4] > reference[4] + 0.02 && corrected[7] < reference[7] - 0.02);
        }
    }
}

#[test]
fn resident_filter_changes_density_once_and_preserves_fog() {
    let Some(gpu) = WgpuBackend::new() else {
        assert!(std::env::var_os("SPEKTRAFILM_REQUIRE_GPU").is_none());
        return;
    };
    for method in ["hanatos2025", "arctic2026beta04", "mallett2019"] {
        let mut p = recipe();
        p.settings.rgb_to_raw_method = method.into();
        p.camera.photographic_filter = CameraFilter::Wratten25;
        p.camera.compensate_filter = true;
        let print = build("kodak_doublex", p).unwrap();
        let reference = print.try_process(chart(), &CpuBackend).unwrap();
        let resident = print
            .process_resident_with_options(&chart(), &gpu, Default::default())
            .unwrap()
            .unwrap();
        let max_error = reference
            .data
            .iter()
            .zip(&resident.data)
            .map(|(&a, &b)| (a - b).abs())
            .fold(0.0 as Scalar, Scalar::max);
        println!("{method} CPU/resident print max linear error {max_error}");
        assert!(reference.data.iter().chain(&resident.data).all(|v| v.is_finite()));
        assert!(max_error < 0.0001, "{method}: {max_error}");
    }
    gpu.configure_negative_cache(true);
    let mut p = recipe();
    p.io.scan_film = true;
    p.camera.film_format_mm = 36.;
    let border = Border135 {
        overscan: [20; 4],
        edge_exposure_centi_ev: Some(-100),
        ..Default::default()
    };
    p.border_135 = Some(border);
    let input = ImageBuf::from_data(64, 42, [0.45 as Scalar, 0.12, 0.08].repeat(64 * 42));
    let layout = border.layout([64, 42], 36.).unwrap().unwrap();
    let render = |params| {
        build("kodak_doublex", params)
            .unwrap()
            .process_resident_with_options(
                &input,
                &gpu,
                FilmRenderOptions {
                    source: Some(SourceIdentity(25)),
                    ..Default::default()
                },
            )
            .unwrap()
            .unwrap()
    };
    let bare = render(p.clone());
    p.camera.photographic_filter = CameraFilter::Wratten25;
    let physical = render(p.clone());
    p.camera.compensate_filter = true;
    let comp = render(p.clone());
    let again = render(p);
    assert_eq!(again.data, comp.data);
    let stats = gpu.negative_cache_stats();
    assert_eq!((stats.hits, stats.misses), (1, 3));
    let mut shoulder = 0;
    let mut changed = 0;
    for y in 0..bare.height {
        for x in 0..bare.width {
            let site = layout.sample([x, y]);
            let a = bare.get(x, y)[1];
            let b = physical.get(x, y)[1];
            let c = comp.get(x, y)[1];
            if site.gate_open {
                changed += usize::from((a - b).abs() > 0.005 && (a - c).abs() > 0.005);
            } else {
                assert!((a - b).abs() < 0.001 && (a - c).abs() < 0.001);
                shoulder += usize::from(site.film_present);
            }
        }
    }
    assert!(
        changed > 100 && shoulder > 100,
        "gate changed={changed}, film shoulder={shoulder}, center={:?}/{:?}/{:?}",
        bare.get(50, 40),
        physical.get(50, 40),
        comp.get(50, 40)
    );
}

#[test]
fn color_stock_rejects_monochrome_filter() {
    let mut p = recipe();
    p.camera.photographic_filter = CameraFilter::Wratten25;
    assert!(
        build("kodak_portra_400", p)
            .err()
            .unwrap()
            .contains("monochrome")
    );
}

#[test]
fn original_recipe_preserves_missing_log_convention_and_simplified_render_rejects_filters() {
    let data = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../data");
    let film = load_profile_by_name(&data, "kodak_doublex").unwrap();
    let paper = load_profile_by_name(&data, "kodak_2302").unwrap();
    let sample = film
        .data
        .wavelengths
        .iter()
        .position(|&nm| nm == 750.0)
        .unwrap();
    assert!(film.data.log_sensitivity[sample][0].is_nan());
    assert_eq!(film.log_sensitivity_f64()[sample][0], 0.0);
    let old: RuntimeParams = serde_json::from_str(r#"{"camera":{"auto_exposure":false}}"#).unwrap();
    assert_eq!(old.camera.photographic_filter, CameraFilter::Off);
    let pipe = Pipeline::new(film, paper, recipe());
    assert_eq!(
        pipe.try_process(chart(), &CpuBackend).unwrap_err(),
        spektrafilm_gpu::OutputError::CameraFilterRequiresSpectral
    );
    assert_eq!(
        pipe.try_with_resident_params(&chart(), |_| ()).unwrap_err(),
        spektrafilm_gpu::OutputError::CameraFilterRequiresSpectral
    );
}

#[test]
fn measured_response_on_trix_reversal_reaches_direct_scan() {
    let mut p = recipe();
    p.io.scan_film = true;
    let bare = build("kodak_trix", p.clone())
        .unwrap()
        .try_process(chart(), &CpuBackend)
        .unwrap();
    p.camera.photographic_filter = CameraFilter::Wratten25;
    p.camera.compensate_filter = true;
    let red = build("kodak_trix", p)
        .unwrap()
        .try_process(chart(), &CpuBackend)
        .unwrap();
    assert!((bare.data[1] - red.data[1]).abs() < 0.001);
    assert!(red.data[4] > bare.data[4] && red.data[7] < bare.data[7]);
}
