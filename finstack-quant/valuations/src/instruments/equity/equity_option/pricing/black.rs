//! Black–Scholes / tree pricing and cash greeks for equity options.

use super::inputs::{
    collect_inputs, collect_inputs_extended, early_exercise_market_params, escrowed_spot_drho,
    future_dividends, resolve_lifecycle_value,
};
use crate::instruments::common_impl::helpers::year_fraction;
use crate::instruments::common_impl::parameters::{OptionMarketParams, OptionType};
use crate::instruments::equity::equity_option::types::EquityOption;
use crate::instruments::{ExerciseStyle, SettlementType};
use crate::pricer::expect_inst;
use finstack_quant_core::dates::{Date, DayCount};
use finstack_quant_core::market_data::context::MarketContext;
use finstack_quant_core::money::Money;
use finstack_quant_core::Result;
use finstack_quant_models::closed_form::vanilla::{bs_greeks_unchecked, bs_price_unchecked};
use finstack_quant_models::trees::binomial_tree::BinomialTree;

/// Present value using Black–Scholes; result currency is the instrument currency.
pub(crate) fn compute_pv(
    inst: &EquityOption,
    curves: &MarketContext,
    as_of: Date,
) -> Result<Money> {
    if let Some(value) = resolve_lifecycle_value(inst, curves, as_of)? {
        return Ok(value);
    }
    let ccy = inst.notional.currency();
    let unit_price = match inst.exercise_style {
        ExerciseStyle::European => {
            let (spot, r, q, sigma, t) = collect_inputs(inst, curves, as_of)?;
            bs_price_unchecked(spot, inst.strike, r, q, sigma, t, inst.option_type)
        }
        ExerciseStyle::American => {
            let steps = inst
                .instrument_pricing_overrides
                .model_config
                .tree_steps
                .unwrap_or(201);
            let tree = BinomialTree::leisen_reimer(steps);
            let (params, dividends) = early_exercise_market_params(inst, curves, as_of)?;
            if dividends.is_empty() {
                tree.price_american(&params)?
            } else {
                tree.price_american_with_discrete_dividends(&params, &dividends)?
            }
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
            let (params, dividends) = early_exercise_market_params(inst, curves, as_of)?;
            let exercise_times: Vec<f64> = schedule
                .iter()
                .filter_map(|date| {
                    let year_fraction = DayCount::Act365F
                        .year_fraction(as_of, *date, Default::default())
                        .ok()?;
                    (year_fraction > 0.0 && year_fraction <= params.time_to_expiry)
                        .then_some(year_fraction)
                })
                .collect();
            if exercise_times.is_empty() {
                return Err(finstack_quant_core::Error::Validation(
                    "Bermudan equity option has no exercise dates remaining after valuation date"
                        .to_string(),
                ));
            }
            if dividends.is_empty() {
                tree.price_bermudan(&params, &exercise_times)?
            } else {
                tree.price_bermudan_with_discrete_dividends(&params, &exercise_times, &dividends)?
            }
        }
    };

    let unit_price = finstack_quant_models::closed_form::checked_closed_form_value(
        unit_price,
        "equity option unit price",
    )?;
    Money::new(unit_price * inst.notional.amount(), ccy)
}
/// Cash greeks for an equity option (scaled by contract size; vega per 1% vol).
#[derive(Debug, Clone, Copy, Default, PartialEq)]
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
    if resolve_lifecycle_value(inst, curves, as_of)?.is_some() {
        let delta = match (inst.exercise, inst.settlement) {
            (Some(exercise), SettlementType::Physical)
                if exercise.exercised && as_of <= exercise.settlement_date =>
            {
                let has_discrete_dividend = inst
                    .discrete_dividends
                    .iter()
                    .any(|(date, _)| *date > as_of && *date <= exercise.settlement_date);
                let carry = if has_discrete_dividend {
                    1.0
                } else {
                    let q = if let Some(dividend_yield_id) = &inst.div_yield_id {
                        match curves.get_price(dividend_yield_id.as_str())? {
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
                    let t = year_fraction(DayCount::Act365F, as_of, exercise.settlement_date)?;
                    (-q * t).exp()
                };
                let direction = match inst.option_type {
                    OptionType::Call => 1.0,
                    OptionType::Put => -1.0,
                };
                direction * carry * inst.notional.amount()
            }
            _ => 0.0,
        };
        return Ok(EquityOptionGreeks {
            delta,
            ..Default::default()
        });
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
            let greeks_unit = bs_greeks_unchecked(
                spot,
                inst.strike,
                r,
                q,
                sigma,
                t,
                inst.option_type,
                inst.theta_day_basis.days_per_year(),
            );

            // Escrowed-dividend rho correction.
            //
            // Under the escrowed-dividend model the BS inputs use the adjusted
            // spot `S* = S − Σ D_i·e^{−r·t_i}`, which itself depends on `r`.
            // `bs_greeks` computes rho holding `S*` fixed, so it misses the
            // `∂V/∂S* · ∂S*/∂r` chain-rule term. Total rho is
            // rho_total = rho_BS(S*) + delta(S*) · ∂S*/∂r,
            // expressed per 1% rate move (hence the `ONE_PERCENT` factor:
            // `greeks_unit.rho_r` and `vega` are already per-1%, while
            // `delta` and `∂S*/∂r` are per-unit).
            let rho_unit = {
                let disc_curve = curves.get_discount(inst.discount_curve_id.as_str())?;
                let future_divs = future_dividends(inst, disc_curve.as_ref(), as_of)?;
                if future_divs.is_empty() {
                    greeks_unit.rho_r
                } else {
                    // `future_divs` already contains each dividend's PV.
                    let ds_star_dr = escrowed_spot_drho(0.0, &future_divs);
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
            let steps = inst
                .instrument_pricing_overrides
                .model_config
                .tree_steps
                .unwrap_or(201);
            let tree = BinomialTree::leisen_reimer(steps);
            let (params, dividends) = early_exercise_market_params(inst, curves, as_of)?;
            let price_fn = |market_params: &OptionMarketParams| -> Result<f64> {
                if dividends.is_empty() {
                    tree.price_american(market_params)
                } else {
                    tree.price_american_with_discrete_dividends(market_params, &dividends)
                }
            };
            tree_finite_difference_greeks(
                &params,
                inst.notional.amount(),
                inst.theta_day_basis.days_per_year(),
                price_fn,
            )
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
            let (params, dividends) = early_exercise_market_params(inst, curves, as_of)?;
            let exercise_times: Vec<f64> = schedule
                .iter()
                .filter_map(|date| {
                    let year_fraction = DayCount::Act365F
                        .year_fraction(as_of, *date, Default::default())
                        .ok()?;
                    (year_fraction > 0.0 && year_fraction <= params.time_to_expiry)
                        .then_some(year_fraction)
                })
                .collect();
            if exercise_times.is_empty() {
                return Err(finstack_quant_core::Error::Validation(
                    "Bermudan equity option has no exercise dates remaining after valuation date"
                        .to_string(),
                ));
            }
            let price_fn = |market_params: &OptionMarketParams| -> Result<f64> {
                if dividends.is_empty() {
                    tree.price_bermudan(market_params, &exercise_times)
                } else {
                    tree.price_bermudan_with_discrete_dividends(
                        market_params,
                        &exercise_times,
                        &dividends,
                    )
                }
            };
            tree_finite_difference_greeks(
                &params,
                inst.notional.amount(),
                inst.theta_day_basis.days_per_year(),
                price_fn,
            )
        }
    }
}

fn tree_finite_difference_greeks(
    params: &OptionMarketParams,
    scale: f64,
    theta_days_per_year: f64,
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

    // Theta: one day on the instrument's configured reporting basis.
    let dt = 1.0 / theta_days_per_year;
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

/// Registry pricer for Equity Option using Black-Scholes model
pub(crate) struct SimpleEquityOptionBlackPricer;

impl SimpleEquityOptionBlackPricer {
    /// Create new Black-Scholes pricer with default model.
    ///
    /// Uses `ModelKey::Black76` which is the library-wide convention for
    /// lognormal option pricing. BSM and Black-76 are mathematically
    /// equivalent (BSM is Black-76 applied to the forward
    /// `F = S × exp((r-q)T)`), so the same model key covers both.
    pub(crate) fn new() -> Self {
        Self
    }
}

impl Default for SimpleEquityOptionBlackPricer {
    fn default() -> Self {
        Self::new()
    }
}

impl crate::pricer::Pricer for SimpleEquityOptionBlackPricer {
    fn key(&self) -> crate::pricer::PricerKey {
        crate::pricer::PricerKey::new(
            crate::pricer::InstrumentType::EquityOption,
            crate::pricer::ModelKey::Black76,
        )
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
        let equity_option = expect_inst::<crate::instruments::equity::equity_option::EquityOption>(
            instrument,
            crate::pricer::InstrumentType::EquityOption,
        )?;

        // Use the provided as_of date for consistency
        let pv = compute_pv(equity_option, market, as_of).map_err(|e| {
            crate::pricer::PricingError::model_failure_with_context(
                e.to_string(),
                crate::pricer::PricingErrorContext::from_instrument(equity_option)
                    .model(self.key().model),
            )
        })?;

        Ok(crate::results::ValuationResult::stamped(
            equity_option.id(),
            as_of,
            pv,
        ))
    }
}
