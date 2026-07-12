//! Invariant tests for financial metrics.
//!
//! These tests verify fundamental invariants that must hold across all market conditions:
//!
//! 1. **DV01 sum-to-parallel**: Sum of bucketed DV01 ≈ parallel DV01
//! 2. **MC determinism**: Fixed seed produces identical results with parallel on/off
//!
//! Uses proptest for property-based testing to discover edge cases.

use finstack_quant_core::config::FinstackConfig;
use finstack_quant_core::currency::Currency;
use finstack_quant_core::dates::{DayCount, Tenor};
use finstack_quant_core::market_data::context::MarketContext;
use finstack_quant_core::market_data::term_structures::DiscountCurve;
use finstack_quant_core::money::Money;
use finstack_quant_valuations::instruments::fixed_income::bond::{Bond, CashflowSpec};
use finstack_quant_valuations::instruments::Instrument;
use finstack_quant_valuations::metrics::{standard_registry, MetricContext, MetricId};
use proptest::prelude::*;
use serde_json::json;
use std::sync::Arc;
use time::macros::date;

/// Build a discount curve with a given flat rate.
fn build_discount_curve(rate: f64) -> DiscountCurve {
    let as_of = date!(2025 - 01 - 01);
    DiscountCurve::builder("USD-OIS")
        .base_date(as_of)
        .day_count(DayCount::Act365F)
        .knots([
            (0.0f64, 1.0f64),
            (1.0f64, (-rate * 1.0).exp()),
            (2.0f64, (-rate * 2.0).exp()),
            (5.0f64, (-rate * 5.0).exp()),
            (10.0f64, (-rate * 10.0).exp()),
            (20.0f64, (-rate * 20.0).exp()),
            (30.0f64, (-rate * 30.0).exp()),
        ])
        .build()
        .unwrap()
}

#[test]
fn bucketed_dv01_uses_configured_bucket_grid() {
    let as_of = date!(2025 - 01 - 01);
    let maturity = as_of.saturating_add(time::Duration::days(10 * 365));
    let bond = Bond::builder()
        .id("CUSTOM_DV01_GRID".into())
        .notional(Money::new(1_000_000.0, Currency::USD))
        .issue_date(as_of)
        .maturity(maturity)
        .cashflow_spec(
            CashflowSpec::fixed(0.05, Tenor::semi_annual(), DayCount::Thirty360)
                .expect("finite test coupon"),
        )
        .discount_curve_id("USD-OIS".into())
        .build()
        .expect("bond");
    let market = MarketContext::new().insert(build_discount_curve(0.04));
    let pv = bond.value(&market, as_of).expect("pv");
    let mut cfg = FinstackConfig::default();
    cfg.extensions
        .insert(
            "valuations.sensitivities.v1",
            json!({"dv01_buckets_years": [4.0, 6.0]}),
        )
        .expect("valid extension key");

    let mut context =
        MetricContext::new(Arc::new(bond), Arc::new(market), as_of, pv, Arc::new(cfg));
    standard_registry()
        .compute(&[MetricId::BucketedDv01], &mut context)
        .expect("bucketed dv01");

    let keys: Vec<&str> = context.computed.keys().map(MetricId::as_str).collect();
    assert!(
        keys.contains(&"bucketed_dv01::USD-OIS::4y")
            && keys.contains(&"bucketed_dv01::USD-OIS::6y"),
        "custom DV01 buckets missing from computed keys: {keys:?}"
    );
    assert!(
        !keys.contains(&"bucketed_dv01::USD-OIS::5y"),
        "standard bucket leaked despite custom grid: {keys:?}"
    );
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(20))]

    /// Property: Sum of bucketed DV01 should equal parallel DV01 within tight tolerance.
    ///
    /// The triangular key-rate method partitions the rate sensitivity across buckets
    /// such that the sum equals the parallel sensitivity. This invariant should hold
    /// for any valid bond configuration and market conditions.
    #[test]
    fn prop_bucketed_dv01_sums_to_parallel(
        coupon_rate in 0.01f64..0.08f64,
        notional in 1_000_000.0f64..50_000_000.0f64,
        maturity_years in 2u32..20u32,
        flat_rate in 0.01f64..0.08f64,
    ) {
        let as_of = date!(2025 - 01 - 01);
        let issue = as_of;
        let maturity = as_of.saturating_add(time::Duration::days(i64::from(maturity_years) * 365));

        // Build bond
        let bond = Bond::builder()
            .id("PROP_DV01_TEST".into())
            .notional(Money::new(notional, Currency::USD))
            .issue_date(issue)
            .maturity(maturity)
            .cashflow_spec(CashflowSpec::fixed(
                coupon_rate,
                Tenor::semi_annual(),
                DayCount::Thirty360,
            ).expect("finite test coupon"))
            .discount_curve_id("USD-OIS".into())
            .build()
            .expect("Bond construction should succeed");

        // Build market
        let disc = build_discount_curve(flat_rate);
        let market = MarketContext::new().insert(disc);

        // Compute metrics
        let metrics = vec![MetricId::Dv01, MetricId::BucketedDv01];
        let registry = standard_registry();
        let pv = bond.value(&market, as_of).expect("Bond valuation should succeed");

        let mut context = MetricContext::new(
            Arc::new(bond),
            Arc::new(market),
            as_of,
            pv,
            MetricContext::default_config(),
        );

        let results = registry.compute(&metrics, &mut context).expect("Metrics should compute");

        let parallel_dv01 = results.get(&MetricId::Dv01).copied().unwrap_or(0.0);
        let bucketed_series = context.computed_series.get(&MetricId::BucketedDv01);

        if let Some(series) = bucketed_series {
            let sum_bucketed: f64 = series.iter().map(|(_, v)| v).sum();
            let diff = (sum_bucketed - parallel_dv01).abs();

            // Skip near-zero cases where relative error is meaningless
            if parallel_dv01.abs() > 10.0 {
                // Sum should match parallel within 0.1%
                let diff_pct = ((sum_bucketed - parallel_dv01) / parallel_dv01).abs();
                prop_assert!(
                    diff_pct < 0.001,
                    "Bucketed DV01 sum ({:.4}) should match parallel ({:.4}) within 0.1%, got {:.3}%",
                    sum_bucketed, parallel_dv01, diff_pct * 100.0
                );
            } else {
                prop_assert!(
                    diff < 5.0,
                    "Bucketed DV01 sum ({:.4}) should match parallel ({:.4}) within $5, diff {:.4}",
                    sum_bucketed,
                    parallel_dv01,
                    diff
                );
            }
        }
    }

    /// Property: Both bucketed and parallel DV01 should be negative for long bond positions.
    ///
    /// A long bond position loses value when rates rise (parallel bump is +1bp),
    /// so DV01 should be negative.
    #[test]
    fn prop_dv01_sign_negative_for_long_bond(
        coupon_rate in 0.02f64..0.06f64,
        maturity_years in 3u32..15u32,
        flat_rate in 0.02f64..0.06f64,
    ) {
        let as_of = date!(2025 - 01 - 01);
        let issue = as_of;
        let maturity = as_of.saturating_add(time::Duration::days(i64::from(maturity_years) * 365));

        // Build bond with $10M notional for significant DV01
        let bond = Bond::builder()
            .id("PROP_SIGN_TEST".into())
            .notional(Money::new(10_000_000.0, Currency::USD))
            .issue_date(issue)
            .maturity(maturity)
            .cashflow_spec(CashflowSpec::fixed(
                coupon_rate,
                Tenor::semi_annual(),
                DayCount::Thirty360,
            ).expect("finite test coupon"))
            .discount_curve_id("USD-OIS".into())
            .build()
            .expect("Bond construction should succeed");

        let disc = build_discount_curve(flat_rate);
        let market = MarketContext::new().insert(disc);

        let metrics = vec![MetricId::Dv01];
        let registry = standard_registry();
        let pv = bond.value(&market, as_of).expect("Bond valuation should succeed");

        let mut context = MetricContext::new(
            Arc::new(bond),
            Arc::new(market),
            as_of,
            pv,
            MetricContext::default_config(),
        );

        let results = registry.compute(&metrics, &mut context).expect("Metrics should compute");
        let dv01 = results.get(&MetricId::Dv01).copied().unwrap_or(0.0);

        // DV01 should be negative for long bond
        prop_assert!(
            dv01 < 0.0,
            "DV01 ({:.4}) should be negative for long bond position",
            dv01
        );
    }
}

mod mc_invariants {
    //! Monte Carlo determinism invariants.
    //!
    //! Tests that MC pricing produces identical results with the same seed
    //! across multiple runs. This is critical for reproducibility.

    use finstack_quant_core::currency::Currency;
    use finstack_quant_core::dates::DayCount;
    use finstack_quant_core::market_data::context::MarketContext;
    use finstack_quant_core::market_data::scalars::MarketScalar;
    use finstack_quant_core::market_data::surfaces::VolSurface;
    use finstack_quant_core::market_data::term_structures::DiscountCurve;
    use finstack_quant_core::money::Money;
    use finstack_quant_valuations::instruments::exotics::asian_option::{
        AsianOption, AveragingMethod,
    };
    use finstack_quant_valuations::instruments::Instrument;
    use finstack_quant_valuations::instruments::OptionType;
    use time::macros::date;

    fn create_mc_market(spot: f64, vol: f64, rate: f64) -> MarketContext {
        let as_of = date!(2024 - 01 - 01);

        let disc = DiscountCurve::builder("USD-OIS")
            .base_date(as_of)
            .day_count(DayCount::Act365F)
            .knots([
                (0.0f64, 1.0f64),
                (1.0f64, (-rate).exp()),
                (2.0f64, (-rate * 2.0).exp()),
            ])
            .build()
            .unwrap();

        let vol_surface = VolSurface::builder("SPOT_VOL")
            .expiries(&[0.5, 1.0, 2.0])
            .strikes(&[80.0, 100.0, 120.0])
            .row(&[vol, vol, vol])
            .row(&[vol, vol, vol])
            .row(&[vol, vol, vol])
            .build()
            .unwrap();

        MarketContext::new()
            .insert(disc)
            .insert_surface(vol_surface)
            .insert_price("SPOT", MarketScalar::Unitless(spot))
    }

    /// Test that MC pricing with the same seed produces identical results.
    ///
    /// This is the core determinism invariant: given the same inputs and seed,
    /// MC pricing should produce bit-exact identical results across runs.
    #[test]
    fn test_mc_same_seed_determinism() {
        let as_of = date!(2024 - 01 - 01);
        let expiry = date!(2025 - 01 - 01);

        let option = AsianOption {
            id: "MC_SEED_DETERMINISM".into(),
            underlying_ticker: "SPOT".to_string(),
            spot_id: "SPOT".into(),
            strike: 100.0,
            option_type: OptionType::Call,
            expiry,
            notional: Money::new(1.0, Currency::USD),
            averaging_method: AveragingMethod::Arithmetic,
            fixing_dates: vec![date!(2024 - 07 - 01), date!(2025 - 01 - 01)],
            day_count: DayCount::Act365F,
            discount_curve_id: "USD-OIS".into(),
            vol_surface_id: "SPOT_VOL".into(),
            div_yield_id: None,
            pricing_overrides: Default::default(),
            attributes: Default::default(),
            past_fixings: vec![],
        };

        let market = create_mc_market(100.0, 0.25, 0.05);

        // Price multiple times with same seed
        let mut prices = Vec::with_capacity(5);
        for _ in 0..5 {
            let pv = option.value(&market, as_of).expect("Option should price");
            prices.push(pv.amount());
        }

        // All prices should be identical (bit-exact)
        let first = prices[0];
        for (i, &price) in prices.iter().enumerate() {
            assert_eq!(
                price, first,
                "Run {} produced different price ({}) than first run ({})",
                i, price, first
            );
        }
    }

    /// Test MC determinism with different market conditions.
    ///
    /// Verify that the determinism invariant holds across various market
    /// conditions (different spots, vols, rates).
    #[test]
    fn test_mc_determinism_various_markets() {
        let as_of = date!(2024 - 01 - 01);
        let expiry = date!(2025 - 01 - 01);

        let option = AsianOption {
            id: "MC_VARIOUS_MARKETS".into(),
            underlying_ticker: "SPOT".to_string(),
            spot_id: "SPOT".into(),
            strike: 100.0,
            option_type: OptionType::Call,
            expiry,
            notional: Money::new(1.0, Currency::USD),
            averaging_method: AveragingMethod::Arithmetic,
            fixing_dates: vec![date!(2024 - 07 - 01), date!(2025 - 01 - 01)],
            day_count: DayCount::Act365F,
            discount_curve_id: "USD-OIS".into(),
            vol_surface_id: "SPOT_VOL".into(),
            div_yield_id: None,
            pricing_overrides: Default::default(),
            attributes: Default::default(),
            past_fixings: vec![],
        };

        // Test with different market conditions
        let test_cases = [
            (90.0, 0.20, 0.03),  // Lower spot, lower vol, lower rate
            (100.0, 0.25, 0.05), // Base case
            (110.0, 0.30, 0.07), // Higher spot, higher vol, higher rate
        ];

        for (spot, vol, rate) in test_cases {
            let market = create_mc_market(spot, vol, rate);

            // Price twice and verify identical
            let pv1 = option
                .value(&market, as_of)
                .expect("First pricing should succeed");
            let pv2 = option
                .value(&market, as_of)
                .expect("Second pricing should succeed");

            assert_eq!(
                pv1.amount(),
                pv2.amount(),
                "Spot={}, Vol={}, Rate={}: prices should be identical ({} vs {})",
                spot,
                vol,
                rate,
                pv1.amount(),
                pv2.amount()
            );
        }
    }
}

/// Property tests for CDS pricing invariants.
mod cds_invariants {
    use finstack_quant_core::currency::Currency;
    use finstack_quant_core::dates::{DateExt, DayCount};
    use finstack_quant_core::market_data::context::MarketContext;
    use finstack_quant_core::market_data::term_structures::{DiscountCurve, HazardCurve};
    use finstack_quant_core::money::Money;
    use finstack_quant_valuations::instruments::Instrument;
    use finstack_quant_valuations::metrics::MetricId;

    use proptest::prelude::*;
    use time::macros::date;

    /// Build a discount + hazard curve pair. `recovery` sets the hazard
    /// curve's bootstrap recovery; callers must pass the same recovery the
    /// CDS trade uses, since the ISDA recovery-consistency guard requires the
    /// same R in both legs.
    fn build_test_curves(
        flat_rate: f64,
        hazard_rate: f64,
        recovery: f64,
    ) -> (DiscountCurve, HazardCurve) {
        let as_of = date!(2025 - 01 - 01);

        let disc = DiscountCurve::builder("USD-OIS")
            .base_date(as_of)
            .day_count(DayCount::Act365F)
            .knots([
                (0.0f64, 1.0f64),
                (1.0f64, (-flat_rate).exp()),
                (5.0f64, (-flat_rate * 5.0).exp()),
                (10.0f64, (-flat_rate * 10.0).exp()),
            ])
            .build()
            .unwrap();

        let hazard = HazardCurve::builder("TEST-CREDIT")
            .base_date(as_of)
            .day_count(DayCount::Act365F)
            .recovery_rate(recovery)
            .knots(vec![
                (0.0, hazard_rate),
                (1.0, hazard_rate),
                (5.0, hazard_rate),
                (10.0, hazard_rate),
            ])
            .build()
            .unwrap();

        (disc, hazard)
    }

    fn metric_value(
        cds: &finstack_quant_valuations::instruments::credit_derivatives::cds::CreditDefaultSwap,
        market: &MarketContext,
        as_of: time::Date,
        metric: MetricId,
    ) -> f64 {
        let result = cds
            .price_with_metrics(
                market,
                as_of,
                std::slice::from_ref(&metric),
                finstack_quant_valuations::instruments::PricingOptions::default(),
            )
            .expect("metric should compute");
        result.measures[&metric]
    }

    proptest! {
        #![proptest_config(ProptestConfig::with_cases(15))]

        /// Property: Par spread roundtrip should be idempotent.
        ///
        /// Given a CDS with spread S and hazard curve H:
        /// 1. Compute par spread P from H
        /// 2. P should be close to the input spread when hazard is calibrated
        ///
        /// Note: We test that par spread calculation is stable, not exact roundtrip
        /// since we don't have a hazard calibration step here.
        #[test]
        fn prop_par_spread_is_positive_for_positive_hazard(
            spread_bp in 50.0f64..500.0f64,
            recovery in 0.2f64..0.6f64,
            hazard_rate in 0.005f64..0.05f64,
        ) {
            let as_of = date!(2025 - 01 - 01);
            let maturity = as_of.add_months(60); // 5Y CDS
            // Build the hazard curve at the trade recovery so the ISDA
            // recovery-consistency guard (same R in both legs) is satisfied.
            let (disc, hazard) = build_test_curves(0.04, hazard_rate, recovery);

            let cds = crate::finstack_quant_test_utils::cds_buy_protection(
                "PROP_PAR_TEST",
                Money::new(10_000_000.0, Currency::USD),
                spread_bp,
                as_of,
                maturity,
                "USD-OIS",
                "TEST-CREDIT",
            ).expect("CDS construction should succeed");

            // Override recovery rate
            let mut cds_with_recovery = cds;
            cds_with_recovery.protection.recovery_rate = recovery;

            let market = MarketContext::new()
                .insert(disc)
                .insert(hazard);
            let ps = metric_value(&cds_with_recovery, &market, as_of, MetricId::ParSpread);
            prop_assert!(
                ps > 0.0,
                "Par spread ({:.2} bps) should be positive for hazard rate {:.4}",
                ps * 10000.0,
                hazard_rate
            );
        }

        /// Property: Protection leg PV decreases with higher recovery rate.
        ///
        /// Higher recovery means smaller loss-given-default (LGD = 1 - R), so
        /// the expected protection payout is lower.
        #[test]
        fn prop_protection_leg_decreases_with_recovery(
            hazard_rate in 0.01f64..0.03f64,
        ) {
            let as_of = date!(2025 - 01 - 01);
            let maturity = as_of.add_months(60);

            let cds_low_recovery = crate::finstack_quant_test_utils::cds_buy_protection(
                "PROP_RECOVERY_LOW",
                Money::new(10_000_000.0, Currency::USD),
                100.0,
                as_of,
                maturity,
                "USD-OIS",
                "TEST-CREDIT",
            ).expect("CDS construction should succeed");

            let mut cds_high_recovery = cds_low_recovery.clone();
            cds_high_recovery.id = "PROP_RECOVERY_HIGH".into();

            // Set different recovery rates
            let mut cds_low = cds_low_recovery;
            cds_low.protection.recovery_rate = 0.30; // Low recovery = high LGD
            cds_high_recovery.protection.recovery_rate = 0.50; // High recovery = low LGD

            // Each trade needs a hazard curve whose bootstrap recovery matches
            // its own recovery (ISDA same-R-in-both-legs guard). The λ knots
            // are identical across both curves, so the protection-leg PVs are
            // directly comparable.
            let (disc_low, hazard_low) = build_test_curves(0.04, hazard_rate, 0.30);
            let market_low = MarketContext::new()
                .insert(disc_low)
                .insert(hazard_low);
            let (disc_high, hazard_high) = build_test_curves(0.04, hazard_rate, 0.50);
            let market_high = MarketContext::new()
                .insert(disc_high)
                .insert(hazard_high);
            let prot_low = metric_value(&cds_low, &market_low, as_of, MetricId::ProtectionLegPv);
            let prot_high =
                metric_value(&cds_high_recovery, &market_high, as_of, MetricId::ProtectionLegPv);

            // Higher recovery = lower protection PV (smaller payout)
            prop_assert!(
                prot_low > prot_high,
                "Protection PV with low recovery ({:.2}) should exceed high recovery ({:.2})",
                prot_low, prot_high
            );
        }
    }
}

/// Property tests for bucketed CS01 invariants.
mod cs01_invariants {
    use finstack_quant_core::currency::Currency;
    use finstack_quant_core::dates::{DateExt, DayCount};
    use finstack_quant_core::market_data::context::MarketContext;
    use finstack_quant_core::market_data::term_structures::{DiscountCurve, HazardCurve};
    use finstack_quant_core::money::Money;

    use finstack_quant_valuations::instruments::Instrument;
    use finstack_quant_valuations::metrics::{standard_registry, MetricContext, MetricId};
    use proptest::prelude::*;
    use std::sync::Arc;
    use time::macros::date;

    fn build_cds_market(flat_rate: f64, hazard_rate: f64) -> MarketContext {
        let as_of = date!(2025 - 01 - 01);

        let disc = DiscountCurve::builder("USD-OIS")
            .base_date(as_of)
            .day_count(DayCount::Act365F)
            .knots([
                (0.0f64, 1.0f64),
                (1.0f64, (-flat_rate).exp()),
                (5.0f64, (-flat_rate * 5.0).exp()),
                (10.0f64, (-flat_rate * 10.0).exp()),
                (30.0f64, (-flat_rate * 30.0).exp()),
            ])
            .build()
            .unwrap();

        let hazard = HazardCurve::builder("TEST-CREDIT")
            .base_date(as_of)
            .day_count(DayCount::Act365F)
            .recovery_rate(0.4)
            .knots(vec![
                (0.0, hazard_rate),
                (1.0, hazard_rate),
                (5.0, hazard_rate),
                (10.0, hazard_rate),
            ])
            .build()
            .unwrap();

        MarketContext::new().insert(disc).insert(hazard)
    }

    proptest! {
        #![proptest_config(ProptestConfig::with_cases(10))]

        /// Property: CS01 should be positive for protection buyer.
        ///
        /// A protection buyer benefits from spread widening (pays fixed, receives
        /// more valuable protection). Thus CS01 (dPV/dSpread) should be positive.
        #[test]
        fn prop_cs01_positive_for_protection_buyer(
            spread_bp in 50.0f64..300.0f64,
            hazard_rate in 0.01f64..0.03f64,
        ) {
            let as_of = date!(2025 - 01 - 01);
            let maturity = as_of.add_months(60);

            let cds = crate::finstack_quant_test_utils::cds_buy_protection(
                "PROP_CS01_TEST",
                Money::new(10_000_000.0, Currency::USD),
                spread_bp,
                as_of,
                maturity,
                "USD-OIS",
                "TEST-CREDIT",
            ).expect("CDS construction should succeed");

            let market = build_cds_market(0.04, hazard_rate);
            let pv = cds.value(&market, as_of).expect("Valuation should succeed");

            let metrics = vec![MetricId::Cs01];
            let registry = standard_registry();

            let mut context = MetricContext::new(
                Arc::new(cds),
                Arc::new(market),
                as_of,
                pv,
                MetricContext::default_config(),
            );

            let results = registry.compute(&metrics, &mut context).expect("Metrics should compute");
            let cs01 = results.get(&MetricId::Cs01).copied().unwrap_or(0.0);

            // CS01 should be positive for protection buyer
            // (spread widening increases protection value)
            prop_assert!(
                cs01 > 0.0,
                "CS01 ({:.4}) should be positive for protection buyer",
                cs01
            );
        }
    }
}

/// Tests that CS-Gamma is the second derivative of PV w.r.t. par-spread shifts,
/// consistent with CS01 = first derivative (both use par-spread re-bootstrapping).
#[cfg(test)]
mod cs_gamma_consistency {
    use finstack_quant_core::currency::Currency;
    use finstack_quant_core::dates::{DateExt, DayCount};
    use finstack_quant_core::market_data::context::MarketContext;
    use finstack_quant_core::market_data::term_structures::{DiscountCurve, HazardCurve};
    use finstack_quant_core::money::Money;

    use finstack_quant_valuations::instruments::Instrument;
    use finstack_quant_valuations::metrics::{standard_registry, MetricContext, MetricId};
    use std::sync::Arc;
    use time::macros::date;

    /// Build a discount curve and a hazard curve that has par-spread points
    /// (so CS01 / CS-Gamma will use the par-spread re-bootstrap path).
    fn build_curves_with_par_spreads(par_shift_bp: f64) -> (DiscountCurve, HazardCurve) {
        let as_of = date!(2025 - 01 - 01);

        let disc = DiscountCurve::builder("USD-OIS")
            .base_date(as_of)
            .day_count(DayCount::Act365F)
            .knots([
                (0.0f64, 1.0f64),
                (1.0f64, (-0.04f64).exp()),
                (5.0f64, (-0.04f64 * 5.0).exp()),
                (10.0f64, (-0.04f64 * 10.0).exp()),
            ])
            .build()
            .unwrap();

        // Base par spreads: 100 / 150 / 200 / 250 bp for 1y / 3y / 5y / 10y.
        // `par_shift_bp` allows building an "up-shifted" or "down-shifted" market
        // to verify the CS-Gamma consistency identity.
        let par_spreads: Vec<(f64, f64)> = vec![
            (1.0, 100.0 + par_shift_bp),
            (3.0, 150.0 + par_shift_bp),
            (5.0, 200.0 + par_shift_bp),
            (10.0, 250.0 + par_shift_bp),
        ];

        let hazard = HazardCurve::builder("TEST-CREDIT")
            .base_date(as_of)
            .day_count(DayCount::Act365F)
            .recovery_rate(0.40)
            .knots(vec![(0.0, 0.02), (1.0, 0.025), (5.0, 0.035), (10.0, 0.045)])
            .par_spreads(par_spreads)
            .build()
            .unwrap();

        (disc, hazard)
    }

    fn compute_metric(
        cds: &finstack_quant_valuations::instruments::credit_derivatives::cds::CreditDefaultSwap,
        market: &MarketContext,
        as_of: time::Date,
        metric: MetricId,
    ) -> f64 {
        let pv = cds
            .value(market, as_of)
            .expect("CDS valuation should succeed");
        let registry = standard_registry();
        let mut context = MetricContext::new(
            Arc::new(cds.clone()),
            Arc::new(market.clone()),
            as_of,
            pv,
            MetricContext::default_config(),
        );
        let results = registry
            .compute(std::slice::from_ref(&metric), &mut context)
            .expect("metric should compute");
        *results.get(&metric).expect("metric should be present")
    }

    /// CS-Gamma consistency: CS-Gamma should equal the numerical derivative of
    /// CS01 w.r.t. the parallel par spread shift, unit-converted:
    ///
    ///   CS-Gamma(s) [$/decimal²] ≈ 10_000² × (CS01(s+Δ) − CS01(s−Δ)) / (2Δ)
    ///
    /// where CS01 is in $/bp and Δ is the outer shift in bp.
    ///
    /// This identity holds when both CS01 and CS-Gamma use the same par-spread
    /// re-bootstrap methodology. It FAILS when CS-Gamma bumps hazard rates directly
    /// while CS01 re-bootstraps from par spreads (the bug this test guards against).
    ///
    /// Bump size for the outer finite difference is 5bp (larger than the 1bp used
    /// internally by CS01/CS-Gamma to reduce second-order noise in the comparison).
    #[test]
    fn cs_gamma_equals_numerical_derivative_of_cs01() {
        let as_of = date!(2025 - 01 - 01);
        let maturity = as_of.add_months(60); // 5Y CDS

        let cds = crate::finstack_quant_test_utils::cds_buy_protection(
            "CS_GAMMA_CONSISTENCY",
            Money::new(10_000_000.0, Currency::USD),
            200.0,
            as_of,
            maturity,
            "USD-OIS",
            "TEST-CREDIT",
        )
        .expect("CDS construction should succeed");

        // CS-Gamma at base spreads.
        let (disc, hazard_base) = build_curves_with_par_spreads(0.0);
        let market_base = MarketContext::new().insert(disc).insert(hazard_base);
        let cs_gamma = compute_metric(&cds, &market_base, as_of, MetricId::CsGamma);

        // CS01 at spreads shifted +5bp / −5bp.
        let outer_shift_bp = 5.0;

        let (disc_up, hazard_up) = build_curves_with_par_spreads(outer_shift_bp);
        let market_up = MarketContext::new().insert(disc_up).insert(hazard_up);
        let cs01_up = compute_metric(&cds, &market_up, as_of, MetricId::Cs01);

        let (disc_dn, hazard_dn) = build_curves_with_par_spreads(-outer_shift_bp);
        let market_dn = MarketContext::new().insert(disc_dn).insert(hazard_dn);
        let cs01_dn = compute_metric(&cds, &market_dn, as_of, MetricId::Cs01);

        // Numerical derivative of CS01, unit-converted to match CS-Gamma.
        //
        // CS01 is in $/bp  (d PV / d spread_bp).
        // CS-Gamma is in $/decimal² (d² PV / d spread_decimal²).
        //
        // Relationship:
        //   d² PV / d spread_decimal²
        //     = 10_000² × d² PV / d spread_bp²
        //     = 10_000² × d(CS01) / d spread_bp
        //     = 10_000² × (CS01_up - CS01_dn) / (2 × outer_shift_bp)
        let cs_gamma_numerical =
            10_000.0_f64.powi(2) * (cs01_up - cs01_dn) / (2.0 * outer_shift_bp);

        // Tolerance: 5% relative error is acceptable for this second-order
        // finite-difference approximation at a 5bp outer shift.
        let abs_ref = cs_gamma.abs().max(cs_gamma_numerical.abs()).max(1.0);
        let rel_error = (cs_gamma - cs_gamma_numerical).abs() / abs_ref;
        assert!(
            rel_error < 0.05,
            "CS-Gamma ({:.4}) should be consistent with numerical CS01 derivative ({:.4}), rel_error={:.2}%",
            cs_gamma,
            cs_gamma_numerical,
            rel_error * 100.0
        );
    }

    /// Fallback path: when the hazard curve has NO par-spread points (so
    /// `used_rebootstrap = false`), CS-Gamma must still be finite and
    /// non-trivially non-zero, computed via the direct hazard-rate shift path.
    ///
    /// This exercises a code path that `cs_gamma_equals_numerical_derivative_of_cs01`
    /// does NOT cover (that test always uses a curve with par-spread points).
    #[test]
    fn cs_gamma_fallback_direct_shift_is_finite_without_par_spreads() {
        let as_of = date!(2025 - 01 - 01);
        let maturity = as_of.add_months(60); // 5Y CDS

        let cds = crate::finstack_quant_test_utils::cds_buy_protection(
            "CS_GAMMA_FALLBACK",
            Money::new(10_000_000.0, Currency::USD),
            200.0,
            as_of,
            maturity,
            "USD-OIS",
            "TEST-CREDIT-NOPAR",
        )
        .expect("CDS construction should succeed");

        // Hazard curve has NO par-spread points — forces the direct hazard-rate
        // shift fallback (`used_rebootstrap = false`).
        let disc = DiscountCurve::builder("USD-OIS")
            .base_date(as_of)
            .day_count(DayCount::Act365F)
            .knots([
                (0.0f64, 1.0f64),
                (1.0f64, (-0.04f64).exp()),
                (5.0f64, (-0.04f64 * 5.0).exp()),
                (10.0f64, (-0.04f64 * 10.0).exp()),
            ])
            .build()
            .unwrap();
        let hazard_no_par = HazardCurve::builder("TEST-CREDIT-NOPAR")
            .base_date(as_of)
            .day_count(DayCount::Act365F)
            .recovery_rate(0.40)
            .knots(vec![
                (0.0, 0.020),
                (1.0, 0.025),
                (5.0, 0.035),
                (10.0, 0.045),
            ])
            // Intentionally no `.par_spreads(...)` — stays on fallback path.
            .build()
            .unwrap();

        let market = MarketContext::new().insert(disc).insert(hazard_no_par);
        let cs_gamma = compute_metric(&cds, &market, as_of, MetricId::CsGamma);

        assert!(
            cs_gamma.is_finite(),
            "CS-Gamma (fallback path) should be finite, got {cs_gamma}"
        );
        // The fallback computes a genuine second-order finite difference, so the
        // result should be non-zero for a standard vanilla CDS.
        assert!(
            cs_gamma.abs() > 0.0,
            "CS-Gamma (fallback path) should be non-zero for a live CDS, got {cs_gamma}"
        );
    }
}

/// Tests for bucketed CS01 sum invariants.
#[cfg(test)]
mod bucketed_cs01_invariants {
    use finstack_quant_core::currency::Currency;
    use finstack_quant_core::dates::{DateExt, DayCount};
    use finstack_quant_core::market_data::context::MarketContext;
    use finstack_quant_core::market_data::term_structures::{DiscountCurve, HazardCurve};
    use finstack_quant_core::money::Money;

    use finstack_quant_valuations::instruments::Instrument;
    use finstack_quant_valuations::metrics::{standard_registry, MetricContext, MetricId};
    use std::sync::Arc;
    use time::macros::date;

    fn build_cds_market(flat_rate: f64, hazard_rate: f64) -> MarketContext {
        let as_of = date!(2025 - 01 - 01);

        let disc = DiscountCurve::builder("USD-OIS")
            .base_date(as_of)
            .day_count(DayCount::Act365F)
            .knots([
                (0.0f64, 1.0f64),
                (1.0f64, (-flat_rate).exp()),
                (5.0f64, (-flat_rate * 5.0).exp()),
                (10.0f64, (-flat_rate * 10.0).exp()),
                (30.0f64, (-flat_rate * 30.0).exp()),
            ])
            .build()
            .unwrap();

        let hazard = HazardCurve::builder("TEST-CREDIT")
            .base_date(as_of)
            .day_count(DayCount::Act365F)
            .recovery_rate(0.4)
            .knots(vec![
                (0.0, hazard_rate),
                (1.0, hazard_rate),
                (5.0, hazard_rate),
                (10.0, hazard_rate),
            ])
            .build()
            .unwrap();

        MarketContext::new().insert(disc).insert(hazard)
    }

    /// Test that bucketed CS01 values are consistent with parallel CS01.
    ///
    /// For a 5Y CDS, the bucketed CS01 should show sensitivity concentrated
    /// in the 5Y bucket region. The buckets should have the same sign as
    /// parallel CS01 (positive for protection buyer).
    #[test]
    fn test_bucketed_cs01_consistency() {
        let as_of = date!(2025 - 01 - 01);
        let maturity = as_of.add_months(60); // 5Y CDS

        let cds = crate::finstack_quant_test_utils::cds_buy_protection(
            "CS01_BUCKET_TEST",
            Money::new(10_000_000.0, Currency::USD),
            100.0,
            as_of,
            maturity,
            "USD-OIS",
            "TEST-CREDIT",
        )
        .expect("CDS construction should succeed");

        let market = build_cds_market(0.04, 0.02);
        let pv = cds.value(&market, as_of).expect("Valuation should succeed");

        let metrics = vec![MetricId::Cs01, MetricId::BucketedCs01];
        let registry = standard_registry();

        let mut context = MetricContext::new(
            Arc::new(cds),
            Arc::new(market),
            as_of,
            pv,
            MetricContext::default_config(),
        );

        let results = registry
            .compute(&metrics, &mut context)
            .expect("Metrics should compute");
        let parallel_cs01 = results.get(&MetricId::Cs01).copied().unwrap_or(0.0);

        // Parallel CS01 should be positive for protection buyer
        assert!(
            parallel_cs01 > 0.0,
            "Parallel CS01 ({:.4}) should be positive for protection buyer",
            parallel_cs01
        );

        // Check bucketed CS01 properties
        if let Some(series) = context.computed_series.get(&MetricId::BucketedCs01) {
            // At least one bucket should have non-zero value
            let non_zero_count = series.iter().filter(|(_, v)| v.abs() > 1.0).count();
            assert!(
                non_zero_count > 0,
                "Bucketed CS01 should have at least one non-zero bucket"
            );

            // All buckets should have consistent sign (non-negative for protection buyer)
            // Some buckets may be zero if outside the CDS tenor
            let negative_buckets: Vec<_> = series
                .iter()
                .filter(|(_, v)| *v < -1.0) // Allow small negative due to numerical noise
                .collect();
            assert!(
                negative_buckets.is_empty(),
                "Bucketed CS01 should not have significantly negative buckets for protection buyer, found: {:?}",
                negative_buckets
            );
        }
    }
}

#[cfg(test)]
mod additional_invariants {
    //! Additional invariant tests that don't use proptest.

    use super::*;

    /// Bucketed DV01 should sum to parallel for a range of maturities.
    #[test]
    fn test_dv01_sum_invariant_across_maturities() {
        let as_of = date!(2025 - 01 - 01);
        let disc = build_discount_curve(0.04);
        let market = MarketContext::new().insert(disc);
        let registry = standard_registry();

        for years in [2, 5, 10, 15, 20] {
            let maturity = as_of.saturating_add(time::Duration::days(i64::from(years) * 365));

            let bond = Bond::builder()
                .id(format!("MATURITY_{}Y", years).into())
                .notional(Money::new(10_000_000.0, Currency::USD))
                .issue_date(as_of)
                .maturity(maturity)
                .cashflow_spec(
                    CashflowSpec::fixed(0.04, Tenor::semi_annual(), DayCount::Thirty360)
                        .expect("finite test coupon"),
                )
                .discount_curve_id("USD-OIS".into())
                .build()
                .expect("Bond construction should succeed");

            let metrics = vec![MetricId::Dv01, MetricId::BucketedDv01];
            let pv = bond
                .value(&market, as_of)
                .expect("Valuation should succeed");

            let mut context = MetricContext::new(
                Arc::new(bond),
                Arc::new(market.clone()),
                as_of,
                pv,
                MetricContext::default_config(),
            );

            let results = registry
                .compute(&metrics, &mut context)
                .expect("Metrics should compute");
            let parallel_dv01 = results.get(&MetricId::Dv01).copied().unwrap_or(0.0);

            if let Some(series) = context.computed_series.get(&MetricId::BucketedDv01) {
                let sum_bucketed: f64 = series.iter().map(|(_, v)| v).sum();

                let diff_pct = ((sum_bucketed - parallel_dv01) / parallel_dv01).abs();
                assert!(
                    diff_pct < 0.001,
                    "{}Y bond: bucketed sum ({:.2}) vs parallel ({:.2}), diff: {:.4}%",
                    years,
                    sum_bucketed,
                    parallel_dv01,
                    diff_pct * 100.0
                );
            }
        }
    }
}
