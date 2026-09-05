//! Valuation result types and output formatting.
//!
//! This module provides the result envelope types returned by pricing operations,
//! encapsulating present values, computed metrics, and execution metadata.
//!
//! # Features
//!
//! - **ValuationResult**: Standard result envelope with PV and metrics
//! - **ResultsMeta**: Metadata tracking config, timing, and FX policy
//! - **Table Export**: Flatten results into `core::table` rows for pandas/Polars consumers
//!
//! # Result Structure
//!
//! Every pricing operation returns a [`crate::results::ValuationResult`] containing:
//!
//! ```text
//! ValuationResult {
//!     value: Money,              // Present value in instrument currency
//!     measures: IndexMap<MetricId, f64>,  // Computed metrics (DV01, Greeks, etc.)
//!     meta: ResultsMeta,         // Execution metadata
//! }
//! ```
//!
//! # Metadata Tracking
//!
//! [`crate::results::ResultsMeta`] captures important context for audit and reproducibility:
//! - **Timestamp**: When the valuation was computed
//! - **Rounding context**: Numeric precision policy applied
//! - **FX policy**: Currency conversion method (if applicable)
//! - **Parallel flag**: Whether parallel execution was used
//!
//! # Quick Example
//!
//! ```
//! use finstack_quant_core::currency::Currency;
//! use finstack_quant_core::money::Money;
//! use finstack_quant_core::types::Rate;
//! use finstack_quant_valuations::instruments::{
//!     Bond, BondConvention, Instrument, PricingOptions,
//! };
//! use finstack_quant_valuations::metrics::MetricId;
//! use finstack_quant_core::market_data::context::MarketContext;
//! use finstack_quant_core::market_data::term_structures::DiscountCurve;
//! use time::macros::date;
//!
//! # fn main() -> finstack_quant_core::Result<()> {
//! let as_of = date!(2025-01-15);
//! let bond = Bond::with_convention(
//!     "CORP-001",
//!     Money::from((1_000_000_i64, Currency::USD)),
//!     Rate::from_decimal(0.05).expect("valid rate fixture"),
//!     date!(2024-01-15),
//!     date!(2034-01-15),
//!     BondConvention::UsCorporate,
//!     "USD-OIS",
//! )?;
//! let market = MarketContext::new().insert(
//!     DiscountCurve::builder("USD-OIS")
//!         .base_date(as_of)
//!         .knots([(0.0, 1.0), (30.0, 0.40)])
//!         .build()?,
//! );
//!
//! let result = bond.price_with_metrics(&market, as_of, &[MetricId::Ytm, MetricId::Dv01], PricingOptions::default())?;
//!
//! assert_eq!(result.value.currency().to_string(), "USD");
//! assert!(result.metric(MetricId::Ytm).is_some());
//! assert!(result.metric(MetricId::Dv01).is_some_and(|dv01| dv01 < 0.0));
//! # Ok(())
//! # }
//! ```
//!
//! # See Also
//!
//! - [`crate::results::ValuationResult`] for the main result type
//! - [`crate::results::ResultsMeta`] for execution metadata
//! - [`crate::metrics`] for available metric calculators
//!
//! # References
//!
//! - Metric and sensitivity interpretation: `docs/REFERENCES.md#tuckman-serrat-fixed-income`

pub(crate) mod dataframe;
mod valuation_result;

pub use dataframe::{ValuationLongRow, ValuationRow};
pub use finstack_quant_core::config::ResultsMeta;
pub use valuation_result::{
    CreditDerivativeValuationDetails, FxValuationDetails, MonteCarloValuationDetails,
    ValuationDetails, ValuationResult,
};
