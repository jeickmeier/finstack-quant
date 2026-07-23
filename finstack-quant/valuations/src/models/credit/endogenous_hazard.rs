//! Endogenous (leverage-dependent) hazard rate model.
//!
//! Provides a feedback loop where PIK accrual increases leverage, which in turn
//! increases the hazard rate and expected loss. Three mapping functions are
//! supported:
//!
//! - **Power law**: `lambda(L) = lambda_0 * (L / L_0)^beta`
//! - **Exponential**: `lambda(L) = lambda_0 * exp(beta * (L - L_0))`
//! - **Tabular**: Linear interpolation from empirical calibration with flat
//!   extrapolation at the edges.
//!
//! All computed hazard rates are finite: floored at 0.0 (never negative) and
//! capped at a large finite ceiling so a divergent exponential / power-law
//! mapping cannot produce a non-finite (`inf`/`NaN`) rate.

use finstack_quant_core::{Error, InputError, Result};
use serde::{Deserialize, Serialize};

// ---------------------------------------------------------------------------
// Types
// ---------------------------------------------------------------------------

/// Map from leverage to hazard rate.
#[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
pub enum LeverageHazardMap {
    /// `lambda(t) = lambda_0 * (L(t) / L_0)^beta`
    PowerLaw {
        /// Power-law exponent (`beta`).
        exponent: f64,
    },
    /// `lambda(t) = lambda_0 * exp(beta * (L(t) - L_0))`
    Exponential {
        /// Exponential sensitivity (`beta`).
        sensitivity: f64,
    },
    /// Tabular: linear interpolation from empirical calibration.
    Tabular {
        /// Leverage breakpoints (must be sorted ascending).
        leverage_points: Vec<f64>,
        /// Corresponding hazard rates at each breakpoint.
        hazard_points: Vec<f64>,
    },
}

/// Specification for endogenous (leverage-dependent) hazard rate.
///
/// Models the relationship between a firm's leverage and its instantaneous
/// hazard rate, enabling a feedback loop where PIK accrual increases the
/// notional (and hence leverage), which drives the hazard rate higher.
#[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
pub struct EndogenousHazardSpec {
    /// Base (reference) hazard rate `lambda_0`.
    base_hazard_rate: f64,
    /// Base (reference) leverage level `L_0`.
    base_leverage: f64,
    /// Mapping function from leverage to hazard rate.
    leverage_hazard_map: LeverageHazardMap,
}

// ---------------------------------------------------------------------------
// Implementation
// ---------------------------------------------------------------------------

impl EndogenousHazardSpec {
    // -- Convenience constructors -------------------------------------------

    /// Validate base parameters common to all parametric models.
    fn validate(base_hazard: f64, base_leverage: f64) -> Result<()> {
        if base_hazard < 0.0 {
            return Err(InputError::NegativeValue.into());
        }
        if base_leverage <= 0.0 {
            return Err(InputError::NonPositiveValue.into());
        }
        Ok(())
    }

    /// Create a power-law endogenous hazard spec.
    ///
    /// `lambda(L) = base_hazard * (L / base_leverage)^exponent`
    ///
    /// # Errors
    ///
    /// Returns an error if `base_hazard < 0` or `base_leverage <= 0`.
    pub fn power_law(base_hazard: f64, base_leverage: f64, exponent: f64) -> Result<Self> {
        Self::validate(base_hazard, base_leverage)?;
        Ok(Self {
            base_hazard_rate: base_hazard,
            base_leverage,
            leverage_hazard_map: LeverageHazardMap::PowerLaw { exponent },
        })
    }

    /// Create an exponential endogenous hazard spec.
    ///
    /// `lambda(L) = base_hazard * exp(sensitivity * (L - base_leverage))`
    ///
    /// # Errors
    ///
    /// Returns an error if `base_hazard < 0` or `base_leverage <= 0`.
    pub fn exponential(base_hazard: f64, base_leverage: f64, sensitivity: f64) -> Result<Self> {
        Self::validate(base_hazard, base_leverage)?;
        Ok(Self {
            base_hazard_rate: base_hazard,
            base_leverage,
            leverage_hazard_map: LeverageHazardMap::Exponential { sensitivity },
        })
    }

    /// Create a tabular endogenous hazard spec from empirical calibration.
    ///
    /// Uses linear interpolation between the given points and flat
    /// extrapolation beyond the edges. `base_hazard_rate` and `base_leverage`
    /// are derived from the first tabular point.
    ///
    /// # Errors
    ///
    /// Returns an error if vectors are empty or have different lengths.
    pub fn tabular(leverage_points: Vec<f64>, hazard_points: Vec<f64>) -> Result<Self> {
        if leverage_points.is_empty() || leverage_points.len() != hazard_points.len() {
            return Err(InputError::DimensionMismatch.into());
        }
        if let Some(bad) = hazard_points
            .iter()
            .find(|h| !(h.is_finite() && **h >= 0.0))
        {
            return Err(Error::Validation(format!(
                "tabular: hazard points must be finite and >= 0, got {bad}"
            )));
        }
        if leverage_points.windows(2).any(|w| !(w[1] > w[0])) {
            return Err(Error::Validation(
                "tabular: leverage points must be strictly increasing — \
                 interpolation assumes an ascending axis"
                    .to_string(),
            ));
        }
        let base_leverage = leverage_points[0];
        let base_hazard_rate = hazard_points[0];
        Ok(Self {
            base_hazard_rate,
            base_leverage,
            leverage_hazard_map: LeverageHazardMap::Tabular {
                leverage_points,
                hazard_points,
            },
        })
    }

    // -- Core computation ---------------------------------------------------

    /// Upper bound applied to every computed hazard rate.
    ///
    /// The exponential map `lambda_0 * exp(beta * (L - L_0))` overflows to
    /// `+inf` for a large `beta * delta_L`; the power-law map can likewise
    /// diverge. A non-finite hazard rate would silently poison every
    /// downstream survival-probability and expected-loss calculation, so the
    /// result is capped here. `1.0e6` per year already implies certain,
    /// effectively instantaneous default (survival `exp(-lambda * dt) ~ 0`
    /// for any non-trivial `dt`), so the cap does not distort economics.
    const MAX_HAZARD_RATE: f64 = 1.0e6;

    /// Compute the hazard rate at a given leverage level.
    ///
    /// The result is always finite, floored at 0.0 (never negative), and
    /// capped at `MAX_HAZARD_RATE` so a divergent
    /// mapping cannot produce a non-finite (`inf`/`NaN`) rate. A degenerate
    /// tabular table (empty, or with mismatched vector lengths — reachable
    /// only via `Deserialize`, since the constructor validates) yields `0.0`.
    pub fn hazard_at_leverage(&self, leverage: f64) -> f64 {
        let raw = match &self.leverage_hazard_map {
            LeverageHazardMap::PowerLaw { exponent } => {
                let ratio = (leverage / self.base_leverage).max(0.0);
                self.base_hazard_rate * ratio.powf(*exponent)
            }
            LeverageHazardMap::Exponential { sensitivity } => {
                self.base_hazard_rate * (*sensitivity * (leverage - self.base_leverage)).exp()
            }
            LeverageHazardMap::Tabular {
                leverage_points,
                hazard_points,
            } => tabular_interpolate(leverage_points, hazard_points, leverage),
        };
        // Order matters: `clamp` would propagate a `NaN` input, so handle the
        // non-finite case explicitly first. A `NaN` raw rate (e.g. `0 * inf`)
        // collapses to `0.0`; `+inf` is capped; finite values are floored.
        if raw.is_nan() {
            0.0
        } else {
            raw.clamp(0.0, Self::MAX_HAZARD_RATE)
        }
    }

    /// Compute the hazard rate after PIK accrual changes the notional.
    ///
    /// Leverage is computed as `accreted_notional / asset_value`.
    pub fn hazard_after_pik_accrual(&self, accreted_notional: f64, asset_value: f64) -> f64 {
        let leverage = accreted_notional / asset_value;
        self.hazard_at_leverage(leverage)
    }

    // -- Accessors ----------------------------------------------------------

    /// Returns the base (reference) hazard rate.
    pub fn base_hazard_rate(&self) -> f64 {
        self.base_hazard_rate
    }

    /// Returns the base (reference) leverage level.
    pub fn base_leverage(&self) -> f64 {
        self.base_leverage
    }

    /// Returns a reference to the leverage-to-hazard mapping.
    pub fn leverage_hazard_map(&self) -> &LeverageHazardMap {
        &self.leverage_hazard_map
    }
}

// ---------------------------------------------------------------------------
// Helper: tabular linear interpolation with flat extrapolation
// ---------------------------------------------------------------------------

/// Linear interpolation between tabular points with flat extrapolation at
/// the edges.
///
/// # Behaviour
///
/// - `xs` is expected to be sorted ascending.
/// - A degenerate table — empty, or with `xs.len() != ys.len()` — has no
///   well-defined hazard and returns `0.0` rather than panicking. The
///   [`tabular`](EndogenousHazardSpec::tabular) constructor rejects such
///   tables, but `#[derive(Deserialize)]` bypasses it, so this function is
///   defensively total: it never panics and never indexes out of bounds.
fn tabular_interpolate(xs: &[f64], ys: &[f64], x: f64) -> f64 {
    // Degenerate table: no data to interpolate. Walk both slices through the
    // same bounded range so a length mismatch can never index out of bounds.
    let n = xs.len();
    if n == 0 || n != ys.len() {
        return 0.0;
    }

    // Flat extrapolation below the first point / above the last point.
    // `get` keeps this panic-free even though `n >= 1` is established above.
    let (Some(&fx), Some(&fy)) = (xs.first(), ys.first()) else {
        return 0.0;
    };
    let (first_x, first_y) = (fx, fy);
    let (Some(&lx), Some(&ly)) = (xs.get(n - 1), ys.get(n - 1)) else {
        return 0.0;
    };
    let (last_x, last_y) = (lx, ly);
    if x <= first_x {
        return first_y;
    }
    if x >= last_x {
        return last_y;
    }

    // Find the bracketing interval and interpolate. `windows(2)` over paired
    // slices avoids any manual indexing.
    for (x_pair, y_pair) in xs.windows(2).zip(ys.windows(2)) {
        let (x0, x1) = (x_pair[0], x_pair[1]);
        let (y0, y1) = (y_pair[0], y_pair[1]);
        if x >= x0 && x <= x1 {
            let span = x1 - x0;
            // Guard a zero-width interval (duplicate breakpoints): fall back
            // to the left endpoint instead of dividing by zero.
            if span.abs() <= f64::EPSILON {
                return y0;
            }
            let t = (x - x0) / span;
            return y0 + t * (y1 - y0);
        }
    }

    // Fallback (should not be reached for valid sorted input).
    last_y
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    /// The tabular constructor must reject invalid tables up front instead
    /// of silently zero-clamping negative hazards at compute time or
    /// interpolating over an unsorted leverage axis.
    #[test]
    fn tabular_rejects_invalid_hazards_and_unsorted_leverage() {
        assert!(
            EndogenousHazardSpec::tabular(vec![1.0, 2.0], vec![0.02, -0.01]).is_err(),
            "negative hazard point must be rejected"
        );
        assert!(
            EndogenousHazardSpec::tabular(vec![1.0, 2.0], vec![0.02, f64::NAN]).is_err(),
            "non-finite hazard point must be rejected"
        );
        assert!(
            EndogenousHazardSpec::tabular(vec![2.0, 1.0], vec![0.02, 0.03]).is_err(),
            "non-increasing leverage points must be rejected"
        );
        assert!(
            EndogenousHazardSpec::tabular(vec![1.0, 2.0], vec![0.02, 0.03]).is_ok(),
            "a valid ascending table must be accepted"
        );
    }

    #[test]
    fn power_law_at_base_leverage_returns_base_hazard() {
        let spec = EndogenousHazardSpec::power_law(0.10, 1.5, 2.5).unwrap();
        assert!((spec.hazard_at_leverage(1.5) - 0.10).abs() < 1e-10);
    }

    #[test]
    fn power_law_increases_with_leverage() {
        let spec = EndogenousHazardSpec::power_law(0.10, 1.5, 2.5).unwrap();
        let h_low = spec.hazard_at_leverage(1.5);
        let h_high = spec.hazard_at_leverage(2.0);
        assert!(h_high > h_low, "h_low={h_low}, h_high={h_high}");
    }

    #[test]
    fn exponential_at_base_returns_base() {
        let spec = EndogenousHazardSpec::exponential(0.10, 1.5, 5.0).unwrap();
        assert!((spec.hazard_at_leverage(1.5) - 0.10).abs() < 1e-10);
    }

    #[test]
    fn exponential_increases_with_leverage() {
        let spec = EndogenousHazardSpec::exponential(0.10, 1.5, 5.0).unwrap();
        let h_low = spec.hazard_at_leverage(1.5);
        let h_high = spec.hazard_at_leverage(2.0);
        assert!(h_high > h_low);
    }

    #[test]
    fn pik_accrual_increases_hazard() {
        let spec = EndogenousHazardSpec::power_law(0.10, 1.5, 2.5).unwrap();
        let h_before = spec.hazard_after_pik_accrual(100.0, 66.67);
        let h_after = spec.hazard_after_pik_accrual(120.0, 66.67);
        assert!(
            h_after > h_before,
            "PIK accrual should increase hazard: before={h_before}, after={h_after}"
        );
    }

    #[test]
    fn tabular_interpolates() {
        let spec =
            EndogenousHazardSpec::tabular(vec![1.0, 1.5, 2.0, 3.0], vec![0.02, 0.05, 0.12, 0.30])
                .unwrap();
        let h = spec.hazard_at_leverage(1.75);
        assert!(h > 0.05 && h < 0.12, "h={h}");
    }

    #[test]
    fn tabular_flat_extrapolation() {
        let spec = EndogenousHazardSpec::tabular(vec![1.0, 2.0], vec![0.05, 0.15]).unwrap();
        let h_below = spec.hazard_at_leverage(0.5);
        let h_above = spec.hazard_at_leverage(5.0);
        assert!(
            (h_below - 0.05).abs() < 1e-10,
            "Below range: flat extrapolation"
        );
        assert!(
            (h_above - 0.15).abs() < 1e-10,
            "Above range: flat extrapolation"
        );
    }

    #[test]
    fn rejects_invalid_inputs() {
        assert!(EndogenousHazardSpec::power_law(-0.10, 1.5, 2.5).is_err());
        assert!(EndogenousHazardSpec::power_law(0.10, 0.0, 2.5).is_err());
        assert!(EndogenousHazardSpec::exponential(0.10, -1.0, 5.0).is_err());
        assert!(EndogenousHazardSpec::tabular(vec![], vec![]).is_err());
        assert!(EndogenousHazardSpec::tabular(vec![1.0], vec![0.05, 0.10]).is_err());
    }

    #[test]
    fn deserialized_empty_tabular_does_not_panic() {
        // The `tabular()` constructor validates, but `#[derive(Deserialize)]`
        // bypasses it: a JSON spec with empty `leverage_points` /
        // `hazard_points` deserialises into a `Tabular` map whose
        // `hazard_at_leverage` previously hit a panicking `assert!`. It must
        // instead return a finite, non-negative value.
        let json = r#"{
            "base_hazard_rate": 0.05,
            "base_leverage": 1.5,
            "leverage_hazard_map": { "Tabular": {
                "leverage_points": [],
                "hazard_points": []
            }}
        }"#;
        let spec: EndogenousHazardSpec =
            serde_json::from_str(json).expect("malformed-but-valid JSON deserialises");
        let h = spec.hazard_at_leverage(2.0);
        assert!(
            h.is_finite() && h >= 0.0,
            "empty tabular hazard must be finite and non-negative, got {h}"
        );
    }

    #[test]
    fn deserialized_mismatched_tabular_does_not_panic() {
        // A `Tabular` map whose two vectors differ in length must not panic
        // (the old `assert!` / panicking indexing path).
        let json = r#"{
            "base_hazard_rate": 0.05,
            "base_leverage": 1.5,
            "leverage_hazard_map": { "Tabular": {
                "leverage_points": [1.0, 2.0, 3.0],
                "hazard_points": [0.05]
            }}
        }"#;
        let spec: EndogenousHazardSpec =
            serde_json::from_str(json).expect("malformed-but-valid JSON deserialises");
        let h = spec.hazard_at_leverage(2.5);
        assert!(
            h.is_finite() && h >= 0.0,
            "mismatched tabular hazard must be finite and non-negative, got {h}"
        );
    }

    #[test]
    fn exponential_hazard_does_not_overflow_to_infinity() {
        // `lambda_0 * exp(beta * (L - L_0))` overflows to `+inf` for a large
        // `beta * delta_L`. A non-finite hazard rate silently poisons every
        // downstream survival-probability and expected-loss calculation, so
        // the model must cap it at a large finite value instead.
        let spec = EndogenousHazardSpec::exponential(0.05, 1.0, 50.0).unwrap();
        let h = spec.hazard_at_leverage(100.0);
        assert!(
            h.is_finite(),
            "exponential hazard must stay finite, not overflow to inf, got {h}"
        );
        assert!(h >= 0.0, "hazard must be non-negative, got {h}");
    }
}
