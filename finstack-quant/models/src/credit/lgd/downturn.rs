//! Downturn LGD adjustments.
//!
//! Provides a stressed-LGD approximation and a regulatory-floor method for
//! computing downturn LGD from base (through-the-cycle) estimates.
//!
//! # Methodology Note
//!
//! The stressed method here is a **proprietary mean-plus-multiple-of-
//! Bernoulli-standard-deviation approximation**: it adds
//! `sensitivity · sqrt(rho) · Phi⁻¹(q) · sqrt(LGD·(1−LGD))` to the base LGD,
//! where `sqrt(LGD·(1−LGD))` is the standard deviation of a Bernoulli loss
//! indicator with mean LGD. It is **not** the Frye-Jacobs (2012) model: the
//! true F-J LGD function is `cLGD = Φ(Φ⁻¹(cDR) − k) / cDR`, which links
//! conditional LGD to the conditional default rate through a single
//! "LGD risk index" k — that model is not implemented here.
//!
//! # Related Literature
//!
//! - Frye, J. & Jacobs, M. (2012). "Credit Loss and Systematic Loss Given
//!   Default." Journal of Credit Risk, 8(1), 109-140. (Related literature
//!   only; see Methodology Note above.)

use finstack_quant_core::error::InputError;
use finstack_quant_core::math::special_functions::standard_normal_inv_cdf;
use finstack_quant_core::Result;

/// Method for computing downturn LGD from base (through-the-cycle) LGD.
#[derive(Debug, Clone, Copy, serde::Serialize, serde::Deserialize)]
#[cfg_attr(feature = "json-schema", derive(schemars::JsonSchema))]
#[serde(rename_all = "snake_case", deny_unknown_fields)]
pub enum DownturnMethod {
    /// Proprietary stressed-LGD approximation (mean plus a multiple of the
    /// Bernoulli standard deviation).
    ///
    /// ```text
    /// LGD_downturn = LGD_base + sensitivity * sqrt(asset_correlation)
    ///              * Phi_inv(stress_quantile) * sqrt(LGD_base * (1 - LGD_base))
    ///   ```
    ///
    /// `sqrt(LGD_base * (1 - LGD_base))` is the standard deviation of a
    /// Bernoulli loss indicator with mean `LGD_base`. The adjustment captures
    /// the systematic component heuristically: in downturns, recoveries fall
    /// because the same macro factor driving defaults also depresses asset
    /// values.
    ///
    /// This is *not* the Frye-Jacobs (2012) model (whose LGD function is
    /// `cLGD = Φ(Φ⁻¹(cDR) − k) / cDR`); Frye & Jacobs (2012) is related
    /// literature only. See the module-level Methodology Note.
    StressedApproximation {
        /// Asset correlation (rho). Typical: 0.10-0.24 per Basel.
        asset_correlation: f64,
        /// LGD sensitivity to systematic factor. Typical: 0.3-0.5.
        lgd_sensitivity: f64,
        /// Stress quantile for downturn scenario. Typical: 0.999 (99.9th percentile).
        stress_quantile: f64,
    },

    /// Regulatory floor: LGD_downturn = max(LGD_base + add_on, floor).
    ///
    /// Basel III mandates that downturn LGD cannot fall below certain floors.
    /// The add-on is a flat increment over the base LGD.
    RegulatoryFloor {
        /// Flat add-on over base LGD. Typical: 0.05-0.10 (5-10pp).
        add_on: f64,
        /// Absolute LGD floor. Typical: 0.10 for secured, 0.25 for unsecured.
        floor: f64,
    },
}

/// Downturn LGD adjuster.
///
/// Wraps a base LGD estimate and applies a downturn adjustment method
/// to produce a stressed LGD for capital calculations.
#[derive(Debug, Clone, Copy, serde::Serialize, serde::Deserialize)]
#[cfg_attr(feature = "json-schema", derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields)]
pub struct DownturnLgd {
    /// Downturn adjustment method.
    method: DownturnMethod,
}

impl DownturnLgd {
    /// Create a stressed-LGD downturn adjuster (proprietary mean-plus-
    /// multiple-of-Bernoulli-stdev approximation; see
    /// [`DownturnMethod::StressedApproximation`] for the formula).
    ///
    /// # Arguments
    ///
    /// * `asset_correlation` - Basel asset correlation `rho` in (0, 1),
    ///   typically 0.10-0.24.
    /// * `lgd_sensitivity` - Non-negative sensitivity of LGD to the systematic
    ///   factor, typically 0.3-0.5.
    /// * `stress_quantile` - Downturn scenario quantile in (0, 1), typically
    ///   0.999 (99.9th percentile).
    ///
    /// # Errors
    ///
    /// Returns an error if:
    /// - `asset_correlation` is not in (0, 1)
    /// - `lgd_sensitivity` is negative
    /// - `stress_quantile` is not in (0, 1)
    pub fn stressed(
        asset_correlation: f64,
        lgd_sensitivity: f64,
        stress_quantile: f64,
    ) -> Result<Self> {
        let dt = Self {
            method: DownturnMethod::StressedApproximation {
                asset_correlation,
                lgd_sensitivity,
                stress_quantile,
            },
        };
        dt.validate()?;
        Ok(dt)
    }

    /// Create a regulatory-floor downturn adjuster.
    ///
    /// # Arguments
    ///
    /// * `add_on` - Non-negative flat increment over the base LGD, typically
    ///   0.05-0.10 (5-10 percentage points).
    /// * `floor` - Absolute LGD floor in [0, 1], typically 0.10 for secured
    ///   and 0.25 for unsecured exposures.
    ///
    /// # Errors
    ///
    /// Returns an error if `add_on < 0`, `floor < 0`, or `floor > 1`.
    pub fn regulatory_floor(add_on: f64, floor: f64) -> Result<Self> {
        let dt = Self {
            method: DownturnMethod::RegulatoryFloor { add_on, floor },
        };
        dt.validate()?;
        Ok(dt)
    }

    /// Validate the invariants of the configured downturn method's parameters.
    ///
    /// [`DownturnLgd::stressed`] and [`DownturnLgd::regulatory_floor`]
    /// already enforce these invariants at construction time, but
    /// `DownturnLgd` also derives `Deserialize` and can be produced directly
    /// from untrusted JSON (e.g. embedded in a config struct's
    /// `#[derive(Deserialize)]` field), bypassing the constructors entirely.
    /// Callers that accept a `DownturnLgd` deserialized from an external
    /// boundary must call this before using it, since an invalid value
    /// (e.g. negative `asset_correlation`, or `stress_quantile == 1.0`)
    /// otherwise propagates `NaN` or `+/-inf` through [`DownturnLgd::adjust`]
    /// silently.
    ///
    /// # Errors
    ///
    /// Returns an error if:
    /// - `StressedApproximation`: `asset_correlation` is not finite or not in (0, 1),
    ///   `lgd_sensitivity` is not finite or negative, or
    ///   `stress_quantile` is not finite or not in (0, 1)
    /// - `RegulatoryFloor`: `add_on` is not finite or negative, or
    ///   `floor` is not finite or not in \[0, 1\]
    pub fn validate(&self) -> Result<()> {
        match self.method {
            DownturnMethod::StressedApproximation {
                asset_correlation,
                lgd_sensitivity,
                stress_quantile,
            } => {
                if !asset_correlation.is_finite()
                    || asset_correlation <= 0.0
                    || asset_correlation >= 1.0
                {
                    return Err(InputError::Invalid.into());
                }
                if !lgd_sensitivity.is_finite() || lgd_sensitivity < 0.0 {
                    return Err(InputError::NegativeValue.into());
                }
                if !stress_quantile.is_finite() || stress_quantile <= 0.0 || stress_quantile >= 1.0
                {
                    return Err(InputError::Invalid.into());
                }
            }
            DownturnMethod::RegulatoryFloor { add_on, floor } => {
                if !add_on.is_finite() || add_on < 0.0 {
                    return Err(InputError::NegativeValue.into());
                }
                if !floor.is_finite() || !(0.0..=1.0).contains(&floor) {
                    return Err(InputError::Invalid.into());
                }
            }
        }
        Ok(())
    }

    /// Load a downturn LGD preset from the credit assumptions registry.
    ///
    /// # Arguments
    ///
    /// * `id` - Exact registry identifier of the downturn preset, such as
    ///   `"basel_secured"`. An unknown ID never falls back to a default preset.
    ///
    /// # Errors
    ///
    /// Returns a not-found error (`InputError::NotFound`) if `id` is unknown,
    /// and a validation error if the registry cannot load or the preset uses
    /// an unsupported method or out-of-range parameters.
    pub fn from_registry_id(id: &str) -> Result<Self> {
        let preset = crate::credit::registry::embedded_registry()?.downturn_lgd_preset(id)?;
        if preset.method == "regulatory_floor" {
            Self::regulatory_floor(preset.add_on, preset.floor)
        } else {
            Err(finstack_quant_core::Error::Validation(format!(
                "unsupported downturn LGD preset method '{}'",
                preset.method
            )))
        }
    }

    /// Basel III secured asset floor (10% LGD floor, 8% add-on).
    pub fn basel_secured() -> Result<Self> {
        Self::from_registry_id(
            crate::credit::registry::embedded_registry()?.default_downturn_lgd_id(),
        )
    }

    /// Basel III unsecured floor (25% LGD floor, 5% add-on).
    pub fn basel_unsecured() -> Result<Self> {
        Self::from_registry_id("basel_unsecured")
    }

    /// Apply downturn adjustment to a base LGD.
    ///
    /// The result is clamped to \[0, 1\].
    ///
    /// # Arguments
    ///
    /// * `base_lgd` - Through-the-cycle LGD estimate in \[0, 1\].
    ///
    /// # Errors
    ///
    /// Returns an error if `base_lgd` is not in \[0, 1\].
    pub fn adjust(&self, base_lgd: f64) -> Result<f64> {
        if !base_lgd.is_finite() || !(0.0..=1.0).contains(&base_lgd) {
            return Err(InputError::Invalid.into());
        }
        let adjusted = match self.method {
            DownturnMethod::StressedApproximation {
                asset_correlation,
                lgd_sensitivity,
                stress_quantile,
            } => {
                let z = standard_normal_inv_cdf(stress_quantile);
                let systematic = lgd_sensitivity
                    * asset_correlation.sqrt()
                    * z
                    * (base_lgd * (1.0 - base_lgd)).sqrt();
                base_lgd + systematic
            }
            DownturnMethod::RegulatoryFloor { add_on, floor } => (base_lgd + add_on).max(floor),
        };
        Ok(adjusted.clamp(0.0, 1.0))
    }

    /// The downturn method in use.
    pub fn method(&self) -> &DownturnMethod {
        &self.method
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn stressed_increases_lgd() {
        let dt = DownturnLgd::stressed(0.15, 0.4, 0.999).expect("valid params");
        let base = 0.45;
        let adjusted = dt.adjust(base).expect("valid base");
        assert!(
            adjusted > base,
            "downturn LGD {} should exceed base {}",
            adjusted,
            base
        );
        assert!(adjusted <= 1.0);
    }

    #[test]
    fn stressed_result_in_range() {
        let dt = DownturnLgd::stressed(0.15, 0.4, 0.999).expect("valid params");
        for &base in &[0.0, 0.10, 0.30, 0.50, 0.70, 0.90, 1.0] {
            let adj = dt.adjust(base).expect("valid base");
            assert!(
                (0.0..=1.0).contains(&adj),
                "adjusted {} out of [0,1] for base {}",
                adj,
                base
            );
        }
    }

    #[test]
    fn regulatory_floor_add_on() {
        let dt = DownturnLgd::regulatory_floor(0.08, 0.25).expect("valid params");

        // base 0.30 + 0.08 = 0.38, max(0.38, 0.25) = 0.38
        let adj = dt.adjust(0.30).expect("valid");
        assert!((adj - 0.38).abs() < 1e-12, "expected 0.38, got {}", adj);
    }

    #[test]
    fn regulatory_floor_binding() {
        let dt = DownturnLgd::regulatory_floor(0.08, 0.25).expect("valid params");

        // base 0.10 + 0.08 = 0.18, max(0.18, 0.25) = 0.25
        let adj = dt.adjust(0.10).expect("valid");
        assert!((adj - 0.25).abs() < 1e-12, "expected 0.25, got {}", adj);
    }

    #[test]
    fn downturn_monotonicity() {
        let dt = DownturnLgd::stressed(0.15, 0.4, 0.999).expect("valid params");
        let bases = [0.10, 0.20, 0.30, 0.40, 0.50, 0.60, 0.70, 0.80, 0.90];
        let adjusted: Vec<f64> = bases
            .iter()
            .map(|&b| dt.adjust(b).expect("valid"))
            .collect();

        for i in 1..adjusted.len() {
            assert!(
                adjusted[i] >= adjusted[i - 1],
                "monotonicity violated: adj[{}]={} < adj[{}]={}",
                i,
                adjusted[i],
                i - 1,
                adjusted[i - 1]
            );
        }
    }

    #[test]
    fn downturn_validation_rejects_invalid() {
        // asset_correlation out of range
        assert!(DownturnLgd::stressed(0.0, 0.4, 0.999).is_err());
        assert!(DownturnLgd::stressed(1.0, 0.4, 0.999).is_err());
        assert!(DownturnLgd::stressed(-0.1, 0.4, 0.999).is_err());

        // negative sensitivity
        assert!(DownturnLgd::stressed(0.15, -0.1, 0.999).is_err());

        // stress_quantile out of range
        assert!(DownturnLgd::stressed(0.15, 0.4, 0.0).is_err());
        assert!(DownturnLgd::stressed(0.15, 0.4, 1.0).is_err());

        // regulatory floor: negative add_on
        assert!(DownturnLgd::regulatory_floor(-0.01, 0.25).is_err());

        // regulatory floor: floor out of range
        assert!(DownturnLgd::regulatory_floor(0.05, -0.1).is_err());
        assert!(DownturnLgd::regulatory_floor(0.05, 1.1).is_err());
        for bad in [f64::NAN, f64::INFINITY, f64::NEG_INFINITY] {
            assert!(DownturnLgd::stressed(bad, 0.4, 0.999).is_err());
            assert!(DownturnLgd::stressed(0.15, bad, 0.999).is_err());
            assert!(DownturnLgd::stressed(0.15, 0.4, bad).is_err());
            assert!(DownturnLgd::regulatory_floor(bad, 0.25).is_err());
            assert!(DownturnLgd::regulatory_floor(0.05, bad).is_err());
        }
    }

    #[test]
    fn downturn_adjust_rejects_invalid_base() {
        let dt = DownturnLgd::stressed(0.15, 0.4, 0.999).expect("valid");
        assert!(dt.adjust(-0.1).is_err());
        assert!(dt.adjust(1.1).is_err());
        assert!(dt.adjust(f64::NAN).is_err());
    }

    #[test]
    fn basel_presets_construct() {
        let secured = DownturnLgd::basel_secured().expect("valid");
        let unsecured = DownturnLgd::basel_unsecured().expect("valid");

        // Both should adjust a moderate base LGD
        let adj_s = secured.adjust(0.20).expect("valid");
        let adj_u = unsecured.adjust(0.20).expect("valid");

        // Secured: max(0.20 + 0.08, 0.10) = 0.28
        assert!((adj_s - 0.28).abs() < 1e-12);
        // Unsecured: max(0.20 + 0.05, 0.25) = 0.25
        assert!((adj_u - 0.25).abs() < 1e-12);
    }

    #[test]
    fn validate_rejects_deserialized_negative_correlation() {
        // Deserialization bypasses the stressed constructor entirely, so a
        // struct built directly (mirroring what serde produces from
        // untrusted JSON) must still be caught by `validate`.
        let json = r#"{"method":{"stressed_approximation":{"asset_correlation":-0.5,"lgd_sensitivity":0.4,"stress_quantile":0.999}}}"#;
        let dt: DownturnLgd = serde_json::from_str(json).expect("deserializes despite bad data");
        let err = dt
            .validate()
            .expect_err("negative asset_correlation must fail validate");
        assert_eq!(err, InputError::Invalid.into());
    }

    #[test]
    fn validate_rejects_deserialized_unit_stress_quantile() {
        // stress_quantile = 1.0 pins Phi^-1(q) at +infinity, which would
        // silently propagate through `adjust` as LGD = 1.0 for any base.
        let json = r#"{"method":{"stressed_approximation":{"asset_correlation":0.15,"lgd_sensitivity":0.4,"stress_quantile":1.0}}}"#;
        let dt: DownturnLgd = serde_json::from_str(json).expect("deserializes despite bad data");
        let err = dt
            .validate()
            .expect_err("stress_quantile = 1.0 must fail validate");
        assert_eq!(err, InputError::Invalid.into());
    }

    #[test]
    fn downturn_serialization_roundtrip() {
        let dt = DownturnLgd::stressed(0.15, 0.4, 0.999).expect("valid");
        let json = serde_json::to_string(&dt).expect("serialize");
        let dt2: DownturnLgd = serde_json::from_str(&json).expect("deserialize");

        let base = 0.45;
        assert!((dt.adjust(base).expect("ok") - dt2.adjust(base).expect("ok")).abs() < 1e-12);
    }
}
