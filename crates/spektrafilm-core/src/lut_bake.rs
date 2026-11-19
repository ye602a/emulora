//! Sample the standalone spectral photo transform on a red-fastest unit RGB grid.
use std::path::Path;

use spektrafilm_gpu::{ComputeBackend, FilmRenderOptions};
use spektrafilm_math::{image::ImageBuf, precision::Scalar};

use crate::{params::RuntimeParams, pipeline::Pipeline, profile::Profile};

/// Linear input in the selected primaries, clipped linear or encoded sRGB output.
/// Only the deterministic, pointwise part of the requested print or film scan
/// can travel in a cube. Fixed exposure, preflash, local DIR and print chemistry
/// enter the normal spectral constructor before it calibrates the stocks.
pub fn bake_3d_lut(
    film: Profile,
    print: Profile,
    mut params: RuntimeParams,
    data_dir: &Path,
    size: u32,
    backend: &dyn ComputeBackend,
) -> Result<ImageBuf, String> {
    if !(2..=65).contains(&size) {
        return Err("3D LUT size must be between 2 and 65".into());
    }
    let io = &params.io;
    if io.output_color_space != "sRGB" {
        return Err(format!(
            "3D LUT output requires sRGB primaries, got {}",
            io.output_color_space
        ));
    }
    pointwise_params(&mut params);
    let pipeline = Pipeline::new_with_spectral(film, print, params, data_dir)?;
    let mut grid = ImageBuf::new(size * size, size);
    let denominator = (size - 1) as Scalar;
    for (index, rgb) in grid.pixels_mut().enumerate() {
        let i = index as u32;
        let coordinates = [i % size, (i / size) % size, i / (size * size)];
        rgb.copy_from_slice(&coordinates.map(|v| v as Scalar / denominator));
    }
    render_probe(&pipeline, grid, backend)
}

/// Isolate deterministic probes from the host's retained negative and work planes.
pub(crate) fn render_probe(
    pipeline: &Pipeline,
    grid: ImageBuf,
    backend: &dyn ComputeBackend,
) -> Result<ImageBuf, String> {
    let output = if backend.is_gpu() {
        let options = FilmRenderOptions {
            isolate_photo_caches: true,
            ..Default::default()
        };
        pipeline
            .process_resident_with_options(&grid, backend, options)
            .map_err(|e| e.to_string())?
            .ok_or("no resident spectral probe route on the selected backend")?
    } else {
        pipeline
            .try_process(grid, backend)
            .map_err(|e| e.to_string())?
    };
    if output.data.iter().any(|v| !v.is_finite()) {
        return Err("non-finite spectral probe sample".into());
    }
    Ok(output)
}

pub(crate) fn pointwise_params(params: &mut RuntimeParams) {
    params.camera.auto_exposure = false;
    params.camera.diffusion_filter.active = false;
    params.camera.lens_blur_um = 0.0;
    params.film_render.halation.active = false;
    // Highlight reconstruction uses the frame maximum even with halation off.
    params.film_render.halation.boost_ev = 0.0;
    params.film_render.grain.active = false;
    params.film_render.glare.active = false;
    params.print_render.glare.active = false;
    let dir = &mut params.film_render.dir_couplers;
    dir.diffusion_size_um = 0.0;
    dir.diffusion_tail_um = 0.0;
    dir.diffusion_tail_weight = 0.0;
    params.enlarger.diffusion_filter.active = false;
    params.enlarger.lens_blur = 0.0;
    params.scanner.lens_blur = 0.0;
    params.scanner.unsharp_mask = [0.0; 2];
    params.border_135 = None;
    params.io.crop = false;
    params.io.upscale_factor = 1.0;
    // Sample the spectral integrations themselves, avoiding a second layer of
    // interpolation when the caller uses the optional CPU acceleration LUTs.
    params.settings.use_enlarger_lut = false;
    params.settings.use_scanner_lut = false;
}
