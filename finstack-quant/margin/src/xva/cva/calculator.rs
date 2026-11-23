//! CVA, DVA, FVA, and bilateral XVA integration routines.

use finstack_quant_core::market_data::term_structures::{DiscountCurve, HazardCurve};
use finstack_quant_core::math::summation::NeumaierAccumulator;

use super::super::mva;
use super::super::types::{CreditXvaRow, ExposureProfile, FundingConfig, FundingXvaRow, XvaResult};

pub(super) struct SampledCurves {
    discount: Vec<f64>,
    counterparty_survival: Vec<f64>,
    own_survival: Vec<f64>,
}

impl SampledCurves {
    fn new(
        profile: &ExposureProfile,
        discount_curve: &DiscountCurve,
        counterparty_hazard: Option<&HazardCurve>,
        own_hazard: Option<&HazardCurve>,
    ) -> finstack_quant_core::Result<Self> {
        Ok(Self {
            counterparty_survival: super::super::dated::survival_factors(
                profile.as_of,
                profile.currency,
                &profile.dates,
                counterparty_hazard,
            )?,
            own_survival: super::super::dated::survival_factors(
                profile.as_of,
                profile.currency,
                &profile.dates,
                own_hazard,
            )?,
            discount: super::super::dated::discount_factors(
                profile.as_of,
                profile.currency,
                &profile.dates,
                discount_curve,
            )?,
        })
    }
}

pub(super) fn validate_recovery_rate(
    label: &str,
    field: &str,
    rate: f64,
) -> finstack_quant_core::Result<()> {
    if !(0.0..=1.0).contains(&rate) {
        return Err(finstack_quant_core::Error::Validation(format!(
            "{label}: {field} {rate} must be in [0, 1]"
        )));
    }
    Ok(())
}

pub(super) fn integrate_credit_leg(
    times: &[f64],
    exposure: &[f64],
    lgd: f64,
    df: &[f64],
    s_default: &[f64],
    s_other: &[f64],
) -> finstack_quant_core::Result<(f64, Vec<CreditXvaRow>)> {
    let n = times.len();
    let mut total = NeumaierAccumulator::new();
    let mut rows = Vec::with_capacity(n);
    let mut prev_survival = 1.0;
    let mut prev_other = 1.0;
    let mut prev_exp = exposure[0];
    let mut prev_df: f64 = 1.0;

    for i in 0..n {
        let t = times[i];
        let exp_t = exposure[i];
        let survival_t = s_default[i];
        let other_t = s_other[i];
        let df_t = df[i];

        let marginal_pd = prev_survival - survival_t;
        let exp_mid = 0.5 * prev_exp + 0.5 * exp_t;
        let df_mid = 0.5 * prev_df + 0.5 * df_t;
        let other_mid = 0.5 * prev_other + 0.5 * other_t;

        let contribution = lgd * exp_mid * marginal_pd * df_mid * other_mid;
        total.add(contribution)?;
        rows.push(CreditXvaRow {
            time: t,
            discount_factor: df_t,
            survival_probability: survival_t,
            marginal_default_probability: marginal_pd,
            exposure_mid: exp_mid,
            discount_factor_mid: df_mid,
            survival_weight: other_mid,
            loss_given_default: lgd,
            contribution,
        });

        prev_survival = survival_t;
        prev_other = other_t;
        prev_exp = exp_t;
        prev_df = df_t;
    }

    Ok((total.get_total()?, rows))
}

pub(super) fn compute_cva_from_sampled(
    exposure_profile: &ExposureProfile,
    sampled: &SampledCurves,
    recovery_rate: f64,
) -> finstack_quant_core::Result<XvaResult> {
    let n = exposure_profile.times.len();
    let (cva, cva_rows) = integrate_credit_leg(
        &exposure_profile.times,
        &exposure_profile.epe,
        1.0 - recovery_rate,
        &sampled.discount,
        &sampled.counterparty_survival,
        &sampled.own_survival,
    )?;

    let mut epe_profile = Vec::with_capacity(n);
    let mut ene_profile = Vec::with_capacity(n);
    let mut effective_epe_profile = Vec::with_capacity(n);
    let mut max_epe: f64 = 0.0;
    let mut effective_epe_running: f64 = 0.0;
    let mut eff_epe_time_integral: f64 = 0.0;
    let effective_epe_horizon = exposure_profile.times[n - 1].min(1.0);
    let mut prev_t: f64 = 0.0;

    for i in 0..n {
        let t = exposure_profile.times[i];
        let epe_t = exposure_profile.epe[i];
        let ene_t = exposure_profile.ene[i];

        effective_epe_running = effective_epe_running.max(epe_t);
        if prev_t < effective_epe_horizon {
            let dt = (t.min(effective_epe_horizon) - prev_t).max(0.0);
            eff_epe_time_integral += effective_epe_running * dt;
        }
        max_epe = max_epe.max(epe_t);

        epe_profile.push((t, epe_t));
        ene_profile.push((t, ene_t));
        effective_epe_profile.push((t, effective_epe_running));
        prev_t = t;
    }

    let effective_epe = if effective_epe_horizon > 0.0 {
        eff_epe_time_integral / effective_epe_horizon
    } else {
        effective_epe_running
    };

    Ok(XvaResult {
        cva,
        dva: None,
        fva: None,
        mva: None,
        mva_detail: None,
        total_xva: cva,
        epe_profile,
        ene_profile,
        max_epe,
        effective_epe_profile,
        effective_epe,
        cva_rows,
        dva_rows: None,
        fva_rows: None,
        meta: finstack_quant_core::config::results_meta(
            &finstack_quant_core::config::FinstackConfig::default(),
        ),
    })
}

pub(super) fn compute_fva_from_sampled(
    exposure_profile: &ExposureProfile,
    sampled: &SampledCurves,
    funding_spread_bp: f64,
    funding_benefit_bp: f64,
) -> finstack_quant_core::Result<(f64, Vec<FundingXvaRow>)> {
    let n = exposure_profile.times.len();
    let spread_cost = funding_spread_bp / 10_000.0;
    let spread_benefit = funding_benefit_bp / 10_000.0;

    let mut fva = NeumaierAccumulator::new();
    let mut rows = Vec::with_capacity(n);
    let mut prev_counterparty_survival = 1.0;
    let mut prev_own_survival = 1.0;
    let mut prev_epe = exposure_profile.epe[0];
    let mut prev_ene = exposure_profile.ene[0];
    let mut prev_df: f64 = 1.0;
    let mut prev_t: f64 = 0.0;

    for i in 0..n {
        let t = exposure_profile.times[i];
        let epe_t = exposure_profile.epe[i];
        let ene_t = exposure_profile.ene[i];

        let counterparty_survival_t = sampled.counterparty_survival[i];
        let own_survival_t = sampled.own_survival[i];
        let df_t = sampled.discount[i];

        let dt = t - prev_t;
        let epe_mid = 0.5 * prev_epe + 0.5 * epe_t;
        let ene_mid = 0.5 * prev_ene + 0.5 * ene_t;
        let df_mid = 0.5 * prev_df + 0.5 * df_t;
        let counterparty_survival_mid =
            0.5 * prev_counterparty_survival + 0.5 * counterparty_survival_t;
        let own_survival_mid = 0.5 * prev_own_survival + 0.5 * own_survival_t;
        let joint_survival_mid = counterparty_survival_mid * own_survival_mid;

        let contribution =
            (epe_mid * spread_cost - ene_mid * spread_benefit) * df_mid * dt * joint_survival_mid;
        fva.add(contribution)?;
        rows.push(FundingXvaRow {
            time: t,
            dt,
            discount_factor: df_t,
            epe_mid,
            ene_mid,
            discount_factor_mid: df_mid,
            joint_survival_mid,
            funding_spread: spread_cost,
            funding_benefit_spread: spread_benefit,
            contribution,
        });

        prev_counterparty_survival = counterparty_survival_t;
        prev_own_survival = own_survival_t;
        prev_epe = epe_t;
        prev_ene = ene_t;
        prev_df = df_t;
        prev_t = t;
    }

    Ok((fva.get_total()?, rows))
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
/// - A discount or survival curve evaluation fails at an exposure time
///
/// # Examples
///
/// ```
/// use finstack_quant_margin::xva::cva::compute_cva;
/// use finstack_quant_margin::xva::types::ExposureProfile;
/// use finstack_quant_core::dates::DayCount;
/// use finstack_quant_core::market_data::term_structures::{DiscountCurve, HazardCurve};
/// use finstack_quant_core::math::interp::{ExtrapolationPolicy, InterpStyle};
/// use time::macros::date;
///
/// # fn main() -> finstack_quant_core::Result<()> {
/// let as_of = date!(2025 - 01 - 01);
///
/// // Flat 2% hazard and 4% discounting out to five years.
/// let hazard_curve = HazardCurve::builder("ACME")
///     .base_date(as_of)
///     .day_count(DayCount::Act365F)
///     .recovery_rate(0.40)
///     .knots([(1.0, 0.02), (5.0, 0.02)])
///     .build()?;
/// let discount_curve = DiscountCurve::builder("USD-OIS")
///     .base_date(as_of)
///     .day_count(DayCount::Act365F)
///     .knots([(0.0, 1.0), (5.0, (-0.04f64 * 5.0).exp())])
///     .interp(InterpStyle::Linear)
///     .extrapolation(ExtrapolationPolicy::Extend)
///     .build()?;
///
/// let profile = ExposureProfile {
///     as_of,
///     currency: finstack_quant_core::currency::Currency::USD,
///     basis: finstack_quant_margin::xva::types::ExposureBasis::External {
///         source_id: "verified-path-mean".into(),
///         policy_id: "netting-set-A-unsecured".into(),
///     },
///     dates: vec![as_of, time::macros::date!(2026-01-01), time::macros::date!(2027-01-01)],
///     times: vec![0.0, 1.0, 2.0],
///     mtm_values: vec![1_000_000.0; 3],
///     epe: vec![1_000_000.0; 3],
///     ene: vec![0.0; 3],
///     diagnostics: finstack_quant_margin::xva::types::ExposureDiagnostics {
///         total_time_points: 3,
///         ..Default::default()
///     },
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
    exposure_profile.validate()?;
    validate_recovery_rate("CVA", "recovery_rate", recovery_rate)?;
    let sampled = SampledCurves::new(
        exposure_profile,
        discount_curve,
        Some(counterparty_hazard_curve),
        None,
    )?;
    compute_cva_from_sampled(exposure_profile, &sampled, recovery_rate)
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
/// - A discount or survival curve evaluation fails at an exposure time
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
    exposure_profile.validate()?;
    validate_recovery_rate("DVA", "own_recovery_rate", own_recovery_rate)?;
    let sampled = SampledCurves::new(
        exposure_profile,
        discount_curve,
        None,
        Some(own_hazard_curve),
    )?;
    let (dva, _) = integrate_credit_leg(
        &exposure_profile.times,
        &exposure_profile.ene,
        1.0 - own_recovery_rate,
        &sampled.discount,
        &sampled.own_survival,
        &sampled.counterparty_survival,
    )?;
    Ok(dva)
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
/// - A discount or survival curve evaluation fails at an exposure time
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
    exposure_profile.validate()?;
    let sampled = SampledCurves::new(exposure_profile, discount_curve, None, None)?;
    let (fva, _) = compute_fva_from_sampled(
        exposure_profile,
        &sampled,
        funding_spread_bp,
        funding_benefit_bp,
    )?;
    Ok(fva)
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
    exposure_profile.validate()?;
    validate_recovery_rate("CVA", "recovery_rate", counterparty_recovery_rate)?;
    let sampled = SampledCurves::new(
        exposure_profile,
        discount_curve,
        Some(counterparty_hazard_curve),
        Some(own_hazard_curve),
    )?;

    let mut result =
        compute_cva_from_sampled(exposure_profile, &sampled, counterparty_recovery_rate)?;

    if let Some(im_profile) = funding.and_then(|fc| fc.im_profile.as_ref())
        && (im_profile.as_of != exposure_profile.as_of
            || im_profile.currency != exposure_profile.currency
            || im_profile.dates.last() != exposure_profile.dates.last())
    {
        return Err(finstack_quant_core::Error::Validation(
            "Bilateral XVA: IM and exposure profiles must share the valuation cut, currency and horizon".into(),
        ));
    }

    validate_recovery_rate("DVA", "own_recovery_rate", own_recovery_rate)?;
    let (dva, dva_rows) = integrate_credit_leg(
        &exposure_profile.times,
        &exposure_profile.ene,
        1.0 - own_recovery_rate,
        &sampled.discount,
        &sampled.own_survival,
        &sampled.counterparty_survival,
    )?;
    result.dva = Some(dva);
    result.dva_rows = Some(dva_rows);

    let fva = if let Some(fc) = funding {
        let (fva_val, fva_rows) = compute_fva_from_sampled(
            exposure_profile,
            &sampled,
            fc.funding_spread_bp,
            fc.effective_benefit_bp(),
        )?;
        result.fva = Some(fva_val);
        result.fva_rows = Some(fva_rows);
        fva_val
    } else {
        result.fva = None;
        0.0
    };

    let mva = match funding.and_then(|fc| fc.im_profile.as_ref().map(|im| (fc, im))) {
        Some((fc, im_profile)) => {
            let detail = mva::compute_mva_internal(
                im_profile,
                fc.margin_funding_spread.as_ref().ok_or_else(|| {
                    finstack_quant_core::Error::Validation(
                        "Bilateral XVA requires an explicit posted-margin funding spread".into(),
                    )
                })?,
                discount_curve,
                Some(own_hazard_curve),
                Some(counterparty_hazard_curve),
            )?;
            let mva_value = detail.mva;
            result.mva = Some(mva_value);
            result.mva_detail = Some(detail);
            mva_value
        }
        None => {
            result.mva = None;
            0.0
        }
    };

    let mut total = NeumaierAccumulator::new();
    for leg in [result.cva, -dva, fva, mva] {
        total.add(leg)?;
    }
    result.total_xva = total.get_total()?;

    Ok(result)
}
