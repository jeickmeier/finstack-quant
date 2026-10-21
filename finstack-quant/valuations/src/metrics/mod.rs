//! Metrics framework: metric identifiers, calculators and registries, with the
//! standard registry that merges every instrument family.
//!
//! Everything but [`standard_registry`](crate::metrics::standard_registry) is defined in
//! `finstack_quant_valuations_core::metrics` and re-exported here unchanged.
//! Metric values are raw `f64`s whose units, bump and sign conventions are the
//! contract on [`MetricId`](finstack_quant_valuations_core::metrics::MetricId); see the core module docs for the architecture.
//!
//! # Quick Start Examples
//!
//! ## Example 1: Computing Bucketed DV01 for a Bond
//!
//! ```
//! use finstack_quant_core::currency::Currency;
//! use finstack_quant_core::money::Money;
//! use finstack_quant_core::types::Rate;
//! use finstack_quant_valuations::instruments::{BondConvention, Instrument, PricingOptions};
//! use finstack_quant_valuations::instruments::fixed_income::bond::Bond;
//! use finstack_quant_valuations::metrics::MetricId;
//! use finstack_quant_core::market_data::context::MarketContext;
//! use finstack_quant_core::market_data::term_structures::DiscountCurve;
//! use time::macros::date;
//!
//! # fn main() -> finstack_quant_core::Result<()> {
//! let as_of = date!(2025-01-01);
//! let bond = Bond::with_convention(
//!     "CORP-001",
//!     Money::from((1_000_000_i64, Currency::USD)),
//!     Rate::from_decimal(0.05).expect("valid rate fixture"),
//!     date!(2024-01-15),
//!     date!(2034-01-15),
//!     BondConvention::UsCorporate,
//!     "USD-OIS",
//! )?;
//!
//! let market = MarketContext::new().insert(
//!     DiscountCurve::builder("USD-OIS")
//!         .base_date(as_of)
//!         .knots([(0.0, 1.0), (30.0, 0.40)])
//!         .build()?,
//! );
//! let metrics = vec![MetricId::BucketedDv01];
//!
//! let result = bond.price_with_metrics(&market, as_of, &metrics, PricingOptions::default())?;
//!
//! let pv = result.value.amount();
//! assert!(pv > 0.0);
//!
//! // A fixed-rate bond loses value when rates rise, so DV01 is negative.
//! let total_dv01 = result
//!     .measures
//!     .get(MetricId::BucketedDv01.as_str())
//!     .copied()
//!     .unwrap_or_default();
//! assert!(total_dv01 < 0.0);
//! # Ok(())
//! # }
//! ```
//!
//! ## Example 2: Computing Parallel DV01 for an Interest Rate Swap
//!
//! ```
//! use finstack_quant_valuations::instruments::{Instrument, PricingOptions};
//! use finstack_quant_valuations::instruments::rates::irs::InterestRateSwap;
//! use finstack_quant_valuations::metrics::MetricId;
//! use finstack_quant_core::market_data::context::MarketContext;
//! use finstack_quant_core::market_data::scalars::ScalarTimeSeries;
//! use finstack_quant_core::market_data::term_structures::{DiscountCurve, ForwardCurve};
//! use time::macros::date;
//!
//! # fn main() -> finstack_quant_core::Result<()> {
//! let as_of = date!(2025-01-01);
//! let swap = InterestRateSwap::example()?;
//!
//! // A seasoned swap needs a discount curve, a projection curve for the
//! // floating leg, and the historical fixings its past resets already consumed.
//! let market = MarketContext::new()
//!     .insert(
//!         DiscountCurve::builder("USD-OIS")
//!             .base_date(as_of)
//!             .knots([(0.0, 1.0), (30.0, 0.40)])
//!             .build()?,
//!     )
//!     .insert(
//!         ForwardCurve::builder("USD-SOFR-3M", 0.25)
//!             .base_date(as_of)
//!             .knots([(0.0, 0.04), (30.0, 0.04)])
//!             .build()?,
//!     )
//!     .insert_series(ScalarTimeSeries::new(
//!         "FIXING:USD-SOFR-3M",
//!         (0..400)
//!             .map(|i| (date!(2023-12-01) + time::Duration::days(i), 0.05))
//!             .collect::<Vec<_>>(),
//!         None,
//!     )?);
//! let metrics = vec![MetricId::Dv01]; // Parallel DV01
//!
//! let result = swap.price_with_metrics(&market, as_of, &metrics, PricingOptions::default())?;
//!
//! let dv01 = result
//!     .measures
//!     .get(MetricId::Dv01.as_str())
//!     .copied()
//!     .unwrap_or_default();
//! // Sign follows the swap's direction: a receiver gains when rates fall.
//! assert!(dv01.abs() > 0.0);
//! # Ok(())
//! # }
//! ```
//!
//! ## Example 3: Computing Theta (Time Decay) for an Option
//!
//! ```
//! use finstack_quant_valuations::instruments::{Instrument, OptionType, PricingOptions};
//! use finstack_quant_valuations::instruments::equity::equity_option::EquityOption;
//! use finstack_quant_valuations::metrics::MetricId;
//! use finstack_quant_core::currency::Currency;
//! use finstack_quant_core::dates::create_date;
//! use finstack_quant_core::market_data::context::MarketContext;
//! use finstack_quant_core::market_data::scalars::MarketScalar;
//! use finstack_quant_core::market_data::surfaces::VolSurface;
//! use finstack_quant_core::market_data::term_structures::DiscountCurve;
//! use finstack_quant_core::money::Money;
//! use time::Month;
//!
//! # fn main() -> finstack_quant_core::Result<()> {
//! let as_of = create_date(2024, Month::January, 1)?;
//! let expiry = create_date(2024, Month::July, 1)?; // 6-month option
//!
//! let option = EquityOption::european(
//!     "OPT-001",
//!     "SPX",
//!     4500.0,
//!     expiry,
//!     100.0,
//!     Currency::USD,
//!     OptionType::Call,
//! )?;
//!
//! // `european` binds the standard ids: USD-OIS, EQUITY-SPOT,
//! // EQUITY-VOL, and EQUITY-DIVYIELD.
//! let market = MarketContext::new()
//!     .insert(
//!         DiscountCurve::builder("USD-OIS")
//!             .base_date(as_of)
//!             .knots([(0.0, 1.0), (5.0, 0.80)])
//!             .build()?,
//!     )
//!     .insert_price("EQUITY-SPOT", MarketScalar::Unitless(4500.0))
//!     .insert_price("EQUITY-DIVYIELD", MarketScalar::Unitless(0.015))
//!     .insert_surface(
//!         VolSurface::builder("EQUITY-VOL")
//!             .expiries(&[0.25, 0.5, 1.0])
//!             .strikes(&[4000.0, 4500.0, 5000.0])
//!             .row(&[0.20, 0.20, 0.20])
//!             .row(&[0.20, 0.20, 0.20])
//!             .row(&[0.20, 0.20, 0.20])
//!             .build()?,
//!     );
//! let metrics = vec![MetricId::Theta];
//!
//! let result = option.price_with_metrics(&market, as_of, &metrics, PricingOptions::default())?;
//!
//! // Negative theta: a long option loses value as time passes.
//! let theta = result
//!     .measures
//!     .get(MetricId::Theta.as_str())
//!     .copied()
//!     .unwrap_or_default();
//! assert!(theta < 0.0);
//! # Ok(())
//! # }
//! ```
//!
//! ## Example 4: Computing Multiple Greeks for an Option
//!
//! ```
//! use finstack_quant_valuations::instruments::{Instrument, OptionType, PricingOptions};
//! use finstack_quant_valuations::instruments::equity::equity_option::EquityOption;
//! use finstack_quant_valuations::metrics::MetricId;
//! use finstack_quant_core::currency::Currency;
//! use finstack_quant_core::dates::create_date;
//! use finstack_quant_core::market_data::context::MarketContext;
//! use finstack_quant_core::market_data::scalars::MarketScalar;
//! use finstack_quant_core::market_data::surfaces::VolSurface;
//! use finstack_quant_core::market_data::term_structures::DiscountCurve;
//! use finstack_quant_core::money::Money;
//! use time::Month;
//!
//! # fn main() -> finstack_quant_core::Result<()> {
//! let as_of = create_date(2024, Month::January, 1)?;
//! let option = EquityOption::european(
//!     "OPT-001",
//!     "SPX",
//!     4500.0,
//!     create_date(2024, Month::July, 1)?,
//!     100.0,
//!     Currency::USD,
//!     OptionType::Call,
//! )?;
//! let market = MarketContext::new()
//!     .insert(
//!         DiscountCurve::builder("USD-OIS")
//!             .base_date(as_of)
//!             .knots([(0.0, 1.0), (5.0, 0.80)])
//!             .build()?,
//!     )
//!     .insert_price("EQUITY-SPOT", MarketScalar::Unitless(4500.0))
//!     .insert_price("EQUITY-DIVYIELD", MarketScalar::Unitless(0.015))
//!     .insert_surface(
//!         VolSurface::builder("EQUITY-VOL")
//!             .expiries(&[0.25, 0.5, 1.0])
//!             .strikes(&[4000.0, 4500.0, 5000.0])
//!             .row(&[0.20, 0.20, 0.20])
//!             .row(&[0.20, 0.20, 0.20])
//!             .row(&[0.20, 0.20, 0.20])
//!             .build()?,
//!     );
//! let metrics = vec![
//!     MetricId::Delta,
//!     MetricId::Gamma,
//!     MetricId::Vega,
//!     MetricId::Theta,
//!     MetricId::Rho,
//! ];
//!
//! let result = option.price_with_metrics(&market, as_of, &metrics, PricingOptions::default())?;
//!
//! let greek = |id: MetricId| result.measures.get(id.as_str()).copied().unwrap_or_default();
//!
//! // Greeks are scaled by the 100-unit contract size, so an at-the-money
//! // call has delta near 50 rather than 0.5. Gamma and vega are positive
//! // and theta is negative for a long option.
//! assert!(greek(MetricId::Delta) > 0.0 && greek(MetricId::Delta) < 100.0);
//! assert!(greek(MetricId::Gamma) > 0.0);
//! assert!(greek(MetricId::Vega) > 0.0);
//! assert!(greek(MetricId::Theta) < 0.0);
//! # Ok(())
//! # }
//! ```
//!

pub use finstack_quant_valuations_core::metrics::*;

/// The shared metric registry merging every built-in family.
///
/// This registry includes metrics for:
/// - **Bonds**: YTM, duration, convexity, accrued interest, credit spreads
/// - **Interest Rate Swaps**: DV01, annuity factors, par rates
/// - **Deposits**: Discount factors, par rates, year fractions
/// - **Risk**: Bucketed DV01, time decay (theta)
///
/// It is the metric registry of the umbrella family
/// [`STANDARD`](crate::pricer::STANDARD). An instrument priced without an
/// explicit registry resolves its own family's registry instead, which
/// answers identically for the instrument types that family owns.
///
/// See unit tests and `examples/` for usage.
pub fn standard_registry() -> &'static MetricRegistry {
    crate::pricer::STANDARD.metrics()
}
