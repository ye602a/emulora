use spektrafilm_core::{
    neutral_filters::NeutralFilters, params::RuntimeParams, pipeline::Pipeline, profile,
};
use spektrafilm_gpu::cpu_backend::CpuBackend;
use spektrafilm_math::image::ImageBuf;
use std::path::Path;

#[test]
fn spectral_refresh_matches_fresh_print_and_rejects_stale_calibration() {
    let data = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../data");
    let film = profile::load_profile_by_name(&data, "kodak_portra_400").unwrap();
    let print = profile::load_profile_by_name(&data, "kodak_portra_endura").unwrap();
    let build =
        |params| Pipeline::new_with_spectral(film.clone(), print.clone(), params, &data).unwrap();
    let mut p = RuntimeParams::default();
    p.camera.auto_exposure = false;
    p.enlarger.print_exposure_compensation = false;
    p.film_render.grain.active = false;
    p.print_render.glare.active = false;
    p.settings.use_enlarger_lut = false;
    p.settings.use_scanner_lut = false;
    let base = build(p.clone());

    for edit in [
        (|p: &mut RuntimeParams| p.film_render.density_curve_gamma = 1.2) as fn(&mut RuntimeParams),
        |p| p.settings.rgb_to_raw_method = "mallett2019".into(),
        |p| p.film_render.development_time = Some(7.0),
        |p| p.print_render.development_time = Some(2.0),
        |p| p.enlarger.preflash_exposure = 0.125,
        |p| p.enlarger.y_filter_shift += 2.0,
        |p| p.settings.neutral_print_filters_from_database = false,
        |p| p.io.input_cctf_decoding = true,
    ] {
        let mut changed = p.clone();
        edit(&mut changed);
        assert!(base.refreshed(changed).is_none());
    }
    let mut gamma = p.clone();
    gamma.film_render.density_curve_gamma = 1.2;
    assert_ne!(
        base.print_exposure_factor(),
        build(gamma).print_exposure_factor()
    );

    p.camera.exposure_compensation_ev = 0.7;
    p.enlarger.m_filter_neutral = -20.0; // This stock/paper pair has a database row.
    let warm = base.refreshed(p.clone()).unwrap();
    let cold = build(p.clone());
    assert_eq!(
        warm.params.enlarger.m_filter_neutral,
        cold.params.enlarger.m_filter_neutral
    );
    assert_ne!(warm.params.enlarger.m_filter_neutral, -20.0);
    let patch = ImageBuf::from_data(4, 4, [0.35, 0.18, 0.07].repeat(16));
    let a = warm.try_process(patch.clone(), &CpuBackend).unwrap();
    let b = cold.try_process(patch, &CpuBackend).unwrap();
    assert_eq!(a.data, b.data);
    p.enlarger.print_exposure_compensation = true;
    let compensated = build(p.clone());
    p.camera.exposure_compensation_ev += 1.0;
    assert!(compensated.refreshed(p.clone()).is_none());
    p.settings.neutral_print_filters_from_database = false;
    let manual = build(p.clone());
    p.enlarger.m_filter_neutral += 1.0;
    assert!(manual.refreshed(p).is_none());
}

#[test]
fn glare_edits_follow_scan_material() {
    let mut p = RuntimeParams::default();
    p.io.scan_film = true;
    p.scan_glare_mut().percent = 0.13;
    p.io.scan_film = false;
    p.scan_glare_mut().percent = 0.04;
    p.io.scan_film = true;
    assert_eq!(p.scan_glare_mut().percent, 0.13);
    assert_eq!(p.print_render.glare.percent, 0.04);
}

#[test]
fn absent_neutral_table_allows_manual_values_but_invalid_table_reports_its_path() {
    let dir = std::env::temp_dir().join(format!("spektrafilm-neutral-test-{}", std::process::id()));
    let filters = dir.join("filters");
    std::fs::create_dir_all(&filters).unwrap();
    assert!(
        NeutralFilters::load(&dir)
            .unwrap()
            .lookup("paper", "light", "film")
            .is_none()
    );
    let file = filters.join("neutral_print_filters.json");
    std::fs::write(&file, r#"{"paper":{"light":{"film":[0,10]}}}"#).unwrap();
    let error = NeutralFilters::load(&dir).unwrap_err();
    std::fs::remove_file(&file).unwrap();
    std::fs::remove_dir(filters).unwrap();
    std::fs::remove_dir(dir).unwrap();
    assert!(error.contains(file.to_str().unwrap()));
}
