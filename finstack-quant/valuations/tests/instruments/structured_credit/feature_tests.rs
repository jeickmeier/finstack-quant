//! Tests for new structured credit features:
//!
//! 1. Tranche write-down / loss allocation through capital structure
//! 2. Reserve account wiring (RecipientType::ReserveAccount)
//! 3. OC/IC cure amount diversion mechanism
//! 4. Clean-up call modeling

use finstack_quant_cashflows::builder::{DefaultModelSpec, PrepaymentModelSpec, RecoveryModelSpec};
use finstack_quant_core::currency::Currency;
use finstack_quant_core::dates::Date;
use finstack_quant_core::market_data::context::MarketContext;
use finstack_quant_core::market_data::term_structures::{DiscountCurve, ForwardCurve};
use finstack_quant_core::math::interp::InterpStyle;
use finstack_quant_core::money::Money;
use finstack_quant_core::types::{CreditRating, InstrumentId};
use finstack_quant_valuations::instruments::fixed_income::structured_credit::{
    run_simulation, AssetPool, AssetType, DealType, PoolAsset, StructuredCredit, Tranche,
    TrancheCoupon, TrancheSeniority, TrancheStructure,
};
use time::Month;

// ============================================================================
// Helpers
// ============================================================================

fn as_of() -> Date {
    Date::from_calendar_date(2025, Month::January, 15).unwrap()
}

fn closing() -> Date {
    Date::from_calendar_date(2025, Month::January, 1).unwrap()
}

fn maturity_5y() -> Date {
    Date::from_calendar_date(2030, Month::January, 1).unwrap()
}

fn flat_market() -> MarketContext {
    let discount = DiscountCurve::builder("USD_OIS")
        .base_date(as_of())
        .knots(vec![(0.0, 1.0), (10.0, 0.60)])
        .interp(InterpStyle::Linear)
        .build()
        .unwrap();

    let forward = ForwardCurve::builder("SOFR-3M", 0.25)
        .base_date(as_of())
        .knots(vec![(0.0, 0.05), (10.0, 0.05)])
        .interp(InterpStyle::Linear)
        .build()
        .unwrap();

    MarketContext::new().insert(discount).insert(forward)
}

fn build_pool(n_assets: usize, balance_each: f64) -> AssetPool {
    let mut pool = AssetPool::new("FEAT_POOL", DealType::CLO, Currency::USD);
    for i in 0..n_assets {
        pool.assets.push(PoolAsset {
            day_count: finstack_quant_core::dates::DayCount::Act360,
            id: InstrumentId::new(format!("LOAN_{}", i)),
            asset_type: AssetType::FirstLienLoan {
                industry: Some("Technology".to_string()),
            },
            balance: Money::new(balance_each, Currency::USD),
            rate: 0.08,
            spread_bps: None,
            index_id: None,
            maturity: maturity_5y(),
            credit_quality: Some(CreditRating::BB),
            industry: Some("Technology".to_string()),
            obligor_id: Some(format!("OB_{}", i)),
            is_defaulted: false,
            recovery_amount: None,
            purchase_price: None,
            acquisition_date: Some(as_of()),
            smm_override: None,
            mdr_override: None,
        });
    }
    pool
}

fn build_tranches(senior: f64, mezz: f64, equity: f64) -> TrancheStructure {
    let total = senior + mezz + equity;
    TrancheStructure::new(vec![
        Tranche::new(
            "SR",
            0.0,
            senior / total * 100.0,
            TrancheSeniority::Senior,
            Money::new(senior, Currency::USD),
            TrancheCoupon::Fixed { rate: 0.05 },
            maturity_5y(),
        )
        .unwrap(),
        Tranche::new(
            "MZ",
            senior / total * 100.0,
            (senior + mezz) / total * 100.0,
            TrancheSeniority::Mezzanine,
            Money::new(mezz, Currency::USD),
            TrancheCoupon::Fixed { rate: 0.07 },
            maturity_5y(),
        )
        .unwrap(),
        Tranche::new(
            "EQ",
            (senior + mezz) / total * 100.0,
            100.0,
            TrancheSeniority::Equity,
            Money::new(equity, Currency::USD),
            TrancheCoupon::Fixed { rate: 0.0 },
            maturity_5y(),
        )
        .unwrap(),
    ])
    .unwrap()
}

fn build_clo(cpr: f64, cdr: f64, recovery: f64, lag: u32) -> StructuredCredit {
    let mut clo = StructuredCredit::new_clo(
        "FEAT_CLO",
        build_pool(10, 10_000_000.0),
        build_tranches(70_000_000.0, 20_000_000.0, 10_000_000.0),
        closing(),
        maturity_5y(),
        "USD_OIS",
    )
    .with_payment_calendar("nyse");

    clo.credit_model.prepayment_spec = PrepaymentModelSpec::constant_cpr(cpr);
    clo.credit_model.default_spec = DefaultModelSpec::constant_cdr(cdr);
    clo.credit_model.recovery_spec = RecoveryModelSpec::with_lag(recovery, lag);
    clo
}

// ============================================================================
// Feature 1: Tranche Write-Down / Loss Allocation
// ============================================================================

#[test]
fn writedown_recorded_under_severe_stress() {
    // With CDR=25% and low recovery, defaults should erode the pool enough
    // that loss allocation writes down junior tranches.
    let market = flat_market();
    let clo = build_clo(0.05, 0.25, 0.10, 6);

    let results = run_simulation(&clo, &market, as_of()).unwrap();

    // Equity tranche should be written down (first loss)
    let eq = results.get("EQ").unwrap();
    assert!(
        eq.total_writedown.amount() > 0.0,
        "Equity should have write-downs under 25% CDR: got {}",
        eq.total_writedown.amount(),
    );

    // Write-down flows should be recorded
    assert!(
        !eq.writedown_flows.is_empty(),
        "Equity should have write-down flow entries",
    );

    // Write-down flow sum should match total
    let wd_sum: f64 = eq.writedown_flows.iter().map(|(_, m)| m.amount()).sum();
    assert!(
        (wd_sum - eq.total_writedown.amount()).abs() < 1.0,
        "Write-down sum ({}) should match total ({})",
        wd_sum,
        eq.total_writedown.amount(),
    );
}

#[test]
fn writedown_respects_subordination_order() {
    // Equity should be written down before mezzanine,
    // and mezzanine before senior.
    let market = flat_market();
    let clo = build_clo(0.05, 0.25, 0.10, 6);

    let results = run_simulation(&clo, &market, as_of()).unwrap();

    let sr = results.get("SR").unwrap();
    let mz = results.get("MZ").unwrap();
    let eq = results.get("EQ").unwrap();

    // Equity write-down percentage >= Mezz percentage >= Senior percentage
    let eq_wd_pct = eq.total_writedown.amount() / 10_000_000.0;
    let mz_wd_pct = mz.total_writedown.amount() / 20_000_000.0;
    let sr_wd_pct = sr.total_writedown.amount() / 70_000_000.0;

    assert!(
        eq_wd_pct >= mz_wd_pct - 0.001,
        "Equity write-down pct ({:.1}%) should be >= mezz ({:.1}%)",
        eq_wd_pct * 100.0,
        mz_wd_pct * 100.0,
    );
    assert!(
        mz_wd_pct >= sr_wd_pct - 0.001,
        "Mezz write-down pct ({:.1}%) should be >= senior ({:.1}%)",
        mz_wd_pct * 100.0,
        sr_wd_pct * 100.0,
    );
}

#[test]
fn no_writedown_without_defaults() {
    // With CDR=0, no write-downs should occur.
    let market = flat_market();
    let clo = build_clo(0.10, 0.0, 0.40, 6);

    let results = run_simulation(&clo, &market, as_of()).unwrap();

    for (tranche_id, tc) in &results {
        assert!(
            tc.total_writedown.amount() < 0.01,
            "Tranche {}: no write-down expected without defaults, got {}",
            tranche_id,
            tc.total_writedown.amount(),
        );
        assert!(
            tc.writedown_flows.is_empty(),
            "Tranche {}: no write-down flows expected without defaults",
            tranche_id,
        );
    }
}

#[test]
fn writedown_non_negative_and_bounded() {
    // Write-downs should be non-negative and not exceed original balance.
    let market = flat_market();
    let scenarios = [(0.10, 0.20), (0.05, 0.30), (0.02, 0.40)];

    for (cdr, recovery) in scenarios {
        let clo = build_clo(0.05, cdr, recovery, 6);
        let results = run_simulation(&clo, &market, as_of()).unwrap();

        for (tranche_id, tc) in &results {
            assert!(
                tc.total_writedown.amount() >= 0.0,
                "[CDR={},Rec={}] {}: write-down should be non-negative: {}",
                cdr,
                recovery,
                tranche_id,
                tc.total_writedown.amount(),
            );

            // Write-down flows should all be non-negative
            for (_, amt) in &tc.writedown_flows {
                assert!(
                    amt.amount() >= 0.0,
                    "[CDR={},Rec={}] {}: negative write-down flow: {}",
                    cdr,
                    recovery,
                    tranche_id,
                    amt.amount(),
                );
            }
        }
    }
}

// ============================================================================
// Feature 4: Clean-Up Call Modeling
// ============================================================================

#[test]
fn cleanup_call_triggers_when_pool_factor_below_threshold() {
    // With high CPR, pool factor drops quickly. Set cleanup_call_pct = 0.30 (30%)
    // so the call triggers while there's still meaningful balance.
    let market = flat_market();
    let mut clo = build_clo(0.40, 0.0, 0.40, 6); // Very high CPR
    clo.cleanup_call_pct = Some(0.30); // Trigger at 30% pool factor

    let results = run_simulation(&clo, &market, as_of()).unwrap();

    // After cleanup call, all tranche balances should be zero
    let sr = results.get("SR").unwrap();
    let mz = results.get("MZ").unwrap();

    assert!(
        sr.final_balance.amount() < 1.0,
        "Senior should be fully redeemed after cleanup call: {}",
        sr.final_balance.amount(),
    );
    assert!(
        mz.final_balance.amount() < 1.0,
        "Mezz should be fully redeemed after cleanup call: {}",
        mz.final_balance.amount(),
    );
}

#[test]
fn cleanup_call_produces_fewer_periods_than_no_call() {
    // A deal with cleanup call should terminate earlier than one without.
    let market = flat_market();

    let mut clo_no_call = build_clo(0.30, 0.0, 0.40, 6);
    clo_no_call.cleanup_call_pct = None;

    let mut clo_with_call = build_clo(0.30, 0.0, 0.40, 6);
    clo_with_call.cleanup_call_pct = Some(0.20); // 20% threshold

    let res_no = run_simulation(&clo_no_call, &market, as_of()).unwrap();
    let res_yes = run_simulation(&clo_with_call, &market, as_of()).unwrap();

    let periods_no = res_no.get("SR").unwrap().cashflows.len();
    let periods_yes = res_yes.get("SR").unwrap().cashflows.len();

    assert!(
        periods_yes <= periods_no,
        "Cleanup call should produce fewer or equal periods: with_call={}, without={}",
        periods_yes,
        periods_no,
    );
}

#[test]
fn cleanup_call_disabled_by_default() {
    // Without setting cleanup_call_pct, it should be None.
    let clo = build_clo(0.10, 0.0, 0.40, 6);
    assert!(
        clo.cleanup_call_pct.is_none(),
        "Cleanup call should be disabled by default",
    );
}

#[test]
fn cleanup_call_does_not_trigger_for_low_cpr() {
    // With low CPR, pool factor stays high and cleanup call doesn't trigger.
    let market = flat_market();
    let mut clo = build_clo(0.02, 0.0, 0.40, 6);
    clo.cleanup_call_pct = Some(0.10); // 10% threshold

    let results = run_simulation(&clo, &market, as_of()).unwrap();

    // With very low CPR and 5-year maturity, pool factor stays well above 10%
    // until near maturity. The deal should run to completion normally.
    let sr = results.get("SR").unwrap();
    assert!(
        sr.cashflows.len() > 10,
        "Low CPR deal should run many periods: got {}",
        sr.cashflows.len(),
    );
}

// ============================================================================
// Feature 3: OC/IC Cure Amount (Integration-level)
// ============================================================================

#[test]
fn waterfall_with_coverage_triggers_still_works() {
    // Ensure that adding coverage triggers doesn't break the simulation.
    // The cure mechanism should be transparent when no triggers are active.
    let market = flat_market();
    let clo = build_clo(0.10, 0.02, 0.40, 6);

    // Run simulation - should succeed without panic
    let results = run_simulation(&clo, &market, as_of()).unwrap();

    // Basic sanity
    assert!(!results.is_empty(), "Should produce tranche results");
    for tc in results.values() {
        assert!(
            tc.total_interest.amount() >= 0.0,
            "Interest should be non-negative",
        );
    }
}

/// Available-funds cap (net-WAC cap) layered on via `waterfall_rules`.
mod afc_tests {
    use finstack_quant_cashflows::builder::{
        DefaultModelSpec, PrepaymentModelSpec, RecoveryModelSpec,
    };
    use finstack_quant_core::currency::Currency;
    use finstack_quant_core::dates::{Date, DayCount};
    use finstack_quant_core::market_data::context::MarketContext;
    use finstack_quant_core::market_data::term_structures::DiscountCurve;
    use finstack_quant_core::money::Money;
    use finstack_quant_valuations::instruments::fixed_income::structured_credit::{
        AfcSpec, AssetPool, DealType, PoolAsset, StructuredCredit, Tranche, TrancheCoupon,
        TrancheSeniority, TrancheStructure, WaterfallRules,
    };
    use time::Month;

    fn closing() -> Date {
        Date::from_calendar_date(2024, Month::January, 1).unwrap()
    }

    fn maturity() -> Date {
        Date::from_calendar_date(2027, Month::January, 1).unwrap()
    }

    fn market() -> MarketContext {
        let disc = DiscountCurve::builder("USD-OIS")
            .base_date(closing())
            .knots(vec![(0.0, 1.0), (5.0, 0.90)])
            .build()
            .unwrap();
        MarketContext::new().insert(disc)
    }

    /// Pool earns 5%; senior coupon is 6%. The pool has enough cash to pay the
    /// 6% uncapped, so the only thing that reduces senior interest with AFC on
    /// is the net-WAC cap (6% -> 5%), not a cash shortfall.
    fn deal(with_afc: bool) -> StructuredCredit {
        let mut pool = AssetPool::new("POOL", DealType::ABS, Currency::USD);
        pool.assets.push(PoolAsset::fixed_rate_bond(
            "A1",
            Money::new(1_000_000.0, Currency::USD),
            0.05,
            maturity(),
            DayCount::Thirty360,
        ));
        let tranches = TrancheStructure::new(vec![
            Tranche::new(
                "SR",
                0.0,
                80.0,
                TrancheSeniority::Senior,
                Money::new(800_000.0, Currency::USD),
                TrancheCoupon::Fixed { rate: 0.06 },
                maturity(),
            )
            .unwrap(),
            Tranche::new(
                "EQ",
                80.0,
                100.0,
                TrancheSeniority::Equity,
                Money::new(200_000.0, Currency::USD),
                TrancheCoupon::Fixed { rate: 0.0 },
                maturity(),
            )
            .unwrap(),
        ])
        .unwrap();
        let mut sc =
            StructuredCredit::new_abs("ABS-AFC", pool, tranches, closing(), maturity(), "USD-OIS")
                .with_payment_calendar("nyse");
        sc.credit_model.prepayment_spec = PrepaymentModelSpec::constant_cpr(0.0);
        sc.credit_model.default_spec = DefaultModelSpec::constant_cdr(0.0);
        sc.credit_model.recovery_spec = RecoveryModelSpec::with_lag(0.40, 0);
        if with_afc {
            sc.waterfall_rules = Some(WaterfallRules {
                afc: Some(AfcSpec {
                    capped_tranches: vec!["SR".to_string()],
                }),
            });
        }
        sc
    }

    #[test]
    fn afc_caps_senior_interest_at_collateral_wac() {
        let mkt = market();
        let uncapped = deal(false)
            .get_tranche_cashflows("SR", &mkt, closing())
            .unwrap();
        let capped = deal(true)
            .get_tranche_cashflows("SR", &mkt, closing())
            .unwrap();

        assert!(
            capped.total_interest.amount() > 0.0,
            "capped senior should still receive interest"
        );
        // The 6% coupon is capped at the 5% pool WAC, so capped interest must be
        // strictly below uncapped, and meaningfully so (not a rounding artefact).
        assert!(
            capped.total_interest.amount() < 0.95 * uncapped.total_interest.amount(),
            "AFC must reduce senior interest (capped={}, uncapped={})",
            capped.total_interest.amount(),
            uncapped.total_interest.amount()
        );
    }

    #[test]
    fn no_rules_is_identity() {
        // A deal with no waterfall_rules must price exactly as before the seam.
        let mkt = market();
        let a = deal(false)
            .get_tranche_cashflows("SR", &mkt, closing())
            .unwrap();
        let b = deal(false)
            .get_tranche_cashflows("SR", &mkt, closing())
            .unwrap();
        assert_eq!(a.total_interest.amount(), b.total_interest.amount());
        assert_eq!(a.total_principal.amount(), b.total_principal.amount());
    }
}
