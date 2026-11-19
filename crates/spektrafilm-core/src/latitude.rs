//! Read-only placement of the scene against a neutral ramp through a calibrated positive output.
use crate::{
    lut_bake::{pointwise_params, render_probe},
    params::RuntimeParams,
    pipeline::Pipeline,
    stages::filming::{input_colorspace_to_xyz, measure_autoexposure_ev},
};
use serde::Serialize;
use spektrafilm_gpu::ComputeBackend;
use spektrafilm_math::{image::ImageBuf, precision::from_f64};

const GRAY: f64 = 0.184;
const SAMPLES: usize = 1024;
fn ramp_ev(index: usize) -> f64 {
    -12.0 + 24.0 * index as f64 / (SAMPLES - 1) as f64
}

/// Boundaries on the linear input stop axis (before manual film EV).
pub struct MediumLatitude {
    pub shadow_ev: f64,
    pub highlight_ev: f64,
    pub black_y: f64,
    pub white_y: f64,
    /// Linear output luminance at 1024 evenly spaced stops from -12 to +12.
    pub ramp_y: Vec<f64>,
}

pub(crate) fn measure(
    pipeline: &Pipeline,
    backend: &dyn ComputeBackend,
) -> Result<MediumLatitude, String> {
    if pipeline.params.io.scan_film && !pipeline.film.is_positive() {
        return Err("latitude requires a positive print or a slide scan".into());
    }
    let mut probe = pipeline.clone();
    pointwise_params(&mut probe.params);
    probe.params.io.output_cctf_encoding = false;
    let y_row = input_colorspace_to_xyz(&probe.params.io.output_color_space)[1];
    let mut ramp = ImageBuf::new(SAMPLES as u32, 1);
    for (i, rgb) in ramp.pixels_mut().enumerate() {
        rgb.fill(from_f64(GRAY * ramp_ev(i).exp2()));
    }
    let output = render_probe(&probe, ramp, backend)?;
    let ramp_y: Vec<f64> = output
        .pixels()
        .map(|rgb| (0..3).map(|c| rgb[c] as f64 * y_row[c] as f64).sum())
        .collect();
    let black_y = ramp_y.iter().copied().fold(f64::INFINITY, f64::min);
    let white_y = ramp_y.iter().copied().fold(f64::NEG_INFINITY, f64::max);
    if !(black_y > 0.0 && white_y > black_y) {
        return Err("latitude: no measurable positive black/white range".into());
    }
    let crossing = |target: f64| {
        ramp_y
            .windows(2)
            .enumerate()
            .find_map(|(i, y)| {
                (y[0] <= target && target <= y[1] && y[1] > y[0]).then(|| {
                    ramp_ev(i) + (target - y[0]) / (y[1] - y[0]) * (ramp_ev(i + 1) - ramp_ev(i))
                })
            })
            .ok_or("latitude: gray ramp does not cross the boundary")
    };
    let shadow_ev = crossing(black_y * 10.0_f64.powf(0.04))?;
    let highlight_ev = crossing(black_y + 0.90 * (white_y - black_y))?;
    if shadow_ev >= highlight_ev {
        return Err("latitude: positive gray ramp required".into());
    }
    Ok(MediumLatitude {
        shadow_ev,
        highlight_ev,
        black_y,
        white_y,
        ramp_y,
    })
}

#[derive(Debug, Serialize)]
pub struct ScenePlacement {
    pub meter_ev: f64,
    #[serde(flatten)]
    pub counts: LatitudeCounts,
}

#[derive(Debug, Default, Serialize)]
pub struct LatitudeCounts {
    pub sampled: usize,
    pub excluded: usize,
    pub below_shadow: usize,
    pub above_highlight: usize,
}

impl MediumLatitude {
    /// Analyze the supplied post-preprocessing, linear RGB scene under the same recipe.
    /// Only automatic metering shifts these samples; camera compensation is already in the ramp.
    pub fn place_scene(
        &self,
        image: &ImageBuf,
        params: &RuntimeParams,
    ) -> Result<ScenePlacement, String> {
        spektrafilm_gpu::output::validate_rgb_image(image).map_err(|e| e.to_string())?;
        let meter_ev = if params.camera.auto_exposure {
            measure_autoexposure_ev(
                image,
                &input_colorspace_to_xyz(&params.io.input_color_space),
                &params.camera.auto_exposure_method,
            ) as f64
        } else {
            0.0
        };
        let lo = GRAY * (self.shadow_ev - meter_ev).exp2();
        let hi = GRAY * (self.highlight_ev - meter_ev).exp2();
        let counts = count_samples(image, lo, hi, |rgb| {
            let [r, g, b] = rgb.map(|v| (v as f64).max(0.0));
            let mass = r * r + g * g + b * b;
            // LAB's power norm weights each positive channel by its squared intensity.
            (r * r * r + g * g * g + b * b * b) / mass
        });
        Ok(ScenePlacement { meter_ev, counts })
    }

    /// Classify the completed photo in linear output luminance, before file quantization.
    pub fn measure_output(
        &self,
        image: &ImageBuf,
        params: &RuntimeParams,
    ) -> Result<LatitudeCounts, String> {
        spektrafilm_gpu::output::validate_rgb_image(image).map_err(|e| e.to_string())?;
        let y = input_colorspace_to_xyz(&params.io.output_color_space)[1];
        Ok(count_samples(
            image,
            self.black_y * 10.0_f64.powf(0.04),
            self.black_y + 0.9 * (self.white_y - self.black_y),
            |rgb| {
                let linear = if params.io.output_cctf_encoding {
                    rgb.map(spektrafilm_math::precision::srgb_decode)
                } else {
                    rgb
                };
                (0..3).map(|c| y[c] as f64 * linear[c] as f64).sum()
            },
        ))
    }
}

fn count_samples(
    image: &ImageBuf,
    lo: f64,
    hi: f64,
    level: impl Fn([spektrafilm_math::precision::Scalar; 3]) -> f64,
) -> LatitudeCounts {
    let mut result = LatitudeCounts::default();
    let step = image.width.max(image.height).div_ceil(1024) as usize;
    for y in (0..image.height).step_by(step) {
        for x in (0..image.width).step_by(step) {
            let rgb = image.get(x, y);
            let light = if rgb.iter().all(|v| v.is_finite()) {
                level(rgb)
            } else {
                f64::NAN
            };
            if light.is_finite() {
                result.sampled += 1;
                result.below_shadow += usize::from(light < lo);
                result.above_highlight += usize::from(light > hi);
            } else {
                result.excluded += 1;
            }
        }
    }
    result
}
