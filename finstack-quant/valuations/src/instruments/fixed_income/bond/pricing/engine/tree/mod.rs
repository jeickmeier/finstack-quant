//! Tree-based pricing engine for bonds with embedded options and OAS calculations.
//!
//! This module provides tree-based pricing for callable/putable bonds and option-adjusted
//! spread (OAS) calculations using either:
//! - **Short-rate tree**: For bonds without credit risk
//! - **Rates+credit tree**: For bonds with credit risk (two-factor model)
//!
//! # Pricing Models
//!
//! ## Short-Rate Tree
//! Used for bonds without embedded credit risk. The tree models interest rate evolution
//! and applies call/put constraints via backward induction.
//!
//! ## Rates+Credit Tree
//! Used when a hazard curve is present in the market context. Models both interest rate
//! and credit risk evolution, with default events and recovery payments.
//!
//! # Examples
//!
//! ```ignore
//! use finstack_quant_valuations::instruments::fixed_income::bond::Bond;
//! use finstack_quant_valuations::instruments::fixed_income::bond::pricing::engine::tree::TreePricer;
//! use finstack_quant_core::market_data::context::MarketContext;
//! use finstack_quant_core::dates::Date;
//!
//! # let bond = Bond::example().unwrap();
//! # let market = MarketContext::new();
//! # let as_of = Date::from_calendar_date(2024, time::Month::January, 15).unwrap();
//! let pricer = TreePricer::new();
//! let oas_bp = pricer.calculate_oas(&bond, &market, as_of, 98.5)?;
//! # Ok::<(), Box<dyn std::error::Error>>(())
//! ```
//!
//! # See Also
//!
//! - `TreePricer` for OAS calculation
//! - tree-valuator implementation details in this module
//! - `TreePricerConfig` for configuration options

mod bond_valuator;
mod config;
#[cfg(test)]
mod tests;
mod tree_pricer;

pub use bond_valuator::BondValuator;
pub use config::{bond_tree_config, TreeModelChoice, TreePricerConfig};
pub use tree_pricer::{calculate_oas, TreePricer};

use crate::instruments::common_impl::traits::Instrument;
use crate::instruments::fixed_income::bond::types::Bond;
use crate::pricer::{
    InstrumentType, ModelKey, Pricer, PricerKey, PricingError, PricingErrorContext,
};
use crate::results::ValuationResult;
use finstack_quant_core::dates::Date;
use finstack_quant_core::market_data::context::MarketContext;
use indexmap::IndexMap;

/// Registry adapter for the option-adjusted-spread (OAS) bond pricer.
///
/// Routes `(InstrumentType::Bond, ModelKey::Tree)` to [`TreePricer::calculate_oas`].
/// Requires a quoted clean price in the bond's `pricing_overrides`.
pub(crate) struct SimpleBondOasPricer;

impl Pricer for SimpleBondOasPricer {
    fn key(&self) -> PricerKey {
        PricerKey::new(InstrumentType::Bond, ModelKey::Tree)
    }

    fn price_dyn(
        &self,
        instrument: &dyn Instrument,
        market: &MarketContext,
        as_of: Date,
    ) -> std::result::Result<ValuationResult, PricingError> {
        let bond = instrument
            .as_any()
            .downcast_ref::<Bond>()
            .ok_or_else(|| PricingError::type_mismatch(InstrumentType::Bond, instrument.key()))?;

        let ctx = PricingErrorContext::new()
            .instrument_id(bond.id())
            .instrument_type(InstrumentType::Bond)
            .model(ModelKey::Tree)
            .curve_id(bond.discount_curve_id.as_str());

        let pv = bond
            .value(market, as_of)
            .map_err(|e| PricingError::model_failure_with_context(e.to_string(), ctx.clone()))?;

        let clean_pct = bond
            .instrument_pricing_overrides
            .market_quotes
            .quoted_clean_price
            .ok_or_else(|| {
                PricingError::invalid_input_with_context(
                    "OAS requires quoted clean price",
                    ctx.clone(),
                )
            })?;

        let config = bond_tree_config(bond)
            .map_err(|e| PricingError::model_failure_with_context(e.to_string(), ctx.clone()))?;
        let oas_bp = TreePricer::with_config(config)
            .calculate_oas(bond, market, as_of, clean_pct)
            .map_err(|e| PricingError::model_failure_with_context(e.to_string(), ctx))?;

        let mut measures = IndexMap::new();
        measures.insert(crate::metrics::MetricId::custom("oas_bp"), oas_bp);

        let result = ValuationResult::stamped(bond.id(), as_of, pv);
        Ok(result.with_measures(measures))
    }
}
