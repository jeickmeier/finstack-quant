use super::super::helpers::*;
use super::super::types::*;
use super::context::AttributionInputs;
use finstack_quant_core::dates::Date;
use finstack_quant_core::market_data::context::MarketContext;
use finstack_quant_core::money::Money;
use finstack_quant_core::Result;
use finstack_quant_valuations::instruments::Instrument;
use finstack_quant_valuations::metrics::collect_cashflows_in_period;
use finstack_quant_valuations::results::ValuationResult;
use std::sync::Arc;

/// Perform metrics-based P&L attribution for an instrument.
///
/// Uses linear approximation with pre-computed risk metrics. Fast but less
/// accurate than full repricing for large market moves.
///
/// # Bucketed DV01 Support
///
/// This function now prioritizes bucketed DV01 (per-curve sensitivities) over
/// aggregate DV01 for rates attribution:
///
/// - **If BucketedDv01 is available**: Computes PnL = Σ(DV01_i × Δr_i) per curve,
///   eliminating basis risk approximation errors.
/// - **Fallback**: Uses aggregate DV01 × avg(Δr_i) with a warning note.
///
/// To get the most accurate rates attribution, include `MetricId::BucketedDv01`
/// in your metrics request when computing valuations.
///
/// # Arguments
///
/// * `instrument` - Instrument to attribute
/// * `market_t0` - Market context at T₀ (for measuring market shifts)
/// * `market_t1` - Market context at T₁ (for measuring market shifts)
/// * `val_t0` - Valuation result at T₀ (with metrics, ideally including BucketedDv01)
/// * `val_t1` - Valuation result at T₁ (with metrics)
/// * `as_of_t0` - Valuation date at T₀
/// * `as_of_t1` - Valuation date at T₁
///
/// # Returns
///
/// P&L attribution using linear approximation with per-curve bucketed metrics.
///
/// # Errors
///
/// Returns error if:
/// - Required metrics are missing
/// - Currency conversion fails
///
/// # Examples
///
/// ```no_run
/// use finstack_quant_core::currency::Currency;
/// use finstack_quant_core::market_data::context::MarketContext;
/// use finstack_quant_core::money::Money;
/// use finstack_quant_attribution::attribute_pnl_metrics_based;
/// use finstack_quant_valuations::instruments::Instrument;
/// use finstack_quant_valuations::instruments::rates::deposit::Deposit;
/// use finstack_quant_valuations::instruments::PricingOptions;
/// use finstack_quant_valuations::metrics::MetricId;
/// use std::sync::Arc;
/// use time::macros::date;
///
/// # fn main() -> Result<(), Box<dyn std::error::Error>> {
/// let as_of_t0 = date!(2025-01-15);
/// let as_of_t1 = date!(2025-01-16);
/// let market_t0 = MarketContext::new();
/// let market_t1 = MarketContext::new();
///
/// // Minimal instrument (for compilation); real attribution requires populated market context.
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
/// // Compute valuations with bucketed metrics for best accuracy
/// let metrics = vec![
///     MetricId::Theta,
///     MetricId::Dv01,
///     MetricId::BucketedDv01,  // ← Include for per-curve rates attribution
///     MetricId::Cs01,
///     MetricId::Vega
/// ];
/// let val_t0 = instrument.price_with_metrics(&market_t0, as_of_t0, &metrics, PricingOptions::default())?;
/// let val_t1 = instrument.price_with_metrics(&market_t1, as_of_t1, &metrics, PricingOptions::default())?;
///
/// let attribution = attribute_pnl_metrics_based(
///     &instrument,
///     &market_t0,
///     &market_t1,
///     &val_t0,
///     &val_t1,
///     as_of_t0,
///     as_of_t1,
/// )?;
/// # let _ = attribution;
/// # Ok(())
/// # }
/// ```
pub fn attribute_pnl_metrics_based(
    instrument: &Arc<dyn Instrument>,
    market_t0: &MarketContext,
    market_t1: &MarketContext,
    val_t0: &ValuationResult,
    val_t1: &ValuationResult,
    as_of_t0: Date,
    as_of_t1: Date,
) -> Result<PnlAttribution> {
    validate_attribution_period(as_of_t0, as_of_t1)?;

    // Total P&L — use date-specific FX to stay consistent with factor decomposition
    let total_pnl = compute_pnl_with_fx(
        val_t0.value,
        val_t1.value,
        val_t1.value.currency(),
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
        AttributionMethod::MetricsBased,
        None,
    );

    // Track whether any non-finite factor P&L was encountered. Invalidating
    // the result prevents residual tolerance from reporting a clean result.
    let mut non_finite_detected = false;

    // Total-return basis : `PnlAttribution::new` captured the
    // raw MTM (`val_t1 − val_t0`) in `mark_to_market_pnl`; add cashflows paid
    // inside [T₀, T₁) so `total_pnl` matches the total-return convention the
    // carry metrics use (Theta / CarryTotal include period cashflows — see
    // `valuations::metrics::sensitivities::theta`). Without this, a coupon
    // payment date produced `residual ≈ −coupon` and a spurious tolerance
    // breach. Mirrors `apply_total_return_carry` on the reprice-based paths;
    // carry itself is NOT adjusted here because the metrics already carry the
    // cashflow component.
    let realized_period_cash = match collect_cashflows_in_period(
        instrument.as_ref(),
        market_t0,
        as_of_t0,
        as_of_t1,
        val_t1.value.currency(),
    ) {
        Ok(coupon_income) if coupon_income.is_finite() => {
            if coupon_income.abs() > 0.0 {
                attribution.total_pnl = attribution
                    .total_pnl
                    .checked_add(Money::new(coupon_income, val_t1.value.currency())?)?;
            }
            Money::new(coupon_income, val_t1.value.currency())?
        }
        Ok(_) => Money::from((0_i64, val_t1.value.currency())),
        Err(e) => {
            attribution.meta.notes.push(format!(
                "Total-return adjustment unavailable (cashflow collection failed: {e}); \
                 total_pnl is MTM-only for this period"
            ));
            Money::from((0_i64, val_t1.value.currency()))
        }
    };

    let inputs = AttributionInputs::new(
        instrument, market_t0, market_t1, val_t0, val_t1, as_of_t0, as_of_t1,
    )?;
    super::carry::apply(
        &inputs,
        &mut attribution,
        &mut non_finite_detected,
        realized_period_cash,
    )?;
    super::rates::apply(&inputs, &mut attribution, &mut non_finite_detected);
    super::credit::apply(&inputs, &mut attribution, &mut non_finite_detected);
    super::fx::apply(&inputs, &mut attribution, &mut non_finite_detected);
    super::volatility::apply(&inputs, &mut attribution, &mut non_finite_detected);
    super::equity::apply_spot(&inputs, &mut attribution, &mut non_finite_detected);
    super::cross_factor::apply(&inputs, &mut attribution, &mut non_finite_detected);

    // 8. Model parameters attribution
    // Requires measuring parameter shifts from instrument at T0 vs T1
    // This needs instrument-specific parameter extraction (prepayment, default, recovery)
    // (See model_params.rs for parameter extraction infrastructure)

    super::equity::apply_dividend(&inputs, &mut attribution, &mut non_finite_detected);
    super::equity::apply_inflation(&inputs, &mut attribution, &mut non_finite_detected);

    // Propagate the flag before finalization so residual computation cannot
    // turn a non-finite factor into an apparently clean result.
    if non_finite_detected {
        attribution.result_invalid = true;
    }

    // Metadata - use reasonable tolerances for metrics-based attribution.
    // Note: Metrics-based attribution is inherently approximate, so larger residuals are expected.
    finalize_attribution(
        &mut attribution,
        instrument.id(),
        "metrics_based",
        0,    // Metrics-based doesn't reprice
        10.0, // $10 absolute tolerance
        1.0,  // 1% relative tolerance
    );

    // Note: For tighter tolerances, consider using waterfall or parallel attribution methods

    Ok(attribution)
}
