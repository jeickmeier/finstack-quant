//! Post-hoc translation of a native-currency `PnlAttribution` into a
//! reporting currency (`target_currency`).
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

use crate::types::PnlAttribution;

/// Translate a populated `PnlAttribution` from its native pricing currency
/// into `target_currency`.
///
/// # Arguments
///
/// * `attribution` - Populated attribution in its native pricing currency;
///   aggregate P&L fields are translated in place.
/// * `val_t0` - Opening native-currency value used to compute the
///   translation adjustment).
/// * `target_currency` - Reporting currency into which aggregate P&L is translated.
/// * `market_t0` - Opening market state carrying the FX matrix for the T₀
///   conversion.
/// * `market_t1` - Closing market state carrying the FX matrix for T₁ factor
///   and total-P&L conversion.
/// * `as_of_t0` - Opening valuation date passed to the T₀ FX conversion.
/// * `as_of_t1` - Closing valuation date passed to the T₁ FX conversion.
///
/// # Behavior
///
/// - If `target_currency == attribution.total_pnl.currency()`, this is a no-op
///   (no field is mutated).
/// - Otherwise every factor P&L is converted via `market_t1.convert_money`
///   at `as_of_t1`; `fx_translation_pnl` is set to
///   `val_t0 × (T1_fx − T0_fx)`; `total_pnl` is replaced by
///   `val_t1_target − val_t0_target`. The credit-model structs
///   (`credit_factor_detail`, `credit_carry_decomposition`) and
///   `carry_detail` ARE translated — every Money leaf, including bucket and
///   per-issuer maps, moves to target currency at T1 FX so their documented
///   reconciliation invariants keep closing after translation.
///   Remaining detail maps (`rates_detail`, `credit_detail`,
///   `inflation_detail`, `correlations_detail`, `fx_detail`, `vol_detail`,
///   `cross_factor_detail`, `model_params_detail`, `scalars_detail`) are
///   translated the same way so callers cannot mix native and reporting
///   currency when summing per-curve or per-pair leaves.
/// - The `meta.fx_policy` is stamped with `target_currency` and a note describing
///   the translation.
///
/// # Errors
///
/// Returns an error if any FX conversion fails (typically because the FX
/// matrix lacks the native→target rate).
pub fn translate_to_target_currency(
    attribution: &mut PnlAttribution,
    val_t0: Money,
    target_currency: Currency,
    market_t0: &MarketContext,
    market_t1: &MarketContext,
    as_of_t0: Date,
    as_of_t1: Date,
) -> Result<()> {
    let native_currency = attribution.total_pnl.currency();
    if native_currency == target_currency {
        return Ok(());
    }
    // Mutate a clone so a failed conversion leaves the original unchanged.
    let mut translated = attribution.clone();

    // Convert val_t0 with BOTH the T0 and T1 FX matrices so we can extract the
    // FX move applied to the opening position.
    let val_t0_at_t0 = market_t0.convert_money(val_t0, target_currency, as_of_t0)?;
    let val_t0_at_t1 = market_t1.convert_money(val_t0, target_currency, as_of_t1)?;
    let fx_translation = val_t0_at_t1.checked_sub(val_t0_at_t0)?;

    // Translate every per-factor amount to target_currency at T1 FX.
    let translate =
        |m: Money| -> Result<Money> { market_t1.convert_money(m, target_currency, as_of_t1) };

    translated.fx_translation_pnl = fx_translation;

    // Total in target_currency = MTM translation + the total-return add-back.
    //
    // Native `total_pnl` follows the total-return convention: the methods add
    // intra-period coupon income on top of the raw MTM (`mark_to_market_pnl`)
    // via `apply_total_return_carry`. The MTM component is rebuilt from the
    // T0/T1 values; the coupon add-back (total − MTM, zero when no cashflows
    // occurred) must travel at T1 FX — the same rate at which the translated
    // `carry` still contains it — or the recomputed residual is polluted by
    // the full coupon and `total_pnl` silently flips to MTM-only (quant
    // review M6).
    let native_total_pnl = translated.total_pnl;
    let native_mtm = translated.mark_to_market_pnl.unwrap_or(native_total_pnl);
    let coupon_addback_native = native_total_pnl.checked_sub(native_mtm)?;

    let val_t1_native = val_t0.checked_add(native_mtm)?;
    let val_t1_at_t1 = market_t1.convert_money(val_t1_native, target_currency, as_of_t1)?;
    let translated_mtm = val_t1_at_t1.checked_sub(val_t0_at_t0)?;
    translated.total_pnl = translated_mtm.checked_add(translate(coupon_addback_native)?)?;

    if translated.mark_to_market_pnl.is_some() {
        translated.mark_to_market_pnl = Some(translated_mtm);
    }

    translated.meta.fx_policy = Some(FxPolicyMeta {
        strategy: FxConversionPolicy::CashflowDate,
        target_currency: Some(target_currency),
        notes: format!(
            "translated from {native_currency} to {target_currency} (factors at T1 FX; \
             fx_translation_pnl = val_t0 × (T1_fx − T0_fx))"
        ),
    });

    // Every remaining aggregate and every Money leaf of every detail struct
    // (including the credit-model structs, which are populated BEFORE this
    // translation runs) moves to the target currency at the same T1 FX as the
    // factor fields — otherwise `generic + Σ levels + adder + curve_shape ≡
    // credit_curves_pnl` and the carry partition break by the FX rate.
    translated.for_each_money_mut(|m| {
        *m = translate(*m)?;
        Ok(())
    })?;
    if let Some(m) = translated
        .credit_factor_detail
        .as_mut()
        .and_then(|d| d.adder_magnitude.as_mut())
    {
        *m = translate(*m)?;
    }

    translated.compute_residual()?;
    *attribution = translated;
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
        let total = Money::from((100_i64, Currency::USD));
        let mut attr = PnlAttribution::new(
            total,
            "TEST",
            date!(2025 - 01 - 15),
            date!(2025 - 01 - 16),
            AttributionMethod::Parallel,
        );
        attr.carry = Money::from((40_i64, Currency::USD));
        attr.rates_curves_pnl = Money::from((60_i64, Currency::USD));
        let snapshot = attr.clone();

        translate_to_target_currency(
            &mut attr,
            Money::from((1000_i64, Currency::USD)),
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
        let val_t0_native = Money::from((1000_i64, Currency::EUR));
        let native_pnl = Money::from((100_i64, Currency::EUR));
        let mut attr = PnlAttribution::new(
            native_pnl,
            "EUR-BOND",
            date!(2025 - 01 - 15),
            date!(2025 - 01 - 16),
            AttributionMethod::Parallel,
        );
        attr.rates_curves_pnl = Money::from((80_i64, Currency::EUR));
        attr.carry = Money::from((20_i64, Currency::EUR));
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
        let val_t0_native = Money::from((1000_i64, Currency::EUR));
        // Raw MTM = +100 EUR; apply_total_return_carry added a 30 EUR coupon:
        // total = 130, carry = theta 20 + coupon 30 = 50, rates = 80.
        let mut attr = PnlAttribution::new(
            Money::from((100_i64, Currency::EUR)),
            "EUR-BOND-COUPON",
            date!(2025 - 01 - 15),
            date!(2025 - 01 - 16),
            AttributionMethod::Parallel,
        );
        attr.total_pnl = Money::from((130_i64, Currency::EUR));
        attr.carry = Money::from((50_i64, Currency::EUR));
        attr.rates_curves_pnl = Money::from((80_i64, Currency::EUR));
        attr.compute_residual().expect("residual");
        assert!(
            attr.residual.amount().abs() < 1e-12,
            "native attribution must start reconciled"
        );

        translate_to_target_currency(
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
        translate_to_target_currency(
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
        assert_eq!(policy.target_currency, Some(Currency::USD));
        assert!(policy.notes.contains("translated"));
    }

    /// Audit M5: `credit_factor_detail` and `credit_carry_decomposition` are
    /// populated before the target-currency translation runs, so every Money
    /// leaf of both structs must be translated at the same T1 FX as the
    /// top-level factor fields — otherwise the documented invariants
    /// (`generic + Σ levels + adder + curve_shape ≡ credit_curves_pnl`, and
    /// the carry partition) break by the FX rate and the two sides of one
    /// identity carry different currency tags.
    #[test]
    fn translate_converts_credit_detail_structs() {
        use crate::types::{
            CreditCarryByLevel, CreditCarryDecomposition, CreditFactorAttribution, LevelCarry,
            LevelPnl,
        };
        use finstack_quant_core::types::IssuerId;
        use std::collections::BTreeMap;

        let eur = |v: f64| Money::new(v, Currency::EUR).expect("valid money fixture");
        let mut attr = PnlAttribution::new(
            eur(20.0),
            "EUR-BOND",
            date!(2025 - 01 - 15),
            date!(2025 - 01 - 16),
            AttributionMethod::Parallel,
        );
        attr.credit_curves_pnl = eur(20.0);
        attr.compute_residual().expect("residual");

        // Detail closes in EUR: 10 + 5 + 3 + 2 = 20 = credit_curves_pnl.
        attr.credit_factor_detail = Some(CreditFactorAttribution {
            model_id: "model".into(),
            generic_pnl: eur(10.0),
            levels: vec![LevelPnl {
                level_name: "rating".into(),
                total: eur(5.0),
                by_bucket: BTreeMap::from([("IG".to_string(), eur(5.0))]),
            }],
            adder_pnl_total: eur(3.0),
            curve_shape_pnl: eur(2.0),
            adder_pnl_by_issuer: Some(BTreeMap::from([(IssuerId::new("ISS"), eur(3.0))])),
            adder_magnitude: Some(eur(3.0)),
        });
        attr.credit_carry_decomposition = Some(CreditCarryDecomposition {
            model_id: "model".into(),
            rates_carry_total: eur(7.0),
            credit_carry_total: eur(6.0),
            credit_by_level: CreditCarryByLevel {
                generic: eur(4.0),
                levels: vec![LevelCarry {
                    level_name: "rating".into(),
                    total: eur(1.5),
                    by_bucket: BTreeMap::from([("IG".to_string(), eur(1.5))]),
                }],
                adder_total: eur(0.5),
                adder_by_issuer: Some(BTreeMap::from([(IssuerId::new("ISS"), eur(0.5))])),
            },
        });

        translate_to_target_currency(
            &mut attr,
            Money::from((1000_i64, Currency::EUR)),
            Currency::USD,
            &market(1.10),
            &market(1.20),
            date!(2025 - 01 - 15),
            date!(2025 - 01 - 16),
        )
        .expect("translate");

        let assert_usd = |m: Money, native: f64, label: &str| {
            assert_eq!(m.currency(), Currency::USD, "{label} must be USD");
            assert!(
                (m.amount() - native * 1.20).abs() < 1e-9,
                "{label}: expected {} USD (native {native} × 1.20), got {}",
                native * 1.20,
                m.amount()
            );
        };

        let d = attr.credit_factor_detail.as_ref().expect("detail");
        assert_usd(d.generic_pnl, 10.0, "generic_pnl");
        assert_usd(d.levels[0].total, 5.0, "levels[0].total");
        assert_usd(d.levels[0].by_bucket["IG"], 5.0, "levels[0].by_bucket");
        assert_usd(d.adder_pnl_total, 3.0, "adder_pnl_total");
        assert_usd(d.curve_shape_pnl, 2.0, "curve_shape_pnl");
        assert_usd(
            d.adder_pnl_by_issuer.as_ref().expect("by issuer")[&IssuerId::new("ISS")],
            3.0,
            "adder_pnl_by_issuer",
        );
        assert_usd(
            d.adder_magnitude.expect("adder_magnitude"),
            3.0,
            "adder_magnitude",
        );

        // The detail invariant must still close in USD against the translated
        // credit_curves_pnl (types/detail.rs invariant).
        let detail_sum = d.generic_pnl.amount()
            + d.levels.iter().map(|l| l.total.amount()).sum::<f64>()
            + d.adder_pnl_total.amount()
            + d.curve_shape_pnl.amount();
        assert!(
            (detail_sum - attr.credit_curves_pnl.amount()).abs() < 1e-9,
            "generic + levels + adder + curve_shape ({detail_sum}) must equal \
             translated credit_curves_pnl ({})",
            attr.credit_curves_pnl.amount()
        );

        let c = attr
            .credit_carry_decomposition
            .as_ref()
            .expect("carry decomposition");
        assert_usd(c.rates_carry_total, 7.0, "rates_carry_total");
        assert_usd(c.credit_carry_total, 6.0, "credit_carry_total");
        assert_usd(c.credit_by_level.generic, 4.0, "credit_by_level.generic");
        assert_usd(c.credit_by_level.levels[0].total, 1.5, "carry level total");
        assert_usd(
            c.credit_by_level.levels[0].by_bucket["IG"],
            1.5,
            "carry level by_bucket",
        );
        assert_usd(c.credit_by_level.adder_total, 0.5, "adder_total");
        assert_usd(
            c.credit_by_level
                .adder_by_issuer
                .as_ref()
                .expect("by issuer")[&IssuerId::new("ISS")],
            0.5,
            "carry adder_by_issuer",
        );
    }

    #[test]
    fn translate_residual_is_zero_after_translation() {
        let mut attr = PnlAttribution::new(
            Money::from((50_i64, Currency::EUR)),
            "TEST",
            date!(2025 - 01 - 15),
            date!(2025 - 01 - 16),
            AttributionMethod::Parallel,
        );
        attr.rates_curves_pnl = Money::from((30_i64, Currency::EUR));
        attr.carry = Money::from((20_i64, Currency::EUR));
        attr.compute_residual().expect("native residual");
        assert!(attr.residual.amount().abs() < 1e-9);

        translate_to_target_currency(
            &mut attr,
            Money::from((500_i64, Currency::EUR)),
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

    #[test]
    fn translate_converts_remaining_detail_maps() {
        use crate::types::{FxAttribution, RatesCurvesAttribution};
        use finstack_quant_core::types::CurveId;
        use indexmap::IndexMap;

        let eur = |v: f64| Money::new(v, Currency::EUR).expect("valid money fixture");
        let mut attr = PnlAttribution::new(
            eur(80.0),
            "EUR-BOND",
            date!(2025 - 01 - 15),
            date!(2025 - 01 - 16),
            AttributionMethod::Parallel,
        );
        attr.rates_curves_pnl = eur(80.0);
        attr.compute_residual().expect("residual");

        let mut by_curve = IndexMap::new();
        by_curve.insert(CurveId::new("EUR-OIS"), eur(50.0));
        let mut by_tenor = IndexMap::new();
        by_tenor.insert((CurveId::new("EUR-OIS"), "5Y".to_string()), eur(50.0));
        attr.rates_detail = Some(RatesCurvesAttribution {
            by_curve,
            by_tenor,
            discount_total: eur(50.0),
            forward_total: eur(30.0),
        });

        let mut by_pair = IndexMap::new();
        by_pair.insert((Currency::EUR, Currency::USD), eur(10.0));
        attr.fx_detail = Some(FxAttribution { by_pair });

        translate_to_target_currency(
            &mut attr,
            Money::from((1000_i64, Currency::EUR)),
            Currency::USD,
            &market(1.10),
            &market(1.20),
            date!(2025 - 01 - 15),
            date!(2025 - 01 - 16),
        )
        .expect("translate");

        let rates = attr.rates_detail.as_ref().expect("rates detail");
        assert_eq!(rates.discount_total.currency(), Currency::USD);
        assert!((rates.discount_total.amount() - 60.0).abs() < 1e-9);
        assert_eq!(
            rates.by_curve[&CurveId::new("EUR-OIS")].currency(),
            Currency::USD
        );
        assert!((rates.by_curve[&CurveId::new("EUR-OIS")].amount() - 60.0).abs() < 1e-9);
        assert_eq!(
            rates.by_tenor[&(CurveId::new("EUR-OIS"), "5Y".to_string())].currency(),
            Currency::USD
        );

        let fx = attr.fx_detail.as_ref().expect("fx detail");
        assert_eq!(
            fx.by_pair[&(Currency::EUR, Currency::USD)].currency(),
            Currency::USD
        );
        assert!((fx.by_pair[&(Currency::EUR, Currency::USD)].amount() - 12.0).abs() < 1e-9);
    }
}
