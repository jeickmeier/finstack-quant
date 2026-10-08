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
    build_credit_factor_attribution, note_unplanned_cascade, plan_credit_cascade, CreditCascadeStep,
};
use super::factors::*;
use super::helpers::*;
use super::model_params;
use super::types::*;
use crate::policy_map::try_map_policy;
use crate::AttributionRequest;
use finstack_quant_calibration::recalibration::CachedRecalibrationProvider;
use finstack_quant_core::market_data::context::MarketContext;
use finstack_quant_core::money::Money;
use finstack_quant_core::Result;
use finstack_quant_valuations::instruments::model_params::ModelParamsSnapshot;
use indexmap::IndexMap;

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
const CROSS_FACTOR_TOLERANCE: f64 = 1e-12;

/// One independently restored factor of the parallel method.
///
/// `flags` selects the market family restored to T₀. It is empty for model
/// parameters, which restore the instrument instead of the market. `label`
/// names the factor in cross-pair keys.
#[derive(Clone)]
struct FactorSpec {
    factor: AttributionFactor,
    flags: MarketRestoreFlags,
    label: &'static str,
}

const fn spec(
    factor: AttributionFactor,
    flags: MarketRestoreFlags,
    label: &'static str,
) -> FactorSpec {
    FactorSpec {
        factor,
        flags,
        label,
    }
}

const RATES: FactorSpec = spec(
    AttributionFactor::RatesCurves,
    MarketRestoreFlags::RATES,
    "Rates",
);
const DISCOUNT: FactorSpec = spec(
    AttributionFactor::RatesCurves,
    MarketRestoreFlags::DISCOUNT,
    "Discount",
);
const FORWARD: FactorSpec = spec(
    AttributionFactor::RatesCurves,
    MarketRestoreFlags::FORWARD,
    "Forward",
);
const CREDIT: FactorSpec = spec(
    AttributionFactor::CreditCurves,
    MarketRestoreFlags::CREDIT,
    "Credit",
);
const INFLATION: FactorSpec = spec(
    AttributionFactor::InflationCurves,
    MarketRestoreFlags::INFLATION,
    "Inflation",
);
const CORRELATIONS: FactorSpec = spec(
    AttributionFactor::Correlations,
    MarketRestoreFlags::CORRELATION,
    "Correlations",
);
const FX: FactorSpec = spec(AttributionFactor::Fx, MarketRestoreFlags::FX, "FX");
const VOL: FactorSpec = spec(
    AttributionFactor::Volatility,
    MarketRestoreFlags::VOL,
    "Vol",
);
const SCALARS: FactorSpec = spec(
    AttributionFactor::MarketScalars,
    MarketRestoreFlags::SCALARS,
    "Spot",
);
const MODEL_PARAMS: FactorSpec = spec(
    AttributionFactor::ModelParameters,
    MarketRestoreFlags::empty(),
    "ModelParameters",
);

/// Default factors, in the order of the module-level algorithm.
const DEFAULT_FACTORS: [FactorSpec; 8] = [
    RATES,
    CREDIT,
    INFLATION,
    CORRELATIONS,
    FX,
    VOL,
    MODEL_PARAMS,
    SCALARS,
];

/// `full_cross_attribution` factors: rates split into discount and forward
/// curves so their interaction is a pair of its own.
const FULL_CROSS_FACTORS: [FactorSpec; 9] = [
    DISCOUNT,
    FORWARD,
    CREDIT,
    INFLATION,
    CORRELATIONS,
    FX,
    VOL,
    SCALARS,
    MODEL_PARAMS,
];

/// Default cross pairs, by factor label (see the module docs). Rates×Inflation
/// captures linkers, Credit×Correlations tranches and Credit×Vol convertibles.
const DEFAULT_CROSS_PAIRS: [(&str, &str); 9] = [
    ("Rates", "Credit"),
    ("Rates", "Vol"),
    ("Spot", "Vol"),
    ("Spot", "Credit"),
    ("FX", "Vol"),
    ("FX", "Rates"),
    ("Credit", "Vol"),
    ("Rates", "Inflation"),
    ("Credit", "Correlations"),
];

/// First-order outcome for one factor.
struct FactorEval {
    spec: FactorSpec,
    /// T₀ snapshot of the factor's family (empty for model parameters).
    snapshot: MarketSnapshot,
    /// `(factor P&L, value with the factor at T₀)`. `None` when T₀ holds no
    /// data for the family or the factor failed softly.
    repriced: Option<(Money, Money)>,
    /// Diagnostic for a soft failure.
    note: Option<String>,
}

/// Note the factors skipped because T0 had no market data for the family
/// while T1 does — e.g. a hazard curve first marked between T0 and T1, a vol
/// surface introduced, an FX matrix attached. Without a note the entire move
/// silently flows into the residual and operators cannot distinguish
/// "cross-effect residual" from "factor dropped because T0 data was missing".
fn note_skipped_empty_t0_factors(
    attribution: &mut PnlAttribution,
    market_t0: &MarketContext,
    market_t1: &MarketContext,
    factor_use: InstrumentFactorUse,
    dependencies: &finstack_quant_valuations::instruments::MarketDependencies,
) {
    let used: Vec<FactorSpec> = DEFAULT_FACTORS
        .into_iter()
        .filter(|spec| {
            spec.factor != AttributionFactor::ModelParameters
                && factor_use.uses_attribution_factor(&spec.factor)
        })
        .collect();
    // One snapshot of the families the instrument uses answers every has-data
    // question while iterating the market's curves once. The T1 snapshot is
    // built lazily, only when a used family is absent at T0.
    let flags = used
        .iter()
        .fold(MarketRestoreFlags::empty(), |all, spec| all | spec.flags);
    let snap_t0 = MarketSnapshot::extract_with_dependencies(market_t0, flags, dependencies);
    let mut snap_t1: Option<MarketSnapshot> = None;
    for spec in used {
        if snap_t0.has_data(spec.flags) {
            continue;
        }
        let snap_t1 = snap_t1.get_or_insert_with(|| {
            MarketSnapshot::extract_with_dependencies(market_t1, flags, dependencies)
        });
        if snap_t1.has_data(spec.flags) {
            let name = format!("{:?}", spec.factor);
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

/// Perform parallel P&L attribution for an instrument.
///
/// Each factor is isolated independently by restoring T₀ values for that
/// factor while keeping all others at T₁. Cross-effects and non-linearities
/// appear in the residual.
///
/// # Arguments
///
/// * `request` - Instrument, T₀ and T₁ markets and dates, configuration and
///   the optional overrides on [`AttributionRequest`]: execution policy,
///   `full_cross_attribution`, opening model parameters, credit-factor model
///   and prepared endpoint values.
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
        execution_policy,
        full_cross_attribution,
        model_params_t0,
        credit_factor_model,
        credit_factor_detail_options,
        ..
    } = *request;
    validate_attribution_period(as_of_t0, as_of_t1)?;
    let recalibration_provider = CachedRecalibrationProvider::new();
    let dependencies = instrument.market_dependencies()?;

    // Endpoint repricings remain part of the workflow's accounting even when
    // the portfolio engine prepared them. The prepared path removes duplicate
    // calls; it does not change the logical cost represented in metadata.
    let mut num_repricings = 2;

    // Step 1: price at T₀ (with T₀ model parameters when supplied) and T₁.
    let (instrument_t0, val_t0, val_t1) = endpoint_values(request)?;
    let ccy = val_t1.currency();
    let mut attribution = seed_attribution(
        request,
        val_t0,
        val_t1,
        AttributionMethod::Parallel,
        execution_policy,
    )?;
    let factor_use = InstrumentFactorUse::of(instrument.as_ref());

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
    // factor captures only the *pricing impact* of swapping the FX matrix
    // inside the pricer.
    //
    // FIXINGS UNDER CARRY: the frozen T₀ market has no
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
    let val_carry = instrument_t0.value(market_t0, as_of_t1)?;
    num_repricings += 1;

    let theta = compute_pnl(val_t0, val_carry, ccy, market_t1, as_of_t1)?;
    let carry_inputs =
        total_return_carry_inputs(instrument_t0.as_ref(), market_t0, as_of_t0, as_of_t1, ccy)?;
    num_repricings += apply_total_return_carry(&mut attribution, theta, carry_inputs)?;

    // Steps 3-10: restore each factor to T₀ on the T₁ market and reprice.
    // Every factor is an independent full revaluation, so they fan out under
    // the execution policy and reduce in list order.
    let params_t0 = model_params_t0
        .cloned()
        .unwrap_or_else(|| instrument.model_params_snapshot());
    let factors: &[FactorSpec] = if full_cross_attribution {
        &FULL_CROSS_FACTORS
    } else {
        &DEFAULT_FACTORS
    };
    let factors: Vec<FactorSpec> = factors
        .iter()
        .filter(|spec| {
            factor_use.uses_attribution_factor(&spec.factor)
                && (spec.factor != AttributionFactor::ModelParameters
                    || !matches!(params_t0, ModelParamsSnapshot::None))
        })
        .cloned()
        .collect();

    let eval_factor = |spec: &FactorSpec| -> Result<FactorEval> {
        let eval = |snapshot, repriced, note| FactorEval {
            spec: spec.clone(),
            snapshot,
            repriced,
            note,
        };
        if spec.factor == AttributionFactor::ModelParameters {
            let (stage, reprice) = match model_params::with_model_params(instrument, &params_t0) {
                Ok(restored) => ("repricing", restored.value(market_t1, as_of_t1)),
                Err(e) => ("parameter modification", Err(e)),
            };
            return match reprice {
                Ok(reprice) => {
                    let pnl = compute_pnl(reprice, val_t1, ccy, market_t1, as_of_t1)?;
                    Ok(eval(MarketSnapshot::default(), Some((pnl, reprice)), None))
                }
                // A model-parameter failure aborts a full-cross run but is
                // only noted on the default path, leaving the move in residual.
                Err(e) if !full_cross_attribution => Ok(eval(
                    MarketSnapshot::default(),
                    None,
                    Some(format!(
                        "Model parameters attribution: {stage} failed - {e}"
                    )),
                )),
                Err(e) => Err(e),
            };
        }

        let snapshot =
            MarketSnapshot::extract_with_dependencies(market_t0, spec.flags, &dependencies);
        if !snapshot.has_data(spec.flags) {
            return Ok(eval(snapshot, None, None));
        }
        let market_with_t0 = MarketSnapshot::restore_market(market_t1, &snapshot, spec.flags);
        let reprice = instrument.value(&market_with_t0, as_of_t1)?;
        // FX-exposure (pricing-impact) P&L: both values are in the instrument's
        // native currency, so the conversions are same-currency identities.
        let pnl = if spec.factor == AttributionFactor::Fx {
            compute_pnl_with_fx(
                reprice, val_t1, ccy, market_t0, market_t1, as_of_t0, as_of_t1,
            )?
        } else {
            compute_pnl(reprice, val_t1, ccy, market_t1, as_of_t1)?
        };
        Ok(eval(snapshot, Some((pnl, reprice)), None))
    };

    // Factors that repriced, with the value each produced, for the cross pairs.
    let mut active: Vec<(FactorSpec, Money)> = Vec::new();
    let mut val_with_t0_credit: Option<Money> = None;
    let mut credit_snapshot = MarketSnapshot::default();
    for eval in try_map_policy(execution_policy, &factors, eval_factor)? {
        let FactorEval {
            spec,
            snapshot,
            repriced,
            note,
        } = eval;
        attribution.meta.notes.extend(note);
        if spec.factor == AttributionFactor::CreditCurves {
            // Retained for the cascade reprice below.
            credit_snapshot = snapshot;
        }
        let Some((pnl, reprice)) = repriced else {
            continue;
        };
        num_repricings += 1;
        match spec.factor {
            // Discount and forward both land in rates on the full-cross path.
            AttributionFactor::RatesCurves => {
                attribution.rates_curves_pnl = attribution.rates_curves_pnl.checked_add(pnl)?;
            }
            AttributionFactor::CreditCurves => {
                attribution.credit_curves_pnl = pnl;
                val_with_t0_credit = Some(reprice);
            }
            AttributionFactor::InflationCurves => attribution.inflation_curves_pnl = pnl,
            AttributionFactor::Correlations => attribution.correlations_pnl = pnl,
            AttributionFactor::Fx => {
                attribution.fx_pnl = pnl;
                stamp_fx_policy(
                    &mut attribution,
                    ccy,
                    "Combined FX exposure and translation P&L (see parallel.rs for details)",
                );
            }
            AttributionFactor::Volatility => attribution.vol_pnl = pnl,
            AttributionFactor::MarketScalars => attribution.market_scalars_pnl = pnl,
            AttributionFactor::ModelParameters => attribution.model_params_pnl = pnl,
            AttributionFactor::Carry => {}
        }
        active.push((spec, reprice));
    }

    // Cross-factor pairs: every pair of active factors on the full-cross
    // path, otherwise the default pairs whose two factors both repriced.
    type ActiveFactor = (FactorSpec, Money);
    let cross_pairs: Vec<(ActiveFactor, ActiveFactor)> = if full_cross_attribution {
        active
            .iter()
            .enumerate()
            .flat_map(|(i, a)| {
                active
                    .iter()
                    .skip(i + 1)
                    .map(move |b| (a.clone(), b.clone()))
            })
            .collect()
    } else {
        let find = |label: &str| active.iter().find(|(spec, _)| spec.label == label).cloned();
        DEFAULT_CROSS_PAIRS
            .iter()
            .filter_map(|(a, b)| Some((find(a)?, find(b)?)))
            .collect()
    };

    // Each pair restores both factors to T₀ at once. A combined `(A | B)`
    // restore from `market_t0` equals stacking the two restores, because
    // `restore_market` only touches flagged families. A pair with model
    // parameters restores the other factor's market and prices the
    // T₀-parameter instrument.
    let reprice_cross = |((a, val_a), (b, val_b)): &(ActiveFactor, ActiveFactor)| {
        let flags = a.flags | b.flags;
        let priced = if a.factor == AttributionFactor::ModelParameters
            || b.factor == AttributionFactor::ModelParameters
        {
            &instrument_t0
        } else {
            instrument
        };
        let combined = MarketSnapshot::extract_with_dependencies(market_t0, flags, &dependencies);
        let market_combined = MarketSnapshot::restore_market(market_t1, &combined, flags);
        let val_both = priced.value(&market_combined, as_of_t1)?;
        let pnl = cross_interaction_pnl(val_t1, *val_a, *val_b, val_both)?;
        Ok::<_, finstack_quant_core::Error>((format!("{}×{}", a.label, b.label), pnl))
    };
    let mut cross_total = 0.0;
    let mut cross_by_pair: IndexMap<String, Money> = IndexMap::new();
    for (pair, pnl) in try_map_policy(execution_policy, &cross_pairs, reprice_cross)? {
        num_repricings += 1;
        if pnl.amount().abs() > CROSS_FACTOR_TOLERANCE {
            cross_total += pnl.amount();
            cross_by_pair.insert(pair, pnl);
        }
    }
    if !cross_by_pair.is_empty() {
        attribution.cross_factor_pnl = Money::new(cross_total, ccy)?;
        attribution.cross_factor_detail = Some(CrossFactorDetail {
            total: attribution.cross_factor_pnl,
            by_pair: cross_by_pair,
        });
    }

    // Credit-factor hierarchy detail via cumulative-bump cascade.
    //
    // The cascade mirrors the waterfall semantics (see `waterfall::apply_credit_cascade`):
    // each step's market is one par-spread bump of the cumulative bp from the
    // same fixed `market_t0_credit` base. The step's P&L is then the *marginal*
    // contribution `V_k − V_{k−1}`, which telescopes so that
    // `Σ steps ≡ V_final − V_0`. With the `CurveShape` step snapping the
    // issuer's hazard curves to T1 at the end, `V_final` reduces to the
    // instrument's T1 valuation (modulo non-issuer hazard curves) and the
    // telescope closes to `credit_curves_pnl` without any residual back-solve,
    // so cross-bp convexity never leaks into `curve_shape_pnl`.
    if let Some(model) = credit_factor_model {
        match plan_credit_cascade(model, instrument, market_t0, market_t1)? {
            Some(cascade) => {
                // T1 market with T0 hazard for the issuer's curves.
                let market_t0_credit = MarketSnapshot::restore_market(
                    market_t1,
                    &credit_snapshot,
                    MarketRestoreFlags::CREDIT,
                );

                // Base value for the cascade: exactly the reference point
                // `credit_curves_pnl` is measured against. Reuse the credit
                // factor's reprice when present; otherwise reprice once here.
                let base_credit_val = match val_with_t0_credit {
                    Some(v) => v,
                    None => {
                        num_repricings += 1;
                        instrument.value(&market_t0_credit, as_of_t1)?
                    }
                };

                // Reprice each step's end-state market. Standalone attribution
                // can fan these out; portfolio callers pass `Serial` so the
                // outer position loop owns Rayon.
                let steps = cascade.steps_with_cumulative_bp();
                let reprice_step = |(step, cumulative_bp): &(&CreditCascadeStep, f64)| {
                    let market_step = cascade.step_market(
                        step,
                        *cumulative_bp,
                        market_t0,
                        &market_t0_credit,
                        market_t1,
                        &recalibration_provider,
                    )?;
                    instrument.value(&market_step, as_of_t1)
                };
                let step_values: Vec<Money> =
                    try_map_policy(execution_policy, &steps, reprice_step)?;
                num_repricings += step_values.len();

                // Telescope to per-step P&Ls: V_k − V_{k−1}, V_0 = base_credit_val.
                let mut step_pnls: Vec<Money> = Vec::with_capacity(cascade.steps.len());
                let mut prev = base_credit_val;
                for v in &step_values {
                    step_pnls.push(compute_pnl(prev, *v, ccy, market_t1, as_of_t1)?);
                    prev = *v;
                }

                attribution.credit_factor_detail = Some(build_credit_factor_attribution(
                    model,
                    &cascade,
                    credit_factor_detail_options,
                    &step_pnls,
                )?);
            }
            None => note_unplanned_cascade(&mut attribution, instrument.id(), "parallel"),
        }
    }

    note_skipped_empty_t0_factors(
        &mut attribution,
        market_t0,
        market_t1,
        factor_use,
        &dependencies,
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
    use finstack_quant_valuations::instruments::Instrument;
    use std::sync::{Arc, OnceLock};
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
