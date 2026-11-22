/// Spectral upsampling service: loads the Hanatos2025 spectra LUT and computes
/// the TC LUT for a given film stock's sensitivity.
///
/// Port of Python `compute_hanatos2025_tc_lut` and `_load_hanatos2025_spectra_lut`.
use std::path::Path;

use spektrafilm_math::npy;
use spektrafilm_math::spectral::{self, N_WAVELENGTHS, TcLut};

// Pull in BLAS for the spectra→tc_lut contraction (matches Python's
// opt_einsum + numpy summation pattern bit-for-bit on macOS Accelerate).
#[cfg(not(target_os = "windows"))]
#[allow(unused_imports)]
use blas_src as _;

fn dgemm_no_transpose(a: &[f64], b: &[f64], c: &mut [f64], m: usize, n: usize, k: usize) {
    assert_eq!(a.len(), m * k, "dgemm_no_transpose: a.len() != m*k");
    assert_eq!(b.len(), k * n, "dgemm_no_transpose: b.len() != k*n");
    assert_eq!(c.len(), m * n, "dgemm_no_transpose: c.len() != m*n");

    #[cfg(not(target_os = "windows"))]
    unsafe {
        cblas::dgemm(
            cblas::Layout::RowMajor,
            cblas::Transpose::None,
            cblas::Transpose::None,
            m as i32,
            n as i32,
            k as i32,
            1.0,
            a,
            k as i32,
            b,
            n as i32,
            0.0,
            c,
            n as i32,
        );
    }

    #[cfg(target_os = "windows")]
    {
        for row in 0..m {
            for col in 0..n {
                let mut sum = 0.0f64;
                for kk in 0..k {
                    sum += a[row * k + kk] * b[kk * n + col];
                }
                c[row * n + col] = sum;
            }
        }
    }
}

/// Load the spectra LUT from the .npy file.
/// Shape: (size, size, 81) — maps tc coordinates → 81-wavelength spectra.
pub fn load_spectra_lut(data_dir: &Path) -> Result<SpectraLut, String> {
    let path = data_dir
        .join("luts")
        .join("spectral_upsampling")
        .join("irradiance_xy_tc.npy");

    let file = std::fs::File::open(&path)
        .map_err(|e| format!("opening spectra LUT {}: {e}", path.display()))?;
    let reader = std::io::BufReader::new(file);

    let (shape, data) =
        npy::load_npy_f32(reader).map_err(|e| format!("loading spectra LUT: {e}"))?;

    if shape.len() != 3 || shape[2] != N_WAVELENGTHS {
        return Err(format!(
            "spectra LUT shape mismatch: expected (N, N, {N_WAVELENGTHS}), got {shape:?}"
        ));
    }
    if shape[0] != shape[1] {
        return Err(format!(
            "spectra LUT must be square, got {}x{}",
            shape[0], shape[1]
        ));
    }

    Ok(SpectraLut {
        size: shape[0],
        n_wavelengths: shape[2],
        data,
    })
}

/// Load the arctic2026beta04 effective-reflectance LUT.
///
/// Shape: (size, size, 81). The table is a D65-recovered unit-bright
/// reflectance surface in triangular coordinates. At render time we relight it
/// by the film reference illuminant before integrating against film
/// sensitivity.
pub fn load_arctic2026beta04_lut(data_dir: &Path) -> Result<SpectraLut, String> {
    let path = data_dir
        .join("luts")
        .join("spectral_upsampling")
        .join("arctic2026beta04_reflectance_xy_tc.npy");

    let file = std::fs::File::open(&path)
        .map_err(|e| format!("opening arctic2026beta04 LUT {}: {e}", path.display()))?;
    let reader = std::io::BufReader::new(file);

    let (shape, data) =
        npy::load_npy_f32(reader).map_err(|e| format!("loading arctic2026beta04 LUT: {e}"))?;

    if shape.len() != 3 || shape[2] != N_WAVELENGTHS {
        return Err(format!(
            "arctic2026beta04 LUT shape mismatch: expected (N, N, {N_WAVELENGTHS}), got {shape:?}"
        ));
    }
    if shape[0] != shape[1] {
        return Err(format!(
            "arctic2026beta04 LUT must be square, got {}x{}",
            shape[0], shape[1]
        ));
    }

    Ok(SpectraLut {
        size: shape[0],
        n_wavelengths: shape[2],
        data,
    })
}

pub struct SpectraLut {
    pub size: usize,
    pub n_wavelengths: usize,
    /// Flat: [size * size * n_wavelengths]
    pub data: Vec<f32>,
}

impl SpectraLut {
    /// Get spectrum at grid position (i, j). Returns slice of n_wavelengths.
    pub fn spectrum(&self, i: usize, j: usize) -> &[f32] {
        let start = (i * self.size + j) * self.n_wavelengths;
        &self.data[start..start + self.n_wavelengths]
    }
}

/// Compute the TC LUT for a given film stock.
///
/// Port of Python `compute_hanatos2025_tc_lut`:
///   tc_lut[i][j][c] = sum_wl( spectra_lut[i][j][wl] * sensitivity[wl][c] )
///
/// The result maps tc coordinates → per-channel film raw exposure,
/// normalized so that the reference illuminant midgray produces balanced
/// exposure on the green channel (matching Python's `raw / raw_midgray[1]`).
pub fn compute_tc_lut(spectra_lut: &SpectraLut, sensitivity: &[[f64; 3]]) -> TcLut {
    let size = spectra_lut.size;
    let n_wl = spectra_lut.n_wavelengths.min(sensitivity.len());
    let channels = 3;

    let mut data = vec![0.0f64; size * size * channels];

    for i in 0..size {
        for j in 0..size {
            let spectrum = spectra_lut.spectrum(i, j);
            let mut raw = [0.0f64; 3];
            for wl in 0..n_wl {
                for c in 0..3 {
                    raw[c] += spectrum[wl] as f64 * sensitivity[wl][c];
                }
            }
            let base = (i * size + j) * channels;
            data[base] = raw[0];
            data[base + 1] = raw[1];
            data[base + 2] = raw[2];
        }
    }

    TcLut {
        size,
        channels,
        data,
    }
}

/// Build a TC→film-raw LUT from an effective-reflectance surface by relighting
/// each reflectance with the film/reference illuminant first:
///
/// `raw[i,j,c] = sum_wl reflectance[i,j,wl] * illuminant[wl] * sensitivity[wl,c]`
pub fn compute_reflectance_tc_lut(
    reflectance_lut: &SpectraLut,
    sensitivity: &[[f64; 3]],
    illuminant: &[f64],
) -> TcLut {
    let size = reflectance_lut.size;
    let n_wl = reflectance_lut
        .n_wavelengths
        .min(sensitivity.len())
        .min(illuminant.len());
    let channels = 3;
    let mut data = vec![0.0f64; size * size * channels];

    for i in 0..size {
        for j in 0..size {
            let spectrum = reflectance_lut.spectrum(i, j);
            let mut raw = [0.0f64; 3];
            for wl in 0..n_wl {
                let lit = spectrum[wl] as f64 * illuminant[wl];
                for c in 0..3 {
                    raw[c] += lit * sensitivity[wl][c];
                }
            }
            let base = (i * size + j) * channels;
            data[base] = raw[0];
            data[base + 1] = raw[1];
            data[base + 2] = raw[2];
        }
    }

    TcLut {
        size,
        channels,
        data,
    }
}

/// Compute the TC LUT with spectral bandpass window (erf4 model).
///
/// The window models the camera's UV/IR sensitivity cutoff. It's applied
/// to the sensitivity before integration, with normalization to preserve
/// white balance.
///
/// `window_params`: (c_uv, sigma_uv, c_ir, sigma_ir) — erf4 bandpass parameters.
/// `illuminant`: reference illuminant SPD for normalization (f64 to
/// match Python parity — f32 illuminants drop ~7 decimal places per
/// sample, accumulating ~1e-9 error per LUT cell).
pub fn compute_tc_lut_with_window(
    spectra_lut: &SpectraLut,
    sensitivity: &[[f64; 3]],
    window_params: &[f64],
    illuminant: &[f64],
) -> TcLut {
    compute_camera_lut_with_window(
        spectra_lut,
        sensitivity,
        sensitivity,
        window_params,
        illuminant,
    )
}

pub(crate) fn compute_camera_lut_with_window(
    spectra_lut: &SpectraLut,
    sensitivity: &[[f64; 3]],
    bare_film: &[[f64; 3]],
    window_params: &[f64],
    illuminant: &[f64],
) -> TcLut {
    let n_wl = spectra_lut
        .n_wavelengths
        .min(sensitivity.len())
        .min(N_WAVELENGTHS);

    // Compute erf4 bandpass window in f64
    let window = if window_params.len() >= 4 {
        eval_erf4_bandpass(window_params)
    } else {
        vec![[1.0f64; 3]; N_WAVELENGTHS]
    };

    // Window normalization — Python:
    //   norm_num = np.sum(sens * illuminant[:, None] * window, axis=0)
    //   norm_den = np.sum(sens * illuminant[:, None], axis=0)
    //   normalization = norm_num / norm_den
    //   window /= normalization
    // Multiplication order per cell: `(sens * illuminant) * window`
    // — left-to-right. The 81-wavelength reduction in numpy uses
    // pairwise summation; for 81 elements that's recursive halving.
    // We replicate it via `pairwise_sum_f64`.
    let mut num_per_wl = [
        Vec::<f64>::with_capacity(n_wl),
        Vec::with_capacity(n_wl),
        Vec::with_capacity(n_wl),
    ];
    let mut den_per_wl = [
        Vec::<f64>::with_capacity(n_wl),
        Vec::with_capacity(n_wl),
        Vec::with_capacity(n_wl),
    ];
    for wl in 0..n_wl {
        for c in 0..3 {
            let si = bare_film[wl][c] * illuminant[wl];
            num_per_wl[c].push(si * window[wl][c]);
            den_per_wl[c].push(si);
        }
    }
    let mut window_normalized = window.clone();
    for c in 0..3 {
        let num_c = pairwise_sum_f64(&num_per_wl[c]);
        let den_c = pairwise_sum_f64(&den_per_wl[c]);
        if num_c > 1e-10 && den_c > 1e-10 {
            let normalization_c = num_c / den_c;
            for wl in 0..n_wl {
                window_normalized[wl][c] = window[wl][c] / normalization_c;
            }
        }
    }

    // Compute raw LUT via BLAS dgemm to match Python's
    // `opt_einsum.contract('ijl,lm->ijm', spectra, sens*window)`.
    // numpy/opt_einsum routes this through GEMM with pairwise
    // accumulation, so a hand-rolled left-to-right loop is off by
    // 1-2 ULP per cell. Going through dgemm matches bit-for-bit.
    let size = spectra_lut.size;
    let channels = 3;
    let n_pix = size * size;
    let mut spec_f64 = vec![0.0f64; n_pix * n_wl];
    for i in 0..n_pix {
        let src =
            &spectra_lut.data[i * spectra_lut.n_wavelengths..i * spectra_lut.n_wavelengths + n_wl];
        let dst = &mut spec_f64[i * n_wl..(i + 1) * n_wl];
        for (d, s) in dst.iter_mut().zip(src.iter()) {
            *d = *s as f64;
        }
    }
    // Build sens*window as (n_wl × 3) row-major.
    let mut sw_flat = vec![0.0f64; n_wl * 3];
    for wl in 0..n_wl {
        for c in 0..3 {
            sw_flat[wl * 3 + c] = sensitivity[wl][c] * window_normalized[wl][c];
        }
    }
    let mut data = vec![0.0f64; n_pix * channels];
    dgemm_no_transpose(&spec_f64, &sw_flat, &mut data, n_pix, channels, n_wl);

    TcLut {
        size,
        channels,
        data,
    }
}

/// Compute the midgray normalization factor for Hanatos2025.
///
/// Port of Python: `raw_midgray = einsum('k,km->m', illuminant * 0.184, sensitivity)`
/// then normalize by `raw_midgray[1]` (green channel).
pub fn compute_midgray_normalization(sensitivity: &[[f64; 3]], illuminant: &[f32]) -> f64 {
    let n_wl = sensitivity.len().min(illuminant.len());
    let mut raw_midgray = [0.0f64; 3];
    for wl in 0..n_wl {
        for c in 0..3 {
            raw_midgray[c] += illuminant[wl] as f64 * 0.184 * sensitivity[wl][c];
        }
    }
    if raw_midgray[1] > 1e-10 {
        1.0 / raw_midgray[1]
    } else {
        1.0
    }
}

/// Public re-export: pairwise sum for f64 spectra/wavelength reductions.
/// See `pairwise_sum_f64` doc below.
pub fn pairwise_sum_f64_pub(xs: &[f64]) -> f64 {
    pairwise_sum_f64(xs)
}

/// Pairwise (recursive-halving) f64 summation. Matches numpy's
/// `np.add.reduce` reduction pattern, which is what `np.sum` uses for
/// 1-D arrays. For 81 elements this is recursive halving; the result
/// differs from a naive left-to-right sum by 1-2 ULPs but matches numpy
/// bit-for-bit.
fn pairwise_sum_f64(xs: &[f64]) -> f64 {
    match xs.len() {
        0 => 0.0,
        1 => xs[0],
        2 => xs[0] + xs[1],
        n => {
            let mid = n / 2;
            pairwise_sum_f64(&xs[..mid]) + pairwise_sum_f64(&xs[mid..])
        }
    }
}

/// Evaluate the erf4 spectral bandpass window.
/// params: (c_uv, sigma_uv, c_ir, sigma_ir)
/// Returns [81][3] window values (same for all 3 channels in erf4 model).
fn eval_erf4_bandpass(params: &[f64]) -> Vec<[f64; 3]> {
    let sqrt2 = std::f64::consts::SQRT_2;
    let c_uv = params[0];
    let sigma_uv = params[1];
    let c_ir = params[2];
    let sigma_ir = params[3];

    let mut window = vec![[0.0f64; 3]; N_WAVELENGTHS];
    for (i, response) in window.iter_mut().enumerate() {
        let wl = spectral::WAVELENGTH_MIN as f64 + (i as f64) * spectral::WAVELENGTH_STEP as f64;
        let edge_uv = 0.5 * (1.0 + erf((wl - c_uv) / (sigma_uv * sqrt2)));
        let edge_ir = 0.5 * (1.0 - erf((wl - c_ir) / (sigma_ir * sqrt2)));
        let w = edge_uv * edge_ir;
        *response = [w, w, w];
    }
    window
}

/// The unwarped poly4 fit in the shipped profiles. Grid axis zero is tc.x.
pub(crate) fn apply_hanatos_surface(lut: &mut TcLut, rows: &[Vec<f64>], white: (f64, f64)) {
    let scale = (lut.size - 1) as f64;
    for (cell, raw) in lut.data.as_chunks_mut::<3>().0.iter_mut().enumerate() {
        let x = (cell / lut.size) as f64 / scale - white.0;
        let y = (cell % lut.size) as f64 / scale - white.1;
        let (xx, yy) = (x * x, y * y);
        // Upstream omits the fitted constant to anchor the reference white.
        let basis = [
            x,
            y,
            xx,
            yy,
            x * y,
            xx * x,
            yy * y,
            xx * y,
            x * yy,
            xx * xx,
            yy * yy,
            xx * x * y,
            xx * yy,
            x * yy * y,
        ];
        for (channel, coefficients) in raw.iter_mut().zip(rows) {
            let z: f64 = basis
                .iter()
                .zip(&coefficients[1..])
                .map(|(a, b)| a * b)
                .sum();
            let stops = z / (0.5 * z).hypot(1.0);
            *channel *= stops.exp2();
        }
    }
}

/// Apply the camera bandpass to stock sensitivity with optional normalization of each
/// emulsion's integrated exposure to its own reference white.
pub(crate) fn apply_camera_cuts(
    response: &mut [[f64; 3]],
    wavelengths: &[f64],
    white: &[f64],
    uv: [f32; 3],
    ir: [f32; 3],
    normalize: bool,
) -> Result<(), String> {
    if wavelengths.len() != white.len()
        || response.len() != white.len()
        || wavelengths
            .iter()
            .enumerate()
            .any(|(i, &nm)| nm != 380. + 5. * i as f64)
    {
        return Err("UV/IR cuts require the 380-780 nm film grid at 5 nm intervals".into());
    }
    let transmission = |nm: f64, cut: [f32; 3], sign: f64| {
        let [amount, center, width] = cut.map(f64::from);
        if amount == 0. {
            return 1.;
        }
        1. - amount * 0.5 * libm::erfc(sign * (nm - center) / width)
    };
    let mut before = [0.; 3];
    let mut after = [0.; 3];
    for ((row, &nm), &light) in response.iter_mut().zip(wavelengths).zip(white) {
        let pass = transmission(nm, uv, 1.) * transmission(nm, ir, -1.);
        for c in 0..3 {
            before[c] += row[c] * light;
            row[c] *= pass;
            after[c] += row[c] * light;
        }
    }
    let gain: [f64; 3] = std::array::from_fn(|c| before[c] / after[c]);
    if gain.iter().any(|g| !g.is_finite() || *g <= 0.) {
        return Err("UV/IR cuts leave no usable reference exposure in a film channel".into());
    }
    if normalize {
        for row in response {
            for c in 0..3 {
                row[c] *= gain[c];
            }
        }
    }
    Ok(())
}

/// f64 erf via libm — matches scipy.special.erf at full f64 precision.
#[inline]
fn erf(x: f64) -> f64 {
    libm::erf(x)
}

#[cfg(test)]
mod camera_cut_tests {
    use super::apply_camera_cuts;

    #[test]
    fn camera_bandpass_preserves_reference_exposure_and_matches_erf_edges() {
        let wl: Vec<_> = (0..81).map(|i| 380. + 5. * f64::from(i)).collect();
        let white: Vec<_> = wl.iter().map(|nm| nm / 500.).collect();
        let bare: Vec<_> = wl.iter().map(|nm| [1., nm / 500., 500. / nm]).collect();
        let integral = |rows: &[[f64; 3]], c: usize| -> f64 {
            rows.iter()
                .zip(&white)
                .map(|(row, light)| row[c] * light)
                .sum()
        };
        let mut response = bare.clone();
        apply_camera_cuts(
            &mut response,
            &wl,
            &white,
            [1., 430., 10.],
            [0.5, 650., 10.],
            true,
        )
        .unwrap();
        for c in 0..3 {
            assert!((integral(&response, c) - integral(&bare, c)).abs() < 1e-10);
            let t = |i: usize| response[i][c] / bare[i][c];
            // The middle of the passband has unit transmission before normalization.
            assert!((t(10) / t(32) - 0.5).abs() < 1e-12);
            assert!((t(12) / t(32) - 0.9213503964748575).abs() < 1e-12);
            assert!((t(54) / t(32) - 0.75).abs() < 1e-12);
            assert!((t(80) / t(32) - 0.5).abs() < 1e-12);
        }
    }
}

#[cfg(test)]
mod surface_tests {
    use super::*;
    #[test]
    fn portra_poly4_matches_python_on_asymmetric_cells() {
        let data = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../data");
        let film = crate::profile::load_profile_by_name(&data, "kodak_portra_400").unwrap();
        let mut lut = TcLut {
            size: 5,
            channels: 3,
            data: vec![1.; 75],
        };
        let white = spektrafilm_math::spectral::xy_to_tc(0.30, 0.35);
        apply_hanatos_surface(
            &mut lut,
            &film.data.hanatos2025_adaptation_surface_params,
            white,
        );
        // Python eval_poly4_log_exposure_surface, on the identical 5x5 grid.
        for (i, j, expected) in [
            (
                1,
                3,
                [1.0635566571931094, 0.98604838101348, 0.972682698898348],
            ),
            (
                3,
                1,
                [1.4478405338805653, 0.9345969923697535, 0.9117978724158349],
            ),
            (
                0,
                0,
                [3.824774280179037, 2.2461086902572447, 0.8649790749012143],
            ),
        ] {
            for (c, v) in expected.into_iter().enumerate() {
                assert!((lut.data[(i * 5 + j) * 3 + c] - v).abs() < 1e-12);
            }
        }
    }
}
