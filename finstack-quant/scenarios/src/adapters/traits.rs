//! Engine-internal effect enum produced by adapter functions.
//!
//! Adapter modules expose free functions that translate an [`crate::spec::OperationSpec`]
//! into a [`Vec<ScenarioEffect>`]. The engine's centralized `match` in
//! [`crate::engine::ScenarioEngine::apply`] dispatches each operation variant
//! to the appropriate adapter function and then collapses the resulting
//! effects against the mutable [`crate::engine::ExecutionContext`].
//!
//! This module no longer defines a polymorphic `ScenarioAdapter` trait — the
//! enum dispatch in the engine provides compile-time exhaustiveness over the
//! [`crate::spec::OperationSpec`] variants and avoids the silent "operation not supported"
//! warning the previous trait-based dispatch produced.

use crate::spec::RateBindingSpec;
use crate::warning::Warning;
use finstack_quant_core::market_data::bumps::MarketBump;
use finstack_quant_statements::types::NodeId;

/// Represents the outcome of a scenario operation that can be collected and applied later.
/// This allows the engine to separate the "decision" phase from the "mutation" phase.
#[derive(Debug)]
pub(crate) enum ScenarioEffect {
    /// A market data bump to be applied to the context.
    MarketBump(MarketBump),
    /// A structured warning to be recorded.
    Warning(Warning),

    /// Update a curve in the market (discount, forward, hazard, inflation, or vol-index).
    UpdateCurve(finstack_quant_core::market_data::context::CurveStorage),

    /// Apply a forecast adjustment to the statement model (% change).
    StmtForecastPercent {
        /// The ID of the statement item node.
        node_id: NodeId,
        /// The percentage change to apply.
        pct: f64,
    },
    /// Apply a forecast assignment to the statement model (absolute value).
    StmtForecastAssign {
        /// The ID of the statement item node.
        node_id: NodeId,
        /// The value to assign.
        value: f64,
    },

    /// Update rate binding.
    RateBinding {
        /// Binding specification to apply.
        binding: RateBindingSpec,
    },

    /// Apply a price shock to instruments.
    InstrumentPriceShock {
        /// Filter by instrument types (if present).
        types: Option<Vec<finstack_quant_valuations::pricer::InstrumentType>>,
        /// Filter by attributes (if present).
        attrs: Option<indexmap::IndexMap<String, String>>,
        /// The percentage shock to apply.
        pct: f64,
    },
    /// Apply a spread shock to instruments.
    InstrumentSpreadShock {
        /// Filter by instrument types (if present).
        types: Option<Vec<finstack_quant_valuations::pricer::InstrumentType>>,
        /// Filter by attributes (if present).
        attrs: Option<indexmap::IndexMap<String, String>>,
        /// The spread shock in basis points.
        bp: f64,
    },

    /// Shock asset correlation on StructuredCredit instruments.
    AssetCorrelationShock {
        /// Additive shock in correlation points.
        delta_pts: f64,
    },
    /// Shock prepay-default correlation on StructuredCredit instruments.
    PrepayDefaultCorrelationShock {
        /// Additive shock in correlation points.
        delta_pts: f64,
    },
}
