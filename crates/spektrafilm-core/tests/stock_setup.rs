//! Public constructor and refresh regressions for physical stock defaults.

use std::path::{Path, PathBuf};

use spektrafilm_core::{
    params::RuntimeParams,
    pipeline::Pipeline,
    profile::{self, Profile},
    stock_setup::{StockDefaults, PHYSICAL_STOCK_DEFAULTS_V1_ID, StockRoute},
};

fn data_dir() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../../data")
}

fn profiles(stock: &str) -> (Profile, Profile) {
    (
        profile::load_profile_by_name(&data_dir(), stock).unwrap(),
        profile::load_profile_by_name(&data_dir(), "kodak_portra_endura").unwrap(),
    )
}

fn parameters(route: StockRoute) -> RuntimeParams {
    let mut params = RuntimeParams::default();
    params.camera.auto_exposure = false;
    params.camera.exposure_compensation_ev = 0.37;
    params.enlarger.print_exposure_compensation = false;
    params.enlarger.print_exposure = 1.11;
    params.io.input_color_space = "sRGB".into();
    params.io.input_cctf_decoding = false;
    params.io.scan_film = route != StockRoute::Paper;
    params.film_render.grain.gpu_multilayer = true;
    params.film_render.grain.output_sampling = Some([1.5, 0.7]);
    params
}

// Frozen adapter statements, intentionally independent of stock_setup.rs.
// Preserve their original pre-constructor / post-constructor placement.
fn reference_pipeline(
    stock: &str,
    route: StockRoute,
    mut film: Profile,
    mut paper: Profile,
    mut params: RuntimeParams,
) -> Pipeline {
    // Frozen engine only recognized D50/D55/D65 in these roles.
    if film.info.reference_illuminant == "T" {
        film.info.reference_illuminant = "D55".into();
    }
    if paper.info.viewing_illuminant == "K75P" {
        paper.info.viewing_illuminant = "D50".into();
    }

    assert_eq!(film.is_positive(), route == StockRoute::Slide);
    let sigma = if film.info.usage == "cine" {
        50.0
    } else {
        65.0
    };
    params.film_render.halation.halation_first_sigma_um = [sigma; 3];
    params.film_render.halation.halation_strength = match film.info.antihalation.as_str() {
        "strong" => [0.015, 0.005, 0.0],
        "weak" => [0.08, 0.02, 0.0],
        "no" => [0.30, 0.10, 0.015],
        _ => params.film_render.halation.halation_strength,
    };
    let mut pipeline = Pipeline::new_with_spectral(film, paper, params, &data_dir()).unwrap();
    if route == StockRoute::Slide {
        let scale = match stock {
            "fujifilm_provia_100f" => 1.3,
            "fujifilm_velvia_100" => 0.9,
            _ => panic!("slide lacks a stock-specific DIR override"),
        };
        let dir = &mut pipeline.params.film_render.dir_couplers;
        dir.gamma_samelayer_rgb = [0.12 * scale, 0.08 * scale, 0.06 * scale];
        dir.gamma_interlayer_r_to_gb = [0.12 * scale, 0.06 * scale];
        dir.gamma_interlayer_g_to_rb = [0.08 * scale, 0.06 * scale];
        dir.gamma_interlayer_b_to_rg = [0.06 * scale, 0.06 * scale];
    }
    pipeline
}

fn assert_calibration_equal(actual: &Pipeline, expected: &Pipeline) {
    assert_eq!(
        actual.print_exposure_factor().to_bits(),
        expected.print_exposure_factor().to_bits(),
        "the stock wrapper must preserve the original calibration ordering"
    );
    assert_eq!(
        actual.print_illuminant_slice(),
        expected.print_illuminant_slice()
    );
    assert_eq!(actual.preflash_raw(), expected.preflash_raw());
    let actual_lut = actual.tc_lut().expect("actual Hanatos calibration");
    let expected_lut = expected.tc_lut().expect("reference Hanatos calibration");
    assert_eq!(actual_lut.size, expected_lut.size);
    assert_eq!(
        actual_lut
            .data
            .iter()
            .map(|value| value.to_bits())
            .collect::<Vec<_>>(),
        expected_lut
            .data
            .iter()
            .map(|value| value.to_bits())
            .collect::<Vec<_>>()
    );
}

#[test]
fn physical_stock_defaults_preserve_calibration_and_parameters() {
    for (stock, route) in [
        ("kodak_portra_400", StockRoute::Paper),
        ("kodak_gold_200", StockRoute::Paper),
        ("kodak_vision3_500t", StockRoute::Paper),
        ("kodak_doublex", StockRoute::Paper),
        ("fujifilm_provia_100f", StockRoute::Slide),
        ("fujifilm_velvia_100", StockRoute::Slide),
        ("kodak_portra_400", StockRoute::NegativeScan),
    ] {
        let (film, paper) = profiles(stock);
        let params = parameters(route);
        let expected = reference_pipeline(stock, route, film.clone(), paper.clone(), params.clone());
        let selection = StockDefaults::Physical(route);
        let actual = Pipeline::new_with_spectral_and_stock_defaults(
            film,
            paper,
            params,
            &data_dir(),
            selection,
        )
        .unwrap();
        assert_eq!(actual.stock_defaults(), Some(selection));
        assert_eq!(selection.id(), PHYSICAL_STOCK_DEFAULTS_V1_ID);
        assert_eq!(
            serde_json::to_value(&actual.params).unwrap(),
            serde_json::to_value(&expected.params).unwrap(),
            "physical stock defaults differed from the v1 reference for {stock} / {route:?}"
        );
        assert_calibration_equal(&actual, &expected);
    }
}

#[test]
fn ordinary_constructors_leave_physical_defaults_opt_in() {
    let params = RuntimeParams::default();
    assert!(!params.film_render.grain.gpu_multilayer);
    let (film, paper) = profiles("kodak_vision3_500t");
    let plain = Pipeline::new(film.clone(), paper.clone(), params.clone());
    assert_eq!(plain.stock_defaults(), None);
    assert_eq!(
        serde_json::to_value(&plain.params).unwrap(),
        serde_json::to_value(&params).unwrap()
    );
    let spectral = Pipeline::new_with_spectral(film, paper, params, &data_dir()).unwrap();
    assert_eq!(spectral.stock_defaults(), None);
    assert_eq!(
        spectral.params.film_render.halation.halation_first_sigma_um,
        [65.0; 3]
    );
    assert_eq!(
        spectral.params.film_render.halation.halation_strength,
        [0.05, 0.015, 0.0]
    );
    let (slide, paper) = profiles("fujifilm_provia_100f");
    let legacy =
        Pipeline::new_with_spectral(slide, paper, parameters(StockRoute::Slide), &data_dir())
            .unwrap();
    assert_eq!(legacy.stock_defaults(), None);
    assert_eq!(
        legacy.params.film_render.dir_couplers.gamma_samelayer_rgb,
        [0.12, 0.08, 0.06]
    );
}

#[test]
fn runtime_refresh_keeps_slide_controls_and_bw_broadcast() {
    for (stock, route) in [
        ("fujifilm_provia_100f", StockRoute::Slide),
        ("fujifilm_velvia_100", StockRoute::Slide),
        ("kodak_doublex", StockRoute::Paper),
    ] {
        let (film, paper) = profiles(stock);
        let original = parameters(route);
        let selection = StockDefaults::Physical(route);
        let pipeline = Pipeline::new_with_spectral_and_stock_defaults(
            film.clone(),
            paper.clone(),
            original.clone(),
            &data_dir(),
            selection,
        )
        .unwrap();
        let factor = pipeline.print_exposure_factor().to_bits();
        let mut changed = original;
        changed.camera.exposure_compensation_ev = -0.2;
        changed.enlarger.print_exposure = 0.8;
        changed.scanner.unsharp_mask = [0.4, 0.5];
        let refreshed = pipeline.refreshed(changed.clone()).unwrap();
        let expected = reference_pipeline(stock, route, film, paper, changed);
        assert_eq!(refreshed.stock_defaults(), Some(selection));
        assert_eq!(refreshed.print_exposure_factor().to_bits(), factor);
        assert_eq!(
            serde_json::to_value(&refreshed.params).unwrap(),
            serde_json::to_value(&expected.params).unwrap()
        );
        if stock == "kodak_doublex" {
            assert_eq!(
                refreshed.params.film_render.halation.halation_strength,
                [0.015; 3]
            );
        }
    }
}

#[test]
fn bundled_filming_profiles_accept_exactly_their_physical_routes() {
    let mut profiles_checked = 0;
    let mut slides_checked = 0;
    let data = data_dir();
    let paper = profile::load_profile_by_name(&data, "kodak_portra_endura").unwrap();
    let missing_data = data.join("not-a-resource-directory");
    for entry in std::fs::read_dir(data.join("profiles")).unwrap() {
        let path = entry.unwrap().path();
        if path.extension().is_none_or(|extension| extension != "json") {
            continue;
        }
        let stock = path.file_stem().unwrap().to_str().unwrap();
        let film = profile::load_profile_by_name(&data, stock).unwrap();
        profiles_checked += 1;
        slides_checked += usize::from(film.is_filming() && film.is_positive());
        for route in [StockRoute::Paper, StockRoute::Slide, StockRoute::NegativeScan] {
            let preset = StockDefaults::Physical(route);
            let params = parameters(route);
            if film.is_printing() || film.is_positive() != (route == StockRoute::Slide) {
                let error = Pipeline::new_with_spectral_and_stock_defaults(
                    film.clone(), paper.clone(), params, &missing_data, preset,
                ).err().unwrap();
                assert!(error.contains("does not match route"), "{stock}/{route:?}: {error}");
                continue;
            }
            let pipeline = Pipeline::new_with_spectral_and_stock_defaults(
                film.clone(), paper.clone(), params.clone(), &data, preset,
            ).unwrap_or_else(|error| panic!("{stock}/{route:?}: {error}"));
            let radius = if film.info.usage == "cine" { 50.0 } else { 65.0 };
            assert_eq!(pipeline.params.film_render.halation.halation_first_sigma_um, [radius; 3]);
            assert_eq!(pipeline.stock_defaults(), Some(preset));
            if route == StockRoute::Slide
                && !matches!(stock, "fujifilm_provia_100f" | "fujifilm_velvia_100")
            {
                let plain = Pipeline::new_with_spectral(film.clone(), paper.clone(), params, &data)
                    .unwrap();
                assert_eq!(
                    serde_json::to_value(&pipeline.params.film_render.dir_couplers).unwrap(),
                    serde_json::to_value(&plain.params.film_render.dir_couplers).unwrap(),
                    "{stock} must retain its ordinary slide DIR defaults"
                );
            }
        }
    }
    assert_eq!(profiles_checked, 36);
    assert_eq!(slides_checked, 5);
}

#[test]
fn cinema_prints_use_the_film_reference_and_print_viewing_lights() {
    use spektrafilm_gpu::cpu_backend::CpuBackend;
    use spektrafilm_math::{image::ImageBuf, precision::{from_f64, srgb_decode}};

    // Full-tier SpektraLab Windows 1.3.4-beta.3, native Vulkan, shared
    // linear ProPhoto TIFF. Each cross-pair independently exercises one lamp.
    let mut params = RuntimeParams::default();
    params.camera.auto_exposure = false;
    params.film_render.grain.active = false;
    params.film_render.halation.active = false;
    params.film_render.halation.boost_ev = 0.0;
    params.film_render.dir_couplers.active = false;
    params.print_render.glare.active = false;
    let level = srgb_decode(from_f64(118.0 / 255.0));
    for (film, paper, reference) in [
        ("kodak_vision3_500t", "kodak_portra_endura", [115.70, 116.25, 118.56]),
        ("kodak_portra_400", "kodak_2383", [113.63, 115.36, 118.44]),
        ("kodak_vision3_200t", "kodak_2393", [109.31, 110.63, 112.50]),
    ] {
        let pipeline = Pipeline::new_with_spectral(
            profile::load_profile_by_name(&data_dir(), film).unwrap(),
            profile::load_profile_by_name(&data_dir(), paper).unwrap(),
            params.clone(), &data_dir(),
        ).unwrap();
        let out = pipeline.try_process(ImageBuf::from_data(1, 1, vec![level; 3]), &CpuBackend).unwrap();
        for (value, target) in out.data.iter().zip(reference) {
            assert!((*value as f64 * 255.0 - target).abs() < 0.25, "{film}/{paper}: {:?}", out.data);
        }
    }
}

#[test]
fn physical_cinema_defaults_preserve_both_daylight_substitutions() {
    use spektrafilm_gpu::cpu_backend::CpuBackend;
    use spektrafilm_math::{image::ImageBuf, precision::from_f64};

    let stock = "kodak_vision3_500t";
    let film = profile::load_profile_by_name(&data_dir(), stock).unwrap();
    let paper = profile::load_profile_by_name(&data_dir(), "kodak_2383").unwrap();
    let mut p = parameters(StockRoute::Paper);
    p.camera.auto_exposure = false;
    p.film_render.grain.active = false;
    p.print_render.glare.active = false;
    let expected = reference_pipeline(stock, StockRoute::Paper, film.clone(), paper.clone(), p.clone());
    let actual = Pipeline::new_with_spectral_and_stock_defaults(
        film, paper, p, &data_dir(), StockDefaults::Physical(StockRoute::Paper),
    ).unwrap();
    let input = ImageBuf::from_data(2, 1, [0.184, 0.184, 0.184, 0.08, 0.26, 0.72]
        .into_iter().map(from_f64).collect());
    assert_eq!(
        actual.try_process(input.clone(), &CpuBackend).unwrap().data,
        expected.try_process(input, &CpuBackend).unwrap().data,
    );
}
