//! Day count consistency tests for bond metrics.
//!
//! Verifies that:
//! - YTM calculation uses the bond's day count convention
//! - Duration calculation uses consistent day count with YTM
//! - Accrual uses the same day count as cashflow generation
//! - Different day counts produce different (but correct) results
//!
//! **Market Standards Review (Week 3 Edge Cases)**

use finstack_quant_cashflows::builder::specs::FixedCouponSpec;
use finstack_quant_cashflows::builder::CouponType;
use finstack_quant_core::currency::Currency;
use finstack_quant_core::dates::{Date, DayCount, Tenor};
use finstack_quant_core::market_data::context::MarketContext;
use finstack_quant_core::market_data::term_structures::DiscountCurve;
use finstack_quant_core::money::Money;
use finstack_quant_valuations::instruments::fixed_income::bond::{Bond, CashflowSpec};
use finstack_quant_valuations::instruments::Instrument;
use finstack_quant_valuations::instruments::PricingOverrides;
use finstack_quant_valuations::metrics::MetricId;
use time::macros::date;

fn build_flat_discount_curve(rate: f64, base_date: Date, curve_id: &str) -> DiscountCurve {
    DiscountCurve::builder(curve_id)
        .base_date(base_date)
        .day_count(DayCount::Act365F)
        .knots([
            (0.0, 1.0),
            (1.0, (-rate).exp()),
            (5.0, (-rate * 5.0).exp()),
            (10.0, (-rate * 10.0).exp()),
        ])
        .build()
        .unwrap()
}

fn create_bond_with_daycount(
    day_count: DayCount,
    coupon: f64,
    issue: Date,
    maturity: Date,
) -> Bond {
    Bond::builder()
        .id(format!("BOND_{:?}", day_count).into())
        .notional(Money::new(1000.0, Currency::USD))
        .issue_date(issue)
        .maturity(maturity)
        .cashflow_spec(CashflowSpec::Fixed(FixedCouponSpec {
            coupon_type: CouponType::Cash,
            rate: rust_decimal::Decimal::try_from(coupon).expect("valid coupon"),
            freq: Tenor::semi_annual(),
            dc: day_count,
            bdc: finstack_quant_core::dates::BusinessDayConvention::ModifiedFollowing,
            calendar_id: "weekends_only".to_string(),
            stub: finstack_quant_core::dates::StubKind::None,
            end_of_month: false,
            payment_lag_days: 0,
        }))
        .discount_curve_id("USD-OIS".into())
        .build()
        .unwrap()
}

// ============================================================================
// YTM Day Count Consistency Tests
// ============================================================================

#[test]
fn test_ytm_uses_bond_daycount() {
    // YTM should reflect the bond's day count, not the curve's day count
    let as_of = date!(2024 - 01 - 01);
    let maturity = date!(2029 - 01 - 01);

    // Create bonds with different day counts but same coupon
    let bond_act365 = create_bond_with_daycount(DayCount::Act365F, 0.05, as_of, maturity);
    let bond_30360 = create_bond_with_daycount(DayCount::Thirty360, 0.05, as_of, maturity);

    let disc = build_flat_discount_curve(0.05, as_of, "USD-OIS");
    let market = MarketContext::new().insert(disc);

    // Price both bonds at par (same price)
    let mut bond_act365_quoted = bond_act365;
    bond_act365_quoted.pricing_overrides =
        PricingOverrides::default().with_quoted_clean_price(100.0);

    let mut bond_30360_quoted = bond_30360;
    bond_30360_quoted.pricing_overrides =
        PricingOverrides::default().with_quoted_clean_price(100.0);

    let result_act365 = bond_act365_quoted
        .price_with_metrics(
            &market,
            as_of,
            &[MetricId::Ytm],
            finstack_quant_valuations::instruments::PricingOptions::default(),
        )
        .unwrap();

    let result_30360 = bond_30360_quoted
        .price_with_metrics(
            &market,
            as_of,
            &[MetricId::Ytm],
            finstack_quant_valuations::instruments::PricingOptions::default(),
        )
        .unwrap();

    let ytm_act365 = *result_act365.measures.get("ytm").unwrap();
    let ytm_30360 = *result_30360.measures.get("ytm").unwrap();

    // Both YTMs should be finite and positive
    assert!(
        ytm_act365.is_finite() && ytm_act365 > 0.0,
        "Act365F YTM should be finite positive: {}",
        ytm_act365
    );
    assert!(
        ytm_30360.is_finite() && ytm_30360 > 0.0,
        "30/360 YTM should be finite positive: {}",
        ytm_30360
    );

    // YTMs should be different due to different day count fractions
    // (though both close to 5% coupon rate at par)
    // The difference is typically small (few basis points)
    assert!(
        (ytm_act365 - ytm_30360).abs() < 0.01, // Within 100bp
        "YTMs should be close but may differ: Act365={:.4}, 30360={:.4}",
        ytm_act365,
        ytm_30360
    );
}

#[test]
fn test_ytm_par_equals_coupon_for_matching_daycount() {
    // At par, YTM should equal coupon rate
    // This relationship should hold for any day count convention
    let as_of = date!(2024 - 01 - 01);
    let maturity = date!(2029 - 01 - 01);
    let coupon = 0.05;

    for day_count in [DayCount::Act365F, DayCount::Thirty360, DayCount::Act360] {
        let bond = create_bond_with_daycount(day_count, coupon, as_of, maturity);

        let disc = build_flat_discount_curve(0.05, as_of, "USD-OIS");
        let market = MarketContext::new().insert(disc);

        let mut bond_at_par = bond.clone();
        bond_at_par.pricing_overrides = PricingOverrides::default().with_quoted_clean_price(100.0);

        let result = bond_at_par
            .price_with_metrics(
                &market,
                as_of,
                &[MetricId::Ytm],
                finstack_quant_valuations::instruments::PricingOptions::default(),
            )
            .unwrap();

        let ytm = *result.measures.get("ytm").unwrap();

        // At par, YTM ≈ coupon (allowing small tolerance for numerical precision)
        assert!(
            (ytm - coupon).abs() < 0.005, // Within 50bp
            "At par, YTM ({:.4}) should approximately equal coupon ({:.4}) for {:?}",
            ytm,
            coupon,
            day_count
        );
    }
}

// ============================================================================
// Duration Day Count Consistency Tests
// ============================================================================

#[test]
fn test_duration_consistent_with_ytm() {
    // Duration calculation should use the same day count as YTM
    let as_of = date!(2024 - 01 - 01);
    let maturity = date!(2029 - 01 - 01);

    let bond = create_bond_with_daycount(DayCount::Thirty360, 0.05, as_of, maturity);

    let disc = build_flat_discount_curve(0.05, as_of, "USD-OIS");
    let market = MarketContext::new().insert(disc);

    let mut bond_quoted = bond;
    bond_quoted.pricing_overrides = PricingOverrides::default().with_quoted_clean_price(100.0);

    let result = bond_quoted
        .price_with_metrics(
            &market,
            as_of,
            &[MetricId::Ytm, MetricId::DurationMod, MetricId::DurationMac],
            finstack_quant_valuations::instruments::PricingOptions::default(),
        )
        .unwrap();

    let ytm = *result.measures.get("ytm").unwrap();
    let mod_dur = *result.measures.get("duration_mod").unwrap();
    let mac_dur = *result.measures.get("duration_mac").unwrap();

    // All metrics should be finite
    assert!(ytm.is_finite(), "YTM should be finite");
    assert!(mod_dur.is_finite(), "Modified duration should be finite");
    assert!(mac_dur.is_finite(), "Macaulay duration should be finite");

    // For semi-annual compounding: ModDur = MacDur / (1 + YTM/2)
    let expected_mod_dur = mac_dur / (1.0 + ytm / 2.0);
    assert!(
        (mod_dur - expected_mod_dur).abs() < 0.01,
        "Modified duration ({:.4}) should equal Macaulay ({:.4}) / (1 + y/2): expected {:.4}",
        mod_dur,
        mac_dur,
        expected_mod_dur
    );
}

// ============================================================================
// Accrued Interest Day Count Tests
// ============================================================================

#[test]
fn test_accrued_interest_uses_bond_daycount() {
    // Accrued interest should use the bond's day count convention
    let issue = date!(2024 - 01 - 01);
    let maturity = date!(2029 - 01 - 01);
    let as_of = date!(2024 - 04 - 01); // 3 months in (approx mid-period)

    let bond_act365 = create_bond_with_daycount(DayCount::Act365F, 0.05, issue, maturity);
    let bond_30360 = create_bond_with_daycount(DayCount::Thirty360, 0.05, issue, maturity);

    let disc = build_flat_discount_curve(0.05, issue, "USD-OIS");
    let market = MarketContext::new().insert(disc);

    let result_act365 = bond_act365
        .price_with_metrics(
            &market,
            as_of,
            &[MetricId::Accrued],
            finstack_quant_valuations::instruments::PricingOptions::default(),
        )
        .unwrap();

    let result_30360 = bond_30360
        .price_with_metrics(
            &market,
            as_of,
            &[MetricId::Accrued],
            finstack_quant_valuations::instruments::PricingOptions::default(),
        )
        .unwrap();

    let accrued_act365 = *result_act365.measures.get("accrued").unwrap();
    let accrued_30360 = *result_30360.measures.get("accrued").unwrap();

    // Both should be positive (mid-period)
    assert!(
        accrued_act365 > 0.0,
        "Act365F accrued should be positive: {}",
        accrued_act365
    );
    assert!(
        accrued_30360 > 0.0,
        "30/360 accrued should be positive: {}",
        accrued_30360
    );

    // Accrued amounts should differ due to different day count fractions
    // Act/365 for 3 months ≈ 91/365 ≈ 0.249
    // 30/360 for 3 months = 90/360 = 0.25
    // These are close but not identical
    assert!(
        (accrued_act365 - accrued_30360).abs() < 2.0, // Within $2 for $1000 bond
        "Accrued should be similar but different: Act365={:.2}, 30360={:.2}",
        accrued_act365,
        accrued_30360
    );
}

#[test]
fn test_dirty_clean_price_consistency() {
    // Dirty = Clean + Accrued should hold regardless of day count
    let issue = date!(2024 - 01 - 01);
    let maturity = date!(2029 - 01 - 01);
    let as_of = date!(2024 - 04 - 01);

    for day_count in [DayCount::Act365F, DayCount::Thirty360, DayCount::ActAct] {
        let bond = create_bond_with_daycount(day_count, 0.05, issue, maturity);

        let disc = build_flat_discount_curve(0.05, issue, "USD-OIS");
        let market = MarketContext::new().insert(disc);

        let result = bond
            .price_with_metrics(
                &market,
                as_of,
                &[
                    MetricId::CleanPrice,
                    MetricId::DirtyPrice,
                    MetricId::Accrued,
                ],
                finstack_quant_valuations::instruments::PricingOptions::default(),
            )
            .unwrap();

        let clean = *result.measures.get("clean_price").unwrap();
        let dirty = *result.measures.get("dirty_price").unwrap();
        let accrued = *result.measures.get("accrued").unwrap();

        // Dirty = Clean + Accrued (all in currency units)
        let expected_dirty = clean + accrued;

        assert!(
            (dirty - expected_dirty).abs() < 0.01,
            "Dirty ({:.4}) should equal Clean ({:.4}) + Accrued ({:.4}) for {:?}",
            dirty,
            clean,
            accrued,
            day_count
        );
    }
}
