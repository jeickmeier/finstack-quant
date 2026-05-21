//! Multi-period P&L attribution for financial instruments.
//!
//! This module provides comprehensive P&L attribution capabilities to decompose
//! daily MTM changes into constituent factors: carry, curve shifts, credit spreads,
//! FX, volatility, model parameters, and market scalars.
//!
//! # Overview
//!
//! P&L attribution answers the question: "Why did my position's value change from
//! T₀ to T₁?" by isolating the impact of each market factor and model parameter.
//!
//! # Choosing a methodology
//!
//! This module is intentionally layered by cost and fidelity. **Start at the top
//! tier** and only move down when you actually need the extra information — the
//! heavier methodologies are substantially more expensive and introduce more
//! moving parts in production pipelines.
//!
//! | Tier         | Entry point                                                      | Use when                                    |
//! |--------------|------------------------------------------------------------------|---------------------------------------------|
//! | Minimal      | [`crate::attribution::simple_pnl_bridge`]                        | You just want total P&L, no decomposition   |
//! | Linear       | [`crate::attribution::attribute_pnl_metrics_based`]              | Fast daily attribution for small moves      |
//! | Parallel     | [`crate::attribution::attribute_pnl_parallel`]                   | Factor isolation with a residual line       |
//! | Waterfall    | [`crate::attribution::attribute_pnl_waterfall`]                  | Sum-preserving, path-ordered decomposition  |
//! | Taylor       | [`crate::attribution::attribute_pnl_taylor`]                     | Second-order sensitivity-based breakdown    |
//!
//! The simple bridge is a single function (~30 LOC). The linear path uses
//! pre-computed metrics (DV01, theta, etc.) and is the right default for most
//! daily batch jobs. Parallel/waterfall/taylor are **opt-in** advanced paths
//! reserved for scenarios where factor attribution genuinely drives a business
//! decision; they all involve non-trivial per-factor repricing and should be
//! benchmarked before being wired into hot paths.
//!
//! # Documentation Rules For Attribution APIs
//!
//! Attribution docs should state:
//!
//! - whether a contribution is exact, path-dependent, or an approximation
//! - what units and sign conventions are used for input metrics and output P&L terms
//! - whether curve, spread, vol, or scalar moves are parallel, bucketed, or model-specific
//! - how residual should be interpreted and when it is expected to be large
//!
//! # Methodologies
//!
//! Four attribution methodologies are supported:
//!
//! - **Parallel Attribution**: Independent factor isolation. Each factor is analyzed
//!   in isolation by restoring T₀ values while keeping all other factors at T₁.
//!   Residual captures cross-effects and non-linearities.
//!
//! - **Waterfall Attribution**: Sequential factor application. Factors are applied
//!   one-by-one in a specified order, with each factor's P&L computed after applying
//!   all previous factors. Guarantees sum = total P&L (minimal residual by construction).
//!
//! - **Metrics-Based Attribution**: Linear approximation using existing metrics
//!   (Theta, DV01, CS01, etc.). Fast but less accurate for large market moves.
//!
//! - **Taylor Attribution**: Sensitivity-based Taylor expansion. Computes first-order
//!   (and optionally second-order) sensitivities at T₀ via bump-and-reprice, then
//!   multiplies by observed market moves. Complements the waterfall approach with
//!   an independent, factor-additive decomposition.
//!
//! # Attribution Factors
//!
//! The following factors are supported:
//!
//! - **Carry**: Time decay (theta) and accruals
//! - **RatesCurves**: Discount and forward curve shifts (IR risk)
//! - **CreditCurves**: Hazard curve shifts (credit spread risk)
//! - **InflationCurves**: Inflation curve shifts
//! - **Correlations**: Base correlation curve changes (structured credit)
//! - **Fx**: FX rate changes
//! - **Volatility**: Implied volatility changes
//! - **ModelParameters**: Model-specific parameters (prepayment, default, recovery, conversion)
//! - **MarketScalars**: Dividends, equity/commodity prices, inflation indices
//!
//! # Examples
//!
//! ## Basic Parallel Attribution
//!
//! ```ignore
//! use finstack_valuations::attribution::attribute_pnl_parallel;
//! use finstack_valuations::instruments::Instrument;
//! use finstack_valuations::instruments::rates::deposit::Deposit;
//! use finstack_core::config::FinstackConfig;
//! use finstack_core::currency::Currency;
//! use finstack_core::market_data::context::MarketContext;
//! use finstack_core::money::Money;
//! use std::sync::Arc;
//! use time::macros::date;
//!
//! # fn main() -> Result<(), Box<dyn std::error::Error>> {
//! let as_of_t0 = date!(2025-01-15);
//! let as_of_t1 = date!(2025-01-16);
//! let market_t0 = MarketContext::new();
//! let market_t1 = MarketContext::new();
//! let config = FinstackConfig::default();
//!
//! let instrument = Arc::new(
//!     Deposit::builder()
//!         .id("DEP-1D".into())
//!         .notional(Money::new(1_000_000.0, Currency::USD))
//!         .start_date(as_of_t0)
//!         .maturity(as_of_t1)
//!         .day_count(finstack_core::dates::DayCount::Act360)
//!         .discount_curve_id("USD-OIS".into())
//!         .build()
//!         .expect("deposit builder should succeed"),
//! ) as Arc<dyn Instrument>;
//!
//! let attribution = attribute_pnl_parallel(
//!     &instrument,
//!     &market_t0,
//!     &market_t1,
//!     as_of_t0,
//!     as_of_t1,
//!     &config,
//!     None,
//! )?;
//!
//! println!("Total P&L: {}", attribution.total_pnl);
//! println!("Carry: {}", attribution.carry);
//! println!("Rates: {}", attribution.rates_curves_pnl);
//! println!("Residual: {} ({:.2}%)",
//!     attribution.residual,
//!     attribution.meta.residual_pct
//! );
//! # Ok(())
//! # }
//! ```
//!
//! ## Waterfall Attribution
//!
//! ```ignore
//! use finstack_valuations::attribution::{
//!     attribute_pnl_waterfall, AttributionFactor
//! };
//! use finstack_valuations::instruments::Instrument;
//! use finstack_valuations::instruments::rates::deposit::Deposit;
//! use finstack_core::config::FinstackConfig;
//! use finstack_core::currency::Currency;
//! use finstack_core::market_data::context::MarketContext;
//! use finstack_core::money::Money;
//! use std::sync::Arc;
//! use time::macros::date;
//!
//! # fn main() -> Result<(), Box<dyn std::error::Error>> {
//! let factor_order = vec![
//!     AttributionFactor::Carry,
//!     AttributionFactor::RatesCurves,
//!     AttributionFactor::CreditCurves,
//!     AttributionFactor::Fx,
//! ];
//!
//! let as_of_t0 = date!(2025-01-15);
//! let as_of_t1 = date!(2025-01-16);
//! let market_t0 = MarketContext::new();
//! let market_t1 = MarketContext::new();
//! let config = FinstackConfig::default();
//! let instrument = Arc::new(
//!     Deposit::builder()
//!         .id("DEP-1D".into())
//!         .notional(Money::new(1_000_000.0, Currency::USD))
//!         .start_date(as_of_t0)
//!         .maturity(as_of_t1)
//!         .day_count(finstack_core::dates::DayCount::Act360)
//!         .discount_curve_id("USD-OIS".into())
//!         .build()
//!         .expect("deposit builder should succeed"),
//! ) as Arc<dyn Instrument>;
//!
//! let attribution = attribute_pnl_waterfall(
//!     &instrument,
//!     &market_t0,
//!     &market_t1,
//!     as_of_t0,
//!     as_of_t1,
//!     &config,
//!     factor_order,
//!     false, // strict validation
//!     None,
//! )?;
//!
//! // Residual should be minimal (< 0.01%)
//! assert!(attribution.residual_within_tolerance(0.01, 1.0));
//! # Ok(())
//! # }
//! ```
//!
//! ## Metrics-Based Attribution
//!
//! ```ignore
//! use finstack_valuations::attribution::attribute_pnl_metrics_based;
//! use finstack_valuations::attribution::default_attribution_metrics;
//! use finstack_valuations::instruments::Instrument;
//! use finstack_valuations::instruments::PricingOptions;
//! use finstack_valuations::instruments::rates::deposit::Deposit;
//! use finstack_core::currency::Currency;
//! use finstack_core::market_data::context::MarketContext;
//! use finstack_core::money::Money;
//! use std::sync::Arc;
//! use time::macros::date;
//!
//! // Requires pre-computed valuations with metrics
//! # fn main() -> Result<(), Box<dyn std::error::Error>> {
//! let as_of_t0 = date!(2025-01-15);
//! let as_of_t1 = date!(2025-01-16);
//! let market_t0 = MarketContext::new();
//! let market_t1 = MarketContext::new();
//! let metrics = default_attribution_metrics();
//!
//! let instrument = Arc::new(
//!     Deposit::builder()
//!         .id("DEP-1D".into())
//!         .notional(Money::new(1_000_000.0, Currency::USD))
//!         .start_date(as_of_t0)
//!         .maturity(as_of_t1)
//!         .day_count(finstack_core::dates::DayCount::Act360)
//!         .discount_curve_id("USD-OIS".into())
//!         .build()
//!         .expect("deposit builder should succeed"),
//! ) as Arc<dyn Instrument>;
//!
//! let val_t0 = instrument.price_with_metrics(&market_t0, as_of_t0, &metrics, PricingOptions::default())?;
//! let val_t1 = instrument.price_with_metrics(&market_t1, as_of_t1, &metrics, PricingOptions::default())?;
//!
//! let attribution = attribute_pnl_metrics_based(
//!     &instrument,
//!     &market_t0,
//!     &market_t1,
//!     &val_t0,
//!     &val_t1,
//!     as_of_t0,
//!     as_of_t1,
//! )?;
//! # let _ = attribution;
//! # Ok(())
//! # }
//! ```
//!
//! # Per-Tenor Curve Attribution
//!
//! ```ignore
//! use finstack_valuations::attribution::attribute_pnl_parallel;
//! use finstack_valuations::instruments::Instrument;
//! use finstack_valuations::instruments::rates::deposit::Deposit;
//! use finstack_core::config::FinstackConfig;
//! use finstack_core::currency::Currency;
//! use finstack_core::market_data::context::MarketContext;
//! use finstack_core::money::Money;
//! use std::sync::Arc;
//! use time::macros::date;
//!
//! # fn main() -> Result<(), Box<dyn std::error::Error>> {
//! let as_of_t0 = date!(2025-01-15);
//! let as_of_t1 = date!(2025-01-16);
//! let market_t0 = MarketContext::new();
//! let market_t1 = MarketContext::new();
//! let config = FinstackConfig::default();
//!
//! let instrument = Arc::new(
//!     Deposit::builder()
//!         .id("DEP-1D".into())
//!         .notional(Money::new(1_000_000.0, Currency::USD))
//!         .start_date(as_of_t0)
//!         .maturity(as_of_t1)
//!         .day_count(finstack_core::dates::DayCount::Act360)
//!         .discount_curve_id("USD-OIS".into())
//!         .build()
//!         .expect("deposit builder should succeed"),
//! ) as Arc<dyn Instrument>;
//!
//! let attribution = attribute_pnl_parallel(
//!     &instrument,
//!     &market_t0,
//!     &market_t1,
//!     as_of_t0,
//!     as_of_t1,
//!     &config,
//!     None,
//! )?;
//!
//! if let Some(rates_detail) = &attribution.rates_detail {
//!     for ((curve_id, tenor), pnl) in &rates_detail.by_tenor {
//!         println!("{} {}: {}", curve_id, tenor, pnl);
//!     }
//! }
//! # Ok(())
//! # }
//! ```
//!
//! # References
//!
//! - Fixed-income sensitivity intuition: `docs/REFERENCES.md#tuckman-serrat-fixed-income`
//! - Risk decomposition and factor attribution: `docs/REFERENCES.md#meucci-risk-and-asset-allocation`

pub(crate) mod credit_cascade;
pub(crate) mod credit_decomposition;
pub(crate) mod credit_factor;
pub(crate) mod execution;
pub(crate) mod factors;
pub(crate) mod helpers;
pub(crate) mod metrics_based;
pub(crate) mod model_params;
pub(crate) mod parallel;
pub(crate) mod spec;
pub mod taylor;
pub(crate) mod types;
pub(crate) mod waterfall;

// Re-export core types
pub use credit_factor::{
    compute_credit_factor_attribution, credit_factor_model_id, CreditAttributionInput,
    CreditFactorDetailOptions, CreditFactorModelRef,
};
pub use types::{
    AttributionFactor, AttributionMeta, AttributionMethod, CarryDetail, CorrelationsAttribution,
    CreditCarryByLevel, CreditCarryDecomposition, CreditCurvesAttribution, CreditFactorAttribution,
    CrossFactorDetail, FxAttribution, InflationCurvesAttribution, LevelCarry, LevelPnl,
    ModelParamsAttribution, PnlAttribution, RatesCurvesAttribution, ScalarsAttribution, SourceLine,
    VolAttribution,
};

// Re-export attribution functions
pub use metrics_based::attribute_pnl_metrics_based;
pub use model_params::{
    extract_model_params, measure_conversion_shift, measure_default_shift,
    measure_prepayment_shift, measure_recovery_shift, try_measure_conversion_shift,
    try_measure_default_shift, try_measure_prepayment_shift, try_measure_recovery_shift,
    with_model_params, ModelParamsSnapshot,
};
pub use parallel::attribute_pnl_parallel;
pub use spec::{
    default_attribution_metrics, AttributionConfig, AttributionEnvelope, AttributionResult,
    AttributionResultEnvelope, AttributionSpec, ATTRIBUTION_SCHEMA_V1,
};
pub use taylor::{
    attribute_pnl_taylor, attribute_pnl_taylor_standard, TaylorAttributionConfig,
    TaylorAttributionResult, TaylorFactorResult,
};
pub use waterfall::{attribute_pnl_waterfall, default_waterfall_order};
// Market snapshot helpers
pub use factors::{
    CurveRestoreFlags, MarketRestoreFlags, MarketSnapshot, ScalarsSnapshot, VolatilitySnapshot,
};
pub use helpers::{compute_pnl, compute_pnl_with_fx, reprice_instrument};

use crate::instruments::common_impl::traits::Instrument;
use finstack_core::currency::Currency;
use finstack_core::dates::Date;
use finstack_core::market_data::context::MarketContext;
use finstack_core::money::Money;
use std::sync::Arc;

/// Minimal, no-frills P&L bridge: `value(T₁) − value(T₀)`.
///
/// This is the **cheapest** attribution entry point — it prices the
/// instrument once at each date in each market state and returns the
/// scalar total P&L in `target_ccy` (FX-converted on the way out). Use
/// it when you just need the headline number and don't care which
/// factors contributed. For a factor-level decomposition, reach for
/// one of the `attribute_pnl_*` functions listed in the module docs.
///
/// This is intentionally a thin wrapper over
/// [`reprice_instrument`] + [`compute_pnl_with_fx`]: the function is
/// cheap, it allocates no scratch buffers, and it contains no factor
/// iteration. Benchmark the heavier methodologies against this
/// baseline to quantify the cost of factor attribution.
///
/// # Arguments
///
/// * `instrument` - Instrument to price at both dates.
/// * `market_t0`, `market_t1` - Market states at T₀ and T₁.
/// * `as_of_t0`, `as_of_t1` - Valuation dates.
/// * `target_ccy` - Currency to report P&L in; FX is resolved through
///   `market_t1`.
///
/// # Returns
///
/// The total P&L `v_t1 − v_t0` in `target_ccy`.
///
/// # Errors
///
/// Returns an error if either repricing call fails or if the FX
/// conversion cannot be resolved from `market_t1`.
///
/// # Examples
///
/// ```ignore
/// use finstack_valuations::attribution::simple_pnl_bridge;
/// use finstack_valuations::instruments::Instrument;
/// use finstack_core::currency::Currency;
/// use finstack_core::market_data::context::MarketContext;
/// use std::sync::Arc;
/// use time::macros::date;
///
/// # fn main() -> Result<(), Box<dyn std::error::Error>> {
/// # let instrument: Arc<dyn Instrument> = unimplemented!("obtain the instrument under test");
/// let market_t0 = MarketContext::new();
/// let market_t1 = MarketContext::new();
///
/// let pnl = simple_pnl_bridge(
///     &instrument,
///     &market_t0,
///     &market_t1,
///     date!(2025 - 01 - 15),
///     date!(2025 - 01 - 16),
///     Currency::USD,
/// )?;
/// println!("Daily P&L: {pnl}");
/// # Ok(())
/// # }
/// ```
pub fn simple_pnl_bridge(
    instrument: &Arc<dyn Instrument>,
    market_t0: &MarketContext,
    market_t1: &MarketContext,
    as_of_t0: Date,
    as_of_t1: Date,
    target_ccy: Currency,
) -> finstack_core::Result<Money> {
    let v_t0 = reprice_instrument(instrument, market_t0, as_of_t0)?;
    let v_t1 = reprice_instrument(instrument, market_t1, as_of_t1)?;
    compute_pnl_with_fx(
        v_t0, v_t1, target_ccy, market_t0, market_t1, as_of_t0, as_of_t1,
    )
}
