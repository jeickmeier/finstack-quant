//! Hierarchy-target expansion and resolution.

use crate::error::Result;
use crate::spec::{CurveKind, OperationSpec};
use crate::warning::Warning;
use finstack_quant_core::market_data::hierarchy::{
    HierarchyTarget, MarketDataHierarchy, ResolutionMode, ResolvedCurveMatch,
};
use finstack_quant_core::types::{CurveId, PriceId};
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
        spot_id: PriceId,
    },
    BaseCorrelation {
        surface_id: CurveId,
    },
}

fn resolve_hierarchy_matches(
    hierarchy: &MarketDataHierarchy,
    target: &HierarchyTarget,
) -> Vec<ResolvedCurveMatch> {
    dedup_matches_keep_deepest(hierarchy.resolve_matches(target, ResolutionMode::Cumulative))
}

/// Collapse duplicate curve hits to a single match per `curve_id`, keeping the
/// deepest `matched_depth` seen for each.
fn dedup_matches_keep_deepest(matches: Vec<ResolvedCurveMatch>) -> Vec<ResolvedCurveMatch> {
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
            out.push(ResolvedCurveMatch {
                curve_id: m.curve_id,
                matched_depth: depth,
            });
        }
    }
    out
}

/// Direct operations after hierarchy expansion, plus any skip/no-match warnings.
pub(super) struct ExpansionOutcome<'a> {
    pub(super) operations: std::borrow::Cow<'a, [OperationSpec]>,
    pub(super) warnings: Vec<Warning>,
}

/// Resolve one hierarchy target into direct operations.
///
/// Emits [`Warning::HierarchyNoMatch`] when the target resolves to nothing and
/// [`Warning::HierarchyResolvedIdSkipped`] for each resolved id absent from the
/// operation's market collection.
fn expand_target(
    hierarchy: &MarketDataHierarchy,
    target: &HierarchyTarget,
    op_kind: &str,
    warnings: &mut Vec<Warning>,
    exists: impl Fn(&CurveId) -> bool,
    mut make: impl FnMut(CurveId) -> (HierarchyExpansionKey, OperationSpec),
) -> Vec<HierarchyExpansion> {
    let matches = resolve_hierarchy_matches(hierarchy, target);
    if matches.is_empty() {
        warnings.push(Warning::HierarchyNoMatch {
            target_path: target.path.join("/"),
            op_kind: op_kind.to_string(),
        });
    }
    let mut expansions = Vec::with_capacity(matches.len());
    for m in matches {
        if exists(&m.curve_id) {
            let (key, operation) = make(m.curve_id);
            expansions.push(HierarchyExpansion {
                matched_depth: m.matched_depth,
                key,
                operation,
            });
        } else {
            warnings.push(Warning::HierarchyResolvedIdSkipped {
                curve_id: m.curve_id.as_str().to_string(),
                op_kind: op_kind.to_string(),
            });
        }
    }
    expansions
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
    if !operations.iter().any(OperationSpec::is_hierarchy) {
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

    for op in operations {
        let expansions = match op {
            OperationSpec::HierarchyCurveParallelBp {
                curve_kind,
                target,
                bp,
                discount_curve_id,
            } => expand_target(
                hierarchy,
                target,
                "HierarchyCurveParallelBp",
                &mut warnings,
                |id| curve_kind_target_exists(market, *curve_kind, id),
                |curve_id| {
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
                },
            ),
            OperationSpec::HierarchyVolSurfaceParallelPct { target, pct } => expand_target(
                hierarchy,
                target,
                "HierarchyVolSurfaceParallelPct",
                &mut warnings,
                |id| market.get_surface(id.as_str()).is_ok(),
                |curve_id| {
                    (
                        HierarchyExpansionKey::VolSurface {
                            vol_surface_id: curve_id.clone(),
                        },
                        OperationSpec::VolSurfaceParallelPct {
                            vol_surface_id: curve_id,
                            pct: *pct,
                        },
                    )
                },
            ),
            OperationSpec::HierarchyEquityPricePct { target, pct } => expand_target(
                hierarchy,
                target,
                "HierarchyEquityPricePct",
                &mut warnings,
                |id| market.get_price(id.as_str()).is_ok(),
                |curve_id| {
                    (
                        HierarchyExpansionKey::EquityPrice {
                            spot_id: PriceId::new(curve_id.as_str()),
                        },
                        OperationSpec::EquityPricePct {
                            ids: vec![curve_id.as_str().to_string()],
                            pct: *pct,
                        },
                    )
                },
            ),
            OperationSpec::HierarchyBaseCorrParallelPts { target, points } => expand_target(
                hierarchy,
                target,
                "HierarchyBaseCorrParallelPts",
                &mut warnings,
                |id| market.get_base_correlation(id.as_str()).is_ok(),
                |curve_id| {
                    (
                        HierarchyExpansionKey::BaseCorrelation {
                            surface_id: curve_id.clone(),
                        },
                        OperationSpec::BaseCorrParallelPts {
                            surface_id: curve_id,
                            points: *points,
                        },
                    )
                },
            ),
            other => {
                slots.push(Slot::Direct(other.clone()));
                continue;
            }
        };
        slots.push(Slot::Expanded(expansions));
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
