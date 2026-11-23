//! Metrics for term loan instruments.
//!
//! This module provides risk and return metrics for term loans including:
//! - **Yield metrics**: YTM, YTC, YTW, YT2Y/3Y/4Y
//! - **Spread metrics**: Discount Margin, All-In Rate
//! - **Risk metrics**: DV01, CS01, Theta (bucketed and parallel)
//!
//! # Available Metrics
//!
//! ## Yield and Return
//!
//! - **YTM** (Yield-to-Maturity): IRR to final maturity
//! - **YTC** (Yield-to-Call): IRR to first call date
//! - **YTW** (Yield-to-Worst): Minimum yield across all call dates and maturity
//! - **YT2Y/3Y/4Y**: IRR to fixed 2/3/4-year horizons
//! - **All-In Rate**: Effective borrower cost including fees
//! - **OID EIR Amortization**: Effective interest rate amortization schedule
//!
//! ## Spread Metrics
//!
//! - **Discount Margin**: Additive spread for floating-rate loans to match price
//!
//! ## Risk Metrics
//!
//! - **DV01**: Dollar value of 1bp parallel shift (combined parallel + key-rate)
//! - **BucketedDV01**: Key-rate risk by tenor bucket
//! - **CS01**: Credit spread sensitivity (parallel)
//! - **BucketedCS01**: Credit spread sensitivity by tenor
//! - **Theta**: Time decay (1-day price change)
//! - **Recovery01**: PV change per +1% recovery on a loan with a credit curve
//! - **Jump to Default**: Recovery of par less current value on such a loan
//!
//! # Quick Example
//!
//! ```text
//! use finstack_quant_valuations_fixed_income::term_loan::TermLoan;
//! use finstack_quant_valuations_core::metrics::{MetricId, MetricRegistry};
//! use finstack_quant_core::market_data::context::MarketContext;
//! use finstack_quant_core::dates::Date;
//! use time::Month;
//!
//! # fn example() -> Result<(), Box<dyn std::error::Error>> {
//! let loan = TermLoan::example().unwrap();
//! let market = MarketContext::new();
//! let as_of = Date::from_calendar_date(2025, Month::January, 15)?;
//!
//! // Compute base value
//! // let pv = loan.value(&market, as_of)?;
//!
//! // Request metrics
//! let metrics = vec![MetricId::Ytm, MetricId::Dv01, MetricId::custom("ytw")];
//! // let result = loan.price_with_metrics(&market, as_of, &metrics, finstack_quant_valuations_core::instruments::PricingOptions::default())?;
//! # Ok(())
//! # }
//! ```
//!
//! # Registration
//!
//! Metrics are registered via [`register_term_loan_metrics()`] in the global registry.
//!
//! # See Also
//!
//! - [`YtmCalculator`] for yield-to-maturity implementation
//! - [`DiscountMarginCalculator`] for floating-rate spread calculation
//! - [`AllInRateCalculator`] for borrower cost calculation

mod cs01;
mod default_risk;
mod discount_margin;
pub(crate) mod irr_helpers;
mod oas;
mod register;
mod risk_view;
mod ytm;
mod ytn;
mod ytw;

pub(crate) use discount_margin::DiscountMarginCalculator;
pub(crate) use oas::OasCalculator;
pub(crate) use register::register_term_loan_metrics;
pub(crate) use ytm::YtmCalculator;
pub(crate) use ytn::{Yt2yCalculator, Yt3yCalculator, Yt4yCalculator};
pub(crate) use ytw::YtwCalculator;
