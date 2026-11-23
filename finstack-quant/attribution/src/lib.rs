#![forbid(unsafe_code)]
#![warn(clippy::float_cmp)]
#![deny(clippy::unwrap_used)]
#![deny(clippy::expect_used)]
#![deny(clippy::panic)]
#![deny(clippy::unreachable)]
#![cfg_attr(
    test,
    allow(
        clippy::unwrap_used,
        clippy::expect_used,
        clippy::panic,
        clippy::unreachable,
        clippy::indexing_slicing,
        clippy::float_cmp,
    )
)]
#![doc(test(attr(allow(clippy::expect_used))))]

//! Multi-period P&L attribution for financial instruments.
//!
//! Attribution explains the change in an instrument's value between two dates
//! by separating carry and market-factor effects.
//!
//! # Methods
//!
//! | Entry point | Use when |
//! |---|---|
//! | [`pnl_bridge`] | Only raw endpoint P&L in an explicit currency is required |
//! | [`attribute_pnl_metrics_based`] | Already-priced first- and optional second-order sensitivities provide a fast approximation |
//! | [`attribute_pnl`] with [`AttributionMethod::Parallel`] | Independent factor effects and an interaction residual are required |
//! | [`attribute_pnl`] with [`AttributionMethod::Waterfall`] | An ordered full-revaluation decomposition is required |
//! | [`attribute_pnl`] with [`AttributionMethod::Taylor`] | A bump-and-reprice first- or second-order decomposition is required |
//! | [`attribute_pnl_many`] | Shared [`AttributionInputs`] applied to an ordered instrument batch |
//!
//! [`attribute_pnl`] runs any method, metrics-based included, from one
//! [`AttributionRequest`] carrying the instrument, both market states and
//! dates, the Finstack configuration, and the optional
//! credit-factor model, model-parameter snapshot, execution policy and
//! prepared endpoint values.
//!
//! # Conventions
//!
//! - Positive P&L is a gain to the long-position holder.
//! - Decomposition methods use a total-return basis: `total_pnl` includes cash
//!   paid in `[T₀, T₁)`, while `mark_to_market_pnl` preserves the raw endpoint
//!   value change when available. [`pnl_bridge`] returns only that raw
//!   endpoint change.
//! - Repricing methods isolate the date roll before market moves. Waterfall
//!   orders must begin with [`AttributionFactor::Carry`]; metrics-based carry
//!   uses `CarryTotal + FundingCost` or theta over the elapsed days.
//! - Direct decomposition functions report in the instrument's native pricing
//!   currency; [`pnl_bridge`] accepts an explicit target currency.
//!   [`AttributionConfig::target_currency`] can translate aggregate fields and
//!   every Money leaf of the detail maps: opening value uses T₀ market/date FX,
//!   closing value uses T₁ market/date FX, factors and detail leaves use T₁ FX,
//!   and the opening-position FX move is recorded separately as
//!   `fx_translation_pnl`. The `fx_pnl` field remains the pricing impact of FX
//!   inside the instrument.
//!
//! # Residuals and errors
//!
//! Every result reconciles `total_pnl` to its aggregate factor fields plus the
//! residual. A waterfall residual should be near zero when its order covers all
//! material factors; parallel residuals contain interactions and nonlinearity;
//! metrics-based residuals contain approximation and missing-sensitivity effects;
//! Taylor residuals also contain truncation and soft factor failures.
//!
//! The four decomposition methods reject reversed date ranges; same-day ranges
//! are valid. Waterfall also requires a nonempty, duplicate-free order beginning
//! with carry, and Taylor validates its bump ranges. Repricing, market-data,
//! currency, and FX failures propagate except on documented best-effort factor
//! paths, which record failures in metadata and residual instead.
//!
//! # Example
//!
//! ```rust
//! use finstack_quant_attribution::{attribute_pnl, AttributionMethod, AttributionRequest};
//! use finstack_quant_core::{
//!     config::FinstackConfig,
//!     currency::Currency,
//!     market_data::{context::MarketContext, scalars::MarketScalar},
//!     money::Money,
//! };
//! use finstack_quant_valuations::instruments::{equity::spot::Equity, Instrument};
//! use std::sync::Arc;
//! use time::macros::date;
//!
//! # fn main() -> Result<(), Box<dyn std::error::Error>> {
//! let instrument: Arc<dyn Instrument> = Arc::new(
//!     Equity::new("AAPL", "AAPL", Currency::USD)
//!         .with_spot_id("AAPL-SPOT")
//!         .with_quantity(100.0),
//! );
//! let market_t0 = MarketContext::new().insert_price(
//!     "AAPL-SPOT",
//!     MarketScalar::price(Money::from((180_i64, Currency::USD))),
//! );
//! let market_t1 = MarketContext::new().insert_price(
//!     "AAPL-SPOT",
//!     MarketScalar::price(Money::from((185_i64, Currency::USD))),
//! );
//!
//! let config = FinstackConfig::default();
//! let request = AttributionRequest::new(
//!     &instrument,
//!     &market_t0,
//!     &market_t1,
//!     date!(2025 - 01 - 15),
//!     date!(2025 - 01 - 16),
//!     &config,
//! );
//! let attribution = attribute_pnl(&AttributionMethod::Parallel, &request)?;
//!
//! assert_eq!(attribution.total_pnl, Money::from((500_i64, Currency::USD)));
//! assert_eq!(
//!     attribution.market_scalars_pnl,
//!     Money::from((500_i64, Currency::USD)),
//! );
//! # Ok(())
//! # }
//! ```
//!
//! # References
//!
//! - Fixed-income sensitivity intuition: `docs/REFERENCES.md#tuckman-serrat-fixed-income`
//! - Risk decomposition: `docs/REFERENCES.md#meucci-risk-and-asset-allocation`

mod api;
pub(crate) mod credit_cascade;
pub(crate) mod credit_decomposition;
mod detail;
pub(crate) mod execution;
pub(crate) mod factors;
pub mod long_rows;
pub(crate) mod metrics_based;
pub(crate) mod model_params;
pub(crate) mod parallel;
pub(crate) mod pnl_support;
mod result;
pub(crate) mod return_contribution;
#[cfg(feature = "json-schema")]
pub mod schema;
pub(crate) mod spec;
pub(crate) mod target_currency;
pub(crate) mod taylor;
pub(crate) mod waterfall;

pub use credit_cascade::CreditFactorDetailOptions;
pub use detail::{
    CarryDetail, CorrelationsAttribution, CreditCarryByLevel, CreditCarryDecomposition,
    CreditCurvesAttribution, CreditFactorAttribution, CrossFactorDetail, FxAttribution,
    InflationCurvesAttribution, LevelCarry, LevelPnl, ModelParamsAttribution, MoveUnit,
    RatesCurvesAttribution, ScalarsAttribution, SensitivityBucket, SensitivityStep, SourceLine,
    VolAttribution, WaterfallStep,
};
pub use result::{
    AttributionFactor, AttributionMeta, AttributionMethod, ExecutionPolicy, PnlAttribution,
    PnlBucket, PnlBucketShare,
};

pub use api::{AttributionRequest, attribute_pnl, pnl_bridge};
/// Snapshot/restore primitives used by benches and integration tests.
#[doc(hidden)]
pub use factors::{MarketRestoreFlags, MarketSnapshot};
pub use metrics_based::engine::attribute_pnl_metrics_based;
pub use model_params::ModelParamsSnapshot;
pub use return_contribution::{
    BenchmarkRelativeContribution, FactorContribution, GroupContribution, InstrumentContribution,
    ReturnContributionFactor, ReturnContributionPosition, ReturnContributionResult,
    ReturnContributionSpec, ReturnContributionWeighting, attribute_return_contribution,
    attribute_return_contribution_json, validate_return_contribution_json,
};
pub use spec::{
    AttributionConfig, AttributionEnvelope, AttributionInputs, AttributionResult,
    AttributionResultEnvelope, AttributionSchema, AttributionSpec, attribute_pnl_many,
    default_attribution_metrics, validate_attribution_json,
};
pub use target_currency::translate_to_target_currency;
pub use taylor::engine::TaylorAttributionConfig;
pub use waterfall::default_waterfall_order;

/// Compiles the crate `README.md` Rust samples as doctests.
///
/// The README is *not* included in the rendered crate documentation — this
/// item exists only under `cfg(doctest)` so that every ` ```rust ` block in the
/// README is compiled and run by `cargo test --doc`. Without it those samples
/// are dead text and rot silently on any API change.
#[cfg(doctest)]
#[doc = include_str!("../README.md")]
struct ReadmeDoctests;
