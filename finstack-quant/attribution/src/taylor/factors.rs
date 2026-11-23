//! Per-factor Taylor bump-and-reprice Greek kernels (rates, credit, vol, theta).

use super::engine::*;
use crate::AttributionRequest;
use crate::factors::{MarketRestoreFlags, MarketSnapshot};
use crate::metrics_based::shifts::{CreditKeyRateBucket, measure_credit_curve_shift_bp};
use crate::model_params::ModelParamsSnapshot;
use crate::pnl_support::*;
use crate::{AttributionFactor, MoveUnit};
use finstack_quant_core::Result;
use finstack_quant_core::dates::Date;
use finstack_quant_core::market_data::bumps::{BumpSpec, MarketBump};
use finstack_quant_core::market_data::context::MarketContext;
use finstack_quant_core::money::Money;
use finstack_quant_core::types::CurveId;
use finstack_quant_models::volatility::measure_vol_surface_shift;
use finstack_quant_valuations::instruments::Instrument;
use finstack_quant_valuations::metrics::{STANDARD_BUCKETS_YEARS, bump_surface_vol_absolute};
use finstack_quant_valuations::recalibration::{
    HazardRecalibrationAction, HazardRecalibrationRequest, QuoteBump, RecalibrationProvider,
};
use std::sync::Arc;

/// One key-rate bucket's contribution to a rate/forward factor: the per-bucket
/// DV01 (from a triangular bump) paired with the realized per-tenor curve move.
pub(super) struct KeyRateBucket {
    /// Per-bucket DV01 (currency / bp) from a triangular key-rate bump.
    pub(super) dv01: f64,
    /// Realized zero-rate move at this bucket's tenor (basis points).
    pub(super) move_bp: f64,
}

/// Curvature along the observed factor move, including cross-bucket terms.
/// The largest coordinate is bumped by at most `bump_bp`; normalization is
/// independent of signed sensitivities and therefore stable for hedged books.
pub(super) fn directional_gamma_pnl(
    instrument: &Arc<dyn Instrument>,
    as_of_t0: Date,
    pv_t0: Money,
    bump_bp: f64,
    moves_bp: &[f64],
    bumped_market: impl Fn(f64) -> Result<MarketContext>,
    repricings: &mut usize,
) -> Result<f64> {
    let max_move = moves_bp.iter().map(|m| m.abs()).fold(0.0_f64, f64::max);
    if max_move == 0.0 {
        return Ok(0.0);
    }
    let scale = (bump_bp / max_move).min(1.0);
    let bumped_up = bumped_market(scale)?;
    *repricings += 1;
    let up = instrument.value(&bumped_up, as_of_t0)?;
    let bumped_down = bumped_market(-scale)?;
    *repricings += 1;
    let down = instrument.value(&bumped_down, as_of_t0)?;
    Ok(0.5 * (up.amount() - 2.0 * pv_t0.amount() + down.amount()) / scale.powi(2))
}

/// Triangular key-rate bump spec for bucket `i` of the tenor grid `tenors`.
///
/// The wing buckets use the dedicated half-triangle constructors so the
/// `Σ wᵢ(t) = 1.0` partition-of-unity invariant holds across the whole curve
/// (matching the canonical `BucketedDv01` calculator): the first bucket is
/// flat at 1.0 below its tenor (passing `prev = 0.0` instead would understate
/// sub-3M DV01), and the last bucket is flat at 1.0 beyond 30Y (passing
/// `next = ∞` instead produces a NaN weight for any knot past 30Y and aborts
/// the whole factor).
pub(super) fn key_rate_bump_on_grid(tenors: &[f64], i: usize, bump_bp: f64) -> BumpSpec {
    BumpSpec::triangular_key_rate_bucket_bp(
        tenors[..i].last().copied(),
        tenors[i],
        tenors.get(i + 1).copied(),
        bump_bp,
    )
}

/// Compose the bucket bumps on one curve; a batch passed to `MarketContext::bump`
/// retains only the last bump for each curve ID.
pub(super) fn bump_curve_direction(
    market: &MarketContext,
    curve_id: &CurveId,
    tenors: &[f64],
    moves: &[f64],
    scale: f64,
) -> Result<MarketContext> {
    let mut bumped = market.clone();
    for (i, movement) in moves.iter().enumerate() {
        bumped.apply_curve_bump_in_place(
            curve_id,
            key_rate_bump_on_grid(tenors, i, movement * scale),
        )?;
    }
    Ok(bumped)
}

/// Which rates curve family a key-rate factor is measured on.
#[derive(Clone, Copy)]
pub(super) enum CurveKind {
    /// Discount curve: realized move measured in zero rates, `Rates:` prefix.
    Discount,
    /// Forward/projection curve: realized move measured in forward rates,
    /// `Forward:` prefix.
    Forward,
}

/// Inputs of [`compute_curve_factor`]: the shared T0/T1 endpoints plus the curve
/// being attributed and its repricing counter.
pub(super) struct CurveFactorInputs<'a> {
    pub(super) kind: CurveKind,
    pub(super) instrument: &'a Arc<dyn Instrument>,
    pub(super) market_t0: &'a MarketContext,
    pub(super) market_t1: &'a MarketContext,
    pub(super) as_of_t0: Date,
    pub(super) pv_t0: Money,
    pub(super) curve_id: &'a CurveId,
    pub(super) config: &'a TaylorAttributionConfig,
    pub(super) repricings: &'a mut usize,
}

/// Compute rates-curve sensitivity attribution for a single curve — KEY-RATE
/// AWARE.
///
/// Rather than a single parallel DV01 multiplied by an *average* curve shift
/// (which mis-attributes non-parallel moves — a steepener averages toward zero
/// and inflates the unexplained residual), this bumps each standard key-rate
/// bucket with a triangular weight, measures the DV01 of that bucket, and pairs
/// it with the realized move at that bucket's tenor (continuously compounded
/// zero rate of the discount factor or projection factor, the coordinate the
/// key-rate bump shocks):
///
///   explained = Σ_bucket  DV01_bucket × Δr_bucket
pub(super) fn compute_curve_factor(inputs: CurveFactorInputs<'_>) -> Result<TaylorFactorResult> {
    let CurveFactorInputs {
        kind,
        instrument,
        market_t0,
        market_t1,
        as_of_t0,
        pv_t0,
        curve_id,
        config,
        repricings,
    } = inputs;
    // Realized per-tenor moves on the standard bucket grid (bp).
    let per_tenor_move_bp: Vec<f64> = match kind {
        CurveKind::Discount => {
            let (curve_t0, curve_t1) = (
                market_t0.get_discount(curve_id.as_str())?,
                market_t1.get_discount(curve_id.as_str())?,
            );
            STANDARD_BUCKETS_YEARS
                .iter()
                .map(|&t| {
                    let z1 = curve_t1
                        .zero_rate(t, finstack_quant_core::math::Compounding::Continuous)?;
                    let z0 = curve_t0
                        .zero_rate(t, finstack_quant_core::math::Compounding::Continuous)?;
                    Ok((z1 - z0) * 10_000.0)
                })
                .collect::<Result<Vec<f64>>>()?
        }
        CurveKind::Forward => crate::metrics_based::shifts::measure_forward_curve_tenor_shifts_bp(
            curve_id.as_str(),
            market_t0,
            market_t1,
            &STANDARD_BUCKETS_YEARS,
        )?,
    };

    let mut buckets: Vec<KeyRateBucket> = Vec::with_capacity(STANDARD_BUCKETS_YEARS.len());
    for (i, &move_bp) in per_tenor_move_bp.iter().enumerate() {
        let target = market_t0.curve_target(curve_id)?;
        let up = market_t0.bump([target.clone().into_bump(key_rate_bump_on_grid(
            &STANDARD_BUCKETS_YEARS,
            i,
            config.rate_bump_bp,
        ))])?;
        *repricings += 1;
        let pv_up = instrument.value(&up, as_of_t0)?;

        let down = market_t0.bump([target.into_bump(key_rate_bump_on_grid(
            &STANDARD_BUCKETS_YEARS,
            i,
            -config.rate_bump_bp,
        ))])?;
        *repricings += 1;
        let pv_down = instrument.value(&down, as_of_t0)?;

        let dv01 = (pv_up.amount() - pv_down.amount()) / (2.0 * config.rate_bump_bp);
        buckets.push(KeyRateBucket { dv01, move_bp });
    }

    // Key-rate-aware explained P&L: Σ DV01_bucket × Δr_bucket. Compensated
    // summation keeps the per-bucket accumulation stable.
    let explained = finstack_quant_core::math::summation::neumaier_sum(
        buckets.iter().map(|b| b.dv01 * b.move_bp),
    )?;
    let gamma_pnl = if config.include_gamma {
        Some(directional_gamma_pnl(
            instrument,
            as_of_t0,
            pv_t0,
            config.rate_bump_bp,
            &per_tenor_move_bp,
            |scale| {
                bump_curve_direction(
                    market_t0,
                    curve_id,
                    &STANDARD_BUCKETS_YEARS,
                    &per_tenor_move_bp,
                    scale,
                )
            },
            repricings,
        )?)
    } else {
        None
    };

    let prefix = match kind {
        CurveKind::Discount => "Rates",
        CurveKind::Forward => "Forward",
    };
    Ok(TaylorFactorResult {
        factor: AttributionFactor::RatesCurves,
        factor_name: format!("{prefix}:{curve_id}"),
        explained_pnl: explained,
        gamma_pnl,
        working: FactorWorking::KeyRate {
            unit: MoveUnit::BasisPoint,
            buckets: STANDARD_BUCKETS_YEARS
                .iter()
                .zip(&buckets)
                .map(|(&tenor_years, bucket)| (tenor_years, bucket.dv01, bucket.move_bp))
                .collect(),
        },
    })
}

/// Replay parameters of a calibrated hazard curve.
///
/// `operation` names the Taylor step in the error raised for a curve built
/// without a calibration recipe.
pub(super) fn hazard_replay_params(
    hazard: &finstack_quant_core::market_data::term_structures::HazardCurve,
    operation: &str,
) -> Result<finstack_quant_calibration::api::schema::HazardCurveParams> {
    let recipe = hazard.hazard_calibration().ok_or_else(|| {
        finstack_quant_core::Error::Validation(format!(
            "{operation} requires a calibration recipe for '{}'",
            hazard.id()
        ))
    })?;
    serde_json::from_value(recipe.hazard_params().clone()).map_err(|error| {
        finstack_quant_core::Error::Validation(format!("invalid hazard replay parameters: {error}"))
    })
}

/// Apply a parallel credit spread bump in basis points.
/// Hazard curves replay their calibrated par CDS quotes; discount curves
/// serving a credit role bump their zero rates in the same units.
pub(super) fn bump_credit_market(
    market: &MarketContext,
    curve_id: &CurveId,
    bump_bp: f64,
    provider: &dyn RecalibrationProvider,
) -> Result<MarketContext> {
    if let Ok(hazard) = market.get_hazard(curve_id.as_str()) {
        let params = hazard_replay_params(&hazard, "Taylor quote-space credit risk")?;
        let source = Arc::new(market.clone());
        let bumped = provider.rebuild_hazard_curve(&HazardRecalibrationRequest {
            hazard,
            source_market: Arc::clone(&source),
            target_market: source,
            discount_curve_id: params.discount_curve_id,
            doc_clause: None,
            cds_valuation_convention: None,
            deal_quote_override: None,
            action: HazardRecalibrationAction::SpreadBump(QuoteBump::ParallelBp(bump_bp)),
        })?;
        return market.clone().try_insert(bumped.as_ref().clone());
    }
    market.get_discount(curve_id.as_str())?;
    market.bump([MarketBump::DiscountCurve {
        id: curve_id.clone(),
        spec: BumpSpec::parallel_bp(bump_bp),
    }])
}

/// Compose an observed credit direction using exact replay quote indices.
pub(super) fn bump_credit_keyrates(
    market: &MarketContext,
    curve_id: &CurveId,
    buckets: &[CreditKeyRateBucket],
    scale: f64,
    provider: &dyn RecalibrationProvider,
) -> Result<MarketContext> {
    if let Ok(hazard) = market.get_hazard(curve_id.as_str()) {
        let params = hazard_replay_params(&hazard, "Taylor credit gamma")?;
        let mut bumped = market.clone();
        for bucket in buckets {
            let quote_index = bucket.quote_index.ok_or_else(|| {
                finstack_quant_core::Error::Validation(
                    "Taylor quote-space gamma requires exact replay quote indices".into(),
                )
            })?;
            let source = Arc::new(bumped.clone());
            let hazard = source.get_hazard(curve_id.as_str())?;
            let rebuilt = provider.rebuild_hazard_curve(&HazardRecalibrationRequest {
                hazard,
                source_market: Arc::clone(&source),
                target_market: source,
                discount_curve_id: params.discount_curve_id.clone(),
                doc_clause: None,
                cds_valuation_convention: None,
                deal_quote_override: None,
                action: HazardRecalibrationAction::ExactQuoteIndexBump {
                    quote_index,
                    bump_bp: bucket.move_bp * scale,
                },
            })?;
            bumped.try_insert_mut(rebuilt)?;
        }
        Ok(bumped)
    } else {
        let tenors: Vec<_> = buckets.iter().map(|bucket| bucket.tenor_years).collect();
        let moves: Vec<_> = buckets.iter().map(|bucket| bucket.move_bp).collect();
        bump_curve_direction(market, curve_id, &tenors, &moves, scale)
    }
}

pub(super) struct CreditFactorInputs<'a> {
    pub(super) instrument: &'a Arc<dyn Instrument>,
    pub(super) market_t0: &'a MarketContext,
    pub(super) market_t1: &'a MarketContext,
    pub(super) as_of_t0: Date,
    pub(super) pv_t0: Money,
    pub(super) curve_id: &'a CurveId,
    pub(super) config: &'a TaylorAttributionConfig,
    pub(super) keyrate: Option<&'a [CreditKeyRateBucket]>,
    pub(super) recalibration_provider: &'a dyn RecalibrationProvider,
    pub(super) repricings: &'a mut usize,
}

/// Sum every paired quote-space CS01 and exact quote move when buckets exist.
/// Aggregate CS01 uses an average spread shift when no bucket risk is supplied.
pub(super) fn compute_credit_factor(inputs: CreditFactorInputs<'_>) -> Result<TaylorFactorResult> {
    let CreditFactorInputs {
        instrument,
        market_t0,
        market_t1,
        as_of_t0,
        pv_t0,
        curve_id,
        config,
        keyrate,
        recalibration_provider,
        repricings,
    } = inputs;

    // Key-rate path: per-tenor CS01 × per-tenor credit-curve move.
    if let Some(buckets) = keyrate.filter(|b| !b.is_empty()) {
        let shifts: Vec<f64> = buckets.iter().map(|bucket| bucket.move_bp).collect();
        let explained = finstack_quant_core::math::summation::neumaier_sum(
            buckets
                .iter()
                .map(|bucket| bucket.sensitivity * bucket.move_bp),
        )?;
        let gamma_pnl = if config.include_gamma {
            Some(directional_gamma_pnl(
                instrument,
                as_of_t0,
                pv_t0,
                config.credit_spread_bump_bp,
                &shifts,
                |scale| {
                    bump_credit_keyrates(
                        market_t0,
                        curve_id,
                        buckets,
                        scale,
                        recalibration_provider,
                    )
                },
                repricings,
            )?)
        } else {
            None
        };
        return Ok(TaylorFactorResult {
            factor: AttributionFactor::CreditCurves,
            factor_name: format!("Credit:{}", curve_id),

            explained_pnl: explained,
            gamma_pnl,
            working: FactorWorking::KeyRate {
                unit: MoveUnit::BasisPoint,
                buckets: buckets
                    .iter()
                    .map(|bucket| (bucket.tenor_years, bucket.sensitivity, bucket.move_bp))
                    .collect(),
            },
        });
    }

    // Aggregate approximation uses the same quote-space risk definition as
    // bucketed CS01. A missing replay recipe is an error, never an intensity bump.
    let bumped_up = bump_credit_market(
        market_t0,
        curve_id,
        config.credit_spread_bump_bp,
        recalibration_provider,
    )?;
    *repricings += 1;
    let pv_up = instrument.value(&bumped_up, as_of_t0)?;
    let bumped_down = bump_credit_market(
        market_t0,
        curve_id,
        -config.credit_spread_bump_bp,
        recalibration_provider,
    )?;
    *repricings += 1;
    let pv_down = instrument.value(&bumped_down, as_of_t0)?;

    // Central difference CS01: O(h²) accuracy, $ per bp of credit-curve move.
    let cs01 = (pv_up.amount() - pv_down.amount()) / (2.0 * config.credit_spread_bump_bp);

    let spread_move_bp = measure_credit_curve_shift_bp(curve_id.as_str(), market_t0, market_t1)?;

    let explained = cs01 * spread_move_bp;

    let gamma_pnl = if config.include_gamma {
        let gamma = (pv_up.amount() - 2.0 * pv_t0.amount() + pv_down.amount())
            / (config.credit_spread_bump_bp * config.credit_spread_bump_bp);
        Some(0.5 * gamma * spread_move_bp * spread_move_bp)
    } else {
        None
    };

    Ok(TaylorFactorResult {
        factor: AttributionFactor::CreditCurves,
        factor_name: format!("Credit:{}", curve_id),
        explained_pnl: explained,
        gamma_pnl,
        working: FactorWorking::Scalar {
            unit: MoveUnit::BasisPoint,
            sensitivity: cs01,
            market_move: spread_move_bp,
        },
    })
}

/// Inputs of [`compute_vol_factor`]: the shared T0/T1 endpoints plus the
/// vol-surface dependency, the instrument's reference expiry (years, Act/365)
/// and its repricing counter.
pub(super) struct VolFactorInputs<'a> {
    pub(super) instrument: &'a Arc<dyn Instrument>,
    pub(super) market_t0: &'a MarketContext,
    pub(super) market_t1: &'a MarketContext,
    pub(super) as_of_t0: Date,
    pub(super) pv_t0: Money,
    pub(super) dependency: &'a finstack_quant_valuations::instruments::VolatilityDependency,
    pub(super) config: &'a TaylorAttributionConfig,
    pub(super) repricings: &'a mut usize,
}

/// Compute volatility (vega) attribution for a single vol-surface dependency.
///
/// Measures both surfaces at the instrument's contractual expiry and strike.
pub(super) fn compute_vol_factor(inputs: VolFactorInputs<'_>) -> Result<TaylorFactorResult> {
    let VolFactorInputs {
        instrument,
        market_t0,
        market_t1,
        as_of_t0,
        pv_t0,
        dependency,
        config,
        repricings,
    } = inputs;
    let vol_surface_id = &dependency.vol_surface_id;
    let (expiry, strike) = instrument
        .expiry()
        .zip(dependency.reference_strike)
        .ok_or_else(|| {
            finstack_quant_core::Error::Validation(
                "Taylor volatility factor requires a reference expiry and strike".into(),
            )
        })?;
    let opening = market_t0.get_surface(vol_surface_id)?;
    let closing = market_t1.get_surface(vol_surface_id)?;
    opening.require_secondary_axis(
        finstack_quant_core::market_data::surfaces::VolSurfaceAxis::Strike,
    )?;
    let (t0, t1) = volatility_expiry_times(&opening, &closing, expiry, as_of_t0)?;
    let vol_move =
        100.0 * measure_vol_surface_shift(vol_surface_id, market_t0, market_t1, t0, t1, strike)?;
    let bumped_up = bump_surface_vol_absolute(market_t0, vol_surface_id.as_str(), config.vol_bump)?;
    *repricings += 1;
    let pv_up = instrument.value(&bumped_up, as_of_t0)?;

    let bumped_down =
        bump_surface_vol_absolute(market_t0, vol_surface_id.as_str(), -config.vol_bump)?;
    *repricings += 1;
    let pv_down = instrument.value(&bumped_down, as_of_t0)?;

    // The ledger reports Vega per 0.01 native sigma and Volga per (0.01 sigma)².
    let vol_bump_points = config.vol_bump * 100.0;
    let vega_per_point = (pv_up.amount() - pv_down.amount()) / (2.0 * vol_bump_points);
    let explained = vega_per_point * vol_move;

    let gamma_pnl = if config.include_gamma {
        // Volga in $ per vol-point²: use vol_bump_points consistently.
        //   volga [$/pt²] = ΔΔP / (vol_bump_points)²
        //   gamma_pnl [$] = 0.5 × volga × vol_move_points²
        let volga = (pv_up.amount() - 2.0 * pv_t0.amount() + pv_down.amount())
            / (vol_bump_points * vol_bump_points);
        Some(0.5 * volga * vol_move * vol_move)
    } else {
        None
    };

    Ok(TaylorFactorResult {
        factor: AttributionFactor::Volatility,
        factor_name: format!("Vol:{}", vol_surface_id),
        explained_pnl: explained,
        gamma_pnl,
        working: FactorWorking::Scalar {
            unit: MoveUnit::VolPoint,
            sensitivity: vega_per_point,
            market_move: vol_move,
        },
    })
}

/// Isolate one restore-flag family by splicing `snapshot` into the T₁ market,
/// matching the parallel methodology. The P&L is the T₁ value minus the value
/// with the family restored; an instrument that does not read the family
/// produces exactly zero.
pub(super) fn restored_family_factor(
    request: &AttributionRequest<'_>,
    snapshot: &MarketSnapshot,
    pv_t1: Money,
    factor: AttributionFactor,
    flags: MarketRestoreFlags,
    factor_name: &str,
    repricings: &mut usize,
) -> Result<TaylorFactorResult> {
    let market_with_t0 = MarketSnapshot::restore_market(request.market_t1, snapshot, flags)?;
    *repricings += 1;
    let pv_with_t0 = request
        .instrument
        .value(&market_with_t0, request.as_of_t1)?;
    Ok(TaylorFactorResult {
        factor,
        factor_name: factor_name.to_string(),
        explained_pnl: pv_t1.amount() - pv_with_t0.amount(),
        gamma_pnl: None,
        working: FactorWorking::Repriced { pv: pv_with_t0 },
    })
}

/// Run a restore-and-reprice family when either market carries that family.
pub(super) fn record_restored_family_factor(
    acc: &mut TaylorAttributionResult,
    request: &AttributionRequest<'_>,
    dependencies: &finstack_quant_valuations::instruments::MarketDependencies,
    pv_t1: Money,
    (factor, flags): (AttributionFactor, MarketRestoreFlags),
    factor_kind: &str,
    factor_name: &str,
) {
    let t0_snap = MarketSnapshot::extract_with_dependencies(request.market_t0, flags, dependencies);
    let t1_snap = MarketSnapshot::extract_with_dependencies(request.market_t1, flags, dependencies);
    if !t0_snap.has_data(flags) && !t1_snap.has_data(flags) {
        return;
    }
    let mut repricings = 0;
    let result = restored_family_factor(
        request,
        &t0_snap,
        pv_t1,
        factor,
        flags,
        factor_name,
        &mut repricings,
    );
    acc.record(factor_kind, factor_name, result, repricings);
}

/// Isolate T₀ vs T₁ model-parameter P&L by repricing the T₁ market with the
/// opening snapshot restored onto the instrument.
pub(super) fn compute_model_params_factor(
    instrument: &Arc<dyn Instrument>,
    market_t1: &MarketContext,
    as_of_t1: Date,
    pv_t1: Money,
    params_t0: &ModelParamsSnapshot,
    repricings: &mut usize,
) -> Result<TaylorFactorResult> {
    let instrument_t0 = instrument_at_t0(instrument, Some(params_t0))?;
    *repricings += 1;
    let pv_with_t0 = instrument_t0.value(market_t1, as_of_t1)?;
    let explained = pv_t1.amount() - pv_with_t0.amount();
    Ok(TaylorFactorResult {
        factor: AttributionFactor::ModelParameters,
        factor_name: "ModelParameters".to_string(),
        explained_pnl: explained,
        gamma_pnl: None,
        working: FactorWorking::Repriced { pv: pv_with_t0 },
    })
}

/// Coupon income for the theta period — surfaced separately so
/// `attribute_pnl_taylor` can re-use it when splitting `theta_pnl` into the
/// pure PV move, coupon income and returned principal (instead of calling
/// `collect_period_cash` again, which would re-traverse the
/// instrument's cashflow schedule and risk silent desync if the schedule path
/// is non-deterministic).
pub(super) struct ThetaFactorOutcome {
    pub(super) factor: TaylorFactorResult,
    pub(super) coupon_income: Money,
    pub(super) cash_paid: Money,
}

/// Compute theta (time decay + realized cashflows) by repricing at T1 date
/// with T0 market, then adding all economic receipts in the period.
pub(super) fn compute_theta_factor(
    instrument: &Arc<dyn Instrument>,
    market_t0: &MarketContext,
    as_of_t0: Date,
    as_of_t1: Date,
    pv_t0: Money,
    repricings: &mut usize,
) -> Result<ThetaFactorOutcome> {
    use finstack_quant_valuations::metrics::collect_period_cash;

    *repricings += 1;
    let pv_t0_at_t1 = instrument.value(market_t0, as_of_t1)?;
    let pv_diff = pv_t0_at_t1.amount() - pv_t0.amount();

    let cash = collect_period_cash(
        instrument.as_ref(),
        market_t0,
        as_of_t0,
        as_of_t1,
        pv_t0.currency(),
    )?;

    let theta_pnl = pv_diff + cash.total.amount();
    Ok(ThetaFactorOutcome {
        factor: TaylorFactorResult {
            factor: AttributionFactor::Carry,
            factor_name: "Theta".to_string(),
            explained_pnl: theta_pnl,
            gamma_pnl: None,
            working: FactorWorking::Repriced { pv: pv_t0_at_t1 },
        },
        coupon_income: cash.income,
        cash_paid: cash.total,
    })
}
