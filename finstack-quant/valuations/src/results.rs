//! Valuation result types and output formatting.
//!
//! Defined in `finstack_quant_valuations_core::results` and re-exported here
//! unchanged: [`ValuationResult`](finstack_quant_valuations_core::results::ValuationResult),
//! [`ValuationDetails`](finstack_quant_valuations_core::results::ValuationDetails), [`ResultsMeta`](finstack_quant_valuations_core::results::ResultsMeta),
//! provenance and the row exports.
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
pub use finstack_quant_valuations_core::results::*;
