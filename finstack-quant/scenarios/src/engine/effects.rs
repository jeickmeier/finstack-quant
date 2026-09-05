//! Operation dispatch, effect processing, and market-bump batching.

use super::instrument_shocks::{apply_correlation_effect, apply_instrument_shock, CorrelationKind};
use super::{ExecutionContext, HazardApplyEnv, ScenarioChangeManifest, ScenarioMarketTarget};
use crate::adapters;
use crate::adapters::traits::ScenarioEffect;
use crate::error::Result;
use crate::spec::{CurveKind, OperationSpec};
use crate::warning::Warning;
use finstack_quant_core::market_data::bumps::MarketBump;
use finstack_quant_core::types::CurveId;
use finstack_quant_core::HashSet;

/// Dispatch one operation to its adapter and produce effects.
///
/// Hierarchy variants and `TimeRollForward` are handled upstream; reaching
/// them here is an engine bug and returns [`crate::error::Error::Internal`].
fn generate_effects(
    op: &OperationSpec,
    ctx: &ExecutionContext,
    env: &HazardApplyEnv<'_>,
) -> Result<Vec<ScenarioEffect>> {
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
            env,
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
            env,
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
            detachment_bp,
            points,
        } => Ok(adapters::basecorr::base_corr_bucket_effects(
            surface_id,
            detachment_bp.as_deref(),
            *points,
            ctx,
        )),
        OperationSpec::VolSurfaceParallelPct {
            vol_surface_id,
            pct,
            ..
        } => adapters::vol::vol_parallel_effects(vol_surface_id, *pct, ctx),
        OperationSpec::VolSurfaceBucketPct {
            vol_surface_id,
            tenors,
            strikes,
            pct,
            ..
        } => adapters::vol::vol_bucket_effects(
            vol_surface_id,
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
            Err(crate::error::Error::Internal(format!(
                "scenario engine reached centralized dispatch for an op that should have been \
                 handled upstream (Phase 0 or hierarchy expansion); this indicates a bug in the \
                 dispatch pipeline. Operation: {op:?}"
            )))
        }
    }
}

fn market_target_for_id(op: &OperationSpec, id: &CurveId) -> Option<ScenarioMarketTarget> {
    match op {
        OperationSpec::EquityPricePct { .. } => Some(ScenarioMarketTarget::EquityPrice {
            price_id: id.clone(),
        }),
        OperationSpec::CurveParallelBp { curve_kind, .. }
        | OperationSpec::CurveNodeBp { curve_kind, .. } => Some(ScenarioMarketTarget::Curve {
            curve_kind: *curve_kind,
            curve_id: id.clone(),
        }),
        OperationSpec::VolIndexParallelPts { .. } | OperationSpec::VolIndexNodePts { .. } => {
            Some(ScenarioMarketTarget::VolatilityIndex {
                curve_id: id.clone(),
            })
        }
        OperationSpec::BaseCorrParallelPts { .. } | OperationSpec::BaseCorrBucketPts { .. } => {
            Some(ScenarioMarketTarget::BaseCorrelation {
                surface_id: id.clone(),
            })
        }
        OperationSpec::VolSurfaceParallelPct { .. } | OperationSpec::VolSurfaceBucketPct { .. } => {
            Some(ScenarioMarketTarget::VolSurface {
                vol_surface_id: id.clone(),
            })
        }
        OperationSpec::MarketFxPct { .. }
        | OperationSpec::StmtForecastPercent { .. }
        | OperationSpec::StmtForecastAssign { .. }
        | OperationSpec::RateBinding { .. }
        | OperationSpec::InstrumentPricePctByType { .. }
        | OperationSpec::InstrumentPricePctByAttr { .. }
        | OperationSpec::InstrumentSpreadBpByType { .. }
        | OperationSpec::InstrumentSpreadBpByAttr { .. }
        | OperationSpec::AssetCorrelationPts { .. }
        | OperationSpec::PrepayDefaultCorrelationPts { .. }
        | OperationSpec::HierarchyCurveParallelBp { .. }
        | OperationSpec::HierarchyVolSurfaceParallelPct { .. }
        | OperationSpec::HierarchyEquityPricePct { .. }
        | OperationSpec::HierarchyBaseCorrParallelPts { .. }
        | OperationSpec::TimeRollForward { .. } => None,
    }
}

fn market_target_for_bump(op: &OperationSpec, bump: &MarketBump) -> Option<ScenarioMarketTarget> {
    match (op, bump) {
        (OperationSpec::MarketFxPct { .. }, MarketBump::FxPct { base, quote, .. }) => {
            Some(ScenarioMarketTarget::Fx {
                base: *base,
                quote: *quote,
            })
        }
        (_, MarketBump::Curve { id, .. }) => market_target_for_id(op, id),
        (_, MarketBump::VolBucketPct { vol_surface_id, .. }) => {
            market_target_for_id(op, vol_surface_id)
        }
        (_, MarketBump::BaseCorrBucketPts { surface_id, .. }) => {
            market_target_for_id(op, surface_id)
        }
        _ => None,
    }
}

fn market_target_for_curve_update(
    op: &OperationSpec,
    storage: &finstack_quant_core::market_data::context::CurveStorage,
) -> Option<ScenarioMarketTarget> {
    market_target_for_id(op, storage.id())
}

fn replace_curve_id(op: &OperationSpec) -> Option<&CurveId> {
    match op {
        OperationSpec::CurveParallelBp {
            curve_kind: CurveKind::ParCDS | CurveKind::Inflation,
            curve_id,
            ..
        }
        | OperationSpec::CurveNodeBp {
            curve_kind: CurveKind::ParCDS | CurveKind::Inflation,
            curve_id,
            ..
        } => Some(curve_id),
        _ => None,
    }
}

/// Length of the leading run of independent ParCDS / inflation replacements.
pub(super) fn independent_replace_curve_run_len(ops: &[OperationSpec]) -> usize {
    let mut seen: HashSet<&str> = HashSet::default();
    let mut n = 0;
    for op in ops {
        let Some(id) = replace_curve_id(op) else {
            break;
        };
        if !seen.insert(id.as_str()) {
            break;
        }
        n += 1;
    }
    n
}

#[cfg(not(target_arch = "wasm32"))]
fn is_par_cds_replace(op: &OperationSpec) -> bool {
    matches!(
        op,
        OperationSpec::CurveParallelBp {
            curve_kind: CurveKind::ParCDS,
            ..
        } | OperationSpec::CurveNodeBp {
            curve_kind: CurveKind::ParCDS,
            ..
        }
    )
}

/// Whether a replace-curve run should generate in parallel.
///
/// Parallel generation is enabled only when the run contains at least two
/// ParCDS bootstrap replacements. Inflation-only runs stay serial.
pub(super) fn should_parallel_replace_curves(ops: &[OperationSpec]) -> bool {
    #[cfg(target_arch = "wasm32")]
    {
        let _ = ops;
        false
    }
    #[cfg(not(target_arch = "wasm32"))]
    {
        ops.iter().filter(|op| is_par_cds_replace(op)).count() >= 2
    }
}

/// Generate ParCDS / inflation replacement effects, in parallel when enabled.
pub(super) fn generate_replace_curve_effects_parallel(
    ops: &[OperationSpec],
    ctx: &ExecutionContext,
    env: &HazardApplyEnv<'_>,
) -> Result<Vec<Vec<ScenarioEffect>>> {
    let market = &*ctx.market;
    let as_of = ctx.as_of;
    #[cfg(not(target_arch = "wasm32"))]
    {
        use rayon::prelude::*;
        ops.par_iter()
            .map(|op| adapters::curves::generate_replace_curve_effects(op, market, as_of, env))
            .collect()
    }
    #[cfg(target_arch = "wasm32")]
    {
        ops.iter()
            .map(|op| adapters::curves::generate_replace_curve_effects(op, market, as_of, env))
            .collect()
    }
}

/// Mutable sinks shared while applying one operation's effects.
pub(super) struct EffectSink<'a> {
    pub pending_bumps: &'a mut Vec<MarketBump>,
    pub deferred_stmts: &'a mut Vec<ScenarioEffect>,
    pub warnings: &'a mut Vec<Warning>,
    pub applied: &'a mut usize,
    pub changes: &'a mut ScenarioChangeManifest,
}

pub(super) fn process_effects(
    op: &OperationSpec,
    ctx: &mut ExecutionContext,
    env: &HazardApplyEnv<'_>,
    sink: &mut EffectSink<'_>,
) -> Result<()> {
    let effects = generate_effects(op, ctx, env)?;
    apply_generated_effects(op, effects, ctx, sink)
}

/// Apply precomputed effects for one operation, preserving flush-before-write order.
pub(super) fn apply_generated_effects(
    op: &OperationSpec,
    effects: Vec<ScenarioEffect>,
    ctx: &mut ExecutionContext,
    sink: &mut EffectSink<'_>,
) -> Result<()> {
    for effect in effects {
        match effect {
            ScenarioEffect::MarketBump(b) => {
                if would_conflict_with_pending(sink.pending_bumps, &b) {
                    flush_pending_bumps(sink.pending_bumps, ctx.market)?;
                }
                match market_target_for_bump(op, &b) {
                    Some(target) => sink.changes.record_market_target(target),
                    None => sink.changes.all_dirty = true,
                }
                sink.pending_bumps.push(b);
                *sink.applied += 1;
            }
            ScenarioEffect::Warning(w) => sink.warnings.push(w),
            ScenarioEffect::UpdateCurve(storage) => {
                flush_pending_bumps(sink.pending_bumps, ctx.market)?;
                match market_target_for_curve_update(op, &storage) {
                    Some(target) => sink.changes.record_market_target(target),
                    None => sink.changes.all_dirty = true,
                }
                *ctx.market = std::mem::take(ctx.market).insert(storage);
                *sink.applied += 1;
            }
            ScenarioEffect::InstrumentPriceShock { types, attrs, pct } => {
                flush_pending_bumps(sink.pending_bumps, ctx.market)?;
                let outcome = apply_instrument_shock(
                    types.as_deref(),
                    attrs.as_ref(),
                    pct,
                    "price",
                    &mut ctx.instruments,
                    adapters::instruments::apply_instrument_type_price_shock,
                    adapters::instruments::apply_instrument_attr_price_shock,
                );
                *sink.applied += outcome.count;
                sink.changes
                    .record_instrument_indices(outcome.changed_indices);
                sink.warnings.extend(outcome.warnings);
            }
            ScenarioEffect::InstrumentSpreadShock { types, attrs, bp } => {
                flush_pending_bumps(sink.pending_bumps, ctx.market)?;
                let outcome = apply_instrument_shock(
                    types.as_deref(),
                    attrs.as_ref(),
                    bp,
                    "spread",
                    &mut ctx.instruments,
                    adapters::instruments::apply_instrument_type_spread_shock,
                    adapters::instruments::apply_instrument_attr_spread_shock,
                );
                *sink.applied += outcome.count;
                sink.changes
                    .record_instrument_indices(outcome.changed_indices);
                sink.warnings.extend(outcome.warnings);
            }
            ScenarioEffect::AssetCorrelationShock { delta_pts } => {
                flush_pending_bumps(sink.pending_bumps, ctx.market)?;
                let (count, indices, ws) =
                    apply_correlation_effect(CorrelationKind::Asset, delta_pts, ctx);
                *sink.applied += count;
                sink.changes.record_instrument_indices(indices);
                sink.warnings.extend(ws);
            }
            ScenarioEffect::PrepayDefaultCorrelationShock { delta_pts } => {
                flush_pending_bumps(sink.pending_bumps, ctx.market)?;
                let (count, indices, ws) =
                    apply_correlation_effect(CorrelationKind::PrepayDefault, delta_pts, ctx);
                *sink.applied += count;
                sink.changes.record_instrument_indices(indices);
                sink.warnings.extend(ws);
            }
            stmt @ (ScenarioEffect::StmtForecastPercent { .. }
            | ScenarioEffect::StmtForecastAssign { .. }
            | ScenarioEffect::RateBinding { .. }) => {
                sink.deferred_stmts.push(stmt);
            }
        }
    }
    Ok(())
}

/// Flush any accumulated [`MarketBump`]s through `MarketContext::bump` in a
/// single batched call. No-op when the buffer is empty.
pub(super) fn flush_pending_bumps(
    pending: &mut Vec<MarketBump>,
    market: &mut finstack_quant_core::market_data::context::MarketContext,
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
/// `HashMap<CurveId, BumpSpec>`, so two same-target bumps in one batch would
/// overwrite instead of composing `pre * (1+a) * (1+b)`.
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
            MarketBump::VolBucketPct {
                vol_surface_id: a,
                ..
            },
            MarketBump::VolBucketPct {
                vol_surface_id: b,
                ..
            },
        )
        | (
            MarketBump::BaseCorrBucketPts { surface_id: a, .. },
            MarketBump::BaseCorrBucketPts { surface_id: b, .. },
        ) => a == b,
        (
            MarketBump::Curve { id: a, .. },
            MarketBump::VolBucketPct {
                vol_surface_id: b,
                ..
            },
        )
        | (
            MarketBump::VolBucketPct {
                vol_surface_id: a,
                ..
            },
            MarketBump::Curve { id: b, .. },
        )
        | (MarketBump::Curve { id: a, .. }, MarketBump::BaseCorrBucketPts { surface_id: b, .. })
        | (MarketBump::BaseCorrBucketPts { surface_id: a, .. }, MarketBump::Curve { id: b, .. }) => {
            a == b
        }
        _ => false,
    })
}

#[cfg(test)]
mod replace_curve_tests {
    use super::*;
    use crate::spec::TenorMatchMode;

    fn par_cds_node(id: &str) -> OperationSpec {
        OperationSpec::CurveNodeBp {
            curve_kind: CurveKind::ParCDS,
            curve_id: id.into(),
            discount_curve_id: None,
            nodes: vec![("5Y".into(), 10.0)],
            match_mode: TenorMatchMode::Exact,
        }
    }

    fn inflation_node(id: &str) -> OperationSpec {
        OperationSpec::CurveNodeBp {
            curve_kind: CurveKind::Inflation,
            curve_id: id.into(),
            discount_curve_id: None,
            nodes: vec![("5Y".into(), 10.0)],
            match_mode: TenorMatchMode::Exact,
        }
    }

    #[test]
    fn independent_replace_curve_run_len_counts_distinct_ids() {
        let ops = vec![
            par_cds_node("A"),
            par_cds_node("B"),
            par_cds_node("A"),
            par_cds_node("C"),
        ];
        assert_eq!(independent_replace_curve_run_len(&ops), 2);
    }

    #[test]
    fn independent_replace_curve_run_len_stops_at_non_replace_op() {
        let ops = vec![
            par_cds_node("A"),
            OperationSpec::CurveParallelBp {
                curve_kind: CurveKind::Discount,
                curve_id: "USD-OIS".into(),
                discount_curve_id: None,
                bp: 1.0,
            },
        ];
        assert_eq!(independent_replace_curve_run_len(&ops), 1);
    }

    #[test]
    fn should_parallel_replace_curves_requires_two_par_cds() {
        let one = vec![par_cds_node("A")];
        assert!(!should_parallel_replace_curves(&one));

        let two = vec![par_cds_node("A"), par_cds_node("B")];
        #[cfg(not(target_arch = "wasm32"))]
        assert!(should_parallel_replace_curves(&two));
        #[cfg(target_arch = "wasm32")]
        assert!(!should_parallel_replace_curves(&two));
    }

    #[test]
    fn should_parallel_replace_curves_inflation_only_stays_serial() {
        let ops = vec![inflation_node("CPI-US"), inflation_node("CPI-EU")];
        assert!(!should_parallel_replace_curves(&ops));
    }

    #[test]
    fn should_parallel_replace_curves_one_par_cds_with_inflation_stays_serial() {
        let ops = vec![par_cds_node("A"), inflation_node("CPI")];
        assert!(!should_parallel_replace_curves(&ops));
    }
}
