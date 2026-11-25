//! Valuation-side resolution of closed-form Heston parameters.

use crate::instruments::pricing::market_inputs::get_unitless_scalar_strict;
use finstack_quant_core::market_data::context::MarketContext;
use finstack_quant_models::volatility::heston::HestonParams;

/// Resolve all required Heston scalars from a valuation market context.
///
/// Reads the unitless scalars `HESTON_KAPPA`, `HESTON_THETA`, `HESTON_SIGMA_V`,
/// `HESTON_RHO` and `HESTON_V0` into the canonical [`HestonParams`].
/// Each scalar is required and must be unitless.
///
/// # Arguments
///
/// * `market` - Market context holding the five `HESTON_*` unitless scalars.
///
/// # Errors
///
/// Returns `Error::Validation` when a scalar is missing or carries a unit, or
/// when [`HestonParams::new`] rejects the parameter set.
pub fn heston_params_from_market_strict(
    market: &MarketContext,
) -> finstack_quant_core::Result<HestonParams> {
    let kappa = get_unitless_scalar_strict(market, "HESTON_KAPPA", "Heston")?;
    let theta = get_unitless_scalar_strict(market, "HESTON_THETA", "Heston")?;
    let vol_of_vol = get_unitless_scalar_strict(market, "HESTON_SIGMA_V", "Heston")?;
    let rho = get_unitless_scalar_strict(market, "HESTON_RHO", "Heston")?;
    let v0 = get_unitless_scalar_strict(market, "HESTON_V0", "Heston")?;
    HestonParams::new(v0, kappa, theta, vol_of_vol, rho)
}

/// Default Monte Carlo path count for stochastic-volatility simulation pricers.
pub const DEFAULT_HESTON_MC_PATHS: usize = 100_000;

/// Default Monte Carlo time steps per year for stochastic-volatility simulation pricers.
pub const DEFAULT_HESTON_STEPS_PER_YEAR: f64 = 252.0;

/// Simulation parameters for running the shared Heston QE Monte Carlo engine.
pub struct HestonMcRequest<'a> {
    /// Instrument identifier used for deterministic seed derivation when no seed override is set.
    pub instrument_id: &'a finstack_quant_core::types::InstrumentId,
    /// Model configuration overrides (`mc_paths`, `mc_antithetic`, `mc_seed`, `mc_target_ci_half_width`).
    pub model_config: &'a crate::instruments::pricing_overrides::ModelConfig,
    /// Fallback path count when `model_config.mc_paths` is unset.
    pub default_num_paths: usize,
    /// Initial underlying spot price.
    pub spot: f64,
    /// Continuously compounded risk-free rate consistent with `discount_factor`.
    pub r: f64,
    /// Continuously compounded dividend yield.
    pub q: f64,
    /// Discount factor from valuation date to expiry.
    pub discount_factor: f64,
    /// Payoff and valuation currency.
    pub currency: finstack_quant_core::currency::Currency,
    /// Discretized simulation time grid.
    pub time_grid: finstack_quant_core::math::time_grid::TimeGrid,
    /// Optional cumulative deterministic drift schedule attached to the Heston process.
    pub drift_schedule:
        Option<std::sync::Arc<finstack_quant_models::monte_carlo::process::gbm::DriftSchedule>>,
}

/// Price an instrument under the Heston QE Monte Carlo engine using strict market calibration.
///
/// Resolves `HESTON_*` market scalars via [`heston_params_from_market_strict`],
/// honors `mc_paths`, `mc_antithetic`, `mc_target_ci_half_width`, and `mc_seed`
/// overrides on `req.model_config`, and runs the
/// [QE Heston discretization](finstack_quant_models::monte_carlo::discretization::qe_heston::QeHeston)
/// with the
/// [Philox random-number generator](finstack_quant_models::monte_carlo::rng::philox::PhiloxRng).
///
/// # Arguments
///
/// * `market` - Market context supplying the required `HESTON_*` unitless scalars.
/// * `req` - Prepared grid, carry, spot, currency, and override inputs.
/// * `build_payoff` - Factory constructing the path payoff from the resolved grid and Heston parameters.
pub fn price_heston_mc<P>(
    market: &MarketContext,
    req: HestonMcRequest<'_>,
    build_payoff: impl FnOnce(
        &finstack_quant_core::math::time_grid::TimeGrid,
        &HestonParams,
    ) -> finstack_quant_core::Result<P>,
) -> finstack_quant_core::Result<(finstack_quant_core::money::Money, f64)>
where
    P: finstack_quant_models::monte_carlo::traits::Payoff + Sync,
{
    use crate::instruments::pricing::monte_carlo::resolve_mc_paths;
    use finstack_quant_models::monte_carlo::discretization::qe_heston::QeHeston;
    use finstack_quant_models::monte_carlo::engine::{McEngine, McEngineConfig, McPricingInputs};
    use finstack_quant_models::monte_carlo::process::heston::{HestonProcess, HestonProcessParams};
    use finstack_quant_models::monte_carlo::rng::philox::PhiloxRng;

    let heston_params = heston_params_from_market_strict(market)?;
    let payoff = build_payoff(&req.time_grid, &heston_params)?;
    let mut process = HestonProcess::new(HestonProcessParams {
        r: req.r,
        q: req.q,
        model: heston_params,
    });
    if let Some(drift_schedule) = req.drift_schedule {
        process = process.with_drift_schedule(drift_schedule);
    }

    let num_paths = resolve_mc_paths(req.model_config.mc_paths, req.default_num_paths)?;
    let mut engine_config = McEngineConfig::new(num_paths, req.time_grid);
    if let Some(antithetic) = req.model_config.mc_antithetic {
        engine_config.antithetic = antithetic;
    }
    if let Some(target_ci) = req.model_config.mc_target_ci_half_width {
        engine_config.target_ci_half_width = Some(target_ci);
        // `target_ci_half_width` auto-stopping requires serial execution.
        engine_config.use_parallel = false;
    }

    let engine = McEngine::new(engine_config);
    let rng = PhiloxRng::new(req.model_config.mc_seed(req.instrument_id));
    let discretization = QeHeston::new();
    let initial_state = [req.spot, heston_params.v0];

    let result = engine.price(McPricingInputs {
        rng: &rng,
        process: &process,
        disc: &discretization,
        initial_state: &initial_state,
        payoff: &payoff,
        currency: req.currency,
        discount_factor: req.discount_factor,
    })?;

    Ok((result.mean, result.stderr))
}

#[cfg(test)]
mod tests {
    use super::*;
    use finstack_quant_core::market_data::scalars::MarketScalar;

    #[test]
    fn strict_resolution_rejects_missing_heston_scalars() {
        let error = heston_params_from_market_strict(&MarketContext::new())
            .expect_err("missing Heston scalars must fail");
        assert!(error.to_string().contains("HESTON_KAPPA"));
    }

    #[test]
    fn strict_resolution_names_a_partially_missing_scalar_set() {
        let market = MarketContext::new()
            .insert_price(
                "HESTON_KAPPA",
                MarketScalar::unitless(1.5).expect("valid scalar"),
            )
            .insert_price(
                "HESTON_THETA",
                MarketScalar::unitless(0.06).expect("valid scalar"),
            )
            .insert_price(
                "HESTON_SIGMA_V",
                MarketScalar::unitless(0.4).expect("valid scalar"),
            )
            .insert_price(
                "HESTON_RHO",
                MarketScalar::unitless(-0.5).expect("valid scalar"),
            );
        let error =
            heston_params_from_market_strict(&market).expect_err("missing HESTON_V0 must fail");
        assert!(error.to_string().contains("HESTON_V0"));
    }

    #[test]
    fn strict_resolution_preserves_complete_market_parameters() {
        let market = MarketContext::new()
            .insert_price(
                "HESTON_KAPPA",
                MarketScalar::unitless(1.5).expect("valid scalar"),
            )
            .insert_price(
                "HESTON_THETA",
                MarketScalar::unitless(0.06).expect("valid scalar"),
            )
            .insert_price(
                "HESTON_SIGMA_V",
                MarketScalar::unitless(0.4).expect("valid scalar"),
            )
            .insert_price(
                "HESTON_RHO",
                MarketScalar::unitless(-0.5).expect("valid scalar"),
            )
            .insert_price(
                "HESTON_V0",
                MarketScalar::unitless(0.05).expect("valid scalar"),
            );
        let params =
            heston_params_from_market_strict(&market).expect("complete Heston inputs must resolve");
        assert_eq!(params.kappa, 1.5);
        assert_eq!(params.theta, 0.06);
        assert_eq!(params.vol_of_vol, 0.4);
        assert_eq!(params.rho, -0.5);
        assert_eq!(params.v0, 0.05);
    }
}
