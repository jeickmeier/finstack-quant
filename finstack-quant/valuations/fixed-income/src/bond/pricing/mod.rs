//! Bond pricing engines and utilities.
//!
//! # Engines
//!
//! Each engine implements the core pricing math for one model; the shared
//! `BondPricer` registry adapter preserves the caller's explicit `ModelKey`
//! selection:
//! - `discount`: Non-callable PV = sum(CF_i * DF_i) from projected
//!   cashflows and discount curves; an attached credit curve is ignored
//! - `hazard`: Non-callable survival-weighted PV plus fractional recovery of
//!   par (FRP); an explicit hazard curve is required
//! - [`tree`]: Rates-only and joint rates-credit option rollback and OAS,
//!   including call, put, and return-floor rights
//! - [`merton_mc`]: Merton structural credit Monte Carlo for PIK bonds
//!
//! # Utilities
//!
//! - [`quote_conversions`]: Price/yield/spread conversion functions
//! - [`ytm_solver`]: Robust yield-to-maturity calculation
//! - `settlement`: Settlement date and accrued interest utilities
//!
//! [`tree`]: crate::bond::pricing::tree
//! [`merton_mc`]: crate::bond::pricing::merton_mc
//! [`quote_conversions`]: crate::bond::pricing::quote_conversions
//! [`ytm_solver`]: crate::bond::pricing::ytm_solver

/// Discount curve-based bond pricing (PV = sum CF_i * DF_i).
pub(crate) mod discount;
/// Non-callable hazard-rate pricing with fractional recovery of par.
pub(crate) mod hazard;
/// Merton structural credit Monte Carlo for PIK bonds.
pub mod merton_mc;
pub mod quote_conversions;
pub(crate) mod return_floor;
pub(crate) mod settlement;
pub(crate) mod time_basis;
/// Short-rate and rates-credit option pricing and OAS.
pub mod tree;
pub mod ytm_solver;

pub(crate) use merton_mc::SimpleBondMertonMcPricer;

use crate::bond::Bond;
use finstack_quant_core::dates::Date;
use finstack_quant_core::market_data::context::MarketContext;
use finstack_quant_core::money::Money;
use finstack_quant_valuations_core::instruments::common_impl::helpers::attach_mc_diagnostics;
use finstack_quant_valuations_core::instruments::Instrument;
use finstack_quant_valuations_core::pricer::{
    expect_inst, InstrumentType, ModelKey, Pricer, PricerKey, PricingError, PricingErrorContext,
};
use finstack_quant_valuations_core::results::{
    MonteCarloValuationDetails, ValuationDetails, ValuationResult,
};

/// Registry adapter that preserves the caller-selected bond model.
pub(crate) struct BondPricer {
    model: ModelKey,
}

impl BondPricer {
    /// Create an adapter for one supported bond model.
    pub(crate) const fn new(model: ModelKey) -> Self {
        Self { model }
    }

    fn error_context(&self, bond: &Bond) -> PricingErrorContext {
        let mut context = PricingErrorContext::from_instrument(bond)
            .model(self.model)
            .curve_id(bond.discount_curve_id.as_str());
        if let Some(credit_curve_id) = &bond.credit_curve_id {
            context = context.curve_id(credit_curve_id.as_str());
        }
        context
    }
}

impl Pricer for BondPricer {
    fn key(&self) -> PricerKey {
        PricerKey::new(InstrumentType::Bond, self.model)
    }

    fn price_dyn(
        &self,
        instrument: &dyn Instrument,
        market: &MarketContext,
        as_of: Date,
    ) -> std::result::Result<ValuationResult, PricingError> {
        let bond = expect_inst::<Bond>(instrument, InstrumentType::Bond)?;
        let context = self.error_context(bond);
        let outcome = bond
            .price_for_model_outcome(self.model, market, as_of)
            .map_err(|error| PricingError::from_core(error, context))?;
        let mut result = ValuationResult::stamped(
            bond.id(),
            as_of,
            Money::new(outcome.amount, bond.notional.currency()).map_err(|error| {
                finstack_quant_valuations_core::pricer::PricingError::from_core(
                    error,
                    finstack_quant_valuations_core::pricer::PricingErrorContext::from_instrument(
                        bond,
                    ),
                )
            })?,
        );
        if let Some(lsmc) = outcome.lsmc {
            attach_mc_diagnostics(&mut result, &lsmc.estimate);
            result =
                result.with_details(ValuationDetails::MonteCarlo(MonteCarloValuationDetails {
                    model_key: self.model,
                    standard_error: lsmc.estimate.stderr,
                    training_paths: lsmc.training_paths,
                    training_simulated_paths: lsmc.training_simulated_paths,
                    make_whole_training_paths: lsmc.make_whole_training_paths,
                    make_whole_training_simulated_paths: lsmc.make_whole_training_simulated_paths,
                    estimator_paths: lsmc.estimate.num_paths,
                    simulated_paths: lsmc.pricing_simulated_paths,
                    seed: lsmc.seed,
                    time_grid: lsmc.time_grid,
                    antithetic: lsmc.antithetic,
                    sobol: false,
                    brownian_bridge: false,
                }));
        }
        Ok(result)
    }

    fn price_raw_dyn(
        &self,
        instrument: &dyn Instrument,
        market: &MarketContext,
        as_of: Date,
    ) -> std::result::Result<f64, PricingError> {
        let bond = expect_inst::<Bond>(instrument, InstrumentType::Bond)?;
        bond.price_for_model_raw(self.model, market, as_of)
            .map_err(|error| PricingError::from_core(error, self.error_context(bond)))
    }
}
