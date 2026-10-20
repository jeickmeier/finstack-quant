//! Metrics-based P&L attribution.
//!
//! Fast approximation using pre-computed risk metrics (Theta, DV01, CS01, Vega, etc.)
//! to estimate factor contributions without full repricing. Supports both first-order
//! (linear) and second-order (convexity) terms for improved accuracy.
//!
//! # Algorithm
//!
//! 1. **Carry**: Total-return Theta over the matching attribution horizon
//! 2. **RatesCurves**:
//!    - Per-curve (if BucketedDv01 available): Σ(DV01_i × Δr_i) for each curve i
//!    - Fallback (aggregate DV01): DV01 × avg(Δr_i)
//!    - Second-order: ½ × Convexity × (Δr)² (if available)
//!   3. **CreditCurves**:
//!    - First-order: CS01 × Δs
//!    - Second-order: ½ × CS-Gamma × (Δs)² (if available)
//!   4. **Fx**: FX01 × Δfx
//!   5. **Volatility**:
//!    - First-order: Vega × Δσ
//!    - Second-order: ½ × Volga × (Δσ)²
//!    - Cross-term: CrossGammaSpotVol × Δspot_pct × Δσ_vol_pt
//!      (NOT Vanna — see the unit-contract note at the cross-factor site)
//!   6. **Market Scalars** (for options):
//!    - First-order: Delta × Δspot
//!    - Second-order: ½ × Gamma × (Δspot)²
//!   7. **Inflation**:
//!    - First-order: Inflation01 × Δi
//!    - Second-order: ½ × InflationConvexity × (Δi)²
//!   8. **ModelParameters**: Param01 metrics × param_shift
//!   9. **Residual**: Total P&L - sum(approximations)
//!
//! Aggregate Delta/Gamma and spot cross-Gamma use the first declared market scalar,
//! matching the sensitivity producer. Vega uses the instrument expiry and declared
//! reference strike; absent a reference point, only uniform surface moves are
//! supported. Differing moves across multiple vol sources flag the result invalid.
//! Precomputed Theta and CarryTotal must match the attribution horizon, even when
//! CouponIncome is supplied; the spec executor requests that horizon from the producer.
//! Cashflow collection and payment-date FX conversion failures are returned to the caller.
//!
//! # Advantages
//!
//! - No additional repricing
//! - Per-curve bucketed DV01 when available
//! - Second-order terms when the producer supplies them
//! - Works with already-computed ValuationResults
//!
//! # Disadvantages
//!
//! - Still approximate (third-order+ effects ignored)
//! - Less accurate than parallel/waterfall methods for extreme moves
//! - Large market moves (>100bp rates, >5% vol) can exceed reliable approximation range
//!
//! # Metric Unit Contracts
//!
//! This module expects metrics to follow these unit conventions:
//!
//! | Metric              | Unit            | Definition                                                |
//! |---------------------|-----------------|-----------------------------------------------------------|
//! | DV01                | $ / bp          | Dollar change per 1bp parallel rate shift                 |
//! | Convexity           | per-100 (street)| Street convexity: (∂²P/∂y²) / P / 100 (Bloomberg YAS)     |
//! | IrConvexity         | $ / decimal²    | Raw dollar second derivative ∂²PV/∂r² (swaps)             |
//! | CS01                | $ / bp          | Dollar change per 1bp spread shift                        |
//! | CsGamma             | $ / decimal²    | Dollar second derivative ∂²V/∂s² (spread in decimal)      |
//! | Vega                | $ / vol point   | Dollar change per 1% absolute vol shift                   |
//! | Volga               | $ / vol point²  | Dollar second derivative per vol point²                   |
//! | Theta               | $ / period      | Total-return carry over `theta_period_days`               |
//! | Inflation01         | $ / bp          | Dollar change per 1bp inflation-curve shift               |
//! | InflationConvexity  | $ / decimal²    | Dollar second derivative ∂²V/∂i² (inflation in decimal)   |
//!
//! **Important**: `Convexity` and `IrConvexity` have DIFFERENT producer
//! conventions and are consumed with different formulas :
//! the bond producer emits *street convexity* (`d²P/dy² / P / 100`,
//! Bloomberg YAS), so `ΔP_convexity = ½ × P₀ × Convexity × 100 × (Δr_decimal)²`;
//! the IRS producer emits the raw dollar second derivative `d²PV/dr²`, so
//! `ΔP_convexity = ½ × IrConvexity × (Δr_decimal)²` with no P₀ factor (a
//! near-par swap has PV ≈ 0 but real gamma).
//!
//! `InflationConvexity` uses the `CsGamma`-style $/decimal² convention (no P₀
//! multiplier): `ΔP_inflation_convexity = ½ × InflationConvexity × (Δi_decimal)²`.
//! A pricer emitting `InflationConvexity` in the dimensionless-percentage
//! convention used by `Convexity` would mis-attribute by a factor of P₀
//! (e.g. 1,000,000× for a $1M bond).
//!
//! If your convexity metric uses different units, apply the appropriate scaling
//! factor before passing to attribution.

mod carry;
mod context;
mod credit;
mod cross_factor;
mod equity;
mod fx;
mod inflation;
mod rates;
pub(crate) mod shifts;
mod volatility;

use crate::helpers::*;
use crate::{AttributionMethod, MoveUnit, PnlAttribution};
use context::MetricsContext;
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
/// Prefers bucketed DV01 (per-curve sensitivities) over aggregate DV01 for
/// rates attribution:
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
/// - Supplied carry metrics have a `theta_period_days` horizon different
///   from the attribution window. An absent horizon stamp is accepted only
///   for the default one-day window; recompute metrics for longer windows.
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
///     MetricId::BucketedDv01,
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

    let total_pnl = compute_pnl_with_fx(
        val_t0.value,
        val_t1.value,
        val_t1.value.currency(),
        market_t0,
        market_t1,
        as_of_t0,
        as_of_t1,
    )?;

    let mut attribution = PnlAttribution::new(
        total_pnl,
        instrument.id(),
        as_of_t0,
        as_of_t1,
        AttributionMethod::MetricsBased,
    );

    stamp_endpoints(&mut attribution, val_t0.value, val_t1.value);

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
    let realized_period_cash = Money::new(
        collect_cashflows_in_period(
            instrument.as_ref(),
            market_t0,
            as_of_t0,
            as_of_t1,
            val_t1.value.currency(),
        )?,
        val_t1.value.currency(),
    )?;
    attribution.total_pnl = attribution.total_pnl.checked_add(realized_period_cash)?;

    let inputs = MetricsContext::new(
        instrument, market_t0, market_t1, val_t0, val_t1, as_of_t0, as_of_t1,
    )?;
    if let Some(error) = &inputs.shifts.vol_shift_error {
        if inputs.val_t0.measures.keys().any(|id| {
            matches!(
                id.as_str(),
                "vega"
                    | "volga"
                    | "cross_gamma_spot_vol"
                    | "cross_gamma_rates_vol"
                    | "cross_gamma_credit_vol"
                    | "cross_gamma_fx_vol"
            )
        }) {
            attribution
                .meta
                .notes
                .push(format!("Volatility attribution failed: {error}"));
            non_finite_detected = true;
        }
    }
    carry::apply(&inputs, &mut attribution, &mut non_finite_detected)?;
    rates::apply(&inputs, &mut attribution, &mut non_finite_detected);
    credit::apply(&inputs, &mut attribution, &mut non_finite_detected);
    fx::apply(&inputs, &mut attribution, &mut non_finite_detected);
    volatility::apply(&inputs, &mut attribution, &mut non_finite_detected);
    equity::apply_spot(&inputs, &mut attribution, &mut non_finite_detected);
    cross_factor::apply(&inputs, &mut attribution, &mut non_finite_detected);
    inflation::apply(&inputs, &mut attribution, &mut non_finite_detected);

    if non_finite_detected {
        attribution.result_invalid = true;
    }

    finalize_attribution(
        &mut attribution,
        instrument.id(),
        "metrics_based",
        0,
        10.0,
        1.0,
    );

    Ok(attribution)
}

/// Run the metrics-based method on a repricing request.
///
/// Prices the T₀-parameter instrument at T₀ with `metrics` (its theta horizon
/// set to the attribution window) and the instrument at T₁, then attributes
/// with [`attribute_pnl_metrics_based`]. `None` requests the default menu
/// narrowed to what the instrument type supports: that menu is a superset, not
/// a request. Prepared endpoints on the request are not used, because the
/// method needs priced measures.
pub(crate) fn attribute_request(request: &crate::AttributionRequest<'_>) -> Result<PnlAttribution> {
    let crate::AttributionRequest {
        instrument,
        market_t0,
        market_t1,
        as_of_t0,
        as_of_t1,
        config,
        model_params_t0,
        ..
    } = *request;
    let instrument_t0 = instrument_at_t0(instrument, model_params_t0)?;
    let mut metrics_instrument = instrument_t0.clone_box();
    // A zero-day window has no carry (the metrics-based carry step zeroes it),
    // so the theta horizon is only set for a real window.
    let window_days = (as_of_t1 - as_of_t0).whole_days();
    if window_days > 0 {
        if let Some(overrides) = metrics_instrument.get_metric_pricing_overrides_mut() {
            let days = u32::try_from(window_days).map_err(|_| {
                finstack_quant_core::Error::Validation(format!(
                    "attribution window of {window_days} days exceeds the theta horizon range"
                ))
            })?;
            overrides.theta_period = Some(finstack_quant_core::dates::Tenor::new(
                days,
                finstack_quant_core::dates::TenorUnit::Days,
            )?);
        }
    }
    let metrics_instrument: Arc<dyn Instrument> = Arc::from(metrics_instrument);
    let metrics = match request.metrics {
        Some(metrics) => metrics.to_vec(),
        None => finstack_quant_valuations::metrics::standard_registry().applicable_subset(
            &crate::default_attribution_metrics(),
            metrics_instrument.key(),
        ),
    };

    // Attach FinstackConfig so sensitivity bump knobs (e.g. rate_bump_bp)
    // reach the producer instead of silently falling back to defaults.
    let pricing_options =
        finstack_quant_calibration::recalibration::pricing_options().with_config(config);
    let val_t0 = metrics_instrument.price_with_metrics(
        market_t0,
        as_of_t0,
        &metrics,
        pricing_options.clone(),
    )?;
    let val_t1 = instrument.price_with_metrics(market_t1, as_of_t1, &[], pricing_options)?;

    let mut attribution = attribute_pnl_metrics_based(
        &metrics_instrument,
        market_t0,
        market_t1,
        &val_t0,
        &val_t1,
        as_of_t0,
        as_of_t1,
    )?;
    attribution.meta.num_repricings = 2;
    if model_params_t0.is_some() {
        let closing_value_opening_params = instrument_t0.value(market_t1, as_of_t1)?;
        attribution.model_params_pnl = val_t1.value.checked_sub(closing_value_opening_params)?;
        let step = sensitivity_step(
            "ModelParameters",
            FactorWorking::Repriced {
                pv: closing_value_opening_params,
            },
            attribution.model_params_pnl,
            None,
            &mut attribution.meta.notes,
        );
        attribution.sensitivity_steps.push(step);
        attribution.meta.num_repricings += 1;
    }
    Ok(attribution)
}

fn note_warning(
    attribution: &mut PnlAttribution,
    message: impl Into<String>,
    instrument_id: &str,
    factor: &str,
) {
    let message = message.into();
    tracing::warn!(
        instrument_id = %instrument_id,
        factor,
        detail = %message,
        "Attribution soft warning"
    );
    attribution.meta.notes.push(message);
}

/// Append a second-order-only row: `gamma_pnl` from a convexity/gamma metric
/// applied to `market_move` (in `unit`); the first-order amount is zero.
fn push_second_order_step(
    attribution: &mut PnlAttribution,
    factor: &str,
    unit: MoveUnit,
    market_move: f64,
    gamma_pnl: f64,
) {
    let currency = attribution.total_pnl.currency();
    let step = sensitivity_step(
        factor,
        FactorWorking::SecondOrder { unit, market_move },
        Money::from((0_i64, currency)),
        Some(money_or_zero(gamma_pnl, currency)),
        &mut attribution.meta.notes,
    );
    attribution.sensitivity_steps.push(step);
}

/// Append a first-order row for `factor` whose P&L is `explained_pnl`.
fn push_first_order_step(
    attribution: &mut PnlAttribution,
    factor: &str,
    working: FactorWorking,
    explained_pnl: f64,
) {
    let currency = attribution.total_pnl.currency();
    let step = sensitivity_step(
        factor,
        working,
        money_or_zero(explained_pnl, currency),
        None,
        &mut attribution.meta.notes,
    );
    attribution.sensitivity_steps.push(step);
}

#[cfg(test)]
mod tests;
