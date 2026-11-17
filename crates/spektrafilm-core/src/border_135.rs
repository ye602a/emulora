//! Raster placement and film coordinates for a basic 135 border.

use serde::{Deserialize, Serialize};
use spektrafilm_gpu::output::DisplayRegion;

const SHOULDER_MM: f32 = 5.5;
const PERFORATION_PITCH_MM: f32 = 4.75;
const PERFORATION_HALF_SIZE_MM: [f32; 2] = [1.397, 0.9905];
const PERFORATION_CORNER_MM: f32 = 0.51;
const PERFORATION_CENTER_INSET_MM: f32 = 2.0;

/// A positive offset places the film to the right/down in the output.
/// The rotation is clockwise in raster coordinates.
#[derive(Clone, Copy, Debug, Default, Deserialize, Eq, Hash, PartialEq, Serialize)]
pub struct ScanRegistration {
    #[serde(default)]
    pub offset_px: [i32; 2],
    #[serde(default)]
    pub rotation_centi_degrees: i32,
}

/// Overscan is in source pixels, in left, top, right, bottom order.
#[derive(Clone, Copy, Debug, Default, Deserialize, Eq, Hash, PartialEq, Serialize)]
pub struct Border135 {
    pub overscan: [u32; 4],
    #[serde(default)]
    pub registration: ScanRegistration,
    /// Scene-linear exposure of the unmasked shoulder, relative to 18% grey.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub edge_exposure_centi_ev: Option<i32>,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Layout135 {
    pub canvas: [u32; 2],
    pub content: [u32; 4],
    pub pixel_pitch_mm: f32,
    /// Origin and row-major inverse transform, in canvas pixels and film mm.
    pub film_origin_px: [f32; 2],
    pub film_from_canvas: [[f32; 2]; 2],
    pub gate_half_mm: [f32; 2],
}

/// Film presence and the camera aperture are independent of image placement.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct FilmSite {
    pub film_present: bool,
    pub gate_open: bool,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, thiserror::Error)]
pub enum GeometryError {
    #[error("135 border dimensions exceed the supported RGB raster")]
    Dimensions,
    #[error("135 border requires a finite positive physical pixel pitch")]
    PixelPitch,
}

impl Border135 {
    /// The film size is the existing long-side scale of the content.
    /// Zero overscan uses the original render path, regardless of registration.
    pub fn layout(
        self,
        [width, height]: [u32; 2],
        film_format_mm: f32,
    ) -> Result<Option<Layout135>, GeometryError> {
        if self.overscan == [0; 4] {
            return Ok(None);
        }
        let [left, top, right, bottom] = self.overscan;
        let canvas_width = width
            .checked_add(left)
            .and_then(|v| v.checked_add(right))
            .ok_or(GeometryError::Dimensions)?;
        let canvas_height = height
            .checked_add(top)
            .and_then(|v| v.checked_add(bottom))
            .ok_or(GeometryError::Dimensions)?;
        if width == 0
            || height == 0
            || canvas_width
                .checked_mul(canvas_height)
                .and_then(|pixels| pixels.checked_mul(3))
                .is_none()
        {
            return Err(GeometryError::Dimensions);
        }
        let pixel_pitch_mm =
            crate::stages::filming::pixel_size_um(film_format_mm, width, height) / 1000.0;
        if !(pixel_pitch_mm.is_finite() && pixel_pitch_mm > 0.0) {
            return Err(GeometryError::PixelPitch);
        }

        let [x, y] = self.registration.offset_px;
        let origin = [
            left as f32 + width as f32 * 0.5 + x as f32,
            top as f32 + height as f32 * 0.5 + y as f32,
        ];
        let angle = (self.registration.rotation_centi_degrees.rem_euclid(36_000) as f64 / 100.0)
            .to_radians();
        let (sine, cosine) = angle.sin_cos();
        let (s, c) = (sine as f32 * pixel_pitch_mm, cosine as f32 * pixel_pitch_mm);
        let (film_from_canvas, gate_half_mm) = if width >= height {
            ([[c, s], [-s, c]], [width as f32, height as f32])
        } else {
            ([[-s, c], [-c, -s]], [height as f32, width as f32])
        };
        Ok(Some(Layout135 {
            canvas: [canvas_width, canvas_height],
            content: [left, top, width, height],
            pixel_pitch_mm,
            film_origin_px: origin,
            film_from_canvas,
            gate_half_mm: gate_half_mm.map(|pixels| pixels * pixel_pitch_mm * 0.5),
        }))
    }

    pub(crate) fn prepare_gpu(
        self,
        content: [u32; 2],
        film_format_mm: f32,
    ) -> Result<Option<spektrafilm_gpu::FilmBorderGpuParams>, spektrafilm_gpu::output::OutputError>
    {
        use spektrafilm_gpu::{FilmBorderGpuParams, output::OutputError};
        let Some(layout) = self
            .layout(content, film_format_mm)
            .map_err(|_| OutputError::InvalidFilmBorder)?
        else {
            return Ok(None);
        };
        let edge_exposure = self
            .edge_exposure_centi_ev
            .map_or(0.0, |ev| 0.18 * (ev as f32 / 100.0).exp2());
        if !edge_exposure.is_finite() {
            return Err(OutputError::InvalidFilmBorder);
        }
        let strip_half_mm = layout.gate_half_mm[1] + SHOULDER_MM;
        Ok(Some(FilmBorderGpuParams {
            canvas: layout.canvas,
            _pad0: [0; 2],
            content: layout.content,
            origin_px: layout.film_origin_px,
            gate_half_mm: layout.gate_half_mm,
            film_from_canvas: layout.film_from_canvas,
            strip_half_mm,
            edge_exposure,
            perforation_pitch_mm: PERFORATION_PITCH_MM,
            perforation_corner_mm: PERFORATION_CORNER_MM,
            perforation_half_mm: PERFORATION_HALF_SIZE_MM,
            perforation_row_mm: strip_half_mm - PERFORATION_CENTER_INSET_MM,
            _pad1: 0.0,
        }))
    }
}

impl Layout135 {
    /// Recover the original image rectangle using the existing 1:1 output crop.
    pub fn content_region(self) -> DisplayRegion {
        let [_, _, width, height] = self.content;
        DisplayRegion {
            crop: self.content,
            width,
            height,
        }
    }

    /// Film millimetres from the centre of the aperture, along/across the roll.
    pub fn film_at(self, [x, y]: [u32; 2]) -> [f32; 2] {
        let dx = x as f32 + 0.5 - self.film_origin_px[0];
        let dy = y as f32 + 0.5 - self.film_origin_px[1];
        self.film_from_canvas.map(|[a, b]| a * dx + b * dy)
    }

    pub fn sample(self, canvas_pixel: [u32; 2]) -> FilmSite {
        let [s, t] = self.film_at(canvas_pixel);
        let [gate_s, gate_t] = self.gate_half_mm;
        let film_edge = gate_t + SHOULDER_MM;
        let periodic_s = s - PERFORATION_PITCH_MM * (s / PERFORATION_PITCH_MM + 0.5).floor();
        let hole_t = t.abs() - (film_edge - PERFORATION_CENTER_INSET_MM);
        let [half_s, half_t] = PERFORATION_HALF_SIZE_MM;
        let q = [
            (periodic_s.abs() - half_s + PERFORATION_CORNER_MM).max(0.0),
            (hole_t.abs() - half_t + PERFORATION_CORNER_MM).max(0.0),
        ];
        let perforated = q[0] * q[0] + q[1] * q[1] <= PERFORATION_CORNER_MM * PERFORATION_CORNER_MM;
        FilmSite {
            film_present: t.abs() < film_edge && !perforated,
            gate_open: s.abs() < gate_s && t.abs() < gate_t,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::params::RuntimeParams;

    #[test]
    fn zero_overscan_and_legacy_serialization_use_default_geometry() {
        let params: RuntimeParams = serde_json::from_str("{}").unwrap();
        assert!(params.border_135.is_none());
        assert!(
            serde_json::to_value(params)
                .unwrap()
                .get("border_135")
                .is_none()
        );
        let border = Border135 {
            registration: ScanRegistration {
                offset_px: [8, -4],
                rotation_centi_degrees: 150,
            },
            ..Default::default()
        };
        assert_eq!(border.layout([360, 240], 36.0).unwrap(), None);
    }

    #[test]
    fn zero_overscan_preserves_every_gpu_output_code() {
        use crate::{pipeline::Pipeline, profile::load_profile_by_name};
        use spektrafilm_gpu::{FilmRenderOptions, wgpu_backend::WgpuBackend};
        use spektrafilm_math::image::ImageBuf;
        use spektrafilm_math::precision::from_f32;
        use std::path::Path;

        let Some(backend) = WgpuBackend::new() else {
            assert!(std::env::var_os("SPEKTRAFILM_REQUIRE_GPU").is_none());
            return;
        };
        let data = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../data");
        let film = load_profile_by_name(&data, "kodak_portra_400").unwrap();
        let paper = load_profile_by_name(&data, "kodak_portra_endura").unwrap();
        let mut params = RuntimeParams::default();
        params.camera.auto_exposure = false;
        let input = ImageBuf::from_data(
            19,
            13,
            (0..19 * 13 * 3)
                .map(|i| from_f32((i % 97) as f32 / 120.0))
                .collect(),
        );
        let render = |params| {
            Pipeline::new_with_spectral(film.clone(), paper.clone(), params, &data)
                .unwrap()
                .try_with_resident_params(&input, |prepared| {
                    backend.run_film_chain_rgb16(prepared, FilmRenderOptions::default(), None)
                })
                .unwrap()
                .unwrap()
                .unwrap()
        };
        let legacy = render(params.clone());
        params.border_135 = Some(Border135 {
            registration: ScanRegistration {
                offset_px: [15, -8],
                rotation_centi_degrees: 300,
            },
            ..Default::default()
        });
        assert_eq!(render(params), legacy);
    }

    #[test]
    fn asymmetric_overscan_preserves_pitch_and_content_rectangle() {
        let border = Border135 {
            overscan: [13, 65, 19, 70],
            ..Default::default()
        };
        let layout = border.layout([360, 240], 36.0).unwrap().unwrap();
        assert_eq!(layout.canvas, [392, 375]);
        assert_eq!(layout.content, [13, 65, 360, 240]);
        assert_eq!(layout.pixel_pitch_mm, 0.1);
        let view = layout.content_region();
        assert_eq!([view.width, view.height], [360, 240]);
        view.validate(layout.canvas[0], layout.canvas[1]).unwrap();
        assert!(layout.sample([13, 65]).gate_open);
        assert!(layout.sample([372, 304]).gate_open);
        assert!(!layout.sample([12, 65]).gate_open);
        assert!(!layout.sample([373, 304]).gate_open);
        assert_eq!(
            layout.sample([193, 150]),
            FilmSite {
                film_present: true,
                gate_open: true
            }
        );
        assert_eq!(
            layout.sample([193, 30]),
            FilmSite {
                film_present: false,
                gate_open: false
            }
        );
        assert_eq!(
            layout.sample([193, 53]),
            FilmSite {
                film_present: true,
                gate_open: false
            }
        );
        assert_eq!(
            layout.sample([193, 7]),
            FilmSite {
                film_present: false,
                gate_open: false
            }
        );
    }

    #[test]
    fn registration_is_deterministic_and_portrait_keeps_strip_on_long_axis() {
        let border = Border135 {
            overscan: [65; 4],
            registration: ScanRegistration {
                offset_px: [3, -6],
                rotation_centi_degrees: 9000,
            },
            ..Default::default()
        };
        let Layout135 {
            film_from_canvas,
            film_origin_px,
            ..
        } = border.layout([360, 240], 36.0).unwrap().unwrap();
        assert_eq!(film_origin_px, [248.0, 179.0]);
        assert!((film_from_canvas[0][1] - 0.1).abs() < 1e-6);
        let saved = serde_json::to_string(&border).unwrap();
        let decoded: Border135 = serde_json::from_str(&saved).unwrap();
        assert_eq!(
            decoded.layout([360, 240], 36.0),
            border.layout([360, 240], 36.0)
        );

        let portrait = Border135 {
            registration: Default::default(),
            ..border
        }
        .layout([240, 360], 36.0)
        .unwrap()
        .unwrap();
        assert_eq!(
            portrait.sample([29, 245]),
            FilmSite {
                film_present: false,
                gate_open: false
            }
        );
        assert_eq!(
            portrait.sample([185, 245]),
            FilmSite {
                film_present: true,
                gate_open: true
            }
        );
    }

    #[test]
    fn overscan_cannot_wrap_dimensions_or_exceed_shader_indices() {
        let border = Border135 {
            overscan: [32; 4],
            ..Default::default()
        };
        assert_eq!(
            border.layout([0, 384], 36.0),
            Err(GeometryError::Dimensions)
        );
        assert_eq!(
            border.layout([u32::MAX, 200], 36.0),
            Err(GeometryError::Dimensions)
        );
        assert_eq!(
            border.layout([65_535, 65_535], 36.0),
            Err(GeometryError::Dimensions)
        );
        assert_eq!(
            border.layout([128, 120], f32::NAN),
            Err(GeometryError::PixelPitch)
        );
    }
}
