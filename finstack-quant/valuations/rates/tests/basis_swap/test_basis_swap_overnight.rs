//! Overnight RFR vs term SOFR basis wiring.

use finstack_quant_cashflows::builder::FloatingLegCompounding;
use finstack_quant_core::dates::{BusinessDayConvention, Date, DayCount, StubKind, Tenor};
use finstack_quant_core::market_data::context::MarketContext;
use finstack_quant_core::market_data::term_structures::{DiscountCurve, ForwardCurve};
use finstack_quant_core::money::Money;
use finstack_quant_core::types::CurveId;
use finstack_quant_core::{currency::Currency::USD, math::interp::InterpStyle};
use finstack_quant_valuations_core::instruments::FloatLegSpec;
use finstack_quant_valuations_core::instruments::Instrument;
use finstack_quant_valuations_rates::basis_swap::BasisSwap;
use rust_decimal::Decimal;
use time::Month;

fn d(y: i32, m: u8, day: u8) -> Date {
    Date::from_calendar_date(y, Month::try_from(m).unwrap(), day).unwrap()
}

fn market() -> MarketContext {
    let disc = DiscountCurve::builder("USD-OIS")
        .base_date(d(2025, 1, 2))
        .knots(vec![(0.0, 1.0), (1.0, 0.96), (2.0, 0.92)])
        .interp(InterpStyle::LogLinear)
        .day_count(finstack_quant_core::dates::DayCount::Act365F)
        .extrapolation(finstack_quant_core::math::interp::ExtrapolationPolicy::Extend)
        .build()
        .unwrap();
    let ois = ForwardCurve::flat(
        "USD-SOFR-OIS",
        d(2025, 1, 2),
        0.04,
        finstack_quant_core::dates::DayCount::Act365F,
    )
    .unwrap();
    let term = ForwardCurve::flat(
        "USD-SOFR-3M",
        d(2025, 1, 2),
        0.041,
        finstack_quant_core::dates::DayCount::Act365F,
    )
    .unwrap();
    MarketContext::new().insert(disc).insert(ois).insert(term)
}

fn term_leg(curve: &str, compounding: FloatingLegCompounding) -> FloatLegSpec {
    FloatLegSpec {
        end_of_month: false,
        fixing_calendar_id: None,
        index_id: curve.into(),
        forward_curve_id: CurveId::new(curve),
        discount_curve_id: CurveId::new("USD-OIS"),
        start: d(2025, 1, 2),
        end: d(2026, 1, 2),
        frequency: Tenor::quarterly(),
        day_count: DayCount::Act360,
        business_day_convention: BusinessDayConvention::ModifiedFollowing,
        calendar_id: Some("usny".into()),
        stub: StubKind::ShortFront,
        spread_bp: Decimal::ZERO,
        payment_lag_days: 0,
        reset_lag_days: 0,
        compounding,
    }
}

#[test]
fn sofr_ois_versus_sofr_3m_prices() {
    let ctx = market();
    let as_of = d(2025, 1, 2);
    let swap = BasisSwap::new(
        "SOFR-OIS-3M",
        Money::new(10_000_000.0, USD).expect("valid money fixture"),
        term_leg(
            "USD-SOFR-OIS",
            FloatingLegCompounding::CompoundedInArrears { lookback_days: 0 },
        ),
        term_leg("USD-SOFR-3M", FloatingLegCompounding::Simple),
    )
    .expect("overnight vs term basis");
    let pv = swap.value(&ctx, as_of).expect("pv");
    assert_eq!(pv.currency(), USD);
    assert!(pv.amount().is_finite());
    assert!(
        pv.amount().abs() > 1.0,
        "OIS vs 3M basis on a 4% vs 4.1% curve should have non-trivial NPV, got {}",
        pv.amount()
    );
}
