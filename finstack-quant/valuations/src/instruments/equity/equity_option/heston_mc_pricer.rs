//! Equity option Heston Monte Carlo pricer.
//!
//! Prices European equity options under the Heston stochastic volatility model
//! using Monte Carlo simulation with the QE (Quadratic Exponential) discretization
//! scheme.

use crate::instruments::common_impl::traits::Instrument;
use crate::instruments::equity::equity_option::pricer::{
    collect_inputs_extended, require_european,
};
use crate::instruments::equity::equity_option::types::EquityOption;
use crate::models::closed_form::heston::HestonParams as ClosedFormHestonParams;
use crate::pricer::{
    InstrumentType, ModelKey, Pricer, PricerKey, PricingError, PricingErrorContext,
};
use crate::results::ValuationResult;
use finstack_quant_core::dates::Date;
use finstack_quant_core::market_data::context::MarketContext;
use finstack_quant_core::money::Money;

use finstack_quant_monte_carlo::discretization::qe_heston::QeHeston;
use finstack_quant_monte_carlo::engine::McEngine;
use finstack_quant_monte_carlo::payoff::vanilla::{EuropeanCall, EuropeanPut};
use finstack_quant_monte_carlo::process::heston::{HestonParams, HestonProcess};
use finstack_quant_monte_carlo::rng::philox::PhiloxRng;
use finstack_quant_monte_carlo::seed;
use finstack_quant_monte_carlo::time_grid::TimeGrid;

/// Equity option Heston Monte Carlo pricer.
///
/// Prices European equity options under the Heston stochastic volatility model
/// using QE discretization. Heston parameters are sourced from required market
/// scalars (`HESTON_KAPPA`, `HESTON_THETA`, etc.).
pub(crate) struct EquityOptionHestonMcPricer {
    num_paths: usize,
    steps_per_year: f64,
}

impl EquityOptionHestonMcPricer {
    /// Create a new Heston MC pricer with default configuration.
    pub(crate) fn new() -> Self {
        use crate::instruments::common_impl::helpers::mc_defaults;
        Self {
            num_paths: mc_defaults::DEFAULT_MC_PATHS,
            steps_per_year: mc_defaults::DEFAULT_STEPS_PER_YEAR,
        }
    }

    /// Price an equity option using Heston Monte Carlo.
    fn price_internal(
        &self,
        inst: &EquityOption,
        market: &MarketContext,
        as_of: Date,
    ) -> finstack_quant_core::Result<(Money, f64)> {
        if as_of > inst.expiry {
            return Ok((Money::new(0.0, inst.notional.currency()), 0.0));
        }
        // The escrowed-dividend identity used by `collect_inputs_extended`
        // is Black-Scholes-specific; reject it for Heston stochastic vol.
        if inst
            .discrete_dividends
            .iter()
            .any(|(ex_date, _)| *ex_date > as_of && *ex_date <= inst.expiry)
        {
            return Err(finstack_quant_core::Error::Validation(
                "Heston Monte Carlo pricing does not support discrete dividends: \
                 the escrowed-dividend spot adjustment is a Black-Scholes-only \
                 construct and is invalid under stochastic volatility. Use the \
                 Black-Scholes pricer for discrete dividends, or supply a \
                 continuous dividend yield instead."
                    .to_string(),
            ));
        }

        let inputs = collect_inputs_extended(inst, market, as_of)?;
        let (spot, r, q, _sigma, t) = (inputs.spot, inputs.r, inputs.q, inputs.sigma, inputs.t_vol);
        let ccy = inst.notional.currency();

        if t <= 0.0 {
            let intrinsic = match inst.option_type {
                crate::instruments::common_impl::parameters::OptionType::Call => {
                    (spot - inst.strike).max(0.0)
                }
                crate::instruments::common_impl::parameters::OptionType::Put => {
                    (inst.strike - spot).max(0.0)
                }
            };
            return Ok((Money::new(intrinsic * inst.notional.amount(), ccy), 0.0));
        }

        // Heston parameters: **Audit P3b** — use the strict resolver so a
        // missing or mistyped HESTON_* scalar fails loudly here rather than
        // silently selecting the representative SPX defaults. Validation
        // (positive κ/θ/σᵥ/v₀, ρ ∈ (−1, 1)) is still enforced inside
        // `HestonParams::new`. We then convert to the MC engine's own
        // `HestonParams` struct.
        let cf_params = ClosedFormHestonParams::from_market_strict(market, r, q)?;
        let heston_params = HestonParams::new(
            cf_params.r,
            cf_params.q,
            cf_params.kappa,
            cf_params.theta,
            cf_params.sigma_v,
            cf_params.rho,
            cf_params.v0,
        )?;
        let process = HestonProcess::new(heston_params);
        let discretization = QeHeston::new();

        // Build time grid and engine
        let num_steps = ((t * self.steps_per_year).round() as usize).max(10);
        let time_grid = TimeGrid::uniform(t, num_steps)?;
        let maturity_step = time_grid.num_steps();

        let num_paths = crate::instruments::common_impl::helpers::resolve_mc_paths(
            inst.instrument_pricing_overrides.model_config.mc_paths,
            self.num_paths,
        )?;

        // Derive deterministic seed
        let seed_val = if let Some(ref scenario) = inst.metric_pricing_overrides.mc_seed_scenario {
            seed::derive_seed(&inst.id, scenario)
        } else {
            seed::derive_seed(&inst.id, "base")
        };

        let engine = McEngine::builder()
            .num_paths(num_paths)
            .time_grid(time_grid)
            .build()?;

        let rng = PhiloxRng::new(seed_val);
        let discount_factor = (-r * t).exp();

        // Initial state: [spot, v0]
        let initial_state = [spot, cf_params.v0];

        let result = match inst.option_type {
            crate::instruments::common_impl::parameters::OptionType::Call => {
                let payoff = EuropeanCall::new(inst.strike, inst.notional.amount(), maturity_step);
                engine.price(
                    &rng,
                    &process,
                    &discretization,
                    &initial_state,
                    &payoff,
                    ccy,
                    discount_factor,
                )?
            }
            crate::instruments::common_impl::parameters::OptionType::Put => {
                let payoff = EuropeanPut::new(inst.strike, inst.notional.amount(), maturity_step);
                engine.price(
                    &rng,
                    &process,
                    &discretization,
                    &initial_state,
                    &payoff,
                    ccy,
                    discount_factor,
                )?
            }
        };

        Ok((result.mean, result.stderr))
    }
}

impl Default for EquityOptionHestonMcPricer {
    fn default() -> Self {
        Self::new()
    }
}

impl Pricer for EquityOptionHestonMcPricer {
    fn key(&self) -> PricerKey {
        PricerKey::new(InstrumentType::EquityOption, ModelKey::MonteCarloHeston)
    }

    #[tracing::instrument(
        name = "equity_option.heston_mc.price_dyn",
        level = "debug",
        skip(self, instrument, market),
        fields(
            inst_id = %instrument.id(),
            as_of = %as_of,
            num_paths = self.num_paths,
        ),
        err,
    )]
    fn price_dyn(
        &self,
        instrument: &dyn Instrument,
        market: &MarketContext,
        as_of: Date,
    ) -> std::result::Result<ValuationResult, PricingError> {
        let equity_option = instrument
            .as_any()
            .downcast_ref::<EquityOption>()
            .ok_or_else(|| {
                PricingError::type_mismatch(InstrumentType::EquityOption, instrument.key())
            })?;
        require_european(equity_option, "Heston Monte Carlo").map_err(|e| {
            PricingError::model_failure_with_context(
                e.to_string(),
                PricingErrorContext::from_instrument(equity_option)
                    .model(ModelKey::MonteCarloHeston),
            )
        })?;

        let (pv, stderr) = self
            .price_internal(equity_option, market, as_of)
            .map_err(|e| {
                PricingError::model_failure_with_context(
                    e.to_string(),
                    PricingErrorContext::from_instrument(equity_option)
                        .model(ModelKey::MonteCarloHeston),
                )
            })?;

        let mut result = ValuationResult::stamped(equity_option.id(), as_of, pv);
        if stderr > 0.0 {
            result
                .measures
                .insert(crate::metrics::MetricId::custom("mc_stderr"), stderr);
        }
        Ok(result)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::instruments::common_impl::parameters::{ExerciseStyle, OptionType};
    use crate::instruments::{Attributes, SettlementType};
    use finstack_quant_core::currency::Currency;
    use finstack_quant_core::dates::DayCount;
    use finstack_quant_core::market_data::scalars::MarketScalar;
    use finstack_quant_core::market_data::surfaces::VolSurface;
    use finstack_quant_core::market_data::term_structures::DiscountCurve;
    use finstack_quant_core::types::{CurveId, InstrumentId};
    use time::Month;

    fn date(year: i32, month: u8, day: u8) -> Date {
        Date::from_calendar_date(year, Month::try_from(month).expect("month"), day)
            .expect("valid date")
    }

    fn market(as_of: Date) -> MarketContext {
        let curve = DiscountCurve::builder("USD-OIS")
            .base_date(as_of)
            .day_count(DayCount::Act365F)
            .knots([(0.0, 1.0), (10.0, (-0.03_f64 * 10.0).exp())])
            .build()
            .expect("curve");
        let surface = VolSurface::builder("SPX-VOL")
            .expiries(&[0.25, 1.0, 2.0])
            .strikes(&[80.0, 100.0, 120.0])
            .row(&[0.2, 0.2, 0.2])
            .row(&[0.2, 0.2, 0.2])
            .row(&[0.2, 0.2, 0.2])
            .build()
            .expect("surface");
        MarketContext::new()
            .insert(curve)
            .insert_surface(surface)
            .insert_price("SPX-SPOT", MarketScalar::Unitless(100.0))
    }

    fn option(expiry: Date) -> EquityOption {
        EquityOption::builder()
            .id(InstrumentId::new("EQ-OPT-HESTON-TEST"))
            .underlying_ticker("SPX".to_string())
            .strike(100.0)
            .option_type(OptionType::Call)
            .exercise_style(ExerciseStyle::European)
            .expiry(expiry)
            .notional(Money::new(100.0, Currency::USD))
            .day_count(DayCount::Act365F)
            .settlement(SettlementType::Cash)
            .discount_curve_id(CurveId::new("USD-OIS"))
            .spot_id("SPX-SPOT".into())
            .vol_surface_id(CurveId::new("SPX-VOL"))
            .attributes(Attributes::new())
            .build()
            .expect("equity option")
    }

    /// W-31: a single-stock Heston MC option with a future discrete dividend
    /// must be rejected — the escrowed-dividend model is Black-Scholes-only.
    #[test]
    fn rejects_future_discrete_dividend() {
        let as_of = date(2025, 1, 1);
        let mut inst = option(date(2025, 12, 31));
        inst.discrete_dividends = vec![(date(2025, 6, 15), 2.0)];

        let err = EquityOptionHestonMcPricer::new()
            .price_internal(&inst, &market(as_of), as_of)
            .expect_err("discrete dividend must be rejected");
        let msg = err.to_string();
        assert!(
            msg.contains("discrete dividends"),
            "unexpected error message: {msg}"
        );
    }

    /// W-31: a past or post-expiry discrete dividend is not in-window and must
    /// NOT trigger the rejection (it is harmless — the escrowed model ignores
    /// it too).
    #[test]
    fn past_discrete_dividend_does_not_trigger_rejection() {
        let as_of = date(2025, 1, 1);
        let mut inst = option(date(2025, 12, 31));
        inst.discrete_dividends = vec![(date(2024, 6, 15), 2.0)];

        // Pricing may still fail downstream (e.g. missing Heston scalars), but
        // the failure must NOT be the discrete-dividend rejection.
        if let Err(e) =
            EquityOptionHestonMcPricer::new().price_internal(&inst, &market(as_of), as_of)
        {
            assert!(
                !e.to_string().contains("discrete dividends"),
                "out-of-window dividend wrongly rejected: {e}"
            );
        }
    }
}
