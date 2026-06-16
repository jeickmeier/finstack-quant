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

/// Populate carry and `carry_detail` for the Taylor (total-return) path.
///
/// `roll_down` is intentionally `None` for the Taylor path: Taylor's
/// first-order approximation does not separately track time-decay vs
/// spread-shift contributions, so roll-down is rolled into the bond's total
/// return. PR-8b's carry credit decomposition handles `coupon_income.split`
/// but skips `roll_down.split` when this field is `None`.
pub(crate) fn apply_total_return_carry(
    attribution: &mut PnlAttribution,
    theta: Money,
    coupon_income: Money,
    roll_down: Option<Money>,
) -> Result<()> {
    attribution.carry = theta.checked_add(coupon_income)?;
    if coupon_income.amount().abs() > 0.0 {
        attribution.total_pnl = attribution.total_pnl.checked_add(coupon_income)?;
    }
    attribution.carry_detail = Some(CarryDetail {
        total: attribution.carry,
        coupon_income: Some(SourceLine::scalar(coupon_income)),
        pull_to_par: None,
        roll_down: roll_down.map(SourceLine::scalar),
        funding_cost: None,
        theta: Some(theta),
    });
    Ok(())
}

pub(crate) struct TotalReturnCarryInputs {
    pub coupon_income: Money,
    pub roll_down: Option<Money>,
    /// Diagnostics for the caller to merge into `meta.notes`.
    pub warnings: Vec<String>,
    /// True when a non-finite cashflow/metric value was zeroed; the caller
    /// must set `result_invalid` so tolerance checks refuse a clean pass.
    pub invalid: bool,
}

pub(crate) fn total_return_carry_inputs(
    instrument: &dyn Instrument,
    cashflow_market: &MarketContext,
    roll_down_market: &MarketContext,
    as_of_t0: Date,
    as_of_t1: Date,
    currency: Currency,
) -> TotalReturnCarryInputs {
    let mut warnings = Vec::new();
    let mut invalid = false;

    // A failed cashflow collection must be VISIBLE: coupon income feeds
    // `total_pnl` via `apply_total_return_carry`, so silently defaulting to
    // zero would flip the attribution from total-return to MTM-only with no
    // observability .
    let coupon_income = match collect_cashflows_in_period(
        instrument,
        cashflow_market,
        as_of_t0,
        as_of_t1,
        currency,
    ) {
        Ok(value) => factor_money_or_invalid(
            value,
            currency,
            "carry coupon income",
            &mut warnings,
            &mut invalid,
        ),
        Err(e) => {
            tracing::warn!(
                instrument_id = instrument.id(),
                error = %e,
                "cashflow collection failed; coupon income omitted from total-return carry"
            );
            warnings.push(format!(
                "Carry coupon income unavailable (cashflow collection failed: {e}); \
                     total-return adjustment skipped — total_pnl is MTM-only for this period"
            ));
            Money::new(0.0, currency)
        }
    };

    let roll_down_opt = if let Ok(val_res) = instrument.price_with_metrics(
        roll_down_market,
        as_of_t0,
        &[MetricId::RollDown],
        PricingOptions::default(),
    ) {
        val_res.measures.get(MetricId::RollDown.as_str()).copied()
    } else {
        None
    };
    let time_period_days = (as_of_t1 - as_of_t0).whole_days() as f64;
    let roll_down = roll_down_opt.map(|rd| {
        factor_money_or_invalid(
            rd * time_period_days,
            currency,
            "carry roll-down",
            &mut warnings,
            &mut invalid,
        )
    });

    TotalReturnCarryInputs {
        coupon_income,
        roll_down,
        warnings,
        invalid,
    }
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
