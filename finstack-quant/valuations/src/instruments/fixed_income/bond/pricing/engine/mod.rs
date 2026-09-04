//! Bond pricing engines.
//!
//! Each engine implements the core pricing math for a specific model. The
//! shared `BondPricer` registry adapter preserves the caller's explicit
//! `ModelKey` selection:
//!
//! - [`self::discount`]: PV = sum(CF_i * DF_i) using discount curves
//! - [`self::hazard`]: Non-callable survival-weighted PV + fractional recovery
//!   of par (FRP)
//! - [`self::tree`]: Explicit rates-only and joint rates-credit option rollback
//!   and OAS
//! - [`self::merton_mc`]: Merton structural credit Monte Carlo for PIK bonds

/// Discount curve-based bond pricing (PV = sum CF_i * DF_i).
pub mod discount;
/// Non-callable hazard-rate pricing with fractional recovery of par.
pub mod hazard;
/// Merton structural credit Monte Carlo for PIK bonds.
pub mod merton_mc;
/// Short-rate and rates-credit option pricing and OAS.
pub mod tree;

pub(crate) use merton_mc::SimpleBondMertonMcPricer;

use crate::instruments::common_impl::helpers::attach_mc_diagnostics;
use crate::instruments::common_impl::traits::Instrument;
use crate::instruments::fixed_income::bond::Bond;
use crate::pricer::{
    expect_inst, InstrumentType, ModelKey, Pricer, PricerKey, PricingError, PricingErrorContext,
};
use crate::results::{MonteCarloValuationDetails, ValuationDetails, ValuationResult};
use finstack_quant_core::dates::Date;
use finstack_quant_core::market_data::context::MarketContext;
use finstack_quant_core::money::Money;

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
            Money::new(outcome.amount, bond.notional.currency()),
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
