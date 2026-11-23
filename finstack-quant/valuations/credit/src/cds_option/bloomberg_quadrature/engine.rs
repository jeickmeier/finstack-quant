//! Bloomberg CDSO quadrature implementation.

use crate::cds::CreditDefaultSwap;
use crate::cds::pricing::CdsPricer;
use crate::cds_option::CdsOption;
use crate::constants::bloomberg_cdso;
use finstack_quant_core::Result;
use finstack_quant_core::dates::Date;
use finstack_quant_core::market_data::context::MarketContext;
use finstack_quant_core::market_data::term_structures::{DiscountCurve, HazardCurve};
use finstack_quant_core::market_data::traits::Discounting;
use finstack_quant_core::math::ZERO_TOLERANCE;
use finstack_quant_core::math::integration::adaptive_simpson;
use finstack_quant_core::math::solver::BrentSolver;
use finstack_quant_core::math::special_functions::{norm_pdf, norm_pdf_unchecked};
use finstack_quant_core::math::units::BASIS_POINTS_PER_UNIT;
use finstack_quant_core::money::Money;
use finstack_quant_models::types::OptionType;
use rust_decimal::Decimal;

/// Above 10% forward spread, the lognormal-mean calibration is outside the
/// Bloomberg CDSO model's intended liquid-credit regime.
const DISTRESSED_FORWARD_SPREAD_LIMIT: f64 = 0.10;

/// Hard limit on integrand evaluations per half-standard-deviation payoff panel.
///
/// The payoff has one exercise kink, so a panel needs a few hundred samples;
/// the limit only bounds a run that cannot converge.
const PAYOFF_QUADRATURE_MAX_EVALS: usize = 1_000_000;

/// Price a CDS option under the Bloomberg CDSO numerical-quadrature model.
pub fn npv(
    option: &CdsOption,
    cds: &CreditDefaultSwap,
    market: &MarketContext,
    sigma: f64,
    as_of: Date,
) -> Result<Money> {
    let disc = market.get_discount(&option.discount_curve_id)?;
    let surv = market.get_hazard(&option.credit_curve_id)?;

    let ctx = ForwardCdsContext::build(option, disc.as_ref(), surv.as_ref(), cds, as_of, sigma)?;

    // Eq. 2.3: solve `m` so E[V_te(S_te(m))] matches the no-knockout F_0.
    let m = calibrate_lognormal_mean(&ctx)?;

    // Eq. 2.5: O = P(t_e) · E_0[(ξV_te + H(K) + D)+]
    let pv_per_n = price_with_calibrated_mean(&ctx, m, ctx.t_expiry.max(0.0))?;
    Money::from_model_value(
        pv_per_n * option.notional.amount(),
        option.notional.currency(),
    )
}

/// Bloomberg CDSO theta: shorten the exercise time by 1/365.25 while
/// retaining the same calibrated forward price and lognormal mean.
pub fn theta(
    option: &CdsOption,
    cds: &CreditDefaultSwap,
    market: &MarketContext,
    sigma: f64,
    as_of: Date,
) -> Result<f64> {
    let disc = market.get_discount(&option.discount_curve_id)?;
    let surv = market.get_hazard(&option.credit_curve_id)?;
    let ctx = ForwardCdsContext::build(option, disc.as_ref(), surv.as_ref(), cds, as_of, sigma)?;
    if ctx.t_expiry <= 0.0 {
        return Ok(0.0);
    }
    let m = calibrate_lognormal_mean(&ctx)?;
    let base = price_with_calibrated_mean(&ctx, m, ctx.t_expiry)?;
    let shortened_t = (ctx.t_expiry - (1.0 / bloomberg_cdso::THETA_DAYS_IN_YEAR)).max(0.0);
    let bumped = price_with_calibrated_mean(&ctx, m, shortened_t)?;
    Ok((bumped - base) * option.notional.amount())
}

/// Bloomberg CDSO ATM Forward (in basis points) — the bootstrapped forward
/// par spread of the no-knockout forward CDS at expiry.
pub fn forward_par_at_expiry_bp(
    option: &CdsOption,
    cds: &CreditDefaultSwap,
    market: &MarketContext,
    as_of: Date,
) -> Result<f64> {
    let disc = market.get_discount(&option.discount_curve_id)?;
    let surv = market.get_hazard(&option.credit_curve_id)?;
    let ctx = ForwardCdsContext::build(option, disc.as_ref(), surv.as_ref(), cds, as_of, 0.0)?;
    Ok(ctx.forward_par_spread * BASIS_POINTS_PER_UNIT)
}

// Pre-computed deterministic inputs

/// Typed, precomputed strike data for the deterministic exercise term.
///
/// The two variants carry exactly the inputs their payoff branch needs:
///
/// - `Spread`: the strike spread `K` for the annuity-based adjustment
///   `H_spread = ξ (c − K) A(K)` (DOCS 2055833 Eq. 2.4).
/// - `CleanPrice`: the strike clean-price **fraction** `K` (`107.0` wire
///   points become `1.07` exactly once, upstream in
///   [`CdsOptionStrike::clean_price_fraction`]) and the original index
///   factor `f0`, for the direct price term
///   `H_price = ξ (K − 1) · f0 / f` evaluated inside the outer
///   current-factor scale `f`.
///
/// [`CdsOptionStrike::clean_price_fraction`]:
///     super::super::strike::CdsOptionStrike::clean_price_fraction
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum QuadratureStrike {
    /// Forward-spread strike `K` (decimal rate).
    Spread {
        /// Strike spread as a decimal rate.
        strike: f64,
    },
    /// Clean-price strike expressed as a fraction (`1.07`), with the
    /// original index factor `f0` attached to the strike.
    CleanPrice {
        /// Strike clean price as a fraction of par (`107.0%` → `1.07`).
        strike_fraction: f64,
        /// Original index factor `f0` attached to the strike.
        strike_index_factor: f64,
    },
}

/// All deterministic quantities the quadrature integrand and the
/// calibration target need. Built once at the top of `npv()` from the
/// instrument + curves; the calibration loop and the payoff loop both
/// borrow it.
#[doc(hidden)]
pub struct ForwardCdsContext {
    /// `1 − R` on the synthetic underlying.
    pub lgd: f64,
    /// `t_e` in years (`(expiry − as_of)/365`).
    pub t_expiry: f64,
    /// `σ` from the vol surface or instrument override.
    pub sigma: f64,
    /// Discount factor to the contractual exercise-payment date.
    pub df_to_settlement: f64,
    /// Conditional survival probability from valuation date to expiry on
    /// the bootstrapped credit curve.
    pub survival_to_expiry: f64,
    /// Bootstrapped forward spread (decimal), using the clean annuity.
    /// For non-knockout indices this includes expected front-end loss per
    /// unit annuity. Pricing, volatility lookup and spread Greeks share it.
    pub forward_par_spread: f64,
    /// Bootstrapped clean RPV01 of the forward CDS *expressed at expiry*
    /// (i.e. divided by `df_te · q_te`). Used in the F_0 calibration target.
    pub bootstrapped_l_at_expiry: f64,
    /// Year fractions from `t_e` to each post-expiry coupon payment date.
    /// `(payment_date − expiry) / 365`.
    pub times_from_expiry: Vec<f64>,
    /// Premium-leg accrual factors per coupon period in the synthetic
    /// CDS day-count (typically Act/360). One entry per post-expiry
    /// coupon payment.
    pub accrual_factors: Vec<f64>,
    /// Forward discount factors `df(t_pay) / df(t_e)`. One entry per
    /// post-expiry payment.
    pub fwd_discount_factors: Vec<f64>,
    /// Year fraction from the previous coupon date `T_{n(t_e)}` to `t_e`,
    /// in the synthetic CDS day-count. Subtracted from the dirty per-bp
    /// annuity to convert to clean. For forward CDSes whose premium starts
    /// at expiry this is zero.
    pub accrual_pcd_to_expiry: f64,
    /// Index contractual coupon `c` (decimal).
    pub coupon: f64,
    /// Typed, precomputed strike data for the deterministic strike term.
    pub strike: QuadratureStrike,
    /// `ξ = +1` for payer (Call), `−1` for receiver (Put).
    pub option_type: OptionType,
    /// Index-factor scale (1.0 for non-index or original-version index
    /// underlyings).
    pub scale: f64,
    /// Realized index loss per unit of original notional.
    pub realized_loss: f64,
    /// Expected pre-expiry default loss per unit current notional.
    /// Index options include it in the loss-adjusted forward; a non-knockout
    /// single-name payer receives it as a separate default claim.
    pub front_end_protection: f64,
    /// True for index options. Drives `loss_settlement` (settlement of
    /// already-realised index losses and expected front-end protection on
    /// exercise).
    pub is_index: bool,
    /// Whether exercise is conditioned on underlying survival to expiry.
    pub knockout: bool,
}

impl ForwardCdsContext {
    pub fn build(
        option: &CdsOption,
        disc: &DiscountCurve,
        surv: &HazardCurve,
        cds: &CreditDefaultSwap,
        as_of: Date,
        sigma: f64,
    ) -> Result<Self> {
        let cds_pricer = CdsPricer::new();
        // LGD must be strictly positive and finite; recovery is already
        // validated to `(0, 1)` at construction time, but guard against NaN /
        // future construction-path regressions with an explicit error rather
        // than a silent clamp. Checking `is_finite` first is necessary because
        // `NaN <= ZERO_TOLERANCE` is false; using a positive guard
        // (`!is_finite || <= tol`) makes the NaN case explicit.
        let lgd = 1.0 - option.recovery_rate;
        if !lgd.is_finite() || lgd <= ZERO_TOLERANCE {
            return Err(finstack_quant_core::Error::Validation(format!(
                "CDS option recovery_rate={} yields degenerate LGD={:.3e}; \
                 expected recovery in (0, 1) so LGD > {}",
                option.recovery_rate, lgd, ZERO_TOLERANCE
            )));
        }

        let t_expiry = option.time_to_expiry(as_of)?;
        let df_to_expiry = DiscountCurve::df_between_dates(disc, as_of, option.expiry)?;
        let df_to_settlement = DiscountCurve::df_between_dates(
            disc,
            as_of,
            option.exercise_settlement_date.unwrap_or(option.expiry),
        )?;
        let sp_asof_raw = surv.sp_on_date(as_of)?;
        let sp_asof = sp_asof_raw.clamp(ZERO_TOLERANCE, 1.0);
        let sp_expiry_raw = surv.sp_on_date(option.expiry)?;
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
        for (pay_date, accrual) in cashflows.iter() {
            if *pay_date <= option.expiry {
                continue;
            }
            let t_e_to_pay =
                ((*pay_date - option.expiry).whole_days() as f64) / bloomberg_cdso::G_DAYS_IN_YEAR;
            let df_pay = DiscountCurve::df_between_dates(disc, as_of, *pay_date)?;
            if df_to_expiry < ZERO_TOLERANCE {
                return Err(finstack_quant_core::Error::Validation(format!(
                    "degenerate forward discount factor at t_expiry: df_to_expiry={df_to_expiry:.3e}"
                )));
            }
            let fwd_df = df_pay / df_to_expiry;
            let sp_pay_uncond = surv.sp_on_date(*pay_date)?.clamp(0.0, 1.0);
            let sp_pay_cond = (sp_pay_uncond / sp_asof).clamp(0.0, 1.0);
            times_from_expiry.push(t_e_to_pay);
            accrual_factors.push(*accrual);
            fwd_discount_factors.push(fwd_df);
            let cf_pv01 = *accrual * df_pay * sp_pay_cond;
            raw_annuity_at_value_dt_no_aod += cf_pv01;
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
        let pcd = pcd.unwrap_or_else(|| cds.premium_leg.start.min(option.expiry));
        let accrual_pcd_to_expiry = if pcd >= option.expiry {
            0.0
        } else {
            finstack_quant_core::dates::DayCount::year_fraction(
                cds.premium_leg.day_count,
                pcd,
                option.expiry,
                finstack_quant_core::dates::DayCountContext::default(),
            )?
        };

        // Both the curve anchor and spread-state annuity use the same
        // post-expiry coupons and clean accrued-premium subtraction. Their
        // survival probabilities differ: observed term structure for the
        // anchor, the model's flat spread state inside the expectation.
        let survival_denom = if option.knockout || !option.underlying_is_index {
            survival_to_expiry
        } else {
            1.0
        };
        let denom_te = df_to_expiry * survival_denom;
        if denom_te <= ZERO_TOLERANCE {
            return Err(finstack_quant_core::Error::Validation(
                "CDS option has no surviving discounted exercise notional".into(),
            ));
        }
        let pcd_stub_at_value_dt = accrual_pcd_to_expiry * df_to_expiry * survival_to_expiry;
        let risky_annuity_at_value_dt = raw_annuity_at_value_dt_no_aod - pcd_stub_at_value_dt;
        let bootstrapped_l_at_expiry = risky_annuity_at_value_dt / denom_te;
        if risky_annuity_at_value_dt <= ZERO_TOLERANCE {
            return Err(finstack_quant_core::Error::Validation(format!(
                "CDS option '{}' has non-positive forward clean risky annuity {}",
                option.id, risky_annuity_at_value_dt,
            )));
        }

        // The delivered CDS protects only from expiry onward. In particular,
        // a knockout option cannot receive any pre-expiry default payment.
        // Index front-end loss is added exactly once in the exercise payoff.
        let mut forward_cds =
            super::super::pricer::cds_with_bloomberg_protection_end_extension(cds);
        forward_cds.protection_effective_date = Some(cds.protection_start().max(option.expiry));
        let forward_protection_pv = cds_pricer
            .pv_protection_leg(&forward_cds, disc, surv, as_of)?
            .amount();
        let mut forward_par_spread =
            forward_protection_pv / (risky_annuity_at_value_dt * cds.notional.amount());

        let coupon = (option.effective_coupon_bp()? / Decimal::new(10_000, 0)).as_f64();
        let strike = match &option.strike {
            super::super::strike::CdsOptionStrike::Spread(spread) => QuadratureStrike::Spread {
                strike: spread.as_f64(),
            },
            price @ super::super::strike::CdsOptionStrike::CleanPricePct(_) => {
                let strike_index_factor = option.strike_index_factor.ok_or_else(|| {
                    finstack_quant_core::Error::Validation(format!(
                        "CDS option '{}' has a clean-price strike but no \
                         strike_index_factor; construction-time validation \
                         should have rejected this instrument",
                        option.id
                    ))
                })?;
                QuadratureStrike::CleanPrice {
                    strike_fraction: price.clean_price_fraction()?,
                    strike_index_factor,
                }
            }
        };
        let scale = if option.underlying_is_index {
            option.index_factor
        } else {
            1.0
        };
        let realized_loss = option.realized_loss;
        // A non-knockout option survives a pre-expiry default, so it carries
        // the loss from any default between `as_of` and expiry, whatever the
        // delivered CDS's own protection start.
        let front_end_protection = if !option.knockout {
            let fep_start = as_of;
            if fep_start >= option.expiry {
                0.0
            } else {
                let sp_start = surv.sp_on_date(fep_start)?.clamp(ZERO_TOLERANCE, 1.0);
                // Adjacent front-end and delivered-CDS windows share expiry
                // as their boundary, with no overlap or omitted day.
                let sp_end = surv.sp_on_date(option.expiry)?.clamp(0.0, 1.0);
                lgd * (1.0 - (sp_end / sp_start).clamp(0.0, 1.0))
            }
        } else {
            0.0
        };

        // Index ATM spread is loss-adjusted. Store that same coordinate for
        // pricing, volatility lookup and spread Greeks; do not add FEP again
        // when constructing the calibration target or exercise payoff.
        if option.underlying_is_index {
            forward_par_spread += front_end_protection / bootstrapped_l_at_expiry;
        }

        Ok(Self {
            lgd,
            t_expiry,
            sigma,
            df_to_settlement,
            survival_to_expiry,
            forward_par_spread,
            bootstrapped_l_at_expiry,
            times_from_expiry,
            accrual_factors,
            fwd_discount_factors,
            accrual_pcd_to_expiry,
            coupon,
            strike,
            option_type: option.option_type,
            scale,
            realized_loss,
            front_end_protection,
            is_index: option.underlying_is_index,
            knockout: option.knockout,
        })
    }

    /// `ξ` per Eq. 2.1 / Eq. 2.4: `+1` payer, `−1` receiver.
    pub fn sign(&self) -> f64 {
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
    pub fn flat_annuity(&self, s: f64) -> f64 {
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
    pub fn swap_value_per_n(&self, s: f64) -> f64 {
        (s - self.coupon) * self.flat_annuity(s)
    }

    /// Deterministic strike adjustment, per unit **current** notional
    /// (the outer index-factor scale `f` is applied by
    /// [`quadrature_payoff`]).
    ///
    /// - Spread strike (DOCS 2055833 Eq. 2.4):
    ///   `H_spread = ξ (c − K) · A(K)`.
    /// - Clean-price strike: `H_price = ξ (K − 1) · f0 / f`, the direct
    ///   deterministic price term. The strike price is quoted on the
    ///   original factor `f0` notional while the payoff is scaled by the
    ///   current factor `f`, hence the `f0 / f` ratio inside the scale.
    ///   Realized index losses stay exclusively in
    ///   [`Self::signed_loss_settlement_per_n`]; folding them into an
    ///   adjusted strike here as well would double-count them.
    pub fn signed_strike_adjustment_per_n(&self) -> f64 {
        match self.strike {
            QuadratureStrike::Spread { strike } => {
                self.sign() * (self.coupon - strike) * self.flat_annuity(strike)
            }
            QuadratureStrike::CleanPrice {
                strike_fraction,
                strike_index_factor,
            } => {
                let scale = self.scale.max(ZERO_TOLERANCE);
                self.sign() * (strike_fraction - 1.0) * strike_index_factor / scale
            }
        }
    }

    /// The strike spread for Black-formula screen metrics, failing for
    /// clean-price strikes (whose delta/gamma use curve-reprice hedge
    /// semantics instead of the spread `d₁`).
    pub fn spread_strike(&self) -> Result<f64> {
        match self.strike {
            QuadratureStrike::Spread { strike } => Ok(strike),
            QuadratureStrike::CleanPrice {
                strike_fraction, ..
            } => Err(finstack_quant_core::Error::Validation(format!(
                "Black spread d1 metrics are not defined for a clean-price \
                     strike (K = {:.6} fraction); use the curve-reprice \
                     price-strike metrics",
                strike_fraction
            ))),
        }
    }

    /// Eq. 2.5 deterministic loss settlement, per unit current notional
    /// before the index-factor scale is applied.
    pub fn signed_loss_settlement_per_n(&self) -> f64 {
        if !self.is_index {
            return 0.0;
        }
        let scale = self.scale.max(ZERO_TOLERANCE);
        self.sign() * self.realized_loss / scale
    }

    /// Knockout options exercise only if the underlying survives to expiry.
    pub fn exercise_survival_multiplier(&self) -> f64 {
        if self.knockout || !self.is_index {
            self.survival_to_expiry
        } else {
            1.0
        }
    }

    /// `F_0/N` — the clean forward swap value, used as the calibration anchor
    /// for `m` (DOCS 2055833 Eq 2.3).
    ///
    /// The calibration anchor is `(s_par − c) · L_te`; the stored index
    /// spread already includes front-end loss per unit annuity exactly once. Only already-realized index losses enter the separate exercise
    /// settlement term.
    ///
    /// The curve anchor uses observed survival; the random spread-state
    /// annuity uses the model's flat hazard. The same coupon dates, accrued
    /// premium, and forward protection window apply to both. Calibration
    /// enforces the expectation identity rather than substituting a flat
    /// hazard for the observed curve.
    pub fn forward_value(&self) -> f64 {
        (self.forward_par_spread - self.coupon) * self.bootstrapped_l_at_expiry
    }
}

// Calibration of the lognormal mean `m` (DOCS 2055833 Eq. 2.3)

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
///
/// # Arguments
///
/// * `ctx` - Forward CDS valuation inputs including the decimal par spread,
///   volatility, time to expiry in years, and the observed forward value.
///
/// # Errors
///
/// Returns an error for unsupported distressed spreads, an invalid quadrature
/// grid or value, or a failed bracket or solve.
pub fn calibrate_lognormal_mean(ctx: &ForwardCdsContext) -> Result<f64> {
    if ctx.forward_par_spread > DISTRESSED_FORWARD_SPREAD_LIMIT {
        return Err(finstack_quant_core::Error::Validation(format!(
            "distressed CDSO calibration unsupported: forward_par_spread={:.6} exceeds {:.6}",
            ctx.forward_par_spread, DISTRESSED_FORWARD_SPREAD_LIMIT
        )));
    }

    let target = ctx.forward_value();
    let t_expiry = ctx.t_expiry.max(0.0);
    let s0 = (-0.5 * ctx.sigma * ctx.sigma * t_expiry).exp();
    let sigma_sqrt_t = ctx.sigma * t_expiry.sqrt();

    let expected_v_te = |m: f64| -> Result<f64> {
        normal_integral(
            bloomberg_cdso::Z_STEP,
            z_limit(ctx.sigma, ctx.t_expiry),
            |z| {
                let s = m * s0 * (sigma_sqrt_t * z).exp();
                ctx.swap_value_per_n(s)
            },
        )
    };

    let f = |log_m: f64| -> Result<f64> { Ok(expected_v_te(log_m.exp())? - target) };

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
    let f_seed = f(log_seed)?;
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
            let f_new = f(x_lo_new)?;
            if f_new.is_finite() && f_new * lo_f <= 0.0 {
                bracket = Some((x_lo_new, lo_x));
                break;
            }
            lo_x = x_lo_new;
            lo_f = f_new;
        }
        if x_hi_new > hi_x {
            let f_new = f(x_hi_new)?;
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
        return Err(finstack_quant_core::Error::Validation(format!(
            "calibration bracket violation: target={target}, seed={m_seed}, \
             f(m_min)={lo_f:.6e}, f(m_max)={hi_f:.6e}",
        )));
    };
    let solver = BrentSolver::new(1e-12, 100)?;
    let log_m = solver.solve_in_bracket(f, bracket_lo, bracket_hi)?;
    Ok(log_m.exp())
}

// Quadrature integrand (DOCS 2055833 Eq. 2.5)

/// `O / N = P(t_e) · E_0 [ (ξ V_te + H(K) + D)+ ]` per Eq. 2.5, evaluated
/// by adaptive integration against the standard normal density. The `scale` factor
/// folds in the index-factor adjustment for re-versioned indices.
///
/// # Arguments
///
/// - `ctx`: Curve-derived forward, coupon, strike, survival and payment inputs.
/// - `m`: Calibrated mean of the lognormal decimal spread state; strictly positive.
/// - `t_expiry`: Remaining spread-variance horizon in Actual/365F years. Theta
///   may shorten it while holding the calibrated forward and curves fixed.
///
/// # Errors
///
/// Returns an integration error when the payoff is non-finite or adaptive
/// quadrature cannot meet its absolute error target of `1e-12` per notional.
pub fn price_with_calibrated_mean(ctx: &ForwardCdsContext, m: f64, t_expiry: f64) -> Result<f64> {
    quadrature_payoff(
        ctx,
        m,
        ctx.signed_strike_adjustment_per_n(),
        ctx.signed_loss_settlement_per_n(),
        t_expiry,
    )
}

/// Integrate the signed exercise payoff and add any single-name default claim.
///
/// # Arguments
///
/// - `ctx`: Curve-derived forward, volatility, survival and payment inputs.
/// - `m`: Calibrated positive mean of the lognormal decimal spread state.
/// - `h_k`: Signed deterministic strike adjustment per current notional.
/// - `d_loss`: Signed realized-loss exercise payment per current notional.
/// - `t_expiry`: Spread-variance horizon in Actual/365F years; negative values
///   are treated as zero, giving a deterministic spread state.
///
/// # Errors
///
/// Returns an integration error for a non-finite payoff or unmet quadrature
/// tolerance, rather than returning an unconverged option value.
pub fn quadrature_payoff(
    ctx: &ForwardCdsContext,
    m: f64,
    h_k: f64,
    d_loss: f64,
    t_expiry: f64,
) -> Result<f64> {
    let t_expiry = t_expiry.max(0.0);
    let s0 = (-0.5 * ctx.sigma * ctx.sigma * t_expiry).exp();
    let sigma_sqrt_t = ctx.sigma * t_expiry.sqrt();
    let sign = ctx.sign();

    let limit = z_limit(ctx.sigma, t_expiry);
    let density_weighted_payoff = |z: f64| {
        let s = m * s0 * (sigma_sqrt_t * z).exp();
        (sign * ctx.swap_value_per_n(s) + h_k + d_loss).max(0.0) * norm_pdf_unchecked(z)
    };
    // Refine the exercise kink instead of sampling it on the calibration
    // grid. Seed half-standard-deviation panels so a small tail exercise
    // region cannot be missed by the initial Simpson samples.
    let panels = (4.0 * limit).ceil() as usize;
    let width = 2.0 * limit / panels as f64;
    let mut expected_payoff = 0.0;
    for panel in 0..panels {
        let low = -limit + panel as f64 * width;
        expected_payoff += adaptive_simpson(
            |z| Ok(density_weighted_payoff(z)),
            low,
            low + width,
            1e-12 / panels as f64,
            32,
            PAYOFF_QUADRATURE_MAX_EVALS,
        )?;
    }
    let default_payment = if !ctx.is_index && !ctx.knockout && ctx.option_type == OptionType::Call {
        ctx.front_end_protection
    } else {
        0.0
    };
    Ok(
        (ctx.scale * ctx.exercise_survival_multiplier() * expected_payoff + default_payment)
            * ctx.df_to_settlement,
    )
}

/// Bloomberg CDSO standard-normal quadrature half-width.
///
/// DOCS 2055833 and Bloomberg-derived references use a fixed `[-6, 6]`
/// driver grid for normal market vols. The legacy `4·σ√t` guard is retained
/// only for extreme stress inputs so the integration range never narrows
/// relative to the prior high-vol protection.
///
/// # Arguments
///
/// * `sigma` - Annualized lognormal spread volatility as a decimal.
/// * `t_expiry` - Remaining volatility horizon in years, floored at zero.
#[doc(hidden)]
pub fn z_limit(sigma: f64, t_expiry: f64) -> f64 {
    bloomberg_cdso::MIN_Z_LIMIT.max(4.0 * sigma * t_expiry.max(0.0).sqrt())
}

/// Integrate a function against the standard-normal density on `[-limit, limit]`.
///
/// # Arguments
///
/// * `step` - Positive target spacing in standard deviations. The nearest whole
///   number of trapezoids spans the requested integration interval.
/// * `limit` - Positive finite integration half-width in standard deviations.
/// * `value_at` - Finite conditional value evaluated at each normal variate.
///
/// # Errors
///
/// Returns an error for an unrepresentable grid, non-finite node value, or sum overflow.
#[doc(hidden)]
pub fn normal_integral<F>(step: f64, limit: f64, mut value_at: F) -> Result<f64>
where
    F: FnMut(f64) -> f64,
{
    if !step.is_finite() || step <= 0.0 || !limit.is_finite() || limit <= 0.0 {
        return Err(finstack_quant_core::Error::Validation(
            "normal quadrature requires positive finite node spacing and limit".into(),
        ));
    }
    let count = ((2.0 * limit) / step).round();
    if !(1.0..usize::MAX as f64).contains(&count) {
        return Err(finstack_quant_core::Error::Validation(
            "normal quadrature requires a representable positive panel count".into(),
        ));
    }
    let n_steps = count as usize;
    let width = 2.0 * limit / count;
    let mut sum = finstack_quant_core::math::summation::NeumaierAccumulator::new();
    for i in 0..=n_steps {
        let z = if i == n_steps {
            limit
        } else {
            -limit + (i as f64) * width
        };
        let weight = if i == 0 || i == n_steps { 0.5 } else { 1.0 };
        sum.add(weight * width * norm_pdf(z)? * value_at(z))?;
    }
    sum.get_total()
}
