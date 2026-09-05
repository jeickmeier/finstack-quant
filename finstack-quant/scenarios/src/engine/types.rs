//! Execution context and scenario application report types.

use crate::spec::{CurveKind, HazardBumpMode, RateBindingSpec};
use crate::warning::Warning;
use finstack_quant_core::currency::Currency;
use finstack_quant_core::types::CurveId;
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
    pub provider: Option<&'a dyn RecalibrationProvider>,
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
        /// Concrete scalar-price identifier.
        price_id: CurveId,
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
#[serde(deny_unknown_fields)]
pub struct ScenarioChangeManifest {
    /// Concrete market-data targets changed by applied effects.
    pub market_targets: Vec<ScenarioMarketTarget>,
    /// Zero-based indices of portfolio instruments mutated in place.
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
pub struct RollForwardReport {
    /// Original as-of date.
    pub old_date: finstack_quant_core::dates::Date,

    /// New as-of date after roll.
    pub new_date: finstack_quant_core::dates::Date,

    /// Calendar days between `old_date` and `new_date`.
    ///
    /// Always a calendar-day span, including under
    /// [`TimeRollMode::BusinessDays`](crate::spec::TimeRollMode::BusinessDays):
    /// the target date is business-day adjusted, but the span back to
    /// `old_date` is still calendar days. Downstream ACT/365F annualization
    /// depends on this.
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
#[serde(deny_unknown_fields)]
pub struct ApplicationReport {
    /// Number of effects successfully applied to the execution context.
    ///
    /// One user-level operation can produce zero, one, or many effects after
    /// hierarchy expansion and target resolution. This low-level effect count
    /// is therefore not an operation-coverage ratio; inspect `changes` and
    /// `warnings` to determine which targets changed or were skipped.
    pub operations_applied: usize,
    /// Number of user-provided `OperationSpec` entries in the scenario
    /// (before hierarchy expansion and deduplication).
    pub user_operations: usize,
    /// Number of direct (non-hierarchy) operations produced after hierarchy
    /// expansion and resolution-mode deduplication. No-match expansion and
    /// deduplication can make this smaller than `user_operations`. Because
    /// `operations_applied` counts effects rather than operations, the two
    /// counters are not directly comparable.
    pub expanded_operations: usize,

    /// Authoritative metadata describing the state changed by applied effects.
    pub changes: ScenarioChangeManifest,

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

/// JSON envelope returned after applying a scenario to market data and,
/// optionally, a financial model.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
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
    /// Number of effects successfully applied.
    pub operations_applied: usize,
    /// Number of user-provided operations before expansion.
    pub user_operations: usize,
    /// Number of expanded operations the engine attempted.
    pub expanded_operations: usize,
    /// Authoritative metadata describing the state changed by applied effects.
    pub changes: ScenarioChangeManifest,
    /// Structured warnings produced while applying the scenario.
    pub warnings: Vec<Warning>,
    /// Audit stamp copied from the report.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub meta: Option<finstack_quant_core::config::ResultsMeta>,
    /// Roll-forward report, when the scenario contained a time-roll operation.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub time_roll: Option<RollForwardReport>,
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
            instruments: instruments
                .map(|inventory| {
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
                        .collect::<serde_json::Result<Vec<_>>>()
                })
                .transpose()?,
            operations_applied: report.operations_applied,
            user_operations: report.user_operations,
            expanded_operations: report.expanded_operations,
            changes: report.changes,
            warnings: report.warnings,
            meta: report.meta,
            time_roll: report.time_roll,
        })
    }

    /// Split into market JSON, optional model JSON, instrument envelopes, and
    /// the [`ApplicationReport`] this envelope was built from.
    #[must_use]
    pub fn into_parts(
        self,
    ) -> (
        serde_json::Value,
        Option<serde_json::Value>,
        Option<Vec<InstrumentEnvelope>>,
        ApplicationReport,
    ) {
        let report = ApplicationReport {
            operations_applied: self.operations_applied,
            user_operations: self.user_operations,
            expanded_operations: self.expanded_operations,
            changes: self.changes,
            warnings: self.warnings,
            meta: self.meta,
            time_roll: self.time_roll,
        };
        (self.market, self.model, self.instruments, report)
    }
}
