//! Helper utilities for P&L attribution.
//!
//! Provides shared functions for market context manipulation, instrument repricing,
//! and common `PnlAttribution` assembly. Currency conversion itself lives on
//! [`MarketContext::convert_money`] — call sites here use it directly.

use super::types::{AttributionMethod, CarryDetail, PnlAttribution, SourceLine};
use finstack_quant_core::config::FinstackConfig;
use finstack_quant_core::currency::Currency;
use finstack_quant_core::dates::Date;
use finstack_quant_core::market_data::context::MarketContext;
use finstack_quant_core::market_data::term_structures::DiscountCurve;
use finstack_quant_core::math::interp::InterpStyle;
use finstack_quant_core::money::fx::{FxConversionPolicy, FxPolicyMeta};
use finstack_quant_core::money::Money;
use finstack_quant_core::Result;
use finstack_quant_valuations::instruments::Instrument;
use finstack_quant_valuations::instruments::PricingOptions;
use finstack_quant_valuations::metrics::collect_cashflows_in_period;
use finstack_quant_valuations::metrics::MetricId;
use std::sync::Arc;

/// Reprice an instrument at a given date with a market context.
///
/// # Arguments
///
/// * `instrument` - Instrument to price
/// * `market` - Market data context
/// * `as_of` - Valuation date
///
/// # Returns
///
/// Present value in the instrument's native currency.
///
/// # Errors
///
/// Returns error if pricing fails (missing curves, invalid parameters, etc.).
pub(crate) fn reprice_instrument(
    instrument: &Arc<dyn Instrument>,
    market: &MarketContext,
    as_of: Date,
) -> Result<Money> {
    instrument.value(market, as_of)
}

/// Compute P&L between two valuations in target currency.
///
/// Converts both valuations to target currency before computing difference.
///
/// # Arguments
///
/// * `val_t0` - Value at T₀
/// * `val_t1` - Value at T₁
/// * `target_ccy` - Currency for P&L
/// * `market_t1` - Market context at T₁ (for FX conversion)
/// * `as_of_t1` - Date at T₁
///
/// # Returns
///
/// P&L in target currency (val_t1 - val_t0).
///
/// # Errors
///
/// Returns error if currency conversion fails.
pub fn compute_pnl(
    val_t0: Money,
    val_t1: Money,
    target_ccy: Currency,
    market_t1: &MarketContext,
    as_of_t1: Date,
) -> Result<Money> {
    let val_t0_converted = market_t1.convert_money(val_t0, target_ccy, as_of_t1)?;
    let val_t1_converted = market_t1.convert_money(val_t1, target_ccy, as_of_t1)?;

    val_t1_converted.checked_sub(val_t0_converted)
}

/// Compute P&L with explicit FX conversion for each date.
///
/// This allows proper isolation of FX translation effects by using
/// date-appropriate FX rates for conversion.
///
/// # Arguments
///
/// * `val_t0` - Value at T₀
/// * `val_t1` - Value at T₁
/// * `target_ccy` - Currency for P&L
/// * `market_fx_t0` - Market context at T₀ (for T₀ FX conversion)
/// * `market_fx_t1` - Market context at T₁ (for T₁ FX conversion)
/// * `as_of_t0` - Date at T₀
/// * `as_of_t1` - Date at T₁
///
/// # Returns
///
/// P&L in target currency with FX translation properly isolated.
///
/// # Errors
///
/// Returns error if currency conversion fails.
///
/// # Examples
///
/// ```ignore
/// use finstack_quant_core::currency::Currency;
/// use finstack_quant_core::market_data::context::MarketContext;
/// use finstack_quant_core::money::Money;
/// use finstack_quant_attribution::compute_pnl_with_fx;
/// use time::macros::date;
///
/// # fn main() -> Result<(), Box<dyn std::error::Error>> {
/// // For FX attribution: convert T₀ value with T₀ FX, T₁ value with T₁ FX
/// let fx_pnl = compute_pnl_with_fx(
///     Money::new(1_000_000.0, Currency::EUR),
///     Money::new(1_100_000.0, Currency::EUR),
///     Currency::USD,
///     &MarketContext::new(),
///     &MarketContext::new(),
///     date!(2025-01-15),
///     date!(2025-01-16),
/// )?;
/// # let _ = fx_pnl;
/// # Ok(())
/// # }
/// ```
pub fn compute_pnl_with_fx(
    val_t0: Money,
    val_t1: Money,
    target_ccy: Currency,
    market_fx_t0: &MarketContext,
    market_fx_t1: &MarketContext,
    as_of_t0: Date,
    as_of_t1: Date,
) -> Result<Money> {
    let val_t0_converted = market_fx_t0.convert_money(val_t0, target_ccy, as_of_t0)?;
    let val_t1_converted = market_fx_t1.convert_money(val_t1, target_ccy, as_of_t1)?;

    val_t1_converted.checked_sub(val_t0_converted)
}

pub(crate) fn init_attribution(
    total_pnl: Money,
    instrument_id: &str,
    as_of_t0: Date,
    as_of_t1: Date,
    method: AttributionMethod,
    config: Option<&FinstackConfig>,
) -> PnlAttribution {
    match config {
        Some(config) => PnlAttribution::new_with_rounding(
            total_pnl,
            instrument_id,
            as_of_t0,
            as_of_t1,
            method,
            finstack_quant_core::config::rounding_context_from(config),
        ),
        None => PnlAttribution::new(total_pnl, instrument_id, as_of_t0, as_of_t1, method),
    }
}

/// Raw, repricing-derived inputs for the full-window carry decomposition.
pub(crate) struct TotalReturnCarryInputs {
    /// Coupons whose PAYMENT date falls in `[t0, t1)` (drives carry total + total_pnl).
    pub cash_paid: Money,
    /// `accrued(t1) - accrued(t0)` (curve-independent); `None` when the instrument has no `Accrued`.
    pub delta_accrued: Option<Money>,
    /// `F_t1 - F_t0` on a flat-YTM(t0) curve (basis cancels); `None` when `Ytm`/flat pricing is unavailable.
    pub flat_window_diff: Option<Money>,
    /// Diagnostics for the caller to merge into `meta.notes`.
    pub warnings: Vec<String>,
    /// True when a non-finite cashflow/metric value was zeroed; the caller
    /// must set `result_invalid` so tolerance checks refuse a clean pass.
    pub invalid: bool,
}

/// Gather the repricing-based pieces of the carry decomposition over `[as_of_t0, as_of_t1]`,
/// pricing on `market` (the market on which `theta` was computed: `market_t0` for the parallel
/// path, the accumulated market for the waterfall path). Accrued and YTM are read via the
/// instrument's metrics; the flat-YTM window values isolate the constant-yield aging and
/// curve-shape effects with the flat-vs-market level basis cancelled.
pub(crate) fn total_return_carry_inputs(
    instrument: &dyn Instrument,
    market: &MarketContext,
    as_of_t0: Date,
    as_of_t1: Date,
    currency: Currency,
) -> TotalReturnCarryInputs {
    let mut warnings = Vec::new();
    let mut invalid = false;

    let cash_paid =
        match collect_cashflows_in_period(instrument, market, as_of_t0, as_of_t1, currency) {
            Ok(value) => factor_money_or_invalid(
                value,
                currency,
                "carry cash income",
                &mut warnings,
                &mut invalid,
            ),
            Err(e) => {
                warnings.push(format!("carry cash income unavailable: {e}"));
                Money::new(0.0, currency)
            }
        };

    let accrued_at = |as_of: Date| -> Option<f64> {
        instrument
            .price_with_metrics(
                market,
                as_of,
                &[MetricId::Accrued],
                PricingOptions::default(),
            )
            .ok()
            .and_then(|r| r.measures.get(MetricId::Accrued.as_str()).copied())
            .filter(|v| v.is_finite())
    };
    let delta_accrued = match (accrued_at(as_of_t0), accrued_at(as_of_t1)) {
        (Some(a0), Some(a1)) => Some(Money::new(a1 - a0, currency)),
        _ => None,
    };

    let flat_window_diff = flat_window_diff(instrument, market, as_of_t0, as_of_t1, currency);

    TotalReturnCarryInputs {
        cash_paid,
        delta_accrued,
        flat_window_diff,
        warnings,
        invalid,
    }
}

/// `F_t1 - F_t0` on a flat-YTM(t0) curve, or `None` if YTM / flat pricing is unavailable.
fn flat_window_diff(
    instrument: &dyn Instrument,
    market: &MarketContext,
    as_of_t0: Date,
    as_of_t1: Date,
    currency: Currency,
) -> Option<Money> {
    let ytm = instrument
        .price_with_metrics(
            market,
            as_of_t0,
            &[MetricId::Ytm],
            PricingOptions::default(),
        )
        .ok()
        .and_then(|r| r.measures.get(MetricId::Ytm.as_str()).copied())
        .filter(|y| y.is_finite())?;
    let flat = build_flat_ytm_market(instrument, market, ytm).ok()?;
    let f_t0 = instrument.value(&flat, as_of_t0).ok()?.amount();
    let f_t1 = instrument.value(&flat, as_of_t1).ok()?.amount();
    if f_t0.is_finite() && f_t1.is_finite() {
        Some(Money::new(f_t1 - f_t0, currency))
    } else {
        None
    }
}

/// Build a market whose discount curve is replaced by a flat-YTM curve `DF(t) = exp(-ytm·t)`.
/// Ported from `valuations`'s private `build_flat_curve_market` (not reachable cross-crate).
fn build_flat_ytm_market(
    instrument: &dyn Instrument,
    market: &MarketContext,
    ytm: f64,
) -> Result<MarketContext> {
    let curve_id = instrument
        .market_dependencies()?
        .curve_dependencies()
        .discount_curves
        .first()
        .cloned()
        .ok_or_else(|| finstack_quant_core::InputError::NotFound {
            id: format!("discount_curve_for:{}", instrument.id()),
        })?;
    let original = market.get_discount(curve_id.as_str())?;
    let knots: Vec<(f64, f64)> = (0..=120)
        .map(|i| {
            let t = i as f64 * 0.5;
            (t, (-ytm * t).exp())
        })
        .collect();
    let flat_curve = DiscountCurve::builder(curve_id.as_str())
        .base_date(original.base_date())
        .day_count(original.day_count())
        .knots(knots)
        .interp(InterpStyle::LogLinear)
        .build()?;
    Ok(market.clone().insert(flat_curve))
}

/// Assemble the carry total + the fully-labeled detail partition.
///
/// `carry_total = theta + cash_paid` (unchanged); `total_pnl += cash_paid`. The detail:
/// `coupon_income = Δaccrued + cash`, `pull_to_par = (F_t1−F_t0) − Δaccrued`,
/// `roll_down = theta − (F_t1−F_t0)`, which sum to `carry_total`. When accrual / flat pricing is
/// unavailable (non-bonds), falls back to `coupon_income = cash`, `pull_to_par = None`, and the
/// whole price-carry residual goes to `roll_down`. The populated detail lines always partition `total`.
pub(crate) fn apply_total_return_carry(
    attribution: &mut PnlAttribution,
    theta: Money,
    inputs: TotalReturnCarryInputs,
) -> Result<()> {
    attribution.carry = theta.checked_add(inputs.cash_paid)?;
    if inputs.cash_paid.amount().abs() > 0.0 {
        attribution.total_pnl = attribution.total_pnl.checked_add(inputs.cash_paid)?;
    }

    let coupon_income = match inputs.delta_accrued {
        Some(da) => da.checked_add(inputs.cash_paid)?,
        None => inputs.cash_paid,
    };
    let (pull_to_par, roll_down) = match (inputs.delta_accrued, inputs.flat_window_diff) {
        (Some(da), Some(fd)) => (Some(fd.checked_sub(da)?), Some(theta.checked_sub(fd)?)),
        // Fallback (no accrual / flat split, e.g. non-bonds): the whole price-carry residual
        // goes to roll_down so `coupon_income + roll_down = total` still holds.
        _ => (None, Some(attribution.carry.checked_sub(coupon_income)?)),
    };

    attribution.carry_detail = Some(CarryDetail {
        total: attribution.carry,
        coupon_income: Some(SourceLine::scalar(coupon_income)),
        pull_to_par,
        roll_down: roll_down.map(SourceLine::scalar),
        funding_cost: None,
    });
    Ok(())
}

pub(crate) fn stamp_fx_policy(
    attribution: &mut PnlAttribution,
    target_ccy: Currency,
    notes: impl Into<String>,
) {
    attribution.meta.fx_policy = Some(FxPolicyMeta {
        strategy: FxConversionPolicy::CashflowDate,
        target_ccy: Some(target_ccy),
        notes: notes.into(),
    });
}

pub(crate) fn note_warning(
    attribution: &mut PnlAttribution,
    message: impl Into<String>,
    instrument_id: &str,
    factor: &str,
) {
    let message = message.into();
    tracing::warn!(
        instrument_id = %instrument_id,
        factor,
        message = %message,
        "Attribution soft warning"
    );
    attribution.meta.notes.push(message);
}

pub(crate) fn finalize_attribution(
    attribution: &mut PnlAttribution,
    instrument_id: &str,
    method: &str,
    num_repricings: usize,
    tolerance_abs: f64,
    tolerance_pct: f64,
) {
    if let Err(e) = attribution.compute_residual() {
        tracing::warn!(
            error = %e,
            instrument_id = %instrument_id,
            method,
            "Residual computation failed; attribution may be incomplete"
        );
    }

    attribution.meta.num_repricings = num_repricings;
    attribution.meta.tolerance_abs = tolerance_abs;
    attribution.meta.tolerance_pct = tolerance_pct;
}

/// Construct a factor P&L [`Money`] from a computed `f64` amount.
///
/// If `amount` is non-finite (NaN or ±Inf), this function:
/// - Appends a diagnostic note to `notes`,
/// - Sets `*result_invalid = true` so [`crate::PnlAttribution::result_invalid`]
///   is propagated to callers, and
/// - Returns a **zero sentinel** in `currency` so the attribution can continue
///   and produce a complete (though flagged-invalid) result rather than
///   panicking inside [`Money::new`], which panics on non-finite input.
///
/// For finite amounts it delegates directly to [`Money::new`].
#[inline]
pub(crate) fn factor_money_or_invalid(
    amount: f64,
    currency: Currency,
    label: &str,
    notes: &mut Vec<String>,
    result_invalid: &mut bool,
) -> Money {
    if amount.is_finite() {
        Money::new(amount, currency)
    } else {
        notes.push(format!(
            "Non-finite factor P&L ({amount:?}) for {label}; attribution flagged invalid"
        ));
        *result_invalid = true;
        Money::new(0.0, currency)
    }
}

/// Validate that the attribution period is well-formed: `as_of_t1 >= as_of_t0`.
///
/// A reversed period silently flips the sign of theta / carry (`time_period_days`
/// goes negative) and produces a nonsensical decomposition, so it is rejected
/// at every attribution entry point. A zero-length period (`t1 == t0`) is
/// permitted — same-day attribution is a degenerate but valid request, with
/// theta zero over zero elapsed time.
pub(crate) fn validate_attribution_period(as_of_t0: Date, as_of_t1: Date) -> Result<()> {
    if as_of_t1 < as_of_t0 {
        return Err(finstack_quant_core::Error::Validation(format!(
            "attribution period is reversed: as_of_t1 ({as_of_t1}) precedes as_of_t0 ({as_of_t0})"
        )));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use finstack_quant_core::money::fx::{FxConversionPolicy, FxMatrix, FxProvider};
    use finstack_quant_core::Error;
    use std::sync::Arc;
    use time::macros::date;

    // Simple test FX provider
    struct TestFx;
    impl FxProvider for TestFx {
        fn rate(
            &self,
            from: Currency,
            to: Currency,
            _on: Date,
            _policy: FxConversionPolicy,
        ) -> Result<f64> {
            if from == Currency::EUR && to == Currency::USD {
                Ok(1.1)
            } else if from == Currency::USD && to == Currency::EUR {
                Ok(1.0 / 1.1)
            } else if from == to {
                Ok(1.0)
            } else {
                Err(Error::Validation("FX rate not found".to_string()))
            }
        }
    }

    #[test]
    fn validate_attribution_period_accepts_forward_and_same_day() {
        assert!(
            validate_attribution_period(date!(2025 - 01 - 15), date!(2025 - 01 - 16)).is_ok(),
            "a forward period must be accepted"
        );
        // Same-day attribution is degenerate but permitted (theta over zero days).
        assert!(
            validate_attribution_period(date!(2025 - 01 - 15), date!(2025 - 01 - 15)).is_ok(),
            "a zero-length period must be accepted"
        );
    }

    #[test]
    fn validate_attribution_period_rejects_reversed_period() {
        assert!(
            validate_attribution_period(date!(2025 - 01 - 16), date!(2025 - 01 - 15)).is_err(),
            "a reversed period (t1 < t0) must be rejected"
        );
    }

    #[test]
    fn test_compute_pnl() {
        let val_t0 = Money::new(1000.0, Currency::EUR);
        let val_t1 = Money::new(1100.0, Currency::EUR);
        let fx = FxMatrix::new(Arc::new(TestFx));
        let market = MarketContext::new().insert_fx(fx);
        let as_of = date!(2025 - 01 - 15);

        let pnl = compute_pnl(val_t0, val_t1, Currency::USD, &market, as_of)
            .expect("PNL computation should succeed in test");
        // (1100 - 1000) EUR * 1.1 = 110 USD
        assert_eq!(pnl.amount(), 110.0);
        assert_eq!(pnl.currency(), Currency::USD);
    }

    #[test]
    fn test_compute_pnl_with_fx() {
        // Test FX translation isolation
        let pv = Money::new(1000.0, Currency::EUR);

        // T0 market: EUR/USD = 1.1
        let fx_t0 = FxMatrix::new(Arc::new(TestFx));
        let market_t0 = MarketContext::new().insert_fx(fx_t0);

        // T1 market: EUR/USD = 1.2 (10% appreciation)
        struct TestFxT1;
        impl FxProvider for TestFxT1 {
            fn rate(
                &self,
                from: Currency,
                to: Currency,
                _on: Date,
                _policy: FxConversionPolicy,
            ) -> Result<f64> {
                if from == Currency::EUR && to == Currency::USD {
                    Ok(1.2)
                } else if from == Currency::USD && to == Currency::EUR {
                    Ok(1.0 / 1.2)
                } else if from == to {
                    Ok(1.0)
                } else {
                    Err(Error::Validation("FX rate not found".to_string()))
                }
            }
        }
        let fx_t1 = FxMatrix::new(Arc::new(TestFxT1));
        let market_t1 = MarketContext::new().insert_fx(fx_t1);

        let as_of_t0 = date!(2025 - 01 - 15);
        let as_of_t1 = date!(2025 - 01 - 16);

        // PV unchanged in EUR, but FX moved
        let pnl = compute_pnl_with_fx(
            pv,
            pv,
            Currency::USD,
            &market_t0,
            &market_t1,
            as_of_t0,
            as_of_t1,
        )
        .expect("PNL computation with FX should succeed in test");

        // FX translation: 1000 EUR @ 1.2 - 1000 EUR @ 1.1 = 1200 - 1100 = 100 USD
        assert_eq!(pnl.amount(), 100.0);
        assert_eq!(pnl.currency(), Currency::USD);
    }
}
