//! Bloomberg CDSO numerical-quadrature pricer.
//!
//! Implements the model published in:
//!
//! - Bloomberg L.P. Quantitative Analytics. *Pricing Credit Index Options.*
//!   DOCS 2055833 ⟨GO⟩, March 2012.
//!   `docs/REFERENCES.md#bloomberg-cdso`
//!
//! and uses the Bloomberg CDS pricer (DOCS 2057273,
//! `docs/REFERENCES.md#bloomberg-cds-model`) for the bootstrapped
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
//! - For index CDS options, expected front-end protection enters the
//!   loss-adjusted calibration anchor `F_0` exactly once. The deterministic
//!   exercise term `D` contains only already-realized losses.
//!
//! The curve anchor and random-state annuity share contractual coupon dates
//! and clean accrued premium. The anchor uses observed survival; the
//! stochastic state uses the model's flat hazard. The calibrated expectation
//! equals the forward CDS value. Future protection starts at expiry and
//! front-end protection ends there. Only knockout options condition that
//! forward value on survival to expiry.
//!
//! # Numerical integration
//!
//! The smooth calibration expectation uses a trapezoidal normal-driver
//! grid with step `Δz = 0.05`. The option payoff has an exercise kink, so
//! it uses adaptive Simpson integration with a total absolute error target
//! of `1e-12` per unit notional. Integration failures propagate to callers.
//!
//! All time inputs use **calendar days / 365** (DOCS 2055833 §2.1, matching
//! FinancePy's `bloomberg_cdso::G_DAYS_IN_YEAR = 365.0`). Premium-leg accrual factors come
//! from the synthetic underlying CDS in its native day count (Act/360 for
//! USD CDX/iTraxx Main).

use crate::constants::{bloomberg_cdso, numerical, BASIS_POINTS_PER_UNIT};
use crate::instruments::common_impl::parameters::OptionType;
use crate::instruments::credit_derivatives::cds::pricing::CDSPricer;
use crate::instruments::credit_derivatives::cds::CreditDefaultSwap;
use crate::instruments::credit_derivatives::cds_option::CDSOption;
use finstack_quant_core::dates::Date;
use finstack_quant_core::market_data::context::MarketContext;
use finstack_quant_core::market_data::term_structures::{DiscountCurve, HazardCurve};
use finstack_quant_core::math::integration::adaptive_simpson;
use finstack_quant_core::math::solver::BrentSolver;
use finstack_quant_core::money::Money;
use finstack_quant_core::Result;
use rust_decimal::prelude::ToPrimitive;
use rust_decimal::Decimal;

/// `1/√(2π)` — the standard normal density's normalising constant.
const INV_SQRT_2_PI: f64 = 0.398_942_280_401_432_7_f64;
/// Above 10% forward spread, the lognormal-mean calibration is outside the
/// Bloomberg CDSO model's intended liquid-credit regime.
const DISTRESSED_FORWARD_SPREAD_LIMIT: f64 = 0.10;

/// Price a CDS option under the Bloomberg CDSO numerical-quadrature model.
pub fn npv(
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
    let pv_per_n = price_with_calibrated_mean(&ctx, m, ctx.t_expiry.max(0.0))?;
    Money::new(
        pv_per_n * option.notional.amount(),
        option.notional.currency(),
    )
}

/// Bloomberg CDSO theta: shorten the exercise time by 1/365.25 while
/// retaining the same calibrated forward price and lognormal mean.
pub fn theta(
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
    let base = price_with_calibrated_mean(&ctx, m, ctx.t_expiry)?;
    let shortened_t = (ctx.t_expiry - (1.0 / bloomberg_cdso::THETA_DAYS_IN_YEAR)).max(0.0);
    let bumped = price_with_calibrated_mean(&ctx, m, shortened_t)?;
    Ok((bumped - base) * option.notional.amount())
}

/// Bloomberg CDSO ATM Forward (in basis points) — the bootstrapped forward
/// par spread of the no-knockout forward CDS at expiry.
pub fn forward_par_at_expiry_bp(
    option: &CDSOption,
    cds: &CreditDefaultSwap,
    curves: &MarketContext,
    as_of: Date,
) -> Result<f64> {
    let disc = curves.get_discount(&option.discount_curve_id)?;
    let surv = curves.get_hazard(&option.credit_curve_id)?;
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
///   [`CDSOptionStrike::clean_price_fraction`]) and the original index
///   factor `f0`, for the direct price term
///   `H_price = ξ (K − 1) · f0 / f` evaluated inside the outer
///   current-factor scale `f`.
///
/// [`CDSOptionStrike::clean_price_fraction`]:
///     super::strike::CDSOptionStrike::clean_price_fraction
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
    pub realized_index_loss: f64,
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
            return Err(finstack_quant_core::Error::Validation(format!(
                "CDS option recovery_rate={} yields degenerate LGD={:.3e}; \
                 expected recovery in (0, 1) so LGD > {}",
                option.recovery_rate,
                lgd,
                numerical::ZERO_TOLERANCE
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
        let sp_asof = sp_asof_raw.clamp(numerical::ZERO_TOLERANCE, 1.0);
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
            if df_to_expiry < numerical::ZERO_TOLERANCE {
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
        let pcd = pcd.unwrap_or_else(|| cds.premium.start.min(option.expiry));
        let accrual_pcd_to_expiry = if pcd >= option.expiry {
            0.0
        } else {
            finstack_quant_core::dates::DayCount::year_fraction(
                cds.premium.day_count,
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
        if denom_te <= numerical::ZERO_TOLERANCE {
            return Err(finstack_quant_core::Error::Validation(
                "CDS option has no surviving discounted exercise notional".into(),
            ));
        }
        let pcd_stub_at_value_dt = accrual_pcd_to_expiry * df_to_expiry * survival_to_expiry;
        let risky_annuity_at_value_dt = raw_annuity_at_value_dt_no_aod - pcd_stub_at_value_dt;
        let bootstrapped_l_at_expiry = risky_annuity_at_value_dt / denom_te;
        if risky_annuity_at_value_dt <= numerical::ZERO_TOLERANCE {
            return Err(finstack_quant_core::Error::Validation(format!(
                "CDS option '{}' has non-positive forward clean risky annuity {}",
                option.id, risky_annuity_at_value_dt,
            )));
        }

        // The delivered CDS protects only from expiry onward. In particular,
        // a knockout option cannot receive any pre-expiry default payment.
        // Index front-end loss is added exactly once in the exercise payoff.
        let mut forward_cds = super::pricer::cds_with_bloomberg_protection_end_extension(cds);
        forward_cds.protection_effective_date = Some(cds.protection_start().max(option.expiry));
        let forward_protection_pv = cds_pricer
            .pv_protection_leg(&forward_cds, disc, surv, as_of)?
            .amount();
        let mut forward_par_spread =
            forward_protection_pv / (risky_annuity_at_value_dt * cds.notional.amount());

        let coupon = decimal_to_f64(option.effective_underlying_cds_coupon()?)?;
        let strike = match &option.strike {
            super::strike::CDSOptionStrike::Spread(spread) => QuadratureStrike::Spread {
                strike: decimal_to_f64(*spread)?,
            },
            price @ super::strike::CDSOptionStrike::CleanPricePct(_) => {
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
            option.index_factor.unwrap_or(1.0)
        } else {
            1.0
        };
        let realized_index_loss = option.realized_index_loss.unwrap_or(0.0);
        let front_end_protection = if !option.knockout {
            let fep_start = as_of;
            if fep_start >= option.expiry {
                0.0
            } else {
                let sp_start = surv
                    .sp_on_date(fep_start)?
                    .clamp(numerical::ZERO_TOLERANCE, 1.0);
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
            realized_index_loss,
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
                let scale = self.scale.max(numerical::ZERO_TOLERANCE);
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
        let scale = self.scale.max(numerical::ZERO_TOLERANCE);
        self.sign() * self.realized_index_loss / scale
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

    /// Native ATM-forward clean-price coordinate in percentage points, for
    /// moneyness and volatility-surface selection on price-struck index
    /// options.
    ///
    /// Derived from payer/receiver parity under the exact price-strike
    /// payoff used by the quadrature — not from a separate price
    /// approximation. With `F0 = E[V_te]` from the lognormal-mean
    /// calibration anchor, payer − receiver telescopes to
    /// `df · (f·F0 + (K − 1)·f0 + L)`, so the parity strike is
    ///
    /// ```text
    /// K_ATM = 1 − (f·F0 + L) / f0
    /// ```
    ///
    /// returned as `100 · K_ATM`. In the limiting case `f = f0 = 1`,
    /// `L = 0`, `FEP = 0` this reduces to `K_ATM = 1 − F0`.
    ///
    /// # Errors
    ///
    /// Returns a validation error for spread-struck contexts: the
    /// coordinate needs the strike's original factor `f0`.
    pub fn native_atm_forward_clean_price_pct(&self) -> Result<f64> {
        let QuadratureStrike::CleanPrice {
            strike_index_factor,
            ..
        } = self.strike
        else {
            return Err(finstack_quant_core::Error::Validation(
                "the native ATM clean-price coordinate requires a clean-price strike \
                 (it scales by the strike's original index factor f0)"
                    .to_string(),
            ));
        };
        let f = self.scale.max(numerical::ZERO_TOLERANCE);
        let f0 = strike_index_factor.max(numerical::ZERO_TOLERANCE);
        let k_atm = 1.0 - (f * self.forward_value() + self.realized_index_loss) / f0;
        Ok(100.0 * k_atm)
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

    let expected_v_te = |m: f64| -> f64 {
        normal_integral(
            bloomberg_cdso::Z_STEP,
            z_limit(ctx.sigma, ctx.t_expiry),
            |z| {
                let s = m * s0 * (sigma_sqrt_t * z).exp();
                ctx.swap_value_per_n(s)
            },
        )
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
        return Err(finstack_quant_core::Error::Validation(format!(
            "calibration bracket violation: target={target}, seed={m_seed}, \
             f(m_min)={lo_f:.6e}, f(m_max)={hi_f:.6e}",
        )));
    };
    let solver = BrentSolver::new().tolerance(1e-12);
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
        (sign * ctx.swap_value_per_n(s) + h_k + d_loss).max(0.0)
            * (-0.5 * z * z).exp()
            * INV_SQRT_2_PI
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
            density_weighted_payoff,
            low,
            low + width,
            1e-12 / panels as f64,
            32,
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

fn decimal_to_f64(value: Decimal) -> Result<f64> {
    value.to_f64().ok_or_else(|| {
        finstack_quant_core::Error::Validation(format!(
            "Bloomberg CDSO quadrature: cannot represent {value} as f64"
        ))
    })
}

/// Bloomberg CDSO standard-normal quadrature half-width.
///
/// DOCS 2055833 and Bloomberg-derived references use a fixed `[-6, 6]`
/// driver grid for normal market vols. The legacy `4·σ√t` guard is retained
/// only for extreme stress inputs so the integration range never narrows
/// relative to the prior high-vol protection.
#[doc(hidden)]
pub fn z_limit(sigma: f64, t_expiry: f64) -> f64 {
    bloomberg_cdso::MIN_Z_LIMIT.max(4.0 * sigma * t_expiry.max(0.0).sqrt())
}

/// Integrate a function against the standard-normal density on `[-limit, limit]`.
#[doc(hidden)]
pub fn normal_integral<F>(step: f64, limit: f64, mut value_at: F) -> f64
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
    use crate::instruments::credit_derivatives::cds_option::parameters::CDSOptionParams;
    use crate::instruments::credit_derivatives::cds_option::pricer::synthetic_underlying_cds;
    use crate::instruments::CreditParams;
    use finstack_quant_core::currency::Currency;
    use finstack_quant_core::dates::DateExt;
    use finstack_quant_core::market_data::context::MarketContext;
    use finstack_quant_core::market_data::term_structures::{DiscountCurve, HazardCurve};
    use rust_decimal::Decimal;
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
            super::super::strike::CDSOptionStrike::Spread(bp_to_decimal(strike_bp)),
            as_of.add_months(12),
            as_of.add_months(60),
            Money::from((10_000_000_i64, Currency::USD)),
            option_type,
        )
        .expect("valid option params")
        .with_underlying_cds_coupon(bp_to_decimal(coupon_bp));
        let credit = CreditParams::corporate_standard("SN", "HZ-SN");
        let mut option = CDSOption::new("CDSO-UNIT", &params, &credit, "USD-OIS", "CDSO-VOL")
            .expect("valid cds option");
        option
            .instrument_pricing_overrides
            .market_quotes
            .implied_volatility = Some(vol);
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
        let exercise = ctx.scale
            * ctx.exercise_survival_multiplier()
            * (ctx.sign() * ctx.forward_value()
                + ctx.signed_strike_adjustment_per_n()
                + ctx.signed_loss_settlement_per_n())
            .max(0.0);
        let default_payment =
            if !ctx.is_index && !ctx.knockout && ctx.option_type == OptionType::Call {
                ctx.front_end_protection
            } else {
                0.0
            };
        (exercise + default_payment) * ctx.df_to_settlement
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
        let k = ctx
            .spread_strike()
            .expect("Black-76 helper requires a spread strike")
            .max(numerical::ZERO_TOLERANCE);
        let vol_sqrt_t = ctx.sigma * ctx.t_expiry.sqrt();
        let d1 = ((f / k).ln() + 0.5 * vol_sqrt_t * vol_sqrt_t) / vol_sqrt_t;
        let d2 = d1 - vol_sqrt_t;
        ctx.df_to_settlement
            * ctx.exercise_survival_multiplier()
            * ctx.bootstrapped_l_at_expiry
            * (f * normal_cdf(d1) - k * normal_cdf(d2))
    }

    #[test]
    fn normal_quadrature_converges_when_step_is_halved() {
        let coarse = normal_integral(0.05, bloomberg_cdso::MIN_Z_LIMIT, |z| {
            (0.30 * z).exp().max(0.0)
        });
        let fine = normal_integral(0.025, bloomberg_cdso::MIN_Z_LIMIT, |z| {
            (0.30 * z).exp().max(0.0)
        });
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
        assert_eq!(
            z_limit(cdx_ig_46_sigma, cdx_ig_46_t),
            bloomberg_cdso::MIN_Z_LIMIT
        );

        let stressed_sigma = 2.0_f64;
        let stressed_t = 4.0_f64;
        let stressed_limit = z_limit(stressed_sigma, stressed_t);
        assert!(
            (stressed_limit - 16.0).abs() < 1e-12,
            "extreme vol-time inputs should still use the legacy 4σ√t guard, got {stressed_limit}"
        );
    }

    #[test]
    fn index_fep_is_independent_of_premium_payment_date() {
        let as_of = date!(2025 - 01 - 02);
        let mut option = option(as_of, OptionType::Call, 100.0, 0.30);
        option.underlying_is_index = true;
        option.knockout = false;
        let market = market(as_of);
        let expected = context_for(&option, &market, as_of, 0.3).front_end_protection;
        for settlement in [date!(2025 - 01 - 09), date!(2025 - 07 - 01)] {
            option.cash_settlement_date = Some(settlement);
            assert_eq!(
                context_for(&option, &market, as_of, 0.3).front_end_protection,
                expected
            );
        }
    }

    #[test]
    fn index_front_end_protection_is_recomputed_on_bumped_curve() {
        let as_of = date!(2025 - 01 - 01);
        let mut option = option(as_of, OptionType::Call, 100.0, 0.30);
        option.underlying_is_index = true;
        option.knockout = false;
        option.index_factor = Some(1.0);
        option.underlying_cds_coupon = Some(bp_to_decimal(100.0));

        let market = market(as_of);
        let base_ctx = context_for(&option, &market, as_of, 0.30);
        let hazard = market.get_hazard(&option.credit_curve_id).expect("hazard");
        let bumped_hazard = hazard
            .with_parallel_hazard_rate_bump_bp(1.0)
            .expect("bumped hazard");
        let bumped_market = market.insert(bumped_hazard);
        let bumped_ctx = context_for(&option, &bumped_market, as_of, 0.30);

        assert!(
            bumped_ctx.front_end_protection > base_ctx.front_end_protection,
            "FEP should be rebuilt from the bumped hazard curve: base={}, bumped={}",
            base_ctx.front_end_protection,
            bumped_ctx.front_end_protection,
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
    fn calibration_rejects_distressed_forward_spreads() {
        let as_of = date!(2025 - 01 - 01);
        let distressed_market = MarketContext::new()
            .insert(flat_discount("USD-OIS", as_of, 0.03))
            .insert(flat_hazard("HZ-SN", as_of, 0.4, 0.25));
        let option = option(as_of, OptionType::Call, 100.0, 0.30);
        let ctx = context_for(&option, &distressed_market, as_of, 0.30);

        let err = calibrate_lognormal_mean(&ctx).expect_err("distressed calibration should fail");

        assert!(
            err.to_string().contains("distressed"),
            "error should explain distressed-credit calibration guard: {err}"
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
                * call_ctx.df_to_settlement
                * (call_ctx.forward_value()
                    + call_ctx.signed_strike_adjustment_per_n()
                    + call_ctx.signed_loss_settlement_per_n())
                + call.notional.amount()
                    * call_ctx.df_to_settlement
                    * call_ctx.front_end_protection;

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
            let strike_adj = call_ctx.signed_strike_adjustment_per_n();
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
                * call_ctx.df_to_settlement
                * (call_ctx.forward_value()
                    + call_ctx.signed_strike_adjustment_per_n()
                    + call_ctx.signed_loss_settlement_per_n())
                + call.notional.amount()
                    * call_ctx.df_to_settlement
                    * call_ctx.front_end_protection;

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
        let f0_gap = non_knockout_ctx.forward_value() - knockout_ctx.forward_value();
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
    /// `tests/golden/data/pricing/bloomberg/cds_option/cdx_ig_46_payer_atm_jun26.json`.
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
        let base = price_with_calibrated_mean(&ctx, m, ctx.t_expiry).expect("payoff integration");
        let expected = {
            let shortened = (ctx.t_expiry - 1.0 / bloomberg_cdso::THETA_DAYS_IN_YEAR).max(0.0);
            (price_with_calibrated_mean(&ctx, m, shortened).expect("payoff integration") - base)
                * option.notional.amount()
        };
        assert!(
            (actual - expected).abs() < 1e-9,
            "theta must use pure-T-shift on the integrand only: actual={actual}, expected={expected}"
        );

        // The 365 vs 365.25 denominator is small but real ($ ~ 1bp/day on
        // realistic notionals); regressing it would silently move every
        // CDSO theta. Lock the difference > 0 so a typo would fail.
        let shortened_365 = (ctx.t_expiry - 1.0 / 365.0).max(0.0);
        let theta_365 = (price_with_calibrated_mean(&ctx, m, shortened_365)
            .expect("payoff integration")
            - base)
            * option.notional.amount();
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
                .expect("payoff integration")
                - price_with_calibrated_mean(&ctx_today, m_today, ctx_today.t_expiry)
                    .expect("payoff integration"))
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

    /// With a flat observed hazard, identical survival inputs must give
    /// the same annuity in the curve anchor and the spread-state model.
    #[test]
    fn curve_and_state_annuities_share_the_same_cashflow_convention() {
        let as_of = date!(2025 - 01 - 01);
        let market = market(as_of);
        let mut option = option(as_of, OptionType::Call, 100.0, 0.30);
        option.knockout = true;
        let ctx = context_for(&option, &market, as_of, 0.30);
        let triangle_annuity = ctx.flat_annuity(0.02 * ctx.lgd);
        assert!(
            (triangle_annuity - ctx.bootstrapped_l_at_expiry).abs() < 1e-12,
            "same hazard must give same annuity: state={triangle_annuity}, curve={}",
            ctx.bootstrapped_l_at_expiry
        );
    }

    // Price-strike (CDX HY convention) payoff tests

    /// S&P CDS Indices Primer default-adjusted strike, used as a TEST ORACLE
    /// only. Production uses the direct `H_price = ξ(K − 1)·f0/f` term and
    /// keeps realized loss in the `D` settlement; this oracle folds factor
    /// and loss into an equivalent strike quoted on the current factor.
    fn adjusted_strike_pct(k_pct: f64, f0: f64, f: f64, realized_loss: f64) -> f64 {
        let k = k_pct / 100.0;
        100.0 * (1.0 - ((1.0 - k) * f0 - realized_loss) / f)
    }

    #[allow(clippy::too_many_arguments)]
    fn price_strike_option(
        as_of: Date,
        option_type: OptionType,
        strike_price_pct: f64,
        coupon_bp: f64,
        vol: f64,
        strike_factor: f64,
        current_factor: f64,
        realized_loss: Option<f64>,
    ) -> CDSOption {
        use crate::instruments::common_impl::traits::Instrument;

        let params = CDSOptionParams::new(
            super::super::strike::CDSOptionStrike::CleanPricePct(
                Decimal::try_from(strike_price_pct).expect("valid clean-price strike"),
            ),
            as_of.add_months(12),
            as_of.add_months(60),
            Money::from((10_000_000_i64, Currency::USD)),
            option_type,
        )
        .expect("valid option params")
        .as_index(current_factor)
        .expect("valid index factor")
        .with_strike_index_factor(strike_factor)
        .expect("valid strike index factor")
        .with_underlying_cds_coupon(bp_to_decimal(coupon_bp));
        let credit = CreditParams::corporate_standard("HY", "HZ-SN");
        let mut option = CDSOption::new("CDSO-HY-UNIT", &params, &credit, "USD-OIS", "CDSO-VOL")
            .expect("valid cds option");
        option.realized_index_loss = realized_loss;
        option
            .instrument_pricing_overrides
            .market_quotes
            .implied_volatility = Some(vol);
        option
            .validate_invariants()
            .expect("post-construction state must satisfy full validation");
        option
    }

    fn npv_per_unit(option: &CDSOption, market: &MarketContext, as_of: Date, sigma: f64) -> f64 {
        let cds = synthetic_underlying_cds(option, as_of).expect("synthetic cds");
        npv(option, &cds, market, sigma, as_of)
            .expect("npv")
            .amount()
            / option.notional.amount()
    }

    #[test]
    fn sp_primer_factor_loss_fixture_yields_107_9874() {
        // S&P Dow Jones Indices CDS Indices Primer clean-price adjustment:
        // K = 107.0, f0 = 1.00, f = 0.99, one default at 9.25% recovery of
        // a 1% weight name: L = (1 − 0.0925)·0.01 = 0.009075.
        // Published adjusted strike: 107.9874% (rounded to 4 dp).
        let k_adj = adjusted_strike_pct(107.0, 1.0, 0.99, (1.0 - 0.0925) * 0.01);
        assert!(
            (k_adj - 107.9874).abs() < 5e-4,
            "S&P primer adjusted strike mismatch: got {k_adj}, expected 107.9874"
        );
    }

    /// The direct `H_price` production form must equal the adjusted-strike
    /// oracle when the oracle OMITS realized loss from `D` (the loss is
    /// already folded into its strike). This is the plan's anti-double-count
    /// identity: `ξ(K_adj − 1) = ξ(K − 1)·f0/f + ξ·L/f`.
    #[test]
    fn price_strike_direct_form_matches_adjusted_strike_oracle() {
        let as_of = date!(2025 - 01 - 01);
        let mkt = market(as_of);
        let (k_pct, f0, f, loss) = (107.0, 1.0, 0.99, (1.0 - 0.0925) * 0.01);
        let vol = 0.35;

        for option_type in [OptionType::Call, OptionType::Put] {
            let direct =
                price_strike_option(as_of, option_type, k_pct, 500.0, vol, f0, f, Some(loss));
            // Oracle: adjusted strike quoted on the CURRENT factor (f0' = f),
            // realized loss removed from the settlement term.
            let oracle = price_strike_option(
                as_of,
                option_type,
                adjusted_strike_pct(k_pct, f0, f, loss),
                500.0,
                vol,
                f,
                f,
                None,
            );
            let npv_direct = npv_per_unit(&direct, &mkt, as_of, vol);
            let npv_oracle = npv_per_unit(&oracle, &mkt, as_of, vol);
            assert!(
                (npv_direct - npv_oracle).abs() <= 1e-10 * npv_direct.abs().max(1.0),
                "{option_type:?}: direct={npv_direct}, oracle={npv_oracle}"
            );
        }
    }

    /// A deliberately double-counted oracle (adjusted strike AND realized
    /// loss retained in `D`) must NOT match — this protects the production
    /// path against a regression that counts the loss twice.
    #[test]
    fn double_counted_adjusted_strike_oracle_differs() {
        let as_of = date!(2025 - 01 - 01);
        let mkt = market(as_of);
        let (k_pct, f0, f, loss) = (107.0, 1.0, 0.99, (1.0 - 0.0925) * 0.01);
        let vol = 0.35;

        let direct = price_strike_option(
            as_of,
            OptionType::Call,
            k_pct,
            500.0,
            vol,
            f0,
            f,
            Some(loss),
        );
        // Same (valid) factor/loss state as `direct`, but the strike already
        // folds the loss in — retaining it in `D` then counts it twice.
        let double_counted = price_strike_option(
            as_of,
            OptionType::Call,
            adjusted_strike_pct(k_pct, f0, f, loss),
            500.0,
            vol,
            f0,
            f,
            Some(loss),
        );
        let npv_direct = npv_per_unit(&direct, &mkt, as_of, vol);
        let npv_double = npv_per_unit(&double_counted, &mkt, as_of, vol);
        // The double count adds ~ξ·L/f to every exercised node: material.
        assert!(
            (npv_double - npv_direct).abs() > 1e-4,
            "double-counted oracle must differ: direct={npv_direct}, double={npv_double}"
        );
    }

    /// Pre-default state (`f = f0`, `L = 0`): the deterministic price term
    /// reduces to `ξ(K − 1)` exactly.
    #[test]
    fn pre_default_price_term_reduces_to_k_minus_one() {
        let as_of = date!(2025 - 01 - 01);
        let mkt = market(as_of);
        let payer =
            price_strike_option(as_of, OptionType::Call, 107.0, 500.0, 0.35, 1.0, 1.0, None);
        let ctx = context_for(&payer, &mkt, as_of, 0.35);
        assert!(
            (ctx.signed_strike_adjustment_per_n() - 0.07).abs() < 1e-14,
            "payer H_price should be K − 1 = 0.07, got {}",
            ctx.signed_strike_adjustment_per_n()
        );

        let receiver =
            price_strike_option(as_of, OptionType::Put, 107.0, 500.0, 0.35, 1.0, 1.0, None);
        let ctx_r = context_for(&receiver, &mkt, as_of, 0.35);
        assert!(
            (ctx_r.signed_strike_adjustment_per_n() + 0.07).abs() < 1e-14,
            "receiver H_price should be −(K − 1) = −0.07, got {}",
            ctx_r.signed_strike_adjustment_per_n()
        );
    }

    /// Payer/receiver parity for the price-strike payoff:
    /// `payer − receiver = df · (f·F0 + (K − 1)·f0 + L)` per unit
    /// notional, both node-by-node (max(x,0) − max(−x,0) = x) and after
    /// integration against the calibrated lognormal density.
    #[test]
    fn price_strike_payer_receiver_parity_holds() {
        let as_of = date!(2025 - 01 - 01);
        let mkt = market(as_of);
        let (k_pct, f0, f, loss) = (105.0, 1.0, 0.99, 0.004);
        let vol = 0.40;

        let payer = price_strike_option(
            as_of,
            OptionType::Call,
            k_pct,
            500.0,
            vol,
            f0,
            f,
            Some(loss),
        );
        let receiver =
            price_strike_option(as_of, OptionType::Put, k_pct, 500.0, vol, f0, f, Some(loss));
        let ctx = context_for(&payer, &mkt, as_of, vol);

        let parity_rhs =
            ctx.df_to_settlement * (f * ctx.forward_value() + (k_pct / 100.0 - 1.0) * f0 + loss);
        let lhs =
            npv_per_unit(&payer, &mkt, as_of, vol) - npv_per_unit(&receiver, &mkt, as_of, vol);
        assert!(
            (lhs - parity_rhs).abs() < 1e-9,
            "parity violated: payer − receiver = {lhs}, expected {parity_rhs}"
        );
    }

    /// The native ATM-forward clean-price coordinate is the parity strike:
    /// payer and receiver struck there have equal value.
    #[test]
    fn native_atm_forward_clean_price_is_the_parity_strike() {
        let as_of = date!(2025 - 01 - 01);
        let mkt = market(as_of);
        let (f0, f, loss) = (1.0, 0.99, 0.004);
        let vol = 0.40;

        let seed = price_strike_option(
            as_of,
            OptionType::Call,
            105.0,
            500.0,
            vol,
            f0,
            f,
            Some(loss),
        );
        let ctx = context_for(&seed, &mkt, as_of, vol);
        let k_atm_pct = ctx
            .native_atm_forward_clean_price_pct()
            .expect("ATM coordinate");

        let payer = price_strike_option(
            as_of,
            OptionType::Call,
            k_atm_pct,
            500.0,
            vol,
            f0,
            f,
            Some(loss),
        );
        let receiver = price_strike_option(
            as_of,
            OptionType::Put,
            k_atm_pct,
            500.0,
            vol,
            f0,
            f,
            Some(loss),
        );
        let diff =
            npv_per_unit(&payer, &mkt, as_of, vol) - npv_per_unit(&receiver, &mkt, as_of, vol);
        assert!(
            diff.abs() < 1e-8,
            "payer/receiver parity must be zero at K_ATM = {k_atm_pct}: diff = {diff}"
        );
    }

    /// Limiting identity: `f = f0 = 1`, `L = 0`, `FEP = 0` reduces the ATM
    /// coordinate to `K_ATM = 1 − F0` when valued at legal expiry.
    #[test]
    fn native_atm_forward_limit_reduces_to_one_minus_f0() {
        let as_of = date!(2025 - 01 - 01);
        let mkt = market(as_of);
        let option =
            price_strike_option(as_of, OptionType::Call, 107.0, 500.0, 0.35, 1.0, 1.0, None);
        let ctx = context_for(&option, &mkt, option.expiry, 0.35);
        assert!(
            ctx.front_end_protection == 0.0,
            "FEP must be zero at legal expiry"
        );
        let k_atm = ctx
            .native_atm_forward_clean_price_pct()
            .expect("ATM coordinate")
            / 100.0;
        let expected = 1.0 - ctx.forward_value();
        assert!(
            (k_atm - expected).abs() < 1e-12,
            "limiting K_ATM mismatch: got {k_atm}, expected {expected}"
        );
    }

    /// Payer value rises in the clean-price strike and in the credit spread;
    /// receiver value falls in both.
    #[test]
    fn price_strike_monotonicity_in_strike_and_spread() {
        let as_of = date!(2025 - 01 - 01);
        let mkt = market(as_of);
        let wider_spread_market = MarketContext::new()
            .insert(flat_discount("USD-OIS", as_of, 0.03))
            .insert(flat_hazard("HZ-SN", as_of, 0.4, 0.03));
        let vol = 0.40;

        let payer_low =
            price_strike_option(as_of, OptionType::Call, 103.0, 500.0, vol, 1.0, 1.0, None);
        let payer_high =
            price_strike_option(as_of, OptionType::Call, 107.0, 500.0, vol, 1.0, 1.0, None);
        assert!(
            npv_per_unit(&payer_high, &mkt, as_of, vol)
                > npv_per_unit(&payer_low, &mkt, as_of, vol),
            "payer must gain value as the clean-price strike rises"
        );

        let receiver_low =
            price_strike_option(as_of, OptionType::Put, 103.0, 500.0, vol, 1.0, 1.0, None);
        let receiver_high =
            price_strike_option(as_of, OptionType::Put, 107.0, 500.0, vol, 1.0, 1.0, None);
        assert!(
            npv_per_unit(&receiver_high, &mkt, as_of, vol)
                < npv_per_unit(&receiver_low, &mkt, as_of, vol),
            "receiver must lose value as the clean-price strike rises"
        );

        // Wider spreads: protection worth more → payer up, receiver down.
        assert!(
            npv_per_unit(&payer_low, &wider_spread_market, as_of, vol)
                > npv_per_unit(&payer_low, &mkt, as_of, vol),
            "payer must gain value as spreads widen"
        );
        assert!(
            npv_per_unit(&receiver_high, &wider_spread_market, as_of, vol)
                < npv_per_unit(&receiver_high, &mkt, as_of, vol),
            "receiver must lose value as spreads widen"
        );
    }

    /// The small-positive-vol limit converges to the clean deterministic
    /// intrinsic payoff without altering the explicit-vol validation
    /// contract.
    #[test]
    fn price_strike_small_vol_limit_matches_deterministic_payoff() {
        let as_of = date!(2025 - 01 - 01);
        let mkt = market(as_of);
        let vol = 1e-4;
        for option_type in [OptionType::Call, OptionType::Put] {
            let option = price_strike_option(
                as_of,
                option_type,
                105.0,
                500.0,
                vol,
                1.0,
                0.99,
                Some(0.004),
            );
            let ctx = context_for(&option, &mkt, as_of, vol);
            let intrinsic = deterministic_payoff_per_n(&ctx);
            let priced = npv_per_unit(&option, &mkt, as_of, vol);
            assert!(
                (priced - intrinsic).abs() < 1e-6,
                "{option_type:?}: small-vol limit {priced} must approach \
                 deterministic intrinsic {intrinsic}"
            );
        }
    }

    mod production_cds_option_audit {
        use super::*;

        #[test]
        fn exercise_payment_date_changes_discounting_without_changing_variance() {
            let as_of = date!(2025 - 01 - 01);
            let curves = market(as_of);
            let mut option = option(as_of, OptionType::Call, 100.0, 0.3);
            option.knockout = true;
            let cds = synthetic_underlying_cds(&option, as_of).expect("CDS");
            let original = npv(&option, &cds, &curves, 0.3, as_of)
                .expect("price")
                .amount();
            option.exercise_settlement_date = Some(option.expiry + time::Duration::days(5));
            let delayed = npv(&option, &cds, &curves, 0.3, as_of)
                .expect("price")
                .amount();
            assert!((delayed - original * (-0.03_f64 * 5.0 / 365.0).exp()).abs() < 1e-8);
        }

        #[test]
        fn non_knockout_single_name_pays_front_end_protection_only_to_payer() {
            let as_of = date!(2025 - 01 - 01);
            let curves = market(as_of);
            for side in [OptionType::Call, OptionType::Put] {
                let mut option = option(as_of, side, 100.0, 0.3);
                let cds = synthetic_underlying_cds(&option, as_of).expect("CDS");
                option.knockout = true;
                let ko = npv(&option, &cds, &curves, 0.3, as_of)
                    .expect("KO")
                    .amount();
                option.knockout = false;
                let nko = npv(&option, &cds, &curves, 0.3, as_of)
                    .expect("NKO")
                    .amount();
                let t = (option.expiry - as_of).whole_days() as f64 / 365.0;
                let expected = if side == OptionType::Call {
                    option.notional.amount() * (-0.03 * t).exp() * 0.6 * (1.0 - (-0.02 * t).exp())
                } else {
                    0.0
                };
                assert!(
                    (nko - ko - expected).abs() < 1e-8,
                    "{side:?}: NKO-KO={} vs FEP={expected}",
                    nko - ko
                );
            }
        }

        #[test]
        fn valuation_clock_ignores_fixed_premium_and_exercise_settlement_dates() {
            let initial = date!(2025 - 01 - 01);
            let later = date!(2025 - 07 - 01);
            let mut option = option(initial, OptionType::Call, 100.0, 0.3);
            option.cash_settlement_date = Some(date!(2025 - 01 - 06));
            option.exercise_settlement_date = Some(option.expiry + time::Duration::days(5));
            let expected = (option.expiry - later).whole_days() as f64 / 365.0;
            assert!(
                (option.time_to_expiry(later).expect("time") - expected).abs() < 1e-14,
                "model time must run from current valuation date to legal expiry"
            );
        }

        #[test]
        fn front_end_protection_window_advances_with_valuation_date() {
            let initial = date!(2025 - 01 - 01);
            let later = date!(2025 - 07 - 01);
            let mut option = option(initial, OptionType::Call, 100.0, 0.3);
            option.underlying_is_index = true;
            option.knockout = false;
            option.cash_settlement_date = Some(date!(2025 - 01 - 06));
            let curves = market(initial);
            let ctx = context_for(&option, &curves, later, 0.3);
            let t = (option.expiry - later).whole_days() as f64 / 365.0;
            assert!((ctx.front_end_protection - 0.6 * (1.0 - (-0.02 * t).exp())).abs() < 1e-12);
        }

        #[test]
        fn index_parity_contains_front_end_protection_once() {
            let as_of = date!(2025 - 01 - 01);
            let curves = MarketContext::new()
                .insert(flat_discount("USD-OIS", as_of, 0.0))
                .insert(flat_hazard("HZ-SN", as_of, 0.4, 0.02));
            let mut payer = option_with_coupon(as_of, OptionType::Call, 100.0, 100.0, 0.3);
            payer.underlying_is_index = true;
            payer.knockout = false;
            payer.strike =
                super::super::super::strike::CDSOptionStrike::CleanPricePct(Decimal::from(100));
            payer.strike_index_factor = Some(1.0);
            let mut receiver = payer.clone();
            receiver.option_type = OptionType::Put;
            let cds = synthetic_underlying_cds(&payer, as_of).expect("CDS");
            let p = npv(&payer, &cds, &curves, 0.3, as_of)
                .expect("payer")
                .amount();
            let r = npv(&receiver, &cds, &curves, 0.3, as_of)
                .expect("receiver")
                .amount();
            // With no discounting and a par clean-price strike, add back
            // the running premium PV to isolate the complete protection leg.
            let t =
                (payer.cds_maturity + time::Duration::days(1) - as_of).whole_days() as f64 / 365.0;
            let ctx = context_for(&payer, &curves, as_of, 0.3);
            let expiry_t = (payer.expiry - as_of).whole_days() as f64 / 365.0;
            let annuity: f64 = ctx
                .accrual_factors
                .iter()
                .zip(&ctx.times_from_expiry)
                .map(|(a, t)| a * (-0.02 * (expiry_t + t)).exp())
                .sum::<f64>()
                - ctx.accrual_pcd_to_expiry * (-0.02 * expiry_t).exp();
            let expected =
                payer.notional.amount() * (0.6 * (1.0 - (-0.02 * t).exp()) - 0.01 * annuity);
            assert!(
                (p - r - expected).abs() < 1e-7 * payer.notional.amount(),
                "payer-receiver {} vs single protection amount {expected}",
                p - r
            );
        }

        #[test]
        fn knockout_forward_excludes_pre_expiry_defaults() {
            let as_of = date!(2025 - 01 - 01);
            let curves = MarketContext::new()
                .insert(flat_discount("USD-OIS", as_of, 0.0))
                .insert(flat_hazard("HZ-SN", as_of, 0.4, 0.02));
            let mut option = option_with_coupon(as_of, OptionType::Call, 100.0, 100.0, 0.3);
            option.knockout = true;
            let ctx = context_for(&option, &curves, as_of, 0.3);
            let expiry_t = (option.expiry - as_of).whole_days() as f64 / 365.0;
            let maturity_t =
                (option.cds_maturity + time::Duration::days(1) - as_of).whole_days() as f64 / 365.0;
            let expected = 0.6 * ((-0.02 * expiry_t).exp() - (-0.02 * maturity_t).exp());
            let actual = (ctx.forward_value() + ctx.coupon * ctx.bootstrapped_l_at_expiry)
                * ctx.df_to_settlement
                * ctx.survival_to_expiry;
            assert!(
                (actual - expected).abs() < 1e-9,
                "KO forward protection {actual} vs post-expiry-only {expected}"
            );
        }
    }
}
