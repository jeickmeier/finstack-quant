//! Strongly-typed metric identifiers for compile-time validation.
//!
//! Provides a comprehensive set of metric IDs covering bond, IRS, deposit,
//! and risk metrics. Each ID is strongly-typed to prevent runtime errors
//! and enable compile-time validation of metric dependencies.
//!
//! # Metric Categories
//!
//! - **Bond metrics**: Yield, duration, convexity, pricing, credit spreads
//! - **IRS metrics**: DV01, annuity factors, par rates, present values
//! - **Deposit metrics**: Discount factors, par rates, year fractions
//! - **Risk metrics**: DV01 (standard for all parallel rate sensitivity), CS01, BucketedDV01, BucketedCS01, Theta, and all standardized "01" sensitivity metrics
//! - **Standardized sensitivity metrics**: Dividend01, Inflation01, Prepayment01, Default01, Severity01, Conversion01, Nav01, Carry01, Hurdle01, Dv01Domestic, Dv01Foreign, Fx01, SpreadDv01, Correlation01, FxVega, ConvexityAdjustmentRisk
//! - **Custom metrics**: User-defined metrics with dynamic identifiers

mod composite;
mod credit;
mod equity_volatility;
pub(crate) mod group;
mod instrument;
pub(crate) mod metric_id;
mod rates;
mod risk;
mod standard;
pub(crate) mod suggest;
#[cfg(test)]
mod tests;
pub(crate) mod unit;

pub(crate) use group::MetricGroup;
pub(crate) use metric_id::MetricId;
pub(crate) use suggest::{MAX_METRIC_SUGGESTIONS, closest_metric_names};
#[cfg(test)]
pub(super) use unit::MetricUnit;
