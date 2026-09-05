//! Engine-internal effect enum produced by adapter functions.
//!
//! Adapter modules translate an [`crate::spec::OperationSpec`] into a
//! [`Vec<ScenarioEffect>`]. [`crate::engine::ScenarioEngine::apply`] then
//! applies those effects to the mutable [`crate::engine::ExecutionContext`].

use crate::spec::RateBindingSpec;
use crate::warning::Warning;
use finstack_quant_core::market_data::bumps::MarketBump;
use finstack_quant_statements::types::NodeId;

/// Outcome of a scenario operation, collected before mutation.
#[derive(Debug)]
pub(crate) enum ScenarioEffect {
    /// Market-data bump applied to the context.
    MarketBump(MarketBump),
    /// Structured warning recorded on the application report.
    Warning(Warning),
    /// Replace a curve in the market (discount, forward, hazard, inflation, or vol-index).
    UpdateCurve(finstack_quant_core::market_data::context::CurveStorage),
    /// Percentage forecast adjustment on a statement node.
    StmtForecastPercent {
        /// Statement node identifier.
        node_id: NodeId,
        /// Percentage change (`-10.0` reduces forecasts by 10%).
        pct: f64,
    },
    /// Absolute forecast assignment on a statement node.
    StmtForecastAssign {
        /// Statement node identifier.
        node_id: NodeId,
        /// Scalar replacing selected forecasts, in the node's units.
        value: f64,
    },
    /// Statement rate binding to apply after market shocks.
    RateBinding {
        /// Binding specification to apply.
        binding: RateBindingSpec,
    },
    /// Price shock routed to matching instruments.
    InstrumentPriceShock {
        /// Type filter, when present.
        types: Option<Vec<finstack_quant_valuations::pricer::InstrumentType>>,
        /// Attribute filter, when present.
        attrs: Option<indexmap::IndexMap<String, String>>,
        /// Percentage price shock.
        pct: f64,
    },
    /// Spread shock routed to matching instruments.
    InstrumentSpreadShock {
        /// Type filter, when present.
        types: Option<Vec<finstack_quant_valuations::pricer::InstrumentType>>,
        /// Attribute filter, when present.
        attrs: Option<indexmap::IndexMap<String, String>>,
        /// Spread shock in basis points.
        bp: f64,
    },
    /// Asset-correlation shock on structured-credit instruments.
    AssetCorrelationShock {
        /// Additive shock in correlation points.
        delta_pts: f64,
    },
    /// Prepay-default correlation shock on structured-credit instruments.
    PrepayDefaultCorrelationShock {
        /// Additive shock in correlation points.
        delta_pts: f64,
    },
}
