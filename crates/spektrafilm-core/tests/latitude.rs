use spektrafilm_core::{
    params::RuntimeParams,
    pipeline::Pipeline,
    profile::load_profile_by_name,
    stock_setup::{StockDefaults, StockRoute},
};
use spektrafilm_gpu::{
    FilmRenderOptions, SourceIdentity, cpu_backend::CpuBackend, wgpu_backend::WgpuBackend,
};
use spektrafilm_math::{image::ImageBuf, precision::from_f64};
use std::path::Path;

#[test]
fn host_pre_gain_and_manual_film_ev_are_each_counted_once() {
    let data = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../data");
    let film = load_profile_by_name(&data, "kodak_portra_400").unwrap();
    let paper = load_profile_by_name(&data, "kodak_portra_endura").unwrap();
    let mut p = RuntimeParams::default();
    p.camera.auto_exposure = false;
    p.enlarger.print_exposure_compensation = false;
    p.io.input_color_space = "sRGB".into();
    p.film_render.grain.active = false;
    p.film_render.halation.active = false;
    p.film_render.halation.boost_ev = 0.;
    p.print_render.glare.active = false;
    assert!(
        Pipeline::new(film.clone(), paper.clone(), p.clone())
            .measure_latitude(&CpuBackend)
            .is_err()
    );
    let build = |p| {
        Pipeline::new_with_spectral_and_stock_defaults(
            film.clone(),
            paper.clone(),
            p,
            &data,
            StockDefaults::Physical(StockRoute::Paper),
        )
        .unwrap()
    };
    let base = build(p.clone());
    let medium = base.measure_latitude(&CpuBackend).unwrap();
    let scene = ImageBuf::from_data(
        201,
        1,
        (0..201)
            .flat_map(|i| [from_f64(0.184 * 2_f64.powf((i as f64 - 100.) / 10.)); 3])
            .collect(),
    );
    let mut lifted = scene.clone();
    for v in &mut lifted.data {
        *v *= from_f64(2.0);
    }
    let pre = medium.place_scene(&lifted, &p).unwrap();
    assert_eq!(pre.meter_ev, 0.);
    p.camera.exposure_compensation_ev = 1.;
    let adjusted = build(p.clone());
    let shifted = adjusted.measure_latitude(&CpuBackend).unwrap();
    assert!((shifted.shadow_ev - medium.shadow_ev + 1.).abs() < 0.01);
    assert!((shifted.highlight_ev - medium.highlight_ev + 1.).abs() < 0.01);
    let manual = shifted.place_scene(&scene, &p).unwrap();
    assert_eq!(manual.counts.above_highlight, pre.counts.above_highlight);
    assert_eq!(manual.counts.below_shadow, pre.counts.below_shadow);
    let mut metered = p.clone();
    metered.camera.auto_exposure = true;
    metered.camera.auto_exposure_method = "balanced".into();
    assert!((shifted.place_scene(&lifted, &metered).unwrap().meter_ev + 1.).abs() < 0.01);
    p.io.scan_film = true;
    assert!(
        Pipeline::new_with_spectral(film, paper, p, &data)
            .unwrap()
            .measure_latitude(&CpuBackend)
            .is_err()
    );

    let Some(gpu) = WgpuBackend::new() else {
        assert!(std::env::var_os("SPEKTRAFILM_REQUIRE_GPU").is_none());
        return;
    };
    gpu.configure_negative_cache(true);
    gpu.configure_work_images(true);
    let render = || {
        base.process_resident_with_options(
            &scene,
            &gpu,
            FilmRenderOptions {
                source: Some(SourceIdentity(0x14)),
                ..Default::default()
            },
        )
        .unwrap()
        .unwrap()
    };
    let before = render();
    let stats = gpu.negative_cache_stats();
    let work = gpu.work_image_bytes();
    let measured = base.measure_latitude(&gpu).unwrap();
    assert!((measured.shadow_ev - medium.shadow_ev).abs() < 0.02);
    assert!((measured.highlight_ev - medium.highlight_ev).abs() < 0.02);
    assert_eq!(gpu.negative_cache_stats().stores, stats.stores);
    assert_eq!(gpu.work_image_bytes(), work);
    assert_eq!(render().data, before.data);
    assert_eq!(gpu.negative_cache_stats().hits, stats.hits + 1);
}
