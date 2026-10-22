//! Equity option Heston Monte Carlo pricer.
//!
//! Prices European equity options under the Heston stochastic volatility model
//! using Monte Carlo simulation with the QE (Quadratic Exponential) discretization
//! scheme.

use crate::equity_option::pricing::inputs::EquityOptionInputs;
use crate::equity_option::pricing::{model_error, stochastic_vol_inputs, ModelInputs};
use crate::equity_option::types::EquityOption;
use finstack_quant_core::dates::Date;
use finstack_quant_core::market_data::context::MarketContext;
use finstack_quant_core::math::time_grid::TimeGrid;
use finstack_quant_core::money::Money;
use finstack_quant_models::monte_carlo::discretization::qe_heston::QeHeston;
use finstack_quant_models::monte_carlo::engine::{McEngine, McEngineConfig};
use finstack_quant_models::monte_carlo::payoff::vanilla::European;
use finstack_quant_models::monte_carlo::process::heston::HestonProcess;
use finstack_quant_models::monte_carlo::rng::philox::PhiloxRng;
use finstack_quant_valuations_core::instruments::Instrument;
use finstack_quant_valuations_core::pricer::{
    expect_inst, InstrumentType, ModelKey, Pricer, PricerKey, PricingError,
};
use finstack_quant_valuations_core::results::ValuationResult;

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
        use finstack_quant_valuations_core::instruments::common_impl::helpers::mc_defaults;
        Self {
            num_paths: mc_defaults::DEFAULT_MC_PATHS,
            steps_per_year: mc_defaults::DEFAULT_STEPS_PER_YEAR,
        }
    }

    /// Price an equity option using Heston Monte Carlo; returns the PV and
    /// its Monte Carlo standard error (zero for a settled value).
    fn price_internal(
        &self,
        inst: &EquityOption,
        market: &MarketContext,
        as_of: Date,
    ) -> std::result::Result<(Money, f64), PricingError> {
        let model = ModelKey::MonteCarloHeston;
        match stochastic_vol_inputs(inst, market, as_of, model, "Heston Monte Carlo")? {
            ModelInputs::Settled(pv) => Ok((pv, 0.0)),
            ModelInputs::Live(inputs) => self
                .simulate(inst, market, inputs)
                .map_err(|e| model_error(inst, model, e)),
        }
    }

    /// Simulate the live option under Heston QE dynamics.
    fn simulate(
        &self,
        inst: &EquityOption,
        market: &MarketContext,
        inputs: EquityOptionInputs,
    ) -> finstack_quant_core::Result<(Money, f64)> {
        let (spot, r, q, t) = (inputs.spot, inputs.r, inputs.q, inputs.t_vol);
        let ccy = inst.currency;

        // Heston parameters: **Audit P3b** — use the strict resolver so a
        // Missing or mistyped HESTON_* scalars fail loudly rather than selecting
        // representative SPX defaults. The canonical `HestonPricingParams::new`
        // enforces positive κ/θ/σᵥ/v₀ and ρ ∈ (−1, 1) for both Monte Carlo and
        // Fourier paths.
        let params = finstack_quant_valuations_core::instruments::pricing::heston_market::heston_params_from_market_strict(
            market, r, q,
        )?;
        let process = HestonProcess::new(params);
        let discretization = QeHeston::new();

        let num_steps = ((t * self.steps_per_year).round() as usize).max(10);
        let time_grid = TimeGrid::uniform(t, num_steps)?;
        let maturity_step = time_grid.num_steps();

        let num_paths =
            finstack_quant_valuations_core::instruments::common_impl::helpers::resolve_mc_paths(
                inst.instrument_pricing_overrides.model_config.mc_paths,
                self.num_paths,
            )?;

        // Derive deterministic seed
        let seed_val = inst
            .instrument_pricing_overrides
            .model_config
            .mc_seed(&inst.id);

        let engine = McEngine::new(McEngineConfig::new(num_paths, time_grid));

        let rng = PhiloxRng::new(seed_val);
        let discount_factor = (-r * t).exp();

        // Initial state: [spot, v0]
        let initial_state = [spot, params.v0];

        let payoff = European::new(inst.option_type, inst.strike, inst.quantity, maturity_step);
        let result = engine.price(
            &rng,
            &process,
            &discretization,
            &initial_state,
            &payoff,
            ccy,
            discount_factor,
        )?;

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
        let equity_option = expect_inst::<EquityOption>(instrument, InstrumentType::EquityOption)?;
        let (pv, stderr) = self.price_internal(equity_option, market, as_of)?;

        let mut result = ValuationResult::stamped(equity_option.id(), as_of, pv);
        if stderr > 0.0 {
            result.measures.insert(
                finstack_quant_valuations_core::metrics::MetricId::custom("mc_stderr"),
                stderr,
            );
        }
        Ok(result)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use finstack_quant_core::currency::Currency;
    use finstack_quant_core::dates::DayCount;
    use finstack_quant_core::market_data::scalars::MarketScalar;
    use finstack_quant_core::market_data::surfaces::VolSurface;
    use finstack_quant_core::market_data::term_structures::DiscountCurve;
    use finstack_quant_core::types::Attributes;
    use finstack_quant_core::types::{CurveId, InstrumentId};
    use finstack_quant_models::types::{ExerciseStyle, OptionType};
    use finstack_quant_valuations_core::instruments::SettlementType;
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
            .quantity(100.0)
            .currency(Currency::USD)
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
