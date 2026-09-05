//! Parallel P&L attribution methodology.
//!
//! Independent factor isolation approach where each factor is analyzed separately
//! by restoring T₀ values for that factor while keeping all other factors at T₁.
//!
//! # Algorithm
//!
//! 1. Price at T₀ and T₁ with actual markets → total_pnl
//! 2. **Carry**: Price at T₁ date with T₀ market (frozen) → isolate time/accrual effect
//! 3. **RatesCurves**: Restore T₀ discount/forward curves, reprice → rates P&L
//! 4. **CreditCurves**: Restore T₀ hazard curves, reprice → credit P&L
//! 5. **InflationCurves**: Restore T₀ inflation curves, reprice → inflation P&L
//! 6. **Correlations**: Restore T₀ base correlation curves, reprice → correlation P&L
//! 7. **Fx**: Restore T₀ FX matrix, reprice → fx P&L
//! 8. **Volatility**: Restore T₀ vol surfaces, reprice → vol P&L
//! 9. **ModelParameters**: Restore T₀ model parameters, reprice → model params P&L
//! 10. **MarketScalars**: Restore T₀ market scalars, reprice → scalars P&L
//! 11. **Residual**: total_pnl - sum(all attributed factors)
//!
//! # Notes
//!
//! - Factors are isolated independently, so cross-effects appear in residual
//!   unless captured by a cross pair (below)
//! - Model parameters attribution requires instrument-specific support (see model_params.rs)
//!
//! # Default cross-factor pairs
//!
//! The default (non-`full_cross_attribution`) path extracts exactly these
//! pairwise interaction terms into `cross_factor_detail`; any interaction
//! between factor pairs NOT in this list still falls into the residual:
//!
//! 1. Rates×Credit
//! 2. Rates×Vol
//! 3. Spot×Vol
//! 4. Spot×Credit
//! 5. FX×Vol
//! 6. FX×Rates
//! 7. Credit×Vol
//! 8. Rates×Inflation
//! 9. Credit×Correlations
//!
//! For exhaustive pairwise coverage (all active factor pairs, including
//! ModelParameters), set `full_cross_attribution = true` on the
//! [`crate::AttributionRequest`].

use super::credit_cascade::{
    build_credit_factor_attribution, plan_credit_cascade, shift_credit_curves_par_spread,
    snap_hazard_to_t1, CreditCascadeStep,
};
use super::factors::*;
use super::helpers::*;
use super::model_params;
use super::types::*;
use crate::policy_map::{try_map_policy, try_map_policy_zip};
use crate::AttributionRequest;
use finstack_quant_calibration::recalibration::CachedRecalibrationProvider;
use finstack_quant_core::dates::Date;
use finstack_quant_core::market_data::context::MarketContext;
use finstack_quant_core::money::Money;
use finstack_quant_core::Result;
use finstack_quant_valuations::instruments::model_params::ModelParamsSnapshot;
use finstack_quant_valuations::instruments::Instrument;
use indexmap::IndexMap;
use std::sync::Arc;

/// Additive cross-factor interaction contribution for a pair of factors.
///
/// Parallel factor P&Ls are measured from the T₁ base
/// (`fᵢ = V(all-T₁) − V(factorᵢ@T₀)`), so for two moved factors the exact
/// two-factor identity is
///
/// ```text
/// total = f_a + f_b + [V(a@T₀) + V(b@T₀) − V(all-T₁) − V(ab@T₀)]
/// ```
///
/// i.e. the additive interaction term is the **negated** mixed second
/// difference `−(V₁₁ − V(a@T₀) − V(b@T₀) + V(ab@T₀))`. Storing this value in
/// `cross_factor_pnl` makes `compute_residual`'s additive convention
/// reconcile: extracting the pairwise cross terms drives the residual of a
/// two-factor attribution to exactly zero. This matches the metrics-based
/// path, whose Taylor cross-gamma term is additive by construction, so
/// `cross_factor_pnl` has the same "additive contribution to the attributed
/// sum" semantics across methods. (A positive value means the factors'
/// co-movement *added* P&L beyond the sum of their isolated effects.)
fn cross_interaction_pnl(
    val_t1: Money,
    val_with_t0_a: Money,
    val_with_t0_b: Money,
    val_with_t0_ab: Money,
) -> Result<Money> {
    val_with_t0_a
        .checked_add(val_with_t0_b)?
        .checked_sub(val_t1)?
        .checked_sub(val_with_t0_ab)
}

/// Cross-factor tolerance for including an interaction term in the detail map.
/// Matches the historical inline filter (`pnl.amount().abs() > 1e-12`).
const CROSS_FACTOR_TOLERANCE: f64 = 1e-12;

#[derive(Debug, Copy, Clone, PartialEq, Eq)]
enum ParallelRestoredFactor {
    Rates,
    Credit,
    Inflation,
    Correlations,
    Volatility,
    MarketScalars,
    Discount,
    Forward,
    FX,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ActiveFactorKind {
    Discount,
    Forward,
    Credit,
    Inflation,
    Correlations,
    FX,
    Volatility,
    MarketScalars,
    ModelParameters,
}

fn get_restore_flags(kind: ActiveFactorKind) -> MarketRestoreFlags {
    match kind {
        ActiveFactorKind::Discount => MarketRestoreFlags::DISCOUNT,
        ActiveFactorKind::Forward => MarketRestoreFlags::FORWARD,
        ActiveFactorKind::Credit => MarketRestoreFlags::HAZARD,
        ActiveFactorKind::Inflation => MarketRestoreFlags::INFLATION,
        ActiveFactorKind::Correlations => MarketRestoreFlags::CORRELATION,
        ActiveFactorKind::FX => MarketRestoreFlags::FX,
        ActiveFactorKind::Volatility => MarketRestoreFlags::VOL,
        ActiveFactorKind::MarketScalars => MarketRestoreFlags::SCALARS,
        ActiveFactorKind::ModelParameters => MarketRestoreFlags::empty(),
    }
}

#[derive(Clone)]
enum ParallelLatentFactorSpec {
    Market {
        factor: ParallelRestoredFactor,
        flags: MarketRestoreFlags,
        snapshot: Box<MarketSnapshot>,
    },
    ModelParams {
        snapshot: ModelParamsSnapshot,
    },
}

struct FirstOrderRepriceResult {
    spec: ParallelLatentFactorSpec,
    pnl: Money,
    reprice_val: Money,
}

struct RestoredFactorEval {
    factor: ParallelRestoredFactor,
    snapshot: MarketSnapshot,
    output: Option<(Money, Money)>,
}

/// Accumulate a cross-factor interaction P&L into the running totals if its
/// magnitude exceeds `CROSS_FACTOR_TOLERANCE`.
fn record_cross_pair(
    pair: &str,
    pnl: Money,
    cross_total: &mut f64,
    cross_by_pair: &mut IndexMap<String, Money>,
) {
    if pnl.amount().abs() > CROSS_FACTOR_TOLERANCE {
        *cross_total += pnl.amount();
        cross_by_pair.insert(pair.to_string(), pnl);
    }
}

/// Note: surface factors that were skipped because T0 had no
/// market data for the family while T1 does — e.g. a hazard curve first
/// marked between T0 and T1, a vol surface introduced, an FX matrix attached.
/// Without a note the entire move silently flows into the residual and
/// operators cannot distinguish "cross-effect residual" from "factor dropped
/// because T0 data was missing".
fn restored_factor_is_used(
    factor: ParallelRestoredFactor,
    factor_use: InstrumentFactorUse,
) -> bool {
    match factor {
        ParallelRestoredFactor::Rates
        | ParallelRestoredFactor::Discount
        | ParallelRestoredFactor::Forward => factor_use.rates,
        ParallelRestoredFactor::Credit => factor_use.credit,
        ParallelRestoredFactor::Inflation => factor_use.inflation,
        ParallelRestoredFactor::Correlations => true,
        ParallelRestoredFactor::FX => factor_use.fx,
        ParallelRestoredFactor::Volatility => factor_use.volatility,
        ParallelRestoredFactor::MarketScalars => factor_use.scalars,
    }
}

fn note_skipped_empty_t0_factors(
    attribution: &mut PnlAttribution,
    market_t0: &MarketContext,
    market_t1: &MarketContext,
    factor_use: InstrumentFactorUse,
    credit_curve_ids: &[finstack_quant_core::types::CurveId],
) {
    use ParallelRestoredFactor as F;
    let families: [(F, &str); 7] = [
        (F::Rates, "RatesCurves"),
        (F::Credit, "CreditCurves"),
        (F::Inflation, "InflationCurves"),
        (F::Correlations, "Correlations"),
        (F::FX, "Fx"),
        (F::Volatility, "Volatility"),
        (F::MarketScalars, "MarketScalars"),
    ];
    // `restored_factor_has_data` only inspects the per-family fields, so a
    // single snapshot of the families the instrument uses answers every
    // has-data question while iterating the market's curves once. Unused
    // book-market families are not diagnostic noise. The T1 snapshot is
    // built lazily and only when at least one used family is absent at T0.
    let mut flags = MarketRestoreFlags::CORRELATION;
    if factor_use.rates {
        flags = flags | MarketRestoreFlags::RATES;
    }
    if factor_use.credit {
        flags = flags | MarketRestoreFlags::CREDIT;
    }
    if factor_use.inflation {
        flags = flags | MarketRestoreFlags::INFLATION;
    }
    if factor_use.fx {
        flags = flags | MarketRestoreFlags::FX;
    }
    if factor_use.volatility {
        flags = flags | MarketRestoreFlags::VOL;
    }
    if factor_use.scalars {
        flags = flags | MarketRestoreFlags::SCALARS;
    }
    let snap_t0 = MarketSnapshot::extract_with_credit_roles(market_t0, flags, credit_curve_ids);
    let mut snap_t1: Option<MarketSnapshot> = None;
    for (factor, name) in families {
        if !restored_factor_is_used(factor, factor_use) {
            continue;
        }
        if restored_factor_has_data(factor, &snap_t0) {
            continue;
        }
        let snap_t1 = snap_t1.get_or_insert_with(|| {
            MarketSnapshot::extract_with_credit_roles(market_t1, flags, credit_curve_ids)
        });
        if restored_factor_has_data(factor, snap_t1) {
            tracing::warn!(
                instrument_id = %attribution.meta.instrument_id,
                factor = name,
                "factor skipped: T0 market has no data for this family while T1 does; \
                 its move is unattributed and falls into the residual"
            );
            attribution.meta.notes.push(format!(
                "Factor {name} skipped: T0 market has no data for this family while T1 \
                 does; its move is unattributed and falls into the residual"
            ));
        }
    }
}

fn extract_if_used(
    used: bool,
    market: &MarketContext,
    flags: MarketRestoreFlags,
    credit_curve_ids: &[finstack_quant_core::types::CurveId],
) -> MarketSnapshot {
    if used {
        MarketSnapshot::extract_with_credit_roles(market, flags, credit_curve_ids)
    } else {
        MarketSnapshot::default()
    }
}

fn restored_factor_has_data(factor: ParallelRestoredFactor, snapshot: &MarketSnapshot) -> bool {
    match factor {
        // Fix: gate Rates on the underlying snapshot so we don't burn a
        // reprice — or, worse, *drop* T1 rate curves when T0 has none — when
        // neither market has rates. Aligns with the other restored-factor
        // variants' semantics.
        ParallelRestoredFactor::Rates => {
            !snapshot.discount_curves.is_empty()
                || !snapshot.forward_curves.is_empty()
                || !snapshot.basis_spread_curves.is_empty()
                || !snapshot.parametric_curves.is_empty()
                || !snapshot.fixing_series.is_empty()
        }
        ParallelRestoredFactor::Credit => {
            !snapshot.hazard_curves.is_empty() || !snapshot.credit_discount_curves.is_empty()
        }
        ParallelRestoredFactor::Inflation => !snapshot.inflation_curves.is_empty(),
        ParallelRestoredFactor::Correlations => !snapshot.base_correlation_curves.is_empty(),
        ParallelRestoredFactor::Volatility => {
            !snapshot.surfaces.is_empty()
                || !snapshot.vol_cubes.is_empty()
                || !snapshot.fx_delta_vol_surfaces.is_empty()
                || !snapshot.vol_index_curves.is_empty()
        }
        ParallelRestoredFactor::MarketScalars => {
            !snapshot.prices.is_empty()
                || !snapshot.series.is_empty()
                || !snapshot.inflation_indices.is_empty()
                || !snapshot.dividends.is_empty()
                || !snapshot.price_curves.is_empty()
        }
        ParallelRestoredFactor::Discount => !snapshot.discount_curves.is_empty(),
        ParallelRestoredFactor::Forward => {
            !snapshot.forward_curves.is_empty()
                || !snapshot.basis_spread_curves.is_empty()
                || !snapshot.parametric_curves.is_empty()
                || !snapshot.fixing_series.is_empty()
        }
        ParallelRestoredFactor::FX => snapshot.fx.is_some(),
    }
}

/// Compute per-factor attribution P&L: reprice the instrument with T0 values
/// for the given factor restored, then compare to T1 value using `compute_pnl`
/// (T1-FX conversion — non-FX factors only).
///
/// Returns `None` if the snapshot contains no data for the factor (so the
/// attribution field stays at its zero default). Returns
/// `Some((factor_pnl, val_with_t0, market_with_t0))` when the factor was
/// populated — the caller uses `val_with_t0` for cross-factor repricings and
/// can reuse `market_with_t0` as the base for compound markets.
#[allow(clippy::too_many_arguments)]
fn reprice_factor_restored_once(
    instrument: &Arc<dyn Instrument>,
    market_t1: &MarketContext,
    snapshot: &MarketSnapshot,
    flags: MarketRestoreFlags,
    has_data: bool,
    as_of_t1: Date,
    val_t1: Money,
) -> Result<Option<(Money, Money)>> {
    if !has_data {
        return Ok(None);
    }
    let market_with_t0 = MarketSnapshot::restore_market(market_t1, snapshot, flags);
    let reprice = reprice_instrument(instrument, &market_with_t0, as_of_t1)?;
    let factor_pnl = compute_pnl(reprice, val_t1, val_t1.currency(), market_t1, as_of_t1)?;
    Ok(Some((factor_pnl, reprice)))
}

/// Reprice the instrument with two factors simultaneously restored to T0 and
/// compute the cross-factor interaction P&L.
///
/// The helper extracts a combined snapshot with the requested flags from
/// `market_t0`, restores it onto `market_t1`, reprices, and feeds the result
/// into `cross_interaction_pnl`. This is equivalent (and bit-identical) to the
/// explicit per-family snapshot-and-restore chaining used previously:
/// `restore_market` only touches flagged families, so a single combined
/// `(A | B)` restore from `market_t0` produces the same market as stacking an
/// `A` restore followed by a `B` restore.
///
/// Each call performs exactly one repricing; the caller is responsible for
/// adding to its repricing counter in a deterministic order (this function is
/// invoked from a parallel iterator, so it does not mutate shared counters).
#[allow(clippy::too_many_arguments)]
fn reprice_cross_factor(
    instrument: &Arc<dyn Instrument>,
    market_t0: &MarketContext,
    market_t1: &MarketContext,
    as_of_t1: Date,
    flags: MarketRestoreFlags,
    val_t1: Money,
    val_with_t0_a: Money,
    val_with_t0_b: Money,
) -> Result<Money> {
    let dependencies = instrument.market_dependencies()?;
    let credit_curve_ids = &dependencies.curves.credit_curves;
    let combined = MarketSnapshot::extract_with_credit_roles(market_t0, flags, credit_curve_ids);
    let market_combined = MarketSnapshot::restore_market(market_t1, &combined, flags);
    let reprice = reprice_instrument(instrument, &market_combined, as_of_t1)?;
    cross_interaction_pnl(val_t1, val_with_t0_a, val_with_t0_b, reprice)
}

/// Perform parallel P&L attribution for an instrument.
///
/// Each factor is isolated independently by restoring T₀ values for that
/// factor while keeping all others at T₁. Cross-effects and non-linearities
/// appear in the residual.
///
/// # Arguments
///
/// * `instrument` - Instrument to attribute
/// * `market_t0` - Market context at T₀
/// * `market_t1` - Market context at T₁
/// * `as_of_t0` - Valuation date at T₀
/// * `as_of_t1` - Valuation date at T₁
/// * `config` - Finstack configuration (for rounding, etc.)
/// * `execution_policy` - Sequential or parallel execution policy recorded in
///   the attribution metadata and used for factor repricing work.
///
/// # Returns
///
/// Complete P&L attribution with factor decomposition.
///
/// # Errors
///
/// Returns error if:
/// - Pricing fails at T₀ or T₁
/// - Currency conversion fails
/// - Market data is missing
///
/// # Examples
///
/// ```no_run
/// use finstack_quant_attribution::{
///     attribute_pnl, AttributionMethod, AttributionRequest, ExecutionPolicy,
/// };
/// use finstack_quant_valuations::instruments::Instrument;
/// use finstack_quant_valuations::instruments::rates::deposit::Deposit;
/// use finstack_quant_core::config::FinstackConfig;
/// use finstack_quant_core::currency::Currency;
/// use finstack_quant_core::market_data::context::MarketContext;
/// use finstack_quant_core::money::Money;
/// use std::sync::Arc;
/// use time::macros::date;
///
/// # fn main() -> Result<(), Box<dyn std::error::Error>> {
/// let as_of_t0 = date!(2025-01-15);
/// let as_of_t1 = date!(2025-01-16);
/// let market_t0 = MarketContext::new();
/// let market_t1 = MarketContext::new();
/// let config = FinstackConfig::default();
///
/// let instrument = Arc::new(
///     Deposit::builder()
///         .id("DEP-1D".into())
///         .notional(Money::from((1_000_000_i64, Currency::USD)))
///         .start_date(as_of_t0)
///         .maturity(as_of_t1)
///         .day_count(finstack_quant_core::dates::DayCount::Act360)
///         .discount_curve_id("USD-OIS".into())
///         .build()
///         .expect("deposit builder should succeed"),
/// ) as Arc<dyn Instrument>;
///
/// let request = AttributionRequest {
///     execution_policy: ExecutionPolicy::Parallel,
///     ..AttributionRequest::new(
///         &instrument,
///         &market_t0,
///         &market_t1,
///         as_of_t0,
///         as_of_t1,
///         &config,
///     )
/// };
/// let attribution = attribute_pnl(&AttributionMethod::Parallel, &request)?;
///
/// println!("Total P&L: {}", attribution.total_pnl);
/// println!("Carry: {}", attribution.carry);
/// println!("Rates: {}", attribution.rates_curves_pnl);
/// println!("Residual: {} ({:.2}%)",
///     attribution.residual,
///     attribution.meta.residual_pct
/// );
/// # Ok(())
/// # }
/// ```
///
/// # Credit-factor cascade
///
/// When `credit_factor_model` is `Some(_)` and the instrument has a
/// resolvable issuer + hazard exposure, the credit P&L is decomposed by a
/// **cumulative-bump cascade** that mirrors the waterfall path exactly:
///
/// 1. Start from `market_t0_credit` — the T1 market with the issuer's hazard
///    curves reverted to T0.
/// 2. For each parallel step (`generic`, level_k, `adder`), accumulate the
///    running bp shift `cumulative_bp` and reprice the instrument at
///    `market_t0_credit` shifted by `cumulative_bp` bp on the issuer's hazard
///    curves.
/// 3. For the final `curve_shape` step, snap those hazard curves to T1
///    wholesale — capturing any non-parallel (steepening / twist / term-
///    structure) component the parallel bumps could not explain.
/// 4. Each step's P&L is the **marginal contribution** `V_k − V_{k−1}`. The
///    sum telescopes to `V_final − base_credit_val ≡ credit_curves_pnl`, with
///    no residual back-solve and no cross-bp convexity leaking into
///    `curve_shape_pnl`.
///
/// See `credit_cascade::plan_credit_cascade` for the multi-curve issuer averaging caveat.
///
/// # Performance
///
/// When a `CreditFactorModel` is supplied with `L` hierarchy levels, the credit
/// cascade performs `L + 3` additional repricings (PC, one per level, Adder,
/// and CurveShape) compared to the single-step credit reprice without a model.
/// For typical L = 1–3 and portfolios of thousands of instruments this is
/// acceptable; consider `MetricsBased` or `Taylor` for cost-sensitive use
/// cases (they remain linear, no reprice).
#[tracing::instrument(skip_all, fields(instrument_id = %request.instrument.id(), method = "parallel"))]
pub(crate) fn attribute_pnl_parallel(request: &AttributionRequest<'_>) -> Result<PnlAttribution> {
    let AttributionRequest {
        instrument,
        market_t0,
        market_t1,
        as_of_t0,
        as_of_t1,
        config,
        execution_policy,
        full_cross_attribution,
        model_params_t0,
        credit_factor_model,
        credit_factor_detail_options,
        prepared_endpoints,
        ..
    } = *request;
    validate_attribution_period(as_of_t0, as_of_t1)?;
    let recalibration_provider = CachedRecalibrationProvider::new();
    let dependencies = instrument.market_dependencies()?;
    let credit_curve_ids = &dependencies.curves.credit_curves;

    // Endpoint repricings remain part of the workflow's accounting even when
    // the portfolio engine prepared them. The prepared path removes duplicate
    // calls; it does not change the logical cost represented in metadata.
    let mut num_repricings = 2;

    // Step 1: Price at T₀ and T₁
    // Use T₀ model parameters for T₀ valuation if available
    let instrument_t0 = if let Some(params) = model_params_t0 {
        model_params::with_model_params(instrument, params)?
    } else {
        Arc::clone(instrument)
    };
    let (val_t0, val_t1) = if let Some(endpoints) = prepared_endpoints {
        endpoints
    } else {
        (
            reprice_instrument(&instrument_t0, market_t0, as_of_t0)?,
            reprice_instrument(instrument, market_t1, as_of_t1)?,
        )
    };

    // Total P&L (with FX translation)
    let total_pnl = compute_pnl_with_fx(
        val_t0,
        val_t1,
        val_t1.currency(),
        market_t0,
        market_t1,
        as_of_t0,
        as_of_t1,
    )?;

    let mut attribution = init_attribution(
        total_pnl,
        instrument.id(),
        as_of_t0,
        as_of_t1,
        AttributionMethod::Parallel,
        Some(config),
    );
    // Policy-visibility invariant: stamp the execution policy the
    // attribution ran under (workspace rule: results carry the parallel flag).
    attribution.meta.execution_policy = Some(execution_policy);
    let factor_use = InstrumentFactorUse::of(instrument.as_ref());

    let mut val_with_t0_rates: Option<Money> = None;
    let mut val_with_t0_credit: Option<Money> = None;
    let mut val_with_t0_fx: Option<Money> = None;
    let mut val_with_t0_vol: Option<Money> = None;
    let mut val_with_t0_scalars: Option<Money> = None;
    let mut credit_snapshot = MarketSnapshot::default();

    // Step 2: Carry attribution (time decay + accruals + roll-down)
    //
    // METHODOLOGY: Price at T₁ date with T₀ market (frozen curves).
    // This captures the combined effect of:
    //   - Theta (pure time decay): coupon accrual, option decay, funding cost
    //   - Roll-down: benefit from moving down a positively-sloped curve
    //
    // These sub-components are separated in metrics-based attribution (where
    // Theta is pre-computed), but in parallel attribution the total carry
    // is reported. Use `carry_detail` for the decomposition when available.
    //
    // FX CONVENTION: attribution reports in the instrument's NATIVE pricing
    // currency, so the `compute_pnl` conversion here is a same-currency
    // identity (no FX rate is ever applied on this path). Reporting-currency
    // translation happens exclusively in `translate_to_target_currency`; the FX
    // factor (Step 7) captures only the *pricing impact* of swapping the FX
    // matrix inside the pricer.
    // Carry freezes the market at T₀: it reprices at the T₁ date against the
    // unchanged T₀ market context, so the T₀ context is used directly rather
    // than deep-cloned (the reprice and carry-input helpers only borrow it).
    //
    // FIXINGS UNDER CARRY (prior fix): the frozen T₀ market has no
    // fixing for the T₁ date, so seasoned floating-rate pricing falls back to
    // last-observation-carried-forward (`ScalarTimeSeries::value_on` LOCF) —
    // i.e. carry assumes the latest known fixing persists, the "unchanged
    // market" convention. The actual reset P&L (yesterday's projection
    // becoming today's fixing) is attributed to the rates factor: fixing
    // series restore with the FORWARD family in `MarketSnapshot`.
    //
    // Carry must isolate *pure time passage*: it reprices the T₀-parameter
    // instrument (`instrument_t0`), not `instrument`. Using `instrument` here
    // would fold any T₀→T₁ model-parameter drift into theta, since `val_t0`
    // was itself priced with `instrument_t0`.
    let val_carry = reprice_instrument(&instrument_t0, market_t0, as_of_t1)?;
    num_repricings += 1;

    let theta = compute_pnl(val_t0, val_carry, val_t1.currency(), market_t1, as_of_t1)?;

    let carry_inputs = total_return_carry_inputs(
        instrument_t0.as_ref(),
        market_t0,
        as_of_t0,
        as_of_t1,
        val_t1.currency(),
    )?;
    // Merge diagnostics BEFORE moving carry_inputs into apply_total_return_carry.
    for w in &carry_inputs.warnings {
        attribution.meta.notes.push(w.clone());
    }
    // `total_return_carry_inputs` performed extra `price_with_metrics`
    // repricings (Accrued×2, YTM, flat-curve value×2) — count one.
    num_repricings += 1;

    apply_total_return_carry(&mut attribution, theta, carry_inputs)?;

    if full_cross_attribution {
        let discount_snap = extract_if_used(
            factor_use.rates,
            market_t0,
            MarketRestoreFlags::DISCOUNT,
            credit_curve_ids,
        );
        let forward_snap = extract_if_used(
            factor_use.rates,
            market_t0,
            MarketRestoreFlags::FORWARD,
            credit_curve_ids,
        );
        let credit_snap_ext = extract_if_used(
            factor_use.credit,
            market_t0,
            MarketRestoreFlags::CREDIT,
            credit_curve_ids,
        );
        let inflation_snap = extract_if_used(
            factor_use.inflation,
            market_t0,
            MarketRestoreFlags::INFLATION,
            credit_curve_ids,
        );
        let correlation_snap = MarketSnapshot::extract_with_credit_roles(
            market_t0,
            MarketRestoreFlags::CORRELATION,
            credit_curve_ids,
        );
        let fx_snap = extract_if_used(
            factor_use.fx,
            market_t0,
            MarketRestoreFlags::FX,
            credit_curve_ids,
        );
        let vol_snap = extract_if_used(
            factor_use.volatility,
            market_t0,
            MarketRestoreFlags::VOL,
            credit_curve_ids,
        );
        let scalars_snap = extract_if_used(
            factor_use.scalars,
            market_t0,
            MarketRestoreFlags::SCALARS,
            credit_curve_ids,
        );

        let mut factor_specs = Vec::new();

        if !discount_snap.discount_curves.is_empty() {
            factor_specs.push(ParallelLatentFactorSpec::Market {
                factor: ParallelRestoredFactor::Discount,
                flags: MarketRestoreFlags::DISCOUNT,
                snapshot: Box::new(discount_snap),
            });
        }
        if restored_factor_has_data(ParallelRestoredFactor::Forward, &forward_snap) {
            factor_specs.push(ParallelLatentFactorSpec::Market {
                factor: ParallelRestoredFactor::Forward,
                flags: MarketRestoreFlags::FORWARD,
                snapshot: Box::new(forward_snap),
            });
        }
        if restored_factor_has_data(ParallelRestoredFactor::Credit, &credit_snap_ext) {
            factor_specs.push(ParallelLatentFactorSpec::Market {
                factor: ParallelRestoredFactor::Credit,
                flags: MarketRestoreFlags::CREDIT,
                snapshot: Box::new(credit_snap_ext.clone()),
            });
        }
        // Retain the credit snapshot for the cascade reprice later (Step 4).
        credit_snapshot = credit_snap_ext;
        if !inflation_snap.inflation_curves.is_empty() {
            factor_specs.push(ParallelLatentFactorSpec::Market {
                factor: ParallelRestoredFactor::Inflation,
                flags: MarketRestoreFlags::INFLATION,
                snapshot: Box::new(inflation_snap),
            });
        }
        if !correlation_snap.base_correlation_curves.is_empty() {
            factor_specs.push(ParallelLatentFactorSpec::Market {
                factor: ParallelRestoredFactor::Correlations,
                flags: MarketRestoreFlags::CORRELATION,
                snapshot: Box::new(correlation_snap),
            });
        }
        if fx_snap.fx.is_some() {
            factor_specs.push(ParallelLatentFactorSpec::Market {
                factor: ParallelRestoredFactor::FX,
                flags: MarketRestoreFlags::FX,
                snapshot: Box::new(fx_snap),
            });
        }
        // Audit M4: gate on the shared helper, which also checks SABR vol
        // cubes, FX delta-quoted surfaces and vol-index curves — a cube-only
        // vol market must still receive vol repricing.
        if restored_factor_has_data(ParallelRestoredFactor::Volatility, &vol_snap) {
            factor_specs.push(ParallelLatentFactorSpec::Market {
                factor: ParallelRestoredFactor::Volatility,
                flags: MarketRestoreFlags::VOL,
                snapshot: Box::new(vol_snap),
            });
        }
        if restored_factor_has_data(ParallelRestoredFactor::MarketScalars, &scalars_snap) {
            factor_specs.push(ParallelLatentFactorSpec::Market {
                factor: ParallelRestoredFactor::MarketScalars,
                flags: MarketRestoreFlags::SCALARS,
                snapshot: Box::new(scalars_snap),
            });
        }

        let params_t0 = model_params_t0
            .cloned()
            .unwrap_or_else(|| instrument.model_params_snapshot());
        if !matches!(params_t0, ModelParamsSnapshot::None) {
            factor_specs.push(ParallelLatentFactorSpec::ModelParams {
                snapshot: params_t0,
            });
        }

        let reprice_first_order =
            |spec: &ParallelLatentFactorSpec| -> Result<FirstOrderRepriceResult> {
                match spec {
                    ParallelLatentFactorSpec::Market {
                        factor,
                        flags,
                        snapshot,
                    } => {
                        let market_with_t0 =
                            MarketSnapshot::restore_market(market_t1, snapshot, *flags);
                        let reprice = reprice_instrument(instrument, &market_with_t0, as_of_t1)?;
                        let pnl = if matches!(factor, ParallelRestoredFactor::FX) {
                            compute_pnl_with_fx(
                                reprice,
                                val_t1,
                                val_t1.currency(),
                                market_t0,
                                market_t1,
                                as_of_t0,
                                as_of_t1,
                            )?
                        } else {
                            compute_pnl(reprice, val_t1, val_t1.currency(), market_t1, as_of_t1)?
                        };
                        Ok(FirstOrderRepriceResult {
                            spec: spec.clone(),
                            pnl,
                            reprice_val: reprice,
                        })
                    }
                    ParallelLatentFactorSpec::ModelParams { snapshot } => {
                        let instrument_with_t0_params =
                            model_params::with_model_params(instrument, snapshot)?;
                        let reprice =
                            reprice_instrument(&instrument_with_t0_params, market_t1, as_of_t1)?;
                        let pnl =
                            compute_pnl(reprice, val_t1, val_t1.currency(), market_t1, as_of_t1)?;
                        Ok(FirstOrderRepriceResult {
                            spec: spec.clone(),
                            pnl,
                            reprice_val: reprice,
                        })
                    }
                }
            };
        let first_order_results =
            try_map_policy(execution_policy, &factor_specs, reprice_first_order)?;

        let mut val_with_t0_discount: Option<Money> = None;
        let mut val_with_t0_forward: Option<Money> = None;
        let mut val_with_t0_inflation: Option<Money> = None;
        let mut val_with_t0_correlation: Option<Money> = None;
        let mut val_with_t0_params: Option<Money> = None;

        for res in first_order_results {
            num_repricings += 1;
            match &res.spec {
                ParallelLatentFactorSpec::Market { factor, .. } => match factor {
                    ParallelRestoredFactor::Discount => {
                        val_with_t0_discount = Some(res.reprice_val);
                        attribution.rates_curves_pnl =
                            attribution.rates_curves_pnl.checked_add(res.pnl)?;
                    }
                    ParallelRestoredFactor::Forward => {
                        val_with_t0_forward = Some(res.reprice_val);
                        attribution.rates_curves_pnl =
                            attribution.rates_curves_pnl.checked_add(res.pnl)?;
                    }
                    ParallelRestoredFactor::Credit => {
                        val_with_t0_credit = Some(res.reprice_val);
                        attribution.credit_curves_pnl = res.pnl;
                    }
                    ParallelRestoredFactor::Inflation => {
                        val_with_t0_inflation = Some(res.reprice_val);
                        attribution.inflation_curves_pnl = res.pnl;
                    }
                    ParallelRestoredFactor::Correlations => {
                        val_with_t0_correlation = Some(res.reprice_val);
                        attribution.correlations_pnl = res.pnl;
                    }
                    ParallelRestoredFactor::FX => {
                        val_with_t0_fx = Some(res.reprice_val);
                        attribution.fx_pnl = res.pnl;
                    }
                    ParallelRestoredFactor::Volatility => {
                        val_with_t0_vol = Some(res.reprice_val);
                        attribution.vol_pnl = res.pnl;
                    }
                    ParallelRestoredFactor::MarketScalars => {
                        val_with_t0_scalars = Some(res.reprice_val);
                        attribution.market_scalars_pnl = res.pnl;
                    }
                    // The full-cross path pushes Discount/Forward separately,
                    // never the combined Rates spec. A future edit adding it
                    // must extend the val_with_t0 bookkeeping — soft-skip with
                    // a warn instead of panicking in a deny-panic crate.
                    ParallelRestoredFactor::Rates => {
                        tracing::warn!(
                            "unexpected combined Rates spec in full-cross first-order loop; \
                             result ignored (Discount/Forward are tracked separately)"
                        );
                    }
                },
                ParallelLatentFactorSpec::ModelParams { .. } => {
                    val_with_t0_params = Some(res.reprice_val);
                    attribution.model_params_pnl = res.pnl;
                }
            }
        }

        if val_with_t0_fx.is_some() {
            stamp_fx_policy(
                &mut attribution,
                val_t1.currency(),
                "Combined FX exposure and translation P&L (see parallel.rs for details)",
            );
        }

        let mut active_list = Vec::new();
        if let Some(val) = val_with_t0_discount {
            active_list.push((ActiveFactorKind::Discount, "Discount", val));
        }
        if let Some(val) = val_with_t0_forward {
            active_list.push((ActiveFactorKind::Forward, "Forward", val));
        }
        if let Some(val) = val_with_t0_credit {
            active_list.push((ActiveFactorKind::Credit, "Credit", val));
        }
        if let Some(val) = val_with_t0_inflation {
            active_list.push((ActiveFactorKind::Inflation, "Inflation", val));
        }
        if let Some(val) = val_with_t0_correlation {
            active_list.push((ActiveFactorKind::Correlations, "Correlations", val));
        }
        if let Some(val) = val_with_t0_fx {
            active_list.push((ActiveFactorKind::FX, "FX", val));
        }
        if let Some(val) = val_with_t0_vol {
            active_list.push((ActiveFactorKind::Volatility, "Vol", val));
        }
        if let Some(val) = val_with_t0_scalars {
            active_list.push((ActiveFactorKind::MarketScalars, "Spot", val));
        }
        if let Some(val) = val_with_t0_params {
            active_list.push((ActiveFactorKind::ModelParameters, "ModelParameters", val));
        }

        let mut cross_specs = Vec::new();
        for i in 0..active_list.len() {
            for j in (i + 1)..active_list.len() {
                let (kind_a, label_a, val_a) = active_list[i];
                let (kind_b, label_b, val_b) = active_list[j];
                let label = format!("{}×{}", label_a, label_b);
                cross_specs.push((label, kind_a, kind_b, val_a, val_b));
            }
        }

        let reprice_cross_spec = |(label, kind_a, kind_b, val_a, val_b): &(
            String,
            ActiveFactorKind,
            ActiveFactorKind,
            Money,
            Money,
        )|
         -> Result<(String, Money)> {
            let pnl = if matches!(kind_a, ActiveFactorKind::ModelParameters)
                || matches!(kind_b, ActiveFactorKind::ModelParameters)
            {
                let market_factor = if matches!(kind_a, ActiveFactorKind::ModelParameters) {
                    *kind_b
                } else {
                    *kind_a
                };
                let val_market = if matches!(kind_a, ActiveFactorKind::ModelParameters) {
                    *val_b
                } else {
                    *val_a
                };
                let val_params = if matches!(kind_a, ActiveFactorKind::ModelParameters) {
                    *val_a
                } else {
                    *val_b
                };

                let m_flags = get_restore_flags(market_factor);
                let combined_snap =
                    MarketSnapshot::extract_with_credit_roles(market_t0, m_flags, credit_curve_ids);
                let market_combined =
                    MarketSnapshot::restore_market(market_t1, &combined_snap, m_flags);
                let reprice_both = reprice_instrument(&instrument_t0, &market_combined, as_of_t1)?;
                cross_interaction_pnl(val_t1, val_market, val_params, reprice_both)?
            } else {
                let combined_flags = get_restore_flags(*kind_a) | get_restore_flags(*kind_b);
                reprice_cross_factor(
                    instrument,
                    market_t0,
                    market_t1,
                    as_of_t1,
                    combined_flags,
                    val_t1,
                    *val_a,
                    *val_b,
                )?
            };
            Ok((label.clone(), pnl))
        };
        let cross_results = try_map_policy(execution_policy, &cross_specs, reprice_cross_spec)?;

        let mut cross_total = 0.0;
        let mut cross_by_pair: IndexMap<String, Money> = IndexMap::new();
        for (label, pnl) in cross_results {
            num_repricings += 1;
            record_cross_pair(&label, pnl, &mut cross_total, &mut cross_by_pair);
        }

        if !cross_by_pair.is_empty() {
            attribution.cross_factor_pnl = Money::new(cross_total, val_t1.currency())?;
            attribution.cross_factor_detail = Some(CrossFactorDetail {
                total: attribution.cross_factor_pnl,
                by_pair: cross_by_pair,
            });
        }
    } else {
        // Steps 3-6: ordinary restored-market factors before FX. Order is
        // preserved exactly so repricing counts and first-error behavior stay
        // stable.
        let pre_fx_specs = [
            (ParallelRestoredFactor::Rates, MarketRestoreFlags::RATES),
            (ParallelRestoredFactor::Credit, MarketRestoreFlags::CREDIT),
            (
                ParallelRestoredFactor::Inflation,
                MarketRestoreFlags::INFLATION,
            ),
            (
                ParallelRestoredFactor::Correlations,
                MarketRestoreFlags::CORRELATION,
            ),
        ];
        let pre_fx_specs: Vec<_> = pre_fx_specs
            .into_iter()
            .filter(|(factor, _)| restored_factor_is_used(*factor, factor_use))
            .collect();
        let eval_pre_fx = |(factor, flags): &(ParallelRestoredFactor, MarketRestoreFlags)| {
            let snapshot =
                MarketSnapshot::extract_with_credit_roles(market_t0, *flags, credit_curve_ids);
            let has_data = restored_factor_has_data(*factor, &snapshot);
            reprice_factor_restored_once(
                instrument, market_t1, &snapshot, *flags, has_data, as_of_t1, val_t1,
            )
            .map(|output| RestoredFactorEval {
                factor: *factor,
                snapshot,
                output,
            })
        };
        let pre_fx_evals = try_map_policy(execution_policy, &pre_fx_specs, eval_pre_fx)?;
        let mut val_with_t0_inflation: Option<Money> = None;
        let mut val_with_t0_correlation: Option<Money> = None;
        for eval in pre_fx_evals {
            let RestoredFactorEval {
                factor,
                snapshot,
                output,
            } = eval;
            if matches!(factor, ParallelRestoredFactor::Credit) {
                credit_snapshot = snapshot;
            }
            if let Some((pnl, reprice)) = output {
                num_repricings += 1;
                match factor {
                    ParallelRestoredFactor::Rates => {
                        attribution.rates_curves_pnl = pnl;
                        val_with_t0_rates = Some(reprice);
                    }
                    ParallelRestoredFactor::Credit => {
                        attribution.credit_curves_pnl = pnl;
                        val_with_t0_credit = Some(reprice);
                    }
                    ParallelRestoredFactor::Inflation => {
                        attribution.inflation_curves_pnl = pnl;
                        val_with_t0_inflation = Some(reprice);
                    }
                    ParallelRestoredFactor::Correlations => {
                        attribution.correlations_pnl = pnl;
                        val_with_t0_correlation = Some(reprice);
                    }
                    _ => {}
                }
            }
        }

        // Step 7: FX attribution
        let fx_snapshot = if factor_use.fx {
            MarketSnapshot::extract_with_credit_roles(
                market_t0,
                MarketRestoreFlags::FX,
                credit_curve_ids,
            )
        } else {
            MarketSnapshot::default()
        };
        if fx_snapshot.fx.is_some() {
            let market_with_t0_fx =
                MarketSnapshot::restore_market(market_t1, &fx_snapshot, MarketRestoreFlags::FX);
            let fx_reprice = reprice_instrument(instrument, &market_with_t0_fx, as_of_t1)?;
            num_repricings += 1;
            val_with_t0_fx = Some(fx_reprice);

            // FX-exposure (pricing-impact) P&L: the FX matrix was restored to
            // T₀ inside the pricer for `fx_reprice`. Both values are in the
            // instrument's native currency, so the `compute_pnl_with_fx`
            // conversions below are same-currency identities — the with-fx
            // variant matters only for external callers with target ≠ native;
            // reporting-currency translation lives in `translate_to_target_currency`.
            attribution.fx_pnl = compute_pnl_with_fx(
                fx_reprice,
                val_t1,
                val_t1.currency(),
                market_t0,
                market_t1,
                as_of_t0,
                as_of_t1,
            )?;

            // Stamp FX policy metadata for audit trail
            stamp_fx_policy(
                &mut attribution,
                val_t1.currency(),
                "Combined FX exposure and translation P&L (see parallel.rs for details)",
            );
        }

        // Step 8: Volatility attribution.
        if restored_factor_is_used(ParallelRestoredFactor::Volatility, factor_use) {
            let flags = MarketRestoreFlags::VOL;
            let snapshot =
                MarketSnapshot::extract_with_credit_roles(market_t0, flags, credit_curve_ids);
            // Audit M4: use the shared has-data helper so cube-only /
            // FX-delta-only / vol-index-only markets are not silently skipped.
            let has_vol = restored_factor_has_data(ParallelRestoredFactor::Volatility, &snapshot);
            if let Some((pnl, reprice)) = reprice_factor_restored_once(
                instrument, market_t1, &snapshot, flags, has_vol, as_of_t1, val_t1,
            )? {
                num_repricings += 1;
                attribution.vol_pnl = pnl;
                val_with_t0_vol = Some(reprice);
            }
        }

        // Step 9: Model parameters attribution
        let params_t0 = model_params_t0
            .cloned()
            .unwrap_or_else(|| instrument.model_params_snapshot());
        if !matches!(params_t0, ModelParamsSnapshot::None) {
            // Create instrument with T₀ parameters
            match model_params::with_model_params(instrument, &params_t0) {
                Ok(instrument_with_t0_params) => {
                    // Reprice with T₁ market
                    match reprice_instrument(&instrument_with_t0_params, market_t1, as_of_t1) {
                        Ok(val_with_t0_params) => {
                            num_repricings += 1;

                            attribution.model_params_pnl = compute_pnl(
                                val_with_t0_params,
                                val_t1,
                                val_t1.currency(),
                                market_t1,
                                as_of_t1,
                            )?;
                        }
                        Err(e) => {
                            attribution.meta.notes.push(format!(
                                "Model parameters attribution: repricing failed - {}",
                                e
                            ));
                        }
                    }
                }
                Err(e) => {
                    attribution.meta.notes.push(format!(
                        "Model parameters attribution: parameter modification failed - {}",
                        e
                    ));
                }
            }
        }

        // Step 10: Market scalars attribution.
        if restored_factor_is_used(ParallelRestoredFactor::MarketScalars, factor_use) {
            let flags = MarketRestoreFlags::SCALARS;
            let snapshot =
                MarketSnapshot::extract_with_credit_roles(market_t0, flags, credit_curve_ids);
            let has_scalars =
                restored_factor_has_data(ParallelRestoredFactor::MarketScalars, &snapshot);
            if let Some((pnl, reprice)) = reprice_factor_restored_once(
                instrument,
                market_t1,
                &snapshot,
                flags,
                has_scalars,
                as_of_t1,
                val_t1,
            )? {
                num_repricings += 1;
                attribution.market_scalars_pnl = pnl;
                val_with_t0_scalars = Some(reprice);
            }
        }

        let mut cross_total = 0.0;
        let mut cross_by_pair: IndexMap<String, Money> = IndexMap::new();

        // (pair_label, flag_A, flag_B, reprice_A, reprice_B) — order preserved
        // exactly as before for reduction-order stability.
        type CrossSpec<'a> = (
            &'a str,
            MarketRestoreFlags,
            MarketRestoreFlags,
            Option<Money>,
            Option<Money>,
        );
        let cross_specs: [CrossSpec<'_>; 9] = [
            (
                "Rates×Credit",
                MarketRestoreFlags::RATES,
                MarketRestoreFlags::CREDIT,
                val_with_t0_rates,
                val_with_t0_credit,
            ),
            (
                "Rates×Vol",
                MarketRestoreFlags::RATES,
                MarketRestoreFlags::VOL,
                val_with_t0_rates,
                val_with_t0_vol,
            ),
            (
                "Spot×Vol",
                MarketRestoreFlags::SCALARS,
                MarketRestoreFlags::VOL,
                val_with_t0_scalars,
                val_with_t0_vol,
            ),
            (
                "Spot×Credit",
                MarketRestoreFlags::CREDIT,
                MarketRestoreFlags::SCALARS,
                val_with_t0_scalars,
                val_with_t0_credit,
            ),
            (
                "FX×Vol",
                MarketRestoreFlags::FX,
                MarketRestoreFlags::VOL,
                val_with_t0_fx,
                val_with_t0_vol,
            ),
            (
                "FX×Rates",
                MarketRestoreFlags::RATES,
                MarketRestoreFlags::FX,
                val_with_t0_fx,
                val_with_t0_rates,
            ),
            // Credit×Vol captures convertibles, where equity volatility drives
            // the conversion option while credit curves discount the bond floor.
            (
                "Credit×Vol",
                MarketRestoreFlags::CREDIT,
                MarketRestoreFlags::VOL,
                val_with_t0_credit,
                val_with_t0_vol,
            ),
            // Rates×Inflation captures linkers (real-rate exposure
            // is the product of nominal rates and the CPI projection) and
            // Credit×Correlations captures tranches (loss allocation is
            // jointly driven by hazard levels and base correlation). Both are
            // appended after the historical seven pairs so the preceding pair
            // reduction order stays stable.
            (
                "Rates×Inflation",
                MarketRestoreFlags::RATES,
                MarketRestoreFlags::INFLATION,
                val_with_t0_rates,
                val_with_t0_inflation,
            ),
            (
                "Credit×Correlations",
                MarketRestoreFlags::CREDIT,
                MarketRestoreFlags::CORRELATION,
                val_with_t0_credit,
                val_with_t0_correlation,
            ),
        ];

        // Each cross-factor block is an independent full revaluation. Reprice them
        // in parallel, then reduce in the fixed `cross_specs` order so the result
        // is bit-identical to the previous sequential loop.
        let reprice_default_cross = |(pair, flag_a, flag_b, reprice_a, reprice_b): &CrossSpec<
            '_,
        >|
         -> Result<Option<(String, Money)>> {
            let (Some(val_a), Some(val_b)) = (*reprice_a, *reprice_b) else {
                return Ok(None);
            };
            let pnl = reprice_cross_factor(
                instrument,
                market_t0,
                market_t1,
                as_of_t1,
                *flag_a | *flag_b,
                val_t1,
                val_a,
                val_b,
            )?;
            Ok(Some(((*pair).to_string(), pnl)))
        };
        let cross_results = try_map_policy(execution_policy, &cross_specs, reprice_default_cross)?;
        for result in cross_results.into_iter().flatten() {
            let (pair, pnl) = result;
            num_repricings += 1;
            record_cross_pair(&pair, pnl, &mut cross_total, &mut cross_by_pair);
        }

        if !cross_by_pair.is_empty() {
            attribution.cross_factor_pnl = Money::new(cross_total, val_t1.currency())?;
            attribution.cross_factor_detail = Some(CrossFactorDetail {
                total: attribution.cross_factor_pnl,
                by_pair: cross_by_pair,
            });
        }
    }

    // Step 10c: Credit-factor hierarchy detail via cumulative-bump cascade.
    //
    // The cascade mirrors the waterfall semantics (see `waterfall::apply_credit_cascade`):
    // each step's market is built by accumulating the previous parallel-bump
    // bp shifts plus the current step's bp from the same fixed
    // `market_t0_credit` base. The step's P&L is then the *marginal*
    // contribution `V_k − V_{k−1}`, which telescopes so that
    // `Σ steps ≡ V_final − V_0`. With the `CurveShape` step snapping the
    // issuer's hazard curves to T1 at the end, `V_final` reduces to the
    // instrument's T1 valuation (modulo non-issuer hazard curves) and the
    // telescope closes to `credit_curves_pnl` without any residual back-solve.
    //
    // This replaces an earlier "marginal-from-same-base" formulation in which
    // each step bumped the T0-hazard base by *only* its own `delta_bp` and
    // `curve_shape_pnl` was back-solved as `credit_curves_pnl − Σ steps`.
    // For instruments with non-trivial CS-gamma that approach silently routed
    // cross-bp convexity into `curve_shape_pnl`, firing the curve-shape
    // tracing warning even when the hazard move was perfectly parallel. The
    // cumulative form gives a single consistent decomposition across both
    // parallel and waterfall methods.
    if let Some(model) = credit_factor_model {
        match plan_credit_cascade(model, instrument, market_t0, market_t1)? {
            Some(cascade) => {
                // Build a T1-base market with T0 hazard for the issuer's curves.
                // Re-use the credit snapshot extracted in Step 4.
                let market_t0_credit = MarketSnapshot::restore_market(
                    market_t1,
                    &credit_snapshot,
                    MarketRestoreFlags::CREDIT,
                );

                // Base value for the cascade: the instrument priced at T1
                // markets with the issuer's hazard curves reverted to T0 —
                // exactly the reference point `credit_curves_pnl` is measured
                // against. Reuse the Step-4 credit reprice when present;
                // otherwise reprice once here.
                let base_credit_val = match val_with_t0_credit {
                    Some(v) => v,
                    None => {
                        num_repricings += 1;
                        reprice_instrument(instrument, &market_t0_credit, as_of_t1)?
                    }
                };

                // Precompute the cumulative bp for each parallel step. The
                // `CurveShape` step carries no bp (it is a snap-to-T1) and
                // contributes `None`; everywhere else `Some(running_bp)`.
                let cumulative_bp: Vec<Option<f64>> = {
                    let mut running_bp = 0.0_f64;
                    cascade
                        .steps
                        .iter()
                        .map(|step| {
                            if matches!(
                                step.kind,
                                super::credit_cascade::CreditStepKind::CurveShape
                            ) {
                                None
                            } else {
                                running_bp += step.delta_bp;
                                Some(running_bp)
                            }
                        })
                        .collect()
                };

                // Reprice each step's end-state market. Standalone attribution
                // can fan these out; portfolio callers pass `Serial` so the
                // outer position loop owns Rayon.
                let reprice_cascade_step =
                    |(step, cumulative_bp): (&CreditCascadeStep, &Option<f64>)| -> Result<Money> {
                        let market_step = match step.kind {
                            super::credit_cascade::CreditStepKind::CurveShape => snap_hazard_to_t1(
                                &market_t0_credit,
                                market_t1,
                                &cascade.hazard_curve_ids,
                            ),
                            _ => shift_credit_curves_par_spread(
                                market_t0,
                                &market_t0_credit,
                                &cascade.hazard_curve_ids,
                                cascade.discount_curve_id.as_ref(),
                                cumulative_bp.unwrap_or(0.0),
                                &recalibration_provider,
                            )?,
                        };
                        reprice_instrument(instrument, &market_step, as_of_t1)
                    };
                let step_values: Vec<Money> = try_map_policy_zip(
                    execution_policy,
                    &cascade.steps,
                    &cumulative_bp,
                    reprice_cascade_step,
                )?;
                num_repricings += step_values.len();

                // Telescope to per-step P&Ls: V_k − V_{k−1}, V_0 = base_credit_val.
                // The sum telescopes to V_final − base_credit_val, which is
                // `credit_curves_pnl` when the CurveShape snap leaves us at the
                // T1 hazard state (the standard case).
                let mut step_pnls: Vec<Money> = Vec::with_capacity(cascade.steps.len());
                let mut prev = base_credit_val;
                for v in &step_values {
                    let pnl = compute_pnl(prev, *v, val_t1.currency(), market_t1, as_of_t1)?;
                    step_pnls.push(pnl);
                    prev = *v;
                }

                let detail = build_credit_factor_attribution(
                    model,
                    &cascade,
                    credit_factor_detail_options,
                    &step_pnls,
                )?;
                attribution.credit_factor_detail = Some(detail);
            }
            None => {
                tracing::warn!(
                    instrument_id = instrument.id(),
                    method = "parallel",
                    "Credit factor model supplied but credit cascade could not be planned"
                );
                attribution.meta.notes.push(format!(
                    "credit_factor_model supplied but no resolvable issuer/hazard cascade for {}; credit_factor_detail omitted",
                    instrument.id()
                ));
            }
        }
    }

    note_skipped_empty_t0_factors(
        &mut attribution,
        market_t0,
        market_t1,
        factor_use,
        credit_curve_ids,
    );

    finalize_attribution(
        &mut attribution,
        instrument.id(),
        "parallel",
        num_repricings,
        1.0,
        0.1,
    );

    Ok(attribution)
}

#[cfg(test)]
mod tests {
    #[allow(dead_code, unused_imports)]
    mod test_utils {
        include!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/tests/support/attribution_test_utils.rs"
        ));
    }

    use super::*;
    use finstack_quant_core::config::FinstackConfig;
    use finstack_quant_core::currency::Currency;
    use finstack_quant_core::dates::Date;
    use finstack_quant_core::market_data::term_structures::DiscountCurve;
    use finstack_quant_core::market_data::term_structures::HazardCurve;
    use finstack_quant_core::math::interp::InterpStyle;
    use finstack_quant_core::money::Money;
    use std::sync::OnceLock;
    use test_utils::TestInstrument;
    use time::macros::date;

    #[derive(Clone)]
    struct RatesCreditInteractionInstrument {
        id: String,
    }

    finstack_quant_valuations::impl_empty_cashflow_provider!(
        RatesCreditInteractionInstrument,
        finstack_quant_cashflows::builder::CashflowRepresentation::NoResidual
    );

    impl RatesCreditInteractionInstrument {
        fn new(id: &str) -> Self {
            Self { id: id.to_string() }
        }
    }

    impl Instrument for RatesCreditInteractionInstrument {
        fn id(&self) -> &str {
            &self.id
        }

        fn key(&self) -> finstack_quant_valuations::pricer::InstrumentType {
            finstack_quant_valuations::pricer::InstrumentType::Bond
        }

        fn as_any(&self) -> &dyn std::any::Any {
            self
        }

        fn as_any_mut(&mut self) -> &mut dyn std::any::Any {
            self
        }

        fn attributes(&self) -> &finstack_quant_valuations::instruments::Attributes {
            static ATTRS: OnceLock<finstack_quant_valuations::instruments::Attributes> =
                OnceLock::new();
            ATTRS.get_or_init(finstack_quant_valuations::instruments::Attributes::default)
        }

        fn attributes_mut(&mut self) -> &mut finstack_quant_valuations::instruments::Attributes {
            unreachable!("RatesCreditInteractionInstrument::attributes_mut should not be called")
        }

        fn clone_box(&self) -> Box<dyn Instrument> {
            Box::new(self.clone())
        }

        fn market_dependencies(
            &self,
        ) -> finstack_quant_core::Result<finstack_quant_valuations::instruments::MarketDependencies>
        {
            let mut deps = finstack_quant_valuations::instruments::MarketDependencies::new();
            deps.add_discount_curve("USD-OIS");
            deps.add_credit_curve("ACME-HAZ");
            Ok(deps)
        }

        fn base_value(&self, market: &MarketContext, _as_of: Date) -> Result<Money> {
            let rate = market.get_discount("USD-OIS")?.zero(1.0);
            let hazard = market.get_hazard("ACME-HAZ")?.hazard_rate(1.0);
            Ok(
                Money::new(1_000_000.0 * rate * hazard, Currency::USD)
                    .expect("valid money fixture"),
            )
        }

        fn price_with_metrics(
            &self,
            market: &MarketContext,
            as_of: Date,
            _metrics: &[finstack_quant_valuations::metrics::MetricId],
            _options: finstack_quant_valuations::instruments::PricingOptions,
        ) -> finstack_quant_valuations::Result<finstack_quant_valuations::results::ValuationResult>
        {
            Ok(
                finstack_quant_valuations::results::ValuationResult::stamped(
                    self.id(),
                    as_of,
                    self.value(market, as_of)?,
                ),
            )
        }
    }

    #[test]
    fn test_parallel_attribution_simple() {
        let as_of_t0 = date!(2025 - 01 - 15);
        let as_of_t1 = date!(2025 - 01 - 16);

        // Create test instrument with different values at T0 and T1
        let _instrument_t0 = Arc::new(TestInstrument::new(
            "TEST-001",
            Money::from((1000_i64, Currency::USD)),
        ));

        // Simulate P&L by creating a different value for T1
        // In practice, the same instrument would be repriced with different markets
        let val_t0 = Money::from((1000_i64, Currency::USD));
        let val_t1 = Money::from((1100_i64, Currency::USD));

        // Create minimal markets
        let _market_t0 = MarketContext::new();
        let _market_t1 = MarketContext::new();
        let _config = FinstackConfig::default();

        // For this test, we'll manually construct the attribution since our test
        // instrument returns fixed values
        let total_pnl = val_t1
            .checked_sub(val_t0)
            .expect("PNL calculation should succeed in test");
        let attribution = PnlAttribution::new(
            total_pnl,
            "TEST-001",
            as_of_t0,
            as_of_t1,
            AttributionMethod::Parallel,
        );

        assert_eq!(attribution.total_pnl.amount(), 100.0);
        assert_eq!(attribution.residual.amount(), 100.0); // Initially all in residual
    }

    #[test]
    fn test_parallel_attribution_with_curve_change() {
        let as_of_t0 = date!(2025 - 01 - 15);
        let as_of_t1 = date!(2025 - 01 - 16);

        // Create discount curves at T0 and T1
        let curve_t0 = DiscountCurve::builder("USD-OIS")
            .base_date(as_of_t0)
            .knots(vec![(0.0, 1.0), (1.0, 0.98)])
            .interp(InterpStyle::Linear)
            .build()
            .expect("DiscountCurve builder should succeed with valid test data");

        let curve_t1 = DiscountCurve::builder("USD-OIS")
            .base_date(as_of_t1)
            .knots(vec![(0.0, 1.0), (1.0, 0.97)]) // Rates increased (curve lower)
            .interp(InterpStyle::Linear)
            .build()
            .expect("DiscountCurve builder should succeed with valid test data");

        let market_t0 = MarketContext::new().insert(curve_t0);
        let market_t1 = MarketContext::new().insert(curve_t1);

        // Extract and verify snapshots work
        let rates_snapshot = MarketSnapshot::extract(&market_t0, MarketRestoreFlags::RATES);
        assert_eq!(rates_snapshot.discount_curves.len(), 1);

        let restored =
            MarketSnapshot::restore_market(&market_t1, &rates_snapshot, MarketRestoreFlags::RATES);
        assert!(restored.get_discount("USD-OIS").is_ok());
    }

    #[test]
    fn test_parallel_attribution_extracts_rates_credit_cross_factor() {
        let as_of_t0 = date!(2025 - 01 - 15);
        let as_of_t1 = date!(2025 - 01 - 16);
        let config = FinstackConfig::default();
        let instrument: Arc<dyn Instrument> =
            Arc::new(RatesCreditInteractionInstrument::new("TEST-RATES-CREDIT"));

        let market_t0 = MarketContext::new()
            .insert(
                DiscountCurve::builder("USD-OIS")
                    .base_date(as_of_t0)
                    .knots(vec![(0.0, 1.0), (1.0, 0.99)])
                    .interp(InterpStyle::Linear)
                    .build()
                    .expect("discount curve should build"),
            )
            .insert(
                HazardCurve::builder("ACME-HAZ")
                    .base_date(as_of_t0)
                    .knots(vec![(1.0, 0.01)])
                    .recovery_rate(0.40)
                    .build()
                    .expect("hazard curve should build"),
            );

        let market_t1 = MarketContext::new()
            .insert(
                DiscountCurve::builder("USD-OIS")
                    .base_date(as_of_t1)
                    .knots(vec![(0.0, 1.0), (1.0, 0.98)])
                    .interp(InterpStyle::Linear)
                    .build()
                    .expect("discount curve should build"),
            )
            .insert(
                HazardCurve::builder("ACME-HAZ")
                    .base_date(as_of_t1)
                    .knots(vec![(1.0, 0.02)])
                    .recovery_rate(0.40)
                    .build()
                    .expect("hazard curve should build"),
            );

        let attribution = crate::attribute_pnl(
            &AttributionMethod::Parallel,
            &crate::AttributionRequest {
                execution_policy: ExecutionPolicy::Parallel,
                ..crate::AttributionRequest::new(
                    &instrument,
                    &market_t0,
                    &market_t1,
                    as_of_t0,
                    as_of_t1,
                    &config,
                )
            },
        )
        .expect("parallel attribution should succeed");

        assert!(attribution.cross_factor_pnl.amount().abs() > 0.0);
        let detail = attribution
            .cross_factor_detail
            .expect("cross factor detail should be populated");
        assert!(
            detail
                .by_pair
                .get("Rates×Credit")
                .expect("rates-credit entry")
                .amount()
                .abs()
                > 0.0
        );
    }
}
