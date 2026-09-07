//! CMS static replication under a normalized payment-to-annuity mapping.
//!
//! Following Hagan (2003), equations 2.7–2.9, the discounted payoff is
//! `DF_pay * E^A[h(S) payoff(S)] / E^A[h(S)]`. We model
//! `h(s) ∝ (1+s/m)^(-m*payment_delay) / A_par(s)` and normalize at the
//! forward for numerical conditioning. Every hedge swaption uses the same
//! current annuity; it cancels from the ratio, never strike by strike.
//!
//! Expectations are replicated using OTM Black swaptions on either side of
//! the forward. This removes deterministic intrinsic value from the integrals
//! and preserves zero-volatility limits even for deep ITM or negative strikes.
//! Adaptive quadrature integrates in log strike, splitting at the payoff kink,
//! forward and smile knots. The clamped smile's largest volatility determines
//! the lognormal tail bounds. This is a one-factor annuity approximation, not
//! an exact multi-factor rates model.
//!
//! Reference: <https://www.deriscope.com/docs/Hagan_Convexity_Conundrums.pdf>.

use crate::instruments::common_impl::pricing::time::relative_df_discount_curve;
use crate::instruments::common_impl::traits::Instrument;
use crate::instruments::rates::cms_option::types::CmsOption;
use crate::instruments::OptionType;
use crate::pricer::{
    InstrumentType, ModelKey, Pricer, PricerKey, PricingError, PricingErrorContext,
};
use crate::results::ValuationResult;
use finstack_quant_core::dates::{Date, DateExt, DayCount, DayCountContext};
use finstack_quant_core::market_data::context::MarketContext;
use finstack_quant_core::math::integration::gauss_legendre_integrate_adaptive;
use finstack_quant_core::money::Money;
use finstack_quant_core::Result;
use finstack_quant_models::closed_form::{black_call, black_put};

/// Inputs shared by CMS options and CMS swap embedded options.
pub(crate) struct CmsOptionletInputs<'a> {
    /// Positive forward reference-swap rate, in decimal units.
    pub forward_rate: f64,
    /// ACT/365F years to fixing on the volatility expiry axis.
    pub time_to_fixing: f64,
    /// Discount factor from valuation to coupon payment.
    pub df_pay: f64,
    /// Lognormal swaption smile, queried at each hedge strike.
    pub vol_surface: &'a finstack_quant_core::market_data::surfaces::VolSurface,
    /// Reference-swap tenor in years.
    pub cms_tenor: f64,
    /// Reference fixed-leg payments per year.
    pub payments_per_year: f64,
    /// ACT/365F years from reference-swap start to coupon payment.
    pub payment_delay: f64,
}

/// Value and first two rate derivatives of the payment-to-annuity mapping.
fn annuity_weight(rate: f64, tenor: f64, m: f64, delay: f64) -> (f64, f64, f64) {
    let n = tenor * m;
    let x = rate / m;
    // Series avoids cancellation in A, A' and A'' near zero. Keep fourth
    // order so the second derivative remains accurate across the branch.
    let (a, ap, app) = if (n * x).abs() < 1e-3 {
        let c1 = -(n + 1.0) / 2.0;
        let c2 = (n + 1.0) * (n + 2.0) / 6.0;
        let c3 = -(n + 1.0) * (n + 2.0) * (n + 3.0) / 24.0;
        let c4 = (n + 1.0) * (n + 2.0) * (n + 3.0) * (n + 4.0) / 120.0;
        (
            tenor * (1.0 + x * (c1 + x * (c2 + x * (c3 + x * c4)))),
            tenor / m * (c1 + x * (2.0 * c2 + x * (3.0 * c3 + x * 4.0 * c4))),
            tenor / (m * m) * (2.0 * c2 + x * (6.0 * c3 + x * 12.0 * c4)),
        )
    } else {
        let q = (-n * x.ln_1p()).exp();
        let b = -(-n * x.ln_1p()).exp_m1();
        let bp = n * q / (m + rate);
        let bpp = -n * (n + 1.0) * q / (m + rate).powi(2);
        (
            b / rate,
            bp / rate - b / rate.powi(2),
            bpp / rate - 2.0 * bp / rate.powi(2) + 2.0 * b / rate.powi(3),
        )
    };
    let delta = m * delay;
    let weight = (-delta * x.ln_1p()).exp() / a;
    let log_prime = -delta / (m + rate) - ap / a;
    let log_second = delta / (m + rate).powi(2) - app / a + (ap / a).powi(2);
    (
        weight,
        weight * log_prime,
        weight * (log_prime.powi(2) + log_second),
    )
}

/// Replicate a discounted CMS optionlet per unit notional and accrual.
///
/// # Arguments
/// * `inputs` - Forward, smile, discounting and reference-swap conventions.
/// * `strike` - Economic strike in decimal rate units, including nonpositive strikes.
/// * `option_type` - Call for a caplet, put for a floorlet.
pub(crate) fn replicated_cms_optionlet(
    inputs: &CmsOptionletInputs<'_>,
    strike: f64,
    option_type: OptionType,
) -> Result<f64> {
    if inputs.vol_surface.quote_type()
        != finstack_quant_core::market_data::surfaces::VolQuoteType::BlackLognormal
    {
        return Err(finstack_quant_core::Error::Validation(
            "CMS static replication requires Black lognormal swaption volatility quotes".into(),
        ));
    }
    let f = inputs.forward_rate;
    let t = inputs.time_to_fixing;
    let sign = match option_type {
        OptionType::Call => 1.0,
        OptionType::Put => -1.0,
    };
    let intrinsic = (sign * (f - strike)).max(0.0);
    if t <= 0.0 {
        return Ok(inputs.df_pay * intrinsic);
    }
    let max_vol = inputs
        .vol_surface
        .vols()
        .iter()
        .copied()
        .fold(0.0_f64, f64::max);
    if max_vol == 0.0 {
        return Ok(inputs.df_pay * intrinsic);
    }
    let weight = |k| {
        annuity_weight(
            k,
            inputs.cms_tenor,
            inputs.payments_per_year,
            inputs.payment_delay,
        )
    };
    let scale = weight(f).0;
    let otm = |k: f64| {
        let vol =
            finstack_quant_models::volatility::get_surface_vol_clamped(inputs.vol_surface, t, k);
        if k < f {
            black_put(f, k, vol, t)
        } else {
            black_call(f, k, vol, t)
        }
    };
    // Include the tails of both the density and its rate-weighted moments.
    // Larger moment shifts accommodate the polynomial annuity mapping.
    let sigma_t = max_vol * t.sqrt();
    let width = 12.0 * sigma_t + (2.0 + inputs.payment_delay.abs()) * sigma_t.powi(2);
    if !width.is_finite() || width > 500.0 {
        return Err(finstack_quant_core::Error::Validation(
            "CMS replication lognormal tail range is not numerically representable".into(),
        ));
    }
    let mut splits = vec![-width, 0.0, width];
    for k in inputs
        .vol_surface
        .strikes()
        .iter()
        .copied()
        .chain(std::iter::once(strike))
    {
        if k > 0.0 {
            let x = (k / f).ln();
            if x > -width && x < width {
                splits.push(x);
            }
        }
    }
    splits.sort_by(f64::total_cmp);
    splits.dedup();
    let mut numerator = intrinsic;
    let mut denominator = 1.0;
    // Derivative jump of h(s) payoff(s) at a positive strike.
    if strike > 0.0 {
        numerator += weight(strike).0 / scale * otm(strike);
    }
    let tolerance = 1e-11 / (splits.len() - 1) as f64;
    for interval in splits.windows(2) {
        let integrate = |payoff: bool| {
            gauss_legendre_integrate_adaptive(
                |x: f64| {
                    let k = f * x.exp();
                    let (_, hp, hpp) = weight(k);
                    let second = if payoff {
                        if sign * (k - strike) > 0.0 {
                            sign * (2.0 * hp + (k - strike) * hpp)
                        } else {
                            0.0
                        }
                    } else {
                        hpp
                    };
                    second / scale * otm(k) * k
                },
                interval[0],
                interval[1],
                16,
                tolerance,
                16,
            )
        };
        numerator += integrate(true)?;
        denominator += integrate(false)?;
    }
    let value = inputs.df_pay * numerator / denominator;
    if !denominator.is_finite() || denominator <= 0.0 || !value.is_finite() || value < -1e-10 {
        return Err(finstack_quant_core::Error::Validation(
            "CMS replication produced an invalid normalized expectation; check the swaption smile"
                .into(),
        ));
    }
    Ok(value.max(0.0))
}

/// CMS option pricer using smile-aware static replication and a normalized
/// one-factor payment-to-annuity approximation.
pub struct CmsReplicationPricer;

impl CmsReplicationPricer {
    /// Create a new CMS replication pricer.
    pub fn new() -> Self {
        Self
    }

    /// Core pricing logic: iterate over fixings and apply static replication.
    fn price_internal(
        &self,
        inst: &CmsOption,
        curves: &MarketContext,
        as_of: Date,
    ) -> Result<Money> {
        inst.validate()?;
        let mut total_pv = 0.0;

        let strike = inst.strike_f64()?;
        let discount_curve = curves.get_discount(inst.discount_curve_id.as_ref())?;
        let vol_surface = curves.get_surface(inst.vol_surface_id.as_str())?;

        // Payments-per-year for the par annuity closed form.
        // Matches the fixed-leg payment frequency of the underlying CMS swap.
        let reference_swap = inst.reference_swap();
        let m = reference_swap.payments_per_year();

        for (i, &fixing_date) in inst.fixing_dates.iter().enumerate() {
            let payment_date = inst.payment_dates[i];
            let accrual_fraction = inst.accrual_fractions[i];

            if payment_date <= as_of {
                continue; // Period already settled
            }

            // Seasoned period: the CMS rate fixed in the past, so the option
            // payoff is pure intrinsic on the *recorded* fixing (mirroring the
            // cap/floor pricer) — never on a rate re-projected from the live
            // curve, which books phantom P&L.
            if fixing_date < as_of {
                let observed = crate::instruments::rates::hw1f::fixings::historical_cms_fixing(
                    curves,
                    &inst.forward_curve_id,
                    inst.cms_tenor,
                    fixing_date,
                )?;
                let df_pay =
                    relative_df_discount_curve(discount_curve.as_ref(), as_of, payment_date)?;
                let intrinsic = match inst.option_type {
                    OptionType::Call => (observed - strike).max(0.0),
                    OptionType::Put => (strike - observed).max(0.0),
                };
                total_pv += df_pay * intrinsic * accrual_fraction;
                continue;
            }

            // Forward-starting swap parameters for this fixing
            let swap_start = reference_swap.reference_swap_start(fixing_date)?;
            let swap_end = swap_start.add_months((inst.cms_tenor * 12.0).round() as i32);

            // F (forward swap rate). The market annuity A₀ is intentionally
            // discarded: the static replication uses the closed-form par
            // annuity `A_par(·)` consistently in both `g(k)` and `C_sw(k)`
            // (see the annuity-consistency note at the boundary term below).
            let (forward_rate, _annuity_mkt) =
                reference_swap.forward_rate_and_annuity(curves, as_of, swap_start, swap_end)?;

            if forward_rate <= 0.0 {
                return Err(finstack_quant_core::Error::Validation(format!(
                    "Forward swap rate {:.6} is non-positive for fixing date {}; \
                     Black-76 requires positive forward rates",
                    forward_rate, fixing_date
                )));
            }

            // Time-to-fixing is calendar time for the vol axis: ACT/365F.
            let ttf =
                DayCount::Act365F.year_fraction(as_of, fixing_date, DayCountContext::default())?;

            // DF to payment date from discount curve (relative to as_of)
            let df_pay = relative_df_discount_curve(discount_curve.as_ref(), as_of, payment_date)?;

            // --- Static Replication (shared engine) ---
            let period_pv = replicated_cms_optionlet(
                &CmsOptionletInputs {
                    forward_rate,
                    time_to_fixing: ttf,
                    df_pay,
                    vol_surface: vol_surface.as_ref(),
                    cms_tenor: inst.cms_tenor,
                    payments_per_year: m,
                    payment_delay:
                        crate::instruments::rates::cms_common::signed_act365f_year_fraction(
                            swap_start,
                            payment_date,
                        )?,
                },
                strike,
                inst.option_type,
            )?;

            total_pv += period_pv * accrual_fraction;
        }

        Money::new(total_pv * inst.notional.amount(), inst.notional.currency())
    }
}

impl Default for CmsReplicationPricer {
    fn default() -> Self {
        Self::new()
    }
}

impl Pricer for CmsReplicationPricer {
    fn key(&self) -> PricerKey {
        PricerKey::new(InstrumentType::CmsOption, ModelKey::StaticReplication)
    }

    fn price_dyn(
        &self,
        instrument: &dyn Instrument,
        market: &MarketContext,
        as_of: Date,
    ) -> std::result::Result<ValuationResult, PricingError> {
        let cms = crate::pricer::expect_inst::<CmsOption>(instrument, InstrumentType::CmsOption)?;

        let pv = self
            .price_internal(cms, market, as_of)
            .map_err(|e| PricingError::from_core(e, PricingErrorContext::default()))?;

        Ok(ValuationResult::stamped(cms.id(), as_of, pv))
    }
}
