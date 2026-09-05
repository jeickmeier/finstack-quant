//! Margin Valuation Adjustment (MVA): funding cost of initial margin.
//!
//! Posting initial margin (IM) under the BCBS-IOSCO uncleared-margin rules ties
//! up funding for the life of the netting set. MVA prices that cost:
//!
//! ```text
//! MVA = ∫₀ᵀ λ_B(t) · E[IM(t)] · DF(t) · S(t) dt
//! ```
//!
//! where `λ_B(t)` is the bank's funding spread (decimal, from bp inputs),
//! `E[IM(t)]` the expected IM profile, `DF(t)` the risk-free discount factor,
//! and `S(t)` the survival probability (optional; `S ≡ 1` when omitted).
//! [`compute_mva`](crate::xva::mva::compute_mva) conditions on the bank's own
//! survival only; the bilateral
//! engine ([`crate::xva::cva::compute_bilateral_xva`]) uses joint
//! (first-to-default) survival `S_B(t)·S_C(t)`, since a counterparty default
//! terminates the netting set and returns the posted margin.
//!
//! `E[IM(t)]` is built deterministically from the current ISDA SIMM number and
//! a decay profile ([`im_profile_from_simm`](crate::xva::mva::im_profile_from_simm))
//! — the standard practitioner approximation. Per-path IM from a stochastic
//! exposure engine is not modeled here; callers holding a simulated IM
//! distribution supply its path mean as the profile.
//!
//! # Integration convention
//!
//! [`compute_mva`](crate::xva::mva::compute_mva) uses the same midpoint/trapezoid style as the CVA engine
//! (`xva::cva`): per bucket `[tᵢ₋₁, tᵢ]` it multiplies bucket-midpoint values
//! of spread, IM, DF, and survival by `Δt`. IM is treated as flat
//! (left-constant) before the first grid point; include a small first grid
//! point (e.g. `1.0/365.0`) if exact `t = 0` anchoring matters.
//!
//! # Aggregation
//!
//! MVA is a positive funding cost using the same sign convention as CVA and
//! FVA. [`crate::xva::cva::compute_bilateral_xva`] computes it automatically
//! whenever [`crate::xva::types::FundingConfig::im_profile`] is set and reports
//! `CVA − DVA + FVA + MVA` in
//! [`crate::xva::types::XvaResult::total_xva`].
//!
//! # Model Boundaries
//!
//! **Counterparty-posted IM** is not represented: the caller-supplied
//! [`crate::xva::types::ExposureProfile`] is taken as the net exposure, so
//! counterparty-posted SIMM IM does not reduce EPE gap risk and gap risk is
//! overstated for UMR counterparties.
//!
//! # References
//!
//! - Green, A. (2015). *XVA: Credit, Funding and Capital Valuation
//!   Adjustments*. Wiley. Chapter 10 (MVA). `docs/REFERENCES.md#green-xva`
//! - Andersen, L., Pykhtin, M., & Sokol, A. (2017). "Rethinking the margin
//!   period of risk." *Journal of Credit Risk*, 13(1).
//! - ISDA SIMM v2.6: `docs/REFERENCES.md#isda-simm`
//! - Green XVA: `docs/REFERENCES.md#green-xva`

use finstack_quant_core::currency::Currency;
use finstack_quant_core::market_data::term_structures::{DiscountCurve, HazardCurve};

use crate::calculators::im::simm::SimmCalculator;
use crate::types::SimmSensitivities;

/// Deterministic decay applied to today's SIMM IM to approximate `E[IM(t)]`.
///
/// `IM(t) = IM(0) × factor(t)`. This is the standard first-order practitioner
/// approximation for MVA (Green 2015, ch. 10): IM shrinks as the portfolio's
/// remaining risk runs off.
///
/// # Variants
///
/// | Variant | `factor(t)` | Use case |
/// |---|---|---|
/// | `Constant` | `1` | Evergreen / constantly re-hedged books |
/// | `LinearToMaturity` | `max(1 − t/T, 0)` | Amortizing linear-risk books |
/// | `SqrtTime` | `sqrt(max(1 − t/T, 0))` | DV01-style risk ∝ √(remaining time) |
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
#[cfg_attr(feature = "json-schema", derive(schemars::JsonSchema))]
#[serde(rename_all = "snake_case", deny_unknown_fields)]
pub enum ImDecayProfile {
    /// IM stays at today's level for the whole horizon.
    Constant,
    /// IM decays linearly to zero at `maturity_years`.
    LinearToMaturity {
        /// Portfolio maturity `T` in years (must be positive and finite).
        maturity_years: f64,
    },
    /// IM decays like the square root of remaining time to `maturity_years`.
    SqrtTime {
        /// Portfolio maturity `T` in years (must be positive and finite).
        maturity_years: f64,
    },
}

impl ImDecayProfile {
    /// Decay factor at time `t` (years). Always in `[0, 1]` for `t ≥ 0`.
    ///
    /// # References
    ///
    /// - Green, A. (2015). *XVA*. Wiley. Chapter 10. `docs/REFERENCES.md#green-xva`
    pub fn factor(&self, t: f64) -> f64 {
        match self {
            Self::Constant => 1.0,
            Self::LinearToMaturity { maturity_years } => (1.0 - t / maturity_years).max(0.0),
            Self::SqrtTime { maturity_years } => (1.0 - t / maturity_years).max(0.0).sqrt(),
        }
    }

    /// Validate decay parameters.
    ///
    /// # Errors
    ///
    /// Returns an error if `maturity_years` is non-positive or non-finite.
    pub fn validate(&self) -> finstack_quant_core::Result<()> {
        match self {
            Self::Constant => Ok(()),
            Self::LinearToMaturity { maturity_years } | Self::SqrtTime { maturity_years } => {
                if !maturity_years.is_finite() || *maturity_years <= 0.0 {
                    return Err(finstack_quant_core::Error::Validation(format!(
                        "ImDecayProfile: maturity_years {maturity_years} must be positive and finite"
                    )));
                }
                Ok(())
            }
        }
    }
}

/// Expected initial-margin profile `E[IM(t)]` on a time grid.
///
/// Values are in the aggregation currency chosen when the profile was built
/// (e.g. the `currency` argument of [`im_profile_from_simm`]).
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
#[cfg_attr(feature = "json-schema", derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields)]
pub struct ImProfile {
    /// Time points in years from the valuation date (strictly increasing, positive).
    pub times: Vec<f64>,
    /// Expected IM at each time point (non-negative, finite).
    pub im_values: Vec<f64>,
}

impl ImProfile {
    /// Validate internal consistency.
    ///
    /// # Errors
    ///
    /// Returns an error if the profile is empty, lengths differ, times are not
    /// strictly increasing and positive, or IM values are negative/non-finite.
    pub fn validate(&self) -> finstack_quant_core::Result<()> {
        if self.times.is_empty() {
            return Err(finstack_quant_core::Error::Validation(
                "ImProfile: times must not be empty".into(),
            ));
        }
        if self.im_values.len() != self.times.len() {
            return Err(finstack_quant_core::Error::Validation(format!(
                "ImProfile: vector lengths must be equal (times={}, im_values={})",
                self.times.len(),
                self.im_values.len()
            )));
        }
        for (i, &t) in self.times.iter().enumerate() {
            if !t.is_finite() || t <= 0.0 {
                return Err(finstack_quant_core::Error::Validation(format!(
                    "ImProfile: times[{i}] = {t} must be positive and finite"
                )));
            }
            if i > 0 && t <= self.times[i - 1] {
                return Err(finstack_quant_core::Error::Validation(format!(
                    "ImProfile: times must be strictly increasing at index {i}"
                )));
            }
        }
        for (i, &im) in self.im_values.iter().enumerate() {
            if !im.is_finite() || im < 0.0 {
                return Err(finstack_quant_core::Error::Validation(format!(
                    "ImProfile: im_values[{i}] = {im} must be non-negative and finite"
                )));
            }
        }
        Ok(())
    }
}

/// Build a deterministic IM profile from current SIMM sensitivities:
/// `IM(t) = SIMM(sensitivities) × decay(t)`.
///
/// The base IM is `calculator.calculate_from_sensitivities_parts(sensitivities,
/// currency).expect("valid sensitivity fixture").0` — the full cross-risk-class ISDA SIMM aggregate.
///
/// # Arguments
///
/// * `calculator` - SIMM calculator (fixes version / registry parameters)
/// * `sensitivities` - Current portfolio sensitivities in SIMM buckets
/// * `currency` - Aggregation currency for the SIMM total
/// * `decay` - Deterministic decay profile applied to the base IM
/// * `time_grid` - Strictly increasing positive year fractions
///
/// # Errors
///
/// Returns an error if `decay` or `time_grid` fails validation.
///
/// # References
///
/// - ISDA SIMM v2.6: `docs/REFERENCES.md#isda-simm`
/// - Green, A. (2015). *XVA*. Wiley. Chapter 10. `docs/REFERENCES.md#green-xva`
pub fn im_profile_from_simm(
    calculator: &SimmCalculator,
    sensitivities: &SimmSensitivities,
    currency: Currency,
    decay: &ImDecayProfile,
    time_grid: &[f64],
) -> finstack_quant_core::Result<ImProfile> {
    decay.validate()?;
    validate_time_grid(time_grid)?;
    sensitivities.validate()?;
    let (base_im, _breakdown) =
        calculator.calculate_from_sensitivities_parts(sensitivities, currency)?;
    let profile = ImProfile {
        times: time_grid.to_vec(),
        im_values: time_grid
            .iter()
            .map(|&t| base_im * decay.factor(t))
            .collect(),
    };
    profile.validate()?;
    Ok(profile)
}

fn validate_time_grid(time_grid: &[f64]) -> finstack_quant_core::Result<()> {
    if time_grid.is_empty() {
        return Err(finstack_quant_core::Error::Validation(
            "im_profile_from_simm: time_grid must not be empty".into(),
        ));
    }
    for (i, &t) in time_grid.iter().enumerate() {
        if !t.is_finite() || t <= 0.0 {
            return Err(finstack_quant_core::Error::Validation(format!(
                "im_profile_from_simm: time_grid[{i}] = {t} must be positive and finite"
            )));
        }
        if i > 0 && t <= time_grid[i - 1] {
            return Err(finstack_quant_core::Error::Validation(format!(
                "im_profile_from_simm: time_grid must be strictly increasing at index {i}"
            )));
        }
    }
    Ok(())
}

/// Result of an MVA computation.
///
/// All monetary quantities are f64 in the IM profile's currency, matching the
/// convention of [`crate::xva::types::XvaResult`].
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
#[cfg_attr(feature = "json-schema", derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields)]
pub struct MvaResult {
    /// MVA (positive = lifetime funding cost of posting IM).
    pub mva: f64,
    /// Time-weighted average IM over the profile horizon:
    /// `(1/T) ∫₀ᵀ IM(t) dt` under the same trapezoid convention as `mva`.
    pub average_im: f64,
    /// Echo of the IM profile used: `(time, IM(t))` pairs.
    pub im_profile: Vec<(f64, f64)>,
}

/// Compute MVA over an expected-IM profile.
///
/// ```text
/// MVA = Σᵢ s_mid(tᵢ) · IM_mid(tᵢ) · DF_mid(tᵢ) · S_mid(tᵢ) · Δtᵢ
/// ```
///
/// midpoint/trapezoid on the profile grid with a `t = 0` bucket edge
/// (`DF(0) = 1`, `S(0) = 1`, IM flat-before-first-point per the module docs).
///
/// # Arguments
///
/// * `im_profile` - Expected IM profile `E[IM(t)]` (from
///   [`im_profile_from_simm`], or a caller-supplied mean per-path IM)
/// * `funding_spread_curve` - `(time_years, spread_bp)` pairs, linearly
///   interpolated with flat extrapolation; a single pair means a flat spread
/// * `discount_curve` - Risk-free discount curve
/// * `survival_curve` - Optional bank (own) hazard curve; when `None`, `S ≡ 1`
///
/// # Errors
///
/// Returns an error if the profile or spread curve fails validation, or if any
/// curve evaluation returns a non-finite value.
///
/// # Relationship to `compute_bilateral_xva`
///
/// This is the **unilateral** form: it conditions only on the bank's own
/// survival. [`crate::xva::cva::compute_bilateral_xva`] computes MVA itself
/// whenever [`crate::xva::types::FundingConfig::im_profile`] is set, weights it
/// by *joint* survival (consistent with its FVA leg), and folds the result into
/// [`crate::xva::types::XvaResult::total_xva`].
/// Prefer that entry point for netting-set-level XVA; use this function for
/// standalone MVA analysis.
///
/// # References
///
/// - Green, A. (2015). *XVA*. Wiley. Chapter 10, eq. (10.4)-(10.7). `docs/REFERENCES.md#green-xva`
pub fn compute_mva(
    im_profile: &ImProfile,
    funding_spread_curve: &[(f64, f64)],
    discount_curve: &DiscountCurve,
    survival_curve: Option<&HazardCurve>,
) -> finstack_quant_core::Result<MvaResult> {
    compute_mva_internal(
        im_profile,
        funding_spread_curve,
        discount_curve,
        survival_curve,
        None,
    )
}

/// Compute MVA with optional joint-survival weighting for the bilateral engine.
pub(crate) fn compute_mva_internal(
    im_profile: &ImProfile,
    funding_spread_curve: &[(f64, f64)],
    discount_curve: &DiscountCurve,
    survival_curve: Option<&HazardCurve>,
    counterparty_survival_curve: Option<&HazardCurve>,
) -> finstack_quant_core::Result<MvaResult> {
    im_profile.validate()?;
    validate_spread_curve(funding_spread_curve)?;

    let n = im_profile.times.len();
    let mut mva = 0.0;
    let mut im_time_integral = 0.0;
    let mut prev_t = 0.0;
    // IM is flat (left-constant) before the first grid point (see module docs).
    let mut prev_im = im_profile.im_values[0];
    let mut prev_df = 1.0;
    let mut prev_own_sp = 1.0;
    let mut prev_counterparty_sp = 1.0;

    for i in 0..n {
        let t = im_profile.times[i];
        let im_t = im_profile.im_values[i];

        let df_t = discount_curve.df(t);
        if !df_t.is_finite() {
            return Err(finstack_quant_core::Error::Validation(format!(
                "MVA: non-finite discount factor at t={t}: DF(t)={df_t}"
            )));
        }
        let own_sp_t = match survival_curve {
            Some(curve) => {
                let sp = curve.sp(t);
                if !sp.is_finite() {
                    return Err(finstack_quant_core::Error::Validation(format!(
                        "MVA: non-finite survival probability at t={t}: S(t)={sp}"
                    )));
                }
                sp
            }
            None => 1.0,
        };
        let counterparty_sp_t = match counterparty_survival_curve {
            Some(curve) => {
                let sp = curve.sp(t);
                if !sp.is_finite() {
                    return Err(finstack_quant_core::Error::Validation(format!(
                        "MVA: non-finite counterparty survival probability at t={t}: S_c(t)={sp}"
                    )));
                }
                sp
            }
            None => 1.0,
        };

        let dt = t - prev_t;
        let im_mid = 0.5 * (prev_im + im_t);
        let df_mid = 0.5 * (prev_df + df_t);
        // Product of bucket midpoints, matching the joint-survival convention
        // used by the FVA leg in `xva::cva`.
        let sp_mid =
            0.5 * (prev_own_sp + own_sp_t) * 0.5 * (prev_counterparty_sp + counterparty_sp_t);
        let spread_mid = finstack_quant_core::math::interp::interp_knots_flat(
            funding_spread_curve,
            0.5 * (prev_t + t),
        ) / 10_000.0;

        mva += spread_mid * im_mid * df_mid * sp_mid * dt;
        im_time_integral += im_mid * dt;

        prev_t = t;
        prev_im = im_t;
        prev_df = df_t;
        prev_own_sp = own_sp_t;
        prev_counterparty_sp = counterparty_sp_t;
    }

    let horizon = im_profile.times[n - 1];
    Ok(MvaResult {
        mva,
        average_im: im_time_integral / horizon,
        im_profile: im_profile
            .times
            .iter()
            .copied()
            .zip(im_profile.im_values.iter().copied())
            .collect(),
    })
}

fn validate_spread_curve(curve: &[(f64, f64)]) -> finstack_quant_core::Result<()> {
    if curve.is_empty() {
        return Err(finstack_quant_core::Error::Validation(
            "MVA: funding_spread_curve must not be empty".into(),
        ));
    }
    for (i, &(t, s)) in curve.iter().enumerate() {
        if !t.is_finite() || t < 0.0 {
            return Err(finstack_quant_core::Error::Validation(format!(
                "MVA: funding_spread_curve[{i}] time {t} must be non-negative and finite"
            )));
        }
        if !s.is_finite() || s < 0.0 {
            return Err(finstack_quant_core::Error::Validation(format!(
                "MVA: funding_spread_curve[{i}] spread {s} bp must be non-negative and finite"
            )));
        }
        if i > 0 && t <= curve[i - 1].0 {
            return Err(finstack_quant_core::Error::Validation(format!(
                "MVA: funding_spread_curve times must be strictly increasing at index {i}"
            )));
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::calculators::im::simm::{SimmCalculator, SimmVersion};
    use crate::types::SimmSensitivities;
    use finstack_quant_core::currency::Currency;
    use finstack_quant_core::dates::Date;
    use finstack_quant_core::market_data::term_structures::{DiscountCurve, HazardCurve};
    use time::Month;

    fn flat_discount_curve(rate: f64) -> DiscountCurve {
        let base = Date::from_calendar_date(2025, Month::January, 1).expect("Valid date");
        let knots: Vec<(f64, f64)> = (0..=60)
            .map(|i| {
                let t = i as f64 * 0.5;
                (t, (-rate * t).exp())
            })
            .collect();
        DiscountCurve::builder("USD-OIS")
            .base_date(base)
            .knots(knots)
            .interp(finstack_quant_core::math::interp::InterpStyle::LogLinear)
            .build()
            .expect("DiscountCurve should build")
    }

    fn flat_hazard_curve(lambda: f64) -> HazardCurve {
        let base = Date::from_calendar_date(2025, Month::January, 1).expect("Valid date");
        HazardCurve::builder("BANK-SELF")
            .base_date(base)
            .knots([(0.0, lambda), (30.0, lambda)])
            .recovery_rate(0.40)
            .build()
            .expect("HazardCurve should build")
    }

    fn constant_im_profile(im: f64, times: &[f64]) -> ImProfile {
        ImProfile {
            times: times.to_vec(),
            im_values: vec![im; times.len()],
        }
    }

    #[test]
    fn decay_factors_match_definitions() {
        assert!((ImDecayProfile::Constant.factor(7.3) - 1.0).abs() < 1e-15);
        let lin = ImDecayProfile::LinearToMaturity {
            maturity_years: 2.0,
        };
        assert!((lin.factor(0.0) - 1.0).abs() < 1e-15);
        assert!((lin.factor(1.0) - 0.5).abs() < 1e-15);
        assert!(lin.factor(3.0).abs() < 1e-15); // clamped at 0 past maturity
        let sq = ImDecayProfile::SqrtTime {
            maturity_years: 2.0,
        };
        assert!((sq.factor(1.0) - 0.5f64.sqrt()).abs() < 1e-15);
        assert!(sq.factor(2.0).abs() < 1e-15);
    }

    #[test]
    fn decay_validate_rejects_bad_maturity() {
        assert!(ImDecayProfile::LinearToMaturity {
            maturity_years: 0.0
        }
        .validate()
        .is_err());
        assert!(ImDecayProfile::SqrtTime {
            maturity_years: f64::NAN
        }
        .validate()
        .is_err());
        assert!(ImDecayProfile::Constant.validate().is_ok());
    }

    #[test]
    fn im_profile_from_simm_scales_base_im_by_decay() {
        let calc = SimmCalculator::new(SimmVersion::V2_6).expect("calculator");
        let mut sens = SimmSensitivities::new(Currency::USD);
        sens.add_ir_delta(Currency::USD, "5Y", 50_000.0);
        let (base_im, _) = calc
            .calculate_from_sensitivities_parts(&sens, Currency::USD)
            .expect("valid sensitivity fixture");
        assert!(base_im > 0.0, "SIMM IM must be positive for a nonzero DV01");

        let decay = ImDecayProfile::LinearToMaturity {
            maturity_years: 4.0,
        };
        let grid = [1.0, 2.0, 4.0];
        let profile = im_profile_from_simm(&calc, &sens, Currency::USD, &decay, &grid)
            .expect("profile should build");

        assert_eq!(profile.times, grid.to_vec());
        assert!((profile.im_values[0] - base_im * 0.75).abs() < 1e-9);
        assert!((profile.im_values[1] - base_im * 0.50).abs() < 1e-9);
        assert!(profile.im_values[2].abs() < 1e-9);
    }

    #[test]
    fn im_profile_from_simm_rejects_bad_grid() {
        let calc = SimmCalculator::new(SimmVersion::V2_6).expect("calculator");
        let sens = SimmSensitivities::new(Currency::USD);
        let decay = ImDecayProfile::Constant;
        assert!(im_profile_from_simm(&calc, &sens, Currency::USD, &decay, &[]).is_err());
        assert!(im_profile_from_simm(&calc, &sens, Currency::USD, &decay, &[1.0, 0.5]).is_err());
        assert!(im_profile_from_simm(&calc, &sens, Currency::USD, &decay, &[0.0, 1.0]).is_err());
    }

    #[test]
    fn mva_flat_spread_zero_rates_no_survival() {
        // IM(t) = 1_000_000 constant, spread 50bp flat, DF = 1, S = 1,
        // grid [1, 2] with the flat-before-first-point convention:
        // bucket [0,1]: 0.0050 × 1e6 × 1 × 1 × 1 = 5_000
        // bucket [1,2]: 0.0050 × 1e6 × 1 × 1 × 1 = 5_000
        // MVA = 10_000 exactly; average_im = 1e6.
        let profile = constant_im_profile(1_000_000.0, &[1.0, 2.0]);
        let discount = flat_discount_curve(0.0);
        let result =
            compute_mva(&profile, &[(0.0, 50.0)], &discount, None).expect("MVA should compute");
        assert!(
            (result.mva - 10_000.0).abs() < 1e-6,
            "MVA {} != 10_000",
            result.mva
        );
        assert!((result.average_im - 1_000_000.0).abs() < 1e-6);
        assert_eq!(
            result.im_profile,
            vec![(1.0, 1_000_000.0), (2.0, 1_000_000.0)]
        );
    }

    #[test]
    fn mva_weights_by_bank_survival() {
        // Same as above with own-survival S(t) = exp(−0.02 t):
        // S(1) = 0.980198673, S(2) = 0.960789439
        // bucket1 S_mid = (1 + 0.980198673)/2 = 0.990099337
        // bucket2 S_mid = (0.980198673 + 0.960789439)/2 = 0.970494056
        // MVA = 5_000 × (0.990099337 + 0.970494056) = 9_802.96696…
        let profile = constant_im_profile(1_000_000.0, &[1.0, 2.0]);
        let discount = flat_discount_curve(0.0);
        let survival = flat_hazard_curve(0.02);
        let result = compute_mva(&profile, &[(0.0, 50.0)], &discount, Some(&survival))
            .expect("MVA should compute");
        let expected = 5_000.0
            * (((1.0 + (-0.02f64).exp()) / 2.0) + (((-0.02f64).exp() + (-0.04f64).exp()) / 2.0));
        assert!(
            (result.mva - expected).abs() / expected < 1e-9,
            "MVA {} != {expected}",
            result.mva
        );
        assert!(result.mva < 10_000.0);
    }

    #[test]
    fn mva_linear_decay_profile() {
        // LinearToMaturity T=2 sampled on [1, 2]: IM = [500_000, 0].
        // Flat-before-first-point convention ⇒ bucket [0,1] uses IM = 5e5:
        // bucket1: 0.01 × 5e5 × 1 = 5_000
        // bucket2: 0.01 × (5e5 + 0)/2 × 1 = 2_500
        // MVA = 7_500; average_im = (5e5·1 + 2.5e5·1)/2 = 375_000.
        let profile = ImProfile {
            times: vec![1.0, 2.0],
            im_values: vec![500_000.0, 0.0],
        };
        let discount = flat_discount_curve(0.0);
        let result =
            compute_mva(&profile, &[(0.0, 100.0)], &discount, None).expect("MVA should compute");
        assert!((result.mva - 7_500.0).abs() < 1e-6, "MVA {}", result.mva);
        assert!((result.average_im - 375_000.0).abs() < 1e-6);
    }

    #[test]
    fn mva_interpolates_spread_curve() {
        // Spread curve [(0, 50bp), (2, 150bp)], constant IM 1e6, DF=1, S=1,
        // grid [1, 2]. Bucket midpoints: t=0.5 → 75bp; t=1.5 → 125bp.
        // MVA = 0.0075×1e6 + 0.0125×1e6 = 7_500 + 12_500 = 20_000.
        let profile = constant_im_profile(1_000_000.0, &[1.0, 2.0]);
        let discount = flat_discount_curve(0.0);
        let result = compute_mva(&profile, &[(0.0, 50.0), (2.0, 150.0)], &discount, None)
            .expect("MVA should compute");
        assert!((result.mva - 20_000.0).abs() < 1e-6, "MVA {}", result.mva);
    }

    #[test]
    fn mva_discounting_reduces_value() {
        let profile = constant_im_profile(1_000_000.0, &[1.0, 2.0]);
        let flat = flat_discount_curve(0.0);
        let discounted = flat_discount_curve(0.03);
        let mva_flat = compute_mva(&profile, &[(0.0, 50.0)], &flat, None)
            .expect("ok")
            .mva;
        let mva_disc = compute_mva(&profile, &[(0.0, 50.0)], &discounted, None)
            .expect("ok")
            .mva;
        assert!(mva_disc < mva_flat);
        assert!(mva_disc > 0.0);
    }

    #[test]
    fn mva_rejects_invalid_inputs() {
        let discount = flat_discount_curve(0.0);
        // Empty profile
        let empty = ImProfile {
            times: vec![],
            im_values: vec![],
        };
        assert!(compute_mva(&empty, &[(0.0, 50.0)], &discount, None).is_err());
        // Length mismatch
        let bad = ImProfile {
            times: vec![1.0],
            im_values: vec![1.0, 2.0],
        };
        assert!(compute_mva(&bad, &[(0.0, 50.0)], &discount, None).is_err());
        // Negative IM
        let neg = ImProfile {
            times: vec![1.0],
            im_values: vec![-5.0],
        };
        assert!(compute_mva(&neg, &[(0.0, 50.0)], &discount, None).is_err());
        // Empty / negative spread curve
        let ok = constant_im_profile(1.0, &[1.0]);
        assert!(compute_mva(&ok, &[], &discount, None).is_err());
        assert!(compute_mva(&ok, &[(0.0, -1.0)], &discount, None).is_err());
        assert!(compute_mva(&ok, &[(1.0, 10.0), (0.5, 10.0)], &discount, None).is_err());
    }

    #[test]
    fn mva_serde_round_trip_and_strictness() {
        let decay = ImDecayProfile::LinearToMaturity {
            maturity_years: 2.0,
        };
        let json = serde_json::to_string(&decay).expect("serialize");
        let back: ImDecayProfile = serde_json::from_str(&json).expect("deserialize");
        assert_eq!(back, decay);

        // Unknown fields must be rejected on all new inbound types.
        assert!(serde_json::from_str::<ImProfile>(
            r#"{"times":[1.0],"im_values":[2.0],"surprise":1}"#
        )
        .is_err());
        assert!(serde_json::from_str::<MvaResult>(
            r#"{"mva":1.0,"average_im":1.0,"im_profile":[[1.0,1.0]],"surprise":1}"#
        )
        .is_err());
        assert!(serde_json::from_str::<ImDecayProfile>(
            r#"{"linear_to_maturity":{"maturity_years":2.0,"surprise":1}}"#
        )
        .is_err());
    }
}
