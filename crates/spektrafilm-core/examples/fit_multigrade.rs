//! Refit the bundled MG IV paper under RS's TH-KG3 head to HARMAN's
//! published full-grade ISO ranges. Prints paper_grades in 00..5 order.
//! Pass --hard-grades for optical and sensitometric diagnostics instead.
use spektrafilm_core::{
    enlarger::enlarger_filtered_illuminant_f64, print_morph::morph_density_curves, profile::Profile,
};
use std::path::Path;

// Invert a monotonic response on a bounded interval (also clamps unattainable targets).
fn invert(lo: f64, hi: f64, target: f64, f: impl Fn(f64) -> f64) -> f64 {
    let (mut lo, mut hi) = (lo, hi);
    for _ in 0..40 {
        let mid = (lo + hi) * 0.5;
        if f(mid) < target {
            lo = mid;
        } else {
            hi = mid;
        }
    }
    (lo + hi) * 0.5
}

fn main() {
    let path = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../data/profiles/ilford_multigrade_iv_rc.json");
    let paper: Profile = serde_json::from_reader(std::fs::File::open(path).unwrap()).unwrap();
    let axis = &paper.data.log_exposure;
    let curves = morph_density_curves(
        axis,
        paper.data.density_curves_model.as_ref().unwrap(),
        &Default::default(),
        false,
    )
    .unwrap();
    let sens = paper.log_sensitivity_f64();
    let base = paper.data.base_density_rows[0][0];
    let response = |light: &[f64], axis: &[f64], curves: &[[f64; 3]], lo: f64, hi: f64| {
        let maximum: f64 = curves.last().unwrap().iter().sum();
        let raw: [f64; 3] = std::array::from_fn(|c| {
            sens.iter()
                .zip(light)
                .map(|(s, l)| l * 10f64.powf(s[c]))
                .sum::<f64>()
                .log10()
        });
        let mean = raw.iter().sum::<f64>() / 3.; // RS midgray exposure normalization
        let density = |x: f64| {
            (0..3)
                .map(|c| {
                    let pos = ((x + raw[c] - mean - axis[0]) / (axis[1] - axis[0]))
                        .clamp(0., (axis.len() - 1) as f64);
                    let i = pos as usize;
                    curves[i][c]
                        + pos.fract() * (curves[(i + 1).min(axis.len() - 1)][c] - curves[i][c])
                })
                .sum::<f64>()
        };
        let x = |d| invert(-8., 8., d, density);
        (
            100. * (x(hi * maximum) - x(lo)),
            10f64.powf(x(-0.184f64.log10() - base)),
        )
    };
    let head = |cc: f64| enlarger_filtered_illuminant_f64("TH-KG3", 0., (-cc).max(0.), cc.max(0.));
    let fit = |cc| response(&head(cc), axis, &curves, 0.04, 0.90);
    if std::env::args().nth(1).as_deref() == Some("--hard-grades") {
        let midpoint = (fit(-250.).0 + 60.) / 2.;
        let cc = invert(-250., 0., midpoint, |cc| fit(cc).0);
        let stored = paper.density_curves_f64();
        for m in [39.1, 86.26, -cc, 151.7, 250., 1000.] {
            let light = head(-m);
            let (r, exposure) = fit(-m);
            eprintln!(
                "M {m:.6}: R {r:.6}, exposure {exposure:.8}, stored R {:.5}, central (10%-80% net) width {:.5}",
                response(&light, axis, &stored, 0.04, 0.90).0,
                response(
                    &light,
                    axis,
                    &curves,
                    0.10 * curves.last().unwrap().iter().sum::<f64>(),
                    0.80
                )
                .0
            );
        }
        let best_head = (-1000..=250)
            .map(|cc| (fit(cc as f64).0, cc))
            .min_by(|a, b| a.0.total_cmp(&b.0))
            .unwrap();
        let dense_axis: Vec<_> = (0..=8192)
            .map(|i| axis[0] + (axis[axis.len() - 1] - axis[0]) * i as f64 / 8192.)
            .collect();
        let dense_curves = morph_density_curves(
            &dense_axis,
            paper.data.density_curves_model.as_ref().unwrap(),
            &Default::default(),
            false,
        )
        .unwrap();
        eprintln!(
            "Head sweep (1 CC): minimum R {:.5} at signed Y/M {}; 250 M on dense CDF grid R {:.5}",
            best_head.0,
            best_head.1,
            response(&head(-250.), &dense_axis, &dense_curves, 0.04, 0.90).0
        );
        let bare = head(0.);
        for cutoff in [440., 460., 480.] {
            let light: Vec<_> = bare
                .iter()
                .zip(&paper.data.wavelengths)
                .map(|(&v, &wl)| if wl <= cutoff { v } else { 0. })
                .collect();
            eprintln!(
                "ideal shortpass {cutoff} nm: R {:.5}",
                response(&light, axis, &curves, 0.04, 0.90).0
            );
        }
        // After normalization by lane 0, each wavelength is a point in
        // (S1/S0, S2/S0). Nonnegative spectra give convex mixtures of them.
        let blue = (430 - 380) / 5;
        let green = (530 - 380) / 5;
        let ratios = |r: &[f64; 3]| r.map(|v| 10f64.powf(v - r[0]));
        let b = ratios(&sens[blue]);
        let g = ratios(&sens[green]);
        let residual = sens
            .iter()
            .map(|row| {
                let r = ratios(row);
                let t = ((r[1] - g[1]) / (b[1] - g[1])).clamp(0., 1.);
                (1..3)
                    .map(|c| (r[c] - (g[c] + t * (b[c] - g[c]))).abs())
                    .fold(0., f64::max)
            })
            .fold(0., f64::max);
        let mut best = (f64::INFINITY, 0.);
        for i in 0..=1000 {
            let t = i as f64 / 1000.;
            let mut light = [0.; 81];
            light[blue] = t / 10f64.powf(sens[blue][0]);
            light[green] = (1. - t) / 10f64.powf(sens[green][0]);
            let r = response(&light, axis, &curves, 0.04, 0.90).0;
            if r < best.0 {
                best = (r, t);
            }
        }
        eprintln!(
            "Blue/green mixture sweep: minimum R {:.5}, blue fraction {:.3} of lane 0, max ratio residual {residual:.3e}",
            best.0, best.1
        );
        return;
    }
    let mut packs = Vec::new();
    for r in [180., 160., 130., 110., 90., 60., 40.] {
        let cc = invert(-250., 250., r, |cc| fit(cc).0);
        let (range, exposure) = fit(cc);
        eprintln!("target {r:.0}, attained {range:.2}, signed Y/M {cc:.2}, exposure {exposure:.5}");
        packs.push(
            serde_json::json!({"magenta":(-cc).max(0.),"yellow":cc.max(0.),"exposure":exposure}),
        );
    }
    println!("{}", serde_json::to_string_pretty(&packs).unwrap());
}
