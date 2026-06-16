//! Post-hoc translation of a native-currency `PnlAttribution` into a
//! reporting currency (`target_ccy`).
//!
//! By default every per-instrument attribution method reports in the
//! instrument's own pricing currency (`val_t1.currency()`). When a portfolio
//! report wants a single rolled-up currency (e.g. a multi-currency book
//! reporting in USD), the native-currency attribution is translated here:
//!
//! - Each factor P&L is converted from native to target using `market_t1`'s
//!   FX at `as_of_t1`.
//! - A new `fx_translation_pnl` component captures the FX impact on the
//!   **opening position**:
//!
//! ```text
//! fx_translation_pnl = val_t0_native × (T1_fx − T0_fx)
//!                    = val_t0_target_at_T1 − val_t0_target_at_T0
//! ```
//!
//! The translated decomposition reconciles cleanly:
//!
//! ```text
//! total_pnl_target
//!   = val_t1_target − val_t0_target
//!   = val_t1_native × T1_fx − val_t0_native × T0_fx
//!   ≡ Σ_factor (factor_native × T1_fx) + fx_translation_pnl
//! ```
//!
//! This split treats the existing `fx_pnl` field as **pricing-impact** FX
//! (the FX matrix feeding into a cross-currency instrument's own pricer) and
//! the new `fx_translation_pnl` as **reporting-currency** FX (the translation
//! adjustment).

use finstack_quant_core::currency::Currency;
use finstack_quant_core::dates::Date;
use finstack_quant_core::market_data::context::MarketContext;
use finstack_quant_core::money::fx::{FxConversionPolicy, FxPolicyMeta};
use finstack_quant_core::money::Money;
use finstack_quant_core::Result;

use crate::types::{CarryDetail, PnlAttribution, SourceLine};

/// Translate a populated `PnlAttribution` from its native pricing currency
/// into `target_ccy`.
///
/// # Arguments
///
/// * `attribution` - the populated attribution, in `val_t1.currency()`.
/// * `val_t0` - opening native-currency value (used to compute the
///   translation adjustment).
/// * `target_ccy` - reporting currency to translate into.
/// * `market_t0`, `market_t1` - market states (carry the FX matrices used for
///   the T0 / T1 conversions).
/// * `as_of_t0`, `as_of_t1` - valuation dates passed to `convert_money`.
///
/// # Behavior
///
/// - If `target_ccy == attribution.total_pnl.currency()`, this is a no-op
///   (no field is mutated).
/// - Otherwise every factor P&L is converted via `market_t1.convert_money`
///   at `as_of_t1`; `fx_translation_pnl` is set to
///   `val_t0 × (T1_fx − T0_fx)`; `total_pnl` is replaced by
///   `val_t1_target − val_t0_target`. Detail breakdowns (rates_detail,
///   credit_detail, fx_detail, ...) are NOT translated by this helper —
///   their key amounts remain in native currency. The aggregate fields are
///   the supported reporting surface in target_ccy.
/// - The `meta.fx_policy` is stamped with `target_ccy` and a note describing
///   the translation.
///
/// # Errors
///
/// Returns an error if any FX conversion fails (typically because the FX
/// matrix lacks the native→target rate).
pub fn translate_to_target_ccy(
    attribution: &mut PnlAttribution,
    val_t0: Money,
    target_ccy: Currency,
    market_t0: &MarketContext,
    market_t1: &MarketContext,
    as_of_t0: Date,
    as_of_t1: Date,
) -> Result<()> {
    let native_ccy = attribution.total_pnl.currency();
    if native_ccy == target_ccy {
        return Ok(()); // No-op: report stays in native currency.
    }

    // Convert val_t0 with BOTH the T0 and T1 FX matrices so we can extract the
    // FX move applied to the opening position.
    let val_t0_at_t0 = market_t0.convert_money(val_t0, target_ccy, as_of_t0)?;
    let val_t0_at_t1 = market_t1.convert_money(val_t0, target_ccy, as_of_t1)?;
    let fx_translation = val_t0_at_t1.checked_sub(val_t0_at_t0)?;

    // Translate every per-factor amount to target_ccy at T1 FX.
    let translate =
        |m: Money| -> Result<Money> { market_t1.convert_money(m, target_ccy, as_of_t1) };

    attribution.carry = translate(attribution.carry)?;
    attribution.rates_curves_pnl = translate(attribution.rates_curves_pnl)?;
    attribution.credit_curves_pnl = translate(attribution.credit_curves_pnl)?;
    attribution.inflation_curves_pnl = translate(attribution.inflation_curves_pnl)?;
    attribution.correlations_pnl = translate(attribution.correlations_pnl)?;
    attribution.fx_pnl = translate(attribution.fx_pnl)?;
    attribution.vol_pnl = translate(attribution.vol_pnl)?;
    attribution.cross_factor_pnl = translate(attribution.cross_factor_pnl)?;
    attribution.model_params_pnl = translate(attribution.model_params_pnl)?;
    attribution.market_scalars_pnl = translate(attribution.market_scalars_pnl)?;
    attribution.fx_translation_pnl = fx_translation;

    // Total in target_ccy = MTM translation + the total-return add-back.
    //
    // Native `total_pnl` follows the total-return convention: the methods add
    // intra-period coupon income on top of the raw MTM (`mark_to_market_pnl`)
    // via `apply_total_return_carry`. The MTM component is rebuilt from the
    // T0/T1 values; the coupon add-back (total − MTM, zero when no cashflows
    // occurred) must travel at T1 FX — the same rate at which the translated
    // `carry` still contains it — or the recomputed residual is polluted by
    // the full coupon and `total_pnl` silently flips to MTM-only (quant
    // review M6).
    let native_total_pnl = attribution.total_pnl;
    let native_mtm = attribution.mark_to_market_pnl.unwrap_or(native_total_pnl);
    let coupon_addback_native = native_total_pnl.checked_sub(native_mtm)?;

    let val_t1_native = val_t0.checked_add(native_mtm)?;
    let val_t1_at_t1 = market_t1.convert_money(val_t1_native, target_ccy, as_of_t1)?;
    let translated_mtm = val_t1_at_t1.checked_sub(val_t0_at_t0)?;
    attribution.total_pnl = translated_mtm.checked_add(translate(coupon_addback_native)?)?;

    // mark_to_market_pnl in target_ccy retains the raw price change interpretation.
    if let Some(_mtm) = attribution.mark_to_market_pnl {
        attribution.mark_to_market_pnl = Some(translated_mtm);
    }

    // Residual is recomputed against the translated sum.
    attribution.residual = Money::new(0.0, target_ccy);

    // Stamp the FX policy so downstream consumers know the report currency is
    // a translation, not native.
    attribution.meta.fx_policy = Some(FxPolicyMeta {
        strategy: FxConversionPolicy::CashflowDate,
        target_ccy: Some(target_ccy),
        notes: format!(
            "translated from {native_ccy} to {target_ccy} (factors at T1 FX; \
             fx_translation_pnl = val_t0 × (T1_fx − T0_fx))"
        ),
    });

    // Carry-detail fields are typed Money; translate them so callers reading
    // carry_detail.total etc. see consistent target-currency amounts. Detail
    // maps (rates_detail.by_curve, fx_detail.by_pair, ...) remain in native
    // currency — see the doc on this function.
    if let Some(d) = attribution.carry_detail.as_mut() {
        translate_carry_detail(d, target_ccy, market_t1, as_of_t1)?;
    }

    attribution.compute_residual()?;
    Ok(())
}

fn translate_carry_detail(
    detail: &mut CarryDetail,
    target_ccy: Currency,
    market_t1: &MarketContext,
    as_of_t1: Date,
) -> Result<()> {
    let convert = |m: Money| -> Result<Money> { market_t1.convert_money(m, target_ccy, as_of_t1) };
    detail.total = convert(detail.total)?;
    if let Some(line) = detail.coupon_income.as_mut() {
        translate_source_line(line, target_ccy, market_t1, as_of_t1)?;
    }
    if let Some(m) = detail.pull_to_par.as_mut() {
        *m = convert(*m)?;
    }
    if let Some(line) = detail.roll_down.as_mut() {
        translate_source_line(line, target_ccy, market_t1, as_of_t1)?;
    }
    if let Some(m) = detail.funding_cost.as_mut() {
        *m = convert(*m)?;
    }
    if let Some(m) = detail.theta.as_mut() {
        *m = convert(*m)?;
    }
    Ok(())
}

fn translate_source_line(
    line: &mut SourceLine,
    target_ccy: Currency,
    market_t1: &MarketContext,
    as_of_t1: Date,
) -> Result<()> {
    let convert = |m: Money| -> Result<Money> { market_t1.convert_money(m, target_ccy, as_of_t1) };
    line.total = convert(line.total)?;
    if let Some(m) = line.rates_part.as_mut() {
        *m = convert(*m)?;
    }
    if let Some(m) = line.credit_part.as_mut() {
        *m = convert(*m)?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::{AttributionMethod, PnlAttribution};
    use finstack_quant_core::money::fx::{FxConversionPolicy, FxMatrix, FxProvider};
    use finstack_quant_core::Error;
    use std::sync::Arc;
    use time::macros::date;

    /// Test FX provider with controllable EUR/USD rate.
    struct FixedEurUsd(f64);
    impl FxProvider for FixedEurUsd {
        fn rate(
            &self,
            from: Currency,
            to: Currency,
            _on: Date,
            _policy: FxConversionPolicy,
        ) -> Result<f64> {
            if from == to {
                Ok(1.0)
            } else if from == Currency::EUR && to == Currency::USD {
                Ok(self.0)
            } else if from == Currency::USD && to == Currency::EUR {
                Ok(1.0 / self.0)
            } else {
                Err(Error::Validation("FX rate not found".to_string()))
            }
        }
    }

    fn market(fx_rate: f64) -> MarketContext {
        MarketContext::new().insert_fx(FxMatrix::new(Arc::new(FixedEurUsd(fx_rate))))
    }

    #[test]
    fn translate_is_noop_when_target_equals_native() {
        let total = Money::new(100.0, Currency::USD);
        let mut attr = PnlAttribution::new(
            total,
            "TEST",
            date!(2025 - 01 - 15),
            date!(2025 - 01 - 16),
            AttributionMethod::Parallel,
        );
        attr.carry = Money::new(40.0, Currency::USD);
        attr.rates_curves_pnl = Money::new(60.0, Currency::USD);
        let snapshot = attr.clone();

        translate_to_target_ccy(
            &mut attr,
            Money::new(1000.0, Currency::USD),
            Currency::USD,
            &market(1.0),
            &market(1.0),
            date!(2025 - 01 - 15),
            date!(2025 - 01 - 16),
        )
        .expect("no-op translate");

        // Every field is unchanged when target == native.
        assert_eq!(attr.total_pnl, snapshot.total_pnl);
        assert_eq!(attr.carry, snapshot.carry);
        assert_eq!(attr.rates_curves_pnl, snapshot.rates_curves_pnl);
        assert_eq!(attr.fx_translation_pnl, snapshot.fx_translation_pnl);
        assert!(attr.fx_translation_pnl.amount() == 0.0);
    }

    #[test]
    fn translate_eur_attribution_to_usd_produces_translation_pnl() {
        // An EUR-denominated instrument with EUR 100 of native P&L; we
        // translate into USD where the FX rate moved from 1.10 to 1.20.
        // val_t0 = 1000 EUR, val_t1 = 1100 EUR (P&L = +100 EUR).
        //
        // Expected target totals:
        //   val_t0_at_t0 = 1000 × 1.10 = 1100 USD
        //   val_t1_at_t1 = 1100 × 1.20 = 1320 USD
        //   total_target = 1320 − 1100 = 220 USD
        //
        // Decomposition:
        //   translated_native_pnl = 100 × 1.20 = 120 USD (factor side)
        //   fx_translation_pnl    = 1000 × (1.20 − 1.10) = 100 USD
        //   sum                   = 220 USD ✓
        let val_t0_native = Money::new(1000.0, Currency::EUR);
        let native_pnl = Money::new(100.0, Currency::EUR);
        let mut attr = PnlAttribution::new(
            native_pnl,
            "EUR-BOND",
            date!(2025 - 01 - 15),
            date!(2025 - 01 - 16),
            AttributionMethod::Parallel,
        );
        attr.rates_curves_pnl = Money::new(80.0, Currency::EUR);
        attr.carry = Money::new(20.0, Currency::EUR);
        // Force consistent zero-residual starting point.
        attr.compute_residual().expect("residual");
        translate_and_assert_eur_to_usd(attr, val_t0_native);
    }

    /// translation must preserve the total-return convention.
    /// When the native attribution carries coupon income (total_pnl = MTM +
    /// coupon, carry includes the coupon), the translated total must add the
    /// coupon back at T1 FX — the rate the translated carry contains it at —
    /// or the recomputed residual is polluted by the full coupon.
    #[test]
    fn translate_preserves_total_return_coupon_addback() {
        let val_t0_native = Money::new(1000.0, Currency::EUR);
        // Raw MTM = +100 EUR; apply_total_return_carry added a 30 EUR coupon:
        // total = 130, carry = theta 20 + coupon 30 = 50, rates = 80.
        let mut attr = PnlAttribution::new(
            Money::new(100.0, Currency::EUR),
            "EUR-BOND-COUPON",
            date!(2025 - 01 - 15),
            date!(2025 - 01 - 16),
            AttributionMethod::Parallel,
        );
        attr.total_pnl = Money::new(130.0, Currency::EUR);
        attr.carry = Money::new(50.0, Currency::EUR);
        attr.rates_curves_pnl = Money::new(80.0, Currency::EUR);
        attr.compute_residual().expect("residual");
        assert!(
            attr.residual.amount().abs() < 1e-12,
            "native attribution must start reconciled"
        );

        translate_to_target_ccy(
            &mut attr,
            val_t0_native,
            Currency::USD,
            &market(1.10),
            &market(1.20),
            date!(2025 - 01 - 15),
            date!(2025 - 01 - 16),
        )
        .expect("translate");

        // translated_mtm = 1100×1.20 − 1000×1.10 = 220 USD;
        // coupon add-back at T1 FX = 30 × 1.20 = 36 USD → total 256 USD.
        assert!(
            (attr.total_pnl.amount() - 256.0).abs() < 1e-9,
            "total must keep the total-return convention, got {}",
            attr.total_pnl.amount()
        );
        assert!(
            (attr.mark_to_market_pnl.expect("mtm").amount() - 220.0).abs() < 1e-9,
            "mark_to_market_pnl must stay the raw translated MTM"
        );
        // carry 50×1.2 = 60, rates 80×1.2 = 96, fx_translation 1000×0.1 = 100
        // → attributed 256 = total: residual must remain ~0, not −coupon×fx1.
        assert!(
            attr.residual.amount().abs() < 1e-9,
            "translated residual must not absorb the coupon, got {}",
            attr.residual.amount()
        );
    }

    fn translate_and_assert_eur_to_usd(mut attr: PnlAttribution, val_t0_native: Money) {
        translate_to_target_ccy(
            &mut attr,
            val_t0_native,
            Currency::USD,
            &market(1.10),
            &market(1.20),
            date!(2025 - 01 - 15),
            date!(2025 - 01 - 16),
        )
        .expect("translate");

        // Per-factor amounts converted at T1 FX (1.20).
        assert_eq!(attr.total_pnl.currency(), Currency::USD);
        assert!((attr.rates_curves_pnl.amount() - 96.0).abs() < 1e-6);
        assert!((attr.carry.amount() - 24.0).abs() < 1e-6);

        // Translation P&L: 1000 EUR × ΔFX (0.10) = 100 USD.
        assert!((attr.fx_translation_pnl.amount() - 100.0).abs() < 1e-6);

        // Total: 220 USD.
        assert!((attr.total_pnl.amount() - 220.0).abs() < 1e-6);

        // Reconciliation: carry + rates + translation = total.
        let sum =
            attr.carry.amount() + attr.rates_curves_pnl.amount() + attr.fx_translation_pnl.amount();
        assert!(
            (sum - attr.total_pnl.amount()).abs() < 1e-6,
            "carry({}) + rates({}) + translation({}) ≠ total({})",
            attr.carry.amount(),
            attr.rates_curves_pnl.amount(),
            attr.fx_translation_pnl.amount(),
            attr.total_pnl.amount()
        );

        // FX policy stamp records the translation.
        let policy = attr.meta.fx_policy.as_ref().expect("fx policy stamped");
        assert_eq!(policy.target_ccy, Some(Currency::USD));
        assert!(policy.notes.contains("translated"));
    }

    #[test]
    fn translate_residual_is_zero_after_translation() {
        let mut attr = PnlAttribution::new(
            Money::new(50.0, Currency::EUR),
            "TEST",
            date!(2025 - 01 - 15),
            date!(2025 - 01 - 16),
            AttributionMethod::Parallel,
        );
        attr.rates_curves_pnl = Money::new(30.0, Currency::EUR);
        attr.carry = Money::new(20.0, Currency::EUR);
        attr.compute_residual().expect("native residual");
        assert!(attr.residual.amount().abs() < 1e-9);

        translate_to_target_ccy(
            &mut attr,
            Money::new(500.0, Currency::EUR),
            Currency::USD,
            &market(1.10),
            &market(1.10), // no FX move
            date!(2025 - 01 - 15),
            date!(2025 - 01 - 16),
        )
        .expect("translate");

        // With no FX move between T0 and T1, fx_translation_pnl should be 0.
        assert!(attr.fx_translation_pnl.amount().abs() < 1e-9);
        // Residual stays clean post-translation.
        assert!(attr.residual.amount().abs() < 1e-6);
    }
}
