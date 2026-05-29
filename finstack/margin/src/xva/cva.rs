//! Credit and Funding Valuation Adjustments (CVA, DVA, FVA).
//!
//! Implements unilateral CVA, DVA (debit valuation adjustment), FVA (funding
//! valuation adjustment), and bilateral XVA for OTC derivative portfolios.
//!
//! # Mathematical Framework
//!
//! **Unilateral CVA** — expected loss from counterparty default:
//!
//! ```text
//! CVA = (1 - R) × Σᵢ EPE_mid(tᵢ) × [S(tᵢ₋₁) - S(tᵢ)] × DF_mid(tᵢ)
//! ```
//!
//! **DVA** — expected gain from own default (mirror of CVA using ENE):
//!
//! ```text
//! DVA = (1 - R_own) × Σᵢ ENE_mid(tᵢ) × [S_own(tᵢ₋₁) - S_own(tᵢ)] × DF_mid(tᵢ)
//! ```
//!
//! **FVA** — funding cost/benefit on uncollateralized exposure:
//!
//! ```text
//! FVA = Σᵢ [EPE_mid(tᵢ) × s_f⁺ - ENE_mid(tᵢ) × s_f⁻] × DF_mid(tᵢ) × Δtᵢ
//! ```
//!
//! **Bilateral XVA** = CVA - DVA + FVA
//!
//! where `EPE_mid` and `DF_mid` are averaged over consecutive time points
//! for O(Δt²) convergence.
//!
//! Notation:
//! - `R`, `R_own` = recovery rates (counterparty, own)
//! - `EPE(t)` = expected positive exposure at time `t`
//! - `ENE(t)` = expected negative exposure at time `t`
//! - `S(t)`, `S_own(t)` = survival probabilities (counterparty, own)
//! - `DF(t)` = risk-free discount factor at time `t`
//! - `s_f⁺`, `s_f⁻` = funding spread (cost) and funding benefit spread
//!
//! # References
//!
//! - Gregory XVA Challenge: `docs/REFERENCES.md#gregory-xva-challenge`
//! - Green XVA: `docs/REFERENCES.md#green-xva`
//! - BCBS 279 SA-CCR: `docs/REFERENCES.md#bcbs-279-saccr`

use finstack_core::market_data::term_structures::{DiscountCurve, HazardCurve};

use super::types::{ExposureProfile, FundingConfig, XvaResult};

fn validate_exposure_profile_lengths(
    exposure_profile: &ExposureProfile,
    label: &str,
) -> finstack_core::Result<usize> {
    if exposure_profile.times.is_empty() {
        return Err(finstack_core::Error::Validation(format!(
            "{label}: exposure profile must not be empty"
        )));
    }

    let n = exposure_profile.times.len();
    if exposure_profile.epe.len() != n || exposure_profile.ene.len() != n {
        return Err(finstack_core::Error::Validation(format!(
            "{label}: exposure profile vector lengths must be equal \
             (times={n}, epe={}, ene={})",
            exposure_profile.epe.len(),
            exposure_profile.ene.len()
        )));
    }

    Ok(n)
}

fn compute_cva_internal(
    exposure_profile: &ExposureProfile,
    counterparty_hazard_curve: &HazardCurve,
    discount_curve: &DiscountCurve,
    recovery_rate: f64,
    own_survival_curve: Option<&HazardCurve>,
) -> finstack_core::Result<XvaResult> {
    let n = validate_exposure_profile_lengths(exposure_profile, "CVA")?;

    if !(0.0..=1.0).contains(&recovery_rate) {
        return Err(finstack_core::Error::Validation(format!(
            "CVA: recovery_rate {recovery_rate} must be in [0, 1]"
        )));
    }

    let lgd = 1.0 - recovery_rate;

    let mut cva = 0.0;
    let mut epe_profile = Vec::with_capacity(n);
    let mut ene_profile = Vec::with_capacity(n);
    let mut pfe_profile = Vec::with_capacity(n);
    let mut effective_epe_profile = Vec::with_capacity(n);
    let mut max_pfe: f64 = 0.0;
    let mut effective_epe_running: f64 = 0.0;
    let mut eff_epe_time_integral: f64 = 0.0;
    let maturity = exposure_profile.times[n - 1];
    let effective_epe_horizon = maturity.min(1.0);

    let mut prev_survival = 1.0;
    let mut prev_own_survival = 1.0;
    let mut prev_epe: f64 = 0.0;
    let mut prev_df: f64 = 1.0;
    let mut prev_t: f64 = 0.0;

    for i in 0..n {
        let t = exposure_profile.times[i];
        let epe_t = exposure_profile.epe[i];
        let ene_t = exposure_profile.ene[i];

        let survival_t = counterparty_hazard_curve.sp(t);
        if !survival_t.is_finite() {
            return Err(finstack_core::Error::Validation(format!(
                "CVA: non-finite survival probability at t={t}: S(t)={survival_t}"
            )));
        }

        let own_survival_t = if let Some(curve) = own_survival_curve {
            let sp = curve.sp(t);
            if !sp.is_finite() {
                return Err(finstack_core::Error::Validation(format!(
                    "CVA: non-finite own survival probability at t={t}: S_own(t)={sp}"
                )));
            }
            sp
        } else {
            1.0
        };

        let df_t = discount_curve.df(t);
        if !df_t.is_finite() {
            return Err(finstack_core::Error::Validation(format!(
                "CVA: non-finite discount factor at t={t}: DF(t)={df_t}"
            )));
        }

        let marginal_pd = (prev_survival - survival_t).max(0.0);
        let epe_mid = 0.5 * (prev_epe + epe_t);
        let df_mid = 0.5 * (prev_df + df_t);
        let own_survival_mid = 0.5 * (prev_own_survival + own_survival_t);

        cva += lgd * epe_mid * marginal_pd * df_mid * own_survival_mid;

        effective_epe_running = effective_epe_running.max(epe_t);

        if prev_t < effective_epe_horizon {
            let dt = (t.min(effective_epe_horizon) - prev_t).max(0.0);
            eff_epe_time_integral += effective_epe_running * dt;
        }

        // Deterministic single-scenario engine: the exposure
        // distribution is a point mass at max(V(t), 0), so PFE at any
        // quantile equals EPE. See doc on `XvaResult::pfe_profile`.
        let pfe_t = epe_t;
        max_pfe = max_pfe.max(pfe_t);

        epe_profile.push((t, epe_t));
        ene_profile.push((t, ene_t));
        pfe_profile.push((t, pfe_t));
        effective_epe_profile.push((t, effective_epe_running));

        prev_survival = survival_t;
        prev_own_survival = own_survival_t;
        prev_epe = epe_t;
        prev_df = df_t;
        prev_t = t;
    }

    let normalization = effective_epe_horizon;
    let effective_epe = if normalization > 0.0 {
        eff_epe_time_integral / normalization
    } else {
        effective_epe_running
    };

    Ok(XvaResult {
        cva,
        dva: None,
        fva: None,
        bilateral_cva: None,
        epe_profile,
        ene_profile,
        pfe_profile,
        max_pfe,
        effective_epe_profile,
        effective_epe,
    })
}

fn compute_dva_internal(
    exposure_profile: &ExposureProfile,
    own_hazard_curve: &HazardCurve,
    discount_curve: &DiscountCurve,
    own_recovery_rate: f64,
    counterparty_survival_curve: Option<&HazardCurve>,
) -> finstack_core::Result<f64> {
    let n = validate_exposure_profile_lengths(exposure_profile, "DVA")?;

    if !(0.0..=1.0).contains(&own_recovery_rate) {
        return Err(finstack_core::Error::Validation(format!(
            "DVA: own_recovery_rate {own_recovery_rate} must be in [0, 1]"
        )));
    }

    let lgd_own = 1.0 - own_recovery_rate;
    let mut dva = 0.0;
    let mut prev_survival = 1.0;
    let mut prev_counterparty_survival = 1.0;
    let mut prev_ene: f64 = 0.0;
    let mut prev_df: f64 = 1.0;

    for i in 0..n {
        let t = exposure_profile.times[i];
        let ene_t = exposure_profile.ene[i];

        let survival_t = own_hazard_curve.sp(t);
        if !survival_t.is_finite() {
            return Err(finstack_core::Error::Validation(format!(
                "DVA: non-finite survival probability at t={t}: S_own(t)={survival_t}"
            )));
        }

        let counterparty_survival_t = if let Some(curve) = counterparty_survival_curve {
            let sp = curve.sp(t);
            if !sp.is_finite() {
                return Err(finstack_core::Error::Validation(format!(
                    "DVA: non-finite counterparty survival probability at t={t}: S_c(t)={sp}"
                )));
            }
            sp
        } else {
            1.0
        };

        let df_t = discount_curve.df(t);
        if !df_t.is_finite() {
            return Err(finstack_core::Error::Validation(format!(
                "DVA: non-finite discount factor at t={t}: DF(t)={df_t}"
            )));
        }

        let marginal_pd = (prev_survival - survival_t).max(0.0);
        let ene_mid = 0.5 * (prev_ene + ene_t);
        let df_mid = 0.5 * (prev_df + df_t);
        let counterparty_survival_mid =
            0.5 * (prev_counterparty_survival + counterparty_survival_t);

        dva += lgd_own * ene_mid * marginal_pd * df_mid * counterparty_survival_mid;

        prev_survival = survival_t;
        prev_counterparty_survival = counterparty_survival_t;
        prev_ene = ene_t;
        prev_df = df_t;
    }

    Ok(dva)
}

fn compute_fva_internal(
    exposure_profile: &ExposureProfile,
    discount_curve: &DiscountCurve,
    funding_spread_bps: f64,
    funding_benefit_bps: f64,
    counterparty_hazard_curve: Option<&HazardCurve>,
    own_hazard_curve: Option<&HazardCurve>,
) -> finstack_core::Result<f64> {
    let n = validate_exposure_profile_lengths(exposure_profile, "FVA")?;

    let spread_cost = funding_spread_bps / 10_000.0;
    let spread_benefit = funding_benefit_bps / 10_000.0;

    let mut fva = 0.0;
    let mut prev_counterparty_survival = 1.0;
    let mut prev_own_survival = 1.0;
    let mut prev_epe: f64 = 0.0;
    let mut prev_ene: f64 = 0.0;
    let mut prev_df: f64 = 1.0;
    let mut prev_t: f64 = 0.0;

    for i in 0..n {
        let t = exposure_profile.times[i];
        let epe_t = exposure_profile.epe[i];
        let ene_t = exposure_profile.ene[i];

        let counterparty_survival_t = if let Some(curve) = counterparty_hazard_curve {
            let sp = curve.sp(t);
            if !sp.is_finite() {
                return Err(finstack_core::Error::Validation(format!(
                    "FVA: non-finite counterparty survival probability at t={t}: S_c(t)={sp}"
                )));
            }
            sp
        } else {
            1.0
        };

        let own_survival_t = if let Some(curve) = own_hazard_curve {
            let sp = curve.sp(t);
            if !sp.is_finite() {
                return Err(finstack_core::Error::Validation(format!(
                    "FVA: non-finite own survival probability at t={t}: S_own(t)={sp}"
                )));
            }
            sp
        } else {
            1.0
        };

        let df_t = discount_curve.df(t);
        if !df_t.is_finite() {
            return Err(finstack_core::Error::Validation(format!(
                "FVA: non-finite discount factor at t={t}: DF(t)={df_t}"
            )));
        }

        let dt = t - prev_t;
        let epe_mid = 0.5 * (prev_epe + epe_t);
        let ene_mid = 0.5 * (prev_ene + ene_t);
        let df_mid = 0.5 * (prev_df + df_t);
        let counterparty_survival_mid =
            0.5 * (prev_counterparty_survival + counterparty_survival_t);
        let own_survival_mid = 0.5 * (prev_own_survival + own_survival_t);
        let joint_survival_mid = counterparty_survival_mid * own_survival_mid;

        fva +=
            (epe_mid * spread_cost - ene_mid * spread_benefit) * df_mid * dt * joint_survival_mid;

        prev_counterparty_survival = counterparty_survival_t;
        prev_own_survival = own_survival_t;
        prev_epe = epe_t;
        prev_ene = ene_t;
        prev_df = df_t;
        prev_t = t;
    }

    Ok(fva)
}

/// Compute **unilateral** Credit Valuation Adjustment (CVA).
///
/// > **Important — semantics:** this is the *unilateral* form. It assumes
/// > the institution itself does not default, so it does not weight the
/// > expected loss by the institution's own survival probability. For
/// > the bilateral first-to-default form (joint survival weighting), call
/// > [`compute_bilateral_xva`] instead. Mixing unilateral CVA with
/// > unilateral DVA double-counts default risk.
///
/// Uses midpoint/trapezoidal numerical integration for O(Δt²) accuracy:
///
/// ```text
/// CVA = (1 - R) × Σᵢ EPE_mid(tᵢ) × [S(tᵢ₋₁) - S(tᵢ)] × DF_mid(tᵢ)
/// ```
///
/// where `EPE_mid` and `DF_mid` are the averages of consecutive time points.
///
/// For each time bucket `[tᵢ₋₁, tᵢ]`:
/// 1. Compute `EPE_mid = (EPE(tᵢ₋₁) + EPE(tᵢ)) / 2` (trapezoidal)
/// 2. Compute marginal default probability: `PD_i = S(tᵢ₋₁) - S(tᵢ)`
/// 3. Compute `DF_mid = (DF(tᵢ₋₁) + DF(tᵢ)) / 2` (trapezoidal)
/// 4. Accumulate: `(1-R) × EPE_mid × PD_i × DF_mid`
///
/// # Arguments
///
/// * `exposure_profile` - EPE/ENE profile from exposure simulation
/// * `counterparty_hazard_curve` - Hazard curve for the counterparty's credit
/// * `discount_curve` - Risk-free discount curve for present-valuing losses
/// * `recovery_rate` - Assumed recovery rate upon default (0 to 1)
///
/// # Returns
///
/// An [`XvaResult`] containing the CVA value and exposure metrics.
///
/// # Errors
///
/// Returns an error if:
/// - The exposure profile is empty
/// - Exposure profile vectors have inconsistent lengths
/// - Recovery rate is not in `[0, 1]`
/// - Curve evaluations return non-finite values (NaN/infinity)
///
/// # Examples
///
/// ```ignore
/// use crate::xva::cva::compute_cva;
/// use crate::xva::types::ExposureProfile;
/// use finstack_core::market_data::term_structures::{DiscountCurve, HazardCurve};
///
/// # fn example() -> finstack_core::Result<()> {
/// // ... construct profile, hazard_curve, discount_curve ...
/// # let profile = ExposureProfile { times: vec![], mtm_values: vec![], epe: vec![], ene: vec![], diagnostics: None };
/// # let hazard_curve = todo!();
/// # let discount_curve = todo!();
/// let result = compute_cva(&profile, &hazard_curve, &discount_curve, 0.40)?;
/// println!("CVA = {:.2}", result.cva);
/// # Ok(())
/// # }
/// ```
pub fn compute_cva(
    exposure_profile: &ExposureProfile,
    counterparty_hazard_curve: &HazardCurve,
    discount_curve: &DiscountCurve,
    recovery_rate: f64,
) -> finstack_core::Result<XvaResult> {
    compute_cva_internal(
        exposure_profile,
        counterparty_hazard_curve,
        discount_curve,
        recovery_rate,
        None,
    )
}

/// Compute **unilateral** Debit Valuation Adjustment (DVA).
///
/// > **Important — semantics:** this is the *unilateral* form. It assumes
/// > the counterparty does not default, so it does not weight the
/// > expected gain by the counterparty's survival probability. For
/// > the bilateral first-to-default form, call [`compute_bilateral_xva`]
/// > instead.
///
/// DVA is the mirror image of CVA: it captures the expected gain to the
/// institution from its own default on positions where the counterparty
/// has positive exposure (i.e., the institution owes money).
///
/// Uses midpoint/trapezoidal numerical integration for O(Δt²) accuracy:
///
/// ```text
/// DVA = (1 - R_own) × Σᵢ ENE_mid(tᵢ) × [S_own(tᵢ₋₁) - S_own(tᵢ)] × DF_mid(tᵢ)
/// ```
///
/// # Arguments
///
/// * `exposure_profile` - EPE/ENE profile from exposure simulation
/// * `own_hazard_curve` - Hazard curve for the institution's own credit
/// * `discount_curve` - Risk-free discount curve for present-valuing
/// * `own_recovery_rate` - Assumed recovery rate on own default (0 to 1)
///
/// # Returns
///
/// The DVA value as a scalar (positive = benefit to the desk).
///
/// # Errors
///
/// Returns an error if:
/// - The exposure profile is empty
/// - Exposure profile vectors have inconsistent lengths
/// - Recovery rate is not in `[0, 1]`
/// - Curve evaluations return non-finite values
///
/// # References
///
/// - Gregory, J. (2020). *The xVA Challenge*, 4th ed. Wiley. Chapter 17.
/// - BCBS (2011). "Application of own credit risk adjustments to derivatives."
pub fn compute_dva(
    exposure_profile: &ExposureProfile,
    own_hazard_curve: &HazardCurve,
    discount_curve: &DiscountCurve,
    own_recovery_rate: f64,
) -> finstack_core::Result<f64> {
    compute_dva_internal(
        exposure_profile,
        own_hazard_curve,
        discount_curve,
        own_recovery_rate,
        None,
    )
}

/// Compute Funding Valuation Adjustment (FVA).
///
/// FVA captures the cost (or benefit) of funding uncollateralized derivative
/// positions. Positive exposure requires the institution to borrow at a
/// spread above the risk-free rate (funding cost), while negative exposure
/// allows the institution to invest at that spread (funding benefit).
///
/// Uses midpoint/trapezoidal numerical integration:
///
/// ```text
/// FVA = Σᵢ [EPE_mid(tᵢ) × s_f⁺ - ENE_mid(tᵢ) × s_f⁻] × DF_mid(tᵢ) × Δtᵢ
/// ```
///
/// where `s_f⁺` is the funding cost spread and `s_f⁻` is the funding benefit
/// spread, both expressed as decimal fractions (not basis points).
///
/// # Arguments
///
/// * `exposure_profile` - EPE/ENE profile from exposure simulation
/// * `discount_curve` - Risk-free discount curve for present-valuing
/// * `funding_spread_bps` - Funding cost spread in basis points (applied to EPE)
/// * `funding_benefit_bps` - Funding benefit spread in basis points (applied to ENE)
///
/// # Returns
///
/// The FVA value as a scalar. Positive = net funding cost; negative = net benefit.
///
/// # Errors
///
/// Returns an error if:
/// - The exposure profile is empty
/// - Exposure profile vectors have inconsistent lengths
/// - Curve evaluations return non-finite values
///
/// # References
///
/// - Gregory, J. (2020). *The xVA Challenge*, 4th ed. Wiley. Chapter 19.
/// - Green, A. (2015). *XVA: Credit, Funding and Capital Valuation Adjustments*.
///   Wiley. Chapter 5.
pub fn compute_fva(
    exposure_profile: &ExposureProfile,
    discount_curve: &DiscountCurve,
    funding_spread_bps: f64,
    funding_benefit_bps: f64,
) -> finstack_core::Result<f64> {
    compute_fva_internal(
        exposure_profile,
        discount_curve,
        funding_spread_bps,
        funding_benefit_bps,
        None,
        None,
    )
}

/// Compute bilateral XVA: CVA, DVA, FVA, and the combined bilateral adjustment.
///
/// Accounts for:
/// - Counterparty default risk (CVA)
/// - Own-default benefit (DVA)
/// - Funding costs and benefits (FVA, if funding config provided)
///
/// The bilateral adjustment is: **Bilateral XVA = CVA - DVA + FVA**
///
/// # Arguments
///
/// * `exposure_profile` - EPE/ENE profile from exposure simulation
/// * `counterparty_hazard_curve` - Hazard curve for the counterparty's credit
/// * `own_hazard_curve` - Hazard curve for the institution's own credit
/// * `discount_curve` - Risk-free discount curve for present-valuing
/// * `counterparty_recovery_rate` - Recovery rate for counterparty default (0 to 1)
/// * `own_recovery_rate` - Recovery rate for own default (0 to 1)
/// * `funding` - Optional funding configuration for FVA
///
/// # Returns
///
/// An [`XvaResult`] containing CVA, DVA, FVA, bilateral CVA, and exposure metrics.
///
/// # Errors
///
/// Returns an error if any sub-calculation (CVA, DVA, FVA) fails.
///
/// # References
///
/// - Gregory, J. (2020). *The xVA Challenge*, 4th ed. Wiley. Chapters 14, 17, 19.
/// - Brigo, D., Morini, M. & Pallavicini, A. (2013). *Counterparty Credit Risk,
///   Collateral and Funding*. Wiley.
pub fn compute_bilateral_xva(
    exposure_profile: &ExposureProfile,
    counterparty_hazard_curve: &HazardCurve,
    own_hazard_curve: &HazardCurve,
    discount_curve: &DiscountCurve,
    counterparty_recovery_rate: f64,
    own_recovery_rate: f64,
    funding: Option<&FundingConfig>,
) -> finstack_core::Result<XvaResult> {
    let mut result = compute_cva_internal(
        exposure_profile,
        counterparty_hazard_curve,
        discount_curve,
        counterparty_recovery_rate,
        Some(own_hazard_curve),
    )?;

    let dva = compute_dva_internal(
        exposure_profile,
        own_hazard_curve,
        discount_curve,
        own_recovery_rate,
        Some(counterparty_hazard_curve),
    )?;
    result.dva = Some(dva);

    let fva = if let Some(fc) = funding {
        let fva_val = compute_fva_internal(
            exposure_profile,
            discount_curve,
            fc.funding_spread_bps,
            fc.effective_benefit_bps(),
            Some(counterparty_hazard_curve),
            Some(own_hazard_curve),
        )?;
        result.fva = Some(fva_val);
        fva_val
    } else {
        result.fva = None;
        0.0
    };

    // Bilateral CVA = CVA - DVA + FVA
    result.bilateral_cva = Some(result.cva - dva + fva);

    Ok(result)
}

#[cfg(test)]
mod tests {
    use super::*;
    use finstack_core::dates::Date;
    use time::Month;

    /// Helper: build a flat hazard rate curve.
    fn flat_hazard_curve(lambda: f64) -> HazardCurve {
        let base = Date::from_calendar_date(2025, Month::January, 1).expect("Valid date");
        HazardCurve::builder("COUNTERPARTY")
            .base_date(base)
            .knots([(0.0, lambda), (30.0, lambda)])
            .build()
            .expect("HazardCurve should build")
    }

    /// Helper: build a flat discount curve.
    fn flat_discount_curve(rate: f64) -> DiscountCurve {
        let base = Date::from_calendar_date(2025, Month::January, 1).expect("Valid date");
        // Build knot points with DF = exp(-r * t)
        let knots: Vec<(f64, f64)> = (0..=60)
            .map(|i| {
                let t = i as f64 * 0.5;
                (t, (-rate * t).exp())
            })
            .collect();
        DiscountCurve::builder("USD-OIS")
            .base_date(base)
            .knots(knots)
            .interp(finstack_core::math::interp::InterpStyle::LogLinear)
            .build()
            .expect("DiscountCurve should build")
    }

    /// Helper: build a uniform EPE profile.
    fn uniform_profile(epe_value: f64, times: &[f64]) -> ExposureProfile {
        ExposureProfile {
            times: times.to_vec(),
            mtm_values: times.iter().map(|_| epe_value).collect(),
            epe: times.iter().map(|_| epe_value).collect(),
            ene: times.iter().map(|_| 0.0).collect(),
            diagnostics: None,
        }
    }

    // ── CVA formula tests ────────────────────────────────────────

    #[test]
    fn cva_zero_hazard_rate_gives_zero_cva() {
        // With λ=0, survival is always 1.0, so marginal PD = 0 at every step
        // → CVA = 0
        let hazard = flat_hazard_curve(0.0);
        let discount = flat_discount_curve(0.05);
        let times: Vec<f64> = (1..=20).map(|i| i as f64 * 0.5).collect();
        let profile = uniform_profile(1_000_000.0, &times);

        let result =
            compute_cva(&profile, &hazard, &discount, 0.40).expect("CVA computation should work");

        assert!(
            result.cva.abs() < 1e-6,
            "CVA with zero hazard rate should be zero, got {}",
            result.cva
        );
    }

    #[test]
    fn cva_full_recovery_gives_zero_cva() {
        // With R=1.0, LGD = 0 → CVA = 0 regardless of default probability
        let hazard = flat_hazard_curve(0.02);
        let discount = flat_discount_curve(0.05);
        let times: Vec<f64> = (1..=20).map(|i| i as f64 * 0.5).collect();
        let profile = uniform_profile(1_000_000.0, &times);

        let result = compute_cva(&profile, &hazard, &discount, 1.0)
            .expect("CVA computation should work with R=1");

        assert!(
            result.cva.abs() < 1e-12,
            "CVA with 100% recovery should be zero, got {}",
            result.cva
        );
    }

    #[test]
    fn cva_zero_exposure_gives_zero_cva() {
        // With EPE = 0, CVA = 0 regardless of credit quality
        let hazard = flat_hazard_curve(0.05);
        let discount = flat_discount_curve(0.05);
        let times: Vec<f64> = (1..=20).map(|i| i as f64 * 0.5).collect();
        let profile = uniform_profile(0.0, &times);

        let result =
            compute_cva(&profile, &hazard, &discount, 0.40).expect("CVA computation should work");

        assert!(
            result.cva.abs() < 1e-12,
            "CVA with zero exposure should be zero, got {}",
            result.cva
        );
    }

    #[test]
    fn cva_positive_for_nonzero_exposure_and_default_risk() {
        // Non-trivial case: positive exposure, positive hazard rate
        let hazard = flat_hazard_curve(0.02); // ~2% annual default intensity
        let discount = flat_discount_curve(0.03);
        let times: Vec<f64> = (1..=20).map(|i| i as f64 * 0.5).collect();
        let profile = uniform_profile(1_000_000.0, &times);

        let result =
            compute_cva(&profile, &hazard, &discount, 0.40).expect("CVA computation should work");

        assert!(
            result.cva > 0.0,
            "CVA should be positive for non-zero exposure and hazard rate, got {}",
            result.cva
        );

        // Sanity check: CVA should be less than (1-R) * EPE * total default probability
        // Upper bound: (1-R) × EPE × (1 - S(T))
        let total_dp = 1.0 - hazard.sp(10.0); // last time = 10.0
        let upper_bound = 0.60 * 1_000_000.0 * total_dp;
        assert!(
            result.cva < upper_bound,
            "CVA {} should be less than upper bound {} = (1-R) × EPE × total_PD",
            result.cva,
            upper_bound
        );
    }

    #[test]
    fn cva_with_flat_hazard_analytical_check() {
        // For constant EPE, flat hazard rate λ, flat discount rate r:
        //
        // CVA ≈ (1-R) × EPE × Σᵢ [S(tᵢ₋₁) - S(tᵢ)] × DF(tᵢ)
        //
        // For fine grid, this converges to:
        // CVA ≈ (1-R) × EPE × ∫₀ᵀ λ×e^{-λt} × e^{-rt} dt
        //     = (1-R) × EPE × λ/(λ+r) × (1 - e^{-(λ+r)T})
        let lambda = 0.02;
        let r = 0.03;
        let recovery = 0.40;
        let epe = 1_000_000.0;
        let t_max = 10.0;

        // Fine grid for accurate numerical integration
        let dt: f64 = 0.25;
        let n_steps = (t_max / dt).round() as usize;
        let times: Vec<f64> = (1..=n_steps).map(|i| i as f64 * dt).collect();

        let hazard = flat_hazard_curve(lambda);
        let discount = flat_discount_curve(r);
        let profile = uniform_profile(epe, &times);

        let result = compute_cva(&profile, &hazard, &discount, recovery)
            .expect("CVA computation should work");

        // Analytical formula
        let lgd = 1.0 - recovery;
        let analytical = lgd * epe * lambda / (lambda + r) * (1.0 - (-(lambda + r) * t_max).exp());

        // Midpoint/trapezoidal integration gives O(Δt²) convergence.
        // With dt=0.25 and EPE starting from 0 at t=0, the first bucket
        // averages (0 + EPE)/2, introducing a small bias. Expect < 2% error.
        let rel_error = (result.cva - analytical).abs() / analytical;
        assert!(
            rel_error < 0.02,
            "CVA numerical ({:.2}) should be close to analytical ({:.2}), rel_error={:.6}",
            result.cva,
            analytical,
            rel_error
        );
    }

    #[test]
    fn cva_profile_lengths_match() {
        let hazard = flat_hazard_curve(0.02);
        let discount = flat_discount_curve(0.03);
        let times: Vec<f64> = (1..=10).map(|i| i as f64).collect();
        let profile = uniform_profile(100.0, &times);

        let result =
            compute_cva(&profile, &hazard, &discount, 0.40).expect("CVA computation should work");

        assert_eq!(result.epe_profile.len(), times.len());
        assert_eq!(result.ene_profile.len(), times.len());
        assert_eq!(result.pfe_profile.len(), times.len());
    }

    #[test]
    fn effective_epe_profile_is_non_decreasing() {
        // Effective EPE profile should be the running maximum of EPE
        let hazard = flat_hazard_curve(0.02);
        let discount = flat_discount_curve(0.03);
        let times = vec![1.0, 2.0, 3.0, 4.0, 5.0];
        let profile = ExposureProfile {
            times: times.clone(),
            mtm_values: vec![100.0, 200.0, 50.0, 150.0, 80.0],
            epe: vec![100.0, 200.0, 50.0, 150.0, 80.0],
            ene: vec![0.0; 5],
            diagnostics: None,
        };

        let result =
            compute_cva(&profile, &hazard, &discount, 0.40).expect("CVA computation should work");

        // Profile should be non-decreasing
        assert_eq!(result.effective_epe_profile.len(), 5);
        for i in 1..result.effective_epe_profile.len() {
            assert!(
                result.effective_epe_profile[i].1 >= result.effective_epe_profile[i - 1].1,
                "Effective EPE profile must be non-decreasing at index {i}"
            );
        }

        // Peak of effective EPE profile should be 200
        let peak = result
            .effective_epe_profile
            .iter()
            .map(|&(_, v)| v)
            .fold(0.0_f64, f64::max);
        assert!(
            (peak - 200.0).abs() < 1e-12,
            "Peak effective EPE should be 200.0, got {peak}"
        );

        // Effective EPE scalar is time-weighted average:
        // integral / min(1, M) where M=5Y
        // Since we integrate over full 5Y but normalize by 1Y,
        // the scalar can exceed peak for multi-year portfolios.
        assert!(
            result.effective_epe > 0.0,
            "Time-weighted effective EPE should be positive"
        );
    }

    #[test]
    fn max_pfe_equals_max_epe_in_deterministic_model() {
        let hazard = flat_hazard_curve(0.02);
        let discount = flat_discount_curve(0.03);
        let times = vec![1.0, 2.0, 3.0];
        let profile = ExposureProfile {
            times,
            mtm_values: vec![100.0, 300.0, 200.0],
            epe: vec![100.0, 300.0, 200.0],
            ene: vec![0.0; 3],
            diagnostics: None,
        };

        let result =
            compute_cva(&profile, &hazard, &discount, 0.40).expect("CVA computation should work");

        assert!(
            (result.max_pfe - 300.0).abs() < 1e-12,
            "Max PFE should equal max EPE in deterministic model, got {}",
            result.max_pfe
        );
    }

    #[test]
    fn cva_rejects_empty_profile() {
        let hazard = flat_hazard_curve(0.02);
        let discount = flat_discount_curve(0.03);
        let profile = ExposureProfile {
            times: vec![],
            mtm_values: vec![],
            epe: vec![],
            ene: vec![],
            diagnostics: None,
        };

        let result = compute_cva(&profile, &hazard, &discount, 0.40);
        assert!(result.is_err(), "Should reject empty profile");
    }

    #[test]
    fn cva_rejects_invalid_recovery_rate() {
        let hazard = flat_hazard_curve(0.02);
        let discount = flat_discount_curve(0.03);
        let profile = uniform_profile(100.0, &[1.0, 2.0]);

        assert!(compute_cva(&profile, &hazard, &discount, -0.1).is_err());
        assert!(compute_cva(&profile, &hazard, &discount, 1.5).is_err());
    }

    #[test]
    fn cva_higher_hazard_gives_higher_cva() {
        let discount = flat_discount_curve(0.03);
        let times: Vec<f64> = (1..=20).map(|i| i as f64 * 0.5).collect();
        let profile = uniform_profile(1_000_000.0, &times);

        let hazard_low = flat_hazard_curve(0.01);
        let hazard_high = flat_hazard_curve(0.05);

        let cva_low = compute_cva(&profile, &hazard_low, &discount, 0.40)
            .expect("should work")
            .cva;
        let cva_high = compute_cva(&profile, &hazard_high, &discount, 0.40)
            .expect("should work")
            .cva;

        assert!(
            cva_high > cva_low,
            "Higher hazard rate should give higher CVA: low={cva_low}, high={cva_high}"
        );
    }

    #[test]
    fn cva_lower_recovery_gives_higher_cva() {
        let hazard = flat_hazard_curve(0.02);
        let discount = flat_discount_curve(0.03);
        let times: Vec<f64> = (1..=20).map(|i| i as f64 * 0.5).collect();
        let profile = uniform_profile(1_000_000.0, &times);

        let cva_high_r = compute_cva(&profile, &hazard, &discount, 0.60)
            .expect("should work")
            .cva;
        let cva_low_r = compute_cva(&profile, &hazard, &discount, 0.20)
            .expect("should work")
            .cva;

        assert!(
            cva_low_r > cva_high_r,
            "Lower recovery should give higher CVA: R=0.20 → {cva_low_r}, R=0.60 → {cva_high_r}"
        );
    }

    // ── B2: Mismatched vector length validation ─────────────────

    #[test]
    fn cva_rejects_mismatched_epe_length() {
        let hazard = flat_hazard_curve(0.02);
        let discount = flat_discount_curve(0.03);
        let profile = ExposureProfile {
            times: vec![1.0, 2.0, 3.0],
            mtm_values: vec![100.0, 200.0, 300.0],
            epe: vec![100.0, 200.0], // one short
            ene: vec![0.0, 0.0, 0.0],
            diagnostics: None,
        };
        assert!(
            compute_cva(&profile, &hazard, &discount, 0.40).is_err(),
            "Should reject profile with mismatched EPE length"
        );
    }

    #[test]
    fn cva_rejects_mismatched_ene_length() {
        let hazard = flat_hazard_curve(0.02);
        let discount = flat_discount_curve(0.03);
        let profile = ExposureProfile {
            times: vec![1.0, 2.0],
            mtm_values: vec![100.0, 200.0],
            epe: vec![100.0, 200.0],
            ene: vec![0.0], // one short
            diagnostics: None,
        };
        assert!(
            compute_cva(&profile, &hazard, &discount, 0.40).is_err(),
            "Should reject profile with mismatched ENE length"
        );
    }

    // ── M1: Effective EPE time-weighted average ─────────────────

    #[test]
    fn effective_epe_uniform_profile() {
        // For uniform EPE, first-year Effective EPE average should equal EPE.
        let hazard = flat_hazard_curve(0.02);
        let discount = flat_discount_curve(0.03);
        let times: Vec<f64> = (1..=20).map(|i| i as f64 * 0.5).collect();
        let profile = uniform_profile(1_000_000.0, &times);

        let result =
            compute_cva(&profile, &hazard, &discount, 0.40).expect("CVA computation should work");

        assert!(
            (result.effective_epe - 1_000_000.0).abs() < 1e-6,
            "For uniform EPE, effective EPE should equal 1M, got {}",
            result.effective_epe
        );
    }

    #[test]
    fn effective_epe_short_maturity() {
        // For a portfolio shorter than 1Y, normalization = maturity
        let hazard = flat_hazard_curve(0.02);
        let discount = flat_discount_curve(0.03);
        let times = vec![0.25, 0.5];
        let profile = uniform_profile(100.0, &times);

        let result =
            compute_cva(&profile, &hazard, &discount, 0.40).expect("CVA computation should work");

        // EPE = 100, effective EPE running max = 100 at all points
        // integral = 100 × 0.25 + 100 × 0.25 = 50
        // normalization = min(1, 0.5) = 0.5
        // effective_epe = 50 / 0.5 = 100
        assert!(
            (result.effective_epe - 100.0).abs() < 1e-6,
            "For uniform EPE with short maturity, time-weighted avg should equal EPE, got {}",
            result.effective_epe
        );
    }

    // ── DVA tests ────────────────────────────────────────────────

    /// Helper: build a profile with uniform ENE (negative exposure).
    fn uniform_ene_profile(ene_value: f64, times: &[f64]) -> ExposureProfile {
        ExposureProfile {
            times: times.to_vec(),
            mtm_values: times.iter().map(|_| -ene_value).collect(),
            epe: times.iter().map(|_| 0.0).collect(),
            ene: times.iter().map(|_| ene_value).collect(),
            diagnostics: None,
        }
    }

    #[test]
    fn dva_zero_ene_gives_zero_dva() {
        // With ENE = 0, DVA = 0 regardless of own credit quality
        let own_hazard = flat_hazard_curve(0.05);
        let discount = flat_discount_curve(0.03);
        let times: Vec<f64> = (1..=20).map(|i| i as f64 * 0.5).collect();
        let profile = uniform_profile(1_000_000.0, &times); // EPE only, ENE = 0

        let dva = compute_dva(&profile, &own_hazard, &discount, 0.40)
            .expect("DVA computation should work");

        assert!(
            dva.abs() < 1e-12,
            "DVA with zero ENE should be zero, got {dva}"
        );
    }

    #[test]
    fn dva_full_own_recovery_gives_zero_dva() {
        // With R_own = 1.0, LGD_own = 0 → DVA = 0
        let own_hazard = flat_hazard_curve(0.02);
        let discount = flat_discount_curve(0.03);
        let times: Vec<f64> = (1..=20).map(|i| i as f64 * 0.5).collect();
        let profile = uniform_ene_profile(1_000_000.0, &times);

        let dva = compute_dva(&profile, &own_hazard, &discount, 1.0)
            .expect("DVA computation should work with R_own=1");

        assert!(
            dva.abs() < 1e-12,
            "DVA with 100% own recovery should be zero, got {dva}"
        );
    }

    #[test]
    fn dva_positive_for_nonzero_ene_and_own_default_risk() {
        // Non-trivial case: positive ENE, positive own hazard rate
        let own_hazard = flat_hazard_curve(0.02);
        let discount = flat_discount_curve(0.03);
        let times: Vec<f64> = (1..=20).map(|i| i as f64 * 0.5).collect();
        let profile = uniform_ene_profile(1_000_000.0, &times);

        let dva = compute_dva(&profile, &own_hazard, &discount, 0.40)
            .expect("DVA computation should work");

        assert!(
            dva > 0.0,
            "DVA should be positive for non-zero ENE and own hazard rate, got {dva}"
        );
    }

    #[test]
    fn dva_analytical_check_with_flat_curves() {
        // For constant ENE, flat hazard rate λ_own, flat discount rate r:
        // DVA ≈ (1-R_own) × ENE × λ_own/(λ_own+r) × (1 - e^{-(λ_own+r)T})
        let lambda_own = 0.03;
        let r = 0.04;
        let recovery_own = 0.30;
        let ene = 500_000.0;
        let t_max = 10.0;

        let dt: f64 = 0.25;
        let n_steps = (t_max / dt).round() as usize;
        let times: Vec<f64> = (1..=n_steps).map(|i| i as f64 * dt).collect();

        let own_hazard = flat_hazard_curve(lambda_own);
        let discount = flat_discount_curve(r);
        let profile = uniform_ene_profile(ene, &times);

        let dva = compute_dva(&profile, &own_hazard, &discount, recovery_own)
            .expect("DVA computation should work");

        // Analytical formula (same structure as CVA but with ENE and own parameters)
        let lgd_own = 1.0 - recovery_own;
        let analytical = lgd_own * ene * lambda_own / (lambda_own + r)
            * (1.0 - (-(lambda_own + r) * t_max).exp());

        // Expect < 2% relative error (midpoint integration with EPE starting from 0)
        let rel_error = (dva - analytical).abs() / analytical;
        assert!(
            rel_error < 0.02,
            "DVA numerical ({dva:.2}) should be close to analytical ({analytical:.2}), rel_error={rel_error:.6}"
        );
    }

    #[test]
    fn dva_rejects_invalid_own_recovery_rate() {
        let own_hazard = flat_hazard_curve(0.02);
        let discount = flat_discount_curve(0.03);
        let profile = uniform_ene_profile(100.0, &[1.0, 2.0]);

        assert!(compute_dva(&profile, &own_hazard, &discount, -0.1).is_err());
        assert!(compute_dva(&profile, &own_hazard, &discount, 1.5).is_err());
    }

    // ── FVA tests ────────────────────────────────────────────────

    #[test]
    fn fva_zero_funding_spread_gives_zero_fva() {
        let discount = flat_discount_curve(0.03);
        let times: Vec<f64> = (1..=20).map(|i| i as f64 * 0.5).collect();
        let profile = uniform_profile(1_000_000.0, &times);

        let fva = compute_fva(&profile, &discount, 0.0, 0.0).expect("FVA computation should work");

        assert!(
            fva.abs() < 1e-12,
            "FVA with zero funding spread should be zero, got {fva}"
        );
    }

    #[test]
    fn fva_positive_for_positive_exposure_and_spread() {
        // Positive EPE with positive funding spread → positive FVA (cost)
        let discount = flat_discount_curve(0.03);
        let times: Vec<f64> = (1..=20).map(|i| i as f64 * 0.5).collect();
        let profile = uniform_profile(1_000_000.0, &times); // EPE only, ENE = 0

        let fva =
            compute_fva(&profile, &discount, 50.0, 50.0).expect("FVA computation should work");

        assert!(
            fva > 0.0,
            "FVA should be positive for positive exposure with funding spread, got {fva}"
        );
    }

    #[test]
    fn fva_negative_for_negative_exposure_and_benefit() {
        // Negative exposure (ENE only) with funding benefit → negative FVA (benefit)
        let discount = flat_discount_curve(0.03);
        let times: Vec<f64> = (1..=20).map(|i| i as f64 * 0.5).collect();
        let profile = uniform_ene_profile(1_000_000.0, &times); // ENE only, EPE = 0

        let fva =
            compute_fva(&profile, &discount, 50.0, 50.0).expect("FVA computation should work");

        assert!(
            fva < 0.0,
            "FVA should be negative for negative exposure with funding benefit, got {fva}"
        );
    }

    #[test]
    fn fva_analytical_check_with_flat_curves() {
        // For constant EPE, zero ENE, flat discount rate r, and funding spread s:
        // FVA = EPE × s × ∫₀ᵀ DF(t) dt = EPE × s × (1 - e^{-rT}) / r
        let r = 0.04;
        let epe = 1_000_000.0;
        let funding_spread_bps = 60.0; // 60 bps
        let spread = funding_spread_bps / 10_000.0;
        let t_max = 10.0;

        let dt: f64 = 0.25;
        let n_steps = (t_max / dt).round() as usize;
        let times: Vec<f64> = (1..=n_steps).map(|i| i as f64 * dt).collect();

        let discount = flat_discount_curve(r);
        let profile = uniform_profile(epe, &times);

        let fva = compute_fva(&profile, &discount, funding_spread_bps, funding_spread_bps)
            .expect("FVA computation should work");

        // Analytical: EPE × s × (1 - e^{-rT}) / r
        let analytical = epe * spread * (1.0 - (-r * t_max).exp()) / r;

        // Allow < 3% error due to midpoint integration with EPE starting from 0
        let rel_error = (fva - analytical).abs() / analytical;
        assert!(
            rel_error < 0.03,
            "FVA numerical ({fva:.2}) should be close to analytical ({analytical:.2}), rel_error={rel_error:.6}"
        );
    }

    // ── Bilateral XVA tests ──────────────────────────────────────

    #[test]
    fn bilateral_cva_equals_cva_minus_dva() {
        let counterparty_hazard = flat_hazard_curve(0.02);
        let own_hazard = flat_hazard_curve(0.03);
        let discount = flat_discount_curve(0.04);
        let times: Vec<f64> = (1..=20).map(|i| i as f64 * 0.5).collect();

        // Profile with both EPE and ENE
        let profile = ExposureProfile {
            times: times.clone(),
            mtm_values: times.iter().map(|_| 500_000.0).collect(),
            epe: times.iter().map(|_| 800_000.0).collect(),
            ene: times.iter().map(|_| 300_000.0).collect(),
            diagnostics: None,
        };

        let result = compute_bilateral_xva(
            &profile,
            &counterparty_hazard,
            &own_hazard,
            &discount,
            0.40,
            0.40,
            None, // no FVA
        )
        .expect("Bilateral XVA should compute");

        let dva = result.dva.expect("DVA should be computed");
        let bilateral = result
            .bilateral_cva
            .expect("Bilateral CVA should be computed");

        // Without FVA: bilateral_cva = cva - dva
        let expected_bilateral = result.cva - dva;
        assert!(
            (bilateral - expected_bilateral).abs() < 1e-10,
            "bilateral_cva ({bilateral:.6}) should equal cva ({:.6}) - dva ({dva:.6}) = {expected_bilateral:.6}",
            result.cva
        );

        // DVA should be positive (there is ENE and own default risk)
        assert!(dva > 0.0, "DVA should be positive, got {dva}");

        // Bilateral should be less than unilateral CVA (DVA offsets)
        assert!(
            bilateral < result.cva,
            "Bilateral CVA ({bilateral:.2}) should be less than unilateral CVA ({:.2})",
            result.cva
        );
    }

    #[test]
    fn bilateral_xva_with_fva() {
        let counterparty_hazard = flat_hazard_curve(0.02);
        let own_hazard = flat_hazard_curve(0.03);
        let discount = flat_discount_curve(0.04);
        let times: Vec<f64> = (1..=20).map(|i| i as f64 * 0.5).collect();

        let profile = ExposureProfile {
            times: times.clone(),
            mtm_values: times.iter().map(|_| 500_000.0).collect(),
            epe: times.iter().map(|_| 800_000.0).collect(),
            ene: times.iter().map(|_| 300_000.0).collect(),
            diagnostics: None,
        };

        let funding = FundingConfig {
            funding_spread_bps: 50.0,
            funding_benefit_bps: Some(30.0),
        };

        let result = compute_bilateral_xva(
            &profile,
            &counterparty_hazard,
            &own_hazard,
            &discount,
            0.40,
            0.40,
            Some(&funding),
        )
        .expect("Bilateral XVA with FVA should compute");

        let dva = result.dva.expect("DVA should be computed");
        let fva = result.fva.expect("FVA should be computed");
        let bilateral = result
            .bilateral_cva
            .expect("Bilateral CVA should be computed");

        // bilateral_cva = cva - dva + fva
        let expected_bilateral = result.cva - dva + fva;
        assert!(
            (bilateral - expected_bilateral).abs() < 1e-10,
            "bilateral_cva ({bilateral:.6}) should equal cva - dva + fva = {expected_bilateral:.6}"
        );

        // FVA should be positive since EPE > ENE and funding_spread > funding_benefit
        assert!(fva > 0.0, "FVA should be positive, got {fva}");
    }

    #[test]
    fn bilateral_xva_symmetric_funding() {
        // Test that FundingConfig with None benefit defaults to symmetric
        let counterparty_hazard = flat_hazard_curve(0.02);
        let own_hazard = flat_hazard_curve(0.03);
        let discount = flat_discount_curve(0.04);
        let times: Vec<f64> = (1..=20).map(|i| i as f64 * 0.5).collect();

        let profile = uniform_profile(1_000_000.0, &times);

        let funding_symmetric = FundingConfig {
            funding_spread_bps: 50.0,
            funding_benefit_bps: None, // defaults to 50.0
        };

        let funding_explicit = FundingConfig {
            funding_spread_bps: 50.0,
            funding_benefit_bps: Some(50.0),
        };

        let result_sym = compute_bilateral_xva(
            &profile,
            &counterparty_hazard,
            &own_hazard,
            &discount,
            0.40,
            0.40,
            Some(&funding_symmetric),
        )
        .expect("Should compute");

        let result_exp = compute_bilateral_xva(
            &profile,
            &counterparty_hazard,
            &own_hazard,
            &discount,
            0.40,
            0.40,
            Some(&funding_explicit),
        )
        .expect("Should compute");

        let fva_sym = result_sym.fva.expect("FVA should be computed");
        let fva_exp = result_exp.fva.expect("FVA should be computed");
        assert!(
            (fva_sym - fva_exp).abs() < 1e-12,
            "Symmetric FVA ({fva_sym}) should equal explicit ({fva_exp})"
        );
    }

    #[test]
    fn bilateral_xva_applies_first_to_default_weighting() {
        let counterparty_hazard = flat_hazard_curve(0.08);
        let own_hazard = flat_hazard_curve(0.12);
        let discount = flat_discount_curve(0.03);
        let times: Vec<f64> = (1..=20).map(|i| i as f64 * 0.5).collect();

        let profile = ExposureProfile {
            times: times.clone(),
            mtm_values: times.iter().map(|_| 250_000.0).collect(),
            epe: times.iter().map(|_| 800_000.0).collect(),
            ene: times.iter().map(|_| 500_000.0).collect(),
            diagnostics: None,
        };

        let funding = FundingConfig {
            funding_spread_bps: 60.0,
            funding_benefit_bps: Some(40.0),
        };

        let unilateral_cva = compute_cva(&profile, &counterparty_hazard, &discount, 0.40)
            .expect("unilateral CVA should compute")
            .cva;
        let unilateral_dva = compute_dva(&profile, &own_hazard, &discount, 0.35)
            .expect("unilateral DVA should compute");
        let standalone_fva =
            compute_fva(&profile, &discount, 60.0, 40.0).expect("standalone FVA should compute");

        let bilateral = compute_bilateral_xva(
            &profile,
            &counterparty_hazard,
            &own_hazard,
            &discount,
            0.40,
            0.35,
            Some(&funding),
        )
        .expect("bilateral XVA should compute");

        let bilateral_dva = bilateral.dva.expect("DVA should be populated");
        let bilateral_fva = bilateral.fva.expect("FVA should be populated");

        assert!(
            bilateral.cva < unilateral_cva,
            "First-to-default weighting should reduce bilateral CVA component: bilateral={} unilateral={}",
            bilateral.cva,
            unilateral_cva
        );
        assert!(
            bilateral_dva < unilateral_dva,
            "First-to-default weighting should reduce bilateral DVA component: bilateral={} unilateral={}",
            bilateral_dva,
            unilateral_dva
        );
        assert!(
            bilateral_fva.abs() < standalone_fva.abs(),
            "Joint-survival weighting should reduce FVA magnitude: bilateral={} standalone={}",
            bilateral_fva,
            standalone_fva
        );
    }

    #[test]
    fn xva_result_returns_none_for_optional_fields() {
        // Existing compute_cva should still work and return None for new fields
        let hazard = flat_hazard_curve(0.02);
        let discount = flat_discount_curve(0.03);
        let times: Vec<f64> = (1..=20).map(|i| i as f64 * 0.5).collect();
        let profile = uniform_profile(1_000_000.0, &times);

        let result =
            compute_cva(&profile, &hazard, &discount, 0.40).expect("CVA computation should work");

        assert!(result.cva > 0.0, "CVA should still be positive");
        assert!(
            result.dva.is_none(),
            "DVA should be None for unilateral CVA"
        );
        assert!(
            result.fva.is_none(),
            "FVA should be None for unilateral CVA"
        );
        assert!(
            result.bilateral_cva.is_none(),
            "Bilateral CVA should be None for unilateral CVA"
        );
    }

    #[test]
    fn xva_result_serde_roundtrip_with_optional_fields() {
        // Ensure XvaResult can be serialized/deserialized with new Optional fields
        let hazard = flat_hazard_curve(0.02);
        let discount = flat_discount_curve(0.03);
        let times: Vec<f64> = (1..=20).map(|i| i as f64 * 0.5).collect();
        let profile = uniform_profile(1_000_000.0, &times);

        let result =
            compute_cva(&profile, &hazard, &discount, 0.40).expect("CVA computation should work");

        let json = serde_json::to_string(&result).expect("Should serialize");
        let deserialized: XvaResult = serde_json::from_str(&json).expect("Should deserialize");

        assert!(
            (deserialized.cva - result.cva).abs() < 1e-12,
            "CVA should survive roundtrip"
        );
        assert!(deserialized.dva.is_none());
        assert!(deserialized.fva.is_none());
        assert!(deserialized.bilateral_cva.is_none());
    }
}
