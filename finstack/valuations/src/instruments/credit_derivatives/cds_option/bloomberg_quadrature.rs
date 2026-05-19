//! Bloomberg CDSO numerical-quadrature pricer.
//!
//! Implements the model published in:
//!
//! - Bloomberg L.P. Quantitative Analytics. *Pricing Credit Index Options.*
//!   DOCS 2055833 ⟨GO⟩, March 2012.
//!
//! and uses the Bloomberg CDS pricer (DOCS 2057273) for the bootstrapped
//! `F_0` calibration target.
//!
//! # Model summary (DOCS 2055833 §2.2, Eqs. 2.2–2.5)
//!
//! ```text
//! S_te    = m · exp(−½σ²t_e + σ·√t_e · ε),     ε ~ N(0,1)
//! V_te    = ξ N (S_te − c) · L_te(S_te)
//! H(K)    = ξ N (c − K) · A(K)
//! D       = ξ N₀ · loss(t_v)
//! O       = P(t_e) · E_0 [ (V_te + H(K) + D)+ ]
//! F_0     = E_0 [V_te]                          (calibration anchor)
//! ```
//!
//! - `S_te` is the (random) realised forward CDS spread at expiry.
//! - `L_te(S)` is the *flat-spread* forward risky annuity at hazard
//!   `λ = S/(1−R)` over `[t_e, t_M]` — the "credit triangle" simplification
//!   §2.5: continuous coupon, constant rate to expiry → analytic `λ(S)`.
//! - `A(K)` is the same flat-spread annuity evaluated at `S = K`.
//! - `m` is calibrated so `F_0` matches the bootstrapped clean forward swap
//!   value.
//! - `ξ = +1` for payer (call), `ξ = −1` for receiver (put).
//! - For index CDS options, expected front-end protection is represented as
//!   part of the deterministic exercise payoff `D`, not as an extra term in
//!   `F_0`.
//!
//! # Numerical integration
//!
//! Trapezoidal rule on the standard normal density over `z ∈ [−6, 6]` with
//! step `Δz = 0.05`. The integrand is smooth (lognormal × piecewise-linear
//! in `(s−c)L(s)`) so 240 quadrature nodes give 1e-9 absolute precision —
//! well below the precision the calibration achieves.
//!
//! All time inputs use **calendar days / 365** (DOCS 2055833 §2.1, matching
//! FinancePy's `G_DAYS_IN_YEAR = 365.0`). Premium-leg accrual factors come
//! from the synthetic underlying CDS in its native day count (Act/360 for
//! USD CDX/iTraxx Main).

use crate::constants::{numerical, BASIS_POINTS_PER_UNIT};
use crate::instruments::common_impl::parameters::OptionType;
use crate::instruments::credit_derivatives::cds::pricer::CDSPricer;
use crate::instruments::credit_derivatives::cds::CreditDefaultSwap;
use crate::instruments::credit_derivatives::cds_option::CDSOption;
use finstack_core::dates::{adjust, BusinessDayConvention, CalendarRegistry, Date, DateExt};
use finstack_core::market_data::context::MarketContext;
use finstack_core::market_data::term_structures::{DiscountCurve, HazardCurve};
use finstack_core::math::solver::BrentSolver;
use finstack_core::money::Money;
use finstack_core::Result;
use rust_decimal::prelude::ToPrimitive;
use rust_decimal::Decimal;

/// Bloomberg CDSO calendar-day denominator (DOCS 2055833 §2.1).
const G_DAYS_IN_YEAR: f64 = 365.0;

/// Bloomberg CDSO theta day basis (DOCS 2055833 §2.5: *"shortening the
/// exercise time `t_e` by `1/365.25`"*). The 0.25-day inconsistency with
/// `G_DAYS_IN_YEAR` is faithful to the published convention; the regression
/// test [`tests::theta_uses_pure_t_shift_with_365_25_denominator`] pins the
/// formulation against accidental rewrites.
const THETA_DAYS_IN_YEAR: f64 = 365.25;

/// Minimum standard-normal quadrature range — covers `±6σ` of the *driver*
/// `ε`, adequate when the payoff weight does not shift the integration mass.
const MIN_Z_LIMIT: f64 = 6.0;
const Z_STEP: f64 = 0.05;

/// CDX option front-end-protection start lag observed on Bloomberg CDSO.
///
/// For `cdx_ig_46_payer_atm_jun26`, measuring expected FEP from T+2 business
/// days to legal expiry reproduces the Bloomberg Market Value to sub-dollar
/// precision. This is distinct from the premium cash-settlement date used in
/// Black time-to-expiry.
const INDEX_OPTION_FEP_START_LAG_BD: i32 = 2;

/// `1/√(2π)` — the standard normal density's normalising constant.
const INV_SQRT_2_PI: f64 = 0.398_942_280_401_432_7_f64;

// =====================================================================
// Public entry points
// =====================================================================

/// Price a CDS option under the Bloomberg CDSO numerical-quadrature model.
pub(crate) fn npv(
    option: &CDSOption,
    cds: &CreditDefaultSwap,
    curves: &MarketContext,
    sigma: f64,
    as_of: Date,
) -> Result<Money> {
    let disc = curves.get_discount(&option.discount_curve_id)?;
    let surv = curves.get_hazard(&option.credit_curve_id)?;

    let ctx = ForwardCdsContext::build(option, disc.as_ref(), surv.as_ref(), cds, as_of, sigma)?;

    // Eq. 2.3: solve `m` so E[V_te(S_te(m))] matches the no-knockout F_0.
    let m = calibrate_lognormal_mean(&ctx)?;

    // Eq. 2.5: O = P(t_e) · E_0[(ξV_te + H(K) + D)+]
    let pv_per_n = price_with_calibrated_mean(&ctx, m, ctx.t_expiry.max(0.0));
    Ok(Money::new(
        pv_per_n * option.notional.amount(),
        option.notional.currency(),
    ))
}

/// Bloomberg CDSO theta: shorten the exercise time by 1/365.25 while
/// retaining the same calibrated forward price and lognormal mean.
pub(crate) fn theta(
    option: &CDSOption,
    cds: &CreditDefaultSwap,
    curves: &MarketContext,
    sigma: f64,
    as_of: Date,
) -> Result<f64> {
    let disc = curves.get_discount(&option.discount_curve_id)?;
    let surv = curves.get_hazard(&option.credit_curve_id)?;
    let ctx = ForwardCdsContext::build(option, disc.as_ref(), surv.as_ref(), cds, as_of, sigma)?;
    if ctx.t_expiry <= 0.0 {
        return Ok(0.0);
    }
    let m = calibrate_lognormal_mean(&ctx)?;
    let base = price_with_calibrated_mean(&ctx, m, ctx.t_expiry);
    let shortened_t = (ctx.t_expiry - (1.0 / THETA_DAYS_IN_YEAR)).max(0.0);
    let bumped = price_with_calibrated_mean(&ctx, m, shortened_t);
    Ok((bumped - base) * option.notional.amount())
}

/// Bloomberg CDSO ATM Forward (in basis points) — the bootstrapped forward
/// par spread of the no-knockout forward CDS at expiry.
pub(crate) fn forward_par_at_expiry_bp(
    option: &CDSOption,
    cds: &CreditDefaultSwap,
    curves: &MarketContext,
    as_of: Date,
) -> Result<f64> {
    let disc = curves.get_discount(&option.discount_curve_id)?;
    let surv = curves.get_hazard(&option.credit_curve_id)?;
    let ctx = ForwardCdsContext::build(option, disc.as_ref(), surv.as_ref(), cds, as_of, 0.0)?;
    Ok(ctx.display_forward_par_spread * BASIS_POINTS_PER_UNIT)
}

// =====================================================================
// Pre-computed deterministic inputs
// =====================================================================

/// All deterministic quantities the quadrature integrand and the
/// calibration target need. Built once at the top of `npv()` from the
/// instrument + curves; the calibration loop and the payoff loop both
/// borrow it.
struct ForwardCdsContext {
    /// `1 − R` on the synthetic underlying.
    lgd: f64,
    /// `t_e` in years (`(expiry − as_of)/365`).
    t_expiry: f64,
    /// `σ` from the vol surface or instrument override.
    sigma: f64,
    /// `df(t_e)` from valuation date to expiry, on the option's discount
    /// curve.
    df_to_expiry: f64,
    /// Conditional survival probability from valuation date to expiry on
    /// the bootstrapped credit curve.
    survival_to_expiry: f64,
    /// Bootstrapped forward par spread `s_par` (decimal), computed using
    /// the PCD-corrected annuity. Used as the calibration anchor for the
    /// clean forward F_0 = (par − c) · L_te (DOCS 2055833 §2.3).
    forward_par_spread: f64,
    /// Bloomberg HELP CDSO "ATM Fwd" display value (decimal): same
    /// `spot_protection_pv` numerator, but with the Bloomberg-screen
    /// drop-first-cashflow annuity in the denominator. Reported to the
    /// metrics framework via [`forward_par_at_expiry_bp`] but NOT used in
    /// the option NPV calibration.
    display_forward_par_spread: f64,
    /// Bootstrapped clean RPV01 of the forward CDS *expressed at expiry*
    /// (i.e. divided by `df_te · q_te`). Used in the F_0 calibration target.
    bootstrapped_l_at_expiry: f64,
    /// Year fractions from `t_e` to each post-expiry coupon payment date.
    /// `(payment_date − expiry) / 365`.
    times_from_expiry: Vec<f64>,
    /// Premium-leg accrual factors per coupon period in the synthetic
    /// CDS day-count (typically Act/360). One entry per post-expiry
    /// coupon payment.
    accrual_factors: Vec<f64>,
    /// Forward discount factors `df(t_pay) / df(t_e)`. One entry per
    /// post-expiry payment.
    fwd_discount_factors: Vec<f64>,
    /// Year fraction from the previous coupon date `T_{n(t_e)}` to `t_e`,
    /// in the synthetic CDS day-count. Subtracted from the dirty per-bp
    /// annuity to convert to clean. For forward CDSes whose premium starts
    /// at expiry this is zero.
    accrual_pcd_to_expiry: f64,
    /// Index contractual coupon `c` (decimal).
    coupon: f64,
    /// Option strike `K` (decimal).
    strike: f64,
    /// `ξ = +1` for payer (Call), `−1` for receiver (Put).
    option_type: OptionType,
    /// Index-factor scale (1.0 for non-index or original-version index
    /// underlyings).
    scale: f64,
    /// Realized index loss per unit of original notional.
    realized_index_loss: f64,
    /// Expected front-end protection per unit notional for index options,
    /// measured from the option FEP start date to legal expiry.
    front_end_protection: f64,
    /// True for index options. Drives `loss_settlement` (settlement of
    /// already-realised index losses and expected front-end protection on
    /// exercise).
    is_index: bool,
    /// Whether exercise is conditioned on underlying survival to expiry.
    knockout: bool,
}

impl ForwardCdsContext {
    fn build(
        option: &CDSOption,
        disc: &DiscountCurve,
        surv: &HazardCurve,
        cds: &CreditDefaultSwap,
        as_of: Date,
        sigma: f64,
    ) -> Result<Self> {
        let cds_pricer = CDSPricer::new();
        // LGD must be strictly positive and finite; recovery is already
        // validated to `(0, 1)` at construction time, but guard against NaN /
        // future construction-path regressions with an explicit error rather
        // than a silent clamp. Checking `is_finite` first is necessary because
        // `NaN <= ZERO_TOLERANCE` is false; using a positive guard
        // (`!is_finite || <= tol`) makes the NaN case explicit.
        let lgd = 1.0 - option.recovery_rate;
        if !lgd.is_finite() || lgd <= numerical::ZERO_TOLERANCE {
            return Err(finstack_core::Error::Validation(format!(
                "CDS option recovery_rate={} yields degenerate LGD={:.3e}; \
                 expected recovery in (0, 1) so LGD > {}",
                option.recovery_rate,
                lgd,
                numerical::ZERO_TOLERANCE
            )));
        }

        let t_expiry = option.time_to_expiry(as_of)?;
        let df_to_expiry = DiscountCurve::df_between_dates(disc, as_of, option.expiry)?;
        let sp_asof_raw = surv.sp_on_date(as_of).unwrap_or(1.0);
        let sp_asof = sp_asof_raw.clamp(numerical::ZERO_TOLERANCE, 1.0);
        let sp_expiry_raw = surv.sp_on_date(option.expiry).unwrap_or(1.0);
        let sp_expiry = sp_expiry_raw.clamp(0.0, 1.0);
        // Survival probabilities should be monotonically non-increasing on a
        // valid hazard curve. A `sp(expiry) > sp(as_of)` is a curve-data
        // problem; log it rather than silently clamping to 1.0 so operators
        // can investigate upstream calibration without altering the pricing
        // path (clamp preserves numerical stability for downstream callers).
        let survival_to_expiry_raw = sp_expiry / sp_asof;
        if survival_to_expiry_raw > 1.0 + 1e-9 {
            tracing::warn!(
                target: "cds_option::bloomberg_quadrature",
                instrument_id = %option.id,
                expiry = %option.expiry,
                sp_asof = sp_asof_raw,
                sp_expiry = sp_expiry_raw,
                "hazard curve survival is non-monotonic: sp(expiry) > sp(as_of); \
                 conditional survival clamped to [0, 1] for option pricing"
            );
        }
        let survival_to_expiry = survival_to_expiry_raw.clamp(0.0, 1.0);

        // Premium-leg coupon schedule of the synthetic forward CDS,
        // restricted to payments strictly after expiry. We also build the
        // no-AoD post-expiry risky-annuity sum directly here — Bloomberg's
        // ATM Fwd "Premium Leg" (HELP CDSO) and the ISDA-standard
        // `risky_annuity` denominator (DOCS 2057273 Eq 3.3) are coupons-in-
        // survival only, *without* the accrual-on-default integral. Using
        // A forward premium-leg PV that includes AoD would systematically
        // overstate the annuity by the AoD contribution, which on cdx_ig_46
        // shifts ATM Fwd by ~0.1 bp.
        let cashflows = cds_pricer.premium_cashflow_accruals(cds, as_of)?;
        let mut times_from_expiry = Vec::with_capacity(cashflows.len());
        let mut accrual_factors = Vec::with_capacity(cashflows.len());
        let mut fwd_discount_factors = Vec::with_capacity(cashflows.len());
        let mut raw_annuity_at_value_dt_no_aod = 0.0_f64;
        let mut first_post_expiry_pv01_at_value_dt = 0.0_f64;
        let mut seen_first_post_expiry = false;
        for (pay_date, accrual) in cashflows.iter() {
            if *pay_date <= option.expiry {
                continue;
            }
            let t_e_to_pay = ((*pay_date - option.expiry).whole_days() as f64) / G_DAYS_IN_YEAR;
            let df_pay = DiscountCurve::df_between_dates(disc, as_of, *pay_date)?;
            if df_to_expiry < numerical::ZERO_TOLERANCE {
                return Err(finstack_core::Error::Validation(format!(
                    "degenerate forward discount factor at t_expiry: df_to_expiry={df_to_expiry:.3e}"
                )));
            }
            let fwd_df = df_pay / df_to_expiry;
            let sp_pay_uncond = surv.sp_on_date(*pay_date).unwrap_or(1.0).clamp(0.0, 1.0);
            let sp_pay_cond = (sp_pay_uncond / sp_asof).clamp(0.0, 1.0);
            times_from_expiry.push(t_e_to_pay);
            accrual_factors.push(*accrual);
            fwd_discount_factors.push(fwd_df);
            let cf_pv01 = *accrual * df_pay * sp_pay_cond;
            raw_annuity_at_value_dt_no_aod += cf_pv01;
            if !seen_first_post_expiry {
                first_post_expiry_pv01_at_value_dt = cf_pv01;
                seen_first_post_expiry = true;
            }
        }

        // Pre-expiry "previous coupon date" → expiry year-fraction. For a
        // forward CDS whose premium starts at expiry this is zero.
        let mut pcd: Option<Date> = None;
        for (pay_date, _) in cashflows.iter() {
            if *pay_date <= option.expiry {
                pcd = Some(*pay_date);
            } else {
                break;
            }
        }
        let pcd = pcd.unwrap_or_else(|| cds.premium.start.min(option.expiry));
        let accrual_pcd_to_expiry = if pcd >= option.expiry {
            0.0
        } else {
            finstack_core::dates::DayCount::year_fraction(
                cds.premium.day_count,
                pcd,
                option.expiry,
                finstack_core::dates::DayCountContext::default(),
            )
            .unwrap_or(0.0)
        };

        // Bootstrapped forward par spread and clean forward RPV01 — the
        // Bloomberg CDSO "ATM Fwd" computation, per the published Help
        // methodology (HELP CDSO <GO> "Calculating ATM Forward Spread for
        // CDSO"):
        //
        //   ATM Fwd = Default_Leg(0, T_mat) / Premium_Leg(T_exp, T_mat)
        //
        //   Default Leg: PV of expected loss from the **valuation date** to
        //   the underlying CDS maturity — i.e., the *spot* protection PV.
        //   Premium Leg: PV of a 1bp premium stream from [T_exp + 1, T_mat]
        //   on the underlying CDS schedule, **subtracting the PV01 of the
        //   first cashflow** (Bloomberg's verbatim wording — the first
        //   post-expiry coupon, i.e. the one whose accrual period straddles
        //   T_exp, is dropped in full).
        //
        // A plain post-expiry premium-leg sum includes the first coupon in full;
        // `pv_protection_leg` integrates from `max(as_of, protection_start)`
        // to maturity — when the synthetic CDS has `premium.start ≤ as_of`
        // and `protection_effective_date = None` (the spot configuration set
        // up by `synthetic_underlying_cds`), this is exactly the spot
        // Default_Leg(0, T_mat) Bloomberg's formula calls for.
        let denom_te = (df_to_expiry * survival_to_expiry).max(numerical::ZERO_TOLERANCE);
        // Bloomberg HELP CDSO ATM Fwd: "subtract the PV01 of the first
        // cashflow." Apply this rule only when there is a STRADDLING
        // first period (premium.start strictly before T_exp), in which
        // case dropping the full first cashflow PV01 is the BBG screen
        // formula. When `premium.start ≥ T_exp` (no straddle), the first
        // post-expiry cashflow is a stub starting at premium.start with
        // no pre-expiry component to net out, and dropping it would
        // distort the option's internal forward vs. the standard CDS
        // par_spread of the same underlying — so we leave the annuity
        // unchanged in that case (matching the legacy PCD behaviour
        // that the put/call-parity-at-forward invariant relies on).
        // For the calibration target F_0 = (par − c) · L_te and the
        // quadrature integrand V_te(s) = (s − c) · L_te(s), we use the
        // economically-meaningful "PCD subtraction" — the pre-expiry
        // portion of the period straddling expiry. This preserves
        // put/call parity at ATF for forward CDSes whose schedule starts
        // at T_exp (no straddle ⇒ no subtraction), and gives a
        // self-consistent calibration.
        let pcd_stub_at_value_dt = accrual_pcd_to_expiry * denom_te;
        let risky_annuity_at_value_dt = raw_annuity_at_value_dt_no_aod - pcd_stub_at_value_dt;
        let bootstrapped_l_at_expiry = risky_annuity_at_value_dt / denom_te;

        // Bloomberg HELP CDSO ATM Fwd display formula: "Premium Leg = PV
        // of 1bp stream from [T_exp+1, T_mat], subtracting the PV01 of
        // the first cashflow." When the synthetic CDS schedule has a
        // straddling first period (premium.start strictly < T_exp), this
        // is the literal full-first-cashflow drop. When there's no
        // straddle, the Bloomberg formula degenerates to the standard
        // post-expiry sum (the "first cashflow" is then a thin stub
        // already starting at T_exp). The displayed `forward_par_spread`
        // uses this annuity; the calibration/integrand use the PCD
        // version above. Decoupling display from math is necessary
        // because the drop-first formula moves par by ~0.16 bp on
        // cdx_ig_46 while the calibration would overshoot if it tracked
        // the same shift.
        let drop_first_cashflow = cds.premium.start < option.expiry;
        let display_annuity_at_value_dt = if drop_first_cashflow {
            raw_annuity_at_value_dt_no_aod - first_post_expiry_pv01_at_value_dt
        } else {
            risky_annuity_at_value_dt
        };

        // Bloomberg DOCS 2057273 §3 protection-leg convention:
        // "Protection starts immediately, therefore the full number of days
        // for protection and coupon is (TM − T + 1)." We honour the +1-day
        // inclusive end of protection here (scoped to the option pricer) by
        // building a temporary CDS with `premium.end + 1 day` and computing
        // protection on that. We don't change `pv_protection_leg` globally
        // because non-forward-CDS pricing in finstack assumes the standard
        // [T, TM] integration; the +1-day rule is a CDSO-specific tightening
        // that closes ~0.05 bp of the cdx_ig_46 ATM Fwd residual.
        let spot_cds_plus_one = super::pricer::cds_with_bloomberg_protection_end_extension(cds);
        let spot_protection_pv = cds_pricer
            .pv_protection_leg(&spot_cds_plus_one, disc, surv, as_of)?
            .amount();
        // ECONOMIC forward par — used for calibration target F_0 = h1 + (par − c) · L_te.
        // Uses the same PCD-corrected annuity as `bootstrapped_l_at_expiry` so the
        // calibration is internally self-consistent.
        //
        // A non-positive annuity is a degenerate curve / schedule and would
        // silently inflate the par spread by `1 / ZERO_TOLERANCE` if clamped;
        // surface it as an error so callers see the underlying problem.
        let economic_denom_par_raw = risky_annuity_at_value_dt * cds.notional.amount();
        if economic_denom_par_raw <= numerical::ZERO_TOLERANCE {
            return Err(finstack_core::Error::Validation(format!(
                "degenerate economic risky annuity for CDS option '{}' par-spread \
                 denominator: annuity={:.6e}, notional={}; cannot compute F_0 \
                 calibration anchor",
                option.id,
                risky_annuity_at_value_dt,
                cds.notional.amount()
            )));
        }
        let forward_par_spread = spot_protection_pv / economic_denom_par_raw;
        // DISPLAY-ONLY par (Bloomberg HELP CDSO ATM Fwd formula). Reported via
        // `forward_par_at_expiry_bp` for the par_spread metric. Differs from
        // the economic par when the synthetic CDS schedule has a straddling
        // first period (premium.start strictly < T_exp); decoupling it from
        // the calibration anchor lets us reproduce Bloomberg's screen ATM
        // Fwd to within 0.05 bp without inducing a calibration shift in the
        // option NPV path.
        let display_denom_par_raw = display_annuity_at_value_dt * cds.notional.amount();
        if display_denom_par_raw <= numerical::ZERO_TOLERANCE {
            return Err(finstack_core::Error::Validation(format!(
                "degenerate display risky annuity for CDS option '{}' par-spread \
                 denominator: annuity={:.6e}, notional={}; cannot compute display \
                 ATM-forward",
                option.id,
                display_annuity_at_value_dt,
                cds.notional.amount()
            )));
        }
        let display_forward_par_spread = spot_protection_pv / display_denom_par_raw;

        let coupon = decimal_to_f64(option.effective_underlying_cds_coupon())?;
        let strike = decimal_to_f64(option.strike)?;
        let scale = if option.underlying_is_index {
            option.index_factor.unwrap_or(1.0)
        } else {
            1.0
        };
        let realized_index_loss = option.realized_index_loss.unwrap_or(0.0);
        let front_end_protection = if option.underlying_is_index && !option.knockout {
            let fep_start = index_option_front_end_protection_start(option, as_of)?;
            if fep_start >= option.expiry {
                0.0
            } else {
                let sp_start = surv
                    .sp_on_date(fep_start)
                    .unwrap_or(1.0)
                    .clamp(numerical::ZERO_TOLERANCE, 1.0);
                let sp_end = surv
                    .sp_on_date(option.expiry)
                    .unwrap_or(1.0)
                    .clamp(0.0, 1.0);
                lgd * (1.0 - (sp_end / sp_start).clamp(0.0, 1.0))
            }
        } else {
            0.0
        };

        Ok(Self {
            lgd,
            t_expiry,
            sigma,
            df_to_expiry,
            survival_to_expiry,
            forward_par_spread,
            display_forward_par_spread,
            bootstrapped_l_at_expiry,
            times_from_expiry,
            accrual_factors,
            fwd_discount_factors,
            accrual_pcd_to_expiry,
            coupon,
            strike,
            option_type: option.option_type,
            scale,
            realized_index_loss,
            front_end_protection,
            is_index: option.underlying_is_index,
            knockout: option.knockout,
        })
    }

    /// `ξ` per Eq. 2.1 / Eq. 2.4: `+1` payer, `−1` receiver.
    fn sign(&self) -> f64 {
        match self.option_type {
            OptionType::Call => 1.0,
            OptionType::Put => -1.0,
        }
    }

    /// Forward clean risky annuity *at expiry* under a flat hazard
    /// `λ = s / (1−R)`, evaluated on the synthetic CDS schedule:
    ///
    /// ```text
    /// L(s) = Σ_i α_i · exp(−λ · t_i_from_expiry) · fwd_df_i  −  α_pcd→te
    /// ```
    ///
    /// — the "credit triangle" simplification (DOCS 2055833 §2.5) lets us
    /// identify hazard with `s/(1−R)` directly so no per-node solve is
    /// needed inside the quadrature integrand. The PCD subtraction
    /// (`α_pcd→te`) corresponds to Bloomberg's "subtract the PV01 of the
    /// first cashflow" rule (HELP CDSO) reduced to its economically
    /// well-defined form: the option holder owes premium only from
    /// `T_e + 1` onward, so the pre-expiry portion of the period
    /// straddling expiry is netted out. For schedules where
    /// `premium.start = T_e` (no straddle), `accrual_pcd_to_expiry = 0`
    /// and this reduces to the raw post-expiry sum.
    fn flat_annuity(&self, s: f64) -> f64 {
        let lambda = s / self.lgd;
        let mut acc = -self.accrual_pcd_to_expiry;
        for ((alpha, t), fwd_df) in self
            .accrual_factors
            .iter()
            .zip(self.times_from_expiry.iter())
            .zip(self.fwd_discount_factors.iter())
        {
            let surv = (-lambda * t).exp();
            acc += alpha * surv * fwd_df;
        }
        acc
    }

    /// Per-unit-notional swap value at expiry under flat-spread `s`:
    /// `V_te(s)/N = (s − c) · L(s)`.
    fn swap_value_per_n(&self, s: f64) -> f64 {
        (s - self.coupon) * self.flat_annuity(s)
    }

    /// Eq. 2.4 deterministic strike adjustment, per unit notional.
    fn strike_adjustment_per_n(&self) -> f64 {
        self.sign() * (self.coupon - self.strike) * self.flat_annuity(self.strike)
    }

    /// Eq. 2.5 deterministic loss settlement, per unit current notional
    /// before the index-factor scale is applied.
    fn loss_settlement_per_n(&self) -> f64 {
        if !self.is_index {
            return 0.0;
        }
        let scale = self.scale.max(numerical::ZERO_TOLERANCE);
        self.sign() * (self.realized_index_loss / scale + self.front_end_protection)
    }

    /// Knockout options exercise only if the underlying survives to expiry.
    fn exercise_survival_multiplier(&self) -> f64 {
        if self.knockout {
            self.survival_to_expiry
        } else {
            1.0
        }
    }

    /// `F_0/N` — the clean forward swap value, used as the calibration anchor
    /// for `m` (DOCS 2055833 Eq 2.3).
    ///
    /// The calibration anchor is `(s_par − c) · L_te`. Index front-end
    /// protection enters [`Self::loss_settlement_per_n`] as an exercise
    /// payoff term, consistent with index CDS option mechanics.
    ///
    /// Note on L_te self-consistency: the integrand `V_te(s) = (s − c)·L(s)`
    /// uses the credit-triangle `L(s)` per DOCS 2055833 §2.5 (λ(s) =
    /// s/(1−R)), while F_0 uses the bootstrapped term-structure `L_te`.
    /// On cdx_ig_46 these differ by ~0.66% at par. Empirically, the
    /// bootstrap-anchored F_0 plus the index FEP payoff convention matches
    /// Bloomberg Market Value to sub-dollar precision, while calibrating
    /// against the credit-triangle F_0 overshoots materially.
    fn no_knockout_forward(&self) -> f64 {
        (self.forward_par_spread - self.coupon) * self.bootstrapped_l_at_expiry
    }
}

// =====================================================================
// Calibration of the lognormal mean `m` (DOCS 2055833 Eq. 2.3)
// =====================================================================

/// Solve the scalar nonlinear equation
///
/// ```text
/// E_0 [V_te(S_te(m))] = F_0
/// ```
///
/// where `S_te(m, ε) = m · exp(−½σ²t + σ√t·ε)`, `ε ∼ N(0, 1)`. Brent
/// root-finding in log-`m` space (positivity is enforced and the search is
/// well-conditioned across the realistic spread range).
///
/// Bracketing strategy: the calibrated `m` is mathematically very close to
/// the bootstrapped forward par spread `s_par` (their gap is `O(σ²t)` from
/// Itô plus a curvature-of-V_te correction). We therefore seed the bracket
/// at `s_par` and expand multiplicatively (doubling outward) until a sign
/// change is found. Empirically 4–10 quadrature evaluations are needed
/// versus the prior 200-step linear scan (~48k evals per NPV).
fn calibrate_lognormal_mean(ctx: &ForwardCdsContext) -> Result<f64> {
    let target = ctx.no_knockout_forward();
    let t_expiry = ctx.t_expiry.max(0.0);
    let s0 = (-0.5 * ctx.sigma * ctx.sigma * t_expiry).exp();
    let sigma_sqrt_t = ctx.sigma * t_expiry.sqrt();

    let expected_v_te = |m: f64| -> f64 {
        normal_integral(Z_STEP, z_limit(ctx.sigma, ctx.t_expiry), |z| {
            let s = m * s0 * (sigma_sqrt_t * z).exp();
            ctx.swap_value_per_n(s)
        })
    };

    let f = |log_m: f64| -> f64 { expected_v_te(log_m.exp()) - target };

    // Hard bounds (same as before): `m ∈ [1e-8, 100]` covers spreads from
    // sub-bp to fully distressed; we never let the bracket escape these.
    const LOG_M_LO: f64 = -18.420_680_743_952_367; // ln(1e-8)
    const LOG_M_HI: f64 = 4.605_170_185_988_092; // ln(100)
    const MAX_EXPANSIONS: usize = 30;

    // Seed near the bootstrapped forward par spread (positive by
    // construction; clamp into [1e-8, 100] (== [exp(LOG_M_LO), exp(LOG_M_HI)])
    // to be safe for degenerate inputs).
    let m_seed = ctx.forward_par_spread.clamp(1e-8, 100.0);
    let log_seed = m_seed.ln().clamp(LOG_M_LO, LOG_M_HI);
    let f_seed = f(log_seed);
    if f_seed == 0.0 {
        return Ok(log_seed.exp());
    }

    // Multiplicative bracket expansion: walk outward in log-space by
    // factors of 2 each side until we find a sign change.
    //
    // `V_te(s) = (s − c) · L(s)` is NOT strictly monotonic in `s` under
    // credit-triangle hazard: `dV/ds = L(s) + (s − c) · L'(s)` reaches a
    // maximum near `s ≈ c + LGD / t̄` (~12% spread at 60% LGD, t̄ ≈ 5y),
    // beyond which it decreases. `E[V_te(S_te(m))]` inherits this
    // curvature. The bracket-expansion is safe in practice because the
    // seed `m_seed ≈ forward_par_spread` sits well on the increasing
    // (low-spread) side of the peak for all realistic credits, so the
    // first opposite-signed `f` encountered while widening outward is
    // the correct root. Distressed inputs (`forward_par_spread > ~10%`)
    // would invalidate this and should be flagged upstream.
    let (mut lo_x, mut lo_f) = (log_seed, f_seed);
    let (mut hi_x, mut hi_f) = (log_seed, f_seed);
    let mut bracket: Option<(f64, f64)> = None;
    let step = (2.0_f64).ln(); // one doubling per expansion
    for k in 1..=MAX_EXPANSIONS {
        let widen = step * (k as f64);
        let x_lo_new = (log_seed - widen).max(LOG_M_LO);
        let x_hi_new = (log_seed + widen).min(LOG_M_HI);
        if x_lo_new < lo_x {
            let f_new = f(x_lo_new);
            if f_new.is_finite() && f_new * lo_f <= 0.0 {
                bracket = Some((x_lo_new, lo_x));
                break;
            }
            lo_x = x_lo_new;
            lo_f = f_new;
        }
        if x_hi_new > hi_x {
            let f_new = f(x_hi_new);
            if f_new.is_finite() && f_new * hi_f <= 0.0 {
                bracket = Some((hi_x, x_hi_new));
                break;
            }
            hi_x = x_hi_new;
            hi_f = f_new;
        }
        if x_lo_new <= LOG_M_LO && x_hi_new >= LOG_M_HI {
            break; // Hit hard bounds on both sides — no bracket exists.
        }
    }

    let Some((bracket_lo, bracket_hi)) = bracket else {
        return Err(finstack_core::Error::Validation(format!(
            "calibration bracket violation: target={target}, seed={m_seed}, \
             f(m_min)={lo_f:.6e}, f(m_max)={hi_f:.6e}",
        )));
    };
    let solver = BrentSolver::new().tolerance(1e-12);
    let log_m = solver.solve_in_bracket(f, bracket_lo, bracket_hi)?;
    Ok(log_m.exp())
}

// =====================================================================
// Quadrature integrand (DOCS 2055833 Eq. 2.5)
// =====================================================================

/// `O / N = P(t_e) · E_0 [ (ξ V_te + H(K) + D)+ ]` per Eq. 2.5, evaluated
/// by trapezoidal rule on the standard normal density. The `scale` factor
/// folds in the index-factor adjustment for re-versioned indices.
fn price_with_calibrated_mean(ctx: &ForwardCdsContext, m: f64, t_expiry: f64) -> f64 {
    quadrature_payoff(
        ctx,
        m,
        ctx.strike_adjustment_per_n(),
        ctx.loss_settlement_per_n(),
        t_expiry,
    )
}

fn quadrature_payoff(ctx: &ForwardCdsContext, m: f64, h_k: f64, d_loss: f64, t_expiry: f64) -> f64 {
    let t_expiry = t_expiry.max(0.0);
    let s0 = (-0.5 * ctx.sigma * ctx.sigma * t_expiry).exp();
    let sigma_sqrt_t = ctx.sigma * t_expiry.sqrt();
    let sign = ctx.sign();

    let expected_payoff = normal_integral(Z_STEP, z_limit(ctx.sigma, t_expiry), |z| {
        let s = m * s0 * (sigma_sqrt_t * z).exp();
        let v = ctx.swap_value_per_n(s); // V_te / N
        (sign * v + h_k + d_loss).max(0.0)
    });
    ctx.scale * ctx.exercise_survival_multiplier() * expected_payoff * ctx.df_to_expiry
}

// =====================================================================
// Internal utilities
// =====================================================================

fn decimal_to_f64(value: Decimal) -> Result<f64> {
    value.to_f64().ok_or_else(|| {
        finstack_core::Error::Validation(format!(
            "Bloomberg CDSO quadrature: cannot represent {value} as f64"
        ))
    })
}

fn index_option_front_end_protection_start(option: &CDSOption, as_of: Date) -> Result<Date> {
    let calendar_id = option.underlying_convention.default_calendar();
    let calendar = CalendarRegistry::global()
        .resolve_str(calendar_id)
        .ok_or_else(|| {
            finstack_core::Error::Validation(format!(
                "missing CDS option calendar '{calendar_id}' for {:?}",
                option.underlying_convention
            ))
        })?;
    let trade_date = adjust(as_of, BusinessDayConvention::Following, calendar)?;
    trade_date.add_business_days(INDEX_OPTION_FEP_START_LAG_BD, calendar)
}

/// Bloomberg CDSO standard-normal quadrature half-width.
///
/// DOCS 2055833 and Bloomberg-derived references use a fixed `[-6, 6]`
/// driver grid for normal market vols. The legacy `4·σ√t` guard is retained
/// only for extreme stress inputs so the integration range never narrows
/// relative to the prior high-vol protection.
fn z_limit(sigma: f64, t_expiry: f64) -> f64 {
    MIN_Z_LIMIT.max(4.0 * sigma * t_expiry.max(0.0).sqrt())
}

fn normal_integral<F>(step: f64, limit: f64, mut value_at: F) -> f64
where
    F: FnMut(f64) -> f64,
{
    let n_steps = ((2.0 * limit) / step).round() as usize;
    let mut acc = 0.0;
    for i in 0..=n_steps {
        let z = -limit + (i as f64) * step;
        let weight = if i == 0 || i == n_steps { 0.5 } else { 1.0 };
        acc += weight * value_at(z) * (-0.5 * z * z).exp();
    }
    acc * INV_SQRT_2_PI * step
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::calibration::api::engine;
    use crate::calibration::api::schema::CalibrationEnvelope;
    use crate::instruments::credit_derivatives::cds_option::parameters::CDSOptionParams;
    use crate::instruments::credit_derivatives::cds_option::pricer::synthetic_underlying_cds;
    use crate::instruments::CreditParams;
    use finstack_core::currency::Currency;
    use finstack_core::dates::DateExt;
    use finstack_core::market_data::context::MarketContext;
    use finstack_core::market_data::term_structures::{DiscountCurve, HazardCurve};
    use rust_decimal::Decimal;
    use serde_json::Value;
    use std::fs;
    use std::path::PathBuf;
    use time::macros::date;

    fn bp_to_decimal(bp: f64) -> Decimal {
        Decimal::try_from(bp / BASIS_POINTS_PER_UNIT).expect("valid decimal from bp")
    }

    fn flat_discount(id: &str, base: Date, rate: f64) -> DiscountCurve {
        DiscountCurve::builder(id)
            .base_date(base)
            .knots([
                (0.0, 1.0),
                (1.0, (-rate).exp()),
                (5.0, (-rate * 5.0).exp()),
                (10.0, (-rate * 10.0).exp()),
            ])
            .build()
            .expect("flat discount curve")
    }

    fn flat_hazard(id: &str, base: Date, recovery: f64, hazard_rate: f64) -> HazardCurve {
        let par = hazard_rate * BASIS_POINTS_PER_UNIT * (1.0 - recovery);
        HazardCurve::builder(id)
            .base_date(base)
            .recovery_rate(recovery)
            .knots([(1.0, hazard_rate), (5.0, hazard_rate), (10.0, hazard_rate)])
            .par_spreads([(1.0, par), (5.0, par), (10.0, par)])
            .build()
            .expect("flat hazard curve")
    }

    fn market(as_of: Date) -> MarketContext {
        MarketContext::new()
            .insert(flat_discount("USD-OIS", as_of, 0.03))
            .insert(flat_hazard("HZ-SN", as_of, 0.4, 0.02))
    }

    fn option(as_of: Date, option_type: OptionType, strike_bp: f64, vol: f64) -> CDSOption {
        option_with_coupon(as_of, option_type, strike_bp, strike_bp, vol)
    }

    fn option_with_coupon(
        as_of: Date,
        option_type: OptionType,
        strike_bp: f64,
        coupon_bp: f64,
        vol: f64,
    ) -> CDSOption {
        let params = CDSOptionParams::new(
            bp_to_decimal(strike_bp),
            as_of.add_months(12),
            as_of.add_months(60),
            Money::new(10_000_000.0, Currency::USD),
            option_type,
        )
        .expect("valid option params")
        .with_underlying_cds_coupon(bp_to_decimal(coupon_bp));
        let credit = CreditParams::corporate_standard("SN", "HZ-SN");
        let mut option = CDSOption::new("CDSO-UNIT", &params, &credit, "USD-OIS", "CDSO-VOL")
            .expect("valid cds option");
        option.pricing_overrides.market_quotes.implied_volatility = Some(vol);
        option
    }

    fn context_for(
        option: &CDSOption,
        market: &MarketContext,
        as_of: Date,
        sigma: f64,
    ) -> ForwardCdsContext {
        let cds = synthetic_underlying_cds(option, as_of).expect("synthetic cds");
        let disc = market
            .get_discount(&option.discount_curve_id)
            .expect("discount");
        let hazard = market.get_hazard(&option.credit_curve_id).expect("hazard");
        ForwardCdsContext::build(option, disc.as_ref(), hazard.as_ref(), &cds, as_of, sigma)
            .expect("forward cds context")
    }

    fn deterministic_payoff_per_n(ctx: &ForwardCdsContext) -> f64 {
        ctx.scale
            * ctx.exercise_survival_multiplier()
            * ctx.df_to_expiry
            * (ctx.sign() * ctx.no_knockout_forward()
                + ctx.strike_adjustment_per_n()
                + ctx.loss_settlement_per_n())
            .max(0.0)
    }

    fn normal_cdf(x: f64) -> f64 {
        let t = 1.0 / (1.0 + 0.231_641_9 * x.abs());
        let poly = t
            * (0.319_381_530
                + t * (-0.356_563_782
                    + t * (1.781_477_937 + t * (-1.821_255_978 + t * 1.330_274_429))));
        let tail = INV_SQRT_2_PI * (-0.5 * x * x).exp() * poly;
        if x >= 0.0 {
            1.0 - tail
        } else {
            tail
        }
    }

    fn black76_payer_per_n(ctx: &ForwardCdsContext) -> f64 {
        let f = ctx.forward_par_spread.max(numerical::ZERO_TOLERANCE);
        let k = ctx.strike.max(numerical::ZERO_TOLERANCE);
        let vol_sqrt_t = ctx.sigma * ctx.t_expiry.sqrt();
        let d1 = ((f / k).ln() + 0.5 * vol_sqrt_t * vol_sqrt_t) / vol_sqrt_t;
        let d2 = d1 - vol_sqrt_t;
        ctx.df_to_expiry
            * ctx.exercise_survival_multiplier()
            * ctx.bootstrapped_l_at_expiry
            * (f * normal_cdf(d1) - k * normal_cdf(d2))
    }

    fn calibrate_lognormal_mean_to_target(ctx: &ForwardCdsContext, target: f64) -> Result<f64> {
        let t_expiry = ctx.t_expiry.max(0.0);
        let s0 = (-0.5 * ctx.sigma * ctx.sigma * t_expiry).exp();
        let sigma_sqrt_t = ctx.sigma * t_expiry.sqrt();
        let expected_v_te = |m: f64| -> f64 {
            normal_integral(Z_STEP, z_limit(ctx.sigma, ctx.t_expiry), |z| {
                let s = m * s0 * (sigma_sqrt_t * z).exp();
                ctx.swap_value_per_n(s)
            })
        };
        let f = |log_m: f64| -> f64 { expected_v_te(log_m.exp()) - target };
        const LOG_M_LO: f64 = -18.420_680_743_952_367;
        const LOG_M_HI: f64 = 4.605_170_185_988_092;
        const MAX_EXPANSIONS: usize = 30;
        let m_seed = ctx.forward_par_spread.clamp(1e-8, 100.0);
        let log_seed = m_seed.ln().clamp(LOG_M_LO, LOG_M_HI);
        let f_seed = f(log_seed);
        let (mut lo_x, mut lo_f) = (log_seed, f_seed);
        let (mut hi_x, mut hi_f) = (log_seed, f_seed);
        let mut bracket = None;
        let step = (2.0_f64).ln();
        for k in 1..=MAX_EXPANSIONS {
            let widen = step * (k as f64);
            let x_lo_new = (log_seed - widen).max(LOG_M_LO);
            let x_hi_new = (log_seed + widen).min(LOG_M_HI);
            if x_lo_new < lo_x {
                let f_new = f(x_lo_new);
                if f_new.is_finite() && f_new * lo_f <= 0.0 {
                    bracket = Some((x_lo_new, lo_x));
                    break;
                }
                lo_x = x_lo_new;
                lo_f = f_new;
            }
            if x_hi_new > hi_x {
                let f_new = f(x_hi_new);
                if f_new.is_finite() && f_new * hi_f <= 0.0 {
                    bracket = Some((hi_x, x_hi_new));
                    break;
                }
                hi_x = x_hi_new;
                hi_f = f_new;
            }
        }
        let Some((lo, hi)) = bracket else {
            return Err(finstack_core::Error::Validation(format!(
                "diagnostic calibration bracket violation: target={target}, seed={m_seed}, \
                 f_lo={lo_f:.6e}, f_hi={hi_f:.6e}",
            )));
        };
        let solver = BrentSolver::new().tolerance(1e-12);
        solver.solve_in_bracket(f, lo, hi).map(f64::exp)
    }

    fn cdx_fixture() -> (CDSOption, MarketContext) {
        let path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("tests/golden/data/pricing/cds_option/cdx_ig_46_payer_atm_jun26.json");
        let raw = fs::read_to_string(path).expect("read cdx fixture");
        let fixture: Value = serde_json::from_str(&raw).expect("parse fixture");
        let option: CDSOption =
            serde_json::from_value(fixture["inputs"]["instrument_json"]["spec"].clone())
                .expect("parse option spec");
        let envelope: CalibrationEnvelope =
            serde_json::from_value(fixture["inputs"]["market_envelope"].clone())
                .expect("parse envelope");
        let result = engine::execute_with_diagnostics(&envelope).expect("calibrate market");
        let market = MarketContext::try_from(result.result.final_market).expect("market context");
        (option, market)
    }

    fn price_with_fep_split(ctx: &ForwardCdsContext, f0_fep: f64, d_fep: f64) -> Result<f64> {
        let m = calibrate_lognormal_mean_to_target(ctx, ctx.no_knockout_forward() + f0_fep)?;
        let realized_loss = if ctx.is_index {
            ctx.realized_index_loss / ctx.scale.max(numerical::ZERO_TOLERANCE)
        } else {
            0.0
        };
        Ok(
            quadrature_payoff(
                ctx,
                m,
                ctx.strike_adjustment_per_n(),
                ctx.sign() * (realized_loss + d_fep),
                ctx.t_expiry,
            ) * 100_000_000.0,
        )
    }

    fn calibrate_lognormal_mean_to_target_at_t(
        ctx: &ForwardCdsContext,
        target: f64,
        t_expiry: f64,
    ) -> Result<f64> {
        let t_expiry = t_expiry.max(0.0);
        let s0 = (-0.5 * ctx.sigma * ctx.sigma * t_expiry).exp();
        let sigma_sqrt_t = ctx.sigma * t_expiry.sqrt();
        let expected_v_te = |m: f64| -> f64 {
            normal_integral(Z_STEP, z_limit(ctx.sigma, t_expiry), |z| {
                let s = m * s0 * (sigma_sqrt_t * z).exp();
                ctx.swap_value_per_n(s)
            })
        };
        let f = |log_m: f64| -> f64 { expected_v_te(log_m.exp()) - target };
        const LOG_M_LO: f64 = -18.420_680_743_952_367;
        const LOG_M_HI: f64 = 4.605_170_185_988_092;
        let log_seed = ctx.forward_par_spread.clamp(1e-8, 100.0).ln();
        let step = (2.0_f64).ln();
        let mut lo_x = log_seed;
        let mut lo_f = f(lo_x);
        let mut hi_x = log_seed;
        let mut hi_f = lo_f;
        let mut bracket = None;
        for k in 1..=30 {
            let widen = step * (k as f64);
            let x_lo_new = (log_seed - widen).max(LOG_M_LO);
            let x_hi_new = (log_seed + widen).min(LOG_M_HI);
            if x_lo_new < lo_x {
                let f_new = f(x_lo_new);
                if f_new.is_finite() && f_new * lo_f <= 0.0 {
                    bracket = Some((x_lo_new, lo_x));
                    break;
                }
                lo_x = x_lo_new;
                lo_f = f_new;
            }
            if x_hi_new > hi_x {
                let f_new = f(x_hi_new);
                if f_new.is_finite() && f_new * hi_f <= 0.0 {
                    bracket = Some((hi_x, x_hi_new));
                    break;
                }
                hi_x = x_hi_new;
                hi_f = f_new;
            }
        }
        let Some((lo, hi)) = bracket else {
            return Err(finstack_core::Error::Validation(format!(
                "diagnostic t calibration bracket violation: target={target}, t={t_expiry}, \
                 f_lo={lo_f:.6e}, f_hi={hi_f:.6e}",
            )));
        };
        let solver = BrentSolver::new().tolerance(1e-12);
        solver.solve_in_bracket(f, lo, hi).map(f64::exp)
    }

    fn price_with_fep_split_at_t(
        ctx: &ForwardCdsContext,
        f0_fep: f64,
        d_fep: f64,
        t_expiry: f64,
    ) -> Result<f64> {
        let m = calibrate_lognormal_mean_to_target_at_t(
            ctx,
            ctx.no_knockout_forward() + f0_fep,
            t_expiry,
        )?;
        let realized_loss = if ctx.is_index {
            ctx.realized_index_loss / ctx.scale.max(numerical::ZERO_TOLERANCE)
        } else {
            0.0
        };
        Ok(
            quadrature_payoff(
                ctx,
                m,
                ctx.strike_adjustment_per_n(),
                ctx.sign() * (realized_loss + d_fep),
                t_expiry,
            ) * 100_000_000.0,
        )
    }

    fn solve_d_fep_for_target(ctx: &ForwardCdsContext, f0_fep: f64, target: f64) -> Result<f64> {
        let mut lo = -0.002;
        let mut hi = 0.002;
        let mut f_lo = price_with_fep_split(ctx, f0_fep, lo)? - target;
        let f_hi = price_with_fep_split(ctx, f0_fep, hi)? - target;
        assert!(
            f_lo * f_hi <= 0.0,
            "target not bracketed for d_fep solve: f0_fep={f0_fep}, f_lo={f_lo}, f_hi={f_hi}",
        );
        for _ in 0..80 {
            let mid = 0.5 * (lo + hi);
            let f_mid = price_with_fep_split(ctx, f0_fep, mid)? - target;
            if f_mid.abs() < 1e-8 {
                return Ok(mid);
            }
            if f_lo * f_mid <= 0.0 {
                hi = mid;
            } else {
                lo = mid;
                f_lo = f_mid;
            }
        }
        Ok(0.5 * (lo + hi))
    }

    fn solve_d_fep_for_target_at_t(
        ctx: &ForwardCdsContext,
        f0_fep: f64,
        target: f64,
        t_expiry: f64,
    ) -> Result<f64> {
        let mut lo = -0.002;
        let mut hi = 0.002;
        let mut f_lo = price_with_fep_split_at_t(ctx, f0_fep, lo, t_expiry)? - target;
        let f_hi = price_with_fep_split_at_t(ctx, f0_fep, hi, t_expiry)? - target;
        assert!(
            f_lo * f_hi <= 0.0,
            "target not bracketed for d_fep/t solve: f0_fep={f0_fep}, t={t_expiry}, f_lo={f_lo}, f_hi={f_hi}",
        );
        for _ in 0..80 {
            let mid = 0.5 * (lo + hi);
            let f_mid = price_with_fep_split_at_t(ctx, f0_fep, mid, t_expiry)? - target;
            if f_mid.abs() < 1e-8 {
                return Ok(mid);
            }
            if f_lo * f_mid <= 0.0 {
                hi = mid;
            } else {
                lo = mid;
                f_lo = f_mid;
            }
        }
        Ok(0.5 * (lo + hi))
    }


    #[test]
    #[ignore = "diagnostic: cdx_ig_46 CDSO risk metric FEP placement"]
    fn diag_cdx_ig_46_risk_fep_split() {
        use crate::calibration::bumps::hazard::{
            bump_hazard_shift, bump_hazard_spreads_with_doc_clause_and_valuation_convention,
        };
        use crate::calibration::bumps::BumpRequest;
        use crate::instruments::credit_derivatives::cds::CdsValuationConvention;
        use crate::market::conventions::ids::CdsDocClause;
        use crate::metrics::sensitivities::cs01::sensitivity_central_diff;

        const BBG_NPV: f64 = 118_781.76;
        const BBG_VEGA: f64 = 3_411.78;
        const BBG_CS01: f64 = 25_352.02;
        let as_of = date!(2026 - 05 - 07);
        let sigma = 0.3603;
        let (option, market) = cdx_fixture();
        let cds = synthetic_underlying_cds(&option, as_of).expect("synthetic cds");
        let ctx = context_for(&option, &market, as_of, sigma);
        let bumped_ctx = context_for(&option, &market, as_of, sigma + 0.01);
        let hazard = market.get_hazard(&option.credit_curve_id).expect("hazard");
        let up_hazard = bump_hazard_spreads_with_doc_clause_and_valuation_convention(
            hazard.as_ref(),
            &market,
            &BumpRequest::Parallel(1.0),
            Some(&option.discount_curve_id),
            Some(CdsDocClause::IsdaNa),
            Some(CdsValuationConvention::BloombergCdswClean),
        )
        .expect("up hazard");
        let down_hazard = bump_hazard_spreads_with_doc_clause_and_valuation_convention(
            hazard.as_ref(),
            &market,
            &BumpRequest::Parallel(-1.0),
            Some(&option.discount_curve_id),
            Some(CdsDocClause::IsdaNa),
            Some(CdsValuationConvention::BloombergCdswClean),
        )
        .expect("down hazard");
        let zero_hazard = bump_hazard_spreads_with_doc_clause_and_valuation_convention(
            hazard.as_ref(),
            &market,
            &BumpRequest::Parallel(0.0),
            Some(&option.discount_curve_id),
            Some(CdsDocClause::IsdaNa),
            Some(CdsValuationConvention::BloombergCdswClean),
        )
        .expect("zero hazard");
        let up_market = market.clone().insert(up_hazard);
        let down_market = market.clone().insert(down_hazard);
        let zero_market = market.clone().insert(zero_hazard);
        let up_ctx = context_for(&option, &up_market, as_of, sigma);
        let down_ctx = context_for(&option, &down_market, as_of, sigma);

        eprintln!("\n=== cdx_ig_46 CDSO risk FEP split ===");
        eprintln!("base fep(T+2->expiry)         = {:.12}", ctx.front_end_protection);
        eprintln!("target NPV / Vega / CS01      = {BBG_NPV:.4} / {BBG_VEGA:.4} / {BBG_CS01:.4}");
        eprintln!("alpha = fraction of FEP in F0; beta = fraction of FEP in D, solved to match NPV");
        for alpha in [0.0, 0.25, 0.50, 0.75, 1.0] {
            let f0_fep = alpha * ctx.front_end_protection;
            let d_fep = solve_d_fep_for_target(&ctx, f0_fep, BBG_NPV).expect("d solve");
            let beta = d_fep / ctx.front_end_protection;
            let npv = price_with_fep_split(&ctx, f0_fep, d_fep).expect("base price");
            let bumped = price_with_fep_split(
                &bumped_ctx,
                alpha * bumped_ctx.front_end_protection,
                beta * bumped_ctx.front_end_protection,
            )
            .expect("vega price");
            let vega = bumped - npv;

            let up = price_with_fep_split(
                &up_ctx,
                alpha * up_ctx.front_end_protection,
                beta * up_ctx.front_end_protection,
            )
            .expect("up price");
            let down = price_with_fep_split(
                &down_ctx,
                alpha * down_ctx.front_end_protection,
                beta * down_ctx.front_end_protection,
            )
            .expect("down price");
            let cs01 = sensitivity_central_diff(up, down, 1.0);

            eprintln!(
                "alpha={alpha:.2} beta={beta:.6} npv={npv:.4} vega={vega:.4} d_vega={:+.4} cs01={cs01:.4} d_cs01={:+.4}",
                vega - BBG_VEGA,
                cs01 - BBG_CS01
            );
        }

        let production = npv(&option, &cds, &market, sigma, as_of)
            .expect("production npv")
            .amount();
        let production_vega =
            npv(&option, &cds, &market, sigma + 0.01, as_of).expect("production bumped").amount()
                - production;
        eprintln!("production npv/vega           = {production:.4} / {production_vega:.4}");
        let vega_central_half = npv(&option, &cds, &market, sigma + 0.005, as_of)
            .expect("vega central up half")
            .amount()
            - npv(&option, &cds, &market, sigma - 0.005, as_of)
                .expect("vega central down half")
                .amount();
        let vega_central_full = 0.5
            * (npv(&option, &cds, &market, sigma + 0.01, as_of)
                .expect("vega central up full")
                .amount()
                - npv(&option, &cds, &market, sigma - 0.01, as_of)
                    .expect("vega central down full")
                    .amount());
        eprintln!(
            "vega central +/-0.5vp / +/-1vp = {vega_central_half:.4} / {vega_central_full:.4}"
        );
        let fep_start = index_option_front_end_protection_start(&option, as_of)
            .expect("fep start");
        eprintln!("\n-- t-expiry variants with D solved to target NPV --");
        for t_days in [42.0, 42.25, 42.5, 42.75, 43.0] {
            let t = t_days / G_DAYS_IN_YEAR;
            let d_fep = solve_d_fep_for_target_at_t(&ctx, 0.0, BBG_NPV, t).expect("d/t solve");
            let base_t = price_with_fep_split_at_t(&ctx, 0.0, d_fep, t).expect("base t");
            let bumped_t = price_with_fep_split_at_t(&bumped_ctx, 0.0, d_fep, t)
                .expect("bumped t");
            let beta_t = d_fep / ctx.front_end_protection;
            let cs_up_t = price_with_fep_split_at_t(
                &up_ctx,
                0.0,
                beta_t * up_ctx.front_end_protection,
                t,
            )
            .expect("cs up t");
            let cs_down_t = price_with_fep_split_at_t(
                &down_ctx,
                0.0,
                beta_t * down_ctx.front_end_protection,
                t,
            )
            .expect("cs down t");
            let cs_t = sensitivity_central_diff(cs_up_t, cs_down_t, 1.0);
            let m_t =
                calibrate_lognormal_mean_to_target_at_t(&ctx, ctx.no_knockout_forward(), t)
                    .expect("theta m/t");
            let theta_t = quadrature_payoff(
                &ctx,
                m_t,
                ctx.strike_adjustment_per_n(),
                ctx.sign() * d_fep,
                (t - (1.0 / THETA_DAYS_IN_YEAR)).max(0.0),
            ) * option.notional.amount()
                - base_t;
            eprintln!(
                "t_days={t_days:.2} d_fep={d_fep:.12} vega={:.4} d_vega={:+.4} cs01={cs_t:.4} d_cs01={:+.4} theta={theta_t:.4} d_theta={:+.4}",
                bumped_t - base_t,
                bumped_t - base_t - BBG_VEGA,
                cs_t - BBG_CS01,
                theta_t + 1_499.93
            );
        }
        for start in [
            fep_start,
            option.effective_cash_settlement_date(as_of).expect("cash date"),
            option
                .effective_cash_settlement_date(as_of)
                .expect("cash date")
                .add_business_days(2, CalendarRegistry::global().resolve_str(option.underlying_convention.default_calendar()).expect("calendar"))
                .expect("cash plus two"),
        ] {
            let sp_s = hazard.sp_on_date(start).expect("start sp");
            let sp_e = hazard.sp_on_date(option.expiry).expect("expiry sp");
            let d = ctx.lgd * (1.0 - (sp_e / sp_s).clamp(0.0, 1.0));
            let t = 42.5 / G_DAYS_IN_YEAR;
            let p = price_with_fep_split_at_t(&ctx, 0.0, d, t).expect("date/t price");
            let v = price_with_fep_split_at_t(&bumped_ctx, 0.0, d, t).expect("date/t bumped") - p;
            eprintln!(
                "t_days=42.50 fep_start={start} d_fep={d:.12} npv={p:.4} d_npv={:+.4} vega={v:.4} d_vega={:+.4}",
                p - BBG_NPV,
                v - BBG_VEGA
            );
        }

        let prod_up = price_with_fep_split(&up_ctx, 0.0, up_ctx.front_end_protection)
            .expect("production up custom");
        let prod_down = price_with_fep_split(&down_ctx, 0.0, down_ctx.front_end_protection)
            .expect("production down custom");
        let zero = npv(&option, &cds, &zero_market, sigma, as_of)
            .expect("zero rebootstrap")
            .amount();
        let prod_held_fep_up = price_with_fep_split(&up_ctx, 0.0, ctx.front_end_protection)
            .expect("held-fep up");
        let prod_held_fep_down = price_with_fep_split(&down_ctx, 0.0, ctx.front_end_protection)
            .expect("held-fep down");
        let direct_up_market = market.clone().insert(
            bump_hazard_shift(hazard.as_ref(), &BumpRequest::Parallel(1.0))
                .expect("direct up hazard"),
        );
        let direct_down_market = market.clone().insert(
            bump_hazard_shift(hazard.as_ref(), &BumpRequest::Parallel(-1.0))
                .expect("direct down hazard"),
        );
        let direct_up = npv(
            &option,
            &cds,
            &direct_up_market,
            sigma,
            as_of,
        )
        .expect("direct up")
        .amount();
        let direct_down = npv(
            &option,
            &cds,
            &direct_down_market,
            sigma,
            as_of,
        )
        .expect("direct down")
        .amount();
        eprintln!("\n-- cs01 conventions (production FEP placement) --");
        eprintln!(
            "rebootstrap central           = {:.4}",
            sensitivity_central_diff(prod_up, prod_down, 1.0)
        );
        eprintln!("rebootstrap one-sided up      = {:.4}", prod_up - production);
        eprintln!("rebootstrap one-sided down    = {:.4}", production - prod_down);
        eprintln!(
            "rebootstrap zero-base pv      = {zero:.4}  drift={:+.4}",
            zero - production
        );
        eprintln!("rebootstrap up from zero      = {:.4}", prod_up - zero);
        eprintln!("rebootstrap down from zero    = {:.4}", zero - prod_down);
        eprintln!(
            "rebootstrap central held FEP  = {:.4}",
            sensitivity_central_diff(prod_held_fep_up, prod_held_fep_down, 1.0)
        );
        eprintln!(
            "direct hazard central         = {:.4}",
            sensitivity_central_diff(direct_up, direct_down, 1.0)
        );
        for convention in [
            CdsValuationConvention::BloombergCdswCleanFullPremium,
            CdsValuationConvention::QuantLibIsdaParity,
        ] {
            let up_alt = bump_hazard_spreads_with_doc_clause_and_valuation_convention(
                hazard.as_ref(),
                &market,
                &BumpRequest::Parallel(1.0),
                Some(&option.discount_curve_id),
                Some(CdsDocClause::IsdaNa),
                Some(convention),
            )
            .expect("alt up hazard");
            let down_alt = bump_hazard_spreads_with_doc_clause_and_valuation_convention(
                hazard.as_ref(),
                &market,
                &BumpRequest::Parallel(-1.0),
                Some(&option.discount_curve_id),
                Some(CdsDocClause::IsdaNa),
                Some(convention),
            )
            .expect("alt down hazard");
            let up_alt_market = market.clone().insert(up_alt);
            let down_alt_market = market.clone().insert(down_alt);
            let up_alt_pv = npv(&option, &cds, &up_alt_market, sigma, as_of)
                .expect("alt up pv")
                .amount();
            let down_alt_pv = npv(&option, &cds, &down_alt_market, sigma, as_of)
                .expect("alt down pv")
                .amount();
            eprintln!(
                "rebootstrap central {:?} = {:.4}",
                convention,
                sensitivity_central_diff(up_alt_pv, down_alt_pv, 1.0)
            );
        }

        let m = calibrate_lognormal_mean(&ctx).expect("theta calibration");
        let shortened_t = (ctx.t_expiry - (1.0 / THETA_DAYS_IN_YEAR)).max(0.0);
        let shortened_expiry = option.expiry - time::Duration::days(1);
        let sp_start = hazard
            .sp_on_date(fep_start)
            .expect("sp start")
            .max(numerical::ZERO_TOLERANCE);
        let sp_short = hazard
            .sp_on_date(shortened_expiry)
            .expect("sp shortened expiry")
            .clamp(0.0, 1.0);
        let shortened_fep = ctx.lgd * (1.0 - (sp_short / sp_start).clamp(0.0, 1.0));
        let t_start = hazard
            .day_count()
            .year_fraction(
                hazard.base_date(),
                fep_start,
                finstack_core::dates::DayCountContext::default(),
            )
            .expect("fep start time");
        let t_expiry_hazard = hazard
            .day_count()
            .year_fraction(
                hazard.base_date(),
                option.expiry,
                finstack_core::dates::DayCountContext::default(),
            )
            .expect("expiry hazard time");
        let sp_short_frac = hazard.sp((t_expiry_hazard - (1.0 / THETA_DAYS_IN_YEAR)).max(t_start));
        let shortened_fep_frac = ctx.lgd * (1.0 - (sp_short_frac / sp_start).clamp(0.0, 1.0));
        let theta_current = theta(&option, &cds, &market, sigma, as_of).expect("theta");
        let cds_tomorrow =
            synthetic_underlying_cds(&option, as_of + time::Duration::days(1)).expect("tom cds");
        let theta_asof_shift = npv(
            &option,
            &cds_tomorrow,
            &market,
            sigma,
            as_of + time::Duration::days(1),
        )
        .expect("asof shift theta")
        .amount()
            - production;
        let theta_fep_horizon = (quadrature_payoff(
            &ctx,
            m,
            ctx.strike_adjustment_per_n(),
            ctx.sign() * shortened_fep,
            shortened_t,
        ) * option.notional.amount())
            - production;
        eprintln!("\n-- theta conventions --");
        eprintln!("pure t shift current          = {theta_current:.4}");
        eprintln!("as-of +1 calendar revalue     = {theta_asof_shift:.4}");
        eprintln!(
            "pure t + shortened FEP end    = {theta_fep_horizon:.4}  fep_short={shortened_fep:.12}"
        );
        let theta_fep_horizon_frac = (quadrature_payoff(
            &ctx,
            m,
            ctx.strike_adjustment_per_n(),
            ctx.sign() * shortened_fep_frac,
            shortened_t,
        ) * option.notional.amount())
            - production;
        eprintln!(
            "pure t + fractional FEP end   = {theta_fep_horizon_frac:.4}  fep_short={shortened_fep_frac:.12}"
        );
    }

    #[test]
    #[ignore = "diagnostic: exact cdx_ig_46 CDSO NPV decomposition"]
    fn diag_cdx_ig_46_npv_decomposition() {
        let as_of = date!(2026 - 05 - 07);
        let (option, market) = cdx_fixture();
        let sigma = 0.3603;
        let ctx = context_for(&option, &market, as_of, sigma);
        let m = calibrate_lognormal_mean(&ctx).expect("base calibration");

        let h1 = if ctx.knockout {
            0.0
        } else {
            ctx.lgd * (1.0 - ctx.survival_to_expiry)
        };
        let h2 = (ctx.forward_par_spread - ctx.coupon) * ctx.bootstrapped_l_at_expiry;
        let f0 = ctx.no_knockout_forward();
        let h_k_flat = ctx.strike_adjustment_per_n();
        let h_k_boot = ctx.sign() * (ctx.coupon - ctx.strike) * ctx.bootstrapped_l_at_expiry;

        let base = price_with_calibrated_mean(&ctx, m, ctx.t_expiry) * option.notional.amount();
        let no_fep = calibrate_lognormal_mean_to_target(&ctx, h2)
            .map(|m| price_with_calibrated_mean(&ctx, m, ctx.t_expiry) * option.notional.amount());
        let display_target =
            h1 + (ctx.display_forward_par_spread - ctx.coupon) * ctx.bootstrapped_l_at_expiry;
        let display_f0 = calibrate_lognormal_mean_to_target(&ctx, display_target)
            .map(|m| price_with_calibrated_mean(&ctx, m, ctx.t_expiry) * option.notional.amount());
        let boot_h =
            quadrature_payoff(&ctx, m, h_k_boot, ctx.loss_settlement_per_n(), ctx.t_expiry)
                * option.notional.amount();
        let fep_as_payoff = quadrature_payoff(
            &ctx,
            m,
            h_k_flat,
            ctx.loss_settlement_per_n() + h1,
            ctx.t_expiry,
        ) * option.notional.amount();
        let fep_as_payoff_boot_h = quadrature_payoff(
            &ctx,
            m,
            h_k_boot,
            ctx.loss_settlement_per_n() + h1,
            ctx.t_expiry,
        ) * option.notional.amount();
        let fep_from = |start: Date, end: Date| -> f64 {
            let sp_start = market
                .get_hazard(&option.credit_curve_id)
                .expect("hazard")
                .sp_on_date(start)
                .unwrap_or(1.0)
                .max(numerical::ZERO_TOLERANCE);
            let sp_end = market
                .get_hazard(&option.credit_curve_id)
                .expect("hazard")
                .sp_on_date(end)
                .unwrap_or(1.0);
            ctx.lgd * (1.0 - (sp_end / sp_start).clamp(0.0, 1.0))
        };
        let fep_next_day = fep_from(as_of + time::Duration::days(1), option.expiry);
        let fep_t_plus_2 = fep_from(as_of + time::Duration::days(4), option.expiry);
        let fep_cash_settle = fep_from(
            option
                .effective_cash_settlement_date(as_of)
                .expect("cash settlement"),
            option.expiry,
        );
        let fep_next_day_price = quadrature_payoff(
            &ctx,
            m,
            h_k_flat,
            ctx.loss_settlement_per_n() + fep_next_day,
            ctx.t_expiry,
        ) * option.notional.amount();
        let fep_t_plus_2_price = quadrature_payoff(
            &ctx,
            m,
            h_k_flat,
            ctx.loss_settlement_per_n() + fep_t_plus_2,
            ctx.t_expiry,
        ) * option.notional.amount();
        let fep_cash_settle_price = quadrature_payoff(
            &ctx,
            m,
            h_k_flat,
            ctx.loss_settlement_per_n() + fep_cash_settle,
            ctx.t_expiry,
        ) * option.notional.amount();
        let pure_forward_target = f0 - h1;
        let pure_forward_m = calibrate_lognormal_mean_to_target(&ctx, pure_forward_target)
            .expect("pure forward calibration");
        let pure_forward_plus_fep = quadrature_payoff(
            &ctx,
            pure_forward_m,
            h_k_flat,
            ctx.loss_settlement_per_n() + h1,
            ctx.t_expiry,
        ) * option.notional.amount();
        let full_fep_m =
            calibrate_lognormal_mean_to_target(&ctx, f0 + h1).expect("full fep calibration");
        let full_fep_flat_h =
            price_with_calibrated_mean(&ctx, full_fep_m, ctx.t_expiry) * option.notional.amount();
        let full_fep_boot_h = quadrature_payoff(
            &ctx,
            full_fep_m,
            h_k_boot,
            ctx.loss_settlement_per_n(),
            ctx.t_expiry,
        ) * option.notional.amount();
        let legal_t = 41.0 / 365.0;
        let legal_t_price = price_with_calibrated_mean(&ctx, m, legal_t) * option.notional.amount();

        eprintln!("\n=== cdx_ig_46 CDSO NPV decomposition ===");
        eprintln!("target Bloomberg NPV          = 118781.76");
        eprintln!("base price                    = {base:.4}");
        eprintln!("base - target                 = {:+.4}", base - 118_781.76);
        eprintln!("\n-- context --");
        eprintln!("t_expiry                      = {:.10}", ctx.t_expiry);
        eprintln!("df_to_expiry                  = {:.10}", ctx.df_to_expiry);
        eprintln!(
            "survival_to_expiry            = {:.10}",
            ctx.survival_to_expiry
        );
        eprintln!("coupon                        = {:.8}", ctx.coupon);
        eprintln!("strike                        = {:.8}", ctx.strike);
        eprintln!(
            "economic forward par bp       = {:.8}",
            ctx.forward_par_spread * 10_000.0
        );
        eprintln!(
            "display forward par bp        = {:.8}",
            ctx.display_forward_par_spread * 10_000.0
        );
        eprintln!(
            "bootstrapped L                = {:.10}",
            ctx.bootstrapped_l_at_expiry
        );
        eprintln!(
            "flat L(strike)                = {:.10}",
            ctx.flat_annuity(ctx.strike)
        );
        eprintln!(
            "flat L(fwd)                   = {:.10}",
            ctx.flat_annuity(ctx.forward_par_spread)
        );
        eprintln!("\n-- calibration --");
        eprintln!("h1 FEP                        = {:.12}", h1);
        eprintln!("h2 (fwd-coupon)*L             = {:.12}", h2);
        eprintln!("F0 target                     = {:.12}", f0);
        eprintln!("m base bp                     = {:.8}", m * 10_000.0);
        eprintln!("H(K) flat                     = {:.12}", h_k_flat);
        eprintln!("H(K) boot L                   = {:.12}", h_k_boot);
        eprintln!("\n-- variants --");
        match no_fep {
            Ok(price) => {
                eprintln!(
                    "no FEP target                 = {price:.4}  diff={:+.4}",
                    price - 118_781.76
                );
            }
            Err(err) => {
                eprintln!("no FEP target                 = calibration failed: {err}");
            }
        }
        match display_f0 {
            Ok(price) => {
                eprintln!(
                    "display F0 target             = {price:.4}  diff={:+.4}",
                    price - 118_781.76
                );
            }
            Err(err) => {
                eprintln!("display F0 target             = calibration failed: {err}");
            }
        }
        eprintln!(
            "boot L in H(K)                = {boot_h:.4}  diff={:+.4}",
            boot_h - 118_781.76
        );
        eprintln!(
            "FEP as payoff D               = {fep_as_payoff:.4}  diff={:+.4}",
            fep_as_payoff - 118_781.76
        );
        eprintln!(
            "FEP D from as_of+1d           = {fep_next_day_price:.4}  diff={:+.4}  fep={:.12}",
            fep_next_day_price - 118_781.76,
            fep_next_day
        );
        eprintln!(
            "FEP D from T+2 calendar       = {fep_t_plus_2_price:.4}  diff={:+.4}  fep={:.12}",
            fep_t_plus_2_price - 118_781.76,
            fep_t_plus_2
        );
        eprintln!(
            "FEP D from cash settlement    = {fep_cash_settle_price:.4}  diff={:+.4}  fep={:.12}",
            fep_cash_settle_price - 118_781.76,
            fep_cash_settle
        );
        eprintln!(
            "FEP as D + boot L H(K)        = {fep_as_payoff_boot_h:.4}  diff={:+.4}",
            fep_as_payoff_boot_h - 118_781.76
        );
        eprintln!(
            "pure fwd F0 + FEP as D        = {pure_forward_plus_fep:.4}  diff={:+.4}",
            pure_forward_plus_fep - 118_781.76
        );
        eprintln!(
            "full FEP in F0 + flat H(K)    = {full_fep_flat_h:.4}  diff={:+.4}",
            full_fep_flat_h - 118_781.76
        );
        eprintln!(
            "full FEP in F0 + boot H(K)    = {full_fep_boot_h:.4}  diff={:+.4}",
            full_fep_boot_h - 118_781.76
        );
        eprintln!(
            "price with 41/365 t only      = {legal_t_price:.4}  diff={:+.4}",
            legal_t_price - 118_781.76
        );
    }

    #[test]
    fn normal_quadrature_converges_when_step_is_halved() {
        let coarse = normal_integral(0.05, MIN_Z_LIMIT, |z| (0.30 * z).exp().max(0.0));
        let fine = normal_integral(0.025, MIN_Z_LIMIT, |z| (0.30 * z).exp().max(0.0));
        assert!(
            (coarse - fine).abs() < 1e-8,
            "normal quadrature should be stable under step halving: coarse={coarse}, fine={fine}",
        );
    }

    /// Bloomberg CDSO screen reconciliation depends on the published
    /// standard-normal driver grid `[-6, 6]` for ordinary market vols. Keep
    /// that fixed range for realistic short-dated CDSO inputs while retaining
    /// the legacy stress guard for extreme `σ√t`.
    #[test]
    fn z_limit_uses_bloomberg_grid_with_legacy_stress_guard() {
        let cdx_ig_46_sigma = 0.3603_f64;
        let cdx_ig_46_t = 42.0 / 365.0;
        assert_eq!(z_limit(cdx_ig_46_sigma, cdx_ig_46_t), MIN_Z_LIMIT);

        let stressed_sigma = 2.0_f64;
        let stressed_t = 4.0_f64;
        let stressed_limit = z_limit(stressed_sigma, stressed_t);
        assert!(
            (stressed_limit - 16.0).abs() < 1e-12,
            "extreme vol-time inputs should still use the legacy 4σ√t guard, got {stressed_limit}"
        );
    }

    #[test]
    fn zero_vol_limit_matches_bloomberg_deterministic_payoff() {
        let as_of = date!(2025 - 01 - 01);
        let market = market(as_of);
        let option = option(as_of, OptionType::Call, 100.0, 1e-6);
        let cds = synthetic_underlying_cds(&option, as_of).expect("synthetic cds");
        let ctx = context_for(&option, &market, as_of, 1e-6);

        let actual_per_n = npv(&option, &cds, &market, 1e-6, as_of)
            .expect("npv")
            .amount()
            / option.notional.amount();
        let expected_per_n = deterministic_payoff_per_n(&ctx);

        assert!(
            (actual_per_n - expected_per_n).abs() < 1e-8,
            "zero-vol CDSO payoff should converge to Bloomberg deterministic payoff: actual={actual_per_n}, expected={expected_per_n}"
        );
    }

    #[test]
    fn bloomberg_intrinsic_lower_bound_holds() {
        let as_of = date!(2025 - 01 - 01);
        let market = market(as_of);

        for option_type in [OptionType::Call, OptionType::Put] {
            for strike_bp in [50.0, 100.0, 200.0, 400.0] {
                let option = option(as_of, option_type, strike_bp, 0.30);
                let cds = synthetic_underlying_cds(&option, as_of).expect("synthetic cds");
                let ctx = context_for(&option, &market, as_of, 0.30);
                let actual_per_n = npv(&option, &cds, &market, 0.30, as_of)
                    .expect("npv")
                    .amount()
                    / option.notional.amount();
                let lower_bound = deterministic_payoff_per_n(&ctx);

                assert!(
                    actual_per_n + 1e-10 >= lower_bound,
                    "Bloomberg intrinsic lower bound violated for {:?} strike {strike_bp}: actual={actual_per_n}, lower_bound={lower_bound}",
                    option_type,
                );
            }
        }
    }

    #[test]
    fn calibration_mean_is_option_type_invariant() {
        let as_of = date!(2025 - 01 - 01);
        let market = market(as_of);
        let call = option(as_of, OptionType::Call, 125.0, 0.35);
        let put = option(as_of, OptionType::Put, 125.0, 0.35);
        let call_ctx = context_for(&call, &market, as_of, 0.35);
        let put_ctx = context_for(&put, &market, as_of, 0.35);

        let call_m = calibrate_lognormal_mean(&call_ctx).expect("call calibration");
        let put_m = calibrate_lognormal_mean(&put_ctx).expect("put calibration");

        assert!(
            (call_m - put_m).abs() < 1e-12,
            "lognormal mean calibration should not depend on payer/receiver option type: call_m={call_m}, put_m={put_m}"
        );
    }

    #[test]
    fn bloomberg_put_call_parity_holds() {
        let as_of = date!(2025 - 01 - 01);
        let market = market(as_of);

        for strike_bp in [50.0, 100.0, 200.0, 400.0] {
            let call = option(as_of, OptionType::Call, strike_bp, 0.30);
            let put = option(as_of, OptionType::Put, strike_bp, 0.30);
            let call_cds = synthetic_underlying_cds(&call, as_of).expect("call cds");
            let put_cds = synthetic_underlying_cds(&put, as_of).expect("put cds");
            let call_ctx = context_for(&call, &market, as_of, 0.30);

            let call_pv = npv(&call, &call_cds, &market, 0.30, as_of)
                .expect("call npv")
                .amount();
            let put_pv = npv(&put, &put_cds, &market, 0.30, as_of)
                .expect("put npv")
                .amount();
            let expected = call.notional.amount()
                * call_ctx.scale
                * call_ctx.exercise_survival_multiplier()
                * call_ctx.df_to_expiry
                * (call_ctx.no_knockout_forward()
                    + call_ctx.strike_adjustment_per_n()
                    + call_ctx.loss_settlement_per_n());

            assert!(
                (call_pv - put_pv - expected).abs() < 1e-3,
                "Bloomberg parity OC-OP=P_te*(F0+H(K)+D) failed at strike {strike_bp}: call={call_pv}, put={put_pv}, expected_diff={expected}, diff={}",
                (call_pv - put_pv - expected).abs()
            );
        }
    }

    /// Companion to [`bloomberg_put_call_parity_holds`] that decouples the
    /// underlying CDS coupon `c` from the option strike `K`. The default
    /// parity helper sets `c = K`, which collapses `H(K) = ξN(c−K)A(K)` to
    /// zero — so it does not actually exercise the strike-adjustment term.
    /// This test pins parity for the CDX-style case where the underlying
    /// runs at a standard 100 bp coupon and the option is struck above
    /// and below that coupon, making `(c−K)L(K)` non-trivial in the
    /// parity arithmetic.
    #[test]
    fn bloomberg_put_call_parity_holds_with_distinct_coupon_and_strike() {
        let as_of = date!(2025 - 01 - 01);
        let market = market(as_of);
        let coupon_bp = 100.0;
        let vol = 0.30;

        for strike_bp in [50.0, 75.0, 125.0, 200.0, 400.0] {
            let call = option_with_coupon(as_of, OptionType::Call, strike_bp, coupon_bp, vol);
            let put = option_with_coupon(as_of, OptionType::Put, strike_bp, coupon_bp, vol);
            let call_cds = synthetic_underlying_cds(&call, as_of).expect("call cds");
            let put_cds = synthetic_underlying_cds(&put, as_of).expect("put cds");
            let call_ctx = context_for(&call, &market, as_of, vol);

            // Sanity: the strike-adjustment term must be non-zero in this
            // configuration, otherwise the test does not exercise H(K).
            let strike_adj = call_ctx.strike_adjustment_per_n();
            assert!(
                strike_adj.abs() > 1e-12,
                "test setup error: strike adjustment H(K)/N is zero at strike {strike_bp} with coupon {coupon_bp}"
            );

            let call_pv = npv(&call, &call_cds, &market, vol, as_of)
                .expect("call npv")
                .amount();
            let put_pv = npv(&put, &put_cds, &market, vol, as_of)
                .expect("put npv")
                .amount();
            let expected = call.notional.amount()
                * call_ctx.scale
                * call_ctx.exercise_survival_multiplier()
                * call_ctx.df_to_expiry
                * (call_ctx.no_knockout_forward()
                    + call_ctx.strike_adjustment_per_n()
                    + call_ctx.loss_settlement_per_n());

            // For deep-OTM strikes with c ≠ K, |expected| can dwarf the
            // ATM scale (the (c−K)L(K) term grows linearly with the
            // strike–coupon gap), so an absolute tolerance gates on the
            // wrong scale. Use the larger of $1e-3 (ATM-scale floor) and
            // a 1e-7 relative bound — the trapezoidal quadrature targets
            // ~1e-9 relative precision so this leaves plenty of headroom.
            let abs_diff = (call_pv - put_pv - expected).abs();
            let tolerance =
                1e-3_f64.max(1e-7 * expected.abs().max(call_pv.abs()).max(put_pv.abs()));
            assert!(
                abs_diff < tolerance,
                "Bloomberg parity OC-OP=P_te*(F0+H(K)+D) failed at strike {strike_bp} \
                 (coupon {coupon_bp}): call={call_pv}, put={put_pv}, expected_diff={expected}, \
                 diff={abs_diff}, tol={tolerance}, strike_adj_per_N={strike_adj}"
            );
        }
    }

    #[test]
    fn stripped_low_vol_fixture_approaches_black76() {
        // Black-76 has no FEP-equivalent — it values only the spread payoff
        // at exercise — so to compare against it we must use a knockout
        // contract (where the option pays nothing on default-before-expiry).
        let as_of = date!(2025 - 01 - 01);
        let market = market(as_of);
        let sigma = 0.01;
        let mut option = option(as_of, OptionType::Call, 100.0, sigma);
        option.knockout = true;
        let cds = synthetic_underlying_cds(&option, as_of).expect("synthetic cds");
        let ctx = context_for(&option, &market, as_of, sigma);

        let actual_per_n = npv(&option, &cds, &market, sigma, as_of)
            .expect("npv")
            .amount()
            / option.notional.amount();
        let black_per_n = black76_payer_per_n(&ctx);
        let tolerance = 0.01 * black_per_n.abs().max(1e-8);

        assert!(
            (actual_per_n - black_per_n).abs() <= tolerance,
            "stripped low-vol fixture should approach Black-76: actual={actual_per_n}, black={black_per_n}, diff={}, tol={tolerance}",
            (actual_per_n - black_per_n).abs()
        );
    }

    /// Single-name no-knockout behavior is represented through the exercise
    /// survival multiplier, not an index front-end-protection payoff. The
    /// Bloomberg index-option FEP convention is specific to indices.
    #[test]
    fn non_knockout_single_name_does_not_add_index_fep() {
        let as_of = date!(2025 - 01 - 01);
        let market = market(as_of);
        let sigma = 0.30;

        let mut knockout_option = option(as_of, OptionType::Call, 100.0, sigma);
        knockout_option.knockout = true;
        assert!(!knockout_option.underlying_is_index);
        let mut non_knockout_option = option(as_of, OptionType::Call, 100.0, sigma);
        non_knockout_option.knockout = false;
        assert!(!non_knockout_option.underlying_is_index);

        let knockout_cds = synthetic_underlying_cds(&knockout_option, as_of).expect("ko cds");
        let non_knockout_cds =
            synthetic_underlying_cds(&non_knockout_option, as_of).expect("nko cds");

        let knockout_ctx = context_for(&knockout_option, &market, as_of, sigma);
        let non_knockout_ctx = context_for(&non_knockout_option, &market, as_of, sigma);

        // F_0 is the clean forward swap value and does not include an index
        // front-end-protection term for single-name options.
        let f0_gap = non_knockout_ctx.no_knockout_forward() - knockout_ctx.no_knockout_forward();
        assert!(
            f0_gap.abs() < 1e-12,
            "single-name F_0 must not add index FEP: gap={f0_gap}",
        );

        // Pricing must still reflect the knockout survival multiplier.
        let ko_pv = npv(&knockout_option, &knockout_cds, &market, sigma, as_of)
            .expect("ko npv")
            .amount();
        let nko_pv = npv(
            &non_knockout_option,
            &non_knockout_cds,
            &market,
            sigma,
            as_of,
        )
        .expect("nko npv")
        .amount();
        assert!(
            nko_pv > ko_pv,
            "non-knockout single-name option must price above knockout: ko={ko_pv}, nko={nko_pv}",
        );
    }

    /// Pin the Bloomberg CDSO theta convention so it cannot silently drift
    /// from DOCS 2055833 §2.5 ("shorten the exercise time `t_e` by
    /// `1/365.25`"). On `cdx_ig_46_payer_atm_jun26` the pure-T-shift
    /// formulation is empirically closer to the CDSO screen than the
    /// alternative as-of-shift; see the Phase 5b remediation note in
    /// `tests/golden/data/pricing/cds_option/cdx_ig_46_payer_atm_jun26.json`.
    #[test]
    fn theta_uses_pure_t_shift_with_365_25_denominator() {
        let as_of = date!(2025 - 01 - 01);
        let market = market(as_of);
        let option = option(as_of, OptionType::Call, 100.0, 0.30);
        let cds = synthetic_underlying_cds(&option, as_of).expect("synthetic cds");
        let ctx = context_for(&option, &market, as_of, 0.30);

        let actual = theta(&option, &cds, &market, 0.30, as_of).expect("theta");

        // Reference recomputation: same calibrated m, only shift t_expiry.
        let m = calibrate_lognormal_mean(&ctx).expect("calibration");
        let base = price_with_calibrated_mean(&ctx, m, ctx.t_expiry);
        let expected = {
            let shortened = (ctx.t_expiry - 1.0 / THETA_DAYS_IN_YEAR).max(0.0);
            (price_with_calibrated_mean(&ctx, m, shortened) - base) * option.notional.amount()
        };
        assert!(
            (actual - expected).abs() < 1e-9,
            "theta must use pure-T-shift on the integrand only: actual={actual}, expected={expected}"
        );

        // The 365 vs 365.25 denominator is small but real ($ ~ 1bp/day on
        // realistic notionals); regressing it would silently move every
        // CDSO theta. Lock the difference > 0 so a typo would fail.
        let shortened_365 = (ctx.t_expiry - 1.0 / 365.0).max(0.0);
        let theta_365 =
            (price_with_calibrated_mean(&ctx, m, shortened_365) - base) * option.notional.amount();
        assert!(
            (actual - theta_365).abs() > 0.0,
            "theta with 1/365.25 must differ from theta with 1/365.0; if equal, day basis was changed"
        );
    }

    /// Companion guard: confirm the theta path does NOT propagate the
    /// as-of date through the curves. If a future refactor switches to
    /// as-of-shift, df_to_expiry and survival_to_expiry would change and
    /// this test would diverge from the reference reconstruction above.
    #[test]
    fn theta_does_not_advance_curves_with_as_of_shift() {
        let as_of = date!(2025 - 01 - 01);
        let market = market(as_of);
        let option = option(as_of, OptionType::Call, 100.0, 0.30);
        let cds = synthetic_underlying_cds(&option, as_of).expect("synthetic cds");

        let ctx_today = context_for(&option, &market, as_of, 0.30);
        let ctx_tomorrow = context_for(&option, &market, as_of + time::Duration::days(1), 0.30);

        // The two contexts have meaningfully different df_to_expiry and
        // survival_to_expiry on a flat curve (one fewer day of discount /
        // survival). If theta() were doing as-of-shift, its result would
        // approximately equal:
        //     P_tomorrow(m_tomorrow) - P_today(m_today)
        // We assert the actual theta does NOT match that as-of-shift
        // reconstruction (it would be off by ~10–20% on this fixture).
        let m_today = calibrate_lognormal_mean(&ctx_today).expect("calibration today");
        let m_tomorrow = calibrate_lognormal_mean(&ctx_tomorrow).expect("calibration tomorrow");
        let as_of_shift_theta =
            (price_with_calibrated_mean(&ctx_tomorrow, m_tomorrow, ctx_tomorrow.t_expiry)
                - price_with_calibrated_mean(&ctx_today, m_today, ctx_today.t_expiry))
                * option.notional.amount();

        let actual = theta(&option, &cds, &market, 0.30, as_of).expect("theta");

        // We only require the two formulations to be measurably distinct
        // — the precise gap depends on the curve. Any nontrivial difference
        // (> 1% of the larger magnitude) confirms the implementation is
        // not silently doing as-of-shift.
        let denom = actual.abs().max(as_of_shift_theta.abs()).max(1e-9);
        let rel_gap = (actual - as_of_shift_theta).abs() / denom;
        assert!(
            rel_gap > 0.01,
            "theta() must use pure-T-shift, not as-of-shift: pure_t={actual}, as_of_shift={as_of_shift_theta}, rel_gap={rel_gap}"
        );
    }

    /// Item 3 (audit): the Bloomberg CDSO model intentionally uses TWO
    /// different risky annuities — the bootstrapped term-structure annuity
    /// `bootstrapped_l_at_expiry` for the `F_0` calibration anchor, and the
    /// credit-triangle flat-hazard annuity `flat_annuity` inside the
    /// quadrature integrand. They differ by design (~0.6% NPV on cdx_ig_46).
    ///
    /// The audit flagged this as a possible inconsistency. It is NOT a bug:
    /// the module documentation on `no_knockout_forward` records that making
    /// the annuity consistent (credit-triangle `F_0`) moves the cdx_ig_46
    /// option NPV from +0.61% to +6.5% versus the Bloomberg CDSO screen —
    /// i.e. the dual-annuity design is what reproduces Bloomberg. The
    /// Bloomberg-screen golden `cdx_ig_46_payer_atm_jun26.json` is an
    /// immutable external oracle that locks the current behaviour.
    ///
    /// This test PINS the dual-annuity design: it asserts the two annuities
    /// are genuinely different at the bootstrapped forward par spread, so a
    /// future refactor that "unifies" them (and silently regresses the
    /// Bloomberg reconciliation) fails loudly here instead.
    #[test]
    fn f0_anchor_and_integrand_annuities_are_intentionally_distinct() {
        let as_of = date!(2025 - 01 - 01);
        let market = market(as_of);
        let option = option(as_of, OptionType::Call, 100.0, 0.30);
        let ctx = context_for(&option, &market, as_of, 0.30);

        // The integrand's credit-triangle annuity, evaluated at the same
        // forward par spread that anchors F_0.
        let triangle_annuity = ctx.flat_annuity(ctx.forward_par_spread);
        // The F_0 calibration anchor's bootstrapped term-structure annuity.
        let bootstrapped_annuity = ctx.bootstrapped_l_at_expiry;

        assert!(
            triangle_annuity > 0.0 && bootstrapped_annuity > 0.0,
            "both annuities must be positive: triangle={triangle_annuity}, \
             bootstrapped={bootstrapped_annuity}"
        );
        // They must be DISTINCT — the dual-annuity design is deliberate.
        // If a refactor unifies them this difference collapses to ~0 and the
        // assertion fires, prompting a re-check against the Bloomberg golden.
        let rel_diff = (triangle_annuity - bootstrapped_annuity).abs() / bootstrapped_annuity.abs();
        assert!(
            rel_diff > 1e-4,
            "F_0-anchor and integrand annuities must remain distinct (Bloomberg \
             CDSO dual-annuity design): triangle={triangle_annuity}, \
             bootstrapped={bootstrapped_annuity}, rel_diff={rel_diff}. If this \
             fails, a refactor unified the two annuities — re-verify the \
             cdx_ig_46 Bloomberg golden before accepting the change."
        );
    }
}
