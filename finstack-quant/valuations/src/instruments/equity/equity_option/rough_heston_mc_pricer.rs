//! Monte Carlo pricer for the rough Heston stochastic volatility model.
//!
//! Uses [`RoughHestonProcess`] with the [`RoughHestonHybrid`] Volterra discretization
//! to simulate spot and variance paths. The rough Heston discretization handles the
//! fractional kernel internally using standard normal increments — no fBM generator
//! is required.

use super::pricer::{collect_inputs_extended, has_future_discrete_dividends, option_currency};
use super::types::EquityOption;
use crate::instruments::common_impl::parameters::OptionType;
use crate::instruments::common_impl::traits::Instrument;
use crate::pricer::PricingError;
use finstack_quant_core::dates::Date;
use finstack_quant_core::market_data::context::MarketContext;
use finstack_quant_core::money::Money;

/// Monte Carlo pricer for the rough Heston model.
///
/// Simulates spot and variance jointly under the rough Heston dynamics using the
/// hybrid Euler-Maruyama scheme with Volterra integral evaluation at every step.
/// The discretization is O(n^2) per path due to the non-Markovian variance kernel.
///
/// Rough Heston parameters are read from market scalars with the `ROUGH_HESTON_*`
/// prefix — see [`super::rough_heston_fourier_pricer`] for the full key listing.
pub(crate) struct EquityOptionRoughHestonMcPricer {
    /// Number of Monte Carlo paths.
    num_paths: usize,
    /// Number of time steps per path.
    num_steps: usize,
}

impl EquityOptionRoughHestonMcPricer {
    /// Create a new rough Heston MC pricer with explicit configuration.
    pub(crate) fn new(num_paths: usize, num_steps: usize) -> Self {
        Self {
            num_paths,
            num_steps,
        }
    }
}

impl Default for EquityOptionRoughHestonMcPricer {
    fn default() -> Self {
        use crate::instruments::common_impl::helpers::mc_defaults;
        Self::new(
            mc_defaults::DEFAULT_MC_PATHS,
            mc_defaults::DEFAULT_ROUGH_VOL_STEPS,
        )
    }
}

impl crate::pricer::Pricer for EquityOptionRoughHestonMcPricer {
    fn key(&self) -> crate::pricer::PricerKey {
        crate::pricer::PricerKey::new(
            crate::pricer::InstrumentType::EquityOption,
            crate::pricer::ModelKey::MonteCarloRoughHeston,
        )
    }

    #[tracing::instrument(
        name = "equity_option.rough_heston_mc.price_dyn",
        level = "debug",
        skip(self, instrument, market),
        fields(
            inst_id = %instrument.id(),
            as_of = %as_of,
            num_paths = self.num_paths,
            num_steps = self.num_steps,
        ),
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

        if as_of > equity_option.expiry {
            return Ok(crate::results::ValuationResult::stamped(
                equity_option.id(),
                as_of,
                Money::new(0.0, option_currency(equity_option)),
            ));
        }

        // W-31: `collect_inputs_extended` applies the escrowed-dividend model
        // (spot shift + `q = 0`) when `discrete_dividends` is non-empty. The
        // escrowed-dividend identity holds only under Black-Scholes; under the
        // rough Heston stochastic-vol dynamics it is invalid, so feeding the
        // escrowed spot into the MC silently mis-prices a single-stock option
        // with discrete dividends. Reject it explicitly rather than price it
        // wrong.
        if has_future_discrete_dividends(equity_option, as_of) {
            return Err(crate::pricer::PricingError::model_failure_with_context(
                "rough Heston Monte Carlo pricing does not support discrete \
                 dividends: the escrowed-dividend spot adjustment is a \
                 Black-Scholes-only construct and is invalid under stochastic \
                 volatility. Use the Black-Scholes pricer for discrete \
                 dividends, or supply a continuous dividend yield instead."
                    .to_string(),
                crate::pricer::PricingErrorContext::from_instrument(equity_option)
                    .model(crate::pricer::ModelKey::MonteCarloRoughHeston),
            ));
        }

        let inputs = collect_inputs_extended(equity_option, market, as_of).map_err(|e| {
            crate::pricer::PricingError::model_failure_with_context(
                e.to_string(),
                crate::pricer::PricingErrorContext::from_instrument(equity_option)
                    .model(crate::pricer::ModelKey::MonteCarloRoughHeston),
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

        let err_ctx = crate::pricer::PricingErrorContext::from_instrument(equity_option)
            .model(crate::pricer::ModelKey::MonteCarloRoughHeston);

        // Source production rough-Heston parameters from explicit market scalars.
        let s = crate::instruments::equity::equity_option::rough_heston_market::RoughHestonScalars::from_market_strict(market)
            .map_err(|e| crate::pricer::PricingError::from_core(e, err_ctx.clone()))?;

        // Build process
        let hurst_exp = finstack_quant_core::math::fractional::HurstExponent::new(s.hurst)
            .map_err(|e| crate::pricer::PricingError::from_core(e, err_ctx.clone()))?;
        let params = finstack_quant_monte_carlo::process::rough_heston::RoughHestonParams::new(
            r, q, hurst_exp, s.kappa, s.theta, s.sigma_v, s.rho, s.v0,
        )
        .map_err(|e| crate::pricer::PricingError::from_core(e, err_ctx.clone()))?;
        let process =
            finstack_quant_monte_carlo::process::rough_heston::RoughHestonProcess::new(params);

        // Build time grid and discretization
        let time_grid = finstack_quant_monte_carlo::time_grid::TimeGrid::uniform(t, self.num_steps)
            .map_err(|e| crate::pricer::PricingError::from_core(e, err_ctx.clone()))?;
        let times: Vec<f64> = (0..=self.num_steps)
            .map(|i| t * i as f64 / self.num_steps as f64)
            .collect();
        let disc =
            finstack_quant_monte_carlo::discretization::rough_heston::RoughHestonHybrid::new(
                &times, s.hurst,
            )
            .map_err(|e| crate::pricer::PricingError::from_core(e, err_ctx.clone()))?;

        // Derive deterministic seed from instrument id
        let seed_val =
            if let Some(ref scenario) = equity_option.pricing_overrides.metrics.mc_seed_scenario {
                finstack_quant_monte_carlo::seed::derive_seed(&equity_option.id, scenario)
            } else {
                finstack_quant_monte_carlo::seed::derive_seed(&equity_option.id, "base")
            };

        // Resolve and cap the path count via the workspace helper before
        // allocating, so a malicious or typo'd `mc_paths` override can't OOM
        // the host.
        let num_paths = crate::instruments::common_impl::helpers::resolve_mc_paths(
            equity_option.pricing_overrides.model_config.mc_paths,
            self.num_paths,
        )
        .map_err(|e| crate::pricer::PricingError::from_core(e, err_ctx.clone()))?;

        // Build engine and payoff
        let engine = finstack_quant_monte_carlo::engine::McEngine::builder()
            .num_paths(num_paths)
            .time_grid(time_grid)
            .parallel(false)
            .build()
            .map_err(|e| crate::pricer::PricingError::from_core(e, err_ctx.clone()))?;

        let ccy = option_currency(equity_option);
        let discount_factor = (-r * t).exp();
        let initial_state = [spot, s.v0];
        let rng = finstack_quant_monte_carlo::rng::philox::PhiloxRng::new(seed_val);

        let result = match equity_option.option_type {
            OptionType::Call => {
                let payoff = finstack_quant_monte_carlo::payoff::vanilla::EuropeanCall::new(
                    equity_option.strike,
                    equity_option.notional.amount(),
                    self.num_steps,
                );
                engine
                    .price(
                        &rng,
                        &process,
                        &disc,
                        &initial_state,
                        &payoff,
                        ccy,
                        discount_factor,
                    )
                    .map_err(|e| crate::pricer::PricingError::from_core(e, err_ctx.clone()))?
            }
            OptionType::Put => {
                let payoff = finstack_quant_monte_carlo::payoff::vanilla::EuropeanPut::new(
                    equity_option.strike,
                    equity_option.notional.amount(),
                    self.num_steps,
                );
                engine
                    .price(
                        &rng,
                        &process,
                        &disc,
                        &initial_state,
                        &payoff,
                        ccy,
                        discount_factor,
                    )
                    .map_err(|e| crate::pricer::PricingError::from_core(e, err_ctx))?
            }
        };

        let pv = result.mean;
        let mut vr = crate::results::ValuationResult::stamped(equity_option.id(), as_of, pv);
        if result.stderr > 0.0 {
            vr.measures
                .insert(crate::metrics::MetricId::custom("mc_stderr"), result.stderr);
        }
        Ok(vr)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::instruments::common_impl::parameters::ExerciseStyle;
    use crate::instruments::{Attributes, PricingOverrides, SettlementType};
    use crate::pricer::Pricer;
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
            .id(InstrumentId::new("EQ-OPT-ROUGH-HESTON-TEST"))
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
            .pricing_overrides(PricingOverrides::default())
            .attributes(Attributes::new())
            .build()
            .expect("equity option")
    }

    /// W-31: a single-stock rough Heston MC option with a future discrete
    /// dividend must be rejected — the escrowed-dividend model is BS-only.
    #[test]
    fn rejects_future_discrete_dividend() {
        let as_of = date(2025, 1, 1);
        let mut inst = option(date(2025, 12, 31));
        inst.discrete_dividends = vec![(date(2025, 6, 15), 2.0)];

        let err = EquityOptionRoughHestonMcPricer::default()
            .price_dyn(&inst, &market(as_of), as_of)
            .expect_err("discrete dividend must be rejected");
        assert!(
            err.to_string().contains("discrete dividends"),
            "unexpected error message: {err}"
        );
    }

    /// W-31: an out-of-window (past) discrete dividend must NOT trigger the
    /// rejection.
    #[test]
    fn past_discrete_dividend_does_not_trigger_rejection() {
        let as_of = date(2025, 1, 1);
        let mut inst = option(date(2025, 12, 31));
        inst.discrete_dividends = vec![(date(2024, 6, 15), 2.0)];

        if let Err(e) =
            EquityOptionRoughHestonMcPricer::default().price_dyn(&inst, &market(as_of), as_of)
        {
            assert!(
                !e.to_string().contains("discrete dividends"),
                "out-of-window dividend wrongly rejected: {e}"
            );
        }
    }
}
