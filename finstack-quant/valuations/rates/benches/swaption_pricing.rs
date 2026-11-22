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
use finstack_quant_core::market_data::surfaces::{
    VolGridOpts, VolInterpolationMode, VolQuoteType, VolSurface, VolSurfaceAxis,
};
use finstack_quant_core::market_data::term_structures::{DiscountCurve, ForwardCurve};
use finstack_quant_core::math::interp::InterpStyle;
use finstack_quant_core::money::Money;
use finstack_quant_core::types::{CurveId, InstrumentId};
use finstack_quant_models::types::OptionType;
use finstack_quant_models::volatility::sabr::SabrParameters;
use finstack_quant_valuations_core::instruments::{Instrument, SettlementType};
use finstack_quant_valuations_core::metrics::MetricId;
use finstack_quant_valuations_core::pricer::ModelKey;
use finstack_quant_valuations_rates::VolatilityModel;
use finstack_quant_valuations_rates::swaption::{CashSettlementMethod, Swaption};
use finstack_quant_valuations_rates::{FixedLegSpec, FloatLegSpec};
use rust_decimal::Decimal;
use std::hint::black_box;
use time::Month;

#[path = "../../benches/support/mod.rs"]
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
        index_id: "USD-SOFR-3M".into(),
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
        .day_count(finstack_quant_core::dates::DayCount::Act365F)
        .extrapolation(finstack_quant_core::math::interp::ExtrapolationPolicy::Extend)
        .build()
        .unwrap();

    let fwd = ForwardCurve::builder("USD-SOFR-3M")
        .base_date(base)
        .day_count(finstack_quant_core::dates::DayCount::Act365F)
        .knots([
            (0.0, 1.0),
            (0.125, 0.995611221459588),
            (0.25, 0.991195240930358),
            (0.375, 0.986752592135641),
            (0.5, 0.982283810160798),
            (0.625, 0.977789431347014),
            (0.75, 0.973269993185277),
            (0.875, 0.968726034210573),
            (1.0, 0.964158093896324),
            (1.125, 0.959574209193318),
            (1.25, 0.954982273933355),
            (1.375, 0.950382612946264),
            (1.5, 0.945775550224929),
            (1.625, 0.941161408888927),
            (1.75, 0.936540511148462),
            (1.875, 0.93191317826863),
            (2.0, 0.927279730533999),
            (2.1875, 0.920324184202576),
            (2.375, 0.913367292399534),
            (2.5625, 0.906409877451143),
            (2.75, 0.899452755535353),
            (2.9375, 0.892496736538075),
            (3.125, 0.885542623912392),
            (3.3125, 0.878591214540736),
            (3.5, 0.871643298600053),
            (3.6875, 0.864699659429976),
            (3.875, 0.857761073404031),
            (4.0625, 0.850828309803906),
            (4.25, 0.843902130696782),
            (4.4375, 0.836983290815769),
            (4.625, 0.830072537443442),
            (4.8125, 0.823170610298508),
            (5.0, 0.81627824142561),
            (5.3125, 0.804840362948709),
            (5.625, 0.793485262077054),
            (5.9375, 0.782213972982446),
            (6.25, 0.771027490817996),
            (6.5625, 0.759926771886431),
            (6.875, 0.74891273382482),
            (7.1875, 0.737986255805403),
            (7.5, 0.727148178752154),
            (7.8125, 0.71639930557274),
            (8.125, 0.705740401405505),
            (8.4375, 0.695172193881119),
            (8.75, 0.684695373398532),
            (9.0625, 0.674310593414836),
            (9.375, 0.664018470748686),
            (9.6875, 0.653819585896884),
            (10.0, 0.643714483363737),
            (11.25, 0.604595697764404),
            (12.5, 0.567632401630785),
            (13.75, 0.532720803704493),
            (15.0, 0.499761147348737),
            (16.25, 0.468657613416832),
            (17.5, 0.439318221663474),
            (18.75, 0.411654731012661),
            (20.0, 0.385582538979713),
            (21.25, 0.361020580528505),
            (22.5, 0.337891226628893),
            (23.75, 0.316120182763365),
            (25.0, 0.29563638761626),
            (26.25, 0.276371912163538),
            (27.5, 0.258261859365997),
            (28.75, 0.241244264654134),
            (30.0, 0.225259997378556),
        ])
        .interp(finstack_quant_core::math::interp::InterpStyle::LogLinear)
        .extrapolation(finstack_quant_core::math::interp::ExtrapolationPolicy::Extend)
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
        VolGridOpts {
            secondary_axis: VolSurfaceAxis::Strike,
            quote_type: VolQuoteType::BlackLognormal,
            interpolation_mode: VolInterpolationMode::Vol,
        },
        None,
    )
    .unwrap();

    MarketContext::new()
        .try_insert(disc)
        .expect("insert market curve")
        .try_insert(fwd)
        .expect("insert market curve")
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
                                finstack_quant_valuations_core::instruments::PricingOptions::default(),
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
