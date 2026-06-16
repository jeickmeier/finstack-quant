//! Bucketed DV01 metric tests - Risk by tenor bucket.

use finstack_quant_core::currency::Currency;
use finstack_quant_core::dates::{BusinessDayConvention, Date, DayCount, StubKind, Tenor};
use finstack_quant_core::market_data::context::MarketContext;
use finstack_quant_core::market_data::term_structures::{DiscountCurve, ForwardCurve};
use finstack_quant_core::money::Money;
use finstack_quant_valuations::instruments::rates::irs::{
    FloatingLegCompounding, InterestRateSwap, PayReceive,
};
use finstack_quant_valuations::instruments::Instrument;
use finstack_quant_valuations::metrics::MetricId;
use time::macros::date;

fn build_flat_discount_curve(rate: f64, base_date: Date) -> DiscountCurve {
    let mut builder = DiscountCurve::builder("USD_OIS")
        .base_date(base_date)
        .day_count(DayCount::Act360)
        .knots([
            (0.0, 1.0),
            (1.0, (-rate).exp()),
            (2.0, (-rate * 2.0).exp()),
            (3.0, (-rate * 3.0).exp()),
            (5.0, (-rate * 5.0).exp()),
            (7.0, (-rate * 7.0).exp()),
            (10.0, (-rate * 10.0).exp()),
        ]);

    // For zero or negative rates, DFs may be flat or increasing
    if rate.abs() < 1e-10 || rate < 0.0 {
        builder = builder.validation(
            finstack_quant_core::market_data::term_structures::ValidationMode::Raw {
                allow_non_monotonic: true,
                forward_floor: None,
            },
        );
    }

    builder.build().unwrap()
}

fn build_flat_forward_curve(rate: f64, base_date: Date, id: &str) -> ForwardCurve {
    ForwardCurve::builder(id, 0.25) // 3M tenor
        .base_date(base_date)
        .day_count(DayCount::Act360)
        .knots(vec![
            (0.0, rate),
            (1.0, rate),
            (2.0, rate),
            (3.0, rate),
            (5.0, rate),
            (7.0, rate),
            (10.0, rate),
        ])
        .build()
        .unwrap()
}

fn create_swap(as_of: Date, end: Date) -> InterestRateSwap {
    InterestRateSwap {
        id: "IRS_BUCKETED_DV01_TEST".into(),
        notional: Money::new(1_000_000.0, Currency::USD),
        side: PayReceive::Receive,
        fixed: finstack_quant_valuations::instruments::FixedLegSpec {
            discount_curve_id: "USD_OIS".into(),
            rate: rust_decimal::Decimal::try_from(0.05).expect("valid"),
            frequency: Tenor::quarterly(),
            day_count: DayCount::Act360,
            bdc: BusinessDayConvention::ModifiedFollowing,
            calendar_id: None,
            stub: StubKind::None,
            par_method: None,
            compounding_simple: true,
            payment_lag_days: 0,
            end_of_month: false,
            start: as_of,
            end,
        },
        float: finstack_quant_valuations::instruments::FloatLegSpec {
            discount_curve_id: "USD_OIS".into(),
            forward_curve_id: "USD_LIBOR_3M".into(),
            spread_bp: rust_decimal::Decimal::try_from(0.0).expect("valid"),
            frequency: Tenor::quarterly(),
            day_count: DayCount::Act360,
            bdc: BusinessDayConvention::ModifiedFollowing,
            calendar_id: None,
            fixing_calendar_id: None,
            stub: StubKind::None,
            reset_lag_days: 0, // Use 0 for spot-starting swaps to avoid needing historical fixings
            compounding: Default::default(),
            payment_lag_days: 0,
            end_of_month: false,
            start: as_of,
            end,
        },
        margin_spec: None,
        pricing_overrides: finstack_quant_valuations::instruments::PricingOverrides::default(),
        attributes: Default::default(),
    }
}

#[test]
fn test_bucketed_vs_parallel_dv01_sanity() {
    // Sum of key-rate bucketed DV01 should approximate parallel DV01
    let as_of = date!(2024 - 01 - 01);
    let end = date!(2029 - 01 - 01);

    let mut swap = create_swap(as_of, end);
    // Make this an OIS-style swap so that pricing depends only on the discount
    // curve; this keeps bucketed vs parallel DV01 comparable in a single-curve
    // setting, which is what this sanity check is targeting.
    // Use lookback=0 to avoid requiring historical fixings at as_of for the first coupon.
    swap.float.compounding = FloatingLegCompounding::fedfunds();
    swap.float.forward_curve_id = "USD_OIS".into();

    let disc_curve = build_flat_discount_curve(0.05, as_of);
    let market = MarketContext::new().insert(disc_curve);

    let result = swap
        .price_with_metrics(
            &market,
            as_of,
            &[MetricId::BucketedDv01, MetricId::Dv01],
            finstack_quant_valuations::instruments::PricingOptions::default(),
        )
        .unwrap();

    let mut sum_bucketed = 0.0;
    println!("All measures:");
    for (k, v) in &result.measures {
        println!("  {}: {:.2}", k, v);
        if k.as_str().starts_with("bucketed_dv01::") {
            sum_bucketed += *v;
        }
    }
    let parallel = *result.measures.get("dv01").unwrap_or(&0.0);

    println!("\nSum of bucketed DV01: {:.2}", sum_bucketed);
    println!("Parallel DV01: {:.2}", parallel);
    println!("Difference: {:.2}", (sum_bucketed - parallel).abs());

    // For single-curve OIS swap, bucketed should closely match parallel
    // Allow 1% relative + $50 absolute for numerical differences
    let tolerance = parallel.abs() * 0.01 + 50.0;
    assert!(
        (sum_bucketed - parallel).abs() < tolerance,
        "Bucketed sum ({:.2}) should match parallel ({:.2}) within 1%.\n\
         Difference: {:.2} ({:.2}%)",
        sum_bucketed,
        parallel,
        (sum_bucketed - parallel).abs(),
        (sum_bucketed - parallel).abs() / parallel.abs() * 100.0
    );
}

#[test]
fn test_bucketed_dv01_per_curve() {
    // Test per-curve bucketed DV01 for IRS with separate discount and forward curves
    let as_of = date!(2024 - 01 - 01);
    let end = date!(2029 - 01 - 01);

    let swap = create_swap(as_of, end);
    let disc_curve = build_flat_discount_curve(0.05, as_of);
    let fwd_curve = build_flat_forward_curve(0.05, as_of, "USD_LIBOR_3M");
    let market = MarketContext::new().insert(disc_curve).insert(fwd_curve);

    let result = swap
        .price_with_metrics(
            &market,
            as_of,
            &[MetricId::BucketedDv01],
            finstack_quant_valuations::instruments::PricingOptions::default(),
        )
        .unwrap();

    // Verify primary discount curve series exists under the standard aggregate key
    assert!(
        result.measures.contains_key("bucketed_dv01"),
        "Standard BucketedDv01 scalar should be present for BC"
    );

    // Verify per-bucket keys exist for discount curve (now always curve-qualified)
    assert!(
        result.measures.contains_key("bucketed_dv01::USD_OIS::1y"),
        "Discount curve bucketed series should be present under curve-qualified key"
    );

    // Verify per-curve series exist
    let mut discount_curve_buckets = 0;
    let mut forward_curve_buckets = 0;

    for key in result.measures.keys() {
        if key.as_str().starts_with("bucketed_dv01::USD_OIS::") {
            discount_curve_buckets += 1;
        }
        if key.as_str().starts_with("bucketed_dv01::USD_LIBOR_3M::") {
            forward_curve_buckets += 1;
        }
    }

    // Should have buckets for both curves (0.25y, 0.5y, 1y, 2y, 3y, 5y, 7y, 10y, etc.)
    assert!(
        discount_curve_buckets > 0,
        "Should have discount curve bucketed DV01s under bucketed_dv01::USD_OIS::*"
    );
    assert!(
        forward_curve_buckets > 0,
        "Should have forward curve bucketed DV01s under bucketed_dv01::USD_LIBOR_3M::*"
    );

    // Verify totals: sum of per-curve buckets should equal the total
    let total_dv01 = *result.measures.get("bucketed_dv01").unwrap();

    let mut sum_disc = 0.0;
    let mut sum_fwd = 0.0;

    for (key, val) in &result.measures {
        if key.as_str().starts_with("bucketed_dv01::USD_OIS::") {
            sum_disc += val;
        }
        if key.as_str().starts_with("bucketed_dv01::USD_LIBOR_3M::") {
            sum_fwd += val;
        }
    }

    // Total should approximately equal sum of both curves' contributions
    let sum_both = sum_disc + sum_fwd;
    assert!(
        (total_dv01 - sum_both).abs() < 1.0,
        "Total DV01 ({}) should equal sum of per-curve DV01s ({})",
        total_dv01,
        sum_both
    );
}
