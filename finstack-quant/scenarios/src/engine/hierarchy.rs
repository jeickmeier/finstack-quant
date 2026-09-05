//! Hierarchy-target expansion and resolution.

use crate::error::Result;
use crate::spec::{CurveKind, OperationSpec};
use crate::warning::Warning;
use finstack_quant_core::market_data::hierarchy::{
    HierarchyNode, HierarchyTarget, MarketDataHierarchy, ResolutionMode, TagFilter,
};
use finstack_quant_core::types::CurveId;
use finstack_quant_core::{HashMap, HashSet};

struct HierarchyExpansion {
    matched_depth: usize,
    operation: OperationSpec,
    key: HierarchyExpansionKey,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
enum HierarchyExpansionKey {
    Curve {
        curve_kind: CurveKind,
        curve_id: CurveId,
    },
    VolSurface {
        vol_surface_id: CurveId,
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

/// Direct operations after hierarchy expansion, plus any skip/no-match warnings.
pub(super) struct ExpansionOutcome<'a> {
    pub(super) operations: std::borrow::Cow<'a, [OperationSpec]>,
    pub(super) warnings: Vec<Warning>,
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

/// Skip hierarchy-resolved ids that are not in this operation's market collection.
fn retain_existing_targets(
    matches: Vec<HierarchyResolvedMatch>,
    op_kind: &str,
    warnings: &mut Vec<Warning>,
    exists: impl Fn(&CurveId) -> bool,
) -> Vec<HierarchyResolvedMatch> {
    let mut kept = Vec::with_capacity(matches.len());
    for m in matches {
        if exists(&m.curve_id) {
            kept.push(m);
        } else {
            warnings.push(Warning::HierarchyResolvedIdSkipped {
                curve_id: m.curve_id.as_str().to_string(),
                op_kind: op_kind.to_string(),
            });
        }
    }
    kept
}

fn curve_kind_target_exists(
    market: &finstack_quant_core::market_data::context::MarketContext,
    curve_kind: CurveKind,
    id: &CurveId,
) -> bool {
    match curve_kind {
        CurveKind::Discount => market.get_discount(id.as_str()).is_ok(),
        CurveKind::Commodity => market.get_price_curve(id.as_str()).is_ok(),
        CurveKind::Forward => market.get_forward(id.as_str()).is_ok(),
        CurveKind::ParCDS => market.get_hazard(id.as_str()).is_ok(),
        CurveKind::Inflation => market.get_inflation_curve(id.as_str()).is_ok(),
    }
}

/// Expand hierarchy-targeted operations into direct-targeted operations.
///
/// Errors if the spec contains hierarchy operations but the market has no
/// hierarchy attached. Zero-curve matches emit [`Warning::HierarchyNoMatch`];
/// identifiers that exist in the hierarchy but not in the targeted market
/// collection emit [`Warning::HierarchyResolvedIdSkipped`] instead of aborting.
pub(super) fn expand_hierarchy_operations<'a>(
    operations: &'a [OperationSpec],
    market: &finstack_quant_core::market_data::context::MarketContext,
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
                let matches = retain_existing_targets(
                    matches,
                    "HierarchyCurveParallelBp",
                    &mut warnings,
                    |id| curve_kind_target_exists(market, *curve_kind, id),
                );
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
            OperationSpec::HierarchyVolSurfaceParallelPct { target, pct } => {
                let matches = resolve_hierarchy_matches(hierarchy, target);
                if matches.is_empty() {
                    warnings.push(Warning::HierarchyNoMatch {
                        target_path: join_path(target),
                        op_kind: "HierarchyVolSurfaceParallelPct".to_string(),
                    });
                }
                let matches = retain_existing_targets(
                    matches,
                    "HierarchyVolSurfaceParallelPct",
                    &mut warnings,
                    |id| market.get_surface(id.as_str()).is_ok(),
                );
                let exps = expand_matches(matches, |curve_id| {
                    (
                        HierarchyExpansionKey::VolSurface {
                            vol_surface_id: curve_id.clone(),
                        },
                        OperationSpec::VolSurfaceParallelPct {
                            vol_surface_id: curve_id,
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
                let matches = retain_existing_targets(
                    matches,
                    "HierarchyEquityPricePct",
                    &mut warnings,
                    |id| market.get_price(id.as_str()).is_ok(),
                );
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
                let matches = retain_existing_targets(
                    matches,
                    "HierarchyBaseCorrParallelPts",
                    &mut warnings,
                    |id| market.get_base_correlation(id.as_str()).is_ok(),
                );
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
