//! Deterministic scenario execution engine.
//!
//! The engine glues together adapters from this crate to compose multiple
//! [`ScenarioSpec`](crate::spec::ScenarioSpec) definitions and apply them to
//! a mutable [`ExecutionContext`]. Its responsibilities are:
//! - enforce a repeatable ordering of operations
//! - dispatch each `OperationSpec` variant to the appropriate adapter function
//!   via a centralized exhaustive `match`
//! - batch market bumps so the underlying `MarketContext` is cloned at most
//!   once per scenario application instead of once per operation
//! - collect reporting metadata about how many operations ran and any
//!   warnings produced during execution

use crate::adapters::traits::ScenarioEffect;
use crate::error::Result;
use crate::spec::{OperationSpec, RateBindingSpec, ScenarioSpec, VolSurfaceKind};
use crate::warning::Warning;
use finstack_core::market_data::bumps::MarketBump;
use finstack_core::market_data::hierarchy::{
    HierarchyNode, HierarchyTarget, MarketDataHierarchy, ResolutionMode, TagFilter,
};
use finstack_core::types::CurveId;
use finstack_core::{HashMap, HashSet};
use finstack_statements::types::NodeId;
use finstack_statements::FinancialModelSpec;
use finstack_valuations::instruments::Instrument;
use indexmap::IndexMap;

fn rounding_stamp() -> Option<String> {
    Some(format!(
        "{:?}",
        finstack_core::config::RoundingMode::default()
    ))
}

/// Execution context for scenario application.
///
/// The context pins all mutable state that a scenario can touch — market data,
/// optional statement models, instrument inventories, and rate bindings —
/// together with the current valuation date.
///
/// # Fields
/// - `market`: Shared market data collection that stores curves, surfaces,
///   FX matrices, and spot prices.
/// - `model`: Optional financial statement model being shocked. Market-only
///   and instrument-only scenarios can leave this as `None`.
/// - `instruments`: Optional set of instruments to receive price/spread shocks
///   and to calculate carry/theta for time rolls.
/// - `rate_bindings`: Optional mapping from statement node identifiers to
///   detailed rate binding specs; used to sync statement rates after curve shocks.
/// - `calendar`: Optional holiday calendar for calendar-aware tenor calculations.
/// - `as_of`: Valuation date that operations reference.
///
/// # Examples
/// ```ignore
/// use finstack_scenarios::ExecutionContext;
/// use finstack_core::market_data::context::MarketContext;
/// use finstack_statements::FinancialModelSpec;
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
    pub market: &'a mut finstack_core::market_data::context::MarketContext,

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
    pub calendar: Option<&'a dyn finstack_core::dates::HolidayCalendar>,

    /// Valuation date for context.
    pub as_of: time::Date,
}

/// Report describing what happened during [`ScenarioEngine::apply`].
///
/// # Examples
/// ```rust
/// use finstack_scenarios::engine::ApplicationReport;
///
/// let report = ApplicationReport {
///     operations_applied: 3,
///     user_operations: 1,
///     expanded_operations: 3,
///     warnings: vec![],
///     rounding_context: Some("default".into()),
/// };
///
/// assert_eq!(report.operations_applied, 3);
/// assert_eq!(report.user_operations, 1);
/// assert_eq!(report.expanded_operations, 3);
/// ```
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct ApplicationReport {
    /// Number of effects successfully applied to the execution context.
    ///
    /// One user-level `OperationSpec` can produce multiple effects after
    /// hierarchy expansion (e.g. a single `CurveParallelBp` targeting the
    /// `USD` group may expand to one effect per USD-denominated discount or
    /// forward curve). Prefer `user_operations` for scenario-level reporting
    /// and this field for low-level audit.
    pub operations_applied: usize,
    /// Number of user-provided `OperationSpec` entries in the scenario
    /// (before hierarchy expansion and deduplication).
    pub user_operations: usize,
    /// Number of direct (non-hierarchy) operations produced after hierarchy
    /// expansion and resolution-mode deduplication. This is the count of
    /// operations that the engine actually tried to execute; it is always
    /// `>= user_operations` and is what should be compared to
    /// `operations_applied` when assessing scenario coverage.
    pub expanded_operations: usize,

    /// Structured warnings generated during application (non-fatal).
    pub warnings: Vec<Warning>,

    /// Rounding context stamp (for determinism tracking).
    pub rounding_context: Option<String>,
}

/// JSON envelope returned after applying a scenario to market data and,
/// optionally, a financial model.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct ApplicationEnvelope {
    /// Serialized mutated market context.
    pub market_json: String,
    /// Serialized mutated financial model, when a model was supplied.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub model_json: Option<String>,
    /// Number of effects successfully applied.
    pub operations_applied: usize,
    /// Number of user-provided operations before expansion.
    pub user_operations: usize,
    /// Number of expanded operations the engine attempted.
    pub expanded_operations: usize,
    /// Structured warnings produced while applying the scenario.
    pub warnings: Vec<Warning>,
}

impl ApplicationEnvelope {
    /// Build an envelope from a report and mutated contexts.
    ///
    /// # Errors
    ///
    /// Returns a serialization error if the market or model cannot be encoded
    /// as JSON.
    pub fn from_contexts(
        report: ApplicationReport,
        market: &finstack_core::market_data::context::MarketContext,
        model: Option<&finstack_statements::FinancialModelSpec>,
    ) -> serde_json::Result<Self> {
        Ok(Self {
            market_json: serde_json::to_string(market)?,
            model_json: model.map(serde_json::to_string).transpose()?,
            operations_applied: report.operations_applied,
            user_operations: report.user_operations,
            expanded_operations: report.expanded_operations,
            warnings: report.warnings,
        })
    }
}

/// Tracks a hierarchy-expanded operation with metadata needed for deduplication.
struct HierarchyExpansion {
    /// Depth of the matched hierarchy node (deeper = more specific).
    matched_depth: usize,
    /// The expanded direct operation.
    operation: OperationSpec,
    /// Operation family + identifier used for resolution-mode deduplication.
    key: HierarchyExpansionKey,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
enum HierarchyExpansionKey {
    Curve {
        curve_kind: crate::spec::CurveKind,
        curve_id: CurveId,
    },
    VolSurface {
        surface_kind: VolSurfaceKind,
        surface_id: CurveId,
    },
    EquityPrice {
        price_id: CurveId,
    },
    BaseCorrelation {
        surface_id: CurveId,
    },
}

#[derive(Debug, Clone)]
struct HierarchyResolvedMatch {
    curve_id: CurveId,
    matched_depth: usize,
}

fn collect_subtree_matches(
    node: &HierarchyNode,
    matched_depth: usize,
    matches: &mut Vec<HierarchyResolvedMatch>,
) {
    for curve_id in node.curve_ids() {
        matches.push(HierarchyResolvedMatch {
            curve_id: curve_id.clone(),
            matched_depth,
        });
    }
    for child in node.children().values() {
        collect_subtree_matches(child, matched_depth, matches);
    }
}

fn collect_filtered_matches(
    node: &HierarchyNode,
    filter: &TagFilter,
    depth: usize,
    matches: &mut Vec<HierarchyResolvedMatch>,
) {
    if filter.matches(node.tags()) {
        collect_subtree_matches(node, depth, matches);
    }
    for child in node.children().values() {
        collect_filtered_matches(child, filter, depth + 1, matches);
    }
}

fn resolve_hierarchy_matches(
    hierarchy: &MarketDataHierarchy,
    target: &HierarchyTarget,
) -> Vec<HierarchyResolvedMatch> {
    let Some(node) = hierarchy.get_node(&target.path) else {
        return Vec::new();
    };

    let mut matches = Vec::new();
    let start_depth = target.path.len();
    match &target.tag_filter {
        None => collect_subtree_matches(node, start_depth, &mut matches),
        Some(filter) => collect_filtered_matches(node, filter, start_depth, &mut matches),
    }
    dedup_matches_keep_deepest(matches)
}

/// Collapse duplicate curve hits to a single match per `curve_id`, keeping the
/// deepest `matched_depth` seen for each.
fn dedup_matches_keep_deepest(matches: Vec<HierarchyResolvedMatch>) -> Vec<HierarchyResolvedMatch> {
    let mut best: HashMap<CurveId, usize> = HashMap::default();
    for m in &matches {
        best.entry(m.curve_id.clone())
            .and_modify(|d| *d = (*d).max(m.matched_depth))
            .or_insert(m.matched_depth);
    }
    let mut seen: HashSet<CurveId> = HashSet::default();
    let mut out = Vec::with_capacity(best.len());
    for m in matches {
        if seen.insert(m.curve_id.clone()) {
            let depth = best[&m.curve_id];
            out.push(HierarchyResolvedMatch {
                curve_id: m.curve_id,
                matched_depth: depth,
            });
        }
    }
    out
}

/// Returns `true` if any operation is a hierarchy-targeted variant.
#[inline]
fn has_hierarchy_op(operations: &[OperationSpec]) -> bool {
    operations.iter().any(|op| {
        matches!(
            op,
            OperationSpec::HierarchyCurveParallelBp { .. }
                | OperationSpec::HierarchyVolSurfaceParallelPct { .. }
                | OperationSpec::HierarchyEquityPricePct { .. }
                | OperationSpec::HierarchyBaseCorrParallelPts { .. }
        )
    })
}

/// Result of `expand_hierarchy_operations`: the (possibly-borrowed) list of
/// direct operations plus any warnings that should be appended to the
/// `ApplicationReport` (currently only [`Warning::HierarchyNoMatch`]).
struct ExpansionOutcome<'a> {
    operations: std::borrow::Cow<'a, [OperationSpec]>,
    warnings: Vec<Warning>,
}

fn expand_matches(
    matches: Vec<HierarchyResolvedMatch>,
    mut make: impl FnMut(CurveId) -> (HierarchyExpansionKey, OperationSpec),
) -> Vec<HierarchyExpansion> {
    matches
        .into_iter()
        .map(|m| {
            let (key, operation) = make(m.curve_id.clone());
            HierarchyExpansion {
                matched_depth: m.matched_depth,
                key,
                operation,
            }
        })
        .collect()
}

/// Expand hierarchy-targeted operations into direct-targeted operations.
///
/// Errors if the spec contains hierarchy operations but the market context has
/// no hierarchy attached — that combination would otherwise silently produce
/// `operations_applied = 0` and a "not supported" warning, which is too quiet
/// for a stress system.
///
/// When a hierarchy target resolves to zero curves the operation is dropped
/// from the expanded list and a [`Warning::HierarchyNoMatch`] is emitted so
/// the caller can detect the (likely-unintended) no-op.
///
/// Returns a borrowed slice equivalent (via `Cow`) when the input contains no
/// hierarchy variants, avoiding an unnecessary clone of the operation list.
fn expand_hierarchy_operations<'a>(
    operations: &'a [OperationSpec],
    market: &finstack_core::market_data::context::MarketContext,
    mode: ResolutionMode,
) -> Result<ExpansionOutcome<'a>> {
    if !has_hierarchy_op(operations) {
        return Ok(ExpansionOutcome {
            operations: std::borrow::Cow::Borrowed(operations),
            warnings: Vec::new(),
        });
    }

    let hierarchy = market.hierarchy().ok_or_else(|| {
        crate::error::Error::Validation(
            "Scenario contains hierarchy-targeted operations but the market context has no \
             hierarchy attached. Attach a MarketDataHierarchy via MarketContext::set_hierarchy \
             or remove the Hierarchy* operations from the scenario."
                .to_string(),
        )
    })?;

    enum Slot {
        Direct(OperationSpec),
        Expanded(Vec<HierarchyExpansion>),
    }

    let mut slots: Vec<Slot> = Vec::with_capacity(operations.len());
    let mut warnings: Vec<Warning> = Vec::new();

    let join_path = |target: &HierarchyTarget| target.path.join("/");

    for op in operations {
        match op {
            OperationSpec::HierarchyCurveParallelBp {
                curve_kind,
                target,
                bp,
                discount_curve_id,
            } => {
                let matches = resolve_hierarchy_matches(hierarchy, target);
                if matches.is_empty() {
                    warnings.push(Warning::HierarchyNoMatch {
                        target_path: join_path(target),
                        op_kind: "HierarchyCurveParallelBp".to_string(),
                    });
                }
                let exps = expand_matches(matches, |curve_id| {
                    (
                        HierarchyExpansionKey::Curve {
                            curve_kind: *curve_kind,
                            curve_id: curve_id.clone(),
                        },
                        OperationSpec::CurveParallelBp {
                            curve_kind: *curve_kind,
                            curve_id,
                            discount_curve_id: discount_curve_id.clone(),
                            bp: *bp,
                        },
                    )
                });
                slots.push(Slot::Expanded(exps));
            }
            OperationSpec::HierarchyVolSurfaceParallelPct {
                surface_kind,
                target,
                pct,
            } => {
                let matches = resolve_hierarchy_matches(hierarchy, target);
                if matches.is_empty() {
                    warnings.push(Warning::HierarchyNoMatch {
                        target_path: join_path(target),
                        op_kind: "HierarchyVolSurfaceParallelPct".to_string(),
                    });
                }
                let exps = expand_matches(matches, |curve_id| {
                    (
                        HierarchyExpansionKey::VolSurface {
                            surface_kind: *surface_kind,
                            surface_id: curve_id.clone(),
                        },
                        OperationSpec::VolSurfaceParallelPct {
                            surface_kind: *surface_kind,
                            surface_id: curve_id,
                            pct: *pct,
                        },
                    )
                });
                slots.push(Slot::Expanded(exps));
            }
            OperationSpec::HierarchyEquityPricePct { target, pct } => {
                let matches = resolve_hierarchy_matches(hierarchy, target);
                if matches.is_empty() {
                    warnings.push(Warning::HierarchyNoMatch {
                        target_path: join_path(target),
                        op_kind: "HierarchyEquityPricePct".to_string(),
                    });
                }
                let exps = expand_matches(matches, |curve_id| {
                    (
                        HierarchyExpansionKey::EquityPrice {
                            price_id: curve_id.clone(),
                        },
                        OperationSpec::EquityPricePct {
                            ids: vec![curve_id.as_str().to_string()],
                            pct: *pct,
                        },
                    )
                });
                slots.push(Slot::Expanded(exps));
            }
            OperationSpec::HierarchyBaseCorrParallelPts { target, points } => {
                let matches = resolve_hierarchy_matches(hierarchy, target);
                if matches.is_empty() {
                    warnings.push(Warning::HierarchyNoMatch {
                        target_path: join_path(target),
                        op_kind: "HierarchyBaseCorrParallelPts".to_string(),
                    });
                }
                let exps = expand_matches(matches, |curve_id| {
                    (
                        HierarchyExpansionKey::BaseCorrelation {
                            surface_id: curve_id.clone(),
                        },
                        OperationSpec::BaseCorrParallelPts {
                            surface_id: curve_id,
                            points: *points,
                        },
                    )
                });
                slots.push(Slot::Expanded(exps));
            }
            other => slots.push(Slot::Direct(other.clone())),
        }
    }

    let max_depth: HashMap<HierarchyExpansionKey, usize> =
        if matches!(mode, ResolutionMode::MostSpecificWins) {
            let mut md: HashMap<HierarchyExpansionKey, usize> = HashMap::default();
            for slot in &slots {
                if let Slot::Expanded(exps) = slot {
                    for exp in exps {
                        md.entry(exp.key.clone())
                            .and_modify(|best| *best = (*best).max(exp.matched_depth))
                            .or_insert(exp.matched_depth);
                    }
                }
            }
            md
        } else {
            HashMap::default()
        };

    let mut result = Vec::with_capacity(operations.len());
    for slot in slots {
        match slot {
            Slot::Direct(op) => result.push(op),
            Slot::Expanded(exps) => {
                for exp in exps {
                    let keep = match mode {
                        ResolutionMode::Cumulative => true,
                        ResolutionMode::MostSpecificWins => max_depth
                            .get(&exp.key)
                            .is_some_and(|&max| exp.matched_depth == max),
                    };
                    if keep {
                        result.push(exp.operation);
                    }
                }
            }
        }
    }

    Ok(ExpansionOutcome {
        operations: std::borrow::Cow::Owned(result),
        warnings,
    })
}

/// Dispatch a single operation to the appropriate adapter and produce its effects.
///
/// Centralised match — the engine relies on Rust's exhaustiveness checker to
/// catch any newly added [`OperationSpec`] variant at compile time. Hierarchy-
/// targeted variants and `TimeRollForward` are handled separately and are
/// unreachable here (hierarchy variants are expanded upstream and time-roll is
/// processed in Phase 0 before this function is invoked).
fn generate_effects(op: &OperationSpec, ctx: &ExecutionContext) -> Result<Vec<ScenarioEffect>> {
    use crate::adapters;
    match op {
        OperationSpec::MarketFxPct { base, quote, pct } => {
            adapters::fx::fx_pct_effects(*base, *quote, *pct, ctx)
        }
        OperationSpec::EquityPricePct { ids, pct } => {
            adapters::equity::equity_pct_effects(ids, *pct, ctx)
        }
        OperationSpec::CurveParallelBp {
            curve_kind,
            curve_id,
            discount_curve_id,
            bp,
        } => adapters::curves::curve_parallel_effects(
            *curve_kind,
            curve_id,
            discount_curve_id.as_ref(),
            *bp,
            ctx,
        ),
        OperationSpec::CurveNodeBp {
            curve_kind,
            curve_id,
            discount_curve_id,
            nodes,
            match_mode,
        } => adapters::curves::curve_node_effects(
            *curve_kind,
            curve_id,
            discount_curve_id.as_ref(),
            nodes,
            *match_mode,
            ctx,
        ),
        OperationSpec::VolIndexParallelPts { curve_id, points } => {
            adapters::curves::vol_index_parallel_effects(curve_id, *points, ctx)
        }
        OperationSpec::VolIndexNodePts {
            curve_id,
            nodes,
            match_mode,
        } => adapters::curves::vol_index_node_effects(curve_id, nodes, *match_mode, ctx),
        OperationSpec::BaseCorrParallelPts { surface_id, points } => Ok(
            adapters::basecorr::base_corr_parallel_effects(surface_id, *points, ctx),
        ),
        OperationSpec::BaseCorrBucketPts {
            surface_id,
            detachment_bps,
            maturities,
            points,
        } => adapters::basecorr::base_corr_bucket_effects(
            surface_id,
            detachment_bps.as_deref(),
            maturities.as_deref(),
            *points,
            ctx,
        ),
        OperationSpec::VolSurfaceParallelPct {
            surface_id, pct, ..
        } => adapters::vol::vol_parallel_effects(surface_id, *pct, ctx),
        OperationSpec::VolSurfaceBucketPct {
            surface_id,
            tenors,
            strikes,
            pct,
            ..
        } => adapters::vol::vol_bucket_effects(
            surface_id,
            tenors.as_deref(),
            strikes.as_deref(),
            *pct,
            ctx,
        ),
        OperationSpec::StmtForecastPercent { node_id, pct } => Ok(
            adapters::statements::stmt_forecast_percent_effects(node_id, *pct),
        ),
        OperationSpec::StmtForecastAssign { node_id, value } => Ok(
            adapters::statements::stmt_forecast_assign_effects(node_id, *value),
        ),
        OperationSpec::RateBinding { binding } => {
            Ok(adapters::statements::rate_binding_effects(binding))
        }
        OperationSpec::InstrumentPricePctByType {
            instrument_types,
            pct,
        } => Ok(adapters::instruments::instrument_price_by_type_effects(
            instrument_types,
            *pct,
        )),
        OperationSpec::InstrumentPricePctByAttr { attrs, pct } => Ok(
            adapters::instruments::instrument_price_by_attr_effects(attrs, *pct),
        ),
        OperationSpec::InstrumentSpreadBpByType {
            instrument_types,
            bp,
        } => Ok(adapters::instruments::instrument_spread_by_type_effects(
            instrument_types,
            *bp,
        )),
        OperationSpec::InstrumentSpreadBpByAttr { attrs, bp } => Ok(
            adapters::instruments::instrument_spread_by_attr_effects(attrs, *bp),
        ),
        OperationSpec::AssetCorrelationPts { delta_pts } => {
            Ok(adapters::asset_corr::asset_corr_effects(*delta_pts))
        }
        OperationSpec::PrepayDefaultCorrelationPts { delta_pts } => Ok(
            adapters::asset_corr::prepay_default_corr_effects(*delta_pts),
        ),
        OperationSpec::TimeRollForward { .. }
        | OperationSpec::HierarchyCurveParallelBp { .. }
        | OperationSpec::HierarchyVolSurfaceParallelPct { .. }
        | OperationSpec::HierarchyEquityPricePct { .. }
        | OperationSpec::HierarchyBaseCorrParallelPts { .. } => {
            // These variants should never reach the centralized dispatch:
            // `TimeRollForward` is processed in Phase 0 and `Hierarchy*` ops
            // are expanded upstream by `expand_hierarchy_operations`. Returning
            // a typed error rather than panicking preserves the
            // `#![deny(clippy::panic)]` discipline and lets the caller surface
            // the bug through the normal error path instead of crashing the
            // process.
            Err(crate::error::Error::Internal(format!(
                "scenario engine reached centralized dispatch for an op that should have been \
                 handled upstream (Phase 0 or hierarchy expansion); this indicates a bug in the \
                 dispatch pipeline. Operation: {op:?}"
            )))
        }
    }
}

/// Orchestrates the deterministic application of a [`ScenarioSpec`].
///
/// The engine is intentionally lightweight: it does not own any state and can
/// be cloned or reused freely. All mutable inputs are supplied via
/// [`ExecutionContext`].
#[derive(Debug, Default, Clone)]
pub struct ScenarioEngine {
    _private: (),
}

impl ScenarioEngine {
    /// Create a new scenario engine with default settings.
    ///
    /// # Examples
    /// ```rust
    /// use finstack_scenarios::ScenarioEngine;
    ///
    /// let engine = ScenarioEngine::new();
    /// let other = ScenarioEngine::default();
    /// assert_eq!(format!("{:?}", engine), format!("{:?}", other));
    /// ```
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    pub(crate) fn compose_inner(&self, mut scenarios: Vec<ScenarioSpec>) -> ScenarioSpec {
        // Stable sort by priority (lower = higher priority)
        scenarios.sort_by_key(|s| s.priority);

        let composed_id = if scenarios.is_empty() {
            "composed".to_string()
        } else {
            scenarios
                .iter()
                .map(|scenario| scenario.id.as_str())
                .collect::<Vec<_>>()
                .join("+")
        };
        let composed_name = if scenarios.is_empty() {
            Some("Composed Scenario".to_string())
        } else {
            Some(
                scenarios
                    .iter()
                    .map(|scenario| scenario.name.as_deref().unwrap_or(scenario.id.as_str()))
                    .collect::<Vec<_>>()
                    .join(" + "),
            )
        };
        let mut all_operations = Vec::new();
        let resolution_mode = if scenarios.is_empty() {
            ResolutionMode::default()
        } else if scenarios
            .iter()
            .all(|scenario| scenario.resolution_mode == scenarios[0].resolution_mode)
        {
            scenarios[0].resolution_mode
        } else {
            ResolutionMode::Cumulative
        };

        for scenario in scenarios {
            all_operations.extend(scenario.operations);
        }

        ScenarioSpec {
            id: composed_id,
            name: composed_name,
            description: None,
            operations: all_operations,
            priority: 0,
            resolution_mode,
        }
    }

    /// Strict composition: returns an error at compose time when the
    /// concatenated operations would be rejected at apply time.
    ///
    /// Currently the only compose-time-detectable pathology is the presence of
    /// more than one [`OperationSpec::TimeRollForward`] across the composed
    /// scenarios. Production callers should prefer this method.
    pub fn try_compose(
        &self,
        scenarios: Vec<ScenarioSpec>,
    ) -> std::result::Result<ScenarioSpec, crate::error::Error> {
        let composed = self.compose_inner(scenarios);

        let time_roll_count = composed
            .operations
            .iter()
            .filter(|op| matches!(op, OperationSpec::TimeRollForward { .. }))
            .count();
        if time_roll_count > 1 {
            return Err(crate::error::Error::validation(format!(
                "Compose would produce {time_roll_count} TimeRollForward operations; only \
                 one is allowed per composed scenario. Merge the roll periods into a single \
                 `TimeRollForward` (preferred) or remove the duplicates before calling compose."
            )));
        }

        Ok(composed)
    }

    /// Apply a scenario specification to the execution context.
    ///
    /// Operations are applied in this order:
    /// 0. Time roll-forward, if present
    /// 1. Market data (FX, equities, vol surfaces, curves, base correlation) — all
    ///    [`MarketBump`] effects accumulated during this phase are applied to the
    ///    context in a single batched market bump call.
    /// 2. Rate bindings update (if configured)
    /// 3. Statement forecast adjustments
    /// 4. Statement re-evaluation
    ///
    /// If a [`crate::spec::OperationSpec::TimeRollForward`] sets
    /// `apply_shocks = false`, the engine returns immediately after phase 0 and
    /// does not apply the remaining operations in `spec`.
    #[tracing::instrument(skip_all, fields(scenario_id = %spec.id))]
    pub fn apply(
        &self,
        spec: &ScenarioSpec,
        ctx: &mut ExecutionContext,
    ) -> Result<ApplicationReport> {
        // Validate up-front so malformed specs cannot reach adapters. FFI
        // bindings (Python, WASM) deserialize JSON straight into a spec and
        // call this entry point without their own validation pass.
        spec.validate()?;

        let mut applied = 0;
        let mut warnings: Vec<Warning> = Vec::new();

        let user_operations = spec.operations.len();

        // Phase -1: Expand hierarchy-targeted operations to direct operations.
        // Errors fast if the spec contains hierarchy ops but no hierarchy is
        // attached to the market context. Hierarchy targets that resolve to
        // zero curves emit a `Warning::HierarchyNoMatch` so the caller can
        // detect the unintended no-op.
        let ExpansionOutcome {
            operations: expanded_ops,
            warnings: expansion_warnings,
        } = expand_hierarchy_operations(&spec.operations, ctx.market, spec.resolution_mode)?;
        let expanded_operations = expanded_ops.len();
        warnings.extend(expansion_warnings);

        // Phase 0: Time Roll Forward (`spec.validate()` already enforced the
        // at-most-one invariant; no need to re-count here.)
        for op in expanded_ops.iter() {
            if let OperationSpec::TimeRollForward {
                period,
                apply_shocks,
                roll_mode,
            } = op
            {
                let _span = tracing::info_span!("phase_0_time_roll", period = %period).entered();
                crate::adapters::time_roll::apply_time_roll_forward(ctx, period, *roll_mode)?;
                applied += 1;

                if !*apply_shocks {
                    return Ok(ApplicationReport {
                        operations_applied: applied,
                        user_operations,
                        expanded_operations,
                        warnings,
                        rounding_context: rounding_stamp(),
                    });
                }
            }
        }

        let has_rate_bindings = ctx.rate_bindings.is_some();
        let mut deferred_stmts = Vec::new();
        let mut pending_bumps: Vec<MarketBump> = Vec::new();

        // Phase 1: Generate effects and split into market bumps (intra-op
        // batched), curve replacements, instrument shocks, and deferred
        // statement ops. Bumps from the previous iteration are flushed before
        // generating effects for the next op so adapters always observe a
        // fully-applied prior-op market state — this preserves the sequential
        // semantics that downstream cross-curve calibrations depend on.
        {
            let _span = tracing::info_span!("phase_1_market", ops = expanded_operations).entered();
            for op in expanded_ops.iter() {
                if let OperationSpec::TimeRollForward { .. } = op {
                    continue; // handled in Phase 0
                }

                // Apply any bumps queued by the previous iteration so the
                // adapter's `ctx.market` reads reflect everything done so far.
                flush_pending_bumps(&mut pending_bumps, ctx.market)?;

                let effects = generate_effects(op, ctx)?;
                process_effects(
                    effects,
                    ctx,
                    &mut pending_bumps,
                    &mut deferred_stmts,
                    &mut warnings,
                    &mut applied,
                )?;
            }

            // Flush any remaining bumps before moving on to statements.
            flush_pending_bumps(&mut pending_bumps, ctx.market)?;
        }

        // Phase 2: Rate bindings update (from context configuration).
        //
        // The map key is authoritative for routing; mismatched binding.node_id
        // is a hard error so the caller fixes the binding upstream rather than
        // discovering a silent rewrite later.
        if let Some(bindings) = &ctx.rate_bindings {
            let _span = tracing::info_span!("phase_2_rate_bindings").entered();
            for (node_id, binding) in bindings {
                if binding.node_id != *node_id {
                    return Err(crate::error::Error::Validation(format!(
                        "Rate binding node_id mismatch: map key '{node_id}' does not equal \
                         binding.node_id '{}'. The map key is authoritative for routing; \
                         rebuild the binding with node_id set to the map key.",
                        binding.node_id
                    )));
                }

                let Some(model) = ctx.model.as_deref_mut() else {
                    return Err(crate::error::Error::missing_statement_model("rate binding"));
                };
                match crate::adapters::statements::update_rate_from_binding(
                    binding,
                    model,
                    ctx.market,
                    ctx.calendar,
                ) {
                    Ok(true) => {}
                    Ok(false) => warnings.push(Warning::RateBindingNoForecastValues {
                        node_id: node_id.as_str().to_string(),
                        curve_id: binding.curve_id.as_str().to_string(),
                    }),
                    Err(e) => warnings.push(Warning::RateBindingFailed {
                        node_id: node_id.as_str().to_string(),
                        curve_id: binding.curve_id.as_str().to_string(),
                        reason: e.to_string(),
                    }),
                }
            }
        }

        // Phase 3: Statement Operations (Deferred)
        let mut applied_stmt_ops = 0usize;
        {
            let _span = tracing::info_span!("phase_3_statements").entered();
            for effect in deferred_stmts {
                match effect {
                    ScenarioEffect::RateBinding { binding } => {
                        if let Some(rb) = &mut ctx.rate_bindings {
                            rb.insert(binding.node_id.clone(), binding.clone());
                        }
                        let Some(model) = ctx.model.as_deref_mut() else {
                            return Err(crate::error::Error::missing_statement_model(
                                "rate binding",
                            ));
                        };
                        match crate::adapters::statements::update_rate_from_binding(
                            &binding,
                            model,
                            ctx.market,
                            ctx.calendar,
                        ) {
                            Ok(true) => {
                                applied += 1;
                                applied_stmt_ops += 1;
                            }
                            Ok(false) => {
                                applied += 1;
                                applied_stmt_ops += 1;
                                warnings.push(Warning::RateBindingNoForecastValues {
                                    node_id: binding.node_id.as_str().to_string(),
                                    curve_id: binding.curve_id.as_str().to_string(),
                                });
                            }
                            Err(e) => warnings.push(Warning::RateBindingFailed {
                                node_id: binding.node_id.as_str().to_string(),
                                curve_id: binding.curve_id.as_str().to_string(),
                                reason: e.to_string(),
                            }),
                        }
                    }
                    ScenarioEffect::StmtForecastPercent { node_id, pct } => {
                        let Some(model) = ctx.model.as_deref_mut() else {
                            return Err(crate::error::Error::missing_statement_model(
                                "statement forecast percent",
                            ));
                        };
                        match crate::adapters::statements::apply_forecast_percent(
                            model,
                            node_id.as_str(),
                            pct,
                        ) {
                            Ok(true) => {
                                applied += 1;
                                applied_stmt_ops += 1;
                            }
                            Ok(false) => warnings.push(Warning::StatementNodeNoValues {
                                node_id: node_id.as_str().to_string(),
                                op: "forecast_percent".to_string(),
                            }),
                            Err(e) => warnings.push(Warning::StatementOpFailed {
                                node_id: node_id.as_str().to_string(),
                                op: "forecast_percent".to_string(),
                                reason: e.to_string(),
                            }),
                        }
                    }
                    ScenarioEffect::StmtForecastAssign { node_id, value } => {
                        let Some(model) = ctx.model.as_deref_mut() else {
                            return Err(crate::error::Error::missing_statement_model(
                                "statement forecast assign",
                            ));
                        };
                        match crate::adapters::statements::apply_forecast_assign(
                            model,
                            node_id.as_str(),
                            value,
                            None,
                        ) {
                            Ok(true) => {
                                applied += 1;
                                applied_stmt_ops += 1;
                            }
                            Ok(false) => warnings.push(Warning::StatementNodeNoValues {
                                node_id: node_id.as_str().to_string(),
                                op: "forecast_assign".to_string(),
                            }),
                            Err(e) => warnings.push(Warning::StatementOpFailed {
                                node_id: node_id.as_str().to_string(),
                                op: "forecast_assign".to_string(),
                                reason: e.to_string(),
                            }),
                        }
                    }
                    _ => {}
                }
            }
        }

        // Phase 4: Re-evaluate statements only if statement work was performed.
        if applied_stmt_ops > 0 || has_rate_bindings {
            let _span = tracing::info_span!("phase_4_reevaluate").entered();
            let Some(model) = ctx.model.as_deref_mut() else {
                return Err(crate::error::Error::missing_statement_model(
                    "statement re-evaluation",
                ));
            };
            match crate::adapters::statements::reevaluate_model(model) {
                Ok(eval_warnings) => warnings.extend(eval_warnings),
                Err(e) => warnings.push(Warning::ModelReevaluationFailed {
                    reason: e.to_string(),
                }),
            }
        }

        Ok(ApplicationReport {
            operations_applied: applied,
            user_operations,
            expanded_operations,
            warnings,
            rounding_context: rounding_stamp(),
        })
    }
}

/// Process a single op's effects, threading them through `pending_bumps`,
/// `deferred_stmts`, and the running counters. Extracted from `apply` to keep
/// the main pipeline readable; the dispatch is otherwise identical to the
/// inline match.
fn process_effects(
    effects: Vec<ScenarioEffect>,
    ctx: &mut ExecutionContext,
    pending_bumps: &mut Vec<MarketBump>,
    deferred_stmts: &mut Vec<ScenarioEffect>,
    warnings: &mut Vec<Warning>,
    applied: &mut usize,
) -> Result<()> {
    for effect in effects {
        match effect {
            ScenarioEffect::MarketBump(b) => {
                // Within a single op's effects, two bumps targeting the same
                // curve/surface/FX pair must compose sequentially rather than
                // collapse into one batch entry; flush before queueing if so.
                if would_conflict_with_pending(pending_bumps, &b) {
                    flush_pending_bumps(pending_bumps, ctx.market)?;
                }
                pending_bumps.push(b);
                *applied += 1;
            }
            ScenarioEffect::Warning(w) => warnings.push(w),
            ScenarioEffect::UpdateCurve(storage) => {
                // Flush any pending bumps so the curve replacement observes
                // the bumped market state in the same order as the original
                // per-effect application.
                flush_pending_bumps(pending_bumps, ctx.market)?;
                *ctx.market = std::mem::take(ctx.market).insert(storage);
                *applied += 1;
            }
            ScenarioEffect::InstrumentPriceShock { types, attrs, pct } => {
                flush_pending_bumps(pending_bumps, ctx.market)?;
                let (c, w) = apply_instrument_shock(
                    types.as_deref(),
                    attrs.as_ref(),
                    pct,
                    "price",
                    &mut ctx.instruments,
                    crate::adapters::instruments::apply_instrument_type_price_shock,
                    crate::adapters::instruments::apply_instrument_attr_price_shock,
                );
                *applied += c;
                warnings.extend(w);
            }
            ScenarioEffect::InstrumentSpreadShock { types, attrs, bp } => {
                flush_pending_bumps(pending_bumps, ctx.market)?;
                let (c, w) = apply_instrument_shock(
                    types.as_deref(),
                    attrs.as_ref(),
                    bp,
                    "spread",
                    &mut ctx.instruments,
                    crate::adapters::instruments::apply_instrument_type_spread_shock,
                    crate::adapters::instruments::apply_instrument_attr_spread_shock,
                );
                *applied += c;
                warnings.extend(w);
            }
            ScenarioEffect::AssetCorrelationShock { delta_pts } => {
                flush_pending_bumps(pending_bumps, ctx.market)?;
                let (count, ws) = apply_correlation_effect(CorrelationKind::Asset, delta_pts, ctx);
                *applied += count;
                warnings.extend(ws);
            }
            ScenarioEffect::PrepayDefaultCorrelationShock { delta_pts } => {
                flush_pending_bumps(pending_bumps, ctx.market)?;
                let (count, ws) =
                    apply_correlation_effect(CorrelationKind::PrepayDefault, delta_pts, ctx);
                *applied += count;
                warnings.extend(ws);
            }
            stmt @ (ScenarioEffect::StmtForecastPercent { .. }
            | ScenarioEffect::StmtForecastAssign { .. }
            | ScenarioEffect::RateBinding { .. }) => {
                deferred_stmts.push(stmt);
            }
        }
    }
    Ok(())
}

/// Flush any accumulated [`MarketBump`]s through `MarketContext::bump` in a
/// single batched call. No-op when the buffer is empty.
fn flush_pending_bumps(
    pending: &mut Vec<MarketBump>,
    market: &mut finstack_core::market_data::context::MarketContext,
) -> Result<()> {
    if pending.is_empty() {
        return Ok(());
    }
    let drained: Vec<MarketBump> = std::mem::take(pending);
    *market = market.bump(drained)?;
    Ok(())
}

/// Returns `true` when applying `incoming` would collide with a pending bump.
///
/// `MarketContext::bump_observed` keys [`MarketBump::Curve`] effects in a
/// `HashMap<CurveId, BumpSpec>`, so two bumps targeting the same curve in a
/// single batch would overwrite each other instead of composing
/// `pre * (1+a) * (1+b)`. To preserve the established sequential semantics,
/// we flush the pending batch whenever a new bump would land on the same
/// target as an already-queued one.
fn would_conflict_with_pending(pending: &[MarketBump], incoming: &MarketBump) -> bool {
    pending.iter().any(|p| match (p, incoming) {
        (
            MarketBump::FxPct {
                base: ba,
                quote: qa,
                ..
            },
            MarketBump::FxPct {
                base: bb,
                quote: qb,
                ..
            },
        ) => ba == bb && qa == qb,
        (MarketBump::Curve { id: a, .. }, MarketBump::Curve { id: b, .. })
        | (
            MarketBump::VolBucketPct { surface_id: a, .. },
            MarketBump::VolBucketPct { surface_id: b, .. },
        )
        | (
            MarketBump::BaseCorrBucketPts { surface_id: a, .. },
            MarketBump::BaseCorrBucketPts { surface_id: b, .. },
        ) => a == b,
        // A `Curve` bump on the same id as a `VolBucketPct` is also a logical
        // conflict (both target a vol surface) — flush to be safe.
        (MarketBump::Curve { id: a, .. }, MarketBump::VolBucketPct { surface_id: b, .. })
        | (MarketBump::VolBucketPct { surface_id: a, .. }, MarketBump::Curve { id: b, .. })
        | (MarketBump::Curve { id: a, .. }, MarketBump::BaseCorrBucketPts { surface_id: b, .. })
        | (MarketBump::BaseCorrBucketPts { surface_id: a, .. }, MarketBump::Curve { id: b, .. }) => {
            a == b
        }
        _ => false,
    })
}

/// Function that applies an instrument shock filtered by instrument type.
type TypeShockFn = fn(
    &mut [Box<dyn Instrument>],
    &[finstack_valuations::pricer::InstrumentType],
    f64,
) -> (usize, Vec<Warning>);

/// Function that applies an instrument shock filtered by attributes.
type AttrShockFn = fn(
    &mut [Box<dyn Instrument>],
    &indexmap::IndexMap<String, String>,
    f64,
) -> (usize, Vec<Warning>);

/// Apply an instrument shock (price or spread) dispatching by type and attribute filters.
fn apply_instrument_shock(
    types: Option<&[finstack_valuations::pricer::InstrumentType]>,
    attrs: Option<&indexmap::IndexMap<String, String>>,
    value: f64,
    kind: &'static str,
    instruments: &mut Option<&mut Vec<Box<dyn Instrument>>>,
    type_fn: TypeShockFn,
    attr_fn: AttrShockFn,
) -> (usize, Vec<Warning>) {
    let mut applied = 0;
    let mut warnings: Vec<Warning> = Vec::new();

    if let Some(ts) = types {
        if let Some(instruments) = instruments.as_mut() {
            let (c, w) = type_fn(instruments, ts, value);
            applied += c;
            warnings.extend(w);
        } else {
            warnings.push(Warning::InstrumentShockNoPortfolio {
                shock_kind: kind.to_string(),
                filter: "type".to_string(),
            });
        }
    }

    if let Some(ats) = attrs {
        if let Some(instruments) = instruments.as_mut() {
            let (count, w) = attr_fn(instruments, ats, value);
            applied += count;
            warnings.extend(w);
        } else {
            warnings.push(Warning::InstrumentShockNoPortfolio {
                shock_kind: kind.to_string(),
                filter: "attr".to_string(),
            });
        }
    }

    (applied, warnings)
}

/// Which structured-credit correlation parameter a shock targets.
#[derive(Debug, Clone, Copy)]
enum CorrelationKind {
    /// Asset correlation (clamped to `[0, 0.99]`).
    Asset,
    /// Prepay-default correlation (clamped to `[-0.99, 0.99]`).
    PrepayDefault,
}

/// Apply a correlation shock effect to StructuredCredit instruments via downcast.
fn apply_correlation_effect(
    kind: CorrelationKind,
    delta_pts: f64,
    ctx: &mut ExecutionContext,
) -> (usize, Vec<Warning>) {
    use finstack_valuations::instruments::fixed_income::structured_credit::StructuredCredit;

    let Some(instruments) = ctx.instruments.as_mut() else {
        return (0, vec![Warning::CorrelationShockNoPortfolio]);
    };

    let mut count = 0usize;
    let mut warnings = Vec::new();

    for inst in instruments.iter_mut() {
        let Some(sc) = inst.as_any_mut().downcast_mut::<StructuredCredit>() else {
            continue;
        };
        let Some(ref corr) = sc.credit_model.correlation_structure else {
            continue;
        };

        let (new_corr, clamp_info) = match kind {
            CorrelationKind::Asset => corr.bump_asset_with_clamp_info(delta_pts),
            CorrelationKind::PrepayDefault => corr.bump_prepay_default_with_clamp_info(delta_pts),
        };

        if let Some(info) = clamp_info {
            warnings.push(Warning::CorrelationClamped {
                instrument_id: sc.id.to_string(),
                detail: info,
            });
        }
        sc.credit_model.correlation_structure = Some(new_corr);
        count += 1;
    }

    if count == 0 {
        warnings.push(Warning::CorrelationShockNoMatch);
    }

    (count, warnings)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::spec::OperationSpec;

    #[test]
    fn try_compose_rejects_two_time_rolls() {
        use crate::spec::TimeRollMode;

        let engine = ScenarioEngine::new();
        let s1 = ScenarioSpec {
            id: "roll_6m".into(),
            name: Some("Roll 6M".into()),
            description: None,
            operations: vec![OperationSpec::TimeRollForward {
                period: "6M".into(),
                apply_shocks: true,
                roll_mode: TimeRollMode::default(),
            }],
            priority: 1,
            resolution_mode: ResolutionMode::Cumulative,
        };
        let s2 = ScenarioSpec {
            id: "roll_1y".into(),
            name: Some("Roll 1Y".into()),
            description: None,
            operations: vec![OperationSpec::TimeRollForward {
                period: "1Y".into(),
                apply_shocks: true,
                roll_mode: TimeRollMode::default(),
            }],
            priority: 2,
            resolution_mode: ResolutionMode::Cumulative,
        };

        let err = engine
            .try_compose(vec![s1, s2])
            .expect_err("duplicate TimeRollForward must error at compose time");
        let msg = format!("{err}");
        assert!(msg.contains("TimeRollForward"));
    }

    #[test]
    fn try_compose_preserves_source_ids_and_names() {
        let engine = ScenarioEngine::new();
        let scenarios = vec![
            ScenarioSpec {
                id: "rates_up".into(),
                name: Some("Rates Up".into()),
                description: None,
                operations: vec![OperationSpec::StmtForecastPercent {
                    node_id: "Revenue".into(),
                    pct: 1.0,
                }],
                priority: 2,
                resolution_mode: ResolutionMode::MostSpecificWins,
            },
            ScenarioSpec {
                id: "credit_down".into(),
                name: None,
                description: None,
                operations: vec![OperationSpec::StmtForecastPercent {
                    node_id: "Expenses".into(),
                    pct: -1.0,
                }],
                priority: 1,
                resolution_mode: ResolutionMode::Cumulative,
            },
        ];

        let strict = engine.try_compose(scenarios).expect("valid compose");

        assert_eq!(strict.id.as_str(), "credit_down+rates_up");
        assert_eq!(strict.name.as_deref(), Some("credit_down + Rates Up"));
        assert_eq!(strict.operations.len(), 2);
        assert_eq!(strict.resolution_mode, ResolutionMode::Cumulative);
    }

    #[test]
    fn apply_rejects_hierarchy_op_without_hierarchy() {
        use finstack_core::market_data::context::MarketContext;
        use finstack_core::market_data::hierarchy::HierarchyTarget;
        use finstack_statements::FinancialModelSpec;
        use time::macros::date;

        let mut market = MarketContext::new();
        let mut model = FinancialModelSpec::new("test", vec![]);
        let scenario = ScenarioSpec {
            id: "h_no_attach".into(),
            name: None,
            description: None,
            operations: vec![OperationSpec::HierarchyEquityPricePct {
                target: HierarchyTarget {
                    path: vec!["equities".into(), "us".into()],
                    tag_filter: None,
                },
                pct: -10.0,
            }],
            priority: 0,
            resolution_mode: Default::default(),
        };

        let engine = ScenarioEngine::new();
        let mut ctx = ExecutionContext {
            market: &mut market,
            model: Some(&mut model),
            instruments: None,
            rate_bindings: None,
            calendar: None,
            as_of: date!(2025 - 01 - 01),
        };
        let err = engine
            .apply(&scenario, &mut ctx)
            .expect_err("hierarchy op without hierarchy must error");
        assert!(err.to_string().contains("hierarchy"));
    }

    #[test]
    fn apply_emits_warning_when_hierarchy_target_matches_no_curves() {
        use finstack_core::market_data::context::MarketContext;
        use finstack_core::market_data::hierarchy::{HierarchyTarget, MarketDataHierarchy};
        use finstack_statements::FinancialModelSpec;
        use time::macros::date;

        // Empty hierarchy attached, but the target path has no curves.
        let hierarchy = MarketDataHierarchy::default();
        let mut market = MarketContext::new();
        market.set_hierarchy(hierarchy);
        let mut model = FinancialModelSpec::new("test", vec![]);
        let scenario = ScenarioSpec {
            id: "h_empty".into(),
            name: None,
            description: None,
            operations: vec![OperationSpec::HierarchyEquityPricePct {
                target: HierarchyTarget {
                    path: vec!["equities".into(), "us".into()],
                    tag_filter: None,
                },
                pct: -10.0,
            }],
            priority: 0,
            resolution_mode: Default::default(),
        };

        let engine = ScenarioEngine::new();
        let mut ctx = ExecutionContext {
            market: &mut market,
            model: Some(&mut model),
            instruments: None,
            rate_bindings: None,
            calendar: None,
            as_of: date!(2025 - 01 - 01),
        };
        let report = engine
            .apply(&scenario, &mut ctx)
            .expect("apply should succeed");

        assert_eq!(report.operations_applied, 0);
        assert_eq!(report.expanded_operations, 0);
        assert!(
            report.warnings.iter().any(|w| matches!(
                w,
                Warning::HierarchyNoMatch { op_kind, .. } if op_kind == "HierarchyEquityPricePct"
            )),
            "expected HierarchyNoMatch warning, got {:?}",
            report.warnings
        );
    }

    #[test]
    fn market_only_context_applies_without_statement_model() {
        use crate::spec::TimeRollMode;
        use finstack_core::market_data::context::MarketContext;
        use time::macros::date;

        let mut market = MarketContext::new();
        let scenario = ScenarioSpec {
            id: "market_only_roll".into(),
            name: None,
            description: None,
            operations: vec![OperationSpec::TimeRollForward {
                period: "1D".into(),
                apply_shocks: false,
                roll_mode: TimeRollMode::CalendarDays,
            }],
            priority: 0,
            resolution_mode: Default::default(),
        };

        let engine = ScenarioEngine::new();
        let mut ctx = ExecutionContext {
            market: &mut market,
            model: None,
            instruments: None,
            rate_bindings: None,
            calendar: None,
            as_of: date!(2025 - 01 - 01),
        };

        let report = engine
            .apply(&scenario, &mut ctx)
            .expect("market-only scenario should not require a statement model");

        assert_eq!(report.operations_applied, 1);
        assert_eq!(ctx.as_of, date!(2025 - 01 - 02));
    }

    #[test]
    fn statement_operation_without_model_errors_clearly() {
        use finstack_core::market_data::context::MarketContext;
        use time::macros::date;

        let mut market = MarketContext::new();
        let scenario = ScenarioSpec {
            id: "missing_model".into(),
            name: None,
            description: None,
            operations: vec![OperationSpec::StmtForecastPercent {
                node_id: "Revenue".into(),
                pct: -10.0,
            }],
            priority: 0,
            resolution_mode: Default::default(),
        };

        let engine = ScenarioEngine::new();
        let mut ctx = ExecutionContext {
            market: &mut market,
            model: None,
            instruments: None,
            rate_bindings: None,
            calendar: None,
            as_of: date!(2025 - 01 - 01),
        };

        let err = engine
            .apply(&scenario, &mut ctx)
            .expect_err("statement operation should require a statement model");

        assert!(matches!(
            err,
            crate::error::Error::MissingStatementModel { .. }
        ));
    }
}
