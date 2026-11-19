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

/// Nominal aperture in roll coordinates. A 120 back supplies its own measured
/// gate dimensions in tenths of a millimetre (along, across).
#[derive(Clone, Copy, Debug, Deserialize, Eq, Hash, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum GateFormat {
    HalfFrame17,
    FullFrame36,
    Xpan65,
    Roll120 {
        along_tenth_mm: u16,
        across_tenth_mm: u16,
    },
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, Hash, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum RollAxis {
    Horizontal,
    Vertical,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, Hash, PartialEq, Serialize)]
pub struct FilmGate {
    pub format: GateFormat,
    pub axis: RollAxis,
}

/// Camera-side exposure in developed-film coordinates. Gains are thousandths
/// of linear emulsion exposure, in the film front end's three raw channels.
#[derive(Clone, Copy, Debug, Deserialize, Eq, Hash, PartialEq, Serialize)]
pub struct LightLeak {
    pub center_tenth_mm: [i16; 2],
    pub sigma_tenth_mm: [u16; 2],
    pub exposure_milli: [u16; 3],
}

/// A short original dot-matrix imprint. Coordinates locate its upper-left
/// corner on the roll. Callers supply the frame, date or stock identifier.
#[derive(Clone, Copy, Debug, Deserialize, Eq, Hash, PartialEq, Serialize)]
pub struct FilmImprint {
    pub text: ImprintText,
    pub origin_tenth_mm: [i16; 2],
    pub height_tenth_mm: u16,
    pub exposure_milli: [u16; 3],
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, Hash, PartialEq, Serialize)]
#[serde(try_from = "String", into = "String")]
pub struct ImprintText([u8; 16]);

const ALPHABET: &[u8] = b"0123456789ABCDEFGHIJKLMNOPQRSTUVWXYZ- ";
// Five rows per character, each row a three-bit octal digit.
const GLYPHS: [u32; 38] = [
    0o75557, 0o26227, 0o71747, 0o71717, 0o55711, 0o74717, 0o74757, 0o71111, 0o75757, 0o75717,
    0o25755, 0o65656, 0o34443, 0o65556, 0o74647, 0o74644, 0o34553, 0o55755, 0o72227, 0o11152,
    0o55655, 0o44447, 0o57755, 0o57555, 0o25552, 0o65644, 0o25573, 0o65655, 0o34216, 0o72222,
    0o55557, 0o55552, 0o55775, 0o55255, 0o55222, 0o71247, 0o00700, 0,
];

impl TryFrom<String> for ImprintText {
    type Error = &'static str;
    fn try_from(text: String) -> Result<Self, Self::Error> {
        if text.is_empty() || text.len() > 16 || !text.bytes().all(|b| ALPHABET.contains(&b)) {
            return Err(
                "film imprint accepts 1 to 16 uppercase ASCII letters, digits, spaces or dashes",
            );
        }
        let mut bytes = [0; 16];
        bytes[..text.len()].copy_from_slice(text.as_bytes());
        Ok(Self(bytes))
    }
}

impl From<ImprintText> for String {
    fn from(text: ImprintText) -> Self {
        text.0
            .into_iter()
            .take_while(|b| *b != 0)
            .map(char::from)
            .collect()
    }
}

/// Overscan is in source pixels, in left, top, right, bottom order.
#[derive(Clone, Copy, Debug, Default, Deserialize, Eq, Hash, PartialEq, Serialize)]
pub struct Border135 {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub gate: Option<FilmGate>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub light_leak: Option<LightLeak>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub imprint: Option<FilmImprint>,
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
    pub strip_half_mm: f32,
    pub perforated: bool,
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
    #[error("roll-film gate must fit the strip and have positive dimensions")]
    Gate,
}

impl Border135 {
    /// The film size is the existing long-side scale of the content.
    /// Legacy zero overscan uses the original render path. Explicit gates and
    /// active imprints or leaks can also render within the original canvas.
    pub fn layout(
        self,
        [width, height]: [u32; 2],
        film_format_mm: f32,
    ) -> Result<Option<Layout135>, GeometryError> {
        if self.overscan == [0; 4]
            && self.gate.is_none()
            && self.imprint.is_none_or(|v| v.exposure_milli == [0; 3])
            && self.light_leak.is_none_or(|v| v.exposure_milli == [0; 3])
        {
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
        let horizontal = self
            .gate
            .map_or(width >= height, |g| g.axis == RollAxis::Horizontal);
        let raster = if horizontal {
            [width as f32, height as f32]
        } else {
            [height as f32, width as f32]
        };
        let (pixel_pitch_mm, gate_half_mm, strip_half_mm, perforated) =
            if let Some(gate) = self.gate {
                let (along, across, strip, perforated) = match gate.format {
                    GateFormat::HalfFrame17 => (17., 24., 35., true),
                    GateFormat::FullFrame36 => (36., 24., 35., true),
                    GateFormat::Xpan65 => (65., 24., 35., true),
                    GateFormat::Roll120 {
                        along_tenth_mm,
                        across_tenth_mm,
                    } => {
                        if along_tenth_mm == 0 || across_tenth_mm == 0 || across_tenth_mm > 615 {
                            return Err(GeometryError::Gate);
                        }
                        (
                            along_tenth_mm as f32 / 10.,
                            across_tenth_mm as f32 / 10.,
                            61.5,
                            false,
                        )
                    }
                };
                // Centre-crop a mismatched source to cover the gate with square pixels.
                (
                    (along / raster[0]).max(across / raster[1]),
                    [along * 0.5, across * 0.5],
                    strip * 0.5,
                    perforated,
                )
            } else {
                let pitch =
                    crate::stages::filming::pixel_size_um(film_format_mm, width, height) / 1000.;
                let half = raster.map(|pixels| pixels * pitch * 0.5);
                (pitch, half, half[1] + SHOULDER_MM, true)
            };
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
        let film_from_canvas = if horizontal {
            [[c, s], [-s, c]]
        } else {
            [[-s, c], [-c, -s]]
        };
        Ok(Some(Layout135 {
            canvas: [canvas_width, canvas_height],
            content: [left, top, width, height],
            pixel_pitch_mm,
            film_origin_px: origin,
            film_from_canvas,
            gate_half_mm,
            strip_half_mm,
            perforated,
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
        let strip_half_mm = layout.strip_half_mm;
        let mut leak_shape = [0.; 4];
        let mut leak_gain = [0.; 4];
        if let Some(leak) = self.light_leak.filter(|v| v.exposure_milli != [0; 3]) {
            if leak.sigma_tenth_mm.contains(&0) {
                return Err(OutputError::InvalidFilmBorder);
            }
            for i in 0..2 {
                leak_shape[i] = f32::from(leak.center_tenth_mm[i]) / 10.;
                leak_shape[2 + i] = 10. / f32::from(leak.sigma_tenth_mm[i]);
            }
            leak_gain[..3].copy_from_slice(&leak.exposure_milli.map(|v| f32::from(v) / 1000.));
        }
        let mut imprint_shape = [0.; 4];
        let mut imprint_gain = [0.; 4];
        let mut glyphs = [0; 16];
        if let Some(mark) = self.imprint.filter(|v| v.exposure_milli != [0; 3]) {
            if mark.height_tenth_mm == 0 {
                return Err(OutputError::InvalidFilmBorder);
            }
            imprint_shape = [
                f32::from(mark.origin_tenth_mm[0]) / 10.,
                f32::from(mark.origin_tenth_mm[1]) / 10.,
                50. / f32::from(mark.height_tenth_mm),
                0.,
            ];
            for (glyph, byte) in glyphs
                .iter_mut()
                .zip(mark.text.0)
                .take_while(|(_, b)| *b != 0)
            {
                *glyph = GLYPHS[ALPHABET.iter().position(|b| *b == byte).unwrap()];
            }
            imprint_gain[..3].copy_from_slice(&mark.exposure_milli.map(|v| f32::from(v) / 1000.));
        }
        Ok(Some(FilmBorderGpuParams {
            leak_shape,
            leak_gain,
            imprint_shape,
            imprint_gain,
            glyphs,
            canvas: layout.canvas,
            _pad0: [0; 2],
            content: layout.content,
            origin_px: layout.film_origin_px,
            gate_half_mm: layout.gate_half_mm,
            film_from_canvas: layout.film_from_canvas,
            strip_half_mm,
            edge_exposure,
            perforation_pitch_mm: if layout.perforated {
                PERFORATION_PITCH_MM
            } else {
                0.
            },
            perforation_corner_mm: PERFORATION_CORNER_MM,
            perforation_half_mm: PERFORATION_HALF_SIZE_MM,
            perforation_row_mm: strip_half_mm - PERFORATION_CENTER_INSET_MM,
            gate_pitch_um: if self.gate.is_some() {
                layout.pixel_pitch_mm * 1000.
            } else {
                0.
            },
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
        let film_edge = self.strip_half_mm;
        let periodic_s = s - PERFORATION_PITCH_MM * (s / PERFORATION_PITCH_MM + 0.5).floor();
        let hole_t = t.abs() - (film_edge - PERFORATION_CENTER_INSET_MM);
        let [half_s, half_t] = PERFORATION_HALF_SIZE_MM;
        let q = [
            (periodic_s.abs() - half_s + PERFORATION_CORNER_MM).max(0.0),
            (hole_t.abs() - half_t + PERFORATION_CORNER_MM).max(0.0),
        ];
        let perforated = q[0] * q[0] + q[1] * q[1] <= PERFORATION_CORNER_MM * PERFORATION_CORNER_MM;
        FilmSite {
            film_present: t.abs() < film_edge && !(self.perforated && perforated),
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
