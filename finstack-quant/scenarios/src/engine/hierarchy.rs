//! Hierarchy-target expansion and active-target verification.

use crate::error::Result;
use crate::spec::{CurveKind, OperationSpec};
use finstack_quant_core::FxHashMap;
use finstack_quant_core::market_data::bumps::MarketDataTarget;
use finstack_quant_core::market_data::context::MarketContext;
use finstack_quant_core::market_data::hierarchy::{
    HierarchyTarget, MarketDataHierarchy, ResolutionMode,
};

struct HierarchyExpansion {
    matched_depth: usize,
    operation: OperationSpec,
    key: MarketDataTarget,
}

/// Expanded operations after resolving hierarchy targets against the opening snapshot.
pub(super) struct ExpansionOutcome<'a> {
    pub(super) operations: std::borrow::Cow<'a, [OperationSpec]>,
    pub(super) required_targets: Vec<Option<MarketDataTarget>>,
}

/// Resolve one hierarchy target and project matches to its declared operation family.
fn expand_target(
    hierarchy: &MarketDataHierarchy,
    target: &HierarchyTarget,
    op_kind: &str,
    mut project: impl FnMut(MarketDataTarget) -> Option<OperationSpec>,
) -> Result<Vec<HierarchyExpansion>> {
    let matches = hierarchy
        .resolve_matches(target)
        .map_err(|err| crate::error::Error::Validation(format!("{op_kind}: {err}")))?;

    let mut expansions = Vec::with_capacity(matches.len());
    for matched in matches {
        let required = matched.target.clone();
        if let Some(operation) = project(matched.target) {
            expansions.push(HierarchyExpansion {
                matched_depth: matched.matched_depth,
                key: required,
                operation,
            });
        }
    }

    if expansions.is_empty() {
        return Err(crate::error::Error::Validation(format!(
            "Hierarchy operation '{op_kind}' targeting '{}' matched no targets of its declared family",
            target.path.join("/")
        )));
    }
    Ok(expansions)
}

/// Verify that an active hierarchy-selected target exists in `market` with its exact declared kind.
pub(super) fn verify_active_target(
    market: &MarketContext,
    target: &MarketDataTarget,
) -> Result<()> {
    match target {
        MarketDataTarget::DiscountCurve(id) => {
            market.get_discount(id.as_str()).map(|_| ())?;
        }
        MarketDataTarget::ForwardCurve(id) => {
            market.get_forward(id.as_str()).map(|_| ())?;
        }
        MarketDataTarget::HazardCurve(id) => {
            market.get_hazard(id.as_str()).map(|_| ())?;
        }
        MarketDataTarget::InflationCurve(id) => {
            market.get_inflation_curve(id.as_str()).map(|_| ())?;
        }
        MarketDataTarget::PriceCurve(id) => {
            market.get_price_curve(id.as_str()).map(|_| ())?;
        }
        MarketDataTarget::VolSurface(id) => {
            market.get_surface(id.as_str()).map(|_| ())?;
        }
        MarketDataTarget::BaseCorrelation(id) => {
            market.get_base_correlation(id.as_str()).map(|_| ())?;
        }
        MarketDataTarget::PriceScalar(id) => {
            let scalar = market.get_price(id.as_str())?;
            if scalar.get_price().is_none() {
                return Err(crate::error::Error::Validation(format!(
                    "Active target PriceScalar('{}') resolved to MarketScalar::Unitless in market context",
                    id.as_str()
                )));
            }
        }
        MarketDataTarget::UnitlessScalar(id) => {
            let scalar = market.get_price(id.as_str())?;
            if scalar.get_unitless().is_none() {
                return Err(crate::error::Error::Validation(format!(
                    "Active target UnitlessScalar('{}') resolved to MarketScalar::Price in market context",
                    id.as_str()
                )));
            }
        }
        other => {
            return Err(crate::error::Error::Validation(format!(
                "Unsupported active hierarchy target family '{}:{}'",
                other.kind_name(),
                other.id().as_str()
            )));
        }
    }
    Ok(())
}

/// Expand hierarchy-targeted operations into direct-targeted operations using only the
/// opening hierarchy snapshot and each operation's declared target family.
///
/// Errors if the spec contains hierarchy operations without an attached hierarchy,
/// if any selector fails resolution or matches zero candidates of its declared family,
/// without inspecting or falling back across live market storage during expansion.
pub(super) fn expand_hierarchy_operations<'a>(
    operations: &'a [OperationSpec],
    market: &MarketContext,
    mode: ResolutionMode,
) -> Result<ExpansionOutcome<'a>> {
    if !operations.iter().any(OperationSpec::is_hierarchy) {
        return Ok(ExpansionOutcome {
            operations: std::borrow::Cow::Borrowed(operations),
            required_targets: Vec::new(),
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

    for op in operations {
        let expansions = match op {
            OperationSpec::HierarchyCurveParallelBp {
                curve_kind,
                target,
                bp,
                discount_curve_id,
            } => expand_target(hierarchy, target, op.kind(), |matched_target| {
                let ((CurveKind::Discount, MarketDataTarget::DiscountCurve(curve_id))
                | (CurveKind::Forward, MarketDataTarget::ForwardCurve(curve_id))
                | (CurveKind::ParCDS, MarketDataTarget::HazardCurve(curve_id))
                | (CurveKind::Inflation, MarketDataTarget::InflationCurve(curve_id))) =
                    (curve_kind, matched_target)
                else {
                    return None;
                };
                Some(OperationSpec::CurveParallelBp {
                    curve_kind: *curve_kind,
                    curve_id,
                    discount_curve_id: discount_curve_id.clone(),
                    bp: *bp,
                })
            })?,
            OperationSpec::HierarchyCommodityCurveParallelPct { target, pct } => {
                expand_target(hierarchy, target, op.kind(), |matched_target| {
                    let MarketDataTarget::PriceCurve(curve_id) = matched_target else {
                        return None;
                    };
                    Some(OperationSpec::CommodityCurveParallelPct {
                        curve_id,
                        pct: *pct,
                    })
                })?
            }
            OperationSpec::HierarchyVolSurfaceParallelPct { target, pct } => {
                expand_target(hierarchy, target, op.kind(), |matched_target| {
                    let MarketDataTarget::VolSurface(vol_surface_id) = matched_target else {
                        return None;
                    };
                    Some(OperationSpec::VolSurfaceParallelPct {
                        vol_surface_id,
                        pct: *pct,
                    })
                })?
            }
            OperationSpec::HierarchyEquityPricePct { target, pct } => expand_target(
                hierarchy,
                target,
                op.kind(),
                |matched_target| match matched_target {
                    MarketDataTarget::PriceScalar(id) | MarketDataTarget::UnitlessScalar(id) => {
                        Some(OperationSpec::EquityPricePct {
                            ids: vec![id.as_str().to_string()],
                            pct: *pct,
                        })
                    }
                    _ => None,
                },
            )?,
            OperationSpec::HierarchyBaseCorrParallelPts { target, points } => {
                expand_target(hierarchy, target, op.kind(), |matched_target| {
                    let MarketDataTarget::BaseCorrelation(surface_id) = matched_target else {
                        return None;
                    };
                    Some(OperationSpec::BaseCorrParallelPts {
                        surface_id,
                        points: *points,
                    })
                })?
            }
            other => {
                slots.push(Slot::Direct(other.clone()));
                continue;
            }
        };
        slots.push(Slot::Expanded(expansions));
    }

    let max_depth: FxHashMap<MarketDataTarget, usize> =
        if matches!(mode, ResolutionMode::MostSpecificWins) {
            let mut md: FxHashMap<MarketDataTarget, usize> = FxHashMap::default();
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
            FxHashMap::default()
        };

    let mut result = Vec::with_capacity(operations.len());
    let mut required_targets = Vec::with_capacity(operations.len());
    for slot in slots {
        match slot {
            Slot::Direct(op) => {
                result.push(op);
                required_targets.push(None);
            }
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
                        required_targets.push(Some(exp.key));
                    }
                }
            }
        }
    }

    Ok(ExpansionOutcome {
        operations: std::borrow::Cow::Owned(result),
        required_targets,
    })
}
