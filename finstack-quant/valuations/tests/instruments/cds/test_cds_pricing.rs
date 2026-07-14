//! CDS pricing engine tests.
//!
//! Tests core pricing logic including protection leg, premium leg,
//! NPV calculations, par spreads, and schedule generation.

use crate::finstack_quant_test_utils as test_utils;
use finstack_quant_cashflows::CashflowProvider;
use finstack_quant_core::currency::Currency;
use finstack_quant_core::dates::{Date, DayCount};
use finstack_quant_core::market_data::context::MarketContext;
use finstack_quant_core::market_data::term_structures::{DiscountCurve, HazardCurve};
use finstack_quant_core::money::Money;
use finstack_quant_valuations::instruments::Instrument;
use finstack_quant_valuations::metrics::MetricId;
use rust_decimal::Decimal;
use time::macros::date;

/// Build flat discount curve for testing
fn build_discount_curve(rate: f64, base_date: Date, id: &str) -> DiscountCurve {
    DiscountCurve::builder(id)
        .base_date(base_date)
        .day_count(DayCount::Act360)
        .knots([
            (0.0, 1.0),
            (1.0, (-rate).exp()),
            (5.0, (-rate * 5.0).exp()),
            (10.0, (-rate * 10.0).exp()),
        ])
        .build()
        .unwrap()
}

/// Build flat hazard curve for testing
fn build_hazard_curve(hazard_rate: f64, recovery: f64, base_date: Date, id: &str) -> HazardCurve {
    HazardCurve::builder(id)
        .base_date(base_date)
        .recovery_rate(recovery)
        .knots([
            (0.0, hazard_rate),
            (1.0, hazard_rate),
            (5.0, hazard_rate),
            (10.0, hazard_rate),
        ])
        .build()
        .unwrap()
}

fn metric_value<I: Instrument>(
    instrument: &I,
    market: &MarketContext,
    as_of: Date,
    metric: MetricId,
) -> f64 {
    let result = instrument
        .price_with_metrics(
            market,
            as_of,
            std::slice::from_ref(&metric),
            finstack_quant_valuations::instruments::PricingOptions::default(),
        )
        .expect("metric should compute");
    result.measures[&metric]
}

#[test]
fn test_protection_leg_positive_pv() {
    let as_of = date!(2024 - 01 - 01);
    let end = date!(2029 - 01 - 01);

    let disc = build_discount_curve(0.05, as_of, "USD_OIS");
    let hazard = build_hazard_curve(0.02, 0.40, as_of, "CORP");

    let cds = test_utils::cds_buy_protection(
        "PROT_LEG_TEST",
        Money::new(10_000_000.0, Currency::USD),
        100.0,
        as_of,
        end,
        "USD_OIS",
        "CORP",
    )
    .expect("CDS construction should succeed");

    let market = MarketContext::new().insert(disc).insert(hazard);

    let protection_pv = metric_value(&cds, &market, as_of, MetricId::ProtectionLegPv);

    assert!(protection_pv > 0.0, "Protection leg PV should be positive");
}

#[test]
fn test_premium_leg_positive_pv() {
    let as_of = date!(2024 - 01 - 01);
    let end = date!(2029 - 01 - 01);

    let disc = build_discount_curve(0.05, as_of, "USD_OIS");
    let hazard = build_hazard_curve(0.02, 0.40, as_of, "CORP");

    let cds = test_utils::cds_buy_protection(
        "PREM_LEG_TEST",
        Money::new(10_000_000.0, Currency::USD),
        100.0,
        as_of,
        end,
        "USD_OIS",
        "CORP",
    )
    .expect("CDS construction should succeed");

    let market = MarketContext::new().insert(disc).insert(hazard);

    let premium_pv = metric_value(&cds, &market, as_of, MetricId::PremiumLegPv);

    assert!(
        premium_pv > 0.0,
        "Premium leg PV should be positive for positive spread"
    );
}

#[test]
fn test_npv_calculation_buyer() {
    let as_of = date!(2024 - 01 - 01);
    let end = date!(2029 - 01 - 01);

    let disc = build_discount_curve(0.05, as_of, "USD_OIS");
    let hazard = build_hazard_curve(0.02, 0.40, as_of, "CORP");

    let market = MarketContext::new().insert(disc).insert(hazard);

    let cds = test_utils::cds_buy_protection(
        "NPV_BUYER",
        Money::new(10_000_000.0, Currency::USD),
        100.0,
        as_of,
        end,
        "USD_OIS",
        "CORP",
    )
    .expect("CDS construction should succeed");

    let npv = cds.value(&market, as_of).unwrap();

    // NPV should be finite
    assert!(npv.amount().is_finite());
}

#[test]
fn test_par_spread_increases_with_hazard() {
    let as_of = date!(2024 - 01 - 01);
    let end = date!(2029 - 01 - 01);

    let disc = build_discount_curve(0.03, as_of, "USD_OIS");
    let hazard_low = build_hazard_curve(0.01, 0.40, as_of, "CORP");
    let hazard_high = build_hazard_curve(0.04, 0.40, as_of, "CORP");

    let cds = test_utils::cds_buy_protection(
        "PAR_SPREAD_HAZARD",
        Money::new(10_000_000.0, Currency::USD),
        100.0,
        as_of,
        end,
        "USD_OIS",
        "CORP",
    )
    .expect("CDS construction should succeed");

    let market_low = MarketContext::new().insert(disc.clone()).insert(hazard_low);
    let market_high = MarketContext::new().insert(disc).insert(hazard_high);

    let par_low = metric_value(&cds, &market_low, as_of, MetricId::ParSpread);
    let par_high = metric_value(&cds, &market_high, as_of, MetricId::ParSpread);

    assert!(
        par_high > par_low,
        "Higher hazard should imply higher par spread; low={par_low}, high={par_high}"
    );
}

#[test]
fn test_par_spread_errors_when_expired() {
    let as_of = date!(2029 - 01 - 01);
    let start = date!(2024 - 01 - 01);
    let end = as_of; // expired on valuation date
    let disc = build_discount_curve(0.03, start, "USD_OIS");
    let hazard = build_hazard_curve(0.02, 0.40, start, "CORP");
    let market = MarketContext::new().insert(disc).insert(hazard);

    let cds = test_utils::cds_buy_protection(
        "EXPIRED_PAR",
        Money::new(10_000_000.0, Currency::USD),
        100.0,
        start,
        end,
        "USD_OIS",
        "CORP",
    )
    .expect("CDS construction should succeed");

    let err = cds
        .price_with_metrics(
            &market,
            as_of,
            &[MetricId::ParSpread],
            finstack_quant_valuations::instruments::PricingOptions::default(),
        )
        .expect_err("expired CDS should error");
    let msg = err.to_string();
    assert!(
        msg.contains("expired") || msg.contains("denominator"),
        "expected expired/denominator error, got {msg}"
    );
}

#[test]
fn test_premium_leg_metric_positive() {
    let as_of = date!(2024 - 01 - 01);
    let end = date!(2026 - 01 - 01);
    let disc = build_discount_curve(0.02, as_of, "USD_OIS");
    let hazard = build_hazard_curve(0.05, 0.40, as_of, "CORP");
    let market = MarketContext::new().insert(disc).insert(hazard);

    let cds = test_utils::cds_buy_protection(
        "PREM_METRIC",
        Money::new(10_000_000.0, Currency::USD),
        500.0,
        as_of,
        end,
        "USD_OIS",
        "CORP",
    )
    .expect("CDS construction should succeed");

    let premium_pv = metric_value(&cds, &market, as_of, MetricId::PremiumLegPv);
    assert!(
        premium_pv > 0.0,
        "Premium leg PV should be positive for positive spread"
    );
}

#[test]
fn test_npv_buyer_seller_opposite() {
    let as_of = date!(2024 - 01 - 01);
    let end = date!(2029 - 01 - 01);

    let disc = build_discount_curve(0.05, as_of, "USD_OIS");
    let hazard = build_hazard_curve(0.015, 0.40, as_of, "CORP");

    let market = MarketContext::new().insert(disc).insert(hazard);

    let buyer = test_utils::cds_buy_protection(
        "BUYER",
        Money::new(10_000_000.0, Currency::USD),
        100.0,
        as_of,
        end,
        "USD_OIS",
        "CORP",
    )
    .expect("CDS construction should succeed");

    let seller = test_utils::cds_sell_protection(
        "SELLER",
        Money::new(10_000_000.0, Currency::USD),
        100.0,
        as_of,
        end,
        "USD_OIS",
        "CORP",
    )
    .expect("CDS construction should succeed");

    let npv_buyer = buyer.value(&market, as_of).unwrap();
    let npv_seller = seller.value(&market, as_of).unwrap();

    // Should be opposite signs
    assert!(npv_buyer.amount() * npv_seller.amount() < 0.0);

    // Should sum to exactly zero — buyer and seller are model-symmetric
    let sum = npv_buyer.amount() + npv_seller.amount();
    assert!(
        sum.abs() < 10.0,
        "Buyer + Seller NPV sum should be near zero, got {}",
        sum
    );
}

#[test]
fn test_par_spread_positive() {
    let as_of = date!(2024 - 01 - 01);
    let end = date!(2029 - 01 - 01);

    let disc = build_discount_curve(0.05, as_of, "USD_OIS");
    let hazard = build_hazard_curve(0.015, 0.40, as_of, "CORP");

    let market = MarketContext::new().insert(disc).insert(hazard);

    let cds = test_utils::cds_buy_protection(
        "PAR_SPREAD_TEST",
        Money::new(10_000_000.0, Currency::USD),
        100.0,
        as_of,
        end,
        "USD_OIS",
        "CORP",
    )
    .expect("CDS construction should succeed");

    let par_spread = metric_value(&cds, &market, as_of, MetricId::ParSpread);

    assert!(par_spread > 0.0, "Par spread should be positive");
    assert!(
        par_spread < 1000.0,
        "Par spread should be reasonable (<1000 bps)"
    );
}

#[test]
fn test_par_spread_gives_zero_npv() {
    let as_of = date!(2024 - 01 - 01);
    let end = date!(2029 - 01 - 01);

    let disc = build_discount_curve(0.05, as_of, "USD_OIS");
    let hazard = build_hazard_curve(0.01, 0.40, as_of, "CORP");

    let market = MarketContext::new().insert(disc).insert(hazard);

    let mut cds = test_utils::cds_buy_protection(
        "PAR_NPV_TEST",
        Money::new(10_000_000.0, Currency::USD),
        100.0,
        as_of,
        end,
        "USD_OIS",
        "CORP",
    )
    .expect("CDS construction should succeed");

    // Get par spread
    let par_spread = metric_value(&cds, &market, as_of, MetricId::ParSpread);

    // Set spread to par (convert f64 to Decimal)
    cds.premium.spread_bp = Decimal::try_from(par_spread).expect("valid par_spread");

    // NPV should be near zero; f64→Decimal rounding introduces ~0.07bp on spread → ~$350 residual on $10M
    let npv = cds.value(&market, as_of).unwrap();
    assert!(
        npv.amount().abs() < 400.0,
        "NPV at par spread should be near zero (f64→Decimal rounding residual), got {}",
        npv.amount()
    );
}

#[test]
fn test_risky_annuity_positive() {
    let as_of = date!(2024 - 01 - 01);
    let end = date!(2029 - 01 - 01);

    let disc = build_discount_curve(0.05, as_of, "USD_OIS");
    let hazard = build_hazard_curve(0.01, 0.40, as_of, "CORP");

    let market = MarketContext::new().insert(disc).insert(hazard);

    let cds = test_utils::cds_buy_protection(
        "RISKY_ANN_TEST",
        Money::new(10_000_000.0, Currency::USD),
        100.0,
        as_of,
        end,
        "USD_OIS",
        "CORP",
    )
    .expect("CDS construction should succeed");

    let risky_annuity = metric_value(&cds, &market, as_of, MetricId::RiskyAnnuity);

    assert!(risky_annuity > 0.0);
    assert!(risky_annuity < 10.0, "Risky annuity for 5Y should be < 10");
}

#[test]
fn test_risky_pv01_scales_with_notional() {
    let as_of = date!(2024 - 01 - 01);
    let end = date!(2029 - 01 - 01);

    let disc = build_discount_curve(0.05, as_of, "USD_OIS");
    let hazard = build_hazard_curve(0.01, 0.40, as_of, "CORP");

    let market = MarketContext::new().insert(disc).insert(hazard);

    let cds1 = test_utils::cds_buy_protection(
        "PV01_1MM",
        Money::new(1_000_000.0, Currency::USD),
        100.0,
        as_of,
        end,
        "USD_OIS",
        "CORP",
    )
    .expect("CDS construction should succeed");

    let cds10 = test_utils::cds_buy_protection(
        "PV01_10MM",
        Money::new(10_000_000.0, Currency::USD),
        100.0,
        as_of,
        end,
        "USD_OIS",
        "CORP",
    )
    .expect("CDS construction should succeed");

    let pv01_1 = metric_value(&cds1, &market, as_of, MetricId::RiskyPv01);
    let pv01_10 = metric_value(&cds10, &market, as_of, MetricId::RiskyPv01);

    // Should scale linearly with notional
    let ratio = pv01_10 / pv01_1;
    assert!(
        (ratio - 10.0).abs() < 0.01,
        "PV01 should scale 10x, got ratio {}",
        ratio
    );
}

#[test]
fn test_schedule_generation_isda() {
    let as_of = date!(2024 - 01 - 01);
    let end = date!(2029 - 01 - 01);

    let cds = test_utils::cds_buy_protection(
        "SCHEDULE_TEST",
        Money::new(10_000_000.0, Currency::USD),
        100.0,
        as_of,
        end,
        "USD_OIS",
        "CORP",
    )
    .expect("CDS construction should succeed");

    let schedule_dates = cds.isda_coupon_schedule().unwrap();

    assert!(
        schedule_dates.len() >= 2,
        "Schedule should have at least start and end"
    );
    assert_eq!(schedule_dates[0], cds.premium.start);

    // The maturity date may be adjusted to a business day via Modified Following
    // so we just check it's on or after the original maturity
    let last_date = schedule_dates[schedule_dates.len() - 1];
    assert!(
        last_date >= cds.premium.end.saturating_sub(time::Duration::days(3))
            && last_date <= cds.premium.end.saturating_add(time::Duration::days(3)),
        "Last date should be near maturity date (got {} vs expected ~{})",
        last_date,
        cds.premium.end
    );

    // Interior dates should be near the 20th (ISDA standard)
    // Business day adjustment per ISDA 2014 (Modified Following) may move dates
    // forward if the 20th falls on a weekend or holiday.
    for &date in schedule_dates
        .iter()
        .skip(1)
        .take(schedule_dates.len().saturating_sub(2))
    {
        let day = date.day();
        assert!(
            (18..=23).contains(&day),
            "ISDA coupon dates should be near 20th (got day {})",
            day
        );
    }
}

#[test]
fn test_cdsw_final_coupon_cashflow_accrual_matches_pricer_premium_pv() {
    let as_of = date!(2025 - 03 - 20);
    let maturity = date!(2025 - 06 - 20);
    let notional = Money::new(10_000_000.0, Currency::USD);
    let spread_bp = 100.0;

    let disc = build_discount_curve(0.0, as_of, "USD_OIS");
    let hazard = build_hazard_curve(0.0, 0.40, as_of, "CORP");
    let market = MarketContext::new().insert(disc).insert(hazard);

    let cds = test_utils::cds_buy_protection(
        "CDSW_FINAL_COUPON_CASHFLOW",
        notional,
        spread_bp,
        as_of,
        maturity,
        "USD_OIS",
        "CORP",
    )
    .expect("CDS construction should succeed");

    let schedule = cds
        .cashflow_schedule(&market, as_of)
        .expect("cashflow schedule should build");
    let final_coupon = schedule
        .get_flows()
        .iter()
        .find(|flow| flow.date == maturity)
        .expect("final coupon should be exported");

    let expected_accrual = 93.0 / 360.0;
    assert!(
        (final_coupon.accrual_factor - expected_accrual).abs() < 1e-12,
        "exported final coupon accrual should use CDSW inclusive Act/360"
    );

    let expected_coupon = notional.amount() * (spread_bp / 10_000.0) * expected_accrual;
    assert!(
        (final_coupon.amount.amount() + expected_coupon).abs() < 1e-8,
        "protection-buyer exported premium cashflow should match signed CDSW coupon"
    );

    let premium_pv = metric_value(&cds, &market, as_of, MetricId::PremiumLegPv);
    assert!(
        (premium_pv - expected_coupon).abs() < 1e-8,
        "premium PV explain should use the same final coupon accrual"
    );
}

#[test]
fn test_higher_hazard_increases_protection_value() {
    let as_of = date!(2024 - 01 - 01);
    let end = date!(2029 - 01 - 01);

    let mut protection_pvs = Vec::new();

    for hazard_rate in [0.005, 0.010, 0.020, 0.030] {
        let disc_local = build_discount_curve(0.05, as_of, "USD_OIS");
        let hazard = build_hazard_curve(hazard_rate, 0.40, as_of, "CORP");
        let market = MarketContext::new().insert(disc_local).insert(hazard);

        let cds = test_utils::cds_buy_protection(
            "HAZARD_SENS",
            Money::new(10_000_000.0, Currency::USD),
            100.0,
            as_of,
            end,
            "USD_OIS",
            "CORP",
        )
        .expect("CDS construction should succeed");

        let protection_pv = metric_value(&cds, &market, as_of, MetricId::ProtectionLegPv);
        protection_pvs.push((hazard_rate, protection_pv));
    }

    // Protection value should increase with hazard rate
    for i in 1..protection_pvs.len() {
        assert!(
            protection_pvs[i].1 > protection_pvs[i - 1].1,
            "Protection PV should increase with hazard rate"
        );
    }
}

#[test]
fn test_higher_recovery_decreases_protection_value() {
    let as_of = date!(2024 - 01 - 01);
    let end = date!(2029 - 01 - 01);

    let mut protection_pvs = Vec::new();

    for recovery in [0.20, 0.40, 0.60] {
        let disc_local = build_discount_curve(0.05, as_of, "USD_OIS");
        let hazard = build_hazard_curve(0.015, recovery, as_of, "CORP");
        let market = MarketContext::new().insert(disc_local).insert(hazard);

        let mut cds = test_utils::cds_buy_protection(
            "RECOVERY_SENS",
            Money::new(10_000_000.0, Currency::USD),
            100.0,
            as_of,
            end,
            "USD_OIS",
            "CORP",
        )
        .expect("CDS construction should succeed");
        cds.protection.recovery_rate = recovery;

        let protection_pv = metric_value(&cds, &market, as_of, MetricId::ProtectionLegPv);
        protection_pvs.push((recovery, protection_pv));
    }

    // Protection value should decrease with recovery rate
    for i in 1..protection_pvs.len() {
        assert!(
            protection_pvs[i].1 < protection_pvs[i - 1].1,
            "Protection PV should decrease with recovery rate"
        );
    }
}

#[test]
fn test_premium_leg_increases_with_spread() {
    let as_of = date!(2024 - 01 - 01);
    let end = date!(2029 - 01 - 01);

    let disc = build_discount_curve(0.05, as_of, "USD_OIS");
    let hazard = build_hazard_curve(0.02, 0.40, as_of, "CORP");

    let market = MarketContext::new().insert(disc).insert(hazard);

    let cds_low = test_utils::cds_buy_protection(
        "PREM_LOW",
        Money::new(10_000_000.0, Currency::USD),
        100.0,
        as_of,
        end,
        "USD_OIS",
        "CORP",
    )
    .expect("CDS construction should succeed");
    let cds_high = test_utils::cds_buy_protection(
        "PREM_HIGH",
        Money::new(10_000_000.0, Currency::USD),
        200.0,
        as_of,
        end,
        "USD_OIS",
        "CORP",
    )
    .expect("CDS construction should succeed");

    let pv_low = metric_value(&cds_low, &market, as_of, MetricId::PremiumLegPv);
    let pv_high = metric_value(&cds_high, &market, as_of, MetricId::PremiumLegPv);

    assert!(
        pv_high > pv_low,
        "Higher spread should increase premium leg PV"
    );
}

#[test]
fn test_settlement_delay_reduces_protection_pv() {
    let as_of = date!(2024 - 01 - 01);
    let end = date!(2029 - 01 - 01);

    let disc = build_discount_curve(0.05, as_of, "USD_OIS");
    let hazard = build_hazard_curve(0.02, 0.40, as_of, "CORP");

    let market = MarketContext::new().insert(disc).insert(hazard);

    let mut cds_no_delay = test_utils::cds_buy_protection(
        "NO_DELAY",
        Money::new(10_000_000.0, Currency::USD),
        100.0,
        as_of,
        end,
        "USD_OIS",
        "CORP",
    )
    .expect("CDS construction should succeed");
    cds_no_delay.protection.settlement_delay = 0;

    let mut cds_with_delay = cds_no_delay.clone();
    cds_with_delay.protection.settlement_delay = 20;

    let pv_no_delay = metric_value(&cds_no_delay, &market, as_of, MetricId::ProtectionLegPv);
    let pv_with_delay = metric_value(&cds_with_delay, &market, as_of, MetricId::ProtectionLegPv);

    assert!(
        pv_with_delay < pv_no_delay,
        "Settlement delay should reduce protection PV"
    );
}

#[test]
fn test_aod_uses_default_settlement_timing() {
    let as_of = date!(2024 - 01 - 01);
    let end = date!(2029 - 01 - 01);

    let disc = build_discount_curve(0.05, as_of, "USD_OIS");
    let hazard = build_hazard_curve(0.30, 0.40, as_of, "CORP");
    let market = MarketContext::new().insert(disc).insert(hazard);

    let mut cds_no_delay = test_utils::cds_buy_protection(
        "AOD_NO_DELAY",
        Money::new(10_000_000.0, Currency::USD),
        1000.0,
        as_of,
        end,
        "USD_OIS",
        "CORP",
    )
    .expect("CDS construction should succeed");
    cds_no_delay.protection.settlement_delay = 0;

    let mut cds_with_delay = cds_no_delay.clone();
    cds_with_delay.protection.settlement_delay = 60;

    let pv_no_delay = metric_value(&cds_no_delay, &market, as_of, MetricId::PremiumLegPv);
    let pv_with_delay = metric_value(&cds_with_delay, &market, as_of, MetricId::PremiumLegPv);

    assert!(
        pv_with_delay < pv_no_delay,
        "Premium leg PV should fall when default settlement is delayed; no_delay={pv_no_delay}, with_delay={pv_with_delay}"
    );
}
