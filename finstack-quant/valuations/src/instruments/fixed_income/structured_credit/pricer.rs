//! Unified pricer for all structured credit instruments.
//!
//! The pricing logic is identical across ABS, CLO, CMBS, and RMBS since they all
//! use the shared waterfall implementation via the `StructuredCreditInstrument` trait.
//!
//! Hedge swaps attached to a deal ([`super::HedgeSwap`]) are not valued here
//! as an overlay: the simulation engine settles them through the waterfall
//! every period, so their effect is already in the tranche cashflows.

use super::StructuredCredit;
use crate::instruments::{Instrument, PricingOptions};
use crate::metrics::MetricId;
use crate::results::ValuationResult;
use finstack_quant_core::dates::Date;
use finstack_quant_core::market_data::context::MarketContext;

impl StructuredCredit {
    /// Price with additional risk metrics.
    ///
    /// Computes the base NPV plus any requested metrics such as duration,
    /// spread, etc., through the registry pricer with default options.
    ///
    /// # Arguments
    ///
    /// * `context` - Market context holding the deal's discount curve, any
    ///   floating-rate index curves and fixings, and the hedge swaps' curves.
    /// * `as_of` - Requested valuation date; the deal's quote settlement date
    ///   or the last contractual boundary may move the effective date.
    /// * `metrics` - Metric identifiers to compute alongside the value.
    pub fn price_with_metrics_standalone(
        &self,
        context: &MarketContext,
        as_of: Date,
        metrics: &[MetricId],
    ) -> finstack_quant_core::Result<ValuationResult> {
        Ok(Instrument::price_with_metrics(
            self,
            context,
            as_of,
            metrics,
            PricingOptions::default(),
        )?)
    }
}

// Generic pricer implementation is used directly via common_impl::GenericInstrumentPricer
