//! Equity option Black–Scholes pricing engine and greeks.
//!
//! Provides deterministic PV and greeks for `EquityOption` using the
//! Black–Scholes model with continuous dividend yield. Volatility is
//! sourced from a surface (clamped) unless overridden. This mirrors the
//! structure used by `fx_option` and keeps pricing logic separate from
//! instrument definitions.

use crate::instruments::common_impl::helpers::year_fraction;
use crate::instruments::common_impl::parameters::{OptionMarketParams, OptionType};
use crate::instruments::equity::equity_option::types::EquityOption;
use crate::instruments::ExerciseStyle;
use crate::models::trees::binomial_tree::BinomialTree;
use crate::models::{bs_greeks, bs_price, BsGreeks};
use crate::pricer::{ModelKey, PricingError, PricingErrorContext};
use finstack_quant_core::currency::Currency;
use finstack_quant_core::dates::{Date, DayCount};
use finstack_quant_core::market_data::context::MarketContext;
use finstack_quant_core::money::Money;
use finstack_quant_core::Result;

/// Trading days per year for equity options (market standard for theta calculations)
const TRADING_DAYS_PER_YEAR: f64 = 252.0;

/// Reject exercise styles that a selected model does not actually model.
pub(crate) fn require_european(inst: &EquityOption, model: &str) -> Result<()> {
    if !matches!(inst.exercise_style, ExerciseStyle::European) {
        return Err(finstack_quant_core::Error::Validation(format!(
            "{model} supports European EquityOption exercise only; got {:?}",
            inst.exercise_style
        )));
    }
    Ok(())
}

/// Present value using Black–Scholes; result currency is the instrument currency.
pub(crate) fn compute_pv(
    inst: &EquityOption,
    curves: &MarketContext,
    as_of: Date,
) -> Result<Money> {
    if as_of > inst.expiry {
        return Ok(Money::new(0.0, option_currency(inst)));
    }
    let (spot, r, q, sigma, t) = collect_inputs(inst, curves, as_of)?;
    let ccy = option_currency(inst);

    if t <= 0.0 {
        // Expired: intrinsic value scaled by notional amount
        let intrinsic = match inst.option_type {
            OptionType::Call => (spot - inst.strike).max(0.0),
            OptionType::Put => (inst.strike - spot).max(0.0),
        };
        return Ok(Money::new(intrinsic * inst.notional.amount(), ccy));
    }

    // Dispatch based on exercise style
    let unit_price = match inst.exercise_style {
        ExerciseStyle::European => {
            price_bs_unit(spot, inst.strike, r, q, sigma, t, inst.option_type)
        }
        ExerciseStyle::American => {
            // Use Leisen-Reimer tree for American options
            let steps = inst
                .instrument_pricing_overrides
                .model_config
                .tree_steps
                .unwrap_or(201);
            let tree = BinomialTree::leisen_reimer(steps);
            let params = OptionMarketParams {
                spot,
                strike: inst.strike,
                rate: r,
                dividend_yield: q,
                volatility: sigma,
                time_to_expiry: t,
                option_type: inst.option_type,
            };
            tree.price_american(&params)?
        }
        ExerciseStyle::Bermudan => {
            let schedule = inst.exercise_schedule.as_ref().ok_or_else(|| {
                finstack_quant_core::Error::Validation(
                    "Bermudan equity option requires exercise_schedule".to_string(),
                )
            })?;
            let steps = inst
                .instrument_pricing_overrides
                .model_config
                .tree_steps
                .unwrap_or(201);
            let tree = BinomialTree::leisen_reimer(steps);
            let params = OptionMarketParams {
                spot,
                strike: inst.strike,
                rate: r,
                dividend_yield: q,
                volatility: sigma,
                time_to_expiry: t,
                option_type: inst.option_type,
            };
            let exercise_times: Vec<f64> = schedule
                .iter()
                .filter_map(|d| {
                    let yf = DayCount::Act365F
                        .year_fraction(as_of, *d, Default::default())
                        .ok()?;
                    if yf > 0.0 && yf <= t {
                        Some(yf)
                    } else {
                        None
                    }
                })
                .collect();
            if exercise_times.is_empty() {
                return Err(finstack_quant_core::Error::Validation(
                    "Bermudan equity option has no exercise dates remaining after valuation date"
                        .to_string(),
                ));
            }
            tree.price_bermudan(&params, &exercise_times)?
        }
    };

    Ok(Money::new(unit_price * inst.notional.amount(), ccy))
}

pub(crate) fn option_currency(inst: &EquityOption) -> Currency {
    inst.notional.currency()
}

/// Collected market inputs for equity option pricing.
///
/// Separates time-to-expiry calculations by day count convention:
/// - `t_rate`: Time using the discount curve's day count (for rate lookups)
/// - `t_vol`: Time using ACT/365F (equity vol market standard)
#[derive(Debug, Clone, Copy)]
pub(crate) struct EquityOptionInputs {
    /// Spot price of the underlying
    pub(crate) spot: f64,
    /// Risk-free rate (from discount curve)
    /// Effective risk-free rate consistent with `t_vol`
    pub(crate) r: f64,
    /// Dividend yield
    pub(crate) q: f64,
    /// Implied volatility
    pub(crate) sigma: f64,
    /// Time to expiry for rate calculations (curve day count)
    #[allow(dead_code)] // part of public API result struct
    pub(crate) t_rate: f64,
    /// Time to expiry for vol calculations (ACT/365F standard)
    pub(crate) t_vol: f64,
}

/// Collect standard inputs (spot, risk-free, dividend yield, vol, time to expiry).
///
/// **Day Count Convention Handling:**
/// - Rate calculations use the discount curve's own day count
/// - Vol surface lookups use ACT/365F (equity market standard)
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

/// Collect inputs with separate rate and vol time fractions.
///
/// Returns `EquityOptionInputs` with properly separated day count handling:
/// - `t_rate`: Uses the discount curve's day count for rate lookups
/// - `t_vol`: Uses ACT/365F for volatility surface lookups (equity market standard)
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

fn future_dividends(
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
    // Two-clock rate handling (W-35).
    //
    // The discount factor and the volatility surface live on *different* day
    // counts: the discount curve uses its own convention (e.g. ACT/360 for an
    // OIS curve), the vol surface uses ACT/365F (the equity-vol market
    // standard). The two clocks must be kept separate:
    //
    //  - `t_rate` (curve clock): retained for curve-native reporting and
    //    downstream diagnostics. The economic DF is read date-to-date so a
    //    seasoned curve is correctly rebased at `as_of`.
    //  - `t_vol` (ACT/365F): the time-to-expiry that drives the whole
    //    Black–Scholes calculation — `d1`/`d2`, the carry term `(r−q)·t_vol`
    //    and the discount term `e^{−r·t_vol}`.
    //
    // Black–Scholes is a single-`T` model: it applies one time `t_vol` to both
    // the discount and the carry. The *effective* rate `r` must therefore be
    // the rate that, compounded over the BSM clock `t_vol`, reproduces the true
    // curve discount factor:
    //
    //     e^{−r · t_vol} = df            ⟹   r = −ln(df) / t_vol
    //
    // Dividing by `t_vol` (NOT `t_rate`) is deliberate and correct: it is the
    // step that *bridges* the two clocks. It guarantees both legs of BSM are
    // right — the discount `e^{−r·t_vol}` equals `df` exactly, and the forward
    // `F = S·e^{(r−q)·t_vol}` equals the no-arbitrage forward `(S/df)·e^{−q·t_vol}`.
    // Using `r = −ln(df)/t_rate` here would instead make `e^{−r·t_vol} ≠ df`
    // whenever the clocks differ, mispricing both the discount and the carry.
    let disc_curve = curves.get_discount(inst.discount_curve_id.as_str())?;
    let t_rate = year_fraction(disc_curve.day_count(), as_of, inst.expiry)?;
    let df = disc_curve.df_between_dates(as_of, inst.expiry)?;

    // Vol time uses ACT/365F (equity market standard for vol surfaces)
    // This is consistent with how equity volatility is quoted in the market
    let t_vol = year_fraction(DayCount::Act365F, as_of, inst.expiry)?;
    // Effective BSM rate on the vol clock — see the two-clock note above.
    let r = if t_vol > 0.0 { -df.ln() / t_vol } else { 0.0 };

    // Spot from scalar id (unitless or price)
    let spot_scalar = curves.get_price(&inst.spot_id)?;
    let raw_spot = match spot_scalar {
        finstack_quant_core::market_data::scalars::MarketScalar::Unitless(v) => *v,
        finstack_quant_core::market_data::scalars::MarketScalar::Price(m) => m.amount(),
    };

    // Check for discrete dividends — if present, adjust spot and zero out q
    let future_divs = future_dividends(inst, disc_curve.as_ref(), as_of)?;

    let (spot, q) = if !future_divs.is_empty() {
        // Escrowed dividend model: adjust spot, set q=0
        // Dividend amounts are already discounted with their own ex-date DFs.
        let s_adj = adjust_spot_for_discrete_dividends(raw_spot, 0.0, &future_divs);
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
        t_rate,
        t_vol,
    })
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
/// # Returns
///
/// Adjusted spot price. Always returns at least `1e-8` to avoid degenerate
/// pricing when PV of dividends exceeds spot (deep ITM dividend scenario).
///
/// # Example
///
/// ```ignore
/// # fn main() {
/// // Stock at $100, dividend of $2 in 0.25 years, r = 5%
/// // s_adj ≈ 100 - 2 × e^{-0.05×0.25} ≈ 98.01
/// let s_adj = 100.0 - 2.0 * (-0.05_f64 * 0.25).exp();
/// assert!((s_adj - 98.01).abs() < 0.01);
/// # }
/// ```
///
/// # References
///
/// - Hull, J. C. (2018). *Options, Futures, and Other Derivatives*, Chapter 15.
/// - QuantLib: `DividendVanillaOption` with `AnalyticEuropeanEngine`
pub(crate) fn adjust_spot_for_discrete_dividends(
    spot: f64,
    rate: f64,
    dividends: &[(f64, f64)],
) -> f64 {
    let pv_dividends: f64 = dividends
        .iter()
        .filter(|(t, _)| *t > 0.0)
        .map(|(t, d)| d * (-rate * t).exp())
        .sum();
    (spot - pv_dividends).max(1e-8)
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
/// then `r`-independent). The clamp floor applied by
/// [`adjust_spot_for_discrete_dividends`] is intentionally *not* differentiated
/// here: in the clamped (degenerate, PV-of-dividends ≥ spot) regime `S*` is a
/// constant `1e-8` and its true rate derivative is zero, so callers must guard
/// that case separately if they need it.
#[must_use]
pub(crate) fn escrowed_spot_drho(rate: f64, dividends: &[(f64, f64)]) -> f64 {
    dividends
        .iter()
        .filter(|(t, _)| *t > 0.0)
        .map(|(t, d)| d * t * (-rate * t).exp())
        .sum()
}

/// Unit price under Black–Scholes (no contract size scaling).
#[inline]
pub(crate) fn price_bs_unit(
    spot: f64,
    strike: f64,
    r: f64,
    q: f64,
    sigma: f64,
    t: f64,
    option_type: OptionType,
) -> f64 {
    bs_price(spot, strike, r, q, sigma, t, option_type)
}

/// Cash greeks for an equity option (scaled by contract size; vega per 1% vol).
#[derive(Debug, Clone, Copy, Default)]
pub struct EquityOptionGreeks {
    /// Delta: sensitivity to underlying price (scaled by contract size)
    pub delta: f64,
    /// Gamma: rate of change of delta with respect to underlying price
    pub gamma: f64,
    /// Vega: sensitivity to 1% change in volatility
    pub vega: f64,
    /// Theta: time decay per day
    pub theta: f64,
    /// Rho: sensitivity to 1% change in risk-free rate
    pub rho: f64,
}

/// Compute greeks consistent with the pricing inputs.
///
/// Uses proper day count handling:
/// - Rate lookups use the discount curve's day count
/// - Vol time uses ACT/365F (equity market standard)
pub(crate) fn compute_greeks(
    inst: &EquityOption,
    curves: &MarketContext,
    as_of: Date,
) -> Result<EquityOptionGreeks> {
    if as_of > inst.expiry {
        return Ok(EquityOptionGreeks::default());
    }
    let inputs = collect_inputs_extended(inst, curves, as_of)?;
    let (spot, r, q, sigma, t) = (inputs.spot, inputs.r, inputs.q, inputs.sigma, inputs.t_vol);

    if t <= 0.0 {
        // At expiry, delta is the step function of the payoff.
        // ATM (spot == strike) uses the convention 0.5 / -0.5,
        // consistent with QuantLib and Bloomberg.
        let strike = inst.strike;
        let delta_unit = match inst.option_type {
            OptionType::Call => {
                if spot > strike {
                    1.0
                } else if (spot - strike).abs() < 1e-12 * strike.abs().max(1.0) {
                    0.5
                } else {
                    0.0
                }
            }
            OptionType::Put => {
                if spot < strike {
                    -1.0
                } else if (spot - strike).abs() < 1e-12 * strike.abs().max(1.0) {
                    -0.5
                } else {
                    0.0
                }
            }
        };
        let scale = inst.notional.amount();
        return Ok(EquityOptionGreeks {
            delta: delta_unit * scale,
            ..Default::default()
        });
    }

    match inst.exercise_style {
        ExerciseStyle::European => {
            let greeks_unit = bs_greeks(
                spot,
                inst.strike,
                r,
                q,
                sigma,
                t,
                inst.option_type,
                TRADING_DAYS_PER_YEAR,
            );

            // Escrowed-dividend rho correction.
            //
            // Under the escrowed-dividend model the BS inputs use the adjusted
            // spot `S* = S − Σ D_i·e^{−r·t_i}`, which itself depends on `r`.
            // `bs_greeks` computes rho holding `S*` fixed, so it misses the
            // `∂V/∂S* · ∂S*/∂r` chain-rule term. Total rho is
            //   rho_total = rho_BS(S*) + delta(S*) · ∂S*/∂r,
            // expressed per 1% rate move (hence the `ONE_PERCENT` factor:
            // `greeks_unit.rho_r` and `vega` are already per-1%, while
            // `delta` and `∂S*/∂r` are per-unit).
            let rho_unit = {
                let disc_curve = curves.get_discount(inst.discount_curve_id.as_str())?;
                let future_divs = future_dividends(inst, disc_curve.as_ref(), as_of)?;
                if future_divs.is_empty() {
                    greeks_unit.rho_r
                } else {
                    // In the degenerate clamped regime S* is pinned at the
                    // 1e-8 floor and is genuinely r-independent (∂S*/∂r = 0).
                    let ds_star_dr = if spot <= 1e-8 {
                        0.0
                    } else {
                        // Entries already contain PV(D_i); under a parallel
                        // continuously-compounded rate bump, dS*/dr = Σt_i PV(D_i).
                        escrowed_spot_drho(0.0, &future_divs)
                    };
                    const ONE_PERCENT: f64 = 0.01;
                    greeks_unit.rho_r + greeks_unit.delta * ds_star_dr * ONE_PERCENT
                }
            };

            let scale = inst.notional.amount();
            Ok(EquityOptionGreeks {
                delta: greeks_unit.delta * scale,
                gamma: greeks_unit.gamma * scale,
                vega: greeks_unit.vega * scale,
                theta: greeks_unit.theta * scale,
                rho: rho_unit * scale,
            })
        }
        ExerciseStyle::American => {
            // American: Use Tree with Finite Differences
            let steps = inst
                .instrument_pricing_overrides
                .model_config
                .tree_steps
                .unwrap_or(201);
            let tree = BinomialTree::leisen_reimer(steps);
            let params = OptionMarketParams {
                spot,
                strike: inst.strike,
                rate: r,
                dividend_yield: q,
                volatility: sigma,
                time_to_expiry: t,
                option_type: inst.option_type,
            };

            // Helper to price
            let price_fn = |p: &OptionMarketParams| -> Result<f64> { tree.price_american(p) };

            let scale = inst.notional.amount();
            tree_finite_difference_greeks(&params, scale, price_fn)
        }
        ExerciseStyle::Bermudan => {
            let schedule = inst.exercise_schedule.as_ref().ok_or_else(|| {
                finstack_quant_core::Error::Validation(
                    "Bermudan equity option requires exercise_schedule".to_string(),
                )
            })?;
            let steps = inst
                .instrument_pricing_overrides
                .model_config
                .tree_steps
                .unwrap_or(201);
            let tree = BinomialTree::leisen_reimer(steps);
            let params = OptionMarketParams {
                spot,
                strike: inst.strike,
                rate: r,
                dividend_yield: q,
                volatility: sigma,
                time_to_expiry: t,
                option_type: inst.option_type,
            };
            let exercise_times: Vec<f64> = schedule
                .iter()
                .filter_map(|d| {
                    let yf = DayCount::Act365F
                        .year_fraction(as_of, *d, Default::default())
                        .ok()?;
                    if yf > 0.0 && yf <= t {
                        Some(yf)
                    } else {
                        None
                    }
                })
                .collect();
            if exercise_times.is_empty() {
                return Err(finstack_quant_core::Error::Validation(
                    "Bermudan equity option has no exercise dates remaining after valuation date"
                        .to_string(),
                ));
            }

            let price_fn =
                |p: &OptionMarketParams| -> Result<f64> { tree.price_bermudan(p, &exercise_times) };

            let scale = inst.notional.amount();
            tree_finite_difference_greeks(&params, scale, price_fn)
        }
    }
}

fn tree_finite_difference_greeks(
    params: &OptionMarketParams,
    scale: f64,
    mut price_fn: impl FnMut(&OptionMarketParams) -> Result<f64>,
) -> Result<EquityOptionGreeks> {
    let base_price = price_fn(params)?;

    // Delta: small 1%-of-spot central bump (accuracy-limited; the first
    // difference's noise is O(ε_tree / h), so a small bump is fine).
    let h_s = params.spot * 0.01;
    let mut p_up = params.clone();
    p_up.spot += h_s;
    let price_up = price_fn(&p_up)?;
    let mut p_dn = params.clone();
    p_dn.spot -= h_s;
    let price_dn = price_fn(&p_dn)?;

    let delta_unit = (price_up - price_dn) / (2.0 * h_s);

    // Gamma: a 1%-of-spot bump is too small. The central second difference
    // `(p_up − 2·base + p_dn) / h²` has noise of order `ε_tree / h²`, which a
    // 1% bump leaves noise-dominated — gamma is then noisy and biased,
    // especially for short-dated options where the tree's discrete spot grid
    // makes `P(S)` locally piecewise-flat.
    //
    // Use a wider, better-conditioned gamma bump sized to the option's natural
    // spot scale `σ·√t` (the width of the region where gamma actually lives),
    // with a 2%-of-spot floor so the bump never collapses for short-dated /
    // low-vol options. This trades a small, bounded discretisation bias for a
    // large reduction in second-difference noise. A separate, dedicated
    // re-pricing pair is used so the delta bump stays small for accuracy.
    let gamma_unit = {
        let vol_t = params.volatility * params.time_to_expiry.max(0.0).sqrt();
        let h_g = params.spot * vol_t.max(0.02);
        let mut p_g_up = params.clone();
        p_g_up.spot += h_g;
        let price_g_up = price_fn(&p_g_up)?;
        let mut p_g_dn = params.clone();
        p_g_dn.spot = (p_g_dn.spot - h_g).max(1e-8);
        let price_g_dn = price_fn(&p_g_dn)?;
        let h_dn = params.spot - p_g_dn.spot;
        // Non-uniform three-point second derivative. When the down bump is
        // clamped, a symmetric stencil would leak the first derivative into
        // gamma and can dominate the result.
        2.0 * ((price_g_up - base_price) / h_g - (base_price - price_g_dn) / h_dn) / (h_g + h_dn)
    };

    // Vega (1% vol bump)
    let h_v = 0.01;
    let mut p_v_up = params.clone();
    p_v_up.volatility += h_v;
    let price_v_up = price_fn(&p_v_up)?;
    let mut p_v_dn = params.clone();
    p_v_dn.volatility = (p_v_dn.volatility - h_v).max(1e-8);
    let price_v_dn = price_fn(&p_v_dn)?;
    let actual_vol_width = p_v_up.volatility - p_v_dn.volatility;
    let vega_unit = (price_v_up - price_v_dn) / actual_vol_width * h_v;

    // Rho (1% rate bump)
    let h_r = 0.01;
    let mut p_r_up = params.clone();
    p_r_up.rate += h_r;
    let price_r_up = price_fn(&p_r_up)?;
    let mut p_r_dn = params.clone();
    p_r_dn.rate -= h_r;
    let price_r_dn = price_fn(&p_r_dn)?;
    let rho_unit = (price_r_up - price_r_dn) / 2.0;

    // Theta (1 trading-day bump)
    let dt = 1.0 / TRADING_DAYS_PER_YEAR;
    let theta_unit = if params.time_to_expiry > dt {
        let mut p_t = params.clone();
        p_t.time_to_expiry -= dt;
        let price_t = price_fn(&p_t)?;
        price_t - base_price
    } else {
        0.0
    };

    Ok(EquityOptionGreeks {
        delta: delta_unit * scale,
        gamma: gamma_unit * scale,
        vega: vega_unit * scale,
        theta: theta_unit * scale,
        rho: rho_unit * scale,
    })
}

/// Unit greeks (per share, not scaled by contract size).
pub(crate) type UnitGreeks = BsGreeks;

/// Compute unit greeks from explicit inputs (no market lookups).
#[allow(dead_code)] // May be used by external bindings or tests
#[inline]
pub(crate) fn greeks_unit(
    spot: f64,
    strike: f64,
    r: f64,
    q: f64,
    sigma: f64,
    t: f64,
    option_type: OptionType,
) -> UnitGreeks {
    if t <= 0.0 {
        // ATM convention: 0.5 / -0.5 (QuantLib/Bloomberg standard)
        let delta = match option_type {
            OptionType::Call => {
                if spot > strike {
                    1.0
                } else if (spot - strike).abs() < 1e-12 * strike.abs().max(1.0) {
                    0.5
                } else {
                    0.0
                }
            }
            OptionType::Put => {
                if spot < strike {
                    -1.0
                } else if (spot - strike).abs() < 1e-12 * strike.abs().max(1.0) {
                    -0.5
                } else {
                    0.0
                }
            }
        };
        return UnitGreeks {
            delta,
            ..Default::default()
        };
    }

    bs_greeks(
        spot,
        strike,
        r,
        q,
        sigma,
        t,
        option_type,
        TRADING_DAYS_PER_YEAR,
    )
}

// ========================= REGISTRY PRICER =========================

/// Registry pricer for Equity Option using Black-Scholes model
pub(crate) struct SimpleEquityOptionBlackPricer {
    model: crate::pricer::ModelKey,
}

impl SimpleEquityOptionBlackPricer {
    /// Create new Black-Scholes pricer with default model.
    ///
    /// Uses `ModelKey::Black76` which is the library-wide convention for
    /// lognormal option pricing.  BSM and Black-76 are mathematically
    /// equivalent (BSM is Black-76 applied to the forward
    /// `F = S × exp((r-q)T)`), so the same model key covers both.
    pub(crate) fn new() -> Self {
        Self {
            model: crate::pricer::ModelKey::Black76,
        }
    }

    /// Create pricer with specified model key
    pub(crate) fn with_model(model: crate::pricer::ModelKey) -> Self {
        Self { model }
    }
}

impl Default for SimpleEquityOptionBlackPricer {
    fn default() -> Self {
        Self::new()
    }
}

impl crate::pricer::Pricer for SimpleEquityOptionBlackPricer {
    fn key(&self) -> crate::pricer::PricerKey {
        crate::pricer::PricerKey::new(crate::pricer::InstrumentType::EquityOption, self.model)
    }

    #[tracing::instrument(
        name = "equity_option.black.price_dyn",
        level = "debug",
        skip(self, instrument, market),
        fields(
            pricer = ?self.key(),
            inst_id = %instrument.id(),
            as_of = %as_of,
        ),
        err,
    )]
    fn price_dyn(
        &self,
        instrument: &dyn crate::instruments::common_impl::traits::Instrument,
        market: &finstack_quant_core::market_data::context::MarketContext,
        as_of: finstack_quant_core::dates::Date,
    ) -> std::result::Result<crate::results::ValuationResult, crate::pricer::PricingError> {
        use crate::instruments::common_impl::traits::Instrument;

        // Type-safe downcasting
        let equity_option = instrument
            .as_any()
            .downcast_ref::<crate::instruments::equity::equity_option::EquityOption>()
            .ok_or_else(|| {
                crate::pricer::PricingError::type_mismatch(
                    crate::pricer::InstrumentType::EquityOption,
                    instrument.key(),
                )
            })?;

        // Use the provided as_of date for consistency
        // Compute present value using the engine
        let pv = compute_pv(equity_option, market, as_of).map_err(|e| {
            crate::pricer::PricingError::model_failure_with_context(
                e.to_string(),
                crate::pricer::PricingErrorContext::from_instrument(equity_option)
                    .model(self.model),
            )
        })?;

        // Return stamped result
        Ok(crate::results::ValuationResult::stamped(
            equity_option.id(),
            as_of,
            pv,
        ))
    }
}

// ========================= HESTON FOURIER PRICER =========================

use crate::instruments::common_impl::traits::Instrument;
use crate::models::closed_form::heston::{
    heston_call_price_fourier, heston_put_price_fourier, HestonParams,
};

/// Equity option Heston semi-analytical pricer (Fourier inversion).
pub(crate) struct EquityOptionHestonFourierPricer;

impl EquityOptionHestonFourierPricer {
    /// Create a new Heston Fourier transform pricer
    pub(crate) fn new() -> Self {
        Self
    }
}

impl Default for EquityOptionHestonFourierPricer {
    fn default() -> Self {
        Self::new()
    }
}

impl crate::pricer::Pricer for EquityOptionHestonFourierPricer {
    fn key(&self) -> crate::pricer::PricerKey {
        crate::pricer::PricerKey::new(
            crate::pricer::InstrumentType::EquityOption,
            crate::pricer::ModelKey::HestonFourier,
        )
    }

    #[tracing::instrument(
        name = "equity_option.heston_fourier.price_dyn",
        level = "debug",
        skip(self, instrument, market),
        fields(inst_id = %instrument.id(), as_of = %as_of),
        err,
    )]
    fn price_dyn(
        &self,
        instrument: &dyn crate::instruments::common_impl::traits::Instrument,
        market: &MarketContext,
        as_of: Date,
    ) -> std::result::Result<crate::results::ValuationResult, PricingError> {
        let equity_option = instrument
            .as_any()
            .downcast_ref::<EquityOption>()
            .ok_or_else(|| {
                crate::pricer::PricingError::type_mismatch(
                    crate::pricer::InstrumentType::EquityOption,
                    instrument.key(),
                )
            })?;

        require_european(equity_option, "Heston Fourier").map_err(|e| {
            crate::pricer::PricingError::model_failure_with_context(
                e.to_string(),
                crate::pricer::PricingErrorContext::from_instrument(equity_option)
                    .model(crate::pricer::ModelKey::HestonFourier),
            )
        })?;

        if as_of > equity_option.expiry {
            return Ok(crate::results::ValuationResult::stamped(
                equity_option.id(),
                as_of,
                Money::new(0.0, option_currency(equity_option)),
            ));
        }

        reject_future_discrete_dividends_for_stochastic_vol(
            equity_option,
            as_of,
            crate::pricer::ModelKey::HestonFourier,
            "Heston Fourier",
        )?;

        let inputs = collect_inputs_extended(equity_option, market, as_of).map_err(|e| {
            crate::pricer::PricingError::model_failure_with_context(
                e.to_string(),
                crate::pricer::PricingErrorContext::from_instrument(equity_option)
                    .model(crate::pricer::ModelKey::HestonFourier),
            )
        })?;
        let (spot, r, q, _sigma, t) = (inputs.spot, inputs.r, inputs.q, inputs.sigma, inputs.t_vol);

        if t <= 0.0 {
            let intrinsic = match equity_option.option_type {
                OptionType::Call => (spot - equity_option.strike).max(0.0),
                OptionType::Put => (equity_option.strike - spot).max(0.0),
            };
            return Ok(crate::results::ValuationResult::stamped(
                equity_option.id(),
                as_of,
                Money::new(
                    intrinsic * equity_option.notional.amount(),
                    option_currency(equity_option),
                ),
            ));
        }

        // Source production Heston parameters from explicit market scalars.
        // Validation is still enforced inside `HestonParams::new`.
        let err_ctx = crate::pricer::PricingErrorContext::from_instrument(equity_option)
            .model(crate::pricer::ModelKey::HestonFourier);
        let params = HestonParams::from_market_strict(market, r, q)
            .map_err(|e| crate::pricer::PricingError::from_core(e, err_ctx))?;

        let price = match equity_option.option_type {
            OptionType::Call => heston_call_price_fourier(spot, equity_option.strike, t, &params),
            OptionType::Put => heston_put_price_fourier(spot, equity_option.strike, t, &params),
        };

        let pv = Money::new(
            price * equity_option.notional.amount(),
            option_currency(equity_option),
        );
        Ok(crate::results::ValuationResult::stamped(
            equity_option.id(),
            as_of,
            pv,
        ))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::instruments::equity::equity_option::types::EquityOption;
    use crate::instruments::{Attributes, SettlementType};
    use crate::pricer::Pricer;
    use finstack_quant_core::market_data::context::MarketContext;
    use finstack_quant_core::market_data::scalars::MarketScalar;
    use finstack_quant_core::market_data::surfaces::VolSurface;
    use finstack_quant_core::market_data::term_structures::DiscountCurve;
    use finstack_quant_core::types::{CurveId, InstrumentId};
    use time::Month;

    fn date(year: i32, month: u8, day: u8) -> Date {
        Date::from_calendar_date(year, Month::try_from(month).expect("valid month"), day)
            .expect("valid date")
    }

    fn market(as_of: Date, spot: f64, vol: f64, rate: f64, div_yield: f64) -> MarketContext {
        let curve = DiscountCurve::builder("USD-OIS")
            .base_date(as_of)
            .day_count(DayCount::Act365F)
            .knots([(0.0, 1.0), (10.0, (-rate * 10.0).exp())])
            .build()
            .expect("curve");
        let surface = VolSurface::builder("SPX-VOL")
            .expiries(&[0.25, 0.5, 1.0, 2.0])
            .strikes(&[80.0, 100.0, 120.0, 150.0])
            .row(&[vol, vol, vol, vol])
            .row(&[vol, vol, vol, vol])
            .row(&[vol, vol, vol, vol])
            .row(&[vol, vol, vol, vol])
            .build()
            .expect("surface");

        MarketContext::new()
            .insert(curve)
            .insert_surface(surface)
            .insert_price("SPX-SPOT", MarketScalar::Unitless(spot))
            .insert_price("SPX-DIV", MarketScalar::Unitless(div_yield))
    }

    fn option(
        expiry: Date,
        option_type: OptionType,
        exercise_style: ExerciseStyle,
    ) -> EquityOption {
        EquityOption::builder()
            .id(InstrumentId::new("EQ-OPT-TEST"))
            .underlying_ticker("SPX".to_string())
            .strike(100.0)
            .option_type(option_type)
            .exercise_style(exercise_style)
            .expiry(expiry)
            .notional(Money::new(100.0, Currency::USD))
            .day_count(DayCount::Act365F)
            .settlement(SettlementType::Cash)
            .discount_curve_id(CurveId::new("USD-OIS"))
            .spot_id("SPX-SPOT".into())
            .vol_surface_id(CurveId::new("SPX-VOL"))
            .div_yield_id_opt(Some(CurveId::new("SPX-DIV")))
            .attributes(Attributes::new())
            .build()
            .expect("equity option")
    }

    #[test]
    fn test_adjust_spot_for_discrete_dividends_single() {
        // Stock at $100, dividend of $2 in 0.25 years, r = 5%
        let s_adj = adjust_spot_for_discrete_dividends(100.0, 0.05, &[(0.25, 2.0)]);
        // PV(div) = 2 × e^{-0.05×0.25} ≈ 1.9751
        assert!((s_adj - 98.0248).abs() < 0.01);
    }

    #[test]
    fn test_adjust_spot_for_discrete_dividends_multiple() {
        let s_adj = adjust_spot_for_discrete_dividends(100.0, 0.05, &[(0.25, 1.5), (0.5, 1.5)]);
        let expected = 100.0 - 1.5 * (-0.05 * 0.25_f64).exp() - 1.5 * (-0.05 * 0.5_f64).exp();
        assert!((s_adj - expected).abs() < 1e-10);
    }

    #[test]
    fn test_adjust_spot_for_discrete_dividends_floor() {
        // Dividends exceed spot → clamped to 1e-8
        let s_adj = adjust_spot_for_discrete_dividends(1.0, 0.01, &[(0.1, 50.0)]);
        assert!((s_adj - 1e-8).abs() < 1e-12);
    }

    #[test]
    fn test_adjust_spot_for_discrete_dividends_empty() {
        let s_adj = adjust_spot_for_discrete_dividends(100.0, 0.05, &[]);
        assert!((s_adj - 100.0).abs() < 1e-12);
    }

    #[test]
    fn test_adjust_spot_for_discrete_dividends_skips_past() {
        // Dividend at t=0 or negative should be skipped
        let s_adj = adjust_spot_for_discrete_dividends(100.0, 0.05, &[(0.0, 5.0), (-0.1, 3.0)]);
        assert!((s_adj - 100.0).abs() < 1e-12);
    }

    /// Escrowed-dividend rho must include the `∂S*/∂r` chain-rule term.
    ///
    /// With discrete dividends the BS inputs use `S* = S − Σ D·e^{−r·t}`, which
    /// depends on `r`. The analytic rho from `compute_greeks` must therefore
    /// match a finite-difference rho computed by bumping the discount-curve
    /// rate (which re-derives `S*` at the bumped rate). Before the fix, rho
    /// held `S*` fixed and disagreed with the FD rho by `delta·∂S*/∂r`.
    #[test]
    fn escrowed_dividend_rho_includes_spot_rate_sensitivity() {
        let as_of = date(2025, 1, 1);
        let expiry = date(2026, 1, 1); // ~1y
        let mut opt = option(expiry, OptionType::Call, ExerciseStyle::European);
        // A sizeable dividend mid-life makes ∂S*/∂r materially non-zero.
        opt.discrete_dividends = vec![(date(2025, 7, 1), 8.0)];

        let base_rate = 0.04;
        let analytic = compute_greeks(&opt, &market(as_of, 100.0, 0.20, base_rate, 0.0), as_of)
            .expect("analytic greeks")
            .rho;

        // Central finite-difference rho of the full PV over the curve rate.
        // compute_pv re-derives r (and hence S*) from the curve, so this FD
        // captures the ∂S*/∂r contribution that the analytic rho must match.
        let h = 1e-4; // 1bp in rate space
        let pv_up = compute_pv(&opt, &market(as_of, 100.0, 0.20, base_rate + h, 0.0), as_of)
            .expect("pv up")
            .amount();
        let pv_dn = compute_pv(&opt, &market(as_of, 100.0, 0.20, base_rate - h, 0.0), as_of)
            .expect("pv dn")
            .amount();
        // analytic rho is per 1% (100bp); FD slope per unit-rate * 0.01.
        let fd_rho = (pv_up - pv_dn) / (2.0 * h) * 0.01;

        let denom = analytic.abs().max(fd_rho.abs()).max(1e-9);
        assert!(
            (analytic - fd_rho).abs() / denom < 5e-3,
            "escrowed-dividend rho must match FD rho of the full PV (which \
             re-derives S* at the bumped rate): analytic={analytic} fd={fd_rho}"
        );

        // And it must NOT equal the naive rho that holds S* fixed.
        let inputs =
            collect_inputs_extended(&opt, &market(as_of, 100.0, 0.20, base_rate, 0.0), as_of)
                .expect("inputs");
        let naive = bs_greeks(
            inputs.spot,
            opt.strike,
            inputs.r,
            inputs.q,
            inputs.sigma,
            inputs.t_vol,
            opt.option_type,
            TRADING_DAYS_PER_YEAR,
        )
        .rho_r
            * opt.notional.amount();
        assert!(
            (analytic - naive).abs() / denom > 1e-3,
            "the ∂S*/∂r correction must move rho away from the S*-fixed value: \
             analytic={analytic} naive={naive}"
        );
    }

    #[test]
    fn heston_fourier_rejects_future_discrete_dividend() {
        let as_of = date(2025, 1, 1);
        let expiry = date(2026, 1, 1);
        let mut opt = option(expiry, OptionType::Call, ExerciseStyle::European);
        opt.discrete_dividends = vec![(date(2025, 7, 1), 2.0)];

        let err = EquityOptionHestonFourierPricer::new()
            .price_dyn(&opt, &market(as_of, 100.0, 0.20, 0.03, 0.0), as_of)
            .expect_err("Heston Fourier must reject discrete dividends");
        let msg = err.to_string();
        assert!(
            msg.contains("discrete dividends"),
            "unexpected error message: {msg}"
        );
    }

    #[test]
    fn test_expired_atm_delta_convention_matches_compute_greeks_and_unit_greeks() {
        let as_of = date(2025, 1, 1);
        let call = option(as_of, OptionType::Call, ExerciseStyle::European);
        let put = option(as_of, OptionType::Put, ExerciseStyle::European);
        let curves = market(as_of, 100.0, 0.20, 0.03, 0.01);

        let call_greeks = compute_greeks(&call, &curves, as_of).expect("call greeks");
        let put_greeks = compute_greeks(&put, &curves, as_of).expect("put greeks");
        let call_unit = greeks_unit(100.0, 100.0, 0.03, 0.01, 0.20, 0.0, OptionType::Call);
        let put_unit = greeks_unit(100.0, 100.0, 0.03, 0.01, 0.20, 0.0, OptionType::Put);

        assert_eq!(call_greeks.delta, 50.0);
        assert_eq!(put_greeks.delta, -50.0);
        assert_eq!(call_unit.delta, 0.5);
        assert_eq!(put_unit.delta, -0.5);
        assert_eq!(call_greeks.gamma, 0.0);
        assert_eq!(put_greeks.gamma, 0.0);
    }

    /// Short-dated tree FD gamma must be well-conditioned.
    ///
    /// An American call on a non-dividend-paying underlying is never optimally
    /// exercised early, so its price (and gamma) equals the European value.
    /// For a short-dated near-ATM option the analytic BS gamma is therefore a
    /// reliable oracle. With the old 1%-of-spot gamma bump the tree second
    /// difference is noise-dominated and gamma drifts well off the analytic
    /// value; the wider `σ√t`-scaled bump keeps it close.
    #[test]
    fn short_dated_tree_gamma_is_well_conditioned() {
        let as_of = date(2025, 1, 1);
        // ~3-week expiry: short enough that a 1%-of-spot bump is noise-prone.
        let expiry = date(2025, 1, 22);
        let mut american = option(expiry, OptionType::Call, ExerciseStyle::American);
        american
            .instrument_pricing_overrides
            .model_config
            .tree_steps = Some(201);
        // Zero dividend yield => American call == European call.
        let curves = market(as_of, 100.0, 0.20, 0.03, 0.0);

        let tree_greeks = compute_greeks(&american, &curves, as_of).expect("tree greeks");

        // Analytic European gamma with the same inputs.
        let inputs = collect_inputs_extended(&american, &curves, as_of).expect("inputs");
        let analytic = bs_greeks(
            inputs.spot,
            american.strike,
            inputs.r,
            inputs.q,
            inputs.sigma,
            inputs.t_vol,
            american.option_type,
            TRADING_DAYS_PER_YEAR,
        )
        .gamma
            * american.notional.amount();

        assert!(
            analytic > 0.0 && tree_greeks.gamma > 0.0,
            "gamma must be positive: analytic={analytic} tree={}",
            tree_greeks.gamma
        );
        let rel_err = (tree_greeks.gamma - analytic).abs() / analytic;
        assert!(
            rel_err < 0.05,
            "short-dated tree gamma must track analytic gamma within 5%: \
             analytic={analytic} tree={} rel_err={rel_err}",
            tree_greeks.gamma
        );
    }

    #[test]
    fn test_american_call_tree_path_prices_above_european() {
        let as_of = date(2025, 1, 1);
        let expiry = date(2025, 7, 1);
        let mut european = option(expiry, OptionType::Call, ExerciseStyle::European);
        let mut american = option(expiry, OptionType::Call, ExerciseStyle::American);
        european
            .instrument_pricing_overrides
            .model_config
            .tree_steps = Some(51);
        american
            .instrument_pricing_overrides
            .model_config
            .tree_steps = Some(51);
        let curves = market(as_of, 105.0, 0.22, 0.03, 0.01);

        let european_pv = compute_pv(&european, &curves, as_of).expect("european pv");
        let american_pv = compute_pv(&american, &curves, as_of).expect("american pv");

        assert!(american_pv.amount().is_finite());
        assert!(american_pv.amount() >= european_pv.amount());
    }

    #[test]
    fn test_bermudan_schedule_filters_invalid_dates_before_tree_pricing() {
        let as_of = date(2025, 1, 1);
        let expiry = date(2025, 7, 1);
        let mut filtered = option(expiry, OptionType::Put, ExerciseStyle::Bermudan);
        let mut noisy = option(expiry, OptionType::Put, ExerciseStyle::Bermudan);
        filtered
            .instrument_pricing_overrides
            .model_config
            .tree_steps = Some(51);
        noisy.instrument_pricing_overrides.model_config.tree_steps = Some(51);
        filtered.exercise_schedule = Some(vec![date(2025, 3, 1), date(2025, 5, 1)]);
        noisy.exercise_schedule = Some(vec![
            as_of,
            date(2024, 12, 15),
            date(2025, 3, 1),
            date(2025, 5, 1),
            date(2025, 8, 1),
        ]);
        let curves = market(as_of, 95.0, 0.25, 0.03, 0.0);

        let filtered_pv = compute_pv(&filtered, &curves, as_of).expect("filtered bermudan pv");
        let noisy_pv = compute_pv(&noisy, &curves, as_of).expect("noisy bermudan pv");

        assert!((filtered_pv.amount() - noisy_pv.amount()).abs() < 1e-10);
    }

    #[test]
    fn post_expiry_value_and_greeks_are_zero_without_market_data() {
        let expiry = date(2025, 1, 1);
        let as_of = date(2025, 1, 2);
        let option = option(expiry, OptionType::Call, ExerciseStyle::European);
        let empty = MarketContext::new();
        let pv = compute_pv(&option, &empty, as_of).expect("post-expiry PV");
        let greeks = compute_greeks(&option, &empty, as_of).expect("post-expiry greeks");
        assert_eq!(pv.amount(), 0.0);
        assert_eq!(greeks.delta, 0.0);
        assert_eq!(greeks.gamma, 0.0);
        assert_eq!(greeks.vega, 0.0);
    }
}
