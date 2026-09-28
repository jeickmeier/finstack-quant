//! Generic cross-asset composite and synthetic instruments.
//!
//! A composite stores self-contained
//! [`InstrumentJson`] legs and an
//! immutable resolved-quantity state. Pricing, risk, scenarios, and history
//! use those quantities exactly as stored. Only
//! [`CompositeSpec::initialize`], [`CompositeSpec::initialize_fixed`], or
//! [`CompositeInstrument::rebalance`] may calculate a new state.
//!
//! # Invariants
//!
//! - At least two legs; identifiers are unique within one composite.
//! - The same instrument identifier reused anywhere in the tree must carry an
//!   identical definition (content hash).
//! - Nesting depth is at most 8 (`MAX_COMPOSITE_DEPTH`); the full tree may
//!   contain at most 64 (`MAX_COMPOSITE_LEGS`) nodes.
//! - `capital` is a positive amount in `reporting_currency` and is the return
//!   denominator for one composite unit.
//! - Every resolved quantity is finite and non-zero (`|q| > 1e-12`).
//!
//! # Weighting
//!
//! [`WeightingMethod`] converts each leg's signed `score` into a quantity only
//! at initialization or rebalance:
//!
//! - [`WeightingMethod::FixedQuantity`]: `quantity = score`.
//! - [`WeightingMethod::NotionalWeighted`]: allocate a target gross
//!   reporting-currency notional by absolute score, then divide by each
//!   leg's unit notional.
//! - [`WeightingMethod::MetricWeighted`]: scale so each leg's metric
//!   contribution tracks its (optionally neutralized) score relative to
//!   an anchor quantity.
//! - [`WeightingMethod::VolatilityWeighted`]: inverse annualized unit-P&L
//!   volatility versus an anchor quantity.
//! - [`WeightingMethod::UserDefined`]: one scalar expression per leg,
//!   evaluated against reporting-currency value, FX, optional notional,
//!   required metrics, and optional unit-P&L volatility.
//!
//! # History
//!
//! [`history()`](crate::instruments::composite::history) and [`history_from_spec`](crate::instruments::composite::history_from_spec) walk strictly increasing dated market snapshots.
//! Warmup observations are visible to dynamic weighting but are not emitted.
//! Each output row values the state held *into* that close. A scheduled
//! rebalance is close-effective: the row still reports pre-trade P&L, then the
//! next interval opens at the post-trade value. The principal change is
//! external financing, not investment P&L.
//!
//! Period return is `pnl / capital`. The chained `return_index` starts at
//! `100` on the first output row (`pnl`, cashflows, and period return are
//! then zero).
//!
//! # Conventions
//!
//! - Value, additive risk, cashflows, P&L, and capital are reported in
//!   `reporting_currency`; FX conversion uses the market's required FX matrix
//!   on the valuation or cashflow date.
//! - Primitive exposure reports accept only [additive metrics](crate::metrics::is_additive_metric).
//!   Yield, duration, implied volatility, and other non-linear measures are
//!   rejected at the aggregate surface.
//! - Composite instruments do not emit their own contractual cashflows; history
//!   cashflows are the quantity-scaled primitive cashflows on `(start, end]`.
//!
//! # Examples
//!
//! ```
//! use finstack_quant_core::market_data::context::MarketContext;
//! use finstack_quant_valuations::instruments::{CompositeInstrument, Instrument};
//! use time::macros::date;
//!
//! let composite = CompositeInstrument::example()?;
//! assert_eq!(composite.id(), "COMPOSITE-EXAMPLE");
//!
//! let value = composite.value(&MarketContext::new(), date!(2025 - 01 - 02))?;
//! assert!((value.amount() - 10.0).abs() < 1.0e-9);
//!
//! let primitives = composite.flatten_primitives()?;
//! assert_eq!(primitives.len(), 2);
//! # Ok::<(), finstack_quant_core::Error>(())
//! ```
//!
//! # See Also
//!
//! - [`CompositeSpec`] for the unresolved definition and weighting policy
//! - [`CompositeInstrument`] for the priceable resolved state
//! - [`history()`](crate::instruments::composite::history) / [`history_from_spec`](crate::instruments::composite::history_from_spec) for dated total-return and rebalance history
//! - [`crate::instruments::Instrument`] for generic pricing and metric dispatch
//!
//! # References
//!
//! - DV01-neutral curve trades and duration scaling:
//!   `docs/REFERENCES.md#tuckman-serrat-fixed-income`

mod history;
mod types;

pub use history::{history, history_from_spec, CompositeHistoryRow};
pub use types::{
    CompositeExposureReport, CompositeInstrument, CompositeLegSpec, CompositeLegValuation,
    CompositeMarketObservation, CompositeRebalanceResult, CompositeSpec, CompositeState,
    CompositeTrade, CompositeValuationDetails, PrimitiveAggregate, PrimitiveExposure,
    RebalanceRule, ResolvedCompositeLeg, WeightingMethod, MAX_COMPOSITE_DEPTH, MAX_COMPOSITE_LEGS,
};
