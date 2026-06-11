//! Market standards and benchmark validation tests.
//!
//! Tests against known market conventions and expected behaviors.

use crate::deposit::common::*;
use finstack_core::currency::Currency;
use finstack_core::dates::{BusinessDayConvention, DayCount};
use finstack_core::money::Money;
use finstack_core::types::{CurveId, InstrumentId};
use finstack_valuations::instruments::rates::deposit::Deposit;
use finstack_valuations::metrics::MetricId;
use rust_decimal::Decimal;

#[test]
fn test_standard_usd_3m_deposit() {
    // Setup - typical USD 3-month deposit
    let base = date(2025, 1, 1);
    let ctx = ctx_with_flat_rate(base, "USD-OIS", 0.02);

    let dep = DepositBuilder::new(base)
        .notional(Money::new(10_000_000.0, Currency::USD))
        .start_date(base)
        .maturity(date(2025, 4, 1))
        .day_count(DayCount::Act360)
        .quote_rate(0.02)
        .discount_curve_id("USD-OIS")
        .build();

    // Execute - trade NPV (pricing view via `npv_raw`, includes the T+0
    // initial exchange); holder-view `value` excludes it and is ≈ +notional.
    let npv = dep.npv_raw(&ctx, base).unwrap();
    let par = compute_metric(&dep, &ctx, base, MetricId::DepositParRate);

    // Validate - at market rate, trade NPV should be near zero
    // For a $10mm deposit at par rate, trade NPV should be < $1000 (< 10bp of notional).
    // Note: We use 10bp tolerance because the test curve uses continuous compounding
    // while deposits use simple interest (ACT/360), creating a small basis.
    // In production, curves would be calibrated to reprice deposits exactly.
    assert!(
        npv.abs() < 1000.0,
        "trade NPV at market rate should be < $1000 (10bp), got: {npv}",
    );
    assert!((par - 0.02).abs() < 0.005, "Par rate: {}", par);
}

#[test]
fn test_standard_eur_6m_deposit() {
    // Setup - typical EUR 6-month deposit
    let base = date(2025, 1, 1);
    let ctx = ctx_with_flat_rate(base, "EUR-OIS", 0.015);

    let dep = DepositBuilder::new(base)
        .notional(Money::new(10_000_000.0, Currency::EUR))
        .start_date(base)
        .maturity(date(2025, 7, 1))
        .day_count(DayCount::Act360)
        .quote_rate(0.015)
        .discount_curve_id("EUR-OIS")
        .build();

    // Execute - trade NPV (pricing view via `npv_raw`); holder-view `value`
    // excludes the T+0 initial exchange and is ≈ +notional.
    let pv = dep.value(&ctx, base).unwrap();
    let npv = dep.npv_raw(&ctx, base).unwrap();

    // Validate: at market rate, trade NPV should be near zero
    // Small basis from continuous OIS curve vs simple ACT/360 deposit convention
    assert!(pv.currency() == Currency::EUR);
    assert!(
        npv.abs() < 5_000.0,
        "EUR 6M deposit at par rate trade NPV: {npv}",
    );
}

#[test]
fn test_overnight_deposit_libor_style() {
    // Setup - overnight deposit (O/N)
    let base = date(2025, 1, 1);
    let ctx = ctx_with_flat_rate(base, "USD-OIS", 0.05);

    let dep = DepositBuilder::new(base)
        .notional(Money::new(50_000_000.0, Currency::USD))
        .start_date(base)
        .maturity(date(2025, 1, 2))
        .day_count(DayCount::Act360)
        .quote_rate(0.05)
        .build();

    // Execute
    let pv = dep.value(&ctx, base).unwrap();
    let yf = compute_metric(&dep, &ctx, base, MetricId::Yf);

    // Validate
    assert!(yf < 0.003, "Year fraction too large: {}", yf); // ~1/360
    assert!(pv.amount().is_finite());
}

#[test]
fn test_deposit_rate_convention_act360() {
    // Setup - Act/360 is standard for USD money market
    let base = date(2025, 1, 1);
    let ctx = ctx_with_standard_disc(base, "USD-OIS");

    let dep = DepositBuilder::new(base)
        .start_date(base)
        .maturity(date(2025, 7, 1))
        .day_count(DayCount::Act360)
        .quote_rate(0.03)
        .build();

    // Execute
    let yf = compute_metric(&dep, &ctx, base, MetricId::Yf);

    // Validate - 6 months with Act/360 should be ~0.5
    assert!(yf > 0.48 && yf < 0.52, "YF: {}", yf);
}

#[test]
fn test_par_rate_relationship_to_discount_factors() {
    // Setup
    let base = date(2025, 1, 1);
    let ctx = ctx_with_standard_disc(base, "USD-OIS");

    let dep = DepositBuilder::new(base).maturity(date(2025, 7, 1)).build();

    // Execute
    let metrics = compute_metrics(
        &dep,
        &ctx,
        base,
        &[
            MetricId::DfStart,
            MetricId::DfEnd,
            MetricId::Yf,
            MetricId::DepositParRate,
        ],
    );

    let df_s = metrics[&MetricId::DfStart];
    let df_e = metrics[&MetricId::DfEnd];
    let yf = metrics[&MetricId::Yf];
    let par = metrics[&MetricId::DepositParRate];

    // Validate - par = (DF(s)/DF(e) - 1) / yf
    let expected_par = (df_s / df_e - 1.0) / yf;
    assert!((par - expected_par).abs() < RATE_TOLERANCE);
}

#[test]
fn test_dv01_magnitude_check() {
    // Setup - $10mm 6M deposit should have DV01 magnitude ~$500
    let base = date(2025, 1, 1);
    let ctx = ctx_with_standard_disc(base, "USD-OIS");

    let dep = DepositBuilder::new(base)
        .notional(Money::new(10_000_000.0, Currency::USD))
        .maturity(date(2025, 7, 1))
        .build();

    // Execute
    let dv01 = compute_metric(&dep, &ctx, base, MetricId::Dv01);

    // Validate - rough magnitude check based on market standards (about 0.5 yrs * 10M notional * 1bp = ~$500)
    // DV01 is negative for long positions (standard convention)
    assert!(dv01.abs() > 400.0 && dv01.abs() < 600.0, "DV01: {}", dv01);
}

#[test]
fn test_simple_interest_calculation() {
    // Setup - verify simple interest formula
    let base = date(2025, 1, 1);
    let ctx = ctx_with_standard_disc(base, "USD-OIS");
    let notional = 1_000_000.0;
    let rate = 0.05;

    let dep = DepositBuilder::new(base)
        .notional(Money::new(notional, Currency::USD))
        .start_date(base)
        .maturity(date(2025, 7, 1))
        .quote_rate(rate)
        .build();

    // Execute
    let yf = compute_metric(&dep, &ctx, base, MetricId::Yf);

    // Validate - interest = notional × rate × year_fraction
    let expected_interest = notional * rate * yf;
    assert!(expected_interest > 20_000.0 && expected_interest < 30_000.0);
}

#[test]
fn test_multi_currency_portfolio() {
    // Setup - test deposits in multiple currencies
    let base = date(2025, 1, 1);
    let ctx_usd = ctx_with_standard_disc(base, "USD-OIS");
    let ctx_eur = ctx_with_standard_disc(base, "EUR-OIS");
    let ctx_gbp = ctx_with_standard_disc(base, "GBP-OIS");

    let dep_usd = DepositBuilder::new(base)
        .notional(Money::new(1_000_000.0, Currency::USD))
        .maturity(date(2025, 7, 1))
        .quote_rate(0.03)
        .discount_curve_id("USD-OIS")
        .build();

    let dep_eur = DepositBuilder::new(base)
        .notional(Money::new(1_000_000.0, Currency::EUR))
        .maturity(date(2025, 7, 1))
        .quote_rate(0.02)
        .discount_curve_id("EUR-OIS")
        .build();

    let dep_gbp = DepositBuilder::new(base)
        .notional(Money::new(1_000_000.0, Currency::GBP))
        .maturity(date(2025, 7, 1))
        .quote_rate(0.025)
        .discount_curve_id("GBP-OIS")
        .build();

    // Execute
    let pv_usd = dep_usd.value(&ctx_usd, base).unwrap();
    let pv_eur = dep_eur.value(&ctx_eur, base).unwrap();
    let pv_gbp = dep_gbp.value(&ctx_gbp, base).unwrap();

    // Validate - each should price correctly in its own currency
    assert_eq!(pv_usd.currency(), Currency::USD);
    assert_eq!(pv_eur.currency(), Currency::EUR);
    assert_eq!(pv_gbp.currency(), Currency::GBP);
    assert!(pv_usd.amount().is_finite());
    assert!(pv_eur.amount().is_finite());
    assert!(pv_gbp.amount().is_finite());
}

#[test]
fn test_rate_quote_vs_price_quote() {
    // Setup - test that rate quoting works correctly
    let base = date(2025, 1, 1);
    let ctx = ctx_with_standard_disc(base, "USD-OIS");

    // Deposit quoted at rate
    let dep_rate = DepositBuilder::new(base)
        .maturity(date(2025, 7, 1))
        .quote_rate(0.035)
        .build();

    // Deposit at par rate
    let par = compute_metric(&dep_rate, &ctx, base, MetricId::DepositParRate);
    let dep_par = DepositBuilder::new(base)
        .maturity(date(2025, 7, 1))
        .quote_rate(par)
        .build();

    // Execute - trade NPVs (pricing view via `npv_raw`, includes the T+0
    // initial exchange); holder-view `value` is ≈ +notional for both.
    let npv_rate = dep_rate.npv_raw(&ctx, base).unwrap();
    let npv_par = dep_par.npv_raw(&ctx, base).unwrap();

    // Validate - par deposit should have essentially zero trade NPV
    // Market standard: < $0.01 on $1M notional (< 0.001bp numerical precision)
    assert!(
        npv_par.abs() < 0.01,
        "trade NPV at par rate should be < $0.01, got: {npv_par}",
    );
    // Off-market rate should have non-zero trade NPV
    assert!(npv_rate.abs() > 10.0);
}

/// Test T+2 spot lag with NYSE calendar.
///
/// Trade date: Friday (2025-01-03)
/// Expected spot date: Tuesday (2025-01-07) - skips Saturday and Sunday
/// Maturity: 1 month later (2025-02-07)
///
/// This validates that business-day aware spot lag correctly handles weekends
/// and that PV is computed using the adjusted effective dates.
#[test]
fn test_usd_deposit_friday_trade_with_nyse_calendar() {
    // Setup: Friday trade date
    // January 3, 2025 is a Friday
    let trade_date = date(2025, 1, 3);

    // Create market context - curve base starts at spot date (T+2 = Tuesday Jan 7)
    let expected_spot_date = date(2025, 1, 7); // Tuesday (after skipping weekend)
    let ctx = ctx_with_flat_rate(expected_spot_date, "USD-OIS", 0.02);

    // Build deposit with T+2 spot lag and NYSE calendar
    // End date set to ~1 month after expected spot
    let dep = Deposit::builder()
        .id(InstrumentId::new("DEP-USD-1M-FRIDAY"))
        .notional(Money::new(1_000_000.0, Currency::USD))
        .start_date(trade_date) // Will be adjusted by spot lag
        .maturity(date(2025, 2, 7)) // 1 month maturity from spot
        .day_count(DayCount::Act360)
        .quote_rate_opt(Some(Decimal::try_from(0.02).expect("valid decimal")))
        .discount_curve_id(CurveId::new("USD-OIS"))
        .attributes(finstack_valuations::instruments::Attributes::new())
        .spot_lag_days_opt(Some(2))
        .bdc(BusinessDayConvention::ModifiedFollowing)
        .calendar_id_opt(Some("nyse".into()))
        .build()
        .expect("Valid deposit");

    // Compute effective dates
    let effective_start = dep.effective_start_date().unwrap();
    let effective_end = dep.effective_end_date().unwrap();

    // Validate effective dates
    assert_eq!(
        effective_start, expected_spot_date,
        "Spot date should be Tuesday (T+2 business days from Friday)"
    );

    // End date should be Feb 7, 2025 (which is a Friday - business day)
    assert_eq!(effective_end, date(2025, 2, 7));

    // Execute - NPV and metrics should be computable with adjusted dates
    let pv = dep.value(&ctx, trade_date).unwrap();
    let metrics = compute_metrics(
        &dep,
        &ctx,
        trade_date,
        &[
            MetricId::DfStart,
            MetricId::DfEnd,
            MetricId::Yf,
            MetricId::DepositParRate,
        ],
    );
    let df_s = metrics[&MetricId::DfStart];
    let df_e = metrics[&MetricId::DfEnd];
    let yf = metrics[&MetricId::Yf];
    let par = metrics[&MetricId::DepositParRate];

    // Validate - at market rate, PV should be near zero
    assert!(
        pv.amount().abs() < 100.0,
        "PV at market rate should be near zero, got: {}",
        pv.amount()
    );
    assert_eq!(pv.currency(), Currency::USD);
    assert!((df_s - 1.0).abs() < 1e-12, "DF(start): {}", df_s);
    assert!(df_e > 0.0 && df_e < 1.0, "DF(end): {}", df_e);
    let expected_par = (df_s / df_e - 1.0) / yf;
    assert!(
        (par - expected_par).abs() < RATE_TOLERANCE,
        "Par rate: {}, expected: {}",
        par,
        expected_par
    );
}

/// Test that deposit without spot lag uses raw dates.
///
/// When spot_lag_days is not set, the raw start/end dates should be used
/// (optionally BDC-adjusted if calendar is set).
#[test]
fn test_deposit_without_spot_lag_uses_raw_dates() {
    let trade_date = date(2025, 1, 3); // Friday
    let ctx = ctx_with_flat_rate(trade_date, "USD-OIS", 0.02);

    // Build deposit WITHOUT spot_lag_days - should use raw start date
    let dep = Deposit::builder()
        .id(InstrumentId::new("DEP-USD-RAW"))
        .notional(Money::new(1_000_000.0, Currency::USD))
        .start_date(trade_date)
        .maturity(date(2025, 2, 3))
        .day_count(DayCount::Act360)
        .quote_rate_opt(Some(Decimal::try_from(0.02).expect("valid decimal")))
        .discount_curve_id(CurveId::new("USD-OIS"))
        .attributes(finstack_valuations::instruments::Attributes::new())
        // No spot_lag_days_opt - should use raw dates
        .build()
        .expect("Valid deposit");

    // Effective start should be raw start date since no spot lag specified
    let effective_start = dep.effective_start_date().unwrap();
    assert_eq!(
        effective_start, trade_date,
        "Without spot_lag, effective start should equal raw start"
    );

    // Execute - should price without error
    let pv = dep.value(&ctx, trade_date).unwrap();
    assert!(pv.amount().is_finite());
}

/// Test T+0 settlement for GBP (same-day settlement).
///
/// GBP deposits settle on trade date (T+0), unlike USD/EUR which are T+2.
#[test]
fn test_gbp_deposit_t0_settlement() {
    let trade_date = date(2025, 1, 3); // Friday
    let ctx = ctx_with_flat_rate(trade_date, "GBP-OIS", 0.02);

    // Build GBP deposit with T+0 spot lag
    let dep = Deposit::builder()
        .id(InstrumentId::new("DEP-GBP-1M"))
        .notional(Money::new(1_000_000.0, Currency::GBP))
        .start_date(trade_date)
        .maturity(date(2025, 2, 3))
        .day_count(DayCount::Act365F) // GBP uses Act/365
        .quote_rate_opt(Some(Decimal::try_from(0.02).expect("valid decimal")))
        .discount_curve_id(CurveId::new("GBP-OIS"))
        .attributes(finstack_valuations::instruments::Attributes::new())
        .spot_lag_days_opt(Some(0)) // T+0 for GBP
        .bdc(BusinessDayConvention::ModifiedFollowing)
        .build()
        .expect("Valid deposit");

    // Effective start should be trade date (T+0)
    let effective_start = dep.effective_start_date().unwrap();
    assert_eq!(
        effective_start, trade_date,
        "GBP T+0 settlement should start on trade date"
    );

    // Execute
    let pv = dep.value(&ctx, trade_date).unwrap();
    assert_eq!(pv.currency(), Currency::GBP);
    assert!(pv.amount().is_finite());
}

/// Test that business day adjustment properly handles end-of-month rolls.
///
/// ModifiedFollowing convention should roll forward unless it crosses month boundary,
/// in which case it rolls backward.
#[test]
fn test_modified_following_eom_adjustment() {
    // January 31, 2025 is a Friday - business day
    // If end date falls on Saturday, ModifiedFollowing should roll to Friday
    let trade_date = date(2025, 1, 27); // Monday

    // Create context with base at a valid business day (unused in this test but kept for consistency)
    let _ctx = ctx_with_flat_rate(trade_date, "USD-OIS", 0.02);

    // End date: Feb 1, 2025 is a Saturday - should roll to Friday Jan 31 (preceding in same month)
    // Actually, ModifiedFollowing rolls forward first, then back if crosses month.
    // Feb 1 is Saturday -> Following gives Monday Feb 3 which is in Feb (same month? No, different month from Jan 31)
    // Wait, the end date is Feb 1, so we're checking Feb. Feb 1 -> Feb 3 (Mon) is still in Feb.
    // Let's use a clearer example: end on Saturday March 1, 2025
    // March 1, 2025 is a Saturday, so Following would give Monday March 3 (still in March, OK)
    //
    // Better test: end date that would roll into next month
    // Use Saturday Jan 31, 2026 (Jan 31, 2026 is Saturday) - Following gives Monday Feb 2
    // But ModifiedFollowing should roll back to Friday Jan 30
    //
    // Actually Jan 31, 2026 is a Saturday.
    let dep = Deposit::builder()
        .id(InstrumentId::new("DEP-USD-EOM"))
        .notional(Money::new(1_000_000.0, Currency::USD))
        .start_date(trade_date)
        .maturity(date(2026, 1, 31)) // Saturday - should roll back to Friday Jan 30
        .day_count(DayCount::Act360)
        .quote_rate_opt(Some(Decimal::try_from(0.02).expect("valid decimal")))
        .discount_curve_id(CurveId::new("USD-OIS"))
        .attributes(finstack_valuations::instruments::Attributes::new())
        .bdc(BusinessDayConvention::ModifiedFollowing)
        .calendar_id_opt(Some("nyse".into()))
        .build()
        .expect("Valid deposit");

    let effective_end = dep.effective_end_date().unwrap();

    // Jan 31, 2026 is Saturday; ModifiedFollowing: Following gives Mon Feb 2 (different month),
    // so we roll Preceding to Fri Jan 30
    assert_eq!(
        effective_end,
        date(2026, 1, 30),
        "ModifiedFollowing should roll Saturday Jan 31 to Friday Jan 30"
    );
}
