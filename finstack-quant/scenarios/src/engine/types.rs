//! Execution context and scenario application report types.

use crate::spec::{CurveKind, HazardBumpMode, RateBindingSpec};
use crate::warning::Warning;
use finstack_quant_core::currency::Currency;
use finstack_quant_core::types::{CurveId, PriceId};
use finstack_quant_statements::types::NodeId;
use finstack_quant_statements::FinancialModelSpec;
use finstack_quant_valuations::instruments::{Instrument, InstrumentEnvelope};
use finstack_quant_valuations::recalibration::RecalibrationProvider;
use indexmap::IndexMap;

/// Execution context for scenario application.
///
/// Pins the mutable state a scenario can touch — market data, optional
/// statement models, instruments, and rate bindings — together with the
/// valuation date.
///
/// # Examples
/// ```
/// use finstack_quant_scenarios::ExecutionContext;
/// use finstack_quant_core::market_data::context::MarketContext;
/// use finstack_quant_statements::FinancialModelSpec;
/// use time::macros::date;
///
/// let mut market = MarketContext::new();
/// let mut model = FinancialModelSpec::new("demo", vec![]);
/// let as_of = date!(2025 - 01 - 01);
/// let ctx = ExecutionContext {
///     market: &mut market,
///     model: Some(&mut model),
///     instruments: None,
///     rate_bindings: None,
///     calendar: None,
///     as_of,
/// };
///
/// assert_eq!(ctx.as_of, as_of);
/// ```
pub struct ExecutionContext<'a> {
    /// Market data context (curves, surfaces, FX, etc.).
    pub market: &'a mut finstack_quant_core::market_data::context::MarketContext,

    /// Optional financial statements model.
    ///
    /// Statement forecast operations and rate bindings require this to be
    /// `Some`; market-only scenarios can pass `None`.
    pub model: Option<&'a mut FinancialModelSpec>,

    /// Optional vector of instruments for price/spread shocks and carry calculations.
    pub instruments: Option<&'a mut Vec<Box<dyn Instrument>>>,

    /// Optional mapping from statement node IDs to binding specs for automatic rate updates.
    pub rate_bindings: Option<IndexMap<NodeId, RateBindingSpec>>,

    /// Optional holiday calendar for calendar-aware tenor calculations.
    pub calendar: Option<&'a dyn finstack_quant_core::dates::HolidayCalendar>,

    /// Valuation date operations reference.
    pub as_of: time::Date,
}

/// Read-only ParCDS delivery settings threaded through effect generation.
pub(crate) struct HazardApplyEnv<'a> {
    /// Solve-to-par versus first-order hazard-knot delivery.
    pub mode: HazardBumpMode,
    /// Quote-recalibration service shared by this immutable scenario batch.
    pub provider: &'a dyn RecalibrationProvider,
    /// Dependency snapshots against which each current hazard recipe was calibrated.
    pub source_markets: Option<
        &'a IndexMap<
            CurveId,
            std::sync::Arc<finstack_quant_core::market_data::context::MarketContext>,
        >,
    >,
}

/// A concrete market-data target changed while applying a scenario.
///
/// Targets are recorded from hierarchy-expanded operations together with the
/// effects that were actually accepted by the engine. Consequently, every
/// identifier is a resolved market identifier rather than an unresolved
/// hierarchy path or a best-effort reconstruction of the original spec.
#[derive(Debug, Clone, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
#[cfg_attr(feature = "json-schema", derive(schemars::JsonSchema))]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum ScenarioMarketTarget {
    /// A discount, forward, credit, inflation, or commodity curve.
    Curve {
        /// Curve family used to select the market-data collection.
        curve_kind: CurveKind,
        /// Concrete identifier changed in that collection.
        curve_id: CurveId,
    },
    /// A volatility-index curve.
    VolatilityIndex {
        /// Concrete volatility-index curve identifier.
        curve_id: CurveId,
    },
    /// A base-correlation surface.
    BaseCorrelation {
        /// Concrete base-correlation surface identifier.
        surface_id: CurveId,
    },
    /// A volatility surface.
    VolSurface {
        /// Concrete volatility-surface identifier.
        vol_surface_id: CurveId,
    },
    /// An equity or other scalar price entry.
    EquityPrice {
        /// Concrete `MarketContext::get_price` scalar identifier.
        spot_id: PriceId,
    },
    /// A directed FX pair.
    Fx {
        /// Base currency strengthened or weakened by the operation.
        base: Currency,
        /// Quote currency used for the shocked rate.
        quote: Currency,
    },
}

/// Authoritative change manifest produced while applying a scenario.
///
/// Readers use the manifest to invalidate only the market factors and
/// instruments that actually changed, while `all_dirty` provides a
/// conservative escape hatch for changes that cannot be represented precisely.
#[derive(Debug, Clone, Default, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[cfg_attr(feature = "json-schema", derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields)]
pub struct ScenarioChangeManifest {
    /// Concrete market-data targets changed by applied effects.
    pub market_targets: Vec<ScenarioMarketTarget>,
    /// Zero-based indices of portfolio instruments mutated in place.
    #[serde(with = "finstack_quant_core::wire::counts")]
    #[cfg_attr(feature = "json-schema", schemars(with = "Vec<u32>"))]
    pub changed_instrument_indices: Vec<usize>,
    /// Whether the execution context's effective valuation date changed.
    pub as_of_changed: bool,
    /// Whether instruments were inserted, removed, or reordered.
    ///
    /// Scenario operations do not change portfolio shape, so this stays `false`.
    pub portfolio_shape_changed: bool,
    /// Whether callers must conservatively treat every dependency as dirty.
    ///
    /// This is set for effective time rolls because date-sensitive values can
    /// change even when no explicit market or instrument target was mutated.
    pub all_dirty: bool,
}

impl ScenarioMarketTarget {
    /// Display identifier of the changed market object.
    ///
    /// # Returns
    ///
    /// The curve, surface or price identifier, or `BASE/QUOTE` for an FX pair.
    #[must_use]
    pub fn id_label(&self) -> String {
        match self {
            Self::Curve { curve_id, .. } | Self::VolatilityIndex { curve_id } => {
                curve_id.as_str().to_string()
            }
            Self::BaseCorrelation { surface_id } => surface_id.as_str().to_string(),
            Self::VolSurface { vol_surface_id } => vol_surface_id.as_str().to_string(),
            Self::EquityPrice { spot_id } => spot_id.as_str().to_string(),
            Self::Fx { base, quote } => format!("{base}/{quote}"),
        }
    }
}

impl ScenarioChangeManifest {
    pub(super) fn record_market_target(&mut self, target: ScenarioMarketTarget) {
        if !self.market_targets.contains(&target) {
            self.market_targets.push(target);
        }
    }

    pub(super) fn record_instrument_indices(&mut self, indices: impl IntoIterator<Item = usize>) {
        for index in indices {
            if let Err(position) = self.changed_instrument_indices.binary_search(&index) {
                self.changed_instrument_indices.insert(position, index);
            }
        }
    }
}

/// Unit in which an applied shock's size is expressed.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
#[cfg_attr(feature = "json-schema", derive(schemars::JsonSchema))]
#[serde(rename_all = "snake_case")]
pub enum ShockUnit {
    /// Additive basis points of a rate or spread (`1.0` = 0.0001 in decimal
    /// rate space).
    Bp,
    /// Relative change in percent of the current level (`5.0` = +5%).
    Percent,
    /// Additive change in the target's own quote units: volatility-index
    /// points for a volatility-index curve, decimal correlation for base
    /// correlation and structured-credit correlations (`0.02` = +0.02).
    Absolute,
}

/// One requested curve-node shock, as written on the operation.
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
#[cfg_attr(feature = "json-schema", derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields)]
pub struct ShockNode {
    /// Tenor label of the node as supplied on the operation (for example `"5Y"`).
    pub tenor: String,
    /// Shock size requested at that tenor, in the enclosing
    /// [`ShockMagnitude::Nodes`] unit.
    pub value: f64,
}

/// Size and shape of one applied shock.
///
/// Sizes are the shock as requested on the (hierarchy-expanded) operation, in
/// that operation's own quote space, with the resolved location where the
/// engine produced one. Delivery adjustments that change how the request
/// reaches the market object — interpolation splits across neighbouring
/// knots, solve-to-par recalibration, first-order hazard shifts, clamping —
/// are reported in [`ApplicationReport::warnings`], not here.
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
#[cfg_attr(feature = "json-schema", derive(schemars::JsonSchema))]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum ShockMagnitude {
    /// One size applied to the whole target: a parallel curve or surface
    /// shift, or the shock to a scalar price, an FX rate or a set of
    /// instruments.
    Uniform {
        /// Shock size in `unit`.
        value: f64,
        /// Unit of `value`.
        unit: ShockUnit,
    },
    /// Triangular key-rate bump centred on one curve knot, emitted once per
    /// resolved knot of a discount or inflation curve node shock.
    KeyRate {
        /// Knot time the bump is centred on, in years on the curve's own time
        /// axis.
        time_years: f64,
        /// Bump size delivered at that knot, in `unit`. For an off-knot tenor
        /// matched by interpolation this is the knot's calibrated share of
        /// the requested shock.
        value: f64,
        /// Unit of `value`.
        unit: ShockUnit,
    },
    /// Node shocks delivered by rebuilding the curve in one step (forward,
    /// par-CDS, commodity and volatility-index node shocks). Lists the nodes
    /// as requested; the rebuilt curve is not echoed.
    Nodes {
        /// Requested `(tenor, size)` nodes in operation order.
        nodes: Vec<ShockNode>,
        /// Unit of every node `value`.
        unit: ShockUnit,
    },
    /// Volatility-surface bucket shock.
    VolBucket {
        /// Surface grid expiries the shock was restricted to, in years, after
        /// snapping the requested tenors to the grid. Absent when every
        /// expiry is shocked.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        expiries_years: Option<Vec<f64>>,
        /// Strikes the shock was restricted to. Absent when every strike is
        /// shocked.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        strikes: Option<Vec<f64>>,
        /// Shock size in `unit`.
        value: f64,
        /// Unit of `value`.
        unit: ShockUnit,
    },
    /// Base-correlation shock restricted to detachment points.
    DetachmentBucket {
        /// Detachment points the shock was restricted to, in basis points
        /// (`300` = 3%), as supplied on the operation.
        detachments_bp: Vec<i32>,
        /// Shock size in `unit`.
        value: f64,
        /// Unit of `value`.
        unit: ShockUnit,
    },
}

/// What one applied shock was applied to.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[cfg_attr(feature = "json-schema", derive(schemars::JsonSchema))]
#[serde(tag = "scope", rename_all = "snake_case", deny_unknown_fields)]
pub enum AppliedShockTarget {
    /// A resolved market-data target; the same value the shock contributed to
    /// [`ScenarioChangeManifest::market_targets`].
    Market {
        /// Concrete market-data target that was shocked.
        target: ScenarioMarketTarget,
    },
    /// Instruments of the supplied inventory mutated in place.
    Instruments {
        /// Zero-based inventory indices of the instruments the shock changed,
        /// ascending.
        #[serde(with = "finstack_quant_core::wire::counts")]
        #[cfg_attr(feature = "json-schema", schemars(with = "Vec<u32>"))]
        indices: Vec<usize>,
    },
}

/// Level of a scalar market value immediately before and after a shock.
#[derive(Debug, Clone, Copy, PartialEq, serde::Serialize, serde::Deserialize)]
#[cfg_attr(feature = "json-schema", derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields)]
pub struct LevelChange {
    /// Stored level before the shock, in the scalar's own units (the amount
    /// for a monetary price).
    pub before: f64,
    /// Stored level after the shock, in the same units.
    pub after: f64,
}

/// One shock the engine applied, in application order.
///
/// [`ScenarioChangeManifest`] says *which* targets changed (deduplicated, for
/// cache invalidation); this says *by how much*. A target shocked by several
/// effects has one entry per effect.
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
#[cfg_attr(feature = "json-schema", derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields)]
pub struct AppliedShock {
    /// Market target or instruments the shock was applied to.
    pub applies_to: AppliedShockTarget,
    /// Size, unit and shape of the shock.
    pub shock: ShockMagnitude,
    /// Stored level before and after the shock.
    ///
    /// Present only for scalar price targets
    /// ([`ScenarioMarketTarget::EquityPrice`]), where both levels are read
    /// from the market context around the mutation. Absent for FX, curves,
    /// surfaces and instruments.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub level_change: Option<LevelChange>,
}

/// Report from time roll-forward operation.
///
/// # Examples
/// ```rust
/// use finstack_quant_scenarios::RollForwardReport;
/// use indexmap::IndexMap;
/// use time::macros::date;
///
/// let report = RollForwardReport {
///     old_date: date!(2025 - 01 - 01),
///     new_date: date!(2025 - 02 - 01),
///     days: 31,
///     instrument_carry: vec![],
///     total_carry: IndexMap::new(),
///     failed_instruments: vec![],
/// };
/// assert_eq!(report.days, 31);
/// ```
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
#[cfg_attr(feature = "json-schema", derive(schemars::JsonSchema))]
pub struct RollForwardReport {
    /// Original as-of date.
    #[cfg_attr(
        feature = "json-schema",
        schemars(with = "finstack_quant_core::wire::DateWire")
    )]
    pub old_date: finstack_quant_core::dates::Date,

    /// New as-of date after roll.
    #[cfg_attr(
        feature = "json-schema",
        schemars(with = "finstack_quant_core::wire::DateWire")
    )]
    pub new_date: finstack_quant_core::dates::Date,

    /// Calendar days between `old_date` and `new_date`.
    ///
    /// Always a calendar-day span, including under
    /// [`TimeRollMode::BusinessDays`](crate::spec::TimeRollMode::BusinessDays):
    /// the target date is business-day adjusted, but the span back to
    /// `old_date` is still calendar days. Downstream ACT/365F annualization
    /// depends on this.
    #[serde(with = "finstack_quant_core::wire::signed_count")]
    #[cfg_attr(feature = "json-schema", schemars(with = "i32"))]
    pub days: i64,

    /// Per-instrument carry accrual (if instruments provided), grouped by currency.
    pub instrument_carry: Vec<(
        String,
        IndexMap<Currency, finstack_quant_core::money::Money>,
    )>,

    /// Total P&L from carry, grouped by currency.
    pub total_carry: IndexMap<Currency, finstack_quant_core::money::Money>,
    /// Instruments whose carry calculation failed but did not abort the roll.
    pub failed_instruments: Vec<(String, String)>,
}

/// Report describing what happened during [`super::ScenarioEngine::apply`].
///
/// # Examples
/// ```rust
/// use finstack_quant_scenarios::engine::ApplicationReport;
///
/// let report = ApplicationReport {
///     operations_applied: 3,
///     user_operations: 1,
///     expanded_operations: 3,
///     changes: Default::default(),
///     applied_shocks: vec![],
///     warnings: vec![],
///     meta: None,
///     time_roll: None,
/// };
///
/// assert_eq!(report.operations_applied, 3);
/// assert_eq!(report.user_operations, 1);
/// assert_eq!(report.expanded_operations, 3);
/// ```
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
#[cfg_attr(feature = "json-schema", derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields)]
pub struct ApplicationReport {
    /// Number of effects successfully applied to the execution context.
    ///
    /// One user-level operation can produce zero, one, or many effects after
    /// hierarchy expansion and target resolution. This low-level effect count
    /// is therefore not an operation-coverage ratio; inspect `changes` and
    /// `warnings` to determine which targets changed or were skipped.
    #[serde(with = "finstack_quant_core::wire::count")]
    #[cfg_attr(feature = "json-schema", schemars(with = "u32"))]
    pub operations_applied: usize,
    /// Number of user-provided `OperationSpec` entries in the scenario
    /// (before hierarchy expansion and deduplication).
    #[serde(with = "finstack_quant_core::wire::count")]
    #[cfg_attr(feature = "json-schema", schemars(with = "u32"))]
    pub user_operations: usize,
    /// Number of direct (non-hierarchy) operations produced after hierarchy
    /// expansion and resolution-mode deduplication. No-match expansion and
    /// deduplication can make this smaller than `user_operations`. Because
    /// `operations_applied` counts effects rather than operations, the two
    /// counters are not directly comparable.
    #[serde(with = "finstack_quant_core::wire::count")]
    #[cfg_attr(feature = "json-schema", schemars(with = "u32"))]
    pub expanded_operations: usize,

    /// Authoritative metadata describing the state changed by applied effects.
    pub changes: ScenarioChangeManifest,

    /// Size of every market and instrument shock applied, in application
    /// order: one entry per accepted effect, so the entries reconcile with
    /// [`changes`](Self::changes) (every market target named there appears
    /// here at least once, and the instrument indices here union to
    /// `changes.changed_instrument_indices`).
    ///
    /// Statement-forecast operations, rate bindings and the time roll are not
    /// listed. Empty when the scenario applied no market or instrument shock,
    /// and on reports deserialized from JSON written before this field
    /// existed.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub applied_shocks: Vec<AppliedShock>,

    /// Structured warnings generated during application (non-fatal).
    pub warnings: Vec<Warning>,

    /// Audit stamp describing the numeric mode, rounding context, and FX
    /// policy under which the scenario was applied.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub meta: Option<finstack_quant_core::config::ResultsMeta>,

    /// Roll-forward report from the Phase 0 `TimeRollForward` operation,
    /// when the scenario contained one. Carries the per-instrument carry
    /// decomposition and the new valuation date; instruments whose valuation
    /// failed during the roll are also surfaced as
    /// [`Warning::TimeRollInstrumentFailed`] entries in `warnings`.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub time_roll: Option<RollForwardReport>,
}

/// Encode an instrument inventory as canonical envelopes, in input order.
///
/// This is the encoding [`ApplicationEnvelope::from_contexts`] uses for its
/// `instruments` field; host bindings call it to return shocked copies without
/// serializing the whole envelope.
///
/// # Arguments
///
/// * `inventory` - Instruments to encode, typically the shocked copies left in
///   [`ExecutionContext::instruments`] after [`super::ScenarioEngine::apply`].
///
/// # Errors
///
/// Returns a serialization error naming the instrument when an instrument does
/// not support its canonical JSON serializer (custom instruments).
pub fn instrument_envelopes(
    inventory: &[Box<dyn Instrument>],
) -> serde_json::Result<Vec<InstrumentEnvelope>> {
    inventory
        .iter()
        .map(|instrument| {
            instrument
                .to_instrument_json()
                .map(InstrumentEnvelope::new)
                .ok_or_else(|| {
                    <serde_json::Error as serde::ser::Error>::custom(format!(
                        "Instrument '{}' does not support canonical serialization",
                        instrument.id()
                    ))
                })
        })
        .collect()
}

/// JSON envelope returned after applying a scenario to market data and,
/// optionally, a financial model.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
#[cfg_attr(feature = "json-schema", derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields)]
pub struct ApplicationEnvelope {
    /// Mutated market context.
    pub market: serde_json::Value,
    /// Mutated financial model, when a model was supplied.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub model: Option<serde_json::Value>,
    /// Mutated instruments in input order, encoded as canonical envelopes.
    /// Absent when no inventory was supplied; an empty inventory stays empty.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub instruments: Option<Vec<InstrumentEnvelope>>,
    /// Canonical application counters, change manifest, warnings and policy stamp.
    pub report: ApplicationReport,
}

impl ApplicationEnvelope {
    /// Build an envelope from a report and mutated contexts.
    ///
    /// # Errors
    ///
    /// Returns a serialization error if the market or model cannot be encoded
    /// as JSON.
    ///
    /// # Arguments
    ///
    /// * `report` - Application report whose counters, warnings, and change
    ///   manifest are copied into the envelope.
    /// * `market` - Mutated market context.
    /// * `model` - Optional mutated statement model when present.
    /// * `instruments` - Optional mutated inventory in input order. Every
    ///   instrument must support its canonical JSON serializer; unsupported
    ///   custom instruments return a serialization error.
    pub fn from_contexts(
        report: ApplicationReport,
        market: &finstack_quant_core::market_data::context::MarketContext,
        model: Option<&finstack_quant_statements::FinancialModelSpec>,
        instruments: Option<&[Box<dyn Instrument>]>,
    ) -> serde_json::Result<Self> {
        Ok(Self {
            market: serde_json::to_value(market)?,
            model: model.map(serde_json::to_value).transpose()?,
            instruments: instruments.map(instrument_envelopes).transpose()?,
            report,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    // schema-rejection-test: equity_price target `price_id`
    fn equity_price_target_uses_typed_spot_id() {
        let target = ScenarioMarketTarget::EquityPrice {
            spot_id: PriceId::new("AAPL-SPOT"),
        };
        let json = serde_json::to_value(&target).expect("serialize");
        assert_eq!(
            json,
            serde_json::json!({"kind": "equity_price", "spot_id": "AAPL-SPOT"})
        );
        let retired = serde_json::json!({"kind": "equity_price", "price_id": "AAPL-SPOT"});
        let err = serde_json::from_value::<ScenarioMarketTarget>(retired)
            .expect_err("retired price_id must be rejected");
        assert!(err.to_string().contains("price_id"), "{err}");
    }
}
