//! Colour-negative readout for neutral optical printing. Adapted from SpektraLab's
//! negative_hue.cpp / negative_hue.metal (b51e672, GPL-3.0-or-later).
use crate::{
    gamut_compression::{mat_vec, oklab_to_xyz, xyz_to_oklab},
    profile::Profile,
};
use spektrafilm_gpu::NegativeHue;
use spektrafilm_math::{
    colorspace::{SRGB_TO_XYZ_F64, XYZ_TO_SRGB_F64},
    image::ImageBuf,
    precision::{Scalar, from_f64},
};
use std::collections::VecDeque;

const SHEET_HUES: usize = 180;
const CHROMA_STEPS: usize = 56;
const MAP_SIDE: usize = 160;
const LAYER_SAMPLES: usize = 1024;
const GAMUT_LEVELS: usize = 48;
const GAMUT_HUES: usize = 120;
const CALIBRATION_LIGHTNESS: f64 = 0.5687;

fn vlen(q: [f64; 2]) -> f64 {
    q[0].hypot(q[1])
}

pub(crate) fn sheet() -> (ImageBuf, Vec<[f64; 2]>, Vec<bool>) {
    let mut image = ImageBuf::new((1 + SHEET_HUES * CHROMA_STEPS) as u32, 1);
    let mut ab = Vec::with_capacity(1 + SHEET_HUES * CHROMA_STEPS);
    let mut valid = Vec::with_capacity(1 + SHEET_HUES * CHROMA_STEPS);
    for (i, p) in image.data.as_chunks_mut::<3>().0.iter_mut().enumerate() {
        let j = i.saturating_sub(1);
        let angle = (j % SHEET_HUES) as f64 * std::f64::consts::TAU / SHEET_HUES as f64;
        let c = if i == 0 {
            0.0
        } else {
            (j / SHEET_HUES + 1) as f64 * 0.0075
        };
        let pair = [c * angle.cos(), c * angle.sin()];
        let xyz = oklab_to_xyz([CALIBRATION_LIGHTNESS, pair[0], pair[1]]);
        let ok = xyz[0] > 0.0 && xyz[1] > 1e-6 && xyz[2] > 0.0;
        let rgb = if ok {
            mat_vec(&XYZ_TO_SRGB_F64, xyz)
        } else {
            [0.184; 3]
        };
        p.copy_from_slice(&rgb.map(from_f64));
        ab.push(pair);
        valid.push(ok);
    }
    (image, ab, valid)
}

// Visit grid samples inside a triangle. The gamut's hue axis wraps in the caller.
fn raster(p: [[f64; 2]; 3], mut emit: impl FnMut(i32, i32, [f64; 3])) {
    let [a, b, c] = p;
    let det = (b[0] - a[0]) * (c[1] - a[1]) - (c[0] - a[0]) * (b[1] - a[1]);
    if det.abs() < 1e-12 {
        return;
    }
    let bounds = |axis: usize| {
        (a[axis].min(b[axis]).min(c[axis]).ceil() as i32)
            ..=(a[axis].max(b[axis]).max(c[axis]).floor() as i32)
    };
    for x in bounds(0) {
        for y in bounds(1) {
            let dx = x as f64 - a[0];
            let dy = y as f64 - a[1];
            let u = (dx * (c[1] - a[1]) - (c[0] - a[0]) * dy) / det;
            let v = ((b[0] - a[0]) * dy - dx * (b[1] - a[1])) / det;
            if u >= -1e-7 && v >= -1e-7 && u + v <= 1.0000001 {
                emit(x, y, [1.0 - u - v, u, v]);
            }
        }
    }
}

fn layer(k: &NegativeHue, c: usize, d: f64) -> [f64; 2] {
    let x = ((d - k.lo[c] as f64) * k.scale[c] as f64).clamp(0.0, (LAYER_SAMPLES - 1) as f64);
    let j = (x as usize).min(LAYER_SAMPLES - 2);
    let t = x - j as f64;
    std::array::from_fn(|v| {
        let a = k.layers[j * 3 + c][v] as f64;
        a + t * (k.layers[(j + 1) * 3 + c][v] as f64 - a)
    })
}

pub(crate) fn prepare(
    film: &Profile,
    gamma: f64,
    developed: &ImageBuf,
    ab: &[[f64; 2]],
    valid: &[bool],
    paper_faces: &ImageBuf,
) -> Result<NegativeHue, String> {
    let mut k = NegativeHue::default();
    let x: Vec<_> = film.log_exposure_f64().iter().map(|v| v / gamma).collect();
    let curves =
        spektrafilm_model::density_curves::normalize_density_curves_f64(&film.density_curves_f64());
    if x.len() < 8 || x.len() != curves.len() || !x.windows(2).all(|w| w[1] > w[0]) {
        return Err("negative hue requires increasing film exposure knots".into());
    }
    k.layers.resize(LAYER_SAMPLES * 3, [0.0; 2]);
    for c in 0..3 {
        if !curves.iter().any(|row| row[c].is_finite() && row[c] > 0.0) {
            return Err("negative hue requires three non-flat film layers".into());
        }
        let mut d = Vec::with_capacity(x.len());
        for row in &curves {
            let prev = d.last().copied().unwrap_or(0.0);
            let v = if row[c].is_finite() { row[c] } else { prev };
            d.push(if d.is_empty() { v } else { v.max(prev + 1e-7) });
        }
        // Local slope estimates reliability near a layer's toe and shoulder.
        let span = (x.len() / 64).max(2);
        let slope: Vec<_> = (0..x.len())
            .map(|i| {
                let a = i.saturating_sub(span);
                let b = (i + span).min(x.len() - 1);
                (d[b] - d[a]) / (x[b] - x[a])
            })
            .collect();
        let peak = slope.iter().copied().fold(0.0, f64::max);
        let mut straight: Vec<_> = slope.iter().copied().filter(|v| *v >= peak * 0.5).collect();
        straight.sort_by(f64::total_cmp);
        let reference = straight[straight.len() / 2];
        let unreliability =
            |i: usize| (reference / slope[i].max(reference / 101.0) - 1.0).clamp(0.0, 100.0);
        k.lo[c] = d[0] as f32;
        k.scale[c] = ((LAYER_SAMPLES - 1) as f64 / (d[x.len() - 1] - d[0])) as f32;
        for j in 0..LAYER_SAMPLES {
            let value = d[0] + j as f64 * (d[x.len() - 1] - d[0]) / (LAYER_SAMPLES - 1) as f64;
            let i = d.partition_point(|v| *v <= value).clamp(1, d.len() - 1) - 1;
            let t = (value - d[i]) / (d[i + 1] - d[i]);
            k.layers[3 * j + c] = [
                (x[i] + t * (x[i + 1] - x[i])) as f32,
                (unreliability(i) + t * (unreliability(i + 1) - unreliability(i))) as f32,
            ];
        }
    }
    // Calibrate the inverse in normalized layer-exposure ratios. Where the
    // film maps two hues to the same ratio, keep the less saturated reading.
    let mut coord = Vec::with_capacity(ab.len());
    for (i, px) in developed.data.as_chunks::<3>().0.iter().enumerate() {
        let e: [f64; 3] = std::array::from_fn(|c| 10.0_f64.powf(layer(&k, c, px[c] as f64)[0]));
        if i == 0 {
            k.neutral = e.map(|v| v as f32);
        }
        let e: [f64; 3] = std::array::from_fn(|c| e[c] / k.neutral[c] as f64);
        let scale = (MAP_SIDE - 1) as f64 / e.iter().sum::<f64>();
        coord.push([e[0] * scale, e[1] * scale]);
    }
    k.map.resize(MAP_SIDE * MAP_SIDE, [0.0; 2]);
    let mut best = vec![f64::INFINITY; MAP_SIDE * MAP_SIDE];
    let idx = |h: usize, c: usize| {
        if c == 0 {
            0
        } else {
            1 + (c - 1) * SHEET_HUES + h % SHEET_HUES
        }
    };
    for c in 0..CHROMA_STEPS {
        for h in 0..SHEET_HUES {
            let p = idx(h, c);
            let q = idx(h + 1, c);
            let r = idx(h, c + 1);
            let s = idx(h + 1, c + 1);
            for tri in [[p, r, s], [p, s, q]] {
                if !tri.iter().all(|&i| valid[i]) {
                    continue;
                }
                raster(tri.map(|i| coord[i]), |x, y, w| {
                    let cell = x as usize * MAP_SIDE + y as usize;
                    let v = std::array::from_fn(|ch| (0..3).map(|j| w[j] * ab[tri[j]][ch]).sum());
                    let length = vlen(v);
                    if length < best[cell] {
                        best[cell] = length;
                        k.map[cell] = v.map(|v| v as f32);
                    }
                });
            }
        }
    }
    let mut queue: VecDeque<_> = (0..MAP_SIDE * MAP_SIDE)
        .filter(|&i| best[i].is_finite())
        .collect();
    if queue.is_empty() {
        return Err("negative hue has no usable exposure map".into());
    }
    // Extend the closest covered cells into unreachable exposure ratios.
    while let Some(i) = queue.pop_front() {
        let (x, y) = ((i / MAP_SIDE) as i32, (i % MAP_SIDE) as i32);
        for (x, y) in [(x - 1, y), (x + 1, y), (x, y - 1), (x, y + 1)] {
            if (0..MAP_SIDE as i32).contains(&x) && (0..MAP_SIDE as i32).contains(&y) {
                let j = x as usize * MAP_SIDE + y as usize;
                if !best[j].is_finite() {
                    best[j] = best[i];
                    k.map[j] = k.map[i];
                    queue.push_back(j);
                }
            }
        }
    }
    build_gamut(&mut k, paper_faces)?;
    Ok(k)
}

// Six paper density-box faces, with white and black first for the lightness bounds.
pub(crate) fn faces(curves: &[[f64; 3]]) -> Result<ImageBuf, String> {
    let lo: [f64; 3] =
        std::array::from_fn(|c| curves.iter().map(|v| v[c]).fold(f64::INFINITY, f64::min));
    let hi: [f64; 3] = std::array::from_fn(|c| {
        curves
            .iter()
            .map(|v| v[c])
            .fold(f64::NEG_INFINITY, f64::max)
    });
    if !(0..3).all(|c| hi[c] > lo[c] && lo[c].is_finite() && hi[c].is_finite()) {
        return Err("negative hue requires three paper density ranges".into());
    }
    let mut data = Vec::with_capacity((2 + 6 * 41 * 41) * 3);
    data.extend(lo.map(from_f64));
    data.extend(hi.map(from_f64));
    for axis in 0..3 {
        for end in [lo, hi] {
            for i in 0..41 {
                for j in 0..41 {
                    let mut p = end;
                    let a = (axis + 1) % 3;
                    let b = (axis + 2) % 3;
                    p[a] = lo[a] + (hi[a] - lo[a]) * i as f64 / 40.0;
                    p[b] = lo[b] + (hi[b] - lo[b]) * j as f64 / 40.0;
                    data.extend(p.map(from_f64));
                }
            }
        }
    }
    Ok(ImageBuf::from_data((data.len() / 3) as u32, 1, data))
}

fn build_gamut(k: &mut NegativeHue, faces: &ImageBuf) -> Result<(), String> {
    let lab: Vec<_> = faces
        .data
        .as_chunks::<3>()
        .0
        .iter()
        .map(|p| {
            xyz_to_oklab(mat_vec(
                &SRGB_TO_XYZ_F64,
                [p[0] as f64, p[1] as f64, p[2] as f64],
            ))
        })
        .collect();
    let lo = lab[1][0];
    let hi = lab[0][0];
    if hi <= lo {
        return Err("negative hue requires distinct paper black and white".into());
    }
    k.light = [lo as f32, ((GAMUT_LEVELS - 1) as f64 / (hi - lo)) as f32];
    let points: Vec<_> = lab[2..]
        .iter()
        .map(|p| {
            [
                p[2].atan2(p[1]).to_degrees() / 3.0,
                (p[0] - lo) / (hi - lo) * (GAMUT_LEVELS - 1) as f64,
                p[1].hypot(p[2]),
            ]
        })
        .collect();
    k.gamut = vec![-1.0; GAMUT_LEVELS * GAMUT_HUES];
    for face in points.as_chunks::<{ 41 * 41 }>().0 {
        for i in 0..40 {
            for j in 0..40 {
                let a = i * 41 + j;
                for tri in [[a, a + 1, a + 41], [a + 42, a + 1, a + 41]] {
                    let p = tri.map(|n| face[n]);
                    if p.iter().any(|q| q[2] < 1e-4) {
                        continue;
                    }
                    let h = p.map(|q| p[0][0] + (q[0] - p[0][0] + 60.0).rem_euclid(120.0) - 60.0);
                    if h.iter().copied().fold(f64::NEG_INFINITY, f64::max)
                        - h.iter().copied().fold(f64::INFINITY, f64::min)
                        > 30.0
                    {
                        continue;
                    }
                    raster(std::array::from_fn(|n| [h[n], p[n][1]]), |h, l, w| {
                        if (0..GAMUT_LEVELS as i32).contains(&l) {
                            let idx =
                                l as usize * GAMUT_HUES + h.rem_euclid(GAMUT_HUES as i32) as usize;
                            k.gamut[idx] = k.gamut[idx]
                                .max((w[0] * p[0][2] + w[1] * p[1][2] + w[2] * p[2][2]) as f32);
                        }
                    });
                }
            }
        }
    }
    for h in 0..GAMUT_HUES {
        let mut prev = 0;
        k.gamut[h] = 0.0;
        k.gamut[(GAMUT_LEVELS - 1) * GAMUT_HUES + h] = 0.0;
        for l in 1..GAMUT_LEVELS {
            if k.gamut[l * GAMUT_HUES + h] >= 0.0 {
                let a = k.gamut[prev * GAMUT_HUES + h];
                let b = k.gamut[l * GAMUT_HUES + h];
                for j in prev + 1..l {
                    k.gamut[j * GAMUT_HUES + h] =
                        a + (b - a) * (j - prev) as f32 / (l - prev) as f32;
                }
                prev = l;
            }
        }
    }
    Ok(())
}

pub(crate) fn apply(k: &NegativeHue, negative: &[Scalar], rgb: &mut [Scalar]) {
    let r: [[f64; 2]; 3] = std::array::from_fn(|c| layer(k, c, negative[c] as f64));
    let e: [f64; 3] = std::array::from_fn(|c| 10.0_f64.powf(r[c][0]) / k.neutral[c] as f64);
    let sum = e.iter().sum::<f64>();
    let e = e.map(|v| v / sum);
    let lerp = |a: f64, b: f64, t: f64| a + (b - a) * t;
    let coord = |v: f64, n: usize| {
        let v = v.clamp(0.0, (n - 1) as f64);
        let i = (v as usize).min(n - 2);
        (i, v - i as f64)
    };
    let (x, tx) = coord(e[0] * (MAP_SIDE - 1) as f64, MAP_SIDE);
    let (y, ty) = coord(e[1] * (MAP_SIDE - 1) as f64, MAP_SIDE);
    let ab = std::array::from_fn(|c| {
        let at = |i: usize, j: usize| k.map[i * MAP_SIDE + j][c] as f64;
        lerp(
            lerp(at(x, y), at(x, y + 1), ty),
            lerp(at(x + 1, y), at(x + 1, y + 1), ty),
            tx,
        )
    });
    let o = xyz_to_oklab(mat_vec(
        &SRGB_TO_XYZ_F64,
        [rgb[0] as f64, rgb[1] as f64, rgb[2] as f64],
    ));
    // Keep optical lightness and chroma. Trust a hue reading in proportion
    // to its chroma at print lightness and the local film-curve slopes.
    let co = o[1].hypot(o[2]);
    let cr = vlen(ab);
    if co <= 1e-7 || cr <= 1e-9 {
        return;
    }
    let t = ((cr * o[0] / CALIBRATION_LIGHTNESS - 0.005) / 0.015).clamp(0.0, 1.0);
    let u = (0..3).map(|c| e[c] * r[c][1]).sum::<f64>() * 0.5;
    let w = (t * t * (3.0 - 2.0 * t) / (1.0 + u * u) - 0.02) / 0.98;
    if w <= 0.0 {
        return;
    }
    let v = std::array::from_fn(|c| lerp(o[1 + c] / co, ab[c] / cr, w));
    let norm = vlen(v);
    if norm <= 1e-6 {
        return;
    }
    let h = (v[1].atan2(v[0]).to_degrees() / 3.0).rem_euclid(GAMUT_HUES as f64);
    let j = h.floor() as usize % GAMUT_HUES;
    let (l, tl) = coord((o[0] - k.light[0] as f64) * k.light[1] as f64, GAMUT_LEVELS);
    let at = |l: usize| {
        lerp(
            k.gamut[l * GAMUT_HUES + j] as f64,
            k.gamut[l * GAMUT_HUES + (j + 1) % GAMUT_HUES] as f64,
            h.fract(),
        )
    };
    let ratio = co / lerp(at(l), at(l + 1), tl).max(1e-4);
    let mut chroma = co;
    if ratio > 0.85 {
        let z = (ratio - 0.85) / 0.15;
        chroma *= lerp(1.0, (0.85 + 0.15 * z / z.hypot(1.0)) / ratio, w);
    }
    let out = mat_vec(
        &XYZ_TO_SRGB_F64,
        oklab_to_xyz([o[0], chroma * v[0] / norm, chroma * v[1] / norm]),
    );
    rgb.copy_from_slice(&out.map(from_f64));
}
