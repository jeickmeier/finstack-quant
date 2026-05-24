//! StructuredCredit BucketedDv01 smoke tests

use finstack_core::currency::Currency;
use finstack_core::dates::Date;
use finstack_core::market_data::context::MarketContext;
use finstack_core::market_data::term_structures::DiscountCurve;
use finstack_core::money::Money;
use finstack_valuations::instruments::fixed_income::structured_credit::{
    AssetPool, DealType, PoolAsset, StructuredCredit, Tranche, TrancheCoupon, TrancheSeniority,
    TrancheStructure,
};
use finstack_valuations::instruments::Instrument;
use finstack_valuations::metrics::MetricId;
use time::Month;

fn flat_discount_curve(rate: f64, base: Date) -> DiscountCurve {
    DiscountCurve::builder("USD-OIS")
        .base_date(base)
        .knots(vec![
            (0.0, 1.0),
            (0.5, (-rate * 0.5).exp()),
            (1.0, (-rate).exp()),
            (2.0, (-rate * 2.0).exp()),
            (5.0, (-rate * 5.0).exp()),
            (10.0, (-rate * 10.0).exp()),
        ])
        .build()
        .unwrap()
}

fn create_simple_pool() -> AssetPool {
    let mut pool = AssetPool::new("POOL", DealType::ABS, Currency::USD);
    pool.assets.push(PoolAsset::fixed_rate_bond(
        "A1",
        Money::new(5_000_000.0, Currency::USD),
        0.06,
        Date::from_calendar_date(2029, Month::January, 1).unwrap(),
        finstack_core::dates::DayCount::Thirty360,
    ));
    pool
}

fn create_simple_tranches() -> TrancheStructure {
    let senior = Tranche::new(
        "SENIOR",
        0.0,
        100.0,
        TrancheSeniority::Senior,
        Money::new(5_000_000.0, Currency::USD),
        TrancheCoupon::Fixed { rate: 0.035 },
        Date::from_calendar_date(2030, Month::January, 1).unwrap(),
    )
    .unwrap();
    TrancheStructure::new(vec![senior]).unwrap()
}

#[test]
fn test_structured_credit_bucketed_dv01_computed() {
    let as_of = Date::from_calendar_date(2025, Month::January, 1).unwrap();
    let maturity = Date::from_calendar_date(2030, Month::December, 31).unwrap();

    let sc = StructuredCredit::new_abs(
        "TEST_ABS",
        create_simple_pool(),
        create_simple_tranches(),
        Date::from_calendar_date(2024, Month::January, 1).unwrap(),
        maturity,
        "USD-OIS",
    )
    .with_payment_calendar("nyse");

    let market = MarketContext::new().insert(flat_discount_curve(0.04, as_of));

    let result = sc
        .price_with_metrics(
            &market,
            as_of,
            &[MetricId::BucketedDv01],
            finstack_valuations::instruments::PricingOptions::default(),
        )
        .unwrap();

    // BucketedDv01 should be present
    assert!(
        result.measures.contains_key("bucketed_dv01"),
        "BucketedDv01 should be computed"
    );

    let bucketed_dv01 = *result.measures.get("bucketed_dv01").unwrap();
    assert!(
        bucketed_dv01.is_finite(),
        "BucketedDv01 should be finite, got {}",
        bucketed_dv01
    );
}

/// Key-rate (bucketed) z-spread CS01 must reconcile to the parallel CS01.
///
/// StructuredCredit has no credit curve; `BucketedCs01` attributes the parallel
/// 1bp z-spread shock to standard tenor buckets via triangular allocation by
/// cashflow year fraction. Because each cashflow's weights sum to 1, the
/// per-bucket CS01s sum to exactly the parallel z-spread CS01.
#[test]
fn test_structured_credit_bucketed_cs01_reconciles_to_parallel() {
    let as_of = Date::from_calendar_date(2025, Month::January, 1).unwrap();
    let maturity = Date::from_calendar_date(2030, Month::December, 31).unwrap();

    let sc = StructuredCredit::new_abs(
        "TEST_ABS",
        create_simple_pool(),
        create_simple_tranches(),
        Date::from_calendar_date(2024, Month::January, 1).unwrap(),
        maturity,
        "USD-OIS",
    )
    .with_payment_calendar("nyse");

    let market = MarketContext::new().insert(flat_discount_curve(0.04, as_of));

    let result = sc
        .price_with_metrics(
            &market,
            as_of,
            &[MetricId::Cs01, MetricId::BucketedCs01],
            finstack_valuations::instruments::PricingOptions::default(),
        )
        .unwrap();

    let cs01 = *result
        .measures
        .get("cs01")
        .expect("parallel Cs01 should be computed");
    let bucketed = *result
        .measures
        .get("bucketed_cs01")
        .expect("BucketedCs01 should be computed");

    assert!(
        cs01.is_finite() && bucketed.is_finite(),
        "CS01 metrics must be finite (cs01={cs01}, bucketed={bucketed})"
    );
    // Triangular cashflow weights sum to 1 → the bucketed series sums to the
    // parallel z-spread CS01 (the same per-cashflow sum, just partitioned).
    assert!(
        (bucketed - cs01).abs() <= 1e-6 + 1e-7 * cs01.abs(),
        "bucketed CS01 ({bucketed}) must reconcile to parallel CS01 ({cs01})"
    );

    // The per-tenor series must also be present and sum to the same total.
    let series_sum: f64 = result
        .measures
        .iter()
        .filter(|(k, _)| k.as_str().starts_with("bucketed_cs01::"))
        .map(|(_, v)| *v)
        .sum();
    assert!(
        (series_sum - cs01).abs() <= 1e-6 + 1e-7 * cs01.abs(),
        "per-tenor bucketed_cs01 series ({series_sum}) must sum to parallel CS01 ({cs01})"
    );
}
