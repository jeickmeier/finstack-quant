//! Comprehensive tests for private markets fund waterfall functionality.

use finstack_core::currency::Currency;
use finstack_core::dates::{Date, DayCount};
use finstack_core::money::Money;
use finstack_valuations::instruments::equity::pe_fund::*;
use time::Month;

fn test_currency() -> Currency {
    Currency::USD
}

fn test_date(year: i32, month: u8, day: u8) -> Date {
    Date::from_calendar_date(year, Month::try_from(month).unwrap(), day).unwrap()
}

/// Create a simple 2x return scenario for testing.
fn simple_2x_scenario() -> (WaterfallSpec, Vec<FundEvent>) {
    let spec = WaterfallSpec::builder()
        .style(WaterfallStyle::European)
        .return_of_capital()
        .preferred_irr(0.08) // 8% hurdle
        .catchup(1.0) // 100% to GP catch-up
        .promote_tier(0.0, 0.8, 0.2) // 80/20 split after catch-up
        .build()
        .unwrap();

    let events = vec![
        FundEvent::contribution(
            test_date(2020, 1, 1),
            Money::new(1000000.0, test_currency()),
        ),
        FundEvent::distribution(
            test_date(2025, 1, 1),
            Money::new(2000000.0, test_currency()),
        ),
    ];

    (spec, events)
}

#[test]
fn test_return_of_capital_first() {
    let (spec, events) = simple_2x_scenario();
    let engine = EquityWaterfallEngine::new(&spec);
    let ledger = engine.run(&events).unwrap();

    // Find return of capital allocation
    let roc_rows: Vec<_> = ledger
        .rows
        .iter()
        .filter(|r| r.tranche.contains("Return of Capital"))
        .collect();

    assert!(
        !roc_rows.is_empty(),
        "Should have return of capital allocation"
    );

    // LP should get back their $1M capital first
    let total_lp_roc: f64 = roc_rows.iter().map(|r| r.to_lp.amount()).sum();

    assert!(
        (total_lp_roc - 1000000.0).abs() < 1e-6,
        "LP should get $1M return of capital, got ${:.2}",
        total_lp_roc
    );

    // After ROC, LP unreturned should be zero
    if let Some(last_roc) = roc_rows.last() {
        assert!(
            (last_roc.lp_unreturned.amount()).abs() < 1e-6,
            "LP unreturned should be zero after ROC"
        );
    }
}

#[test]
fn test_preferred_return_calculation() {
    let (spec, events) = simple_2x_scenario();
    let engine = EquityWaterfallEngine::new(&spec);
    let ledger = engine.run(&events).unwrap();

    // Find preferred return allocation
    let pref_rows: Vec<_> = ledger
        .rows
        .iter()
        .filter(|r| r.tranche.contains("Preferred Return"))
        .collect();

    if !pref_rows.is_empty() {
        // With 8% hurdle over 5 years, LP should get additional return to meet IRR target
        let total_pref: f64 = pref_rows.iter().map(|r| r.to_lp.amount()).sum();

        assert!(
            total_pref > 0.0,
            "Should have some preferred return allocation"
        );

        // All preferred return should go to LP
        let total_gp_pref: f64 = pref_rows.iter().map(|r| r.to_gp.amount()).sum();
        assert!(
            (total_gp_pref).abs() < 1e-6,
            "GP should get no preferred return"
        );
    }
}

#[test]
fn test_promote_split() {
    let (spec, events) = simple_2x_scenario();
    let engine = EquityWaterfallEngine::new(&spec);
    let ledger = engine.run(&events).unwrap();

    // Find promote allocation
    let promote_rows: Vec<_> = ledger
        .rows
        .iter()
        .filter(|r| r.tranche.contains("Promote"))
        .collect();

    if !promote_rows.is_empty() {
        for row in &promote_rows {
            let total_alloc = row.to_lp.amount() + row.to_gp.amount();
            if total_alloc > 1e-6 {
                let lp_pct = row.to_lp.amount() / total_alloc;
                let gp_pct = row.to_gp.amount() / total_alloc;

                // Should approximate 80/20 split
                assert!((lp_pct - 0.8).abs() < 0.1, "LP should get ~80% in promote");
                assert!((gp_pct - 0.2).abs() < 0.1, "GP should get ~20% in promote");
            }
        }
    }
}

#[test]
fn test_currency_mismatch_error() {
    let spec = WaterfallSpec::builder()
        .return_of_capital()
        .build()
        .unwrap();

    let mixed_currency_events = vec![
        FundEvent::contribution(test_date(2020, 1, 1), Money::new(1000000.0, Currency::USD)),
        FundEvent::distribution(test_date(2025, 1, 1), Money::new(1500000.0, Currency::EUR)), // Different currency
    ];

    let pe = PrivateMarketsFund::new("TEST", Currency::USD, spec, mixed_currency_events);
    let result = pe.run_waterfall();

    assert!(result.is_err(), "Should error on currency mismatch");
    if let Err(finstack_core::Error::CurrencyMismatch { expected, actual }) = result {
        assert_eq!(expected, Currency::USD);
        assert_eq!(actual, Currency::EUR);
    } else {
        panic!("Expected CurrencyMismatch error");
    }
}

#[test]
fn test_american_vs_european_style() {
    // Create identical events for both styles
    let events = vec![
        FundEvent::contribution(
            test_date(2020, 1, 1),
            Money::new(1000000.0, test_currency()),
        ),
        FundEvent::proceeds(
            test_date(2023, 1, 1),
            Money::new(800000.0, test_currency()),
            "Deal_A",
        ),
        FundEvent::proceeds(
            test_date(2025, 1, 1),
            Money::new(1200000.0, test_currency()),
            "Deal_B",
        ),
    ];

    // European style - aggregate at fund level
    let euro_spec = WaterfallSpec::builder()
        .style(WaterfallStyle::European)
        .return_of_capital()
        .promote_tier(0.0, 0.8, 0.2)
        .build()
        .unwrap();

    let euro_engine = EquityWaterfallEngine::new(&euro_spec);
    let euro_ledger = euro_engine.run(&events).unwrap();

    // American style - allocate per deal
    let american_spec = WaterfallSpec::builder()
        .style(WaterfallStyle::American)
        .return_of_capital()
        .promote_tier(0.0, 0.8, 0.2)
        .build()
        .unwrap();

    let american_engine = EquityWaterfallEngine::new(&american_spec);
    let american_ledger = american_engine.run(&events).unwrap();

    // Both should have some allocation rows
    assert!(
        !euro_ledger.rows.is_empty(),
        "European ledger should have allocations"
    );
    assert!(
        !american_ledger.rows.is_empty(),
        "American ledger should have allocations"
    );

    // American style should have deal_id populated
    let american_with_deals = american_ledger
        .rows
        .iter()
        .filter(|r| r.deal_id.is_some())
        .count();
    assert!(
        american_with_deals > 0,
        "American style should have deal_id entries"
    );
}

#[test]
fn test_waterfall_spec_validation() {
    // Valid spec should pass
    let valid_spec = WaterfallSpec::builder()
        .return_of_capital()
        .preferred_irr(0.08)
        .promote_tier(0.0, 0.8, 0.2)
        .build();
    assert!(valid_spec.is_ok());

    // Invalid promote shares (don't sum to 1.0)
    let invalid_spec = WaterfallSpec {
        style: WaterfallStyle::European,
        tranches: smallvec::smallvec![Tranche::PromoteTier {
            hurdle: Hurdle::Irr { rate: 0.0 },
            lp_share: 0.7,
            gp_share: 0.4, // 0.7 + 0.4 = 1.1 > 1.0
        }],
        clawback: None,
        irr_basis: DayCount::Act365F,
        catchup_mode: CatchUpMode::Full,
    };
    assert!(invalid_spec.validate().is_err());

    // Negative shares should fail
    let negative_spec = WaterfallSpec {
        style: WaterfallStyle::European,
        tranches: smallvec::smallvec![Tranche::PromoteTier {
            hurdle: Hurdle::Irr { rate: 0.0 },
            lp_share: -0.2, // Negative share
            gp_share: 1.2,
        }],
        clawback: None,
        irr_basis: DayCount::Act365F,
        catchup_mode: CatchUpMode::Full,
    };
    assert!(negative_spec.validate().is_err());
}

#[test]
fn test_deterministic_allocation() {
    let (spec, events) = simple_2x_scenario();
    let engine = EquityWaterfallEngine::new(&spec);

    // Run multiple times and ensure identical results
    let ledger1 = engine.run(&events).unwrap();
    let ledger2 = engine.run(&events).unwrap();
    let ledger3 = engine.run(&events).unwrap();

    assert_eq!(ledger1.rows.len(), ledger2.rows.len());
    assert_eq!(ledger2.rows.len(), ledger3.rows.len());

    for i in 0..ledger1.rows.len() {
        let row1 = &ledger1.rows[i];
        let row2 = &ledger2.rows[i];
        let row3 = &ledger3.rows[i];

        assert_eq!(row1.date, row2.date);
        assert_eq!(row2.date, row3.date);
        assert_eq!(row1.tranche, row2.tranche);
        assert_eq!(row2.tranche, row3.tranche);
        assert!((row1.to_lp.amount() - row2.to_lp.amount()).abs() < 1e-12);
        assert!((row2.to_lp.amount() - row3.to_lp.amount()).abs() < 1e-12);
        assert!((row1.to_gp.amount() - row2.to_gp.amount()).abs() < 1e-12);
        assert!((row2.to_gp.amount() - row3.to_gp.amount()).abs() < 1e-12);
    }
}

#[test]
fn test_ledger_export_formats() {
    let (spec, events) = simple_2x_scenario();
    let engine = EquityWaterfallEngine::new(&spec);
    let ledger = engine.run(&events).unwrap();

    // Test tabular export
    let (columns, rows) = ledger.to_tabular_data();
    assert_eq!(columns.len(), 10); // Expected number of columns
    assert!(!rows.is_empty(), "Should have data rows");

    // Test JSON export
    let json = ledger.to_json().unwrap();
    assert!(json.contains("rows"), "JSON should contain rows");
    assert!(json.contains("meta"), "JSON should contain metadata");
}

#[test]
fn test_lp_cashflows_via_ledger() {
    let (spec, events) = simple_2x_scenario();
    let pe = PrivateMarketsFund::new("TEST_FUND", test_currency(), spec, events);

    let ledger = pe.run_waterfall().unwrap();
    let lp_flows = ledger.lp_cashflows();
    assert!(
        !lp_flows.is_empty(),
        "Should extract LP cashflows from ledger"
    );

    // Verify we have meaningful flows
    let total_flow: f64 = lp_flows.iter().map(|(_, amount)| amount.amount()).sum();
    assert!(
        total_flow.abs() > 1e-6,
        "Should have meaningful cashflow amounts"
    );
}

/// Property test: LP unreturned capital should never go negative.
#[test]
fn property_lp_unreturned_non_negative() {
    let (spec, events) = simple_2x_scenario();
    let engine = EquityWaterfallEngine::new(&spec);
    let ledger = engine.run(&events).unwrap();

    for row in &ledger.rows {
        assert!(
            row.lp_unreturned.amount() >= -1e-6,
            "LP unreturned should never be negative, got {} on {}",
            row.lp_unreturned.amount(),
            row.date
        );
    }
}

/// Property test: GP carry should be non-decreasing (absent clawback).
#[test]
fn property_gp_carry_monotonic() {
    let (spec, events) = simple_2x_scenario();
    let engine = EquityWaterfallEngine::new(&spec);
    let ledger = engine.run(&events).unwrap();

    let mut prev_carry = 0.0;
    for row in &ledger.rows {
        assert!(
            row.gp_carry_cum.amount() >= prev_carry - 1e-6,
            "GP carry should be non-decreasing, got {} after {} on {}",
            row.gp_carry_cum.amount(),
            prev_carry,
            row.date
        );
        prev_carry = row.gp_carry_cum.amount();
    }
}

/// Test complex waterfall with multiple tiers.
#[test]
fn test_multi_tier_waterfall() {
    let spec = WaterfallSpec::builder()
        .style(WaterfallStyle::European)
        .return_of_capital()
        .preferred_irr(0.08) // 8% hurdle
        .catchup(1.0) // 100% catch-up
        .promote_tier(0.12, 0.8, 0.2) // 80/20 up to 12% IRR
        .promote_tier(0.15, 0.7, 0.3) // 70/30 above 15% IRR
        .build()
        .unwrap();

    let events = vec![
        FundEvent::contribution(
            test_date(2020, 1, 1),
            Money::new(1000000.0, test_currency()),
        ),
        FundEvent::distribution(
            test_date(2025, 1, 1),
            Money::new(3000000.0, test_currency()),
        ), // 3x return
    ];

    let engine = EquityWaterfallEngine::new(&spec);
    let ledger = engine.run(&events).unwrap();

    assert!(!ledger.rows.is_empty(), "Should have allocation rows");

    // Should have multiple promote tiers
    let promote_rows: Vec<_> = ledger
        .rows
        .iter()
        .filter(|r| r.tranche.contains("Promote"))
        .collect();

    // With a 3x return, should hit multiple promote tiers
    assert!(!promote_rows.is_empty(), "Should have promote allocations");
}

/// Golden test: serialize and deserialize waterfall spec.
#[test]
fn test_waterfall_spec_serde_stability() {
    let spec = WaterfallSpec::builder()
        .style(WaterfallStyle::European)
        .return_of_capital()
        .preferred_irr(0.08)
        .catchup(1.0)
        .promote_tier(0.0, 0.8, 0.2)
        .clawback(ClawbackSpec {
            enable: true,
            holdback_pct: Some(0.1),
            settle_on: ClawbackSettle::FundEnd,
        })
        .build()
        .unwrap();

    // Serialize to JSON
    let json = serde_json::to_string(&spec).unwrap();

    // Deserialize back
    let deserialized: WaterfallSpec = serde_json::from_str(&json).unwrap();

    // Should be identical
    assert_eq!(spec, deserialized);

    // Verify key fields are preserved
    assert_eq!(deserialized.style, WaterfallStyle::European);
    assert_eq!(deserialized.tranches.len(), 4);
    assert_eq!(deserialized.irr_basis, DayCount::Act365F);
    assert!(deserialized.clawback.is_some());
}

/// Test allocation ledger serde stability.
#[test]
fn test_allocation_ledger_serde_stability() {
    let (spec, events) = simple_2x_scenario();
    let engine = EquityWaterfallEngine::new(&spec);
    let ledger = engine.run(&events).unwrap();

    // Serialize to JSON
    let json = serde_json::to_string(&ledger).unwrap();

    // Deserialize back
    let deserialized: AllocationLedger = serde_json::from_str(&json).unwrap();

    // Should have same number of rows
    assert_eq!(ledger.rows.len(), deserialized.rows.len());

    // Check first row details match
    if !ledger.rows.is_empty() {
        let orig = &ledger.rows[0];
        let deser = &deserialized.rows[0];

        assert_eq!(orig.date, deser.date);
        assert_eq!(orig.tranche, deser.tranche);
        assert!((orig.to_lp.amount() - deser.to_lp.amount()).abs() < 1e-12);
        assert!((orig.to_gp.amount() - deser.to_gp.amount()).abs() < 1e-12);
    }
}
