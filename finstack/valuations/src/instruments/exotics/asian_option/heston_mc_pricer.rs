//! Asian option Heston Monte Carlo pricer.
//!
//! Prices Asian options under the Heston stochastic volatility model
//! using Monte Carlo simulation with QE discretization. Averaging is
//! performed on the spot component (state[0]) of the Heston path.

use crate::instruments::common_impl::traits::Instrument;
use crate::instruments::exotics::asian_option::types::AsianOption;
use crate::pricer::{
    InstrumentType, ModelKey, Pricer, PricerKey, PricingError, PricingErrorContext, PricingResult,
};
use crate::results::ValuationResult;
use finstack_core::dates::{Date, DayCountContext};
use finstack_core::market_data::context::MarketContext;
use finstack_core::money::Money;

use finstack_monte_carlo::discretization::qe_heston::QeHeston;
use finstack_monte_carlo::engine::McEngine;
use finstack_monte_carlo::payoff::asian::{AsianCall, AsianPut, AveragingMethod};
use finstack_monte_carlo::process::heston::{HestonParams, HestonProcess};
use finstack_monte_carlo::rng::philox::PhiloxRng;
use finstack_monte_carlo::seed;
use finstack_monte_carlo::time_grid::TimeGrid;

/// Asian option Heston Monte Carlo pricer.
///
/// Prices Asian options under the Heston stochastic volatility model using
/// QE discretization. Averaging is performed on the spot component of the
/// Heston two-factor path. Heston parameters are sourced from market scalars
/// with sensible defaults.
pub(crate) struct AsianOptionHestonMcPricer {
    num_paths: usize,
    steps_per_year: f64,
    min_steps: usize,
}

impl AsianOptionHestonMcPricer {
    /// Create a new Asian option Heston MC pricer with default configuration.
    pub(crate) fn new() -> Self {
        Self {
            num_paths: 100_000,
            steps_per_year: 252.0,
            min_steps: 10,
        }
    }

    /// Extract a Heston parameter from market scalars with a fallback default.
    fn heston_scalar(market: &MarketContext, key: &str, default: f64) -> f64 {
        crate::instruments::common_impl::helpers::get_unitless_scalar(market, key, default)
    }

    /// Price an Asian option using Heston Monte Carlo.
    fn price_internal(
        &self,
        inst: &AsianOption,
        market: &MarketContext,
        as_of: Date,
    ) -> finstack_core::Result<(Money, f64)> {
        // Time to maturity
        let t = inst
            .day_count
            .year_fraction(as_of, inst.expiry, DayCountContext::default())?;

        let (hist_sum, hist_prod_log, hist_count) = inst.accumulated_state(as_of);

        if t <= 0.0 {
            // Expired: use realized average
            let average = if hist_count > 0 {
                match inst.averaging_method {
                    crate::instruments::exotics::asian_option::types::AveragingMethod::Arithmetic => {
                        hist_sum / hist_count as f64
                    }
                    crate::instruments::exotics::asian_option::types::AveragingMethod::Geometric => {
                        (hist_prod_log / hist_count as f64).exp()
                    }
                }
            } else {
                let spot_scalar = market.get_price(&inst.spot_id)?;
                match spot_scalar {
                    finstack_core::market_data::scalars::MarketScalar::Unitless(v) => *v,
                    finstack_core::market_data::scalars::MarketScalar::Price(m) => m.amount(),
                }
            };

            let intrinsic = match inst.option_type {
                crate::instruments::OptionType::Call => (average - inst.strike).max(0.0),
                crate::instruments::OptionType::Put => (inst.strike - average).max(0.0),
            };
            return Ok((
                Money::new(intrinsic * inst.notional.amount(), inst.notional.currency()),
                0.0,
            ));
        }

        // Get discount curve and factor
        let disc_curve = market.get_discount(inst.discount_curve_id.as_str())?;
        let discount_factor = disc_curve.df_between_dates(as_of, inst.expiry)?;
        let r = if t > 0.0 && discount_factor > 0.0 {
            -discount_factor.ln() / t
        } else {
            0.0
        };

        // Get spot
        let spot_scalar = market.get_price(&inst.spot_id)?;
        let spot = match spot_scalar {
            finstack_core::market_data::scalars::MarketScalar::Unitless(v) => *v,
            finstack_core::market_data::scalars::MarketScalar::Price(m) => m.amount(),
        };

        // Get dividend yield
        let q = crate::instruments::common_impl::helpers::resolve_optional_dividend_yield(
            market,
            inst.div_yield_id.as_ref(),
        )?;

        // Fetch Heston parameters
        let kappa = Self::heston_scalar(market, "HESTON_KAPPA", 2.0);
        let theta = Self::heston_scalar(market, "HESTON_THETA", 0.04);
        let sigma_v = Self::heston_scalar(market, "HESTON_SIGMA_V", 0.3);
        let rho = Self::heston_scalar(market, "HESTON_RHO", -0.7);
        let v0 = Self::heston_scalar(market, "HESTON_V0", 0.04);

        let heston_params = HestonParams::new(r, q, kappa, theta, sigma_v, rho, v0)?;
        let process = HestonProcess::new(heston_params);
        let discretization = QeHeston::new();

        // Build time grid. Map fixing dates to time steps via the shared
        // helper so each distinct fixing gets its own grid step (W-04) — the
        // naive round()+dedup() merged distinct fixings on a coarse grid.
        let base_num_steps = ((t * self.steps_per_year).round() as usize).max(self.min_steps);
        let fixing_grid = crate::instruments::exotics::asian_option::pricer::map_fixings_to_distinct_steps(
            &inst.fixing_dates,
            inst.day_count,
            as_of,
            t,
            base_num_steps,
        )?;
        let num_steps = fixing_grid.num_steps;
        let fixing_steps = fixing_grid.fixing_steps;

        // Map averaging method
        let averaging = match inst.averaging_method {
            crate::instruments::exotics::asian_option::types::AveragingMethod::Arithmetic => {
                AveragingMethod::Arithmetic
            }
            crate::instruments::exotics::asian_option::types::AveragingMethod::Geometric => {
                AveragingMethod::Geometric
            }
        };

        let time_grid = TimeGrid::uniform(t, num_steps)?;

        let num_paths = inst
            .pricing_overrides
            .model_config
            .mc_paths
            .filter(|&n| n > 0)
            .unwrap_or(self.num_paths);

        // Derive deterministic seed
        let seed_val = if let Some(ref scenario) = inst.pricing_overrides.metrics.mc_seed_scenario {
            seed::derive_seed(&inst.id, scenario)
        } else {
            seed::derive_seed(&inst.id, "base")
        };

        let engine = McEngine::builder()
            .num_paths(num_paths)
            .time_grid(time_grid)
            .build()?;

        let rng = PhiloxRng::new(seed_val);

        // Initial state: [spot, v0]
        let initial_state = [spot, v0];

        let ccy = inst.notional.currency();

        let (pv, mc_stderr) = match inst.option_type {
            crate::instruments::OptionType::Call => {
                let payoff = if hist_count > 0 {
                    AsianCall::with_history(
                        inst.strike,
                        inst.notional.amount(),
                        averaging,
                        fixing_steps,
                        hist_sum,
                        hist_prod_log,
                        hist_count,
                    )
                } else {
                    AsianCall::new(inst.strike, inst.notional.amount(), averaging, fixing_steps)
                };
                let result = engine.price(
                    &rng,
                    &process,
                    &discretization,
                    &initial_state,
                    &payoff,
                    ccy,
                    discount_factor,
                )?;
                (result.mean, result.stderr)
            }
            crate::instruments::OptionType::Put => {
                let payoff = if hist_count > 0 {
                    AsianPut::with_history(
                        inst.strike,
                        inst.notional.amount(),
                        averaging,
                        fixing_steps,
                        hist_sum,
                        hist_prod_log,
                        hist_count,
                    )
                } else {
                    AsianPut::new(inst.strike, inst.notional.amount(), averaging, fixing_steps)
                };
                let result = engine.price(
                    &rng,
                    &process,
                    &discretization,
                    &initial_state,
                    &payoff,
                    ccy,
                    discount_factor,
                )?;
                (result.mean, result.stderr)
            }
        };

        Ok((pv, mc_stderr))
    }
}

impl Default for AsianOptionHestonMcPricer {
    fn default() -> Self {
        Self::new()
    }
}

impl Pricer for AsianOptionHestonMcPricer {
    fn key(&self) -> PricerKey {
        PricerKey::new(InstrumentType::AsianOption, ModelKey::MonteCarloHeston)
    }

    fn price_dyn(
        &self,
        instrument: &dyn Instrument,
        market: &MarketContext,
        as_of: Date,
    ) -> PricingResult<ValuationResult> {
        let asian = instrument
            .as_any()
            .downcast_ref::<AsianOption>()
            .ok_or_else(|| {
                PricingError::type_mismatch(InstrumentType::AsianOption, instrument.key())
            })?;

        let (pv, stderr) = self.price_internal(asian, market, as_of).map_err(|e| {
            PricingError::model_failure_with_context(e.to_string(), PricingErrorContext::default())
        })?;

        let mut result = ValuationResult::stamped(asian.id(), as_of, pv);
        if stderr > 0.0 {
            result
                .measures
                .insert(crate::metrics::MetricId::custom("mc_stderr"), stderr);
        }
        Ok(result)
    }
}
