//! Paired precisions of the light sources named by the shipped profiles.
use spektrafilm_math::spectral::*;

pub(crate) fn spectrum(name: &str) -> Option<(&'static [f32], &'static [f64])> {
    Some(match name {
        "D50" => (&ILLUMINANT_D50, &ILLUMINANT_D50_F64),
        "D55" => (&ILLUMINANT_D55, &ILLUMINANT_D55_F64),
        "D65" => (&ILLUMINANT_D65, &ILLUMINANT_D65_F64),
        "T" => (&cinema::T, &cinema::T_F64),
        "K75P" => (&cinema::K75P, &cinema::K75P_F64),
        "TH-KG3" => (&crate::enlarger::ILLUMINANT_TH_KG3, &crate::enlarger::ILLUMINANT_TH_KG3_F64),
        _ => return None,
    })
}
