//! Operation dispatch, effect processing, and market-bump batching.

use super::instrument_shocks::apply_instrument_operation;
use super::{
    AppliedShock, AppliedShockTarget, ExecutionContext, HazardApplyEnv, LevelChange,
    ScenarioChangeManifest, ScenarioMarketTarget, ShockMagnitude, ShockNode, ShockUnit,
};
use crate::adapters;
use crate::adapters::curves::CurveApplyCtx;
use crate::error::Result;
use crate::spec::{CurveKind, OperationSpec};
use crate::warning::Warning;
use finstack_quant_core::market_data::bumps::{BumpSpec, BumpType, MarketBump};
use finstack_quant_core::market_data::context::CurveStorage;
use finstack_quant_core::market_data::scalars::MarketScalar;
use finstack_quant_core::types::{CurveId, PriceId};
use finstack_quant_core::HashSet;

/// Market change or warning an adapter produces for one operation, collected
/// before mutation.
///
/// Adapter modules translate a market [`OperationSpec`] into a
/// `Vec<ScenarioEffect>`; [`apply_generated_effects`] then applies them to the
/// mutable [`ExecutionContext`].
#[derive(Debug)]
pub(crate) enum ScenarioEffect {
    /// Market-data bump applied to the context.
    MarketBump(MarketBump),
    /// Price-scalar percentage bump, qualified by its market collection.
    PriceBump {
        /// Price-scalar identifier.
        id: CurveId,
        /// Percentage change; `10.0` increases the price by ten percent.
        pct: f64,
    },
    /// Volatility-surface bump, qualified by its market collection.
    SurfaceBump {
        /// Volatility-surface identifier.
        id: CurveId,
        /// Canonical surface bump specification.
        spec: BumpSpec,
    },
    /// Structured warning recorded on the application report.
    Warning(Warning),
    /// Replace a curve in the market (discount, forward, hazard, inflation, or vol-index).
    UpdateCurve(CurveStorage),
}

/// Dispatch one operation to its adapter and produce effects.
///
/// Only market operations produce effects. Hierarchy variants, `TimeRollForward`,
/// statement operations and instrument-scoped operations are handled elsewhere;
/// reaching them here is an engine bug and returns
/// [`crate::error::Error::Internal`].
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
            &CurveApplyCtx::new(ctx, env),
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
            &CurveApplyCtx::new(ctx, env),
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
        OperationSpec::StmtForecastPercent { .. }
        | OperationSpec::StmtForecastAssign { .. }
        | OperationSpec::RateBinding { .. }
        | OperationSpec::InstrumentPricePctByType { .. }
        | OperationSpec::InstrumentPricePctByAttr { .. }
        | OperationSpec::InstrumentSpreadBpByType { .. }
        | OperationSpec::InstrumentSpreadBpByAttr { .. }
        | OperationSpec::AssetCorrelationPts { .. }
        | OperationSpec::PrepayDefaultCorrelationPts { .. }
        | OperationSpec::TimeRollForward { .. }
        | OperationSpec::HierarchyCurveParallelBp { .. }
        | OperationSpec::HierarchyVolSurfaceParallelPct { .. }
        | OperationSpec::HierarchyEquityPricePct { .. }
        | OperationSpec::HierarchyBaseCorrParallelPts { .. } => {
            Err(crate::error::Error::Internal(format!(
                "scenario engine reached market-effect dispatch for an op that should have \
                 been handled elsewhere (Phase 0, hierarchy expansion, statement or instrument \
                 application); this indicates a bug in the dispatch pipeline. Operation: {op:?}"
            )))
        }
    }
}

fn market_target_for_id(op: &OperationSpec, id: &CurveId) -> Option<ScenarioMarketTarget> {
    match op {
        OperationSpec::EquityPricePct { .. } => Some(ScenarioMarketTarget::EquityPrice {
            spot_id: PriceId::new(id.as_str()),
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

/// Unit of the sizes on a curve operation: commodity price curves are shocked
/// in percent of the forward, every other curve kind in basis points.
fn curve_shock_unit(curve_kind: CurveKind) -> ShockUnit {
    match curve_kind {
        CurveKind::Commodity => ShockUnit::Percent,
        CurveKind::Discount | CurveKind::Forward | CurveKind::ParCDS | CurveKind::Inflation => {
            ShockUnit::Bp
        }
    }
}

fn node_shock(nodes: &[(String, f64)], unit: ShockUnit) -> ShockMagnitude {
    ShockMagnitude::Nodes {
        nodes: nodes
            .iter()
            .map(|(tenor, value)| ShockNode {
                tenor: tenor.clone(),
                value: *value,
            })
            .collect(),
        unit,
    }
}

/// Shock size as requested on a direct operation, in its own quote space.
///
/// `None` for operations that shock neither market data nor instruments
/// (statement forecasts, rate bindings) and for the variants handled upstream
/// of effect dispatch.
fn requested_shock(op: &OperationSpec) -> Option<ShockMagnitude> {
    let uniform = |value: f64, unit: ShockUnit| Some(ShockMagnitude::Uniform { value, unit });
    match op {
        OperationSpec::MarketFxPct { pct, .. }
        | OperationSpec::EquityPricePct { pct, .. }
        | OperationSpec::VolSurfaceParallelPct { pct, .. }
        | OperationSpec::InstrumentPricePctByType { pct, .. }
        | OperationSpec::InstrumentPricePctByAttr { pct, .. } => uniform(*pct, ShockUnit::Percent),
        OperationSpec::CurveParallelBp { curve_kind, bp, .. } => {
            uniform(*bp, curve_shock_unit(*curve_kind))
        }
        OperationSpec::CurveNodeBp {
            curve_kind, nodes, ..
        } => Some(node_shock(nodes, curve_shock_unit(*curve_kind))),
        OperationSpec::VolIndexParallelPts { points, .. }
        | OperationSpec::BaseCorrParallelPts { points, .. } => {
            uniform(*points, ShockUnit::Absolute)
        }
        OperationSpec::VolIndexNodePts { nodes, .. } => {
            Some(node_shock(nodes, ShockUnit::Absolute))
        }
        OperationSpec::BaseCorrBucketPts {
            detachment_bp,
            points,
            ..
        } => match detachment_bp {
            Some(detachments_bp) => Some(ShockMagnitude::DetachmentBucket {
                detachments_bp: detachments_bp.clone(),
                value: *points,
                unit: ShockUnit::Absolute,
            }),
            None => uniform(*points, ShockUnit::Absolute),
        },
        OperationSpec::VolSurfaceBucketPct { strikes, pct, .. } => {
            Some(ShockMagnitude::VolBucket {
                expiries_years: None,
                strikes: strikes.clone(),
                value: *pct,
                unit: ShockUnit::Percent,
            })
        }
        OperationSpec::InstrumentSpreadBpByType { bp, .. }
        | OperationSpec::InstrumentSpreadBpByAttr { bp, .. } => uniform(*bp, ShockUnit::Bp),
        OperationSpec::AssetCorrelationPts { delta_pts }
        | OperationSpec::PrepayDefaultCorrelationPts { delta_pts } => {
            uniform(*delta_pts, ShockUnit::Absolute)
        }
        OperationSpec::StmtForecastPercent { .. }
        | OperationSpec::StmtForecastAssign { .. }
        | OperationSpec::RateBinding { .. }
        | OperationSpec::HierarchyCurveParallelBp { .. }
        | OperationSpec::HierarchyVolSurfaceParallelPct { .. }
        | OperationSpec::HierarchyEquityPricePct { .. }
        | OperationSpec::HierarchyBaseCorrParallelPts { .. }
        | OperationSpec::TimeRollForward { .. } => None,
    }
}

/// Shock size for one batched market bump.
///
/// A bump that carries a resolved location (the knot of a key-rate bump, the
/// snapped surface expiries of a bucket bump) reports that location; every
/// other bump reports the operation's request.
fn shock_for_bump(op: &OperationSpec, bump: &MarketBump) -> Option<ShockMagnitude> {
    match (op, bump) {
        (OperationSpec::CurveNodeBp { curve_kind, .. }, MarketBump::Curve { spec, .. }) => {
            match spec.bump_type {
                BumpType::TriangularKeyRate { target_bucket, .. } => {
                    Some(ShockMagnitude::KeyRate {
                        time_years: target_bucket,
                        value: spec.value,
                        unit: curve_shock_unit(*curve_kind),
                    })
                }
                BumpType::Parallel => Some(ShockMagnitude::Uniform {
                    value: spec.value,
                    unit: curve_shock_unit(*curve_kind),
                }),
            }
        }
        (
            OperationSpec::VolSurfaceBucketPct { .. },
            MarketBump::VolBucketPct {
                expiries,
                strikes,
                pct,
                ..
            },
        ) => Some(ShockMagnitude::VolBucket {
            expiries_years: expiries.clone(),
            strikes: strikes.clone(),
            value: *pct,
            unit: ShockUnit::Percent,
        }),
        _ => requested_shock(op),
    }
}

/// Numeric level of a stored scalar: the value itself, or a price's amount.
fn scalar_level(scalar: &MarketScalar) -> f64 {
    match scalar {
        MarketScalar::Unitless(value) => *value,
        MarketScalar::Price(money) => money.amount(),
    }
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
    // Several failing ops report the first failing op in op order, the same
    // error the serial path returns.
    finstack_quant_core::parallel::try_map_ordered(ops, |op| {
        adapters::curves::generate_replace_curve_effects(op, market, as_of, env)
    })
}

/// Mutable sinks shared while applying one operation's effects.
pub(super) struct EffectSink<'a> {
    pub pending_bumps: &'a mut Vec<MarketBump>,
    pub warnings: &'a mut Vec<Warning>,
    pub applied: &'a mut usize,
    pub changes: &'a mut ScenarioChangeManifest,
    pub applied_shocks: &'a mut Vec<AppliedShock>,
}

impl EffectSink<'_> {
    /// Record a changed market target together with the size of its shock.
    fn record_market_shock(
        &mut self,
        target: ScenarioMarketTarget,
        shock: Option<ShockMagnitude>,
        level_change: Option<LevelChange>,
    ) {
        if let Some(shock) = shock {
            self.applied_shocks.push(AppliedShock {
                applies_to: AppliedShockTarget::Market {
                    target: target.clone(),
                },
                shock,
                level_change,
            });
        }
        self.changes.record_market_target(target);
    }

    /// Record instruments mutated in place together with the size of their shock.
    fn record_instrument_shock(&mut self, op: &OperationSpec, changed_indices: Vec<usize>) {
        if changed_indices.is_empty() {
            return;
        }
        if let Some(shock) = requested_shock(op) {
            let mut indices = changed_indices.clone();
            indices.sort_unstable();
            indices.dedup();
            self.applied_shocks.push(AppliedShock {
                applies_to: AppliedShockTarget::Instruments { indices },
                shock,
                level_change: None,
            });
        }
        self.changes.record_instrument_indices(changed_indices);
    }
}

pub(super) fn process_effects(
    op: &OperationSpec,
    ctx: &mut ExecutionContext,
    env: &HazardApplyEnv<'_>,
    sink: &mut EffectSink<'_>,
) -> Result<()> {
    if let Some(outcome) = apply_instrument_operation(op, &mut ctx.instruments)? {
        *sink.applied += outcome.count;
        sink.record_instrument_shock(op, outcome.changed_indices);
        sink.warnings.extend(outcome.warnings);
        return Ok(());
    }
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
            ScenarioEffect::PriceBump { id, pct } => {
                flush_pending_bumps(sink.pending_bumps, ctx.market)?;
                let before = ctx.market.get_price(id.as_str()).ok().map(scalar_level);
                ctx.market
                    .apply_price_bump_pct_in_place(id.as_str(), pct / 100.0)?;
                let after = ctx.market.get_price(id.as_str()).ok().map(scalar_level);
                sink.record_market_shock(
                    ScenarioMarketTarget::EquityPrice {
                        spot_id: PriceId::new(id.as_str()),
                    },
                    Some(ShockMagnitude::Uniform {
                        value: pct,
                        unit: ShockUnit::Percent,
                    }),
                    before
                        .zip(after)
                        .map(|(before, after)| LevelChange { before, after }),
                );
                *sink.applied += 1;
            }
            ScenarioEffect::SurfaceBump { id, spec } => {
                flush_pending_bumps(sink.pending_bumps, ctx.market)?;
                ctx.market.apply_surface_bump_in_place(id.as_str(), spec)?;
                sink.record_market_shock(
                    ScenarioMarketTarget::VolSurface { vol_surface_id: id },
                    requested_shock(op),
                    None,
                );
                *sink.applied += 1;
            }
            ScenarioEffect::MarketBump(b) => {
                match market_target_for_bump(op, &b) {
                    Some(target) => sink.record_market_shock(target, shock_for_bump(op, &b), None),
                    None => sink.changes.all_dirty = true,
                }
                sink.pending_bumps.push(b);
                *sink.applied += 1;
            }
            ScenarioEffect::Warning(w) => sink.warnings.push(w),
            ScenarioEffect::UpdateCurve(storage) => {
                flush_pending_bumps(sink.pending_bumps, ctx.market)?;
                match market_target_for_id(op, storage.id()) {
                    Some(target) => sink.record_market_shock(target, requested_shock(op), None),
                    None => sink.changes.all_dirty = true,
                }
                *ctx.market = std::mem::take(ctx.market).insert(storage);
                *sink.applied += 1;
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
