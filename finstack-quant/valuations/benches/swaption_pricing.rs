//! Swaption pricing benchmarks.
//!
//! Measures performance of swaption operations:
//! - Present value calculation (Black76 model)
//! - SABR-implied volatility pricing
//! - Greeks calculation (delta, vega, gamma)
//! - Forward swap rate and annuity calculation
//!
//! Market Standards Review (Week 5)

use criterion::{BenchmarkId, Criterion, criterion_group, criterion_main};
use finstack_quant_cashflows::builder::FloatingLegCompounding;
use finstack_quant_core::currency::Currency;
use finstack_quant_core::dates::{
    BusinessDayConvention, Date, DateExt, DayCount, DayCountContext, StubKind, Tenor,
};
use finstack_quant_core::market_data::context::MarketContext;
use finstack_quant_core::market_data::surfaces::VolSurface;
use finstack_quant_core::market_data::term_structures::{DiscountCurve, ForwardCurve};
use finstack_quant_core::math::interp::InterpStyle;
use finstack_quant_core::money::Money;
use finstack_quant_core::types::{CurveId, InstrumentId};
use finstack_quant_models::types::OptionType;
use finstack_quant_models::volatility::sabr::SabrParameters;
use finstack_quant_valuations::instruments::rates::swaption::{CashSettlementMethod, Swaption};
use finstack_quant_valuations::instruments::{FixedLegSpec, FloatLegSpec};
use finstack_quant_valuations::instruments::{Instrument, SettlementType, VolatilityModel};
use finstack_quant_valuations::metrics::MetricId;
use finstack_quant_valuations::pricer::ModelKey;
use rust_decimal::Decimal;
use std::hint::black_box;
use time::Month;

mod support;

fn create_swaption(expiry_months: i32, swap_tenor_years: i32) -> Swaption {
    let base = Date::from_calendar_date(2025, Month::January, 1).unwrap();
    let expiry = base.add_months(expiry_months).expect("valid expiry tenor");
    let swap_start = expiry;
    let swap_end = swap_start
        .add_months(12 * swap_tenor_years)
        .expect("valid underlying swap tenor");

    let fixed = FixedLegSpec {
        discount_curve_id: CurveId::new("USD-OIS"),
        rate: Decimal::try_from(0.04).expect("valid literal"),
        frequency: Tenor::semi_annual(),
        day_count: DayCount::Thirty360,
        business_day_convention: BusinessDayConvention::ModifiedFollowing,
        calendar_id: None,
        stub: StubKind::None,
        start: swap_start,
        end: swap_end,

        payment_lag_days: 0,
        end_of_month: false,
    };
    let float = FloatLegSpec {
        discount_curve_id: CurveId::new("USD-OIS"),
        forward_curve_id: CurveId::new("USD-SOFR-3M"),
        spread_bp: Decimal::ZERO,
        frequency: Tenor::quarterly(),
        day_count: DayCount::Act360,
        business_day_convention: BusinessDayConvention::ModifiedFollowing,
        calendar_id: None,
        stub: StubKind::None,
        reset_lag_days: 0,
        fixing_calendar_id: None,
        start: swap_start,
        end: swap_end,
        compounding: FloatingLegCompounding::Simple,
        payment_lag_days: 0,
        end_of_month: false,
    };
    Swaption::builder()
        .id(InstrumentId::new(format!(
            "SWAPTION-{}Mx{}Y",
            expiry_months, swap_tenor_years
        )))
        .option_type(OptionType::Call)
        .notional(Money::new(10_000_000.0, Currency::USD).expect("valid money fixture"))
        .expiry(expiry)
        .settlement(SettlementType::Physical)
        .cash_settlement_method(CashSettlementMethod::default())
        .vol_model(VolatilityModel::Black)
        .vol_surface_id(CurveId::new("SWAPTION-VOL"))
        .underlying_fixed_leg(fixed)
        .underlying_float_leg(float)
        .build()
        .expect("valid benchmark swaption")
}

fn create_swaption_with_sabr(expiry_months: i32, swap_tenor_years: i32) -> Swaption {
    let mut swaption = create_swaption(expiry_months, swap_tenor_years);
    swaption.sabr_params = Some(SabrParameters {
        alpha: 0.15,
        beta: 0.5,
        nu: 0.40,
        rho: -0.25,
        shift: None,
    });
    swaption
}

fn assert_swaption(swaption: &Swaption, expiry_months: i32, swap_tenor: i32, sabr: bool) {
    let base = Date::from_calendar_date(2025, Month::January, 1).unwrap();
    let expiry = base.add_months(expiry_months).unwrap();
    let maturity = expiry.add_months(12 * swap_tenor).unwrap();
    assert_eq!(swaption.expiry, expiry);
    assert_eq!(swaption.underlying_fixed_leg.start, expiry);
    assert_eq!(swaption.underlying_float_leg.start, expiry);
    assert_eq!(swaption.underlying_fixed_leg.end, maturity);
    assert_eq!(swaption.underlying_float_leg.end, maturity);
    assert_eq!(swaption.default_model(), ModelKey::Black76);
    assert_eq!(swaption.sabr_params.is_some(), sabr);
}

fn assert_usd(value: &Money) {
    assert!(value.amount().is_finite());
    assert!(value.amount() >= 0.0);
    assert_eq!(value.currency(), Currency::USD);
}

fn create_market() -> MarketContext {
    let base = Date::from_calendar_date(2025, Month::January, 1).unwrap();

    let disc = DiscountCurve::builder("USD-OIS")
        .base_date(base)
        .knots([
            (0.0, 1.0),
            (1.0, 0.98),
            (2.0, 0.96),
            (5.0, 0.88),
            (10.0, 0.70),
            (30.0, 0.40),
        ])
        .interp(InterpStyle::Linear)
        .build()
        .unwrap();

    let fwd = ForwardCurve::builder("USD-SOFR-3M", 0.25)
        .base_date(base)
        .knots([
            (0.0, 0.035),
            (1.0, 0.038),
            (2.0, 0.040),
            (5.0, 0.045),
            (10.0, 0.050),
            (30.0, 0.055),
        ])
        .interp(InterpStyle::Linear)
        .build()
        .unwrap();

    // Calendar expiries are not exact quarter-year fractions. The one-month
    // pillar also covers the rolled expiry used by the three-month theta case.
    let expiries = [1, 3, 6, 12, 24, 60].map(|months| {
        DayCount::Act365F
            .year_fraction(
                base,
                base.add_months(months).expect("valid surface expiry"),
                DayCountContext::default(),
            )
            .expect("surface expiry year fraction")
    });
    let vol_surface = VolSurface::from_grid(
        "SWAPTION-VOL",
        &expiries,
        &[0.02, 0.03, 0.04, 0.05, 0.06], // Strike rates
        &[0.20; 30],                     // Flat 20% vol
        Default::default(),
        None,
    )
    .unwrap();

    MarketContext::new()
        .insert(disc)
        .insert(fwd)
        .insert_surface(vol_surface)
}

fn bench_swaption_pv(c: &mut Criterion) {
    let mut group = c.benchmark_group("swaption_pv");
    let as_of = Date::from_calendar_date(2025, Month::January, 1).unwrap();

    for (expiry_months, swap_tenor) in [(3, 5), (6, 5), (12, 5), (12, 10)].iter() {
        group.bench_with_input(
            BenchmarkId::from_parameter(format!("{}Mx{}Y", expiry_months, swap_tenor)),
            &(expiry_months, swap_tenor),
            |b, _| {
                let market = create_market();
                let swaption = create_swaption(*expiry_months, *swap_tenor);
                assert_swaption(&swaption, *expiry_months, *swap_tenor, false);
                support::bench(
                    b,
                    || {
                        swaption
                            .value(black_box(&market), black_box(as_of))
                            .expect("Black76 swaption PV")
                    },
                    assert_usd,
                );
            },
        );
    }
    group.finish();
}

fn bench_swaption_sabr(c: &mut Criterion) {
    let mut group = c.benchmark_group("swaption_sabr");
    let as_of = Date::from_calendar_date(2025, Month::January, 1).unwrap();

    for (expiry_months, swap_tenor) in [(3, 5), (6, 5), (12, 5), (12, 10)].iter() {
        group.bench_with_input(
            BenchmarkId::from_parameter(format!("{}Mx{}Y", expiry_months, swap_tenor)),
            &(expiry_months, swap_tenor),
            |b, _| {
                let market = create_market();
                let swaption = create_swaption_with_sabr(*expiry_months, *swap_tenor);
                assert_swaption(&swaption, *expiry_months, *swap_tenor, true);
                support::bench(
                    b,
                    || {
                        swaption
                            .value(black_box(&market), black_box(as_of))
                            .expect("SABR swaption PV")
                    },
                    assert_usd,
                );
            },
        );
    }
    group.finish();
}

fn bench_swaption_greeks(c: &mut Criterion) {
    let mut group = c.benchmark_group("swaption_greeks");
    let as_of = Date::from_calendar_date(2025, Month::January, 1).unwrap();

    let greeks = vec![
        MetricId::Delta,
        MetricId::Gamma,
        MetricId::Vega,
        MetricId::Theta,
    ];

    for (expiry_months, swap_tenor) in [(3, 5), (6, 5), (12, 10)].iter() {
        group.bench_with_input(
            BenchmarkId::from_parameter(format!("{}Mx{}Y", expiry_months, swap_tenor)),
            &(expiry_months, swap_tenor),
            |b, _| {
                let market = create_market();
                let swaption = create_swaption(*expiry_months, *swap_tenor);
                assert_swaption(&swaption, *expiry_months, *swap_tenor, false);
                support::bench(
                    b,
                    || {
                        swaption
                            .price_with_metrics(
                                black_box(&market),
                                black_box(as_of),
                                black_box(&greeks),
                                finstack_quant_valuations::instruments::PricingOptions::default(),
                            )
                            .expect("swaption Greeks")
                    },
                    |result| {
                        assert_usd(&result.value);
                        assert!(result.measures.values().all(|value| value.is_finite()));
                        for metric in &greeks {
                            assert!(
                                result.measures.contains_key(metric),
                                "missing metric {metric}"
                            );
                        }
                    },
                );
            },
        );
    }
    group.finish();
}

fn bench_forward_swap_rate(c: &mut Criterion) {
    let mut group = c.benchmark_group("swaption_forward_rate");
    let as_of = Date::from_calendar_date(2025, Month::January, 1).unwrap();

    for (expiry_months, swap_tenor) in [(3, 5), (6, 5), (12, 5), (12, 10)].iter() {
        group.bench_with_input(
            BenchmarkId::from_parameter(format!("{}Mx{}Y", expiry_months, swap_tenor)),
            &(expiry_months, swap_tenor),
            |b, _| {
                let market = create_market();
                let swaption = create_swaption(*expiry_months, *swap_tenor);
                assert_swaption(&swaption, *expiry_months, *swap_tenor, false);
                support::bench(
                    b,
                    || {
                        swaption
                            .forward_swap_rate(black_box(&market), black_box(as_of))
                            .expect("forward swap rate")
                    },
                    |rate| assert!(rate.is_finite() && *rate > 0.0),
                );
            },
        );
    }
    group.finish();
}

fn bench_swap_annuity(c: &mut Criterion) {
    let mut group = c.benchmark_group("swaption_annuity");
    let as_of = Date::from_calendar_date(2025, Month::January, 1).unwrap();

    for (expiry_months, swap_tenor) in [(3, 5), (6, 5), (12, 5), (12, 10)].iter() {
        group.bench_with_input(
            BenchmarkId::from_parameter(format!("{}Mx{}Y", expiry_months, swap_tenor)),
            &(expiry_months, swap_tenor),
            |b, _| {
                let market = create_market();
                let swaption = create_swaption(*expiry_months, *swap_tenor);
                assert_swaption(&swaption, *expiry_months, *swap_tenor, false);
                let disc = market
                    .get_discount(swaption.get_discount_curve_id())
                    .unwrap();
                support::bench(
                    b,
                    || {
                        swaption
                            .swap_annuity(black_box(disc.as_ref()), black_box(as_of))
                            .expect("swap annuity")
                    },
                    |annuity| assert!(annuity.is_finite() && *annuity > 0.0),
                );
            },
        );
    }
    group.finish();
}

criterion_group!(
    benches,
    bench_swaption_pv,
    bench_swaption_sabr,
    bench_swaption_greeks,
    bench_forward_swap_rate,
    bench_swap_annuity
);
criterion_main!(benches);
