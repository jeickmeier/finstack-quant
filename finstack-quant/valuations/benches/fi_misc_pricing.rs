//! Fixed income misc pricing benchmarks.
//!
//! Covers:
//! - [`TermLoan`]: fixed/floating coupon schedule with amortization.
//! - [`RevolvingCredit`]: deterministic draw/repay schedule, floating coupon.
//! - [`AgencyMbsPassthrough`] / [`AgencyTba`]: prepayment-adjusted cashflows.
//! - [`FiIndexTotalReturnSwap`]: carry-based TRS with yield/duration market data.
//! - [`AgencyCmo`]: sequential and PAC waterfall allocation across collateral pools.
//! - [`DollarRoll`]: front/back month settlement NPV and implied financing rate.

#![allow(clippy::unwrap_used)]

#[path = "support/record.rs"]
mod record;
mod support;

use criterion::{BenchmarkId, Criterion, criterion_group, criterion_main};
use finstack_quant_cashflows::builder::CouponType;
use finstack_quant_cashflows::builder::{AmortizationSpec, PrepaymentModelSpec, RateSpec};
use finstack_quant_core::currency::Currency;
use finstack_quant_core::dates::{
    BusinessDayConvention, Date, DateExt, DayCount, DayCountContext, StubKind, Tenor,
};
use finstack_quant_core::market_data::context::MarketContext;
use finstack_quant_core::market_data::scalars::{MarketScalar, ScalarTimeSeries};
use finstack_quant_core::market_data::term_structures::DiscountCurve;
use finstack_quant_core::math::interp::InterpStyle;
use finstack_quant_core::money::Money;
use finstack_quant_core::types::{CurveId, InstrumentId};
use finstack_quant_valuations::instruments::Instrument;
use finstack_quant_valuations::instruments::TrsScheduleSpec;
use finstack_quant_valuations::instruments::common_impl::helpers::resolve_mc_paths;
use finstack_quant_valuations::instruments::fixed_income::cmo::AgencyCmo;
use finstack_quant_valuations::instruments::fixed_income::dollar_roll::DollarRoll;
use finstack_quant_valuations::instruments::fixed_income::fi_trs::FiIndexTotalReturnSwap;
use finstack_quant_valuations::instruments::fixed_income::mbs_passthrough::{
    AgencyMbsPassthrough, AgencyProgram, PoolType,
};
use finstack_quant_valuations::instruments::fixed_income::revolving_credit::{
    DrawRepaySpec, RevolvingCredit, RevolvingCreditFees,
};
use finstack_quant_valuations::instruments::fixed_income::tba::{AgencyTba, TbaTerm};
use finstack_quant_valuations::instruments::fixed_income::term_loan::TermLoan;
use finstack_quant_valuations::metrics::MetricId;
use rust_decimal::Decimal;
use serde_json::json;
use std::hint::black_box;
use time::Month;

use finstack_quant_valuations_test_support::discount_forward_curves as discount_forward_curve_support;

fn check_money(value: &Money, currency: Currency) {
    assert_eq!(value.currency(), currency);
    assert!(value.amount().is_finite());
}

fn check_valuation(
    result: &finstack_quant_valuations::results::ValuationResult,
    currency: Currency,
    metrics: &[MetricId],
) {
    check_money(&result.value, currency);
    assert!(result.measures.values().all(|value| value.is_finite()));
    for metric in metrics {
        let prefix = format!("{}::", metric.as_str());
        assert!(
            result.measures.contains_key(metric)
                || result
                    .measures
                    .keys()
                    .any(|key| key.as_str().starts_with(&prefix)),
            "missing requested metric {metric}"
        );
    }
}

/// Build the capture payload only inside the lazy `record::capture` closure.
fn mbs_fixture_record(
    mbs: &AgencyMbsPassthrough,
    market: &MarketContext,
    as_of: Date,
    metrics: &[MetricId],
) -> serde_json::Value {
    let oas_requested = metrics.contains(&MetricId::Oas);
    let model = &mbs.instrument_pricing_overrides.model_config;
    let curve = market
        .get_discount(&mbs.discount_curve_id)
        .expect("captured discount curve");
    let quote_accrual_start = as_of
        .replace_day(1)
        .expect("settlement month")
        .max(mbs.issue_date.replace_day(1).expect("issue month"));
    // These are contractual calendar clocks, not cashflow projections or an
    // independent pricing reference. Holder PV can include earlier in-flight
    // accrual; this calendar is explicitly the quote-basis OAS clock.
    let oas_payment_clocks = oas_requested.then(|| {
        (0..mbs.wam_months)
            .map(|month| {
                let accrual_start = quote_accrual_start
                    .add_months(i32::try_from(month).expect("bounded WAM"))
                    .expect("accrual start");
                let accrual_end = accrual_start.add_months(1).expect("accrual end");
                let period_end = accrual_end - time::Duration::days(1);
                let payment_date = mbs
                    .payment_date_for_accrual_period(accrual_start)
                    .expect("agency payment date");
                let tau = curve
                    .day_count()
                    .year_fraction(as_of, payment_date, DayCountContext::default())
                    .expect("payment time");
                let grid_end = f64::from(month + 1) / 12.0;
                json!({
                    "accrual_start": accrual_start.to_string(),
                    "accrual_end": accrual_end.to_string(),
                    "period_end": period_end.to_string(),
                    "payment_date": payment_date.to_string(),
                    "grid_end_years": grid_end,
                    "payment_time_years": tau,
                    "payment_extra_years": tau - grid_end,
                    "accrual_fraction": mbs.day_count
                        .year_fraction(accrual_start, accrual_end, DayCountContext::default())
                        .expect("pool accrual fraction"),
                    "seasoning_at_period_end_months": mbs.seasoning_months(period_end),
                })
            })
            .collect::<Vec<_>>()
    });
    // Match the canonical resolver defaults for the retained original row;
    // every added row supplies these settings explicitly. Non-MC controls
    // report null MC dimensions rather than claiming simulated work.
    let paths = oas_requested
        .then(|| resolve_mc_paths(model.mc_paths, 512).expect("resolved MBS path count"));
    let metric_engine = if oas_requested {
        "hull_white_1f_mc_oas"
    } else if metrics.is_empty() {
        "deterministic_mbs_cashflow_discounting"
    } else {
        "deterministic_rate_dependent_prepayment_effective_risk"
    };
    json!({
        "instrument": mbs,
        "market": market,
        "as_of": as_of.to_string(),
        "model": {
            "pricing_dispatch": format!("{:?}", mbs.default_model()),
            "metric_engine": metric_engine,
        },
        "requested_metrics": metrics,
        "dimensions": {
            "wam_months": mbs.wam_months,
            "seasoning_at_as_of_months": mbs.seasoning_months(as_of),
            "physical_paths": paths,
            "independent_estimators": paths,
            "resolved_seed": oas_requested.then(|| model.mc_seed(&mbs.id)),
            "seed_scenario": oas_requested.then(|| model.mc_seed_scenario.as_deref().unwrap_or("base")),
            "rng": oas_requested.then_some("Philox4x32-10 independent path substreams"),
            "hw1f_mean_reversion": oas_requested.then(|| model.hw1f_mean_reversion.unwrap_or(0.05)),
            "hw1f_sigma": oas_requested.then(|| model.hw1f_sigma.unwrap_or(0.01)),
            "simulation_grid_step_years": oas_requested.then_some(1.0 / 12.0),
            "simulation_steps": oas_requested.then_some(mbs.wam_months),
            "rate_values_per_path": oas_requested.then_some(mbs.wam_months + 1),
            "oas_solver": oas_requested.then(|| json!({
                "tolerance_decimal_spread": 1e-7,
                "max_iterations": 200,
                "bracket": [-0.10, 0.20],
                "initial_bracket_size": 0.05,
            })),
            "pool_day_count": mbs.day_count,
            "discount_curve_day_count": curve.day_count(),
            "payment_calendar": "usny",
            "payment_roll_convention": BusinessDayConvention::Following,
            "effective_stated_delay_days": mbs.effective_stated_delay_days(),
            "holder_projection_policy": "earliest unpaid accrual, including in-flight receivables",
            "quote_basis_accrual_start": quote_accrual_start.to_string(),
            "oas_payment_clocks": oas_payment_clocks,
        },
    })
}

fn as_of() -> Date {
    Date::from_calendar_date(2025, Month::January, 1).unwrap()
}

fn term_loan(maturity: Date) -> TermLoan {
    let base = as_of();
    TermLoan::builder()
        .id("TL-BENCH".into())
        .currency(Currency::USD)
        .notional_limit(Money::new(10_000_000.0, Currency::USD).expect("valid money fixture"))
        .issue_date(base)
        .maturity(maturity)
        .rate(RateSpec::Fixed { rate: 0.05 })
        .frequency(Tenor::semi_annual())
        .day_count(DayCount::Act360)
        .yield_basis(finstack_quant_valuations_core::metrics::returns::YieldBasis::Act360)
        .business_day_convention(BusinessDayConvention::ModifiedFollowing)
        .calendar_id_opt(None)
        .stub(StubKind::None)
        .discount_curve_id(CurveId::from("USD-OIS"))
        .amortization(AmortizationSpec::None)
        .coupon_type(CouponType::Cash)
        .upfront_fee_opt(None)
        .ddtl_opt(None)
        .covenants_opt(None)
        .instrument_pricing_overrides(Default::default())
        .attributes(Default::default())
        .build()
        .unwrap()
}

fn revolving_credit_floating(maturity: Date) -> RevolvingCredit {
    RevolvingCredit::builder()
        .id("RC-BENCH".into())
        .commitment(Money::new(10_000_000.0, Currency::USD).expect("valid money fixture"))
        .drawn(Money::new(5_000_000.0, Currency::USD).expect("valid money fixture"))
        .issue_date(as_of())
        .maturity(maturity)
        .rate(RateSpec::Floating(
            finstack_quant_cashflows::builder::FloatingRateSpec {
                forward_curve_id: "USD-SOFR-3M".into(),
                spread_bp: Decimal::try_from(100.0).unwrap(),
                gearing: Decimal::try_from(1.0).unwrap(),
                gearing_includes_spread: true,
                index_floor_bp: None,
                all_in_floor_bp: None,
                all_in_cap_bp: None,
                index_cap_bp: None,
                overnight_index_constraints: Default::default(),
                reset_frequency: Tenor::quarterly(),
                index_tenor: None,
                reset_lag_days: 2,
                fixing_calendar_id: None,
                compounding: None,
                overnight_basis: None,
                fallback: Default::default(),
            },
        ))
        .day_count(DayCount::Act360)
        .yield_basis(finstack_quant_valuations_core::metrics::returns::YieldBasis::Act360)
        .frequency(Tenor::quarterly())
        .fees(RevolvingCreditFees::flat(25.0, 10.0, 0.0).unwrap())
        .draw_repay_spec(DrawRepaySpec::Deterministic(vec![]))
        .discount_curve_id("USD-OIS".into())
        .recovery_rate(0.40)
        .build()
        .unwrap()
}

fn revolving_credit_market(as_of: Date) -> MarketContext {
    let disc = discount_forward_curve_support::flat_discount("USD-OIS", as_of, 0.05);
    let fwd = finstack_quant_core::market_data::term_structures::ForwardCurve::builder(
        "USD-SOFR-3M",
        0.25,
    )
    .base_date(as_of)
    .knots([
        (0.0, 0.04),
        (1.0, 0.045),
        (3.0, 0.048),
        (5.0, 0.05),
        (7.0, 0.052),
    ])
    .build()
    .unwrap();
    let fixings = ScalarTimeSeries::new(
        "FIXING:USD-SOFR-3M",
        vec![(
            Date::from_calendar_date(2024, Month::December, 30).unwrap(),
            0.04,
        )],
        None,
    )
    .expect("revolver reset fixing");
    MarketContext::new()
        .insert(disc)
        .insert(fwd)
        .insert_series(fixings)
}

fn bench_term_loan_pv(c: &mut Criterion) {
    let mut group = c.benchmark_group("term_loan_pv");
    for years in [3, 5, 7] {
        group.bench_with_input(
            BenchmarkId::from_parameter(format!("{years}Y")),
            &years,
            |b, years| {
                let as_of = as_of();
                let market = MarketContext::new().insert(
                    discount_forward_curve_support::flat_discount("USD-OIS", as_of, 0.05),
                );
                let maturity = Date::from_calendar_date(2025 + years, Month::January, 1).unwrap();
                let instrument = term_loan(maturity);
                support::bench(
                    b,
                    || {
                        instrument
                            .value(black_box(&market), black_box(as_of))
                            .expect("term_loan_pv")
                    },
                    |value| check_money(value, Currency::USD),
                );
            },
        );
    }
    group.finish();
}

fn bench_revolving_credit_pv(c: &mut Criterion) {
    let mut group = c.benchmark_group("revolving_credit_pv");
    for years in [3, 5] {
        group.bench_with_input(
            BenchmarkId::from_parameter(format!("{years}Y")),
            &years,
            |b, years| {
                let as_of = as_of();
                let market = revolving_credit_market(as_of);
                let maturity = Date::from_calendar_date(2025 + years, Month::January, 1).unwrap();
                let instrument = revolving_credit_floating(maturity);
                support::bench(
                    b,
                    || {
                        instrument
                            .value(black_box(&market), black_box(as_of))
                            .expect("revolving_credit_pv")
                    },
                    |value| {
                        check_money(value, Currency::USD);
                        let mut missing_fixing = market.clone();
                        missing_fixing.retain_series_mut(|_, _| false);
                        assert!(
                            instrument.value(&missing_fixing, as_of).is_err(),
                            "historical reset fixing must remain required"
                        );
                    },
                );
            },
        );
    }
    group.finish();
}

/// Floating 5Y term loan: the discount-margin solve reprices its flows at
/// every Brent iteration.
fn bench_term_loan_discount_margin(c: &mut Criterion) {
    let mut group = c.benchmark_group("term_loan_discount_margin");

    group.bench_function("5Y_quarterly", |b| {
        use finstack_quant_valuations::instruments::PricingOptions;

        let as_of = as_of();
        let market = revolving_credit_market(as_of);
        let mut loan = term_loan(Date::from_calendar_date(2030, Month::January, 1).unwrap());
        loan.rate = RateSpec::Floating(finstack_quant_cashflows::builder::FloatingRateSpec {
            forward_curve_id: "USD-SOFR-3M".into(),
            spread_bp: Decimal::from(350),
            gearing: Decimal::ONE,
            gearing_includes_spread: true,
            index_floor_bp: Some(Decimal::ZERO),
            all_in_floor_bp: None,
            all_in_cap_bp: None,
            index_cap_bp: None,
            overnight_index_constraints: Default::default(),
            reset_frequency: Tenor::quarterly(),
            index_tenor: None,
            reset_lag_days: 0,
            fixing_calendar_id: None,
            compounding: None,
            overnight_basis: None,
            fallback: Default::default(),
        });
        loan.frequency = Tenor::quarterly();
        loan.instrument_pricing_overrides
            .market_quotes
            .quoted_clean_price_pct = Some(98.5);

        support::bench(
            b,
            || {
                loan.price_with_metrics(
                    black_box(&market),
                    black_box(as_of),
                    &[MetricId::DiscountMargin],
                    PricingOptions::default(),
                )
                .expect("term_loan_discount_margin")
            },
            |result| check_valuation(result, Currency::USD, &[MetricId::DiscountMargin]),
        );
    });
    group.finish();
}

/// Callable 5Y fixed term loan on the short-rate tree.
fn bench_term_loan_tree(c: &mut Criterion) {
    let mut group = c.benchmark_group("term_loan_tree");

    group.bench_function("5Y_callable", |b| {
        use finstack_quant_valuations::instruments::fixed_income::term_loan::{
            LoanCall, LoanCallSchedule, LoanCallType,
        };

        let as_of = as_of();
        let disc = discount_forward_curve_support::flat_discount("USD-OIS", as_of, 0.05);
        let market = MarketContext::new().insert(disc);
        let mut loan = term_loan(Date::from_calendar_date(2030, Month::January, 1).unwrap());
        loan.call_schedule = Some(LoanCallSchedule {
            calls: vec![LoanCall {
                date: Date::from_calendar_date(2027, Month::January, 1).unwrap(),
                price_pct_of_par: 101.0,
                call_type: LoanCallType::Hard,
            }],
        });

        support::bench(
            b,
            || {
                loan.value(black_box(&market), black_box(as_of))
                    .expect("term_loan_tree")
            },
            |value| check_money(value, Currency::USD),
        );
    });
    group.finish();
}

/// Stochastic 3Y floating revolver: 512 three-factor paths.
fn bench_revolving_credit_mc(c: &mut Criterion) {
    let mut group = c.benchmark_group("revolving_credit_mc");
    group.bench_function("3Y_512_paths", |b| {
        use finstack_quant_valuations::instruments::fixed_income::revolving_credit::{
            StochasticUtilizationSpec, UtilizationProcess,
        };

        let as_of = as_of();
        let market = revolving_credit_market(as_of);
        let mut facility =
            revolving_credit_floating(Date::from_calendar_date(2028, Month::January, 1).unwrap());
        if let RateSpec::Floating(spec) = &mut facility.rate {
            // Reset on the valuation date: no historical fixing is needed.
            spec.reset_lag_days = 0;
        }
        facility.draw_repay_spec = DrawRepaySpec::Stochastic(Box::new(StochasticUtilizationSpec {
            utilization_process: UtilizationProcess::MeanReverting {
                theta: 0.6,
                kappa: 1.0,
                sigma: 0.2,
                spread_sensitivity: 0.0,
            },
            mc_config: None,
        }));
        // 256 antithetic estimators simulate 512 paths.
        facility.instrument_pricing_overrides = facility
            .instrument_pricing_overrides
            .clone()
            .with_mc_paths(256)
            .with_mc_antithetic(true);

        support::bench(
            b,
            || {
                facility
                    .value(black_box(&market), black_box(as_of))
                    .expect("revolving_credit_mc")
            },
            |value| check_money(value, Currency::USD),
        );
    });
    group.finish();
}

fn bench_agency_mbs_pv(c: &mut Criterion) {
    let mut group = c.benchmark_group("agency_mbs_pv");
    group.bench_function("passthrough_example", |b| {
        let as_of = as_of();
        let market = MarketContext::new().insert(discount_forward_curve_support::flat_discount(
            "USD-OIS", as_of, 0.05,
        ));
        let mbs = AgencyMbsPassthrough::example().unwrap();
        support::bench(
            b,
            || {
                mbs.value(black_box(&market), black_box(as_of))
                    .expect("MBS PV")
            },
            |value| {
                check_money(value, Currency::USD);
                record::capture("agency_mbs_pv/passthrough_example", || {
                    json!({
                        "fixture": mbs_fixture_record(&mbs, &market, as_of, &[]),
                        "output": { "value": value, "measures": {} },
                    })
                });
            },
        );
    });
    group.bench_function("tba_example", |b| {
        let as_of = as_of();
        let market = MarketContext::new().insert(discount_forward_curve_support::flat_discount(
            "USD-OIS", as_of, 0.05,
        ));
        let tba = AgencyTba::example().unwrap();
        support::bench(
            b,
            || {
                tba.value(black_box(&market), black_box(as_of))
                    .expect("TBA PV")
            },
            |value| check_money(value, Currency::USD),
        );
    });
    group.finish();
}

/// MBS risk metrics: 512-path MC-OAS solve plus effective duration/convexity.
fn bench_agency_mbs_metrics(c: &mut Criterion) {
    use finstack_quant_valuations::instruments::PricingOptions;
    let mut group = c.benchmark_group("agency_mbs_metrics");
    for (label, metrics) in [
        ("oas", vec![MetricId::Oas]),
        (
            "duration_convexity",
            vec![MetricId::DurationMod, MetricId::Convexity],
        ),
    ] {
        group.bench_function(label, |b| {
            let as_of = as_of();
            let market = MarketContext::new().insert(
                discount_forward_curve_support::flat_discount("USD-OIS", as_of, 0.05),
            );
            let mut mbs = AgencyMbsPassthrough::example().unwrap();
            mbs.instrument_pricing_overrides
                .market_quotes
                .quoted_clean_price_pct = Some(95.0);
            support::bench(
                b,
                || {
                    mbs.price_with_metrics(
                        black_box(&market),
                        black_box(as_of),
                        &metrics,
                        PricingOptions::default(),
                    )
                    .expect("MBS metrics")
                },
                |result| {
                    check_valuation(result, Currency::USD, &metrics);
                    if metrics.contains(&MetricId::Oas) {
                        assert!((-0.10..=0.20).contains(&result.measures["oas"]));
                    }
                    let case = format!("agency_mbs_metrics/{label}");
                    record::capture(&case, || {
                        json!({
                            "fixture": mbs_fixture_record(&mbs, &market, as_of, &metrics),
                            "output": { "value": result.value, "measures": result.measures },
                        })
                    });
                },
            );
        });
    }
    group.finish();
}

/// Frozen B4 OAS sweeps: five axis points and four payment/seasoning controls.
fn bench_agency_mbs_oas_scaling(c: &mut Criterion) {
    use finstack_quant_valuations::instruments::PricingOptions;
    use time::macros::date;

    let mut group = c.benchmark_group("agency_mbs_oas_scaling");
    // Label, physical paths, remaining WAM, agency, custom stated delay,
    // Act/360 seasoning-pair fixture, and seasoned-versus-fresh member.
    for (label, paths, months, agency, stated_delay_days, act360, seasoned) in [
        (
            "paths_512_months_360",
            512,
            360,
            AgencyProgram::Fnma,
            None,
            false,
            false,
        ),
        (
            "paths_2048_months_360",
            2048,
            360,
            AgencyProgram::Fnma,
            None,
            false,
            false,
        ),
        (
            "paths_8192_months_360",
            8192,
            360,
            AgencyProgram::Fnma,
            None,
            false,
            false,
        ),
        (
            "paths_2048_months_60",
            2048,
            60,
            AgencyProgram::Fnma,
            None,
            false,
            false,
        ),
        (
            "paths_2048_months_180",
            2048,
            180,
            AgencyProgram::Fnma,
            None,
            false,
            false,
        ),
        (
            "gnma_i_paths_2048_months_360",
            2048,
            360,
            AgencyProgram::GnmaI,
            None,
            false,
            false,
        ),
        (
            "delay_15_paths_2048_months_360",
            2048,
            360,
            AgencyProgram::Fnma,
            Some(15),
            false,
            false,
        ),
        (
            "fresh_act360_paths_2048_months_300",
            2048,
            300,
            AgencyProgram::Fnma,
            None,
            true,
            false,
        ),
        (
            "seasoned_act360_paths_2048_months_300",
            2048,
            300,
            AgencyProgram::Fnma,
            None,
            true,
            true,
        ),
    ] {
        group.bench_function(label, |b| {
            let as_of = if act360 {
                date!(2026 - 01 - 15)
            } else {
                date!(2025 - 01 - 01)
            };
            let accrual_start = as_of.replace_day(1).expect("accrual month");
            let issue_date = if seasoned {
                date!(2021 - 01 - 01)
            } else {
                accrual_start
            };
            let maturity = accrual_start
                .add_months(i32::try_from(months).expect("bounded WAM"))
                .expect("contractual maturity");
            let mut mbs = AgencyMbsPassthrough::builder()
                .id(InstrumentId::new("MBS-OAS-B4"))
                .pool_id("MBS-OAS-B4-POOL".into())
                .agency(agency)
                .pool_type(PoolType::Generic)
                .original_face(Money::from((1_000_000_i64, Currency::USD)))
                .current_face(Money::from((
                    if act360 { 850_000_i64 } else { 1_000_000_i64 },
                    Currency::USD,
                )))
                .wac(0.045)
                .coupon(0.04)
                .servicing_fee_bp(25.0)
                .guarantee_fee_bp(25.0)
                .wam_months(months)
                .issue_date(issue_date)
                .maturity(maturity)
                .stated_delay_days_opt(stated_delay_days)
                .last_paid_accrual_end_opt(None)
                .prepayment_spec(PrepaymentModelSpec::psa(1.0))
                .discount_curve_id(CurveId::new("USD-OIS"))
                .day_count(if act360 {
                    DayCount::Act360
                } else {
                    DayCount::Thirty360
                })
                .build()
                .expect("B4 MBS fixture");
            mbs.instrument_pricing_overrides
                .market_quotes
                .quoted_clean_price_pct = Some(95.0);
            let model = &mut mbs.instrument_pricing_overrides.model_config;
            model.mc_paths = Some(paths);
            model.hw1f_mean_reversion = Some(0.05);
            model.hw1f_sigma = Some(0.01);
            model.mc_seed_scenario = Some("base".into());
            let discount = DiscountCurve::builder("USD-OIS")
                .base_date(as_of)
                .day_count(DayCount::Act365F)
                .interp(InterpStyle::LogLinear)
                .knots([(0.0, 1.0), (40.0, (-0.05_f64 * 40.0).exp())])
                .build()
                .expect("B4 flat discount curve");
            let market = MarketContext::new().insert(discount);
            let metrics = [MetricId::Oas];
            support::bench(
                b,
                || {
                    mbs.price_with_metrics(
                        black_box(&market),
                        black_box(as_of),
                        black_box(&metrics),
                        PricingOptions::default(),
                    )
                    .expect("B4 MBS OAS")
                },
                |result| {
                    check_valuation(result, Currency::USD, &metrics);
                    assert!((-0.10..=0.20).contains(&result.measures["oas"]));
                    assert_eq!(mbs.wam_months, months);
                    let model = &mbs.instrument_pricing_overrides.model_config;
                    assert_eq!(model.mc_paths, Some(paths));
                    assert_eq!(model.hw1f_mean_reversion, Some(0.05));
                    assert_eq!(model.hw1f_sigma, Some(0.01));
                    assert_eq!(model.mc_seed_scenario.as_deref(), Some("base"));
                    if seasoned {
                        assert!(mbs.seasoning_months(as_of) >= 59);
                    } else {
                        assert_eq!(mbs.seasoning_months(as_of), 0);
                    }
                    let first_payment = mbs
                        .payment_date_for_accrual_period(accrual_start)
                        .expect("first quote-basis payment");
                    assert!(first_payment > as_of);
                    if stated_delay_days == Some(15) {
                        assert_eq!(first_payment, date!(2025 - 01 - 15));
                        let tau = DayCount::Act365F
                            .year_fraction(as_of, first_payment, DayCountContext::default())
                            .expect("first payment time");
                        assert!(tau - 1.0 / 12.0 < 0.0);
                    }
                    if agency == AgencyProgram::GnmaI {
                        let mut fnma = mbs.clone();
                        fnma.agency = AgencyProgram::Fnma;
                        assert!(
                            first_payment
                                < fnma
                                    .payment_date_for_accrual_period(accrual_start)
                                    .expect("FNMA comparison payment")
                        );
                    }
                    let case = format!("agency_mbs_oas_scaling/{label}");
                    record::capture(&case, || {
                        json!({
                            "fixture": mbs_fixture_record(&mbs, &market, as_of, &metrics),
                            "output": { "value": result.value, "measures": result.measures },
                        })
                    });
                },
            );
        });
    }
    group.finish();
}

// FI Index TRS

/// Market context for FI TRS: discount, forward, and scalar yield/duration data.
fn fi_trs_market(as_of: Date) -> MarketContext {
    let disc = discount_forward_curve_support::flat_discount("USD-OIS", as_of, 0.05);
    let fwd = finstack_quant_core::market_data::term_structures::ForwardCurve::builder(
        "USD-SOFR-3M",
        0.25,
    )
    .base_date(as_of)
    .knots([(0.0, 0.04), (1.0, 0.045), (5.0, 0.05)])
    .build()
    .unwrap();
    let fixings = ScalarTimeSeries::new("FIXING:USD-SOFR-3M", vec![(as_of, 0.04)], None).unwrap();
    MarketContext::new()
        .insert(disc)
        .insert(fwd)
        .insert_series(fixings)
        .insert_price("US-CORP-INDEX", MarketScalar::Unitless(100.0))
        .insert_price("US-CORP-YIELD", MarketScalar::Unitless(0.055))
        .insert_price("US-CORP-DURATION", MarketScalar::Unitless(5.5))
        .insert_price("US-CORP-CONVEXITY", MarketScalar::Unitless(0.30))
}

/// Scale FI TRS pricing vs tenor (1Y, 3Y, 5Y).
///
/// Measures the carry-based TRS model: per-period yield computation, financing
/// leg floating-rate projection, and discounting over the payment schedule.
fn bench_fi_trs_pv(c: &mut Criterion) {
    let mut group = c.benchmark_group("fi_trs_pv");
    for (label, years) in [("1Y", 1), ("3Y", 3), ("5Y", 5)] {
        group.bench_with_input(BenchmarkId::from_parameter(label), &years, |b, years| {
            let as_of = as_of();
            let market = fi_trs_market(as_of);
            let maturity =
                Date::from_calendar_date(as_of.year() + years, as_of.month(), 1).unwrap();
            let mut trs = FiIndexTotalReturnSwap::example().unwrap();
            trs.schedule = TrsScheduleSpec::from_params(
                as_of,
                maturity,
                finstack_quant_cashflows::builder::ScheduleParams {
                    frequency: Tenor::quarterly(),
                    day_count: DayCount::Act360,
                    business_day_convention: BusinessDayConvention::Following,
                    calendar_id: "weekends_only".into(),
                    stub: StubKind::None,
                    end_of_month: false,
                    payment_lag_days: 0,
                    adjust_accrual_dates: false,
                    roll_rule: finstack_quant_cashflows::builder::RollRule::None,
                },
            );

            support::bench(
                b,
                || {
                    trs.value(black_box(&market), black_box(as_of))
                        .expect("FI TRS PV")
                },
                |value| check_money(value, Currency::USD),
            );
        });
    }
    group.finish();
}

// CMO

/// Five-tranche sequential CMO waterfall allocation.
///
/// Measures the sequential/PAC waterfall distribution over a monthly cashflow
/// projection horizon for the five-tranche fixture.
fn bench_cmo_waterfall_pv(c: &mut Criterion) {
    use finstack_quant_valuations::instruments::fixed_income::cmo::{CmoTranche, CmoWaterfall};
    use time::macros::date;
    let mut group = c.benchmark_group("cmo_waterfall_pv");
    group.bench_function("5t", |b| {
        let as_of = as_of();
        let market = MarketContext::new().insert(discount_forward_curve_support::flat_discount(
            "USD-OIS", as_of, 0.05,
        ));
        let n_tranches = 5;
        let tranches: Vec<CmoTranche> = (0..n_tranches)
            .map(|i| {
                // Keep total coupon demand under the ~4% collateral
                // pass-through so the deal passes interest-coverage
                // validation at build.
                let coupon = 0.03 + 0.001 * i as f64;
                let balance = 10_000_000.0;
                CmoTranche::sequential(
                    format!("T{}", i).as_str(),
                    Money::new(balance, Currency::USD).expect("valid money fixture"),
                    coupon,
                    (i + 1) as u32,
                )
            })
            .collect();

        let cmo = AgencyCmo::builder()
            .id(InstrumentId::new("CMO-BENCH"))
            .deal_name("BENCH-CMO".into())
            .agency(AgencyProgram::Fnma)
            .issue_date(date!(2024 - 01 - 01))
            .waterfall(CmoWaterfall::new(tranches))
            .reference_tranche_id("T0".to_string())
            .collateral_wac(0.045)
            .collateral_wam_months(360)
            .discount_curve_id(CurveId::new("USD-OIS"))
            .build()
            .unwrap();

        assert_eq!(cmo.waterfall.tranches.len(), n_tranches);
        support::bench(
            b,
            || {
                cmo.value(black_box(&market), black_box(as_of))
                    .expect("CMO waterfall PV")
            },
            |value| check_money(value, Currency::USD),
        );
    });
    group.finish();
}

/// Compare CMO structure types: sequential, PAC/support, IO/PO.
fn bench_cmo_structure_type(c: &mut Criterion) {
    let mut group = c.benchmark_group("cmo_structure_type");
    for (label, fixture) in [
        (
            "sequential",
            AgencyCmo::example as fn() -> finstack_quant_core::Result<AgencyCmo>,
        ),
        ("pac_support", AgencyCmo::example_pac_support),
        ("io_po", AgencyCmo::example_io_po),
    ] {
        group.bench_function(label, |b| {
            let as_of = as_of();
            let market = MarketContext::new().insert(
                discount_forward_curve_support::flat_discount("USD-OIS", as_of, 0.05),
            );
            let cmo = fixture().expect("CMO fixture");
            assert!(!cmo.waterfall.tranches.is_empty());
            support::bench(
                b,
                || {
                    cmo.value(black_box(&market), black_box(as_of))
                        .expect(label)
                },
                |value| check_money(value, Currency::USD),
            );
        });
    }
    group.finish();
}

// Dollar Roll

/// Dollar roll NPV and implied-financing computation.
///
/// Benchmarks the two-leg settlement NPV (front vs. back month price
/// difference discounted to as_of) plus the implied repo rate calculation.
fn bench_dollar_roll_pv(c: &mut Criterion) {
    let mut group = c.benchmark_group("dollar_roll_pv");
    for (label, drop_bp) in [
        ("tight_10bp", 0.10_f64),
        ("wide_50bp", 0.50),
        ("neg_neg10bp", -0.10),
    ] {
        group.bench_with_input(
            BenchmarkId::from_parameter(label),
            &drop_bp,
            |b, drop_bp| {
                let as_of = as_of();
                let market = MarketContext::new().insert(
                    discount_forward_curve_support::flat_discount("USD-OIS", as_of, 0.05),
                );
                let front_price = 98.50_f64;
                let back_price = front_price - drop_bp;
                let roll = DollarRoll::builder()
                    .id(InstrumentId::new("ROLL-BENCH"))
                    .agency(AgencyProgram::Fnma)
                    .coupon(0.04)
                    .term(TbaTerm::ThirtyYear)
                    .settlement_class(finstack_quant_core::dates::SifmaSettlementClass::A)
                    .notional(Money::new(10_000_000.0, Currency::USD).expect("valid money fixture"))
                    .front_settlement_year(2025)
                    .front_settlement_month(3)
                    .back_settlement_year(2025)
                    .back_settlement_month(4)
                    .front_price(front_price)
                    .back_price(back_price)
                    .discount_curve_id(CurveId::new("USD-OIS"))
                    .build()
                    .unwrap();

                support::bench(
                    b,
                    || {
                        roll.value(black_box(&market), black_box(as_of))
                            .expect("dollar roll PV")
                    },
                    |value| check_money(value, Currency::USD),
                );
            },
        );
    }
    group.finish();
}

criterion_group!(
    benches,
    bench_term_loan_pv,
    bench_revolving_credit_pv,
    bench_term_loan_discount_margin,
    bench_term_loan_tree,
    bench_revolving_credit_mc,
    bench_agency_mbs_pv,
    bench_agency_mbs_metrics,
    bench_agency_mbs_oas_scaling,
    bench_fi_trs_pv,
    bench_cmo_waterfall_pv,
    bench_cmo_structure_type,
    bench_dollar_roll_pv,
);
criterion_main!(benches);
