//! Refit the bundled MG IV paper under RS's TH-KG3 head to HARMAN's
//! published full-grade ISO ranges. Prints paper_grades in 00..5 order.
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
    let maximum: f64 = curves.last().unwrap().iter().sum();
    let fit = |cc: f64| {
        let light = enlarger_filtered_illuminant_f64("TH-KG3", 0., (-cc).max(0.), cc.max(0.));
        let raw: [f64; 3] = std::array::from_fn(|c| {
            sens.iter()
                .zip(&light)
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
            100. * (x(0.90 * maximum) - x(0.04)),
            10f64.powf(x(-0.184f64.log10() - base)),
        )
    };
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
