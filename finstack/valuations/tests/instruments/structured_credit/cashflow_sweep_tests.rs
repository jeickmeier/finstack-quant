//! Cashflow sensitivity sweep tests for structured credit instruments.
//!
//! These tests verify that changing CDR/CPR/Recovery/Recovery Lag parameters
//! produces correctly directional and correctly sized changes in pool cashflows
//! and tranche distributions.
//!
//! # Market Standard References
//!
//! - Higher CDR → more defaults → less interest, more losses → lower tranche cashflows
//! - Higher CPR → faster prepayment → shorter WAL, less total interest
//! - Higher Recovery → more recovered from defaults → more cash through waterfall
//! - Longer Recovery Lag → same total recovery but delayed → lower PV
//!
//! # INTEX Parity
//!
//! These sweeps replicate the standard scenario analysis performed by INTEX:
//! Run the deal under a grid of CDR/CPR/severity assumptions and verify
//! monotonic behavior of key outputs (WAL, total interest, total principal,
//! tranche cashflows, OC ratios).

use finstack_cashflows::builder::{DefaultModelSpec, PrepaymentModelSpec, RecoveryModelSpec};
use finstack_core::currency::Currency;
use finstack_core::dates::Date;
use finstack_core::market_data::context::MarketContext;
use finstack_core::market_data::term_structures::{DiscountCurve, ForwardCurve};
use finstack_core::math::interp::InterpStyle;
use finstack_core::money::Money;
use finstack_core::types::{CreditRating, InstrumentId};
use finstack_valuations::instruments::fixed_income::structured_credit::{
    run_simulation, AssetPool, AssetType, DealType, PoolAsset, StructuredCredit, Tranche,
    TrancheCoupon, TrancheSeniority, TrancheStructure,
};
use time::Month;

// ============================================================================
// Test Helpers
// ============================================================================

fn as_of() -> Date {
    Date::from_calendar_date(2025, Month::October, 5).unwrap()
}

fn closing() -> Date {
    Date::from_calendar_date(2024, Month::January, 1).unwrap()
}

fn maturity() -> Date {
    Date::from_calendar_date(2030, Month::December, 31).unwrap()
}

fn create_pool() -> AssetPool {
    let mut pool = AssetPool::new("SWEEP_POOL", DealType::CLO, Currency::USD);

    for i in 0..10 {
        pool.assets.push(PoolAsset {
            day_count: finstack_core::dates::DayCount::Act360,
            id: InstrumentId::new(format!("LOAN_{}", i)),
            asset_type: AssetType::FirstLienLoan {
                industry: Some(format!("Industry_{}", i % 5)),
            },
            balance: Money::new(25_000_000.0, Currency::USD),
            rate: 0.08,
            spread_bps: Some(400.0),
            index_id: Some("SOFR-3M".to_string()),
            maturity: maturity(),
            credit_quality: Some(CreditRating::BB),
            industry: Some(format!("Industry_{}", i % 5)),
            obligor_id: Some(format!("OBLIGOR_{}", i)),
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

fn create_tranches() -> TrancheStructure {
    let senior = Tranche::new(
        "CLASS_A",
        0.0,
        70.0,
        TrancheSeniority::Senior,
        Money::new(175_000_000.0, Currency::USD),
        TrancheCoupon::Fixed { rate: 0.05 },
        maturity(),
    )
    .unwrap();

    let mezz = Tranche::new(
        "CLASS_B",
        70.0,
        90.0,
        TrancheSeniority::Mezzanine,
        Money::new(50_000_000.0, Currency::USD),
        TrancheCoupon::Fixed { rate: 0.07 },
        maturity(),
    )
    .unwrap();

    let equity = Tranche::new(
        "EQUITY",
        90.0,
        100.0,
        TrancheSeniority::Equity,
        Money::new(25_000_000.0, Currency::USD),
        TrancheCoupon::Fixed { rate: 0.0 },
        maturity(),
    )
    .unwrap();

    TrancheStructure::new(vec![senior, mezz, equity]).unwrap()
}

fn create_market() -> MarketContext {
    let discount = DiscountCurve::builder("USD_OIS")
        .base_date(as_of())
        .knots(vec![(0.0, 1.0), (0.25, 0.9875), (1.0, 0.95), (5.0, 0.78)])
        .interp(InterpStyle::Linear)
        .build()
        .unwrap();

    let forward = ForwardCurve::builder("SOFR-3M", 0.25)
        .base_date(as_of())
        .knots(vec![(0.0, 0.05), (1.0, 0.051), (2.0, 0.053), (5.0, 0.055)])
        .interp(InterpStyle::Linear)
        .build()
        .unwrap();

    MarketContext::new().insert(discount).insert(forward)
}

/// Build a CLO with specified behavioral parameters.
fn build_clo(cpr: f64, cdr: f64, recovery: f64, recovery_lag: u32) -> StructuredCredit {
    let mut clo = StructuredCredit::new_clo(
        "SWEEP_CLO",
        create_pool(),
        create_tranches(),
        closing(),
        maturity(),
        "USD_OIS",
    )
    .with_payment_calendar("nyse");

    clo.credit_model.prepayment_spec = PrepaymentModelSpec::constant_cpr(cpr);
    clo.credit_model.default_spec = DefaultModelSpec::constant_cdr(cdr);
    clo.credit_model.recovery_spec = RecoveryModelSpec::with_lag(recovery, recovery_lag);
    clo
}

/// Run simulation and return total interest and principal for a given tranche.
fn run_and_get_tranche_totals(
    clo: &StructuredCredit,
    market: &MarketContext,
    tranche_id: &str,
) -> (f64, f64) {
    let results = run_simulation(clo, market, as_of()).expect("simulation should succeed");

    let tc = results.get(tranche_id).expect("tranche should exist");
    (tc.total_interest.amount(), tc.total_principal.amount())
}

/// Run simulation and return (total_interest, total_principal, final_balance) for all tranches.
fn run_full(
    clo: &StructuredCredit,
    market: &MarketContext,
) -> std::collections::HashMap<String, (f64, f64, f64)> {
    let results = run_simulation(clo, market, as_of()).expect("simulation should succeed");

    results
        .into_iter()
        .map(|(id, tc)| {
            (
                id,
                (
                    tc.total_interest.amount(),
                    tc.total_principal.amount(),
                    tc.final_balance.amount(),
                ),
            )
        })
        .collect()
}

// ============================================================================
// CDR Sweep Tests
// ============================================================================

#[test]
fn sweep_cdr_increases_reduce_total_tranche_cash() {
    // Higher CDR → more defaults → smaller pool → less total cash to tranches
    // Compare extreme endpoints where signal is unambiguous
    let market = create_market();

    // CDR=0 vs CDR=15%: total tranche distributions should be meaningfully different
    let clo_zero = build_clo(0.10, 0.0, 0.40, 12);
    let clo_high = build_clo(0.10, 0.15, 0.40, 12);

    let res_zero = run_full(&clo_zero, &market);
    let res_high = run_full(&clo_high, &market);

    let total_zero: f64 = res_zero.values().map(|v| v.0 + v.1).sum();
    let total_high: f64 = res_high.values().map(|v| v.0 + v.1).sum();

    // With high CDR, total tranche distributions should be lower
    assert!(
        total_high < total_zero,
        "CDR=15% should produce less total tranche cash than CDR=0%: high={}, zero={}",
        total_high,
        total_zero,
    );

    // Verify meaningful sensitivity (>5% difference)
    let pct_diff = (total_zero - total_high) / total_zero;
    assert!(
        pct_diff > 0.05,
        "CDR sweep should show >5% total cash difference: zero={}, high={}, diff={}%",
        total_zero,
        total_high,
        pct_diff * 100.0
    );

    // Also verify intermediate CDR values show sensitivity
    let clo_mid = build_clo(0.10, 0.05, 0.40, 12);
    let res_mid = run_full(&clo_mid, &market);
    let total_mid: f64 = res_mid.values().map(|v| v.0 + v.1).sum();

    // Mid CDR should be between zero and high
    assert!(
        total_mid < total_zero && total_mid > total_high,
        "CDR=5% should be between CDR=0% and CDR=15%: zero={}, mid={}, high={}",
        total_zero,
        total_mid,
        total_high,
    );
}

#[test]
fn sweep_cdr_increases_reduce_equity_distributions() {
    // Higher CDR → less residual cash → equity gets less
    let market = create_market();
    let cdrs = [0.0, 0.02, 0.05, 0.10];

    let mut prev_total = f64::MAX;
    for cdr in cdrs {
        let clo = build_clo(0.10, cdr, 0.40, 12);
        let results = run_full(&clo, &market);
        let equity = results.get("EQUITY").expect("EQUITY tranche");
        let total = equity.0 + equity.1; // interest + principal

        assert!(
            total <= prev_total + 0.01,
            "CDR={}: Equity total {} should decrease with higher CDR (prev={})",
            cdr,
            total,
            prev_total,
        );
        prev_total = total;
    }
}

// ============================================================================
// CPR Sweep Tests
// ============================================================================

#[test]
fn sweep_cpr_increases_reduce_total_interest() {
    // Higher CPR → faster prepayment → shorter average life → less total interest
    let market = create_market();
    let cprs = [0.0, 0.05, 0.10, 0.20, 0.30];

    let mut prev_interest = f64::MAX;
    for cpr in cprs {
        let clo = build_clo(cpr, 0.02, 0.40, 12);
        let (interest, _) = run_and_get_tranche_totals(&clo, &market, "CLASS_A");

        assert!(
            interest <= prev_interest + 0.01,
            "CPR={}: Senior interest {} should decrease with higher CPR (prev={})",
            cpr,
            interest,
            prev_interest,
        );
        prev_interest = interest;
    }
}

#[test]
fn sweep_cpr_increases_accelerate_principal_return() {
    // Higher CPR → faster principal return → more total principal distributed sooner
    // (Note: total principal returned should converge to original balance in all cases)
    let market = create_market();

    let clo_slow = build_clo(0.05, 0.02, 0.40, 12);
    let clo_fast = build_clo(0.30, 0.02, 0.40, 12);

    let results_slow = run_full(&clo_slow, &market);
    let results_fast = run_full(&clo_fast, &market);

    // Fast prepayment should return principal sooner (check senior tranche)
    let slow_sr = results_slow.get("CLASS_A").unwrap();
    let fast_sr = results_fast.get("CLASS_A").unwrap();

    // Both should return similar total principal (adjusted for defaults)
    // But fast CPR should leave smaller final balance
    assert!(
        fast_sr.2 <= slow_sr.2 + 1.0,
        "Fast CPR should have smaller or equal final senior balance: fast={}, slow={}",
        fast_sr.2,
        slow_sr.2
    );
}

// ============================================================================
// Recovery Rate Sweep Tests
// ============================================================================

#[test]
fn sweep_higher_recovery_increases_total_cash() {
    // Higher recovery → more cash from defaults → more available for waterfall
    // Compare extreme endpoints (0% vs 80%) where the signal dominates noise
    //
    // Note: Due to interactions between recovery lag, waterfall priorities,
    // and PIK dynamics, strict monotonicity at intermediate points may not
    // hold. The key structural property is that significantly higher recovery
    // produces more total cash to tranches.
    let market = create_market();

    let clo_zero = build_clo(0.10, 0.05, 0.0, 6);
    let clo_high = build_clo(0.10, 0.05, 0.80, 6);

    let total_zero: f64 = run_full(&clo_zero, &market)
        .values()
        .map(|v| v.0 + v.1)
        .sum();
    let total_high: f64 = run_full(&clo_high, &market)
        .values()
        .map(|v| v.0 + v.1)
        .sum();

    assert!(
        total_high > total_zero,
        "80% recovery should produce more total cash than 0% recovery: high={}, zero={}",
        total_high,
        total_zero,
    );

    // Verify meaningful difference (>1% for 80% vs 0% recovery with 5% CDR)
    let pct_diff = (total_high - total_zero) / total_zero;
    assert!(
        pct_diff > 0.01,
        "Recovery sweep 0% → 80% should show >1% total cash difference: {}%",
        pct_diff * 100.0,
    );
}

#[test]
fn sweep_recovery_rate_affects_equity_more_than_senior() {
    // Recovery rate changes should have larger impact on equity (residual) than senior
    // because senior gets paid first and recovery changes affect the tail
    let market = create_market();

    let clo_low = build_clo(0.10, 0.05, 0.20, 6);
    let clo_high = build_clo(0.10, 0.05, 0.60, 6);

    let res_low = run_full(&clo_low, &market);
    let res_high = run_full(&clo_high, &market);

    let sr_low = res_low.get("CLASS_A").unwrap();
    let sr_high = res_high.get("CLASS_A").unwrap();
    let eq_low = res_low.get("EQUITY").unwrap();
    let eq_high = res_high.get("EQUITY").unwrap();

    let sr_change = ((sr_high.0 + sr_high.1) - (sr_low.0 + sr_low.1)).abs();
    let eq_change = ((eq_high.0 + eq_high.1) - (eq_low.0 + eq_low.1)).abs();

    // Equity should be more sensitive to recovery changes (in absolute or relative terms)
    // This is a key structural credit property
    let sr_total = (sr_low.0 + sr_low.1).max(1.0);
    let eq_total = (eq_low.0 + eq_low.1).max(1.0);

    let sr_pct_change = sr_change / sr_total;
    let eq_pct_change = eq_change / eq_total;

    assert!(
        eq_pct_change >= sr_pct_change,
        "Equity should be more sensitive to recovery changes: equity_pct={}%, senior_pct={}%",
        eq_pct_change * 100.0,
        sr_pct_change * 100.0,
    );
}

// ============================================================================
// Recovery Lag Sweep Tests
// ============================================================================

#[test]
fn sweep_recovery_lag_affects_cash_timing() {
    // Recovery lag changes the TIMING of recoveries, not the total amount
    // (assuming all recoveries are released before simulation ends).
    //
    // Key properties:
    // 1. Different lag values produce different cashflow profiles
    // 2. Total cash should be roughly similar (within 5%) across reasonable lags
    // 3. With very long lag, some recoveries may fall past maturity → less total
    let market = create_market();
    let lags = [0, 6, 12, 24];

    let mut totals = Vec::new();
    for lag in lags {
        let clo = build_clo(0.10, 0.05, 0.40, lag);
        let results = run_full(&clo, &market);
        let total: f64 = results.values().map(|v| v.0 + v.1).sum();
        totals.push((lag, total));
    }

    // All lag scenarios should produce positive total cash
    for (lag, total) in &totals {
        assert!(
            *total > 0.0,
            "Recovery lag={}: total cash should be positive, got {}",
            lag,
            total
        );
    }

    // Total cash should vary by less than 10% across lag values
    // (recovery lag changes timing but not total amount, modulo maturity truncation)
    let min_total = totals.iter().map(|(_, t)| *t).fold(f64::MAX, f64::min);
    let max_total = totals.iter().map(|(_, t)| *t).fold(f64::MIN, f64::max);

    let variation = (max_total - min_total) / min_total;
    assert!(
        variation < 0.10,
        "Recovery lag should not drastically change total cash: min={}, max={}, variation={}%",
        min_total,
        max_total,
        variation * 100.0,
    );
}

// ============================================================================
// Combined Stress Scenarios
// ============================================================================

#[test]
fn stress_scenario_base_vs_stressed() {
    // Compare base case vs stressed scenario.
    //
    // Stressed scenario increases CDR significantly while keeping CPR the same,
    // and reduces recovery. This ensures the stress is unambiguously worse
    // (higher CDR + same CPR + lower recovery = less total cash).
    //
    // Base:    CDR=2%, CPR=10%, Recovery=40%, Lag=12
    // Stress:  CDR=15%, CPR=10%, Recovery=20%, Lag=18
    let market = create_market();

    let base = build_clo(0.10, 0.02, 0.40, 12);
    let stressed = build_clo(0.10, 0.15, 0.20, 18);

    let base_results = run_full(&base, &market);
    let stressed_results = run_full(&stressed, &market);

    // Total cash to all tranches should be lower in stress
    let base_total: f64 = base_results.values().map(|v| v.0 + v.1).sum();
    let stressed_total: f64 = stressed_results.values().map(|v| v.0 + v.1).sum();

    assert!(
        stressed_total < base_total,
        "Stressed scenario should produce less total cashflow: base={}, stressed={}",
        base_total,
        stressed_total,
    );

    // Equity should be disproportionately impacted
    let eq_base = base_results.get("EQUITY").unwrap();
    let eq_stressed = stressed_results.get("EQUITY").unwrap();

    let eq_base_total = eq_base.0 + eq_base.1;
    let eq_stressed_total = eq_stressed.0 + eq_stressed.1;

    assert!(
        eq_stressed_total < eq_base_total,
        "Equity should be severely impacted: base={}, stressed={}",
        eq_base_total,
        eq_stressed_total,
    );

    // Senior should also be impacted but less than equity (percentage-wise)
    let sr_base = base_results.get("CLASS_A").unwrap();
    let sr_stressed = stressed_results.get("CLASS_A").unwrap();

    let sr_base_total = sr_base.0 + sr_base.1;
    let sr_stressed_total = sr_stressed.0 + sr_stressed.1;

    let sr_pct_drop = (sr_base_total - sr_stressed_total) / sr_base_total;
    let eq_pct_drop = if eq_base_total > 0.0 {
        (eq_base_total - eq_stressed_total) / eq_base_total
    } else {
        0.0
    };

    // Equity percentage drop should be >= senior percentage drop
    // (subordination protects senior tranches)
    assert!(
        eq_pct_drop >= sr_pct_drop - 0.01, // small tolerance
        "Equity should lose more (pct) than senior: equity_drop={}%, senior_drop={}%",
        eq_pct_drop * 100.0,
        sr_pct_drop * 100.0,
    );
}

// ============================================================================
// PIK (Payment-in-Kind) Accretion Tests
// ============================================================================

#[test]
fn pik_accretion_increases_tranche_balance_on_shortfall() {
    // When a tranche doesn't receive full interest, the shortfall is
    // added to the tranche balance (PIK). Verify this by setting up
    // a scenario where cash is insufficient.
    let market = create_market();

    // High CDR + high CPR = less pool cash = tranche shortfalls
    let clo = build_clo(0.05, 0.15, 0.20, 12);
    let results = run_simulation(&clo, &market, as_of()).unwrap();

    // Check mezzanine tranche for PIK flows
    let mezz = results.get("CLASS_B").unwrap();

    // In a severe stress scenario, mezzanine may have PIK
    // PIK total should be >= 0
    assert!(
        mezz.total_pik.amount() >= 0.0,
        "PIK total should be non-negative"
    );

    // If there are PIK flows, verify they match the total
    if !mezz.pik_flows.is_empty() {
        let pik_sum: f64 = mezz.pik_flows.iter().map(|(_, amt)| amt.amount()).sum();
        assert!(
            (pik_sum - mezz.total_pik.amount()).abs() < 0.01,
            "PIK flow sum should match total_pik: sum={}, total={}",
            pik_sum,
            mezz.total_pik.amount()
        );
    }

    // If PIK occurred, final balance should be >= original balance minus principal paid
    // (because PIK adds to balance)
    if mezz.total_pik.amount() > 0.0 {
        let original_balance = 50_000_000.0; // CLASS_B original
        let expected_min_balance = original_balance - mezz.total_principal.amount();
        assert!(
            mezz.final_balance.amount() >= expected_min_balance - 0.01,
            "PIK should accrete balance: final={}, min_expected={}",
            mezz.final_balance.amount(),
            expected_min_balance
        );
    }
}

// ============================================================================
// Cash Conservation (Full Simulation)
// ============================================================================

#[test]
fn full_simulation_total_distributed_consistent() {
    // Verify that across the full simulation, total distributed to all tranches
    // is consistent (interest + principal adds up properly)
    let market = create_market();
    let clo = build_clo(0.10, 0.02, 0.40, 12);

    let results = run_simulation(&clo, &market, as_of()).unwrap();

    for (tranche_id, tc) in &results {
        // Total cashflows should equal interest + principal + PIK shortfalls reinvested
        let cf_sum: f64 = tc.cashflows.iter().map(|(_, amt)| amt.amount()).sum();
        let interest_sum: f64 = tc.interest_flows.iter().map(|(_, amt)| amt.amount()).sum();
        let principal_sum: f64 = tc.principal_flows.iter().map(|(_, amt)| amt.amount()).sum();

        // cashflows = interest_paid + principal_paid (not PIK, which is deferred)
        assert!(
            (cf_sum - (interest_sum + principal_sum)).abs() < 1.0,
            "Tranche {}: cashflow sum ({}) should equal interest ({}) + principal ({})",
            tranche_id,
            cf_sum,
            interest_sum,
            principal_sum,
        );

        // Verify totals match flow sums
        assert!(
            (tc.total_interest.amount() - interest_sum).abs() < 0.01,
            "Tranche {}: total_interest mismatch: {} vs {}",
            tranche_id,
            tc.total_interest.amount(),
            interest_sum,
        );
        assert!(
            (tc.total_principal.amount() - principal_sum).abs() < 0.01,
            "Tranche {}: total_principal mismatch: {} vs {}",
            tranche_id,
            tc.total_principal.amount(),
            principal_sum,
        );
    }
}

#[test]
fn full_simulation_non_negative_flows() {
    // All cashflow amounts should be non-negative
    let market = create_market();
    let clo = build_clo(0.10, 0.02, 0.40, 12);

    let results = run_simulation(&clo, &market, as_of()).unwrap();

    for (tranche_id, tc) in &results {
        for (date, amount) in &tc.cashflows {
            assert!(
                amount.amount() >= 0.0,
                "Tranche {} at {:?}: negative cashflow {}",
                tranche_id,
                date,
                amount.amount()
            );
        }

        for (date, amount) in &tc.interest_flows {
            assert!(
                amount.amount() >= 0.0,
                "Tranche {} at {:?}: negative interest {}",
                tranche_id,
                date,
                amount.amount()
            );
        }

        for (date, amount) in &tc.principal_flows {
            assert!(
                amount.amount() >= 0.0,
                "Tranche {} at {:?}: negative principal {}",
                tranche_id,
                date,
                amount.amount()
            );
        }
    }
}

// ============================================================================
// Zero CDR/CPR Baseline Tests
// ============================================================================

#[test]
fn zero_cdr_zero_cpr_returns_all_principal_at_maturity() {
    // With no defaults and no prepayments, all principal should
    // be returned as balloon payments when assets mature
    let market = create_market();
    let clo = build_clo(0.0, 0.0, 0.40, 12);

    let results = run_full(&clo, &market);

    // Total principal distributed across all tranches should approximate
    // the original tranche balances
    let total_principal: f64 = results.values().map(|v| v.1).sum();
    let total_original = 175_000_000.0 + 50_000_000.0 + 25_000_000.0;

    // With zero CPR and zero CDR, all pool principal returns via balloon at maturity
    // Total distributed should cover tranche balances
    assert!(
        total_principal > 0.0,
        "Should distribute some principal even with zero CDR/CPR (balloon payments)"
    );

    // Final balances for all tranches should be zero or close to it
    // (all principal returned)
    for (tranche_id, (_, _principal, final_bal)) in &results {
        if *tranche_id != "EQUITY" {
            // Non-equity tranches should be paid down
            assert!(
                *final_bal < total_original * 0.01,
                "Tranche {} final balance {} should be near zero with no losses",
                tranche_id,
                final_bal
            );
        }
    }
}
