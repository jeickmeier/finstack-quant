//! Edge case and boundary condition tests for basis swaps.
//!
//! Tests validate robustness across extreme market conditions, unusual
//! instrument configurations, and boundary scenarios.

use finstack_quant_cashflows::CashflowProvider;
use finstack_quant_core::cashflow::CFKind;
use finstack_quant_core::dates::{BusinessDayConvention, Date, DayCount, StubKind, Tenor};
use finstack_quant_core::market_data::context::MarketContext;
use finstack_quant_core::market_data::term_structures::{DiscountCurve, ForwardCurve};
use finstack_quant_core::market_data::traits::Discounting;
use finstack_quant_core::money::Money;
use finstack_quant_core::types::CurveId;
use finstack_quant_core::{currency::Currency::USD, math::interp::InterpStyle};
use finstack_quant_valuations_core::instruments::FloatLegSpec;
use finstack_quant_valuations_core::instruments::Instrument;
use finstack_quant_valuations_core::metrics::MetricId;
use finstack_quant_valuations_rates::basis_swap::BasisSwap;
use rust_decimal::Decimal;
use time::Month;

fn d(y: i32, m: u8, day: u8) -> Date {
    Date::from_calendar_date(y, Month::try_from(m).unwrap(), day).unwrap()
}

const CALENDAR_ID: &str = "usny";

fn market() -> MarketContext {
    let disc = DiscountCurve::builder("USD-OIS")
        .base_date(d(2025, 1, 2))
        .knots(vec![(0.0, 1.0), (0.5, 0.99), (1.0, 0.98), (5.0, 0.90)])
        .interp(InterpStyle::LogLinear)
        .day_count(finstack_quant_core::dates::DayCount::Act365F)
        .extrapolation(finstack_quant_core::math::interp::ExtrapolationPolicy::Extend)
        .build()
        .unwrap();
    let f3m = ForwardCurve::builder("USD-SOFR-3M")
        .base_date(d(2025, 1, 2))
        .day_count(finstack_quant_core::dates::DayCount::Act365F)
        .knots([
            (0.0, 1.0),
            (0.125, 0.997487536532938),
            (0.25, 0.994950292856076),
            (0.375, 0.992388490374784),
            (0.5, 0.98980235222721),
            (0.625, 0.98719210325196),
            (0.75, 0.984557969955606),
            (0.875, 0.981900180480004),
            (1.0, 0.97921896456946),
            (1.25, 0.973825219539481),
            (1.5, 0.968415788843127),
            (1.75, 0.962991265325316),
            (2.0, 0.957552240640482),
            (2.25, 0.95209930514862),
            (2.5, 0.946633047812227),
            (2.75, 0.941154056094142),
            (3.0, 0.935662915856331),
            (3.25, 0.930160211259623),
            (3.5, 0.924646524664431),
            (3.75, 0.91912243653247),
            (4.0, 0.913588525329506),
            (4.25, 0.90804536742914),
            (4.5, 0.90249353701767),
            (4.75, 0.896933606000028),
            (5.0, 0.891366143906831),
        ])
        .interp(finstack_quant_core::math::interp::InterpStyle::LogLinear)
        .extrapolation(finstack_quant_core::math::interp::ExtrapolationPolicy::Extend)
        .build()
        .unwrap();
    let f1m = ForwardCurve::builder("USD-SOFR-1M")
        .base_date(d(2025, 1, 2))
        .day_count(finstack_quant_core::dates::DayCount::Act365F)
        .knots([
            (0.0, 1.0),
            (0.125, 0.9976122302682),
            (0.25, 0.995199061524078),
            (0.375, 0.992760705844714),
            (0.5, 0.990297377149241),
            (0.625, 0.987809291167626),
            (0.75, 0.985296665409241),
            (0.875, 0.982759719131244),
            (1.0, 0.980198673306755),
            (1.25, 0.975043262181957),
            (1.5, 0.969869502539092),
            (1.75, 0.964677975480559),
            (2.0, 0.959469261503619),
            (2.25, 0.954243940395089),
            (2.5, 0.949002591126764),
            (2.75, 0.943745791751597),
            (3.0, 0.938474119300664),
            (3.25, 0.933188149680935),
            (3.5, 0.927888457573875),
            (3.75, 0.922575616334903),
            (4.0, 0.917250197893724),
            (4.25, 0.911912772655572),
            (4.5, 0.906563909403366),
            (4.75, 0.901204175200823),
            (5.0, 0.895834135296528),
        ])
        .interp(finstack_quant_core::math::interp::InterpStyle::LogLinear)
        .extrapolation(finstack_quant_core::math::interp::ExtrapolationPolicy::Extend)
        .build()
        .unwrap();
    MarketContext::new()
        .try_insert(disc)
        .expect("insert market curve")
        .try_insert(f3m)
        .expect("insert market curve")
        .try_insert(f1m)
        .expect("insert market curve")
}

/// Vanilla basis swaps must not exchange notionals.
#[test]
fn basis_swap_cashflows_have_no_notional_exchange() {
    let ctx = market();
    let swap = BasisSwap::new(
        "NO-NOTIONAL",
        Money::new(10_000_000.0, USD).expect("valid money fixture"),
        make_leg("USD-SOFR-3M", d(2025, 1, 2), d(2026, 1, 2), Decimal::ZERO),
        make_leg("USD-SOFR-1M", d(2025, 1, 2), d(2026, 1, 2), Decimal::ZERO),
    )
    .expect("swap construction");
    let schedule = swap
        .cashflow_schedule(&ctx, d(2025, 1, 2))
        .expect("basis-swap schedule");
    assert!(
        schedule
            .get_flows()
            .iter()
            .all(|cf| cf.get_kind() != CFKind::Notional),
        "basis swap must not emit issue or redemption notionals"
    );
    assert!(
        schedule
            .get_flows()
            .iter()
            .any(|cf| cf.get_kind() == CFKind::Floating),
        "coupon flows must remain without retain_flows"
    );
}

fn make_leg(forward_curve: &str, start: Date, end: Date, spread_bp: Decimal) -> FloatLegSpec {
    FloatLegSpec {
        end_of_month: false,
        fixing_calendar_id: None,
        index_id: forward_curve.into(),
        forward_curve_id: CurveId::new(forward_curve),
        discount_curve_id: CurveId::new("USD-OIS"),
        start,
        end,
        frequency: Tenor::quarterly(),
        day_count: DayCount::Act360,
        business_day_convention: BusinessDayConvention::ModifiedFollowing,
        calendar_id: Some(CALENDAR_ID.into()),
        stub: StubKind::ShortFront,
        spread_bp,
        payment_lag_days: 0,
        reset_lag_days: 0,
        compounding: Default::default(),
    }
}

#[test]
fn adjusted_payment_dates_control_simple_leg_pv_and_settlement() {
    use finstack_quant_core::market_data::scalars::ScalarTimeSeries;

    let start = d(2025, 7, 31);
    let end = d(2025, 8, 31); // Sunday; September 1 is US Labor Day.
    let ctx = market()
        .insert_series(
            ScalarTimeSeries::new(
                "FIXING:USD-SOFR-3M",
                vec![(start, 0.04)],
                None,
                finstack_quant_core::market_data::scalars::SeriesInterpolation::Step,
                finstack_quant_core::market_data::scalars::SeriesExtrapolation::Error,
            )
            .unwrap(),
        )
        .insert_series(
            ScalarTimeSeries::new(
                "FIXING:USD-SOFR-1M",
                vec![(start, 0.03)],
                None,
                finstack_quant_core::market_data::scalars::SeriesInterpolation::Step,
                finstack_quant_core::market_data::scalars::SeriesExtrapolation::Error,
            )
            .unwrap(),
        );
    let discount = ctx.get_discount("USD-OIS").unwrap();
    let notional = 10_000_000.0;
    // Explicit contractual dates form an independent oracle, including lag
    // applied after the BDC and no second adjustment by the PV calculation.
    for (bdc, lag, payment) in [
        (BusinessDayConvention::Following, 0, d(2025, 9, 2)),
        (BusinessDayConvention::Following, 2, d(2025, 9, 4)),
        (BusinessDayConvention::ModifiedFollowing, 0, d(2025, 8, 29)),
        (BusinessDayConvention::ModifiedFollowing, 2, d(2025, 9, 3)),
    ] {
        let mut primary = make_leg("USD-SOFR-3M", start, end, Decimal::ZERO);
        primary.frequency = Tenor::monthly();
        primary.business_day_convention = bdc;
        primary.payment_lag_days = lag;
        let mut reference = primary.clone();
        reference.index_id = "USD-SOFR-1M".into();
        reference.forward_curve_id = CurveId::new("USD-SOFR-1M");
        let swap = BasisSwap::new(
            "PAYMENT-DATE",
            Money::new(notional, USD).unwrap(),
            primary,
            reference,
        )
        .unwrap();
        for as_of in [
            d(2025, 8, 28),
            d(2025, 8, 29),
            d(2025, 8, 31),
            d(2025, 9, 1),
            d(2025, 9, 2),
            d(2025, 9, 3),
            d(2025, 9, 4),
        ] {
            // Accrual stays July 31--August 31 under this contract, even
            // when ModifiedFollowing pays before its unadjusted end.
            let expected = if as_of < payment {
                notional
                    * (0.04 - 0.03)
                    * (31.0 / 360.0)
                    * discount.df_between_dates(as_of, payment).unwrap()
            } else {
                0.0
            };
            let actual = swap.value(&ctx, as_of).unwrap().amount();
            assert!(
                (actual - expected).abs() < 1e-7,
                "{bdc:?}, lag {lag}, as_of {as_of}: {actual} vs {expected}"
            );
        }
    }
}

#[test]
fn zero_notional_is_rejected() {
    assert!(
        BasisSwap::new(
            "ZERO-NOTIONAL",
            Money::new(0.0, USD).expect("valid money fixture"),
            make_leg("USD-SOFR-3M", d(2025, 1, 2), d(2026, 1, 2), Decimal::ZERO),
            make_leg("USD-SOFR-1M", d(2025, 1, 2), d(2026, 1, 2), Decimal::ZERO),
        )
        .is_err()
    );
}

#[test]
fn very_small_notional() {
    let ctx = market();
    let as_of = d(2025, 1, 2);

    let swap = BasisSwap::new(
        "SMALL-NOTIONAL",
        Money::new(1.0, USD).expect("valid money fixture"),
        make_leg("USD-SOFR-3M", d(2025, 1, 2), d(2026, 1, 2), Decimal::ZERO),
        make_leg("USD-SOFR-1M", d(2025, 1, 2), d(2026, 1, 2), Decimal::ZERO),
    )
    .expect("swap construction");

    let npv = swap.value(&ctx, as_of).unwrap();
    assert!(npv.amount().is_finite());
    assert!(npv.amount().abs() < 10.0);
}

#[test]
fn very_large_notional() {
    let ctx = market();
    let as_of = d(2025, 1, 2);

    let swap = BasisSwap::new(
        "LARGE-NOTIONAL",
        Money::new(1_000_000_000_000.0, USD).expect("valid money fixture"),
        make_leg("USD-SOFR-3M", d(2025, 1, 2), d(2026, 1, 2), Decimal::ZERO),
        make_leg("USD-SOFR-1M", d(2025, 1, 2), d(2026, 1, 2), Decimal::ZERO),
    )
    .expect("swap construction");

    let res = swap
        .price_with_metrics(
            &ctx,
            as_of,
            &[MetricId::Dv01],
            finstack_quant_valuations_core::instruments::PricingOptions::default(),
        )
        .unwrap();

    let dv01 = res
        .measures
        .get("bucketed_dv01::USD-SOFR-3M")
        .copied()
        .expect("primary-forward bucketed DV01 must be present");

    assert!(dv01.is_finite());
    assert!(
        dv01 > 1_000_000.0,
        "Large notional should have substantial DV01: got {}",
        dv01
    );
}

#[test]
fn very_short_maturity() {
    let ctx = market();
    let as_of = d(2025, 1, 2);

    let swap = BasisSwap::new(
        "SHORT-MAT",
        Money::new(10_000_000.0, USD).expect("valid money fixture"),
        make_leg("USD-SOFR-3M", d(2025, 1, 2), d(2025, 2, 2), Decimal::ZERO),
        make_leg("USD-SOFR-1M", d(2025, 1, 2), d(2025, 2, 2), Decimal::ZERO),
    )
    .expect("swap construction");

    let npv = swap.value(&ctx, as_of);
    assert!(npv.is_ok(), "Short maturity swap should price");
}

#[test]
fn very_long_maturity() {
    let ctx = market();
    let as_of = d(2025, 1, 2);

    let swap = BasisSwap::new(
        "LONG-MAT",
        Money::new(10_000_000.0, USD).expect("valid money fixture"),
        make_leg("USD-SOFR-3M", d(2025, 1, 2), d(2055, 1, 2), Decimal::ZERO),
        make_leg("USD-SOFR-1M", d(2025, 1, 2), d(2055, 1, 2), Decimal::ZERO),
    )
    .expect("swap construction");

    let res = swap
        .price_with_metrics(
            &ctx,
            as_of,
            &[MetricId::AnnuityPrimary],
            finstack_quant_valuations_core::instruments::PricingOptions::default(),
        )
        .unwrap();
    let annuity = res.measures[MetricId::AnnuityPrimary.as_str()];

    assert!(annuity.is_finite());
    assert!(annuity > 0.0);
}

#[test]
fn extreme_positive_spread() {
    let ctx = market();
    let as_of = d(2025, 1, 2);

    let swap = BasisSwap::new(
        "EXTREME-SPREAD",
        Money::new(10_000_000.0, USD).expect("valid money fixture"),
        make_leg(
            "USD-SOFR-3M",
            d(2025, 1, 2),
            d(2026, 1, 2),
            Decimal::from(1000),
        ),
        make_leg("USD-SOFR-1M", d(2025, 1, 2), d(2026, 1, 2), Decimal::ZERO),
    )
    .expect("swap construction");

    let npv = swap.value(&ctx, as_of).unwrap();
    assert!(npv.amount().is_finite());
    assert!(npv.amount() > 500_000.0);
}

#[test]
fn extreme_negative_spread() {
    let ctx = market();
    let as_of = d(2025, 1, 2);

    let swap = BasisSwap::new(
        "EXTREME-NEG-SPREAD",
        Money::new(10_000_000.0, USD).expect("valid money fixture"),
        make_leg(
            "USD-SOFR-3M",
            d(2025, 1, 2),
            d(2026, 1, 2),
            Decimal::from(-1000),
        ),
        make_leg("USD-SOFR-1M", d(2025, 1, 2), d(2026, 1, 2), Decimal::ZERO),
    )
    .expect("swap construction");

    let npv = swap.value(&ctx, as_of).unwrap();
    assert!(npv.amount().is_finite());
    assert!(npv.amount() < -500_000.0);
}

#[test]
fn flat_curves_zero_rates() {
    let disc = DiscountCurve::builder("USD-OIS")
        .base_date(d(2025, 1, 2))
        .knots(vec![(0.0, 1.0), (1.0, 1.0), (2.0, 1.0)])
        .interp(InterpStyle::LogLinear)
        .day_count(finstack_quant_core::dates::DayCount::Act365F)
        .extrapolation(finstack_quant_core::math::interp::ExtrapolationPolicy::Extend)
        .build()
        .unwrap();
    let f3m = ForwardCurve::flat(
        "USD-SOFR-3M",
        d(2025, 1, 2),
        1e-6,
        finstack_quant_core::dates::DayCount::Act365F,
    )
    .unwrap();
    let f1m = ForwardCurve::flat(
        "USD-SOFR-1M",
        d(2025, 1, 2),
        1e-6,
        finstack_quant_core::dates::DayCount::Act365F,
    )
    .unwrap();

    let ctx = MarketContext::new()
        .try_insert(disc)
        .expect("insert market curve")
        .try_insert(f3m)
        .expect("insert market curve")
        .try_insert(f1m)
        .expect("insert market curve");

    let swap = BasisSwap::new(
        "ZERO-RATES",
        Money::new(10_000_000.0, USD).expect("valid money fixture"),
        make_leg("USD-SOFR-3M", d(2025, 1, 2), d(2026, 1, 2), Decimal::ZERO),
        make_leg("USD-SOFR-1M", d(2025, 1, 2), d(2026, 1, 2), Decimal::ZERO),
    )
    .expect("swap construction");

    let npv = swap.value(&ctx, d(2025, 1, 2)).unwrap();
    assert!(npv.amount().abs() < 1.0);
}

#[test]
fn negative_rates() {
    let disc = DiscountCurve::builder("USD-OIS")
        .base_date(d(2025, 1, 2))
        .knots(vec![(0.0, 1.0), (1.0, 1.005), (2.0, 1.01)])
        .interp(InterpStyle::LogLinear)
        .day_count(finstack_quant_core::dates::DayCount::Act365F)
        .extrapolation(finstack_quant_core::math::interp::ExtrapolationPolicy::Extend)
        .build()
        .unwrap();
    let f3m = ForwardCurve::builder("USD-SOFR-3M")
        .base_date(d(2025, 1, 2))
        .day_count(finstack_quant_core::dates::DayCount::Act365F)
        .knots([
            (0.0, 1.0),
            (0.125, 0.999999867187509),
            (0.25, 0.99999971875004),
            (0.375, 0.999999554687599),
            (0.5, 0.999999375000195),
            (0.625, 0.999999179687836),
            (0.75, 0.999998968750532),
            (0.875, 0.999998742188291),
            (1.0, 0.999998500001125),
            (1.125, 0.999998242189045),
            (1.25, 0.999997968752063),
            (1.375, 0.999997679690192),
            (1.5, 0.999997375003445),
            (1.625, 0.999997054691837),
            (1.75, 0.999996718755383),
            (1.875, 0.999996367194099),
            (2.0, 0.999996000008),
        ])
        .interp(finstack_quant_core::math::interp::InterpStyle::LogLinear)
        .extrapolation(finstack_quant_core::math::interp::ExtrapolationPolicy::Extend)
        .build()
        .unwrap();
    let f1m = ForwardCurve::builder("USD-SOFR-1M")
        .base_date(d(2025, 1, 2))
        .day_count(finstack_quant_core::dates::DayCount::Act365F)
        .knots([
            (0.0, 1.0),
            (0.125, 0.999999867187509),
            (0.25, 0.99999971875004),
            (0.375, 0.999999554687599),
            (0.5, 0.999999375000195),
            (0.625, 0.999999179687836),
            (0.75, 0.999998968750532),
            (0.875, 0.999998742188291),
            (1.0, 0.999998500001125),
            (1.125, 0.999998242189045),
            (1.25, 0.999997968752063),
            (1.375, 0.999997679690192),
            (1.5, 0.999997375003445),
            (1.625, 0.999997054691837),
            (1.75, 0.999996718755383),
            (1.875, 0.999996367194099),
            (2.0, 0.999996000008),
        ])
        .interp(finstack_quant_core::math::interp::InterpStyle::LogLinear)
        .extrapolation(finstack_quant_core::math::interp::ExtrapolationPolicy::Extend)
        .build()
        .unwrap();

    let ctx = MarketContext::new()
        .try_insert(disc)
        .expect("insert market curve")
        .try_insert(f3m)
        .expect("insert market curve")
        .try_insert(f1m)
        .expect("insert market curve");

    let swap = BasisSwap::new(
        "NEG-RATES",
        Money::new(10_000_000.0, USD).expect("valid money fixture"),
        make_leg("USD-SOFR-3M", d(2025, 1, 2), d(2026, 1, 2), Decimal::ZERO),
        make_leg("USD-SOFR-1M", d(2025, 1, 2), d(2026, 1, 2), Decimal::ZERO),
    )
    .expect("swap construction");

    let res = swap
        .price_with_metrics(
            &ctx,
            d(2025, 1, 2),
            &[MetricId::PvPrimary, MetricId::PvReference],
            finstack_quant_valuations_core::instruments::PricingOptions::default(),
        )
        .unwrap();

    assert!(res.measures[MetricId::PvPrimary.as_str()].is_finite());
    assert!(res.measures[MetricId::PvReference.as_str()].is_finite());
}

#[test]
fn valuation_at_maturity() {
    let ctx = market();
    let maturity = d(2026, 1, 2);

    let swap = BasisSwap::new(
        "AT-MATURITY",
        Money::new(10_000_000.0, USD).expect("valid money fixture"),
        make_leg("USD-SOFR-3M", d(2025, 1, 2), maturity, Decimal::ZERO),
        make_leg("USD-SOFR-1M", d(2025, 1, 2), maturity, Decimal::ZERO),
    )
    .expect("swap construction");

    let npv = swap.value(&ctx, maturity).unwrap();
    assert!(npv.amount().abs() < 100.0);
}

#[test]
fn valuation_after_maturity() {
    let ctx = market();
    let maturity = d(2026, 1, 2);
    let after_maturity = d(2026, 6, 1);

    let swap = BasisSwap::new(
        "AFTER-MATURITY",
        Money::new(10_000_000.0, USD).expect("valid money fixture"),
        make_leg("USD-SOFR-3M", d(2025, 1, 2), maturity, Decimal::ZERO),
        make_leg("USD-SOFR-1M", d(2025, 1, 2), maturity, Decimal::ZERO),
    )
    .expect("swap construction");

    let npv = swap.value(&ctx, after_maturity).unwrap();
    assert!(npv.amount().abs() < 1.0);
}

#[test]
fn identical_forward_curves() {
    let ctx = market();
    let as_of = d(2025, 1, 2);

    let swap = BasisSwap::builder()
        .id("IDENTICAL-CURVES".into())
        .notional(Money::new(10_000_000.0, USD).expect("valid money fixture"))
        .primary_leg(make_leg(
            "USD-SOFR-3M",
            d(2025, 1, 2),
            d(2026, 1, 2),
            Decimal::ZERO,
        ))
        .reference_leg(FloatLegSpec {
            end_of_month: false,
            fixing_calendar_id: None,
            index_id: "USD-SOFR-3M".into(),
            forward_curve_id: CurveId::new("USD-SOFR-3M"),
            discount_curve_id: CurveId::new("USD-OIS"),
            start: d(2025, 1, 2),
            end: d(2026, 1, 2),
            frequency: Tenor::quarterly(),
            day_count: DayCount::Act360,
            business_day_convention: BusinessDayConvention::ModifiedFollowing,
            calendar_id: Some(CALENDAR_ID.into()),
            stub: StubKind::ShortFront,
            spread_bp: Decimal::ZERO,
            payment_lag_days: 0,
            reset_lag_days: 0,
            compounding: Default::default(),
        })
        .allow_same_curve(true)
        .attributes(Default::default())
        .build()
        .expect("swap construction with same curves allowed");

    let res = swap
        .price_with_metrics(
            &ctx,
            as_of,
            &[
                MetricId::PvPrimary,
                MetricId::PvReference,
                MetricId::BasisParSpread,
            ],
            finstack_quant_valuations_core::instruments::PricingOptions::default(),
        )
        .unwrap();

    let pv_primary = res.measures[MetricId::PvPrimary.as_str()];
    let pv_reference = res.measures[MetricId::PvReference.as_str()];
    let par_spread = res.measures[MetricId::BasisParSpread.as_str()];

    assert!(
        (pv_primary - pv_reference).abs() < 1.0,
        "PVs should match for identical curves"
    );
    assert!(
        par_spread.abs() < 0.1,
        "Par spread should be near zero for identical curves"
    );
}

#[test]
fn steep_curve() {
    let disc = DiscountCurve::builder("USD-OIS")
        .base_date(d(2025, 1, 2))
        .knots(vec![(0.0, 1.0), (0.5, 0.95), (1.0, 0.85), (2.0, 0.70)])
        .interp(InterpStyle::LogLinear)
        .day_count(finstack_quant_core::dates::DayCount::Act365F)
        .extrapolation(finstack_quant_core::math::interp::ExtrapolationPolicy::Extend)
        .build()
        .unwrap();
    let f3m = ForwardCurve::builder("USD-SOFR-3M")
        .base_date(d(2025, 1, 2))
        .day_count(finstack_quant_core::dates::DayCount::Act365F)
        .knots([
            (0.0, 1.0),
            (0.125, 0.998126756714382),
            (0.25, 0.995012479192682),
            (0.375, 0.990668808304661),
            (0.5, 0.985111939603063),
            (0.625, 0.978209665521516),
            (0.75, 0.969839194590702),
            (0.875, 0.960039115536311),
            (1.0, 0.948854321055801),
            (1.125, 0.936701492882787),
            (1.25, 0.923982173754537),
            (1.375, 0.91072378725585),
            (1.5, 0.896954628127771),
            (1.625, 0.882703762036534),
            (1.75, 0.868000923831803),
            (1.875, 0.852876414789924),
            (2.0, 0.837360999335754),
        ])
        .interp(finstack_quant_core::math::interp::InterpStyle::LogLinear)
        .extrapolation(finstack_quant_core::math::interp::ExtrapolationPolicy::Extend)
        .build()
        .unwrap();
    let f1m = ForwardCurve::builder("USD-SOFR-1M")
        .base_date(d(2025, 1, 2))
        .day_count(finstack_quant_core::dates::DayCount::Act365F)
        .knots([
            (0.0, 1.0),
            (0.125, 0.998282726204918),
            (0.25, 0.995634556371046),
            (0.375, 0.992062916318795),
            (0.5, 0.987577800493881),
            (0.625, 0.981884838409545),
            (0.75, 0.974700533783853),
            (0.875, 0.966058149897594),
            (1.0, 0.9559974818331),
            (1.125, 0.944933593864856),
            (1.25, 0.933268348983563),
            (1.375, 0.92102727833037),
            (1.5, 0.908236928451824),
            (1.625, 0.894924765620324),
            (1.75, 0.881119077880176),
            (1.875, 0.866848875306405),
            (2.0, 0.852143788966211),
        ])
        .interp(finstack_quant_core::math::interp::InterpStyle::LogLinear)
        .extrapolation(finstack_quant_core::math::interp::ExtrapolationPolicy::Extend)
        .build()
        .unwrap();

    let ctx = MarketContext::new()
        .try_insert(disc)
        .expect("insert market curve")
        .try_insert(f3m)
        .expect("insert market curve")
        .try_insert(f1m)
        .expect("insert market curve");

    let swap = BasisSwap::new(
        "STEEP-CURVE",
        Money::new(10_000_000.0, USD).expect("valid money fixture"),
        make_leg("USD-SOFR-3M", d(2025, 1, 2), d(2027, 1, 2), Decimal::ZERO),
        make_leg("USD-SOFR-1M", d(2025, 1, 2), d(2027, 1, 2), Decimal::ZERO),
    )
    .expect("swap construction");

    let res = swap
        .price_with_metrics(
            &ctx,
            d(2025, 1, 2),
            &[MetricId::Dv01],
            finstack_quant_valuations_core::instruments::PricingOptions::default(),
        )
        .unwrap();

    assert!(res.measures[MetricId::Dv01.as_str()].is_finite());
}

#[test]
fn seasoned_swap_requires_fixings() {
    let disc = DiscountCurve::builder("USD-OIS")
        .base_date(d(2025, 2, 15))
        .knots(vec![(0.0, 1.0), (0.5, 0.99), (1.0, 0.98), (5.0, 0.90)])
        .interp(InterpStyle::LogLinear)
        .day_count(finstack_quant_core::dates::DayCount::Act365F)
        .extrapolation(finstack_quant_core::math::interp::ExtrapolationPolicy::Extend)
        .build()
        .unwrap();
    let f3m = ForwardCurve::builder("USD-SOFR-3M")
        .base_date(d(2025, 2, 15))
        .day_count(finstack_quant_core::dates::DayCount::Act365F)
        .knots([
            (0.0, 1.0),
            (0.125, 0.997487536532938),
            (0.25, 0.994950292856076),
            (0.375, 0.992388490374784),
            (0.5, 0.98980235222721),
            (0.625, 0.98719210325196),
            (0.75, 0.984557969955606),
            (0.875, 0.981900180480004),
            (1.0, 0.97921896456946),
            (1.25, 0.973825219539481),
            (1.5, 0.968415788843127),
            (1.75, 0.962991265325316),
            (2.0, 0.957552240640482),
            (2.25, 0.95209930514862),
            (2.5, 0.946633047812227),
            (2.75, 0.941154056094142),
            (3.0, 0.935662915856331),
            (3.25, 0.930160211259623),
            (3.5, 0.924646524664431),
            (3.75, 0.91912243653247),
            (4.0, 0.913588525329506),
            (4.25, 0.90804536742914),
            (4.5, 0.90249353701767),
            (4.75, 0.896933606000028),
            (5.0, 0.891366143906831),
        ])
        .interp(finstack_quant_core::math::interp::InterpStyle::LogLinear)
        .extrapolation(finstack_quant_core::math::interp::ExtrapolationPolicy::Extend)
        .build()
        .unwrap();
    let f1m = ForwardCurve::builder("USD-SOFR-1M")
        .base_date(d(2025, 2, 15))
        .day_count(finstack_quant_core::dates::DayCount::Act365F)
        .knots([
            (0.0, 1.0),
            (0.125, 0.9976122302682),
            (0.25, 0.995199061524078),
            (0.375, 0.992760705844714),
            (0.5, 0.990297377149241),
            (0.625, 0.987809291167626),
            (0.75, 0.985296665409241),
            (0.875, 0.982759719131244),
            (1.0, 0.980198673306755),
            (1.25, 0.975043262181957),
            (1.5, 0.969869502539092),
            (1.75, 0.964677975480559),
            (2.0, 0.959469261503619),
            (2.25, 0.954243940395089),
            (2.5, 0.949002591126764),
            (2.75, 0.943745791751597),
            (3.0, 0.938474119300664),
            (3.25, 0.933188149680935),
            (3.5, 0.927888457573875),
            (3.75, 0.922575616334903),
            (4.0, 0.917250197893724),
            (4.25, 0.911912772655572),
            (4.5, 0.906563909403366),
            (4.75, 0.901204175200823),
            (5.0, 0.895834135296528),
        ])
        .interp(finstack_quant_core::math::interp::InterpStyle::LogLinear)
        .extrapolation(finstack_quant_core::math::interp::ExtrapolationPolicy::Extend)
        .build()
        .unwrap();

    let ctx = MarketContext::new()
        .try_insert(disc)
        .expect("insert market curve")
        .try_insert(f3m)
        .expect("insert market curve")
        .try_insert(f1m)
        .expect("insert market curve");

    let swap = BasisSwap::new(
        "SEASONED-NO-FIX",
        Money::new(10_000_000.0, USD).expect("valid money fixture"),
        make_leg("USD-SOFR-3M", d(2025, 1, 2), d(2026, 7, 2), Decimal::ZERO),
        make_leg("USD-SOFR-1M", d(2025, 1, 2), d(2026, 7, 2), Decimal::ZERO),
    )
    .expect("swap construction");

    let result = swap.value(&ctx, d(2025, 2, 15));
    assert!(
        result.is_err(),
        "Expected error for seasoned swap without fixings"
    );
    let err_msg = format!("{}", result.unwrap_err());
    assert!(
        err_msg.contains("fixing") || err_msg.contains("Seasoned"),
        "Error should mention fixings requirement: {}",
        err_msg
    );
}

#[test]
fn seasoned_swap_with_fixings_succeeds() {
    let disc = DiscountCurve::builder("USD-OIS")
        .base_date(d(2025, 1, 2))
        .knots(vec![(0.0, 1.0), (0.5, 0.99), (1.0, 0.98), (2.0, 0.96)])
        .interp(InterpStyle::LogLinear)
        .day_count(finstack_quant_core::dates::DayCount::Act365F)
        .extrapolation(finstack_quant_core::math::interp::ExtrapolationPolicy::Extend)
        .build()
        .unwrap();
    let f3m = ForwardCurve::builder("USD-SOFR-3M")
        .base_date(d(2025, 1, 2))
        .day_count(finstack_quant_core::dates::DayCount::Act365F)
        .knots([
            (0.0, 1.0),
            (0.125, 0.997487536532938),
            (0.25, 0.994950292856076),
            (0.375, 0.992388490374784),
            (0.5, 0.98980235222721),
            (0.625, 0.98719210325196),
            (0.75, 0.984557969955606),
            (0.875, 0.981900180480004),
            (1.0, 0.97921896456946),
            (1.125, 0.976522182587467),
            (1.25, 0.973817611559672),
            (1.375, 0.971105357458722),
            (1.5, 0.96838552632258),
            (1.625, 0.965658224247723),
            (1.75, 0.962923557382358),
            (1.875, 0.96018163191965),
            (2.0, 0.957432554090967),
        ])
        .interp(finstack_quant_core::math::interp::InterpStyle::LogLinear)
        .extrapolation(finstack_quant_core::math::interp::ExtrapolationPolicy::Extend)
        .build()
        .unwrap();
    let f1m = ForwardCurve::builder("USD-SOFR-1M")
        .base_date(d(2025, 1, 2))
        .day_count(finstack_quant_core::dates::DayCount::Act365F)
        .knots([
            (0.0, 1.0),
            (0.125, 0.9976122302682),
            (0.25, 0.995199061524078),
            (0.375, 0.992760705844714),
            (0.5, 0.990297377149241),
            (0.625, 0.987809291167626),
            (0.75, 0.985296665409241),
            (0.875, 0.982759719131244),
            (1.0, 0.980198673306755),
            (1.125, 0.97762138823012),
            (1.25, 0.975035644686228),
            (1.375, 0.972441545744155),
            (1.5, 0.969839194590702),
            (1.625, 0.96722869452364),
            (1.75, 0.964610148944963),
            (1.875, 0.961983661354156),
            (2.0, 0.959349335341472),
        ])
        .interp(finstack_quant_core::math::interp::InterpStyle::LogLinear)
        .extrapolation(finstack_quant_core::math::interp::ExtrapolationPolicy::Extend)
        .build()
        .unwrap();

    let fix_3m = finstack_quant_core::market_data::scalars::ScalarTimeSeries::new(
        "FIXING:USD-SOFR-3M",
        vec![(d(2024, 7, 2), 0.0195), (d(2024, 10, 2), 0.0198)],
        None,
        finstack_quant_core::market_data::scalars::SeriesInterpolation::Step,
        finstack_quant_core::market_data::scalars::SeriesExtrapolation::Error,
    )
    .expect("fixings series");
    let fix_1m = finstack_quant_core::market_data::scalars::ScalarTimeSeries::new(
        "FIXING:USD-SOFR-1M",
        vec![(d(2024, 7, 2), 0.0185), (d(2024, 10, 2), 0.0188)],
        None,
        finstack_quant_core::market_data::scalars::SeriesInterpolation::Step,
        finstack_quant_core::market_data::scalars::SeriesExtrapolation::Error,
    )
    .expect("fixings series");

    let ctx = MarketContext::new()
        .try_insert(disc)
        .expect("insert market curve")
        .try_insert(f3m)
        .expect("insert market curve")
        .try_insert(f1m)
        .expect("insert market curve")
        .insert_series(fix_3m)
        .insert_series(fix_1m);

    let swap = BasisSwap::new(
        "SEASONED-WITH-FIX",
        Money::new(10_000_000.0, USD).expect("valid money fixture"),
        make_leg(
            "USD-SOFR-3M",
            d(2024, 7, 2),
            d(2026, 7, 2),
            Decimal::from(5),
        ),
        make_leg("USD-SOFR-1M", d(2024, 7, 2), d(2026, 7, 2), Decimal::ZERO),
    )
    .expect("swap construction");

    let result = swap.value(&ctx, d(2025, 1, 2));
    assert!(
        result.is_ok(),
        "Expected success with fixings: {:?}",
        result
    );

    let npv = result.unwrap();
    assert!(npv.amount().is_finite(), "NPV should be finite");
}
