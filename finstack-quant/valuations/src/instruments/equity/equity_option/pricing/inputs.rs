//! Equity-option market inputs, lifecycle, and discrete-dividend helpers.

use crate::instruments::common_impl::helpers::year_fraction;
use crate::instruments::common_impl::parameters::{OptionMarketParams, OptionType};
use crate::instruments::equity::equity_option::types::EquityOption;
use crate::instruments::{ExerciseStyle, SettlementType};
use crate::pricer::{ModelKey, PricingError, PricingErrorContext};
use finstack_quant_core::dates::{Date, DayCount};
use finstack_quant_core::market_data::context::MarketContext;
use finstack_quant_core::money::Money;
use finstack_quant_core::Result;

pub(crate) fn require_european(inst: &EquityOption, model: &str) -> Result<()> {
    if !matches!(inst.exercise_style, ExerciseStyle::European) {
        return Err(finstack_quant_core::Error::Validation(format!(
            "{model} supports European EquityOption exercise only; got {:?}",
            inst.exercise_style
        )));
    }
    Ok(())
}
/// Resolve a fixed exercise/expiry state before running a live option model.
///
/// Returns `None` while the option is live. From an observed exercise date
/// onward it returns the remaining cash-settlement or physical-delivery value.
/// At or after expiry, a missing observation is an error rather than an
/// implicit auto-exercise assumption.
pub(crate) fn resolve_lifecycle_value(
    inst: &EquityOption,
    market: &MarketContext,
    as_of: Date,
) -> Result<Option<Money>> {
    let Some(exercise) = inst.exercise else {
        if as_of >= inst.expiry {
            return Err(finstack_quant_core::Error::Validation(format!(
                "EquityOption '{}' requires an exercise/expiry observation from expiry {} onward",
                inst.id, inst.expiry
            )));
        }
        return Ok(None);
    };

    if as_of < exercise.date {
        return Ok(None);
    }
    let currency = inst.notional.currency();
    if !exercise.exercised || as_of > exercise.settlement_date {
        return Ok(Some(Money::from((0_i64, currency))));
    }

    let discount_curve = market.get_discount(inst.discount_curve_id.as_str())?;
    let settlement_df = discount_curve.df_between_dates(as_of, exercise.settlement_date)?;
    let unit_value = match inst.settlement {
        SettlementType::Cash => {
            let intrinsic = match inst.option_type {
                OptionType::Call => (exercise.spot - inst.strike).max(0.0),
                OptionType::Put => (inst.strike - exercise.spot).max(0.0),
            };
            intrinsic * settlement_df
        }
        SettlementType::Physical => {
            let spot = crate::instruments::common_impl::helpers::scalar_price_amount(
                market.get_price(&inst.spot_id)?,
                currency,
            )?;
            let t = year_fraction(DayCount::Act365F, as_of, exercise.settlement_date)?;
            let future_dividends = inst
                .discrete_dividends
                .iter()
                .filter(|(date, _)| *date > as_of && *date <= exercise.settlement_date)
                .collect::<Vec<_>>();
            let prepaid_forward = if future_dividends.is_empty() {
                let q = if let Some(dividend_yield_id) = &inst.div_yield_id {
                    match market.get_price(dividend_yield_id.as_str())? {
                        finstack_quant_core::market_data::scalars::MarketScalar::Unitless(
                            value,
                        ) => *value,
                        finstack_quant_core::market_data::scalars::MarketScalar::Price(_) => {
                            return Err(finstack_quant_core::Error::Validation(format!(
                                "Dividend yield '{}' must be unitless",
                                dividend_yield_id
                            )));
                        }
                    }
                } else {
                    0.0
                };
                spot * (-q * t).exp()
            } else {
                let mut dividend_pv = finstack_quant_core::math::NeumaierAccumulator::new();
                for (date, amount) in future_dividends {
                    dividend_pv.add(*amount * discount_curve.df_between_dates(as_of, *date)?);
                }
                spot - dividend_pv.total()
            };
            match inst.option_type {
                OptionType::Call => prepaid_forward - inst.strike * settlement_df,
                OptionType::Put => inst.strike * settlement_df - prepaid_forward,
            }
        }
    };

    Ok(Some(Money::new(
        unit_value * inst.notional.amount(),
        currency,
    )?))
}

/// Collected market inputs for equity option pricing.
///
/// The effective rate `r` reproduces the curve-native date-to-date discount
/// factor when applied over the ACT/365F model time `t_vol`.
#[derive(Debug, Clone, Copy)]
pub(crate) struct EquityOptionInputs {
    /// Spot price of the underlying
    pub(crate) spot: f64,
    /// Effective risk-free rate consistent with `t_vol`
    pub(crate) r: f64,
    /// Dividend yield
    pub(crate) q: f64,
    /// Implied volatility
    pub(crate) sigma: f64,
    /// Time to expiry for vol calculations (ACT/365F standard)
    pub(crate) t_vol: f64,
}

/// Collect standard inputs (spot, risk-free, dividend yield, vol, time to expiry).
///
/// **Day Count Convention Handling:**
/// - Discount factors use the discount curve's own day count
/// - Vol surface lookups and model time use ACT/365F (equity market standard)
///
/// This separation ensures consistent pricing when discount curves use different
/// conventions (e.g., OIS curves with ACT/360) than the vol surface.
pub(crate) fn collect_inputs(
    inst: &EquityOption,
    curves: &MarketContext,
    as_of: Date,
) -> Result<(f64, f64, f64, f64, f64)> {
    let inputs = collect_inputs_extended(inst, curves, as_of)?;
    // Return t_vol as the primary time for the simplified interface
    Ok((inputs.spot, inputs.r, inputs.q, inputs.sigma, inputs.t_vol))
}

/// Collect inputs with curve-native discounting and ACT/365F model time.
///
/// The curve's day count is consumed by its date-to-date discount-factor
/// lookup; `t_vol` uses ACT/365F for volatility lookup and option pricing.
///
/// # Discrete Dividend Handling
///
/// When `discrete_dividends` is non-empty and contains future dividends (ex-date > as_of
/// and ex-date <= expiry), the escrowed dividend model is applied:
/// - Spot is adjusted: `S* = S - Σ D_i × e^{-r × t_i}`
/// - Dividend yield `q` is set to 0.0 (dividends are already priced into S*)
///
/// This is the QuantLib-standard approach for discrete dividends in Black-Scholes.
/// Extract future discrete dividends as `(time_to_ex_date, amount)` pairs.
///
/// Only dividends with an ex-date strictly after `as_of` and on or before
/// `inst.expiry` are returned (past and post-expiry dividends do not affect the
/// option). Times use ACT/365F (the equity-vol market standard). The returned
/// slice drives the escrowed-dividend spot adjustment and its rho correction.
pub(crate) fn has_future_discrete_dividends(inst: &EquityOption, as_of: Date) -> bool {
    inst.discrete_dividends
        .iter()
        .any(|(ex_date, _)| *ex_date > as_of && *ex_date <= inst.expiry)
}

pub(crate) fn reject_future_discrete_dividends_for_stochastic_vol(
    inst: &EquityOption,
    as_of: Date,
    model: ModelKey,
    model_name: &str,
) -> std::result::Result<(), PricingError> {
    if has_future_discrete_dividends(inst, as_of) {
        return Err(PricingError::model_failure_with_context(
            format!(
                "{model_name} pricing does not support discrete dividends: the \
                 escrowed-dividend spot adjustment is a Black-Scholes-only construct \
                 and is invalid under stochastic volatility. Use the Black-Scholes \
                 pricer for discrete dividends, or supply a continuous dividend yield \
                 instead."
            ),
            PricingErrorContext::from_instrument(inst).model(model),
        ));
    }
    Ok(())
}

pub(crate) fn future_dividends(
    inst: &EquityOption,
    disc_curve: &finstack_quant_core::market_data::term_structures::DiscountCurve,
    as_of: Date,
) -> Result<Vec<(f64, f64)>> {
    if inst.discrete_dividends.is_empty() {
        return Ok(Vec::new());
    }
    let divs = inst
        .discrete_dividends
        .iter()
        .filter(|(ex_date, _)| *ex_date > as_of && *ex_date <= inst.expiry)
        .map(|(ex_date, amount)| {
            let t_div = year_fraction(DayCount::Act365F, as_of, *ex_date)?;
            let df = disc_curve.df_between_dates(as_of, *ex_date)?;
            Ok((t_div, *amount * df))
        })
        .collect::<finstack_quant_core::Result<Vec<_>>>()?
        .into_iter()
        .filter(|(t_div, _)| *t_div > 0.0)
        .collect();
    Ok(divs)
}

pub(crate) fn collect_inputs_extended(
    inst: &EquityOption,
    curves: &MarketContext,
    as_of: Date,
) -> Result<EquityOptionInputs> {
    // The curve evaluates the economic discount factor on its own day-count
    // clock. Black–Scholes and the vol surface use ACT/365F, so bridge the two
    // clocks with an effective rate satisfying exp(-r * t_vol) = df.
    let disc_curve = curves.get_discount(inst.discount_curve_id.as_str())?;
    let df = disc_curve.df_between_dates(as_of, inst.expiry)?;

    // Vol time uses ACT/365F (equity market standard for vol surfaces)
    // This is consistent with how equity volatility is quoted in the market
    let t_vol = year_fraction(DayCount::Act365F, as_of, inst.expiry)?;
    // Effective BSM rate on the vol clock — see the two-clock note above.
    // A non-positive/non-finite df means a corrupted curve; error rather than
    // derive an infinite rate that would poison the Black–Scholes price.
    let r = crate::instruments::common_impl::helpers::zero_rate_from_df(
        df,
        t_vol,
        "EquityOption discount curve",
    )?;

    let raw_spot = crate::instruments::common_impl::helpers::scalar_price_amount(
        curves.get_price(&inst.spot_id)?,
        inst.notional.currency(),
    )?;

    // Check for discrete dividends — if present, adjust spot and zero out q
    let future_divs = future_dividends(inst, disc_curve.as_ref(), as_of)?;

    let (spot, q) = if !future_divs.is_empty() {
        // Escrowed dividend model: adjust spot, set q=0
        // Dividend amounts are already discounted with their own ex-date DFs.
        let s_adj = adjust_spot_for_discrete_dividends(raw_spot, 0.0, &future_divs)?;
        (s_adj, 0.0)
    } else {
        // Continuous dividend yield from scalar id if provided
        //
        // When a dividend yield ID is explicitly provided, we require the lookup to succeed
        // and return a unitless scalar. Silent fallback to 0.0 would mask market data
        // configuration errors.
        let q = if let Some(div_id) = &inst.div_yield_id {
            let ms = curves.get_price(div_id.as_str()).map_err(|e| {
                finstack_quant_core::Error::Validation(format!(
                    "Failed to fetch dividend yield '{}': {}",
                    div_id, e
                ))
            })?;
            match ms {
                finstack_quant_core::market_data::scalars::MarketScalar::Unitless(v) => *v,
                finstack_quant_core::market_data::scalars::MarketScalar::Price(m) => {
                    return Err(finstack_quant_core::Error::Validation(format!(
                        "Dividend yield '{}' should be a unitless scalar, got Price({})",
                        div_id,
                        m.currency()
                    )));
                }
            }
        } else {
            0.0
        };
        (raw_spot, q)
    };

    let sigma = crate::instruments::common_impl::vol_resolution::resolve_sigma_at(
        &inst.instrument_pricing_overrides.market_quotes,
        curves,
        inst.vol_surface_id.as_str(),
        t_vol,
        inst.strike,
    )?;

    Ok(EquityOptionInputs {
        spot,
        r,
        q,
        sigma,
        t_vol,
    })
}

fn future_dividend_amounts(inst: &EquityOption, as_of: Date) -> Result<Vec<(f64, f64)>> {
    inst.discrete_dividends
        .iter()
        .filter(|(date, _)| *date > as_of && *date <= inst.expiry)
        .map(|(date, amount)| Ok((year_fraction(DayCount::Act365F, as_of, *date)?, *amount)))
        .collect()
}

pub(crate) fn early_exercise_market_params(
    inst: &EquityOption,
    curves: &MarketContext,
    as_of: Date,
) -> Result<(OptionMarketParams, Vec<(f64, f64)>)> {
    let inputs = collect_inputs_extended(inst, curves, as_of)?;
    let dividends = future_dividend_amounts(inst, as_of)?;
    let spot = if dividends.is_empty() {
        inputs.spot
    } else {
        crate::instruments::common_impl::helpers::scalar_price_amount(
            curves.get_price(&inst.spot_id)?,
            inst.notional.currency(),
        )?
    };
    let params = OptionMarketParams {
        spot,
        strike: inst.strike,
        rate: inputs.r,
        dividend_yield: if dividends.is_empty() { inputs.q } else { 0.0 },
        volatility: inputs.sigma,
        time_to_expiry: inputs.t_vol,
        option_type: inst.option_type,
    };
    params.validate()?;
    Ok((params, dividends))
}

/// Adjust spot price for discrete dividends using the present-value method.
///
/// This is the QuantLib-standard approach for handling discrete dividends in
/// the Black-Scholes framework. The adjusted spot replaces the original spot
/// in all BS formulas (pricing, Greeks, implied vol):
///
/// ```text
/// S_adj = S - Σ D_i × e^{-r × t_i}
/// ```
///
/// where:
/// - `S` = current spot price
/// - `D_i` = dividend amount at time `t_i`
/// - `r` = risk-free rate
/// - `t_i` = time to dividend payment in years (only dividends before expiry)
///
/// # Arguments
///
/// * `spot` - Current spot price of the underlying
/// * `rate` - Risk-free rate (annualized, continuous compounding)
/// * `dividends` - Slice of `(time_to_payment, dividend_amount)` pairs
///   where `time_to_payment` is in years from valuation date
///
/// # Errors
///
/// Returns a validation error when spot is non-finite/non-positive or when
/// the present value of future dividends is greater than or equal to spot.
///
/// # References
///
/// - Hull, J. C. (2018). *Options, Futures, and Other Derivatives*, Chapter 15. `docs/REFERENCES.md#hull-options-futures`
/// - QuantLib: `DividendVanillaOption` with `AnalyticEuropeanEngine`
pub(crate) fn adjust_spot_for_discrete_dividends(
    spot: f64,
    rate: f64,
    dividends: &[(f64, f64)],
) -> Result<f64> {
    if !spot.is_finite() || spot <= 0.0 {
        return Err(finstack_quant_core::Error::Validation(format!(
            "discrete-dividend spot must be finite and positive, got {spot}"
        )));
    }
    let pv_dividends: f64 = dividends
        .iter()
        .filter(|(t, _)| *t > 0.0)
        .map(|(t, d)| d * (-rate * t).exp())
        .sum();
    let adjusted = spot - pv_dividends;
    if !adjusted.is_finite() || adjusted <= 0.0 {
        return Err(finstack_quant_core::Error::Validation(format!(
            "escrowed-dividend model invalid: spot={spot}, PV(dividends)={pv_dividends}, \
             adjusted_spot={adjusted}; use an explicit dividend-jump model"
        )));
    }
    Ok(adjusted)
}

/// Sensitivity of the escrowed (dividend-adjusted) spot to the risk-free rate.
///
/// With the escrowed-dividend model `S* = S − Σ D_i · e^{−r·t_i}`, the adjusted
/// spot itself depends on `r`:
///
/// ```text
/// ∂S*/∂r = Σ D_i · t_i · e^{−r·t_i}
/// ```
///
/// This term is required to obtain a correct rho: the Black–Scholes `rho`
/// computed from `S*` holds `S*` fixed and therefore misses the
/// `∂V/∂S* · ∂S*/∂r` contribution. Total rho is
/// `rho_total = rho_BS(S*) + delta(S*) · ∂S*/∂r`.
///
/// Returns `0.0` when no future dividends are present (the adjusted spot is
/// then rate-independent). Invalid non-positive adjusted spots are rejected by
/// [`adjust_spot_for_discrete_dividends`] before this derivative is used.
#[must_use]
pub(crate) fn escrowed_spot_drho(rate: f64, dividends: &[(f64, f64)]) -> f64 {
    dividends
        .iter()
        .filter(|(t, _)| *t > 0.0)
        .map(|(t, d)| d * t * (-rate * t).exp())
        .sum()
}
