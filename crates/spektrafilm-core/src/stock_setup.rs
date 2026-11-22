//! Opt-in physical stock defaults for spectral pipeline construction.

use crate::{params::RuntimeParams, profile::Profile};

/// Stable identity of the first physical stock-default contract.
pub const PHYSICAL_STOCK_DEFAULTS_V1_ID: &str = "physical-stock-defaults-v1";

/// Film routes used to validate the stock against the intended physical process.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum StockRoute {
    Paper,
    Slide,
    NegativeScan,
}

/// Versioned, opt-in film defaults for halation, slide DIR and calibration lighting.
///
/// The route validates the film type. Callers supply scan, print, exposure and
/// colour settings in the runtime recipe.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum StockDefaults {
    /// First physical stock defaults, identified by [PHYSICAL_STOCK_DEFAULTS_V1_ID].
    Physical(StockRoute),
}

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum StockSetupError {
    #[error("stock {stock:?} of type {film_type:?} does not match route {route:?}")]
    RouteMismatch {
        stock: Option<String>,
        film_type: String,
        route: StockRoute,
    },
}

impl StockDefaults {
    pub const fn id(self) -> &'static str {
        match self {
            Self::Physical(_) => PHYSICAL_STOCK_DEFAULTS_V1_ID,
        }
    }

    pub const fn route(self) -> StockRoute {
        match self {
            Self::Physical(route) => route,
        }
    }

    /// Validate and freeze the stock constants before starting calibration.
    /// Resolution does not mutate the profile or any caller-owned parameters.
    pub(crate) fn resolve(self, film: &Profile) -> Result<ResolvedStockSetup, StockSetupError> {
        let route = self.route();
        if film.is_printing() || film.is_positive() != (route == StockRoute::Slide) {
            return Err(StockSetupError::RouteMismatch {
                stock: film.info.stock.clone(),
                film_type: film.info.film_type.clone(),
                route,
            });
        }
        let slide_scale = match (route, film.info.stock.as_deref()) {
            (StockRoute::Slide, Some("fujifilm_provia_100f")) => Some(1.3),
            (StockRoute::Slide, Some("fujifilm_velvia_100")) => Some(0.9),
            _ => None,
        };
        Ok(ResolvedStockSetup {
            selection: self,
            halation_sigma: if film.info.usage == "cine" {
                50.0
            } else {
                65.0
            },
            halation_strength: match film.info.antihalation.as_str() {
                "strong" => Some([0.015, 0.005, 0.0]),
                "weak" => Some([0.08, 0.02, 0.0]),
                "no" => Some([0.30, 0.10, 0.015]),
                _ => None,
            },
            slide_scale,
        })
    }
}

/// The validated selection belongs to one calibrated pipeline/profile.
/// Keeping it avoids reinterpreting an unchecked stock name on refresh.
#[derive(Debug, Clone, Copy)]
pub(crate) struct ResolvedStockSetup {
    selection: StockDefaults,
    halation_sigma: f64,
    halation_strength: Option<[f64; 3]>,
    slide_scale: Option<f64>,
}

impl ResolvedStockSetup {
    pub(crate) fn selection(self) -> StockDefaults {
        self.selection
    }

    /// Apply before the ordinary film-specific normalization/calibration.
    /// In particular, B&W's channel-0 broadcast must run after this step.
    pub(crate) fn before_calibration(self, params: &mut RuntimeParams) {
        let halation = &mut params.film_render.halation;
        halation.halation_first_sigma_um = [self.halation_sigma; 3];
        if let Some(strength) = self.halation_strength {
            halation.halation_strength = strength;
        }
    }

    /// Apply only after the ordinary constructor has finished calibration.
    /// Reapplying writes fixed stock-specific values; it never compounds the scale.
    pub(crate) fn after_calibration(self, params: &mut RuntimeParams) {
        if let Some(scale) = self.slide_scale {
            let dir = &mut params.film_render.dir_couplers;
            dir.gamma_samelayer_rgb = [0.12 * scale, 0.08 * scale, 0.06 * scale];
            dir.gamma_interlayer_r_to_gb = [0.12 * scale, 0.06 * scale];
            dir.gamma_interlayer_g_to_rb = [0.08 * scale, 0.06 * scale];
            dir.gamma_interlayer_b_to_rg = [0.06 * scale, 0.06 * scale];
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::profile;
    use std::path::{Path, PathBuf};

    fn data_dir() -> PathBuf {
        Path::new(env!("CARGO_MANIFEST_DIR")).join("../../data")
    }

    fn film(stock: &str) -> Profile {
        profile::load_profile_by_name(&data_dir(), stock).unwrap()
    }

    #[test]
    fn physical_defaults_follow_stock_metadata() {
        let base = film("kodak_portra_400");
        let original_strength = [0.13, 0.07, 0.004];
        for (usage, sigma) in [("cine", 50.0), ("still", 65.0), ("unknown", 65.0)] {
            for (antihalation, expected) in [
                ("strong", [0.015, 0.005, 0.0]),
                ("weak", [0.08, 0.02, 0.0]),
                ("no", [0.30, 0.10, 0.015]),
                ("unknown", original_strength),
            ] {
                let mut profile = base.clone();
                profile.info.usage = usage.into();
                profile.info.antihalation = antihalation.into();
                for route in [StockRoute::Paper, StockRoute::NegativeScan] {
                    let mut params = RuntimeParams::default();
                    params.film_render.halation.halation_strength = original_strength;
                    params.camera.exposure_compensation_ev = 0.7;
                    params.film_render.halation.halation_amount = 0.4;
                    params.film_render.grain.output_sampling = Some([1.7, 0.6]);
                    let untouched = serde_json::to_value(&params).unwrap();
                    let selected = StockDefaults::Physical(route).resolve(&profile).unwrap();
                    selected.before_calibration(&mut params);
                    selected.after_calibration(&mut params);
                    assert_eq!(
                        params.film_render.halation.halation_first_sigma_um,
                        [sigma; 3]
                    );
                    assert_eq!(params.film_render.halation.halation_strength, expected);
                    let mut changed = serde_json::to_value(&params).unwrap();
                    changed["film_render"]["halation"]["halation_first_sigma_um"] =
                        untouched["film_render"]["halation"]["halation_first_sigma_um"].clone();
                    changed["film_render"]["halation"]["halation_strength"] =
                        untouched["film_render"]["halation"]["halation_strength"].clone();
                    assert_eq!(
                        changed, untouched,
                        "stock setup must not take over recipe controls"
                    );
                }
            }
        }
    }

    #[test]
    fn stock_specific_slide_controls_are_fixed_and_idempotent() {
        for (stock, scale) in [("fujifilm_provia_100f", 1.3), ("fujifilm_velvia_100", 0.9)] {
            let profile = film(stock);
            let selected = StockDefaults::Physical(StockRoute::Slide)
                .resolve(&profile)
                .unwrap();
            let mut params = RuntimeParams::default();
            params.film_render.dir_couplers.active = false;
            params.film_render.dir_couplers.amount = 0.37;
            selected.before_calibration(&mut params);
            let before = params.film_render.dir_couplers.gamma_samelayer_rgb;
            assert_eq!(
                before,
                [0.341, 0.324, 0.273],
                "slide DIR is a post-calibration step"
            );
            selected.after_calibration(&mut params);
            let dir = &params.film_render.dir_couplers;
            assert_eq!(
                dir.gamma_samelayer_rgb,
                [0.12 * scale, 0.08 * scale, 0.06 * scale]
            );
            assert_eq!(dir.gamma_interlayer_r_to_gb, [0.12 * scale, 0.06 * scale]);
            assert_eq!(dir.gamma_interlayer_g_to_rb, [0.08 * scale, 0.06 * scale]);
            assert_eq!(dir.gamma_interlayer_b_to_rg, [0.06 * scale, 0.06 * scale]);
            assert!(!dir.active);
            assert_eq!(dir.amount, 0.37);
            let once = serde_json::to_value(&params).unwrap();
            selected.after_calibration(&mut params);
            assert_eq!(serde_json::to_value(&params).unwrap(), once);
        }
    }

    #[test]
    fn physically_incompatible_stock_routes_are_rejected() {
        let negative = film("kodak_portra_400");
        assert!(matches!(
            StockDefaults::Physical(StockRoute::Slide).resolve(&negative),
            Err(StockSetupError::RouteMismatch { .. })
        ));
        for route in [StockRoute::Paper, StockRoute::NegativeScan] {
            assert!(matches!(
                StockDefaults::Physical(route).resolve(&film("fujifilm_provia_100f")),
                Err(StockSetupError::RouteMismatch { .. })
            ));
        }
    }
}
