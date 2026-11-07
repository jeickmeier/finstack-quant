//! Frozen correctness checks for the OIS performance baseline.
//!
//! These tests always use the same fixtures and bounds. `--nocapture` exposes
//! `FQ_OIS_DIAGNOSTIC` JSON records; environment variables do not alter the tests.
//! References share only public schedules, calendars, curves and bump APIs with
//! pricing. They never call the overnight projector to calculate expected PVs.

use finstack_quant_cashflows::CashflowProvider;
use finstack_quant_cashflows::builder::{
    CashFlowSchedule, CashflowRepresentation, FloatingLegCompounding, build_periods,
};
use finstack_quant_cashflows::fixings::{ProjectedFixing, materialize_fixings};
use finstack_quant_core::cashflow::CFKind;
use finstack_quant_core::currency::Currency;
use finstack_quant_core::dates::{
    BusinessDayConvention, Date, DayCount, StubKind, Tenor, adjust, calendar_by_id_strict,
};
use finstack_quant_core::market_data::bumps::{BumpSpec, MarketBump};
use finstack_quant_core::market_data::context::MarketContext;
use finstack_quant_core::market_data::scalars::{ScalarTimeSeries, SeriesInterpolation};
use finstack_quant_core::market_data::term_structures::{DiscountCurve, ForwardCurve};
use finstack_quant_core::math::interp::InterpStyle;
use finstack_quant_core::money::Money;
use finstack_quant_valuations_core::instruments::{
    FixedLegSpec, FloatLegSpec, Instrument, PayReceive, PricingOptions,
};
use finstack_quant_valuations_core::metrics::MetricId;
use finstack_quant_valuations_rates::irs::InterestRateSwap;
use rust_decimal_macros::dec;
use serde_json::{Value, json};
use time::Month;
use time::macros::date;

const FACTOR_BOUND: f64 = 1e-12;
const ANNUITY_BOUND: f64 = 1e-12;
const FIXED_PV_BOUND: f64 = 1e-8;
const RAW_RATE_BOUND: f64 = 1e-13;
const NOTIONAL: f64 = 10_000_000.0;

fn close(label: &str, actual: f64, expected: f64, bound: f64) {
    assert!(actual.is_finite() && expected.is_finite());
    assert!(bound.is_finite() && bound > 0.0);
    assert!(
        (actual - expected).abs() <= bound,
        "{label}: actual={actual:.17e}, expected={expected:.17e}, error={:.17e}, bound={bound:.17e}",
        (actual - expected).abs(),
    );
}

fn emit(case: &str, data: Value) {
    eprintln!("FQ_OIS_DIAGNOSTIC {}", json!({"case": case, "data": data}));
}

// Keep these inputs identical to benches/swap_pricing.rs::create_ois_swap.
fn benchmark_swap(tenor: i32) -> InterestRateSwap {
    let start = date!(2025 - 01 - 01);
    let end = Date::from_calendar_date(2025 + tenor, Month::January, 1).unwrap();
    InterestRateSwap::builder()
        .id(format!("OIS-{tenor}Y").into())
        .notional(Money::new(NOTIONAL, Currency::USD).unwrap())
        .side(PayReceive::Pay)
        .fixed_leg(FixedLegSpec {
            discount_curve_id: "USD-OIS".into(),
            rate: dec!(0.04),
            frequency: Tenor::annual(),
            day_count: DayCount::Act360,
            business_day_convention: BusinessDayConvention::ModifiedFollowing,
            calendar_id: None,
            stub: StubKind::None,
            start,
            end,
            payment_lag_days: 2,
            end_of_month: false,
        })
        .float_leg(FloatLegSpec {
            discount_curve_id: "USD-OIS".into(),
            forward_curve_id: "USD-OIS".into(),
            spread_bp: dec!(0.0),
            frequency: Tenor::annual(),
            day_count: DayCount::Act360,
            business_day_convention: BusinessDayConvention::ModifiedFollowing,
            calendar_id: None,
            fixing_calendar_id: None,
            stub: StubKind::None,
            reset_lag_days: 0,
            start,
            end,
            compounding: FloatingLegCompounding::CompoundedInArrears { lookback_days: 0 },
            payment_lag_days: 2,
            end_of_month: false,
        })
        .build()
        .unwrap()
}

// Retain even the two unused term-forward curves from the benchmark market.
fn benchmark_market() -> MarketContext {
    let base = date!(2025 - 01 - 01);
    let discount = DiscountCurve::builder("USD-OIS")
        .base_date(base)
        .knots([
            (0.0, 1.0),
            (1.0, 0.98),
            (2.0, 0.96),
            (5.0, 0.88),
            (10.0, 0.70),
            (30.0, 0.40),
            (50.0, 0.20),
        ])
        .interp(InterpStyle::Linear)
        .build()
        .unwrap();
    let quarterly = ForwardCurve::builder("USD-SOFR-3M", 0.25)
        .base_date(base)
        .knots([
            (0.0, 0.035),
            (1.0, 0.038),
            (2.0, 0.040),
            (5.0, 0.045),
            (10.0, 0.050),
            (30.0, 0.055),
            (50.0, 0.055),
        ])
        .interp(InterpStyle::Linear)
        .build()
        .unwrap();
    let monthly = ForwardCurve::builder("USD-SOFR-1M", 1.0 / 12.0)
        .base_date(base)
        .knots([
            (0.0, 0.034),
            (1.0, 0.037),
            (2.0, 0.039),
            (5.0, 0.044),
            (10.0, 0.049),
            (30.0, 0.054),
            (50.0, 0.054),
        ])
        .interp(InterpStyle::Linear)
        .build()
        .unwrap();
    MarketContext::new()
        .insert(discount)
        .insert(quarterly)
        .insert(monthly)
}

struct CouponReference {
    start: Date,
    end: Date,
    payment: Date,
    alpha: f64,
    factor: f64,
    discount: f64,
}

struct Reference {
    fixed_pv: f64,
    float_pv: f64,
    annuity: f64,
    pv_bound: f64,
    coupons: Vec<CouponReference>,
}

impl Reference {
    fn pv(&self) -> f64 {
        self.float_pv - self.fixed_pv
    }

    fn par(&self) -> f64 {
        self.float_pv / (NOTIONAL * self.annuity)
    }

    fn par_bound(&self) -> f64 {
        self.pv_bound / (NOTIONAL * self.annuity).abs()
            + self.float_pv.abs() * ANNUITY_BOUND / (NOTIONAL * self.annuity.powi(2))
    }

    fn record(&self) -> Value {
        json!({
            "pv": self.pv(),
            "pv_fixed": self.fixed_pv,
            "pv_float": self.float_pv,
            "annuity": self.annuity,
            "par_rate": self.par(),
            "bounds": {
                "compound_factor": FACTOR_BOUND,
                "decimal_rate": FACTOR_BOUND,
                "pv_usd": self.pv_bound,
                "fixed_pv_usd": FIXED_PV_BOUND,
                "annuity": ANNUITY_BOUND,
                "par_rate_decimal": self.par_bound(),
            },
            "coupons": self.coupons.iter().map(|coupon| json!({
                "adjusted_start": coupon.start.to_string(),
                "adjusted_end": coupon.end.to_string(),
                "payment_date": coupon.payment.to_string(),
                "accrual_fraction": coupon.alpha,
                "compound_factor": coupon.factor,
                "decimal_rate": (coupon.factor - 1.0) / coupon.alpha,
                "amount_usd": NOTIONAL * (coupon.factor - 1.0),
                "discount_to_as_of": coupon.discount,
            })).collect::<Vec<_>>(),
        })
    }
}

fn reference(swap: &InterestRateSwap, market: &MarketContext, as_of: Date) -> Reference {
    assert_eq!(swap.fixed_leg.rate, dec!(0.04));
    assert_eq!(swap.float_leg.spread_bp, dec!(0));
    assert_eq!(swap.fixed_leg.day_count, DayCount::Act360);
    assert_eq!(swap.float_leg.day_count, DayCount::Act360);
    assert!(market.get_forward("USD-OIS").is_err());
    let curve = market.get_discount("USD-OIS").unwrap();
    let as_of_df = curve.df_on_date(as_of).unwrap();
    let fixed_periods = build_periods(swap.fixed_leg.schedule_params(false)).unwrap();
    let annuity = fixed_periods
        .iter()
        .filter(|period| period.payment_date > as_of)
        .map(|period| {
            let alpha = (period.accrual_end - period.accrual_start).whole_days() as f64 / 360.0;
            close("fixed accrual", period.accrual_year_fraction, alpha, 1e-15);
            alpha * curve.df_on_date(period.payment_date).unwrap() / as_of_df
        })
        .sum::<f64>();
    let float = &swap.float_leg;
    let calendar = calendar_by_id_strict("usny").unwrap();
    let coupons: Vec<_> =
        build_periods(float.schedule_params(float.calendar_or_weekends(), None, false))
            .unwrap()
            .into_iter()
            .filter(|period| period.payment_date > as_of)
            .map(|period| {
                let start = adjust(
                    period.accrual_start,
                    float.business_day_convention,
                    calendar,
                )
                .unwrap();
                let end =
                    adjust(period.accrual_end, float.business_day_convention, calendar).unwrap();
                assert!(start >= as_of);
                CouponReference {
                    start,
                    end,
                    payment: period.payment_date,
                    alpha: (end - start).whole_days() as f64 / 360.0,
                    factor: curve.df_on_date(start).unwrap() / curve.df_on_date(end).unwrap(),
                    discount: curve.df_on_date(period.payment_date).unwrap() / as_of_df,
                }
            })
            .collect();
    let float_pv = coupons
        .iter()
        .map(|coupon| NOTIONAL * (coupon.factor - 1.0) * coupon.discount)
        .sum();
    let pv_bound = (NOTIONAL
        * FACTOR_BOUND
        * coupons
            .iter()
            .map(|coupon| coupon.discount.abs())
            .sum::<f64>())
    .max(1e-8);
    Reference {
        fixed_pv: NOTIONAL * 0.04 * annuity,
        float_pv,
        annuity,
        pv_bound,
        coupons,
    }
}

fn check_reference(
    swap: &InterestRateSwap,
    market: &MarketContext,
    as_of: Date,
    reference: &Reference,
) -> Value {
    let requested = [
        MetricId::PvFixed,
        MetricId::PvFloat,
        MetricId::ParRate,
        MetricId::Annuity,
    ];
    let result = swap
        .price_with_metrics(market, as_of, &requested, PricingOptions::default())
        .unwrap();
    assert_eq!(result.value.currency(), Currency::USD);
    let pv = swap.value(market, as_of).unwrap();
    close("value PV", pv.amount(), reference.pv(), reference.pv_bound);
    close(
        "metrics PV",
        result.value.amount(),
        reference.pv(),
        reference.pv_bound,
    );
    for (metric, expected, bound) in [
        (MetricId::PvFixed, reference.fixed_pv, FIXED_PV_BOUND),
        (MetricId::PvFloat, reference.float_pv, reference.pv_bound),
        (MetricId::Annuity, reference.annuity, ANNUITY_BOUND),
        (MetricId::ParRate, reference.par(), reference.par_bound()),
    ] {
        close(metric.as_str(), result.measures[&metric], expected, bound);
    }
    let schedule = swap.cashflow_schedule(market, as_of).unwrap();
    assert_eq!(
        schedule.get_meta().representation,
        CashflowRepresentation::Projected
    );
    let floating: Vec<_> = schedule
        .get_flows()
        .iter()
        .filter(|flow| flow.kind == CFKind::FloatReset)
        .collect();
    assert_eq!(floating.len(), reference.coupons.len());
    for (flow, coupon) in floating.iter().zip(&reference.coupons) {
        assert_eq!(flow.date, coupon.payment);
        assert_eq!(flow.amount.currency(), Currency::USD);
        close("float accrual", flow.accrual_factor, coupon.alpha, 1e-15);
        let rate = flow.rate.unwrap();
        close(
            "coupon rate",
            rate,
            (coupon.factor - 1.0) / coupon.alpha,
            FACTOR_BOUND,
        );
        close(
            "coupon factor",
            1.0 + rate * flow.accrual_factor,
            coupon.factor,
            FACTOR_BOUND,
        );
    }
    json!({"value": pv, "metrics": result.measures, "cashflows": schedule})
}

#[test]
fn ois_baseline_retained_tenors_match_coupon_and_dv01_references() {
    let as_of = date!(2025 - 01 - 01);
    let market = benchmark_market();
    let up = market
        .bump([MarketBump::Curve {
            id: "USD-OIS".into(),
            spec: BumpSpec::parallel_bp(1.0),
        }])
        .unwrap();
    let down = market
        .bump([MarketBump::Curve {
            id: "USD-OIS".into(),
            spec: BumpSpec::parallel_bp(-1.0),
        }])
        .unwrap();
    for tenor in [1, 2, 5, 10] {
        let swap = benchmark_swap(tenor);
        let base_reference = reference(&swap, &market, as_of);
        let up_reference = reference(&swap, &up, as_of);
        let down_reference = reference(&swap, &down, as_of);
        assert_eq!(base_reference.coupons.len(), tenor as usize);
        let base_output = check_reference(&swap, &market, as_of, &base_reference);
        let up_output = check_reference(&swap, &up, as_of, &up_reference);
        let down_output = check_reference(&swap, &down, as_of, &down_reference);
        let requested = [
            MetricId::PvFixed,
            MetricId::PvFloat,
            MetricId::ParRate,
            MetricId::Dv01,
        ];
        let result = swap
            .price_with_metrics(&market, as_of, &requested, PricingOptions::default())
            .unwrap();
        for metric in &requested {
            assert!(result.measures[metric].is_finite());
        }
        let expected_dv01 = (up_reference.pv() - down_reference.pv()) / 2.0;
        let dv01_bound = (up_reference.pv_bound + down_reference.pv_bound) / 2.0;
        close(
            "combined DV01 USD/bp",
            result.measures[&MetricId::Dv01],
            expected_dv01,
            dv01_bound,
        );
        emit(
            &format!("ois_baseline/{tenor}Y"),
            json!({
                "fixture": {"instrument": swap, "market": market, "as_of": as_of.to_string(), "model": swap.default_model()},
                "bump": {"curve_ids": ["USD-OIS"], "width_bp": 1.0, "method": "central_parallel_combined", "calibration": null},
                "base": {"reference": base_reference.record(), "actual": base_output},
                "up": {"reference": up_reference.record(), "actual": up_output},
                "down": {"reference": down_reference.record(), "actual": down_output},
                "dv01": {"expected_usd_per_bp": expected_dv01, "actual_usd_per_bp": result.measures[&MetricId::Dv01], "absolute_bound_usd_per_bp": dv01_bound},
            }),
        );
    }
}

fn compact_swap() -> InterestRateSwap {
    let mut swap = benchmark_swap(1);
    swap.id = "OIS-COMPACT-EXPORT".into();
    swap.fixed_leg.start = date!(2025 - 01 - 02);
    swap.fixed_leg.end = date!(2025 - 01 - 10);
    swap.fixed_leg.calendar_id = Some("weekends_only".into());
    swap.fixed_leg.stub = StubKind::ShortFront;
    swap.fixed_leg.business_day_convention = BusinessDayConvention::Following;
    swap.float_leg.start = swap.fixed_leg.start;
    swap.float_leg.end = swap.fixed_leg.end;
    swap.float_leg.calendar_id = Some("weekends_only".into());
    swap.float_leg.fixing_calendar_id = Some("weekends_only".into());
    swap.float_leg.stub = StubKind::ShortFront;
    swap.float_leg.business_day_convention = BusinessDayConvention::Following;
    swap.validate().unwrap();
    swap
}

fn compact_market() -> MarketContext {
    MarketContext::new().insert(
        DiscountCurve::builder("USD-OIS")
            .base_date(date!(2025 - 01 - 02))
            .day_count(DayCount::Act365F)
            .knots([(0.0, 1.0), (1.0, 0.96), (5.0, 0.80)])
            .interp(InterpStyle::Linear)
            .build()
            .unwrap(),
    )
}

fn compact_pairs() -> [(Date, Date, f64); 6] {
    [
        (date!(2025 - 01 - 02), date!(2025 - 01 - 03), 1.0 / 360.0),
        (date!(2025 - 01 - 03), date!(2025 - 01 - 06), 3.0 / 360.0),
        (date!(2025 - 01 - 06), date!(2025 - 01 - 07), 1.0 / 360.0),
        (date!(2025 - 01 - 07), date!(2025 - 01 - 08), 1.0 / 360.0),
        (date!(2025 - 01 - 08), date!(2025 - 01 - 09), 1.0 / 360.0),
        (date!(2025 - 01 - 09), date!(2025 - 01 - 10), 1.0 / 360.0),
    ]
}

fn compact_reference(market: &MarketContext, as_of: Date) -> (f64, Vec<ProjectedFixing>) {
    let curve = market.get_discount("USD-OIS").unwrap();
    let mut product = 1.0;
    let mut observations = Vec::new();
    for (start, end, alpha) in compact_pairs() {
        let rate = if start < as_of {
            market
                .get_series("FIXING:USD-OIS")
                .unwrap()
                .value_on_exact(start)
                .unwrap()
        } else {
            (curve.df_on_date(start).unwrap() / curve.df_on_date(end).unwrap() - 1.0) / alpha
        };
        product *= 1.0 + rate * alpha;
        observations.push(ProjectedFixing {
            series_id: "FIXING:USD-OIS".into(),
            date: start,
            value: Some(rate),
        });
    }
    (product, observations)
}

fn check_compact(
    swap: &InterestRateSwap,
    market: &MarketContext,
    as_of: Date,
) -> (CashFlowSchedule, Value) {
    let (factor, expected_fixings) = compact_reference(market, as_of);
    let before = swap.cashflow_schedule(market, as_of).unwrap();
    let before_json = serde_json::to_value(&before).unwrap();
    let value = swap.value(market, as_of).unwrap();
    let after = swap.cashflow_schedule(market, as_of).unwrap();
    assert_eq!(before_json, serde_json::to_value(&after).unwrap());
    assert_eq!(
        before.get_meta().representation,
        CashflowRepresentation::Projected
    );
    assert_eq!(before.get_flows().len(), 2);
    for kind in [CFKind::Stub, CFKind::FloatReset] {
        assert_eq!(
            before
                .get_flows()
                .iter()
                .filter(|flow| flow.kind == kind)
                .count(),
            1
        );
    }
    assert_eq!(before.get_meta().projected_fixings.len(), 6);
    for (actual, expected) in before
        .get_meta()
        .projected_fixings
        .iter()
        .zip(&expected_fixings)
    {
        assert_eq!(actual.series_id, expected.series_id);
        assert_eq!(actual.date, expected.date);
        close(
            "raw fixing rate",
            actual.value.unwrap(),
            expected.value.unwrap(),
            RAW_RATE_BOUND,
        );
    }
    let alpha = 8.0 / 360.0;
    for flow in before.get_flows() {
        assert_eq!(flow.date, date!(2025 - 01 - 14));
        assert_eq!(flow.amount.currency(), Currency::USD);
        close("compact accrual", flow.accrual_factor, alpha, 1e-15);
        match flow.kind {
            CFKind::Stub => {
                close(
                    "signed fixed coupon",
                    flow.amount.amount(),
                    -NOTIONAL * 0.04 * alpha,
                    FIXED_PV_BOUND,
                );
                close("fixed coupon rate", flow.rate.unwrap(), 0.04, FACTOR_BOUND);
            }
            CFKind::FloatReset => {
                close(
                    "signed floating coupon",
                    flow.amount.amount(),
                    NOTIONAL * (factor - 1.0),
                    NOTIONAL * FACTOR_BOUND,
                );
                close(
                    "compact factor",
                    1.0 + flow.rate.unwrap() * alpha,
                    factor,
                    FACTOR_BOUND,
                );
                close(
                    "compact coupon rate",
                    flow.rate.unwrap(),
                    (factor - 1.0) / alpha,
                    FACTOR_BOUND,
                );
            }
            other => panic!("unexpected compact flow kind {other:?}"),
        }
    }
    assert_eq!(
        swap.dated_cashflows(market, as_of).unwrap(),
        before.dated_flows().unwrap()
    );
    let curve = market.get_discount("USD-OIS").unwrap();
    let discount =
        curve.df_on_date(date!(2025 - 01 - 14)).unwrap() / curve.df_on_date(as_of).unwrap();
    let expected_pv = NOTIONAL * ((factor - 1.0) - 0.04 * alpha) * discount;
    let pv_bound = (NOTIONAL * FACTOR_BOUND * discount.abs()).max(1e-8);
    close("compact net PV", value.amount(), expected_pv, pv_bound);
    let record = json!({
        "as_of": as_of.to_string(), "value": value, "cashflows": before,
        "reference": {"compound_factor": factor, "pv_usd": expected_pv, "pv_bound_usd": pv_bound,
            "factor_bound": FACTOR_BOUND, "raw_rate_bound": RAW_RATE_BOUND, "observations": expected_fixings,
            "pairs": compact_pairs().iter().map(|(start, end, alpha)| json!({"start":start.to_string(),"end":end.to_string(),"weight":alpha})).collect::<Vec<_>>()},
    });
    (before, record)
}

#[test]
fn ois_baseline_compact_export_and_inclusive_fixing_roll() {
    let origin = date!(2025 - 01 - 02);
    let horizon = date!(2025 - 01 - 06);
    let swap = compact_swap();
    let market = compact_market();
    let (schedule, origin_record) = check_compact(&swap, &market, origin);

    let mut receive = swap.clone();
    receive.side = PayReceive::Receive;
    let reversed = receive.cashflow_schedule(&market, origin).unwrap();
    assert_eq!(
        schedule.get_meta().projected_fixings,
        reversed.get_meta().projected_fixings
    );
    assert_eq!(schedule.get_flows().len(), reversed.get_flows().len());
    for pay in schedule.get_flows() {
        // Reversing signed amounts can change canonical row order on one date.
        let receive = reversed
            .get_flows()
            .iter()
            .find(|flow| {
                flow.date == pay.date
                    && flow.kind == pay.kind
                    && flow.amount.currency() == pay.amount.currency()
            })
            .unwrap();
        assert_eq!(pay.amount.amount(), -receive.amount.amount());
    }
    let mut spread = swap.clone();
    spread.float_leg.spread_bp = dec!(100);
    let spread_schedule = spread.cashflow_schedule(&market, origin).unwrap();
    assert_eq!(
        schedule.get_meta().projected_fixings,
        spread_schedule.get_meta().projected_fixings
    );
    for (base, shifted) in schedule.get_flows().iter().zip(spread_schedule.get_flows()) {
        let expected_change = if base.kind == CFKind::FloatReset {
            NOTIONAL * 0.01 * 8.0 / 360.0
        } else {
            0.0
        };
        close(
            "arithmetic spread cash",
            shifted.amount.amount() - base.amount.amount(),
            expected_change,
            FIXED_PV_BOUND,
        );
    }

    let materialized = materialize_fixings(&market, [&schedule], origin, horizon).unwrap();
    let observations = materialized
        .get_series("FIXING:USD-OIS")
        .unwrap()
        .observations();
    let expected_observations: Vec<_> = schedule
        .get_meta()
        .projected_fixings
        .iter()
        .filter(|fixing| fixing.date <= horizon)
        .map(|fixing| (fixing.date, fixing.value.unwrap()))
        .collect();
    assert_eq!(observations, expected_observations);
    assert_eq!(
        observations
            .iter()
            .map(|(date, _)| *date)
            .collect::<Vec<_>>(),
        vec![origin, date!(2025 - 01 - 03), horizon]
    );
    let repeated = materialize_fixings(&materialized, [&schedule], origin, horizon).unwrap();
    assert_eq!(
        repeated
            .get_series("FIXING:USD-OIS")
            .unwrap()
            .observations(),
        observations
    );
    let rolled = materialized.roll_forward(4).unwrap();
    assert_eq!(rolled.get_discount("USD-OIS").unwrap().base_date(), horizon);
    let (_, roll_record) = check_compact(&swap, &rolled, horizon);

    // Accrual has ended on Jan10, but the delayed Jan14 coupon is still owed.
    let before_payment = date!(2025 - 01 - 13);
    let fully_realized = materialize_fixings(&market, [&schedule], origin, before_payment)
        .unwrap()
        .roll_forward(11)
        .unwrap();
    let (_, unpaid_record) = check_compact(&swap, &fully_realized, before_payment);
    assert_ne!(
        swap.value(&fully_realized, before_payment)
            .unwrap()
            .amount(),
        0.0
    );
    let after_payment = date!(2025 - 01 - 15);
    assert_eq!(swap.value(&market, after_payment).unwrap().amount(), 0.0);
    assert!(
        swap.cashflow_schedule(&market, after_payment)
            .unwrap()
            .get_flows()
            .is_empty()
    );

    // A stored same-day observation must still be projected at start of day.
    let mut same_day_values = observations.clone();
    same_day_values
        .iter_mut()
        .find(|(date, _)| *date == horizon)
        .unwrap()
        .1 = 0.75;
    let same_day_market = rolled
        .clone()
        .insert_series(ScalarTimeSeries::new("FIXING:USD-OIS", same_day_values, None).unwrap());
    assert_eq!(
        serde_json::to_value(swap.cashflow_schedule(&rolled, horizon).unwrap()).unwrap(),
        serde_json::to_value(swap.cashflow_schedule(&same_day_market, horizon).unwrap()).unwrap(),
    );
    let (_, same_day_record) = check_compact(&swap, &same_day_market, horizon);

    // Adjacent observations plus interpolation cannot replace exact Jan3 data.
    let missing_values = observations
        .into_iter()
        .filter(|(date, _)| *date != date!(2025 - 01 - 03))
        .collect();
    let missing = rolled.clone().insert_series(
        ScalarTimeSeries::new("FIXING:USD-OIS", missing_values, None)
            .unwrap()
            .with_interpolation(SeriesInterpolation::Linear),
    );
    let expected_error = "Missing fixing for 'USD-OIS' on 2025-01-03 (valuation date: 2025-01-06). The fixing series exists but has no exact observation: Requested item not found: series 'FIXING:USD-OIS' observation on 2025-01-03";
    for error in [
        swap.value(&missing, horizon).unwrap_err(),
        swap.cashflow_schedule(&missing, horizon).unwrap_err(),
    ] {
        let finstack_quant_core::Error::Validation(message) = error else {
            panic!("unexpected missing-fixing error {error:?}")
        };
        assert_eq!(message, expected_error);
    }
    emit(
        "ois_baseline/compact_export_roll",
        json!({
            "fixture": {"instrument": swap, "market": market, "origin": origin.to_string(), "horizon": horizon.to_string()},
        "origin": origin_record, "rolled": roll_record, "rolled_market": rolled,
        "unpaid_delayed_coupon": unpaid_record,
        "stored_same_day_fixing_ignored": same_day_record,
            "materialized_observations": expected_observations,
            "reversed_cashflows": reversed, "spread_100bp_cashflows": spread_schedule,
            "missing_fixing_error": expected_error,
        }),
    );
}

#[path = "ois_p3_regressions.rs"]
mod p3;
