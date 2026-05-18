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
//! - Model parameters attribution requires instrument-specific support (see model_params.rs)

use super::credit_cascade::{
    build_credit_factor_attribution, plan_credit_cascade, shift_hazard_curves,
};
use super::credit_factor::CreditFactorDetailOptions;
use super::factors::*;
use super::helpers::*;
use super::model_params;
use super::types::*;
use crate::instruments::common_impl::traits::Instrument;
use crate::metrics::sensitivities::theta::collect_cashflows_in_period;
use finstack_core::config::FinstackConfig;
use finstack_core::dates::Date;
use finstack_core::factor_model::credit_hierarchy::CreditFactorModel;
use finstack_core::market_data::context::MarketContext;
use finstack_core::money::Money;
use finstack_core::Result;
use indexmap::IndexMap;
use rayon::prelude::*;
use std::sync::Arc;

fn cross_interaction_pnl(
    val_t1: Money,
    val_with_t0_a: Money,
    val_with_t0_b: Money,
    val_with_t0_ab: Money,
) -> Result<Money> {
    val_t1
        .checked_sub(val_with_t0_a)?
        .checked_sub(val_with_t0_b)?
        .checked_add(val_with_t0_ab)
}

/// Cross-factor tolerance for including an interaction term in the detail map.
/// Matches the historical inline filter (`pnl.amount().abs() > 1e-12`).
const CROSS_FACTOR_TOLERANCE: f64 = 1e-12;

#[derive(Clone, Copy)]
enum ParallelRestoredFactor {
    Rates,
    Credit,
    Inflation,
    Correlations,
    Volatility,
    MarketScalars,
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

fn restored_factor_has_data(factor: ParallelRestoredFactor, snapshot: &MarketSnapshot) -> bool {
    match factor {
        ParallelRestoredFactor::Rates => true,
        ParallelRestoredFactor::Credit => !snapshot.hazard_curves.is_empty(),
        ParallelRestoredFactor::Inflation => !snapshot.inflation_curves.is_empty(),
        ParallelRestoredFactor::Correlations => !snapshot.base_correlation_curves.is_empty(),
        ParallelRestoredFactor::Volatility => !snapshot.surfaces.is_empty(),
        ParallelRestoredFactor::MarketScalars => {
            !snapshot.prices.is_empty()
                || !snapshot.series.is_empty()
                || !snapshot.inflation_indices.is_empty()
                || !snapshot.dividends.is_empty()
        }
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
    flags: CurveRestoreFlags,
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
    flags: CurveRestoreFlags,
    val_t1: Money,
    val_with_t0_a: Money,
    val_with_t0_b: Money,
) -> Result<Money> {
    let combined = MarketSnapshot::extract(market_t0, flags);
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
/// ```rust,no_run
/// use finstack_valuations::attribution::attribute_pnl_parallel;
/// use finstack_valuations::instruments::Instrument;
/// use finstack_valuations::instruments::rates::deposit::Deposit;
/// use finstack_core::config::FinstackConfig;
/// use finstack_core::currency::Currency;
/// use finstack_core::market_data::context::MarketContext;
/// use finstack_core::money::Money;
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
///         .notional(Money::new(1_000_000.0, Currency::USD))
///         .start_date(as_of_t0)
///         .maturity(as_of_t1)
///         .day_count(finstack_core::dates::DayCount::Act360)
///         .discount_curve_id("USD-OIS".into())
///         .build()
///         .expect("deposit builder should succeed"),
/// ) as Arc<dyn Instrument>;
///
/// let attribution = attribute_pnl_parallel(
///     &instrument,
///     &market_t0,
///     &market_t1,
///     as_of_t0,
///     as_of_t1,
///     &config,
///     None,
/// )?;
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
#[tracing::instrument(skip_all, fields(instrument_id = %instrument.id(), method = "parallel"))]
pub fn attribute_pnl_parallel(
    instrument: &Arc<dyn Instrument>,
    market_t0: &MarketContext,
    market_t1: &MarketContext,
    as_of_t0: Date,
    as_of_t1: Date,
    config: &FinstackConfig,
    model_params_t0: Option<&model_params::ModelParamsSnapshot>,
) -> Result<PnlAttribution> {
    attribute_pnl_parallel_with_credit_model(
        instrument,
        market_t0,
        market_t1,
        as_of_t0,
        as_of_t1,
        config,
        model_params_t0,
        None,
        &CreditFactorDetailOptions::default(),
    )
}

/// Parallel attribution with optional `CreditFactorModel`.
///
/// When `credit_factor_model` is `Some(_)` and the instrument has a
/// resolvable issuer + hazard exposure, each parallel-bump cascade step
/// (`generic`, level_k, `adder`) is isolated as its own parallel factor:
/// starting from T1 markets, hazard curves are reverted to T0 and then bumped
/// by the step's per-issuer `β·ΔF` (or `Δadder`) bp; the reprice differential
/// is the step's parallel P&L. The non-parallel hazard residual is attributed
/// to the `curve_shape` step, back-solved as
/// `credit_curves_pnl − Σ(parallel steps)` so that `Σ steps ≡ credit_curves_pnl`
/// holds exactly (audit item #1) — no `CreditCascadeResidual` cross-factor
/// entry is needed.
///
/// See `credit_cascade::plan_credit_cascade` for the multi-curve issuer averaging caveat.
///
/// # Performance
///
/// When a `CreditFactorModel` is supplied with `L` hierarchy levels, the credit
/// cascade performs `L + 2` additional repricings (PC, one per level, and
/// Adder) compared to the single-step credit reprice without a model. The
/// `curve_shape` step costs no reprice (it is back-solved). For typical
/// L = 1–3 and portfolios of thousands of instruments this is acceptable;
/// consider `MetricsBased` or `Taylor` for cost-sensitive use cases (they
/// remain linear, no reprice).
#[allow(clippy::too_many_arguments)]
#[tracing::instrument(
    skip_all,
    fields(instrument_id = %instrument.id(), method = "parallel")
)]
pub fn attribute_pnl_parallel_with_credit_model(
    instrument: &Arc<dyn Instrument>,
    market_t0: &MarketContext,
    market_t1: &MarketContext,
    as_of_t0: Date,
    as_of_t1: Date,
    _config: &FinstackConfig,
    model_params_t0: Option<&model_params::ModelParamsSnapshot>,
    credit_factor_model: Option<&CreditFactorModel>,
    credit_factor_detail_options: &CreditFactorDetailOptions,
) -> Result<PnlAttribution> {
    let mut num_repricings = 0;

    // Step 1: Price at T₀ and T₁
    // Use T₀ model parameters for T₀ valuation if available
    let instrument_t0 = if let Some(params) = model_params_t0 {
        model_params::with_model_params(instrument, params)?
    } else {
        Arc::clone(instrument)
    };
    let val_t0 = reprice_instrument(&instrument_t0, market_t0, as_of_t0)?;
    num_repricings += 1;

    let val_t1 = reprice_instrument(instrument, market_t1, as_of_t1)?;
    num_repricings += 1;

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
        Some(_config),
    );

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
    // FX CONVENTION: Both T₀ and carry values are converted at T₁ FX rates
    // (via `compute_pnl`). This isolates the pricing effect of time passage
    // from FX translation effects. The FX factor (Step 7) captures all
    // translation P&L, ensuring consistent summation.
    let market_frozen = market_t0.clone();
    let val_carry = reprice_instrument(instrument, &market_frozen, as_of_t1)?;
    num_repricings += 1;

    let theta = compute_pnl(val_t0, val_carry, val_t1.currency(), market_t1, as_of_t1)?;

    // Realized cashflows (coupons paid) during [T0, T1]. On coupon dates the
    // coupon drops out of PV but the holder receives it as cash — including it
    // here aligns with GenericThetaAny (carry = PV roll + realized cashflows).
    let coupon_income_value = collect_cashflows_in_period(
        instrument.as_ref(),
        &market_frozen,
        as_of_t0,
        as_of_t1,
        val_t1.currency(),
    )
    .unwrap_or(0.0);
    let coupon_income = Money::new(coupon_income_value, val_t1.currency());

    // Include cashflows in total_pnl so that residual stays consistent:
    // total_pnl now represents economic (total-return) P&L.
    apply_total_return_carry(&mut attribution, theta, coupon_income)?;

    // Steps 3-6: ordinary restored-market factors before FX. Order is
    // preserved exactly so repricing counts and first-error behavior stay
    // stable.
    let pre_fx_specs = [
        (ParallelRestoredFactor::Rates, CurveRestoreFlags::RATES),
        (ParallelRestoredFactor::Credit, CurveRestoreFlags::CREDIT),
        (
            ParallelRestoredFactor::Inflation,
            CurveRestoreFlags::INFLATION,
        ),
        (
            ParallelRestoredFactor::Correlations,
            CurveRestoreFlags::CORRELATION,
        ),
    ];
    let pre_fx_evals = pre_fx_specs
        .par_iter()
        .map(|(factor, flags)| {
            let snapshot = MarketSnapshot::extract(market_t0, *flags);
            let has_data = restored_factor_has_data(*factor, &snapshot);
            reprice_factor_restored_once(
                instrument, market_t1, &snapshot, *flags, has_data, as_of_t1, val_t1,
            )
            .map(|output| RestoredFactorEval {
                factor: *factor,
                snapshot,
                output,
            })
        })
        .collect::<Result<Vec<_>>>()?;
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
                ParallelRestoredFactor::Inflation => attribution.inflation_curves_pnl = pnl,
                ParallelRestoredFactor::Correlations => attribution.correlations_pnl = pnl,
                ParallelRestoredFactor::Volatility | ParallelRestoredFactor::MarketScalars => {}
            }
        }
    }

    // Step 7: FX attribution
    //
    // FX P&L has two conceptually distinct components that are COMBINED in this attribution:
    //
    // 1. **FX EXPOSURE P&L** (Pricing Impact):
    //    Impact of FX rate changes on instrument pricing for cross-currency products.
    //    Example: Cross-currency swap pricing depends on FX rates for basis adjustment.
    //    This is isolated by repricing with T₀ FX rates but T₁ curves.
    //
    // 2. **FX TRANSLATION P&L** (Reporting Impact):
    //    Impact of converting instrument PV from native currency to reporting currency.
    //    Example: EUR-denominated bond held by USD reporter. If EUR/USD moves, the
    //    reported USD value changes even if EUR PV is unchanged.
    //    This is captured by using date-appropriate FX rates for conversion.
    //
    // FX CONVERSION METHODOLOGY:
    //   - Non-FX factors (carry, rates, credit, vol, etc.) use `compute_pnl` which
    //     converts BOTH T₀ and T₁ values at T₁ FX. This isolates pricing effects.
    //   - Total P&L uses `compute_pnl_with_fx` which converts T₀ at T₀ FX and T₁
    //     at T₁ FX (actual P&L).
    //   - The FX factor uses `compute_pnl_with_fx` to capture both exposure and
    //     translation effects, absorbing the V₀×(X₁-X₀) translation term that
    //     the non-FX factors exclude.
    //   - Any residual cross-terms (ΔV×ΔX) are inherent in the parallel approach
    //     and appear in the residual by design.
    //
    // For single-currency instruments in their native reporting currency, this step
    // produces zero P&L as expected.
    let fx_snapshot = MarketSnapshot::extract(market_t0, CurveRestoreFlags::FX);
    if fx_snapshot.fx.is_some() {
        let market_with_t0_fx =
            MarketSnapshot::restore_market(market_t1, &fx_snapshot, CurveRestoreFlags::FX);
        let fx_reprice = reprice_instrument(instrument, &market_with_t0_fx, as_of_t1)?;
        num_repricings += 1;
        val_with_t0_fx = Some(fx_reprice);

        // Compute combined FX P&L (exposure + translation)
        // Uses T₀ FX for converting T₀ PV and T₁ FX for converting T₁ PV
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
    let post_fx_specs = [(ParallelRestoredFactor::Volatility, CurveRestoreFlags::VOL)];
    for (factor, flags) in post_fx_specs {
        let snapshot = MarketSnapshot::extract(market_t0, flags);
        if let Some((pnl, reprice)) = reprice_factor_restored_once(
            instrument,
            market_t1,
            &snapshot,
            flags,
            !snapshot.surfaces.is_empty(),
            as_of_t1,
            val_t1,
        )? {
            num_repricings += 1;
            match factor {
                ParallelRestoredFactor::Volatility => {
                    attribution.vol_pnl = pnl;
                    val_with_t0_vol = Some(reprice);
                }
                ParallelRestoredFactor::Rates
                | ParallelRestoredFactor::Credit
                | ParallelRestoredFactor::Inflation
                | ParallelRestoredFactor::Correlations
                | ParallelRestoredFactor::MarketScalars => {
                    unreachable!("only volatility is present in this ordered loop")
                }
            }
        }
    }

    // Step 9: Model parameters attribution
    let params_t0 = model_params_t0
        .cloned()
        .unwrap_or_else(|| model_params::extract_model_params(instrument));
    if !matches!(params_t0, model_params::ModelParamsSnapshot::None) {
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
    let post_model_specs = [(
        ParallelRestoredFactor::MarketScalars,
        CurveRestoreFlags::SCALARS,
    )];
    for (factor, flags) in post_model_specs {
        let snapshot = MarketSnapshot::extract(market_t0, flags);
        let has_scalars = !snapshot.prices.is_empty()
            || !snapshot.series.is_empty()
            || !snapshot.inflation_indices.is_empty()
            || !snapshot.dividends.is_empty();
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
            match factor {
                ParallelRestoredFactor::MarketScalars => {
                    attribution.market_scalars_pnl = pnl;
                    val_with_t0_scalars = Some(reprice);
                }
                ParallelRestoredFactor::Rates
                | ParallelRestoredFactor::Credit
                | ParallelRestoredFactor::Inflation
                | ParallelRestoredFactor::Correlations
                | ParallelRestoredFactor::Volatility => {
                    unreachable!("only market scalars are present in this ordered loop")
                }
            }
        }
    }

    // Step 10b: Explicit cross-factor interaction repricings.
    //
    // Each block restores two factors jointly from `market_t0` onto `market_t1`
    // and compares against the two single-factor repricings already captured
    // above. `restore_market` only touches flagged families, so restoring
    // `A | B` from `market_t0` in one shot is bit-identical to stacking an
    // `A` restore followed by a `B` restore.
    let mut cross_total = 0.0;
    let mut cross_by_pair: IndexMap<String, Money> = IndexMap::new();

    // (pair_label, flag_A, flag_B, reprice_A, reprice_B) — order preserved
    // exactly as before for reduction-order stability.
    type CrossSpec<'a> = (
        &'a str,
        CurveRestoreFlags,
        CurveRestoreFlags,
        Option<Money>,
        Option<Money>,
    );
    let cross_specs: [CrossSpec<'_>; 6] = [
        (
            "Rates×Credit",
            CurveRestoreFlags::RATES,
            CurveRestoreFlags::CREDIT,
            val_with_t0_rates,
            val_with_t0_credit,
        ),
        (
            "Rates×Vol",
            CurveRestoreFlags::RATES,
            CurveRestoreFlags::VOL,
            val_with_t0_rates,
            val_with_t0_vol,
        ),
        (
            "Spot×Vol",
            CurveRestoreFlags::SCALARS,
            CurveRestoreFlags::VOL,
            val_with_t0_scalars,
            val_with_t0_vol,
        ),
        (
            "Spot×Credit",
            CurveRestoreFlags::CREDIT,
            CurveRestoreFlags::SCALARS,
            val_with_t0_scalars,
            val_with_t0_credit,
        ),
        (
            "FX×Vol",
            CurveRestoreFlags::FX,
            CurveRestoreFlags::VOL,
            val_with_t0_fx,
            val_with_t0_vol,
        ),
        (
            "FX×Rates",
            CurveRestoreFlags::RATES,
            CurveRestoreFlags::FX,
            val_with_t0_fx,
            val_with_t0_rates,
        ),
    ];

    // Each cross-factor block is an independent full revaluation. Reprice them
    // in parallel, then reduce in the fixed `cross_specs` order so the result
    // is bit-identical to the previous sequential loop.
    let cross_results = cross_specs
        .par_iter()
        .map(|(pair, flag_a, flag_b, reprice_a, reprice_b)| {
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
            Ok(Some((*pair, pnl)))
        })
        .collect::<Result<Vec<Option<(&str, Money)>>>>()?;
    for result in cross_results.into_iter().flatten() {
        let (pair, pnl) = result;
        num_repricings += 1;
        record_cross_pair(pair, pnl, &mut cross_total, &mut cross_by_pair);
    }

    // Step 10c: Credit-factor hierarchy detail. Each cascade step is isolated
    // independently against T1 (T0 hazard restored, then bumped up by the
    // step's per-issuer bp shift). Cross-effects between steps appear in the
    // residual; we account for the full Σ(step) vs. credit_curves_pnl gap as a
    // single "Credit×Hierarchy" entry under cross_factor_pnl.
    if let Some(model) = credit_factor_model {
        match plan_credit_cascade(model, instrument, market_t0, market_t1, as_of_t0, as_of_t1)? {
            Some(cascade) => {
                // Build a T1-base market with T0 hazard for the issuer's curves.
                // Re-use the credit snapshot extracted in Step 4.
                let market_t0_credit = MarketSnapshot::restore_market(
                    market_t1,
                    &credit_snapshot,
                    CurveRestoreFlags::CREDIT,
                );

                // Each parallel-bump cascade step (Generic / Level / Adder)
                // bumps the T0 hazard curves by exactly that step's bp from the
                // same `market_t0_credit` base — the running market is not
                // chained between steps, so the steps are fully independent.
                // The `CurveShape` step is NOT a bp bump: it represents the
                // non-parallel hazard residual and is back-solved below so the
                // decomposition closes exactly (`Σ steps ≡ credit_curves_pnl`).
                // Reprice the bp-bump steps in parallel, collect in cascade
                // order, then reduce sequentially for bit-identical results.
                let mut step_pnls: Vec<Money> = cascade
                    .steps
                    .par_iter()
                    .map(|step| -> Result<Money> {
                        if matches!(step.kind, super::credit_cascade::CreditStepKind::CurveShape)
                        {
                            // Placeholder; the curve-shape P&L is filled in
                            // below as the closing residual.
                            return Ok(Money::new(0.0, val_t1.currency()));
                        }
                        let market_step = shift_hazard_curves(
                            &market_t0_credit,
                            &cascade.hazard_curve_ids,
                            step.delta_bp,
                        )?;
                        let reprice = reprice_instrument(instrument, &market_step, as_of_t1)?;
                        compute_pnl(reprice, val_t1, val_t1.currency(), market_t1, as_of_t1)
                    })
                    .collect::<Result<Vec<Money>>>()?;
                // CurveShape steps did not cost a reprice.
                num_repricings += step_pnls
                    .iter()
                    .zip(cascade.steps.iter())
                    .filter(|(_, s)| {
                        !matches!(s.kind, super::credit_cascade::CreditStepKind::CurveShape)
                    })
                    .count();

                // Sum of the parallel (bp-bump) steps only.
                let parallel_step_sum: f64 = step_pnls
                    .iter()
                    .zip(cascade.steps.iter())
                    .filter(|(_, s)| {
                        !matches!(s.kind, super::credit_cascade::CreditStepKind::CurveShape)
                    })
                    .map(|(pnl, _)| pnl.amount())
                    .sum();

                // Audit item #1: the non-parallel hazard residual is attributed
                // to the `CurveShape` step (a real curve-shape / term-structure
                // component) rather than left in a generic cross-factor bucket.
                // `curve_shape = credit_curves_pnl − Σ(parallel steps)` makes
                // `Σ all steps ≡ credit_curves_pnl` hold exactly.
                let curve_shape_amt =
                    attribution.credit_curves_pnl.amount() - parallel_step_sum;
                for (pnl, step) in step_pnls.iter_mut().zip(cascade.steps.iter()) {
                    if matches!(step.kind, super::credit_cascade::CreditStepKind::CurveShape) {
                        *pnl = Money::new(curve_shape_amt, val_t1.currency());
                    }
                }

                let detail = build_credit_factor_attribution(
                    model,
                    &cascade,
                    credit_factor_detail_options,
                    &step_pnls,
                );
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

    if !cross_by_pair.is_empty() {
        attribution.cross_factor_pnl = Money::new(cross_total, val_t1.currency());
        attribution.cross_factor_detail = Some(CrossFactorDetail {
            total: attribution.cross_factor_pnl,
            by_pair: cross_by_pair,
        });
    }

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
    #[allow(clippy::expect_used, dead_code, unused_imports)]
    mod test_utils {
        include!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/tests/support/attribution_test_utils.rs"
        ));
    }

    use super::*;
    use finstack_core::currency::Currency;
    use finstack_core::dates::Date;
    use finstack_core::market_data::term_structures::DiscountCurve;
    use finstack_core::market_data::term_structures::HazardCurve;
    use finstack_core::math::interp::InterpStyle;
    use finstack_core::money::Money;
    use std::sync::OnceLock;
    use test_utils::TestInstrument;
    use time::macros::date;

    #[derive(Clone)]
    struct RatesCreditInteractionInstrument {
        id: String,
    }

    crate::impl_empty_cashflow_provider!(
        RatesCreditInteractionInstrument,
        crate::cashflow::builder::CashflowRepresentation::NoResidual
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

        fn key(&self) -> crate::pricer::InstrumentType {
            crate::pricer::InstrumentType::Bond
        }

        fn as_any(&self) -> &dyn std::any::Any {
            self
        }

        fn as_any_mut(&mut self) -> &mut dyn std::any::Any {
            self
        }

        fn attributes(&self) -> &crate::instruments::common_impl::traits::Attributes {
            static ATTRS: OnceLock<crate::instruments::common_impl::traits::Attributes> =
                OnceLock::new();
            ATTRS.get_or_init(crate::instruments::common_impl::traits::Attributes::default)
        }

        fn attributes_mut(&mut self) -> &mut crate::instruments::common_impl::traits::Attributes {
            unreachable!("RatesCreditInteractionInstrument::attributes_mut should not be called")
        }

        fn clone_box(&self) -> Box<dyn Instrument> {
            Box::new(self.clone())
        }

        fn market_dependencies(
            &self,
        ) -> finstack_core::Result<crate::instruments::common_impl::dependencies::MarketDependencies>
        {
            let mut deps = crate::instruments::common_impl::dependencies::MarketDependencies::new();
            deps.add_curves(
                crate::instruments::common_impl::traits::InstrumentCurves::builder()
                    .discount(finstack_core::types::CurveId::new("USD-OIS"))
                    .credit(finstack_core::types::CurveId::new("ACME-HAZ"))
                    .build()?,
            );
            Ok(deps)
        }

        fn base_value(&self, market: &MarketContext, _as_of: Date) -> Result<Money> {
            let rate = market.get_discount("USD-OIS")?.zero(1.0);
            let hazard = market.get_hazard("ACME-HAZ")?.hazard_rate(1.0);
            Ok(Money::new(1_000_000.0 * rate * hazard, Currency::USD))
        }

        fn price_with_metrics(
            &self,
            market: &MarketContext,
            as_of: Date,
            _metrics: &[crate::metrics::MetricId],
            _options: crate::instruments::common_impl::traits::PricingOptions,
        ) -> Result<crate::results::ValuationResult> {
            Ok(crate::results::ValuationResult::stamped(
                self.id(),
                as_of,
                self.value(market, as_of)?,
            ))
        }
    }

    #[test]
    fn test_parallel_attribution_simple() {
        let as_of_t0 = date!(2025 - 01 - 15);
        let as_of_t1 = date!(2025 - 01 - 16);

        // Create test instrument with different values at T0 and T1
        let _instrument_t0 = Arc::new(TestInstrument::new(
            "TEST-001",
            Money::new(1000.0, Currency::USD),
        ));

        // Simulate P&L by creating a different value for T1
        // In practice, the same instrument would be repriced with different markets
        let val_t0 = Money::new(1000.0, Currency::USD);
        let val_t1 = Money::new(1100.0, Currency::USD);

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
        let rates_snapshot = MarketSnapshot::extract(&market_t0, CurveRestoreFlags::RATES);
        assert_eq!(rates_snapshot.discount_curves.len(), 1);

        let restored =
            MarketSnapshot::restore_market(&market_t1, &rates_snapshot, CurveRestoreFlags::RATES);
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
                    .build()
                    .expect("hazard curve should build"),
            );

        let attribution = attribute_pnl_parallel(
            &instrument,
            &market_t0,
            &market_t1,
            as_of_t0,
            as_of_t1,
            &config,
            None,
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
