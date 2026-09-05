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
//! In [`crate::xva::cva::compute_bilateral_xva`], posted IM reduces ENE before
//! DVA integration: `ENE_effective(t) = max(ENE(t) - IM(t), 0)`. The public
//! unilateral [`crate::xva::cva::compute_dva`] function retains the unadjusted
//! formula above.
//!
//! **FVA** — funding cost/benefit on uncollateralized exposure:
//!
//! ```text
//! FVA = Σᵢ [EPE_mid(tᵢ) × s_f⁺ - ENE_mid(tᵢ) × s_f⁻] × DF_mid(tᵢ) × Δtᵢ
//! ```
//!
//! **MVA** — funding cost of posted initial margin (see [`crate::xva::mva`]):
//!
//! ```text
//! MVA = Σᵢ s_im × IM_mid(tᵢ) × DF_mid(tᵢ) × S_joint_mid(tᵢ) × Δtᵢ
//! ```
//!
//! **Total XVA** = CVA − DVA + FVA + MVA
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

use finstack_quant_core::market_data::term_structures::{DiscountCurve, HazardCurve};

use super::mva;
use super::types::{ExposureProfile, FundingConfig, XvaResult};

fn validate_exposure_profile_lengths(
    exposure_profile: &ExposureProfile,
    label: &str,
) -> finstack_quant_core::Result<usize> {
    exposure_profile.validate()?;
    if exposure_profile.times.is_empty() {
        return Err(finstack_quant_core::Error::Validation(format!(
            "{label}: exposure profile must not be empty"
        )));
    }

    let n = exposure_profile.times.len();
    if exposure_profile.epe.len() != n || exposure_profile.ene.len() != n {
        return Err(finstack_quant_core::Error::Validation(format!(
            "{label}: exposure profile vector lengths must be equal \
             (times={n}, epe={}, ene={})",
            exposure_profile.epe.len(),
            exposure_profile.ene.len()
        )));
    }

    Ok(n)
}

fn horizons_match(lhs: f64, rhs: f64) -> bool {
    let scale = lhs.abs().max(rhs.abs()).max(1.0);
    (lhs - rhs).abs() <= 1.0e-12 * scale
}

fn im_at(profile: &mva::ImProfile, t: f64) -> f64 {
    if t <= profile.times[0] {
        return profile.im_values[0];
    }
    for i in 1..profile.times.len() {
        if t <= profile.times[i] {
            let t0 = profile.times[i - 1];
            let t1 = profile.times[i];
            let im0 = profile.im_values[i - 1];
            let im1 = profile.im_values[i];
            return im0 + (im1 - im0) * (t - t0) / (t1 - t0);
        }
    }
    profile.im_values[profile.im_values.len() - 1]
}

fn compute_cva_internal(
    exposure_profile: &ExposureProfile,
    counterparty_hazard_curve: &HazardCurve,
    discount_curve: &DiscountCurve,
    recovery_rate: f64,
    own_survival_curve: Option<&HazardCurve>,
) -> finstack_quant_core::Result<XvaResult> {
    let n = validate_exposure_profile_lengths(exposure_profile, "CVA")?;

    if !(0.0..=1.0).contains(&recovery_rate) {
        return Err(finstack_quant_core::Error::Validation(format!(
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
            return Err(finstack_quant_core::Error::Validation(format!(
                "CVA: non-finite survival probability at t={t}: S(t)={survival_t}"
            )));
        }

        let own_survival_t = if let Some(curve) = own_survival_curve {
            let sp = curve.sp(t);
            if !sp.is_finite() {
                return Err(finstack_quant_core::Error::Validation(format!(
                    "CVA: non-finite own survival probability at t={t}: S_own(t)={sp}"
                )));
            }
            sp
        } else {
            1.0
        };

        let df_t = discount_curve.df(t);
        if !df_t.is_finite() {
            return Err(finstack_quant_core::Error::Validation(format!(
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
        mva: None,
        total_xva: cva,
        epe_profile,
        ene_profile,
        pfe_profile,
        max_pfe,
        effective_epe_profile,
        effective_epe,
        meta: finstack_quant_core::config::results_meta(
            &finstack_quant_core::config::FinstackConfig::default(),
        ),
    })
}

fn compute_dva_internal(
    exposure_profile: &ExposureProfile,
    own_hazard_curve: &HazardCurve,
    discount_curve: &DiscountCurve,
    own_recovery_rate: f64,
    counterparty_survival_curve: Option<&HazardCurve>,
    posted_im: Option<&mva::ImProfile>,
) -> finstack_quant_core::Result<f64> {
    let n = validate_exposure_profile_lengths(exposure_profile, "DVA")?;

    if !(0.0..=1.0).contains(&own_recovery_rate) {
        return Err(finstack_quant_core::Error::Validation(format!(
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
        let im_t = posted_im.map_or(0.0, |profile| im_at(profile, t));
        let ene_t = (exposure_profile.ene[i] - im_t).max(0.0);

        let survival_t = own_hazard_curve.sp(t);
        if !survival_t.is_finite() {
            return Err(finstack_quant_core::Error::Validation(format!(
                "DVA: non-finite survival probability at t={t}: S_own(t)={survival_t}"
            )));
        }

        let counterparty_survival_t = if let Some(curve) = counterparty_survival_curve {
            let sp = curve.sp(t);
            if !sp.is_finite() {
                return Err(finstack_quant_core::Error::Validation(format!(
                    "DVA: non-finite counterparty survival probability at t={t}: S_c(t)={sp}"
                )));
            }
            sp
        } else {
            1.0
        };

        let df_t = discount_curve.df(t);
        if !df_t.is_finite() {
            return Err(finstack_quant_core::Error::Validation(format!(
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
    funding_spread_bp: f64,
    funding_benefit_bp: f64,
    counterparty_hazard_curve: Option<&HazardCurve>,
    own_hazard_curve: Option<&HazardCurve>,
) -> finstack_quant_core::Result<f64> {
    let n = validate_exposure_profile_lengths(exposure_profile, "FVA")?;

    let spread_cost = funding_spread_bp / 10_000.0;
    let spread_benefit = funding_benefit_bp / 10_000.0;

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
                return Err(finstack_quant_core::Error::Validation(format!(
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
                return Err(finstack_quant_core::Error::Validation(format!(
                    "FVA: non-finite own survival probability at t={t}: S_own(t)={sp}"
                )));
            }
            sp
        } else {
            1.0
        };

        let df_t = discount_curve.df(t);
        if !df_t.is_finite() {
            return Err(finstack_quant_core::Error::Validation(format!(
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
/// ```
/// use finstack_quant_margin::xva::cva::compute_cva;
/// use finstack_quant_margin::xva::types::ExposureProfile;
/// use finstack_quant_core::market_data::term_structures::{DiscountCurve, HazardCurve};
/// use time::macros::date;
///
/// # fn main() -> finstack_quant_core::Result<()> {
/// let as_of = date!(2025 - 01 - 01);
///
/// // Flat 2% hazard and 4% discounting out to five years.
/// let hazard_curve = HazardCurve::builder("ACME")
///     .base_date(as_of)
///     .recovery_rate(0.40)
///     .knots([(1.0, 0.02), (5.0, 0.02)])
///     .build()?;
/// let discount_curve = DiscountCurve::builder("USD-OIS")
///     .base_date(as_of)
///     .knots([(0.0, 1.0), (5.0, (-0.04f64 * 5.0).exp())])
///     .build()?;
///
/// let profile = ExposureProfile {
///     times: vec![1.0, 2.0, 3.0],
///     mtm_values: vec![1_000_000.0; 3],
///     epe: vec![1_000_000.0; 3],
///     ene: vec![0.0; 3],
///     diagnostics: None,
/// };
///
/// let result = compute_cva(&profile, &hazard_curve, &discount_curve, 0.40)?;
/// assert!(result.cva > 0.0);
/// # Ok(())
/// # }
/// ```
pub fn compute_cva(
    exposure_profile: &ExposureProfile,
    counterparty_hazard_curve: &HazardCurve,
    discount_curve: &DiscountCurve,
    recovery_rate: f64,
) -> finstack_quant_core::Result<XvaResult> {
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
/// - Gregory, J. (2020). *The xVA Challenge*, 4th ed. Wiley. Chapter 17. `docs/REFERENCES.md#gregory-xva-challenge`
/// - BCBS (2011). "Application of own credit risk adjustments to derivatives."
pub fn compute_dva(
    exposure_profile: &ExposureProfile,
    own_hazard_curve: &HazardCurve,
    discount_curve: &DiscountCurve,
    own_recovery_rate: f64,
) -> finstack_quant_core::Result<f64> {
    compute_dva_internal(
        exposure_profile,
        own_hazard_curve,
        discount_curve,
        own_recovery_rate,
        None,
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
/// * `funding_spread_bp` - Funding cost spread in basis points (applied to EPE)
/// * `funding_benefit_bp` - Funding benefit spread in basis points (applied to ENE)
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
/// - Either funding spread is negative or non-finite
/// - The funding benefit spread exceeds the funding cost spread
/// - Curve evaluations return non-finite values
///
/// # References
///
/// - Gregory, J. (2020). *The xVA Challenge*, 4th ed. Wiley. Chapter 19. `docs/REFERENCES.md#gregory-xva-challenge`
/// - Green, A. (2015). *XVA: Credit, Funding and Capital Valuation Adjustments*.
///   Wiley. Chapter 5. `docs/REFERENCES.md#green-xva`
pub fn compute_fva(
    exposure_profile: &ExposureProfile,
    discount_curve: &DiscountCurve,
    funding_spread_bp: f64,
    funding_benefit_bp: f64,
) -> finstack_quant_core::Result<f64> {
    FundingConfig {
        funding_spread_bp,
        funding_benefit_bp: Some(funding_benefit_bp),
        ..Default::default()
    }
    .validate()?;

    compute_fva_internal(
        exposure_profile,
        discount_curve,
        funding_spread_bp,
        funding_benefit_bp,
        None,
        None,
    )
}

/// Compute bilateral XVA: CVA, DVA, FVA, MVA, and the all-in adjustment.
///
/// Accounts for:
/// - Counterparty default risk (CVA)
/// - Own-default benefit (DVA)
/// - Funding costs and benefits (FVA, when `funding` is provided)
/// - Initial-margin funding cost (MVA, when `funding.im_profile` is provided)
///
/// All four legs are weighted by joint (first-to-default) survival, so
/// components are not double-counted across the credit and funding legs.
///
/// # Outputs
///
/// ```text
/// result.total_xva = CVA − DVA + FVA + MVA
/// ```
///
/// Uncomputed optional legs contribute zero.
///
/// # Arguments
///
/// * `exposure_profile` - EPE/ENE profile from exposure simulation
/// * `counterparty_hazard_curve` - Hazard curve for the counterparty's credit
/// * `own_hazard_curve` - Hazard curve for the institution's own credit
/// * `discount_curve` - Risk-free discount curve for present-valuing
/// * `counterparty_recovery_rate` - Recovery rate for counterparty default (0 to 1)
/// * `own_recovery_rate` - Recovery rate for own default (0 to 1)
/// * `funding` - Optional funding configuration driving FVA and, when it
///   carries an [`crate::xva::mva::ImProfile`], MVA
///
/// # Returns
///
/// An [`XvaResult`] containing CVA, DVA, FVA, MVA, total XVA, and exposure
/// metrics.
///
/// # Errors
///
/// Returns an error if any sub-calculation (CVA, DVA, FVA, MVA) fails,
/// [`FundingConfig`] is invalid, or the IM and exposure profiles have
/// different terminal horizons.
///
/// # References
///
/// - Gregory, J. (2020). *The xVA Challenge*, 4th ed. Wiley. Chapters 14, 17, 19. `docs/REFERENCES.md#gregory-xva-challenge`
/// - Green, A. (2015). *XVA: Credit, Funding and Capital Valuation Adjustments*.
///   Wiley. Chapters 9-10. `docs/REFERENCES.md#green-xva`
/// - Brigo, D., Morini, M. & Pallavicini, A. (2013). *Counterparty Credit Risk,
///   Collateral and Funding*. Wiley. `docs/REFERENCES.md#brigo-mercurio-2006-interest-rate-models`
pub fn compute_bilateral_xva(
    exposure_profile: &ExposureProfile,
    counterparty_hazard_curve: &HazardCurve,
    own_hazard_curve: &HazardCurve,
    discount_curve: &DiscountCurve,
    counterparty_recovery_rate: f64,
    own_recovery_rate: f64,
    funding: Option<&FundingConfig>,
) -> finstack_quant_core::Result<XvaResult> {
    if let Some(fc) = funding {
        fc.validate()?;
    }

    let mut result = compute_cva_internal(
        exposure_profile,
        counterparty_hazard_curve,
        discount_curve,
        counterparty_recovery_rate,
        Some(own_hazard_curve),
    )?;

    let posted_im = funding.and_then(|fc| fc.im_profile.as_ref());
    if let Some(im_profile) = posted_im {
        let exposure_horizon = exposure_profile.times[exposure_profile.times.len() - 1];
        let im_horizon = im_profile.times[im_profile.times.len() - 1];
        if !horizons_match(exposure_horizon, im_horizon) {
            return Err(finstack_quant_core::Error::Validation(format!(
                "Bilateral XVA: IM profile horizon {im_horizon} must match exposure profile \
                 horizon {exposure_horizon}"
            )));
        }
    }

    let dva = compute_dva_internal(
        exposure_profile,
        own_hazard_curve,
        discount_curve,
        own_recovery_rate,
        Some(counterparty_hazard_curve),
        posted_im,
    )?;
    result.dva = Some(dva);

    let fva = if let Some(fc) = funding {
        let fva_val = compute_fva_internal(
            exposure_profile,
            discount_curve,
            fc.funding_spread_bp,
            fc.effective_benefit_bp(),
            Some(counterparty_hazard_curve),
            Some(own_hazard_curve),
        )?;
        result.fva = Some(fva_val);
        fva_val
    } else {
        result.fva = None;
        0.0
    };

    let mva = match funding.and_then(|fc| fc.im_profile.as_ref().map(|im| (fc, im))) {
        Some((fc, im_profile)) => {
            let mva_val = mva::compute_mva_internal(
                im_profile,
                &[(0.0, fc.effective_margin_spread_bp())],
                discount_curve,
                Some(own_hazard_curve),
                Some(counterparty_hazard_curve),
            )?
            .mva;
            result.mva = Some(mva_val);
            mva_val
        }
        None => {
            result.mva = None;
            0.0
        }
    };

    result.total_xva = result.cva - dva + fva + mva;

    Ok(result)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::xva::mva::ImProfile;
    use finstack_quant_core::dates::Date;
    use time::Month;

    /// Helper: build a flat hazard rate curve.
    fn flat_hazard_curve(lambda: f64) -> HazardCurve {
        let base = Date::from_calendar_date(2025, Month::January, 1).expect("Valid date");
        HazardCurve::builder("COUNTERPARTY")
            .base_date(base)
            .knots([(0.0, lambda), (30.0, lambda)])
            .recovery_rate(0.40)
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
            .interp(finstack_quant_core::math::interp::InterpStyle::LogLinear)
            .build()
            .expect("DiscountCurve should build")
    }

    fn uniform_profile(epe_value: f64, times: &[f64]) -> ExposureProfile {
        ExposureProfile {
            times: times.to_vec(),
            mtm_values: times.iter().map(|_| epe_value).collect(),
            epe: times.iter().map(|_| epe_value).collect(),
            ene: times.iter().map(|_| 0.0).collect(),
            diagnostics: None,
        }
    }

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
        // = (1-R) × EPE × λ/(λ+r) × (1 - e^{-(λ+r)T})
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
            times,
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
    fn standalone_fva_rejects_invalid_funding_spreads() {
        let discount = flat_discount_curve(0.03);
        let profile = uniform_profile(1_000_000.0, &[1.0, 2.0]);
        let cases = [
            (-1.0, 0.0, "funding_spread_bp"),
            (f64::NAN, 0.0, "funding_spread_bp"),
            (f64::INFINITY, 0.0, "funding_spread_bp"),
            (50.0, -1.0, "funding_benefit_bp"),
            (50.0, f64::NAN, "funding_benefit_bp"),
            (50.0, f64::INFINITY, "funding_benefit_bp"),
            (50.0, 50.1, "must not exceed"),
        ];

        for (cost, benefit, expected) in cases {
            let err = compute_fva(&profile, &discount, cost, benefit)
                .expect_err("standalone FVA must enforce FundingConfig spread validation");
            assert!(err.to_string().contains(expected), "{err}");
        }
    }

    #[test]
    fn fva_analytical_check_with_flat_curves() {
        // For constant EPE, zero ENE, flat discount rate r, and funding spread s:
        // FVA = EPE × s × ∫₀ᵀ DF(t) dt = EPE × s × (1 - e^{-rT}) / r
        let r = 0.04;
        let epe = 1_000_000.0;
        let funding_spread_bp = 60.0; // 60 bp
        let spread = funding_spread_bp / 10_000.0;
        let t_max = 10.0;

        let dt: f64 = 0.25;
        let n_steps = (t_max / dt).round() as usize;
        let times: Vec<f64> = (1..=n_steps).map(|i| i as f64 * dt).collect();

        let discount = flat_discount_curve(r);
        let profile = uniform_profile(epe, &times);

        let fva = compute_fva(&profile, &discount, funding_spread_bp, funding_spread_bp)
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
        let total = result.total_xva;
        let expected_total = result.cva - dva;
        assert!(
            (total - expected_total).abs() < 1e-10,
            "total_xva ({total:.6}) should equal cva ({:.6}) - dva ({dva:.6}) = {expected_total:.6}",
            result.cva
        );

        // DVA should be positive (there is ENE and own default risk)
        assert!(dva > 0.0, "DVA should be positive, got {dva}");

        // Bilateral should be less than unilateral CVA (DVA offsets)
        assert!(
            total < result.cva,
            "Total XVA ({total:.2}) should be less than unilateral CVA ({:.2}) when DVA is the only other leg",
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
            funding_spread_bp: 50.0,
            funding_benefit_bp: Some(30.0),
            ..Default::default()
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
        let total = result.total_xva;
        let expected_total = result.cva - dva + fva;
        assert!(
            (total - expected_total).abs() < 1e-10,
            "total_xva ({total:.6}) should equal cva - dva + fva = {expected_total:.6}"
        );

        // No IM profile supplied, so MVA is not computed.
        assert!(
            result.mva.is_none(),
            "MVA should be None without an IM profile"
        );

        // FVA should be positive since EPE > ENE and funding_spread > funding_benefit
        assert!(fva > 0.0, "FVA should be positive, got {fva}");
    }

    #[test]
    fn bilateral_xva_includes_mva_in_total() {
        let counterparty_hazard = flat_hazard_curve(0.02);
        let own_hazard = flat_hazard_curve(0.03);
        let discount = flat_discount_curve(0.04);
        let times: Vec<f64> = (1..=20).map(|i| i as f64 * 0.5).collect();
        let profile = uniform_profile(1_000_000.0, &times);

        let im_profile = ImProfile {
            times: times.clone(),
            im_values: vec![2_000_000.0; times.len()],
        };
        let funding = FundingConfig {
            funding_spread_bp: 50.0,
            funding_benefit_bp: Some(30.0),
            im_profile: Some(im_profile),
            margin_funding_spread_bp: None,
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
        .expect("Bilateral XVA with MVA should compute");

        let dva = result.dva.expect("DVA");
        let fva = result.fva.expect("FVA");
        let mva = result
            .mva
            .expect("MVA should be computed when im_profile is set");
        let total = result.total_xva;

        assert!(
            mva > 0.0,
            "MVA should be a positive funding cost, got {mva}"
        );
        let expected_total = result.cva - dva + fva + mva;
        assert!(
            (total - expected_total).abs() < 1e-10,
            "total_xva ({total:.6}) should equal cva - dva + fva + mva = {expected_total:.6}"
        );
    }

    #[test]
    fn bilateral_xva_mva_uses_joint_survival() {
        // The bilateral engine conditions IM funding on neither party having
        // defaulted, so its MVA must be strictly below the standalone
        // (own-survival-only) figure whenever the counterparty can default.
        let counterparty_hazard = flat_hazard_curve(0.05);
        let own_hazard = flat_hazard_curve(0.03);
        let discount = flat_discount_curve(0.02);
        let times: Vec<f64> = (1..=10).map(|i| i as f64).collect();
        let profile = uniform_profile(1_000_000.0, &times);

        let im_profile = ImProfile {
            times: times.clone(),
            im_values: vec![5_000_000.0; times.len()],
        };
        let funding = FundingConfig {
            funding_spread_bp: 80.0,
            funding_benefit_bp: None,
            im_profile: Some(im_profile.clone()),
            margin_funding_spread_bp: None,
        };

        let bilateral_mva = compute_bilateral_xva(
            &profile,
            &counterparty_hazard,
            &own_hazard,
            &discount,
            0.40,
            0.40,
            Some(&funding),
        )
        .expect("should compute")
        .mva
        .expect("MVA");

        let standalone_mva =
            crate::xva::mva::compute_mva(&im_profile, &[(0.0, 80.0)], &discount, Some(&own_hazard))
                .expect("standalone MVA")
                .mva;

        assert!(
            bilateral_mva < standalone_mva,
            "joint-survival MVA ({bilateral_mva:.2}) should be below own-survival-only \
             MVA ({standalone_mva:.2})"
        );
        assert!(bilateral_mva > 0.0);
    }

    #[test]
    fn bilateral_xva_margin_spread_defaults_to_funding_spread() {
        let counterparty_hazard = flat_hazard_curve(0.02);
        let own_hazard = flat_hazard_curve(0.03);
        let discount = flat_discount_curve(0.02);
        let times: Vec<f64> = (1..=8).map(|i| i as f64 * 0.5).collect();
        let profile = uniform_profile(500_000.0, &times);
        let im_profile = ImProfile {
            times: times.clone(),
            im_values: vec![1_000_000.0; times.len()],
        };

        let mva_of = |margin_spread: Option<f64>| {
            let funding = FundingConfig {
                funding_spread_bp: 40.0,
                funding_benefit_bp: None,
                im_profile: Some(im_profile.clone()),
                margin_funding_spread_bp: margin_spread,
            };
            compute_bilateral_xva(
                &profile,
                &counterparty_hazard,
                &own_hazard,
                &discount,
                0.40,
                0.40,
                Some(&funding),
            )
            .expect("should compute")
            .mva
            .expect("MVA")
        };

        let implicit = mva_of(None);
        let explicit_same = mva_of(Some(40.0));
        let explicit_half = mva_of(Some(20.0));

        assert!(
            (implicit - explicit_same).abs() < 1e-12,
            "absent margin spread ({implicit}) should fall back to funding_spread_bp \
             ({explicit_same})"
        );
        // MVA is linear in the spread.
        assert!((explicit_half * 2.0 - implicit).abs() / implicit < 1e-12);
    }

    #[test]
    fn bilateral_xva_rejects_invalid_im_profile() {
        let counterparty_hazard = flat_hazard_curve(0.02);
        let own_hazard = flat_hazard_curve(0.03);
        let discount = flat_discount_curve(0.02);
        let times: Vec<f64> = (1..=4).map(|i| i as f64).collect();
        let profile = uniform_profile(500_000.0, &times);

        let funding = FundingConfig {
            funding_spread_bp: 40.0,
            funding_benefit_bp: None,
            im_profile: Some(ImProfile {
                times: vec![1.0, 2.0],
                im_values: vec![-1.0, 2.0],
            }),
            margin_funding_spread_bp: None,
        };

        assert!(
            compute_bilateral_xva(
                &profile,
                &counterparty_hazard,
                &own_hazard,
                &discount,
                0.40,
                0.40,
                Some(&funding),
            )
            .is_err(),
            "a negative IM value must fail closed"
        );
    }

    #[test]
    fn bilateral_xva_rejects_invalid_direct_funding_config() {
        let counterparty_hazard = flat_hazard_curve(0.02);
        let own_hazard = flat_hazard_curve(0.03);
        let discount = flat_discount_curve(0.02);
        let profile = uniform_profile(500_000.0, &[1.0, 2.0]);
        let funding = FundingConfig {
            funding_spread_bp: -1.0,
            ..Default::default()
        };

        assert!(
            compute_bilateral_xva(
                &profile,
                &counterparty_hazard,
                &own_hazard,
                &discount,
                0.40,
                0.40,
                Some(&funding),
            )
            .is_err(),
            "direct Rust callers must not bypass FundingConfig validation"
        );
    }

    #[test]
    fn bilateral_xva_rejects_im_horizon_mismatch() {
        let counterparty_hazard = flat_hazard_curve(0.02);
        let own_hazard = flat_hazard_curve(0.03);
        let discount = flat_discount_curve(0.02);
        let profile = uniform_profile(500_000.0, &[1.0, 2.0]);
        let funding = FundingConfig {
            funding_spread_bp: 40.0,
            im_profile: Some(ImProfile {
                times: vec![1.0, 3.0],
                im_values: vec![100_000.0, 0.0],
            }),
            ..Default::default()
        };

        assert!(
            compute_bilateral_xva(
                &profile,
                &counterparty_hazard,
                &own_hazard,
                &discount,
                0.40,
                0.40,
                Some(&funding),
            )
            .is_err(),
            "IM and exposure profiles must terminate at the same horizon"
        );
    }

    #[test]
    fn bilateral_xva_accepts_im_horizon_within_scale_aware_tolerance() {
        let counterparty_hazard = flat_hazard_curve(0.02);
        let own_hazard = flat_hazard_curve(0.03);
        let discount = flat_discount_curve(0.02);
        let horizon = 2.0_f64;
        let tolerance = 1.0e-12 * horizon;
        let profile = uniform_profile(500_000.0, &[1.0, horizon]);
        let funding = FundingConfig {
            funding_spread_bp: 40.0,
            im_profile: Some(ImProfile {
                times: vec![1.0, horizon + 0.99 * tolerance],
                im_values: vec![100_000.0, 0.0],
            }),
            ..Default::default()
        };

        compute_bilateral_xva(
            &profile,
            &counterparty_hazard,
            &own_hazard,
            &discount,
            0.40,
            0.40,
            Some(&funding),
        )
        .expect("a terminal mismatch within the scale-aware tolerance must be accepted");
    }

    #[test]
    fn bilateral_xva_rejects_im_horizon_just_outside_scale_aware_tolerance() {
        let counterparty_hazard = flat_hazard_curve(0.02);
        let own_hazard = flat_hazard_curve(0.03);
        let discount = flat_discount_curve(0.02);
        let horizon = 2.0_f64;
        let tolerance = 1.0e-12 * horizon;
        let profile = uniform_profile(500_000.0, &[1.0, horizon]);
        let funding = FundingConfig {
            funding_spread_bp: 40.0,
            im_profile: Some(ImProfile {
                times: vec![1.0, horizon + 1.01 * tolerance],
                im_values: vec![100_000.0, 0.0],
            }),
            ..Default::default()
        };

        assert!(
            compute_bilateral_xva(
                &profile,
                &counterparty_hazard,
                &own_hazard,
                &discount,
                0.40,
                0.40,
                Some(&funding),
            )
            .is_err(),
            "a terminal mismatch just outside the scale-aware tolerance must be rejected"
        );
    }

    #[test]
    fn bilateral_dva_is_reduced_by_posted_im() {
        let counterparty_hazard = flat_hazard_curve(0.02);
        let own_hazard = flat_hazard_curve(0.03);
        let discount = flat_discount_curve(0.02);
        let profile = uniform_ene_profile(1_000_000.0, &[1.0, 2.0, 3.0]);

        let without_im = compute_bilateral_xva(
            &profile,
            &counterparty_hazard,
            &own_hazard,
            &discount,
            0.40,
            0.40,
            None,
        )
        .expect("bilateral XVA without IM")
        .dva
        .expect("DVA");

        let funding = FundingConfig {
            funding_spread_bp: 40.0,
            im_profile: Some(ImProfile {
                times: vec![1.0, 3.0],
                im_values: vec![250_000.0, 750_000.0],
            }),
            ..Default::default()
        };
        let with_im = compute_bilateral_xva(
            &profile,
            &counterparty_hazard,
            &own_hazard,
            &discount,
            0.40,
            0.40,
            Some(&funding),
        )
        .expect("bilateral XVA with IM")
        .dva
        .expect("DVA");

        assert!(
            with_im > 0.0 && with_im < without_im,
            "posted IM should reduce bilateral DVA: with_im={with_im}, without_im={without_im}"
        );
    }

    #[test]
    fn bilateral_dva_linearly_interpolates_im_onto_exposure_grid() {
        let counterparty_hazard = flat_hazard_curve(0.02);
        let own_hazard = flat_hazard_curve(0.03);
        let discount = flat_discount_curve(0.02);
        let profile = uniform_ene_profile(900_000.0, &[1.0, 2.0, 3.0]);

        let dva_for_im = |im_profile: ImProfile| {
            let funding = FundingConfig {
                funding_spread_bp: 0.0,
                im_profile: Some(im_profile),
                ..Default::default()
            };
            compute_bilateral_xva(
                &profile,
                &counterparty_hazard,
                &own_hazard,
                &discount,
                0.40,
                0.40,
                Some(&funding),
            )
            .expect("bilateral XVA")
            .dva
            .expect("DVA")
        };

        let coarse_grid_dva = dva_for_im(ImProfile {
            times: vec![1.0, 3.0],
            im_values: vec![100_000.0, 500_000.0],
        });
        let explicit_grid_dva = dva_for_im(ImProfile {
            times: vec![1.0, 2.0, 3.0],
            im_values: vec![100_000.0, 300_000.0, 500_000.0],
        });

        assert!(
            (coarse_grid_dva - explicit_grid_dva).abs() < 1.0e-12,
            "linear interpolation must reproduce explicit exposure-grid IM: \
             coarse={coarse_grid_dva}, explicit={explicit_grid_dva}"
        );
    }

    #[test]
    fn bilateral_dva_zero_floors_overcollateralized_ene() {
        let counterparty_hazard = flat_hazard_curve(0.02);
        let own_hazard = flat_hazard_curve(0.03);
        let discount = flat_discount_curve(0.02);
        let profile = ExposureProfile {
            times: vec![1.0, 2.0, 3.0],
            mtm_values: vec![-100_000.0, -250_000.0, -400_000.0],
            epe: vec![0.0; 3],
            ene: vec![100_000.0, 250_000.0, 400_000.0],
            diagnostics: None,
        };
        let funding = FundingConfig {
            funding_spread_bp: 0.0,
            im_profile: Some(ImProfile {
                times: vec![1.0, 3.0],
                im_values: vec![500_000.0, 500_000.0],
            }),
            ..Default::default()
        };

        let dva = compute_bilateral_xva(
            &profile,
            &counterparty_hazard,
            &own_hazard,
            &discount,
            0.40,
            0.40,
            Some(&funding),
        )
        .expect("bilateral XVA")
        .dva
        .expect("DVA");

        assert!(
            dva.abs() < 1.0e-12,
            "max(ENE - IM, 0) must zero-floor overcollateralized exposure, got {dva}"
        );
    }

    #[test]
    fn unilateral_compute_dva_remains_unadjusted_by_im() {
        let no_counterparty_default = flat_hazard_curve(0.0);
        let own_hazard = flat_hazard_curve(0.03);
        let discount = flat_discount_curve(0.02);
        let profile = uniform_ene_profile(1_000_000.0, &[1.0, 2.0, 3.0]);

        let unilateral = compute_dva(&profile, &own_hazard, &discount, 0.40)
            .expect("unilateral DVA should compute");
        let no_im_bilateral = compute_bilateral_xva(
            &profile,
            &no_counterparty_default,
            &own_hazard,
            &discount,
            0.40,
            0.40,
            None,
        )
        .expect("bilateral XVA without IM")
        .dva
        .expect("DVA");

        let funding = FundingConfig {
            funding_spread_bp: 0.0,
            im_profile: Some(ImProfile {
                times: vec![1.0, 3.0],
                im_values: vec![1_000_000.0, 1_000_000.0],
            }),
            ..Default::default()
        };
        let im_adjusted_bilateral = compute_bilateral_xva(
            &profile,
            &no_counterparty_default,
            &own_hazard,
            &discount,
            0.40,
            0.40,
            Some(&funding),
        )
        .expect("bilateral XVA with IM")
        .dva
        .expect("DVA");

        assert!(unilateral > 0.0);
        assert!(
            (unilateral - no_im_bilateral).abs() < 1.0e-12,
            "with no counterparty default, unadjusted bilateral DVA must equal public unilateral DVA"
        );
        assert!(
            im_adjusted_bilateral.abs() < 1.0e-12,
            "posted IM must affect bilateral DVA only"
        );
    }

    #[test]
    fn xva_result_requires_total_and_allows_optional_mva() {
        let minimal = r#"{
            "cva": 100.0,
            "total_xva": 100.0,
            "epe_profile": [[1.0, 10.0]],
            "ene_profile": [[1.0, 0.0]],
            "pfe_profile": [[1.0, 10.0]],
            "max_pfe": 10.0,
            "effective_epe_profile": [[1.0, 10.0]],
            "effective_epe": 10.0
        }"#;
        let parsed: XvaResult =
            serde_json::from_str(minimal).expect("minimal payload should parse");
        assert!(parsed.mva.is_none());
        assert_eq!(parsed.total_xva, 100.0);

        // And unset optionals stay off the wire.
        let json = serde_json::to_string(&parsed).expect("serialize");
        assert!(!json.contains("mva"), "unset mva should be skipped: {json}");
        assert!(json.contains("total_xva"), "total_xva is required: {json}");

        let missing_total = minimal.replace("\n            \"total_xva\": 100.0,", "");
        assert!(
            serde_json::from_str::<XvaResult>(&missing_total).is_err(),
            "total_xva must be required"
        );

        // schema-rejection-test: the superseded aggregate key must stay invalid.
        let retired_bilateral = minimal.replace(
            "\n            \"total_xva\": 100.0,",
            "\n            \"bilateral_cva\": 100.0,\n            \"total_xva\": 100.0,",
        );
        assert!(
            serde_json::from_str::<XvaResult>(&retired_bilateral).is_err(),
            "the retired bilateral_cva key must be rejected"
        );
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
            funding_spread_bp: 50.0,
            funding_benefit_bp: None, // defaults to 50.0
            ..Default::default()
        };

        let funding_explicit = FundingConfig {
            funding_spread_bp: 50.0,
            funding_benefit_bp: Some(50.0),
            ..Default::default()
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
            funding_spread_bp: 60.0,
            funding_benefit_bp: Some(40.0),
            ..Default::default()
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
    fn unilateral_xva_omits_uncomputed_legs_and_sets_total_to_cva() {
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
        assert_eq!(result.total_xva, result.cva);
    }

    #[test]
    fn xva_result_serde_roundtrip_with_optional_fields() {
        // Ensure optional legs and the required total survive round-tripping.
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
        assert_eq!(deserialized.total_xva, result.total_xva);
    }
}
