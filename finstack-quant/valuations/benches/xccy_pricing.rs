//! Cross-currency swap (XCCY) pricing benchmarks.
//!
//! The XCCY swap pricer iterates over `n_periods` payment dates on both legs,
//! performing forward-rate projection, discount-factor lookup, and FX conversion
//! per cashflow. With 4Q/year coupon frequency on a 10Y tenor, that is 40 cash
//! flows per leg × 2 legs + 2 principal exchanges — comparable to a long IRS but
//! with two full curve lookups plus FxMatrix queries in the hot loop.
//!
//! No existing `rates_pricing.rs` bench covers XccySwap.
//!
//! Scenarios:
//! - Representative 5Y tenor (coupon count ≈ 20 per leg)
//! - Notional exchange: None / Final / Initial+Final
//! - Reporting in USD (one leg local, one converted via FxMatrix)

#![allow(clippy::unwrap_used)]

mod support;

use criterion::{BenchmarkId, Criterion, Throughput, criterion_group, criterion_main};
use finstack_quant_core::currency::Currency;
use finstack_quant_core::dates::{BusinessDayConvention, Date, DayCount, StubKind, Tenor};
use finstack_quant_core::market_data::context::MarketContext;
use finstack_quant_core::market_data::term_structures::{DiscountCurve, ForwardCurve};
use finstack_quant_core::math::interp::InterpStyle;
use finstack_quant_core::money::Money;
use finstack_quant_core::money::fx::{FxMatrix, SimpleFxProvider};
use finstack_quant_core::types::CurveId;
use finstack_quant_valuations::instruments::FloatLegSpec;
use finstack_quant_valuations::instruments::Instrument;
use finstack_quant_valuations::instruments::NotionalExchange;
use finstack_quant_valuations::instruments::PayReceive;
use finstack_quant_valuations::instruments::rates::xccy_swap::{XccySwap, XccySwapLeg};
use rust_decimal::Decimal;
use std::hint::black_box;
use std::sync::Arc;
use time::Month;

// Market context — 2 discount + 2 forward curves + FxMatrix

fn base_date() -> Date {
    Date::from_calendar_date(2025, Month::January, 2).unwrap()
}

fn create_market() -> MarketContext {
    let base = base_date();

    let usd_disc = DiscountCurve::builder("USD-OIS")
        .base_date(base)
        .knots(vec![(0.0, 1.0), (5.0, 0.80), (10.0, 0.63), (20.0, 0.40)])
        .interp(InterpStyle::LogLinear)
        .day_count(finstack_quant_core::dates::DayCount::Act365F)
        .extrapolation(finstack_quant_core::math::interp::ExtrapolationPolicy::Extend)
        .build()
        .unwrap();

    let eur_disc = DiscountCurve::builder("EUR-OIS")
        .base_date(base)
        .knots(vec![(0.0, 1.0), (5.0, 0.86), (10.0, 0.74), (20.0, 0.54)])
        .interp(InterpStyle::LogLinear)
        .day_count(finstack_quant_core::dates::DayCount::Act365F)
        .extrapolation(finstack_quant_core::math::interp::ExtrapolationPolicy::Extend)
        .build()
        .unwrap();

    let usd_fwd = ForwardCurve::builder("USD-SOFR-3M")
        .base_date(base)
        .day_count(finstack_quant_core::dates::DayCount::Act365F)
        .knots([
            (0.0, 1.0),
            (0.3125, 0.986055173794316),
            (0.625, 0.972342787164834),
            (0.9375, 0.958858543883084),
            (1.25, 0.945598233933184),
            (1.5625, 0.932557731662988),
            (1.875, 0.919732993977195),
            (2.1875, 0.907120058571427),
            (2.5, 0.894715042206304),
            (2.8125, 0.882514139020548),
            (3.125, 0.870513618882206),
            (3.4375, 0.858709825777076),
            (3.75, 0.847099176233458),
            (4.0625, 0.835678157782365),
            (4.375, 0.824443327452359),
            (4.6875, 0.813391310298198),
            (5.0, 0.802518797962478),
            (5.3125, 0.791814814665208),
            (5.625, 0.781268859939772),
            (5.9375, 0.770878419836864),
            (6.25, 0.7606410229172),
            (6.5625, 0.750554239494363),
            (6.875, 0.740615680891749),
            (7.1875, 0.730822998713365),
            (7.5, 0.721173884128187),
            (7.8125, 0.711666067167831),
            (8.125, 0.702297316037272),
            (8.4375, 0.693065436438362),
            (8.75, 0.683968270905898),
            (9.0625, 0.67500369815599),
            (9.375, 0.666169632446515),
            (9.6875, 0.657464022949387),
            (10.0, 0.648884853134453),
            (10.625, 0.632085588760813),
            (11.25, 0.61574529998679),
            (11.875, 0.599850861547898),
            (12.5, 0.584389538810941),
            (13.125, 0.569348975726335),
            (13.75, 0.554717183164189),
            (14.375, 0.540482527621543),
            (15.0, 0.52663372028861),
            (15.625, 0.513159806462254),
            (16.25, 0.500050155295358),
            (16.875, 0.487294449871085),
            (17.5, 0.474882677591444),
            (18.125, 0.462805120869889),
            (18.75, 0.451052348118056),
            (19.375, 0.439615205017046),
            (20.0, 0.428484806064017),
        ])
        .interp(finstack_quant_core::math::interp::InterpStyle::LogLinear)
        .extrapolation(finstack_quant_core::math::interp::ExtrapolationPolicy::Extend)
        .build()
        .unwrap();

    let eur_fwd = ForwardCurve::builder("EUR-EURIBOR-3M")
        .base_date(base)
        .day_count(finstack_quant_core::dates::DayCount::Act365F)
        .knots([
            (0.0, 1.0),
            (0.3125, 0.989131756417234),
            (0.625, 0.978400740755897),
            (0.9375, 0.967805047330097),
            (1.25, 0.957342798996357),
            (1.5625, 0.947012146698642),
            (1.875, 0.936811269021055),
            (2.1875, 0.926738371748034),
            (2.5, 0.916791687431945),
            (2.8125, 0.906969474967915),
            (3.125, 0.897270019175806),
            (3.4375, 0.887691630389179),
            (3.75, 0.878232644051139),
            (4.0625, 0.868891420316941),
            (4.375, 0.859666343663227),
            (4.6875, 0.850555822503796),
            (5.0, 0.841558288811773),
            (5.3125, 0.83267219774808),
            (5.625, 0.82389602729609),
            (5.9375, 0.815228277902361),
            (6.25, 0.806667472123344),
            (6.5625, 0.798212154277953),
            (6.875, 0.789860890105908),
            (7.1875, 0.781612266431739),
            (7.5, 0.773464890834361),
            (7.8125, 0.765417391322109),
            (8.125, 0.757468416013165),
            (8.4375, 0.74961663282125),
            (8.75, 0.741860729146516),
            (9.0625, 0.734199411571537),
            (9.375, 0.726631405562309),
            (9.6875, 0.719155455174179),
            (10.0, 0.71177032276261),
            (10.625, 0.697254032713597),
            (11.25, 0.683060478510428),
            (11.875, 0.669181992819426),
            (12.5, 0.65561110078691),
            (13.125, 0.642340514972244),
            (13.75, 0.629363130419975),
            (14.375, 0.616672019867092),
            (15.0, 0.60426042908157),
            (15.625, 0.592121772328448),
            (16.25, 0.580249627959829),
            (16.875, 0.568637734125288),
            (17.5, 0.557279984599259),
            (18.125, 0.546170424722101),
            (18.75, 0.535303247451624),
            (19.375, 0.52467278952194),
            (20.0, 0.514273527706632),
        ])
        .interp(finstack_quant_core::math::interp::InterpStyle::LogLinear)
        .extrapolation(finstack_quant_core::math::interp::ExtrapolationPolicy::Extend)
        .build()
        .unwrap();

    let provider = Arc::new(SimpleFxProvider::new());
    provider
        .set_quotes(&[(Currency::EUR, Currency::USD, 1.08)])
        .expect("valid fx");
    let fx = FxMatrix::new(provider, None);

    MarketContext::new()
        .try_insert(usd_disc)
        .expect("insert market curve")
        .try_insert(eur_disc)
        .expect("insert market curve")
        .try_insert(usd_fwd)
        .expect("insert market curve")
        .try_insert(eur_fwd)
        .expect("insert market curve")
        .insert_fx(fx)
}

// Instrument builder

fn make_xccy(base: Date, maturity: Date, exchange: NotionalExchange) -> XccySwap {
    let usd_notional = 10_000_000.0_f64;
    let eur_notional = usd_notional / 1.08;

    let leg_usd = XccySwapLeg {
        notional: Money::new(usd_notional, Currency::USD).expect("valid money fixture"),
        side: PayReceive::Receive,
        leg: FloatLegSpec {
            index_id: "USD-SOFR-3M".into(),
            forward_curve_id: CurveId::from("USD-SOFR-3M"),
            discount_curve_id: CurveId::from("USD-OIS"),
            start: base,
            end: maturity,
            frequency: Tenor::quarterly(),
            day_count: DayCount::Act360,
            business_day_convention: BusinessDayConvention::ModifiedFollowing,
            stub: StubKind::ShortFront,
            spread_bp: Decimal::ZERO,
            payment_lag_days: 0,
            calendar_id: None,
            reset_lag_days: 0,
            compounding: Default::default(),
            fixing_calendar_id: None,
            end_of_month: false,
        },
    };

    let leg_eur = XccySwapLeg {
        notional: Money::new(eur_notional, Currency::EUR).expect("valid money fixture"),
        side: PayReceive::Pay,
        leg: FloatLegSpec {
            index_id: "EUR-EURIBOR-3M".into(),
            forward_curve_id: CurveId::from("EUR-EURIBOR-3M"),
            discount_curve_id: CurveId::from("EUR-OIS"),
            start: base,
            end: maturity,
            frequency: Tenor::quarterly(),
            day_count: DayCount::Act360,
            business_day_convention: BusinessDayConvention::ModifiedFollowing,
            stub: StubKind::ShortFront,
            spread_bp: Decimal::ZERO,
            payment_lag_days: 0,
            calendar_id: None,
            reset_lag_days: 0,
            compounding: Default::default(),
            fixing_calendar_id: None,
            end_of_month: false,
        },
    };

    let mut swap = XccySwap::new("XCCY-BENCH", leg_usd, leg_eur, Currency::USD)
        .with_notional_exchange(exchange);
    swap.allow_calendar_fallback = true;
    swap
}

// Benchmark: tenor scaling (InitialAndFinal exchange)

fn bench_xccy_tenor(c: &mut Criterion) {
    let mut group = c.benchmark_group("xccy_tenor");
    let label = "5Y";
    let years = 5;
    let n_periods = years as u64 * 4 * 2; // 4 Q/yr × 2 legs

    group.throughput(Throughput::Elements(n_periods));
    group.bench_with_input(BenchmarkId::from_parameter(label), label, |b, _| {
        let base = base_date();
        let market = create_market();
        let maturity = Date::from_calendar_date(2025 + years, Month::January, 2).unwrap();
        let swap = make_xccy(base, maturity, NotionalExchange::InitialAndFinal);
        support::bench(
            b,
            || {
                black_box(&swap)
                    .value(black_box(&market), black_box(base))
                    .expect("XCCY PV should calculate successfully")
            },
            |value| {
                assert!(value.amount().is_finite());
                assert_eq!(value.currency(), Currency::USD);
                assert_eq!(swap.notional_exchange, NotionalExchange::InitialAndFinal);
                for leg in [&swap.leg1, &swap.leg2] {
                    assert_eq!(leg.leg.start, base);
                    assert_eq!(leg.leg.end, maturity);
                    assert_eq!(leg.leg.frequency, Tenor::quarterly());
                    assert!(!leg.leg.compounding.is_overnight());
                }
            },
        );
    });

    group.finish();
}

// Benchmark: notional exchange convention (5Y swap)

fn bench_xccy_notional_exchange(c: &mut Criterion) {
    let mut group = c.benchmark_group("xccy_notional_exchange");
    for (label, exchange) in [
        ("none", NotionalExchange::None),
        ("final", NotionalExchange::Final),
        ("initial_and_final", NotionalExchange::InitialAndFinal),
    ] {
        group.bench_function(label, |b| {
            let base = base_date();
            let maturity = Date::from_calendar_date(2030, Month::January, 2).unwrap();
            let market = create_market();
            let swap = make_xccy(base, maturity, exchange);
            support::bench(
                b,
                || {
                    black_box(&swap)
                        .value(black_box(&market), black_box(base))
                        .expect("XCCY PV should calculate successfully")
                },
                |value| {
                    assert!(value.amount().is_finite());
                    assert_eq!(value.currency(), Currency::USD);
                    assert_eq!(swap.notional_exchange, exchange);
                    for leg in [&swap.leg1, &swap.leg2] {
                        assert_eq!(leg.leg.start, base);
                        assert_eq!(leg.leg.end, maturity);
                        assert_eq!(leg.leg.frequency, Tenor::quarterly());
                        assert!(!leg.leg.compounding.is_overnight());
                    }
                },
            );
        });
    }

    group.finish();
}

criterion_group!(benches, bench_xccy_tenor, bench_xccy_notional_exchange);
criterion_main!(benches);
