//! IR convexity and cross-gamma metric tests.
//!
//! `MetricId::IrConvexity` (parallel d²PV/dr²) and `MetricId::IrCrossGamma`
//! (mixed discount/forward second derivative) are registered but were
//! previously unexercised. Both must compute to finite values, and — because a
//! payer swap's PV is the exact negative of the otherwise-identical receiver
//! swap's PV — both second-order sensitivities must flip sign and match in
//! magnitude between the two sides.

use finstack_quant_core::currency::Currency;
use finstack_quant_core::dates::{BusinessDayConvention, Date, DayCount, StubKind, Tenor};
use finstack_quant_core::market_data::context::MarketContext;
use finstack_quant_core::market_data::term_structures::{DiscountCurve, ForwardCurve};
use finstack_quant_core::money::Money;
use finstack_quant_valuations::instruments::rates::irs::{InterestRateSwap, PayReceive};
use finstack_quant_valuations::instruments::Instrument;
use finstack_quant_valuations::metrics::MetricId;
use time::macros::date;

fn build_market(rate: f64, base_date: Date) -> MarketContext {
    let disc_curve = DiscountCurve::builder("USD_OIS")
        .base_date(base_date)
        .day_count(DayCount::Act360)
        .knots([
            (0.0, 1.0),
            (1.0, (-rate).exp()),
            (5.0, (-rate * 5.0).exp()),
            (10.0, (-rate * 10.0).exp()),
        ])
        .build()
        .unwrap();
    let fwd_curve = ForwardCurve::builder("USD_LIBOR_3M", 0.25)
        .base_date(base_date)
        .day_count(DayCount::Act360)
        .knots([(0.0, rate), (10.0, rate)])
        .build()
        .unwrap();
    MarketContext::new().insert(disc_curve).insert(fwd_curve)
}

fn create_standard_swap(as_of: Date, end: Date, side: PayReceive) -> InterestRateSwap {
    InterestRateSwap {
        id: "IRS_CONVEXITY_TEST".into(),
        notional: Money::new(1_000_000.0, Currency::USD),
        side,
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
            reset_lag_days: 0,
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

fn metric(
    swap: &InterestRateSwap,
    market: &MarketContext,
    as_of: Date,
    id: MetricId,
    key: &str,
) -> f64 {
    *swap
        .price_with_metrics(
            market,
            as_of,
            std::slice::from_ref(&id),
            finstack_quant_valuations::instruments::PricingOptions::default(),
        )
        .unwrap()
        .measures
        .get(key)
        .unwrap()
}

#[test]
fn test_ir_convexity_finite_and_flips_with_side() {
    let as_of = date!(2024 - 01 - 01);
    let end = date!(2029 - 01 - 01);
    let market = build_market(0.05, as_of);

    let recv = create_standard_swap(as_of, end, PayReceive::Receive);
    let pay = create_standard_swap(as_of, end, PayReceive::Pay);

    let conv_recv = metric(&recv, &market, as_of, MetricId::IrConvexity, "ir_convexity");
    let conv_pay = metric(&pay, &market, as_of, MetricId::IrConvexity, "ir_convexity");

    assert!(
        conv_recv.is_finite() && conv_pay.is_finite(),
        "IrConvexity should be finite (recv={conv_recv}, pay={conv_pay})"
    );
    // Payer PV = -Receiver PV, so the parallel second derivative flips sign and
    // matches in magnitude.
    assert!(
        (conv_recv + conv_pay).abs() <= 1e-6 + 1e-3 * conv_recv.abs(),
        "IrConvexity should flip sign between sides: recv={conv_recv}, pay={conv_pay}"
    );
}

#[test]
fn test_ir_cross_gamma_finite_and_flips_with_side() {
    let as_of = date!(2024 - 01 - 01);
    let end = date!(2029 - 01 - 01);
    let market = build_market(0.05, as_of);

    let recv = create_standard_swap(as_of, end, PayReceive::Receive);
    let pay = create_standard_swap(as_of, end, PayReceive::Pay);

    let xg_recv = metric(
        &recv,
        &market,
        as_of,
        MetricId::IrCrossGamma,
        "ir_cross_gamma",
    );
    let xg_pay = metric(
        &pay,
        &market,
        as_of,
        MetricId::IrCrossGamma,
        "ir_cross_gamma",
    );

    assert!(
        xg_recv.is_finite() && xg_pay.is_finite(),
        "IrCrossGamma should be finite (recv={xg_recv}, pay={xg_pay})"
    );
    // Mixed discount/forward second derivative also flips sign with the side.
    assert!(
        (xg_recv + xg_pay).abs() <= 1e-6 + 1e-3 * xg_recv.abs(),
        "IrCrossGamma should flip sign between sides: recv={xg_recv}, pay={xg_pay}"
    );
}
