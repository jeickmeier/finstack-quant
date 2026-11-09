//! Independent adversarial references for the bounded IRS economics-only change.
//!
//! The dated products below use public curves, calendars and schedules. The
//! canonical overnight projector is called only as a subject of the dedicated
//! compounded-spread test, never to derive an expected value.

use super::{
    ANNUITY_BOUND, FACTOR_BOUND, FIXED_PV_BOUND, NOTIONAL, RAW_RATE_BOUND, benchmark_market,
    benchmark_swap, close, compact_market, compact_swap, emit,
};
use finstack_quant_cashflows::CashflowProvider;
use finstack_quant_cashflows::builder::{
    CashFlowSchedule, CashflowRepresentation, FloatingLegCompounding, build_periods,
};
use finstack_quant_cashflows::fixings::ProjectedFixing;
use finstack_quant_core::cashflow::CFKind;
use finstack_quant_core::currency::Currency;
use finstack_quant_core::dates::{
    BusinessDayConvention, Date, DateExt, DayCount, DayCountContext, StubKind, Tenor, adjust,
    calendar_by_id_strict,
};
use finstack_quant_core::market_data::bumps::{BumpSpec, MarketBump};
use finstack_quant_core::market_data::context::MarketContext;
use finstack_quant_core::market_data::scalars::ScalarTimeSeries;
use finstack_quant_core::market_data::term_structures::{DiscountCurve, ForwardCurve};
use finstack_quant_core::math::interp::InterpStyle;
use finstack_quant_core::money::Money;
use finstack_quant_valuations_core::instruments::pricing::overnight::{
    OvernightCouponProjection, OvernightCouponProjectionInput, OvernightProjectionCurve,
    project_overnight_coupon,
};
use finstack_quant_valuations_core::instruments::{Instrument, PayReceive, PricingOptions};
use finstack_quant_valuations_core::metrics::MetricId;
use finstack_quant_valuations_core::pricer::ModelKey;
use finstack_quant_valuations_rates::irs::InterestRateSwap;
use rust_decimal::prelude::ToPrimitive;
use rust_decimal_macros::dec;
use serde_json::{Value, json};
use std::collections::BTreeMap;
use time::macros::date;

struct Observation {
    start: Date,
    end: Date,
    rate_alpha: f64,
    factor_alpha: f64,
}

struct CouponGrid {
    start: Date,
    end: Date,
    payment: Date,
    alpha: f64,
    observations: Vec<Observation>,
}

struct CouponOracle {
    grid: CouponGrid,
    factor: f64,
    rate: f64,
    amount: f64,
    discount: f64,
    fixings: Vec<ProjectedFixing>,
}

struct Oracle {
    notional: f64,
    fixed_pv: f64,
    float_pv: f64,
    annuity: f64,
    pv_bound: f64,
    fixed_rows: Vec<(Date, f64, f64, CFKind)>,
    coupons: Vec<CouponOracle>,
}

impl Oracle {
    fn pv(&self) -> f64 {
        self.float_pv - self.fixed_pv
    }

    fn par(&self) -> f64 {
        self.float_pv / (self.notional * self.annuity)
    }

    fn par_bound(&self) -> f64 {
        self.pv_bound / (self.notional * self.annuity).abs()
            + self.float_pv.abs() * ANNUITY_BOUND / (self.notional * self.annuity.powi(2)).abs()
    }

    fn fixings(&self) -> Vec<ProjectedFixing> {
        let mut unique = BTreeMap::new();
        for fixing in self.coupons.iter().flat_map(|coupon| &coupon.fixings) {
            let key = (fixing.series_id.clone(), fixing.date);
            if let Some(previous) = unique.insert(key, fixing.value) {
                assert_eq!(previous, fixing.value, "conflicting reference observation");
            }
        }
        unique
            .into_iter()
            .map(|((series_id, date), value)| ProjectedFixing {
                series_id,
                date,
                value,
            })
            .collect()
    }

    fn record(&self) -> Value {
        json!({
            "pv_usd": self.pv(), "pv_fixed_usd": self.fixed_pv,
            "pv_float_usd": self.float_pv, "annuity": self.annuity,
            "par_rate_decimal": self.par(), "observations": self.fixings(),
            "bounds": {"factor": FACTOR_BOUND, "decimal_rate": FACTOR_BOUND,
                "raw_rate": RAW_RATE_BOUND, "pv_usd": self.pv_bound,
                "fixed_pv_usd": FIXED_PV_BOUND, "annuity": ANNUITY_BOUND,
                "par_rate_decimal": self.par_bound()},
            "fixed_rows": self.fixed_rows.iter().map(|(payment, alpha, amount, kind)| json!({
                "payment": payment.to_string(), "alpha": alpha,
                "unsigned_amount_usd": amount, "kind": kind,
            })).collect::<Vec<_>>(),
            "coupons": self.coupons.iter().map(|coupon| json!({
                "start": coupon.grid.start.to_string(), "end": coupon.grid.end.to_string(),
                "payment": coupon.grid.payment.to_string(), "alpha": coupon.grid.alpha,
                "factor": coupon.factor, "rate": coupon.rate,
                "unsigned_amount_usd": coupon.amount, "discount": coupon.discount,
                "grid": coupon.grid.observations.iter().map(|observation| json!({
                    "start": observation.start.to_string(), "end": observation.end.to_string(),
                    "rate_alpha": observation.rate_alpha, "factor_alpha": observation.factor_alpha,
                })).collect::<Vec<_>>(),
            })).collect::<Vec<_>>(),
        })
    }
}

fn daily_grid(start: Date, end: Date, calendar_id: &str, denominator: f64) -> Vec<Observation> {
    let calendar = calendar_by_id_strict(calendar_id).unwrap();
    let mut observations = Vec::new();
    let mut current = start;
    while current < end {
        let next = current.add_business_days(1, calendar).unwrap().min(end);
        let alpha = (next - current).whole_days() as f64 / denominator;
        observations.push(Observation {
            start: current,
            end: next,
            rate_alpha: alpha,
            factor_alpha: alpha,
        });
        current = next;
    }
    observations
}

// The optional forward ID is an explicit oracle input, never inferred from
// whichever curve happens to be available in the market under a generic ID.
fn raw_rate(
    market: &MarketContext,
    as_of: Date,
    forward_id: Option<&str>,
    observation: &Observation,
) -> f64 {
    let index_id = forward_id.unwrap_or("USD-OIS");
    if observation.start < as_of {
        return market
            .get_series(format!("FIXING:{index_id}"))
            .unwrap()
            .value_on_exact(observation.start)
            .unwrap();
    }
    match forward_id {
        Some(id) => {
            let curve = market.get_forward(id).unwrap();
            let t0 = curve
                .day_count()
                .year_fraction(
                    curve.base_date(),
                    observation.start,
                    DayCountContext::default(),
                )
                .unwrap();
            let t1 = curve
                .day_count()
                .year_fraction(
                    curve.base_date(),
                    observation.end,
                    DayCountContext::default(),
                )
                .unwrap();
            curve.rate_period(t0, t1).unwrap() * (t1 - t0) / observation.rate_alpha
        }
        None => {
            let curve = market.get_discount("USD-OIS").unwrap();
            (curve.df_on_date(observation.start).unwrap()
                / curve.df_on_date(observation.end).unwrap()
                - 1.0)
                / observation.rate_alpha
        }
    }
}

fn product(
    market: &MarketContext,
    as_of: Date,
    forward_id: Option<&str>,
    observations: &[Observation],
    compounded_spread: f64,
) -> (f64, Vec<ProjectedFixing>) {
    let mut factor = 1.0;
    let mut fixings = Vec::new();
    for observation in observations {
        let rate = raw_rate(market, as_of, forward_id, observation);
        assert!(rate.is_finite());
        factor *= 1.0 + (rate + compounded_spread) * observation.factor_alpha;
        fixings.push(ProjectedFixing {
            series_id: format!("FIXING:{}", forward_id.unwrap_or("USD-OIS")),
            date: observation.start,
            value: Some(rate),
        });
    }
    (factor, fixings)
}

fn coupon_oracle(
    swap: &InterestRateSwap,
    market: &MarketContext,
    as_of: Date,
    forward_id: Option<&str>,
    grid: CouponGrid,
) -> CouponOracle {
    let (factor, fixings) = product(market, as_of, forward_id, &grid.observations, 0.0);
    let spread = swap.float_leg.spread_bp.to_f64().unwrap() * 1e-4;
    let curve = market.get_discount("USD-OIS").unwrap();
    let discount = curve.df_on_date(grid.payment).unwrap() / curve.df_on_date(as_of).unwrap();
    CouponOracle {
        rate: (factor - 1.0) / grid.alpha + spread,
        amount: swap.notional.amount() * ((factor - 1.0) + spread * grid.alpha),
        grid,
        factor,
        discount,
        fixings,
    }
}

fn assemble_oracle(
    swap: &InterestRateSwap,
    market: &MarketContext,
    as_of: Date,
    coupons: Vec<CouponOracle>,
) -> Oracle {
    assert_eq!(swap.side, PayReceive::Pay);
    assert_eq!(swap.fixed_leg.rate, dec!(0.04));
    assert_eq!(swap.fixed_leg.day_count, DayCount::Act360);
    assert!(!swap.adjust_fixed_accrual_dates);
    let curve = market.get_discount("USD-OIS").unwrap();
    let as_of_df = curve.df_on_date(as_of).unwrap();
    let periods = build_periods(swap.fixed_leg.schedule_params(false)).unwrap();
    let kind = match swap.fixed_leg.stub {
        StubKind::None => CFKind::Fixed,
        StubKind::ShortFront => {
            assert_eq!(
                periods.len(),
                1,
                "only the frozen short single-coupon fixtures"
            );
            CFKind::Stub
        }
        other => panic!("unexpected reference fixture stub {other:?}"),
    };
    let fixed_rows: Vec<_> = periods
        .iter()
        .filter(|period| period.payment_date > as_of)
        .map(|period| {
            let alpha = (period.accrual_end - period.accrual_start).whole_days() as f64 / 360.0;
            close(
                "manual fixed accrual",
                period.accrual_year_fraction,
                alpha,
                1e-15,
            );
            (
                period.payment_date,
                alpha,
                swap.notional.amount() * 0.04 * alpha,
                kind,
            )
        })
        .collect();
    let annuity = fixed_rows
        .iter()
        .map(|(payment, alpha, _, _)| alpha * curve.df_on_date(*payment).unwrap() / as_of_df)
        .sum::<f64>();
    let float_pv = coupons
        .iter()
        .map(|coupon| coupon.amount * coupon.discount)
        .sum();
    let pv_bound = (swap.notional.amount().abs()
        * FACTOR_BOUND
        * coupons
            .iter()
            .map(|coupon| coupon.discount.abs())
            .sum::<f64>())
    .max(1e-8);
    Oracle {
        notional: swap.notional.amount(),
        fixed_pv: swap.notional.amount() * 0.04 * annuity,
        float_pv,
        annuity,
        pv_bound,
        fixed_rows,
        coupons,
    }
}

fn full_oracle(
    swap: &InterestRateSwap,
    market: &MarketContext,
    as_of: Date,
    forward_id: Option<&str>,
    fixing_calendar_id: &str,
    denominator: f64,
) -> Oracle {
    assert_eq!(
        swap.float_leg.compounding,
        FloatingLegCompounding::CompoundedInArrears { lookback_days: 0 }
    );
    assert!(!swap.adjust_float_accrual_dates);
    let float = &swap.float_leg;
    let calendar = calendar_by_id_strict(fixing_calendar_id).unwrap();
    let coupons = build_periods(float.schedule_params(float.calendar_or_weekends(), None, false))
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
            let end = adjust(period.accrual_end, float.business_day_convention, calendar).unwrap();
            coupon_oracle(
                swap,
                market,
                as_of,
                forward_id,
                CouponGrid {
                    start,
                    end,
                    payment: period.payment_date,
                    alpha: (end - start).whole_days() as f64 / denominator,
                    observations: daily_grid(start, end, fixing_calendar_id, denominator),
                },
            )
        })
        .collect();
    assemble_oracle(swap, market, as_of, coupons)
}

fn check_price(
    swap: &InterestRateSwap,
    market: &MarketContext,
    as_of: Date,
    oracle: &Oracle,
) -> (CashFlowSchedule, Value) {
    assert_eq!(swap.default_model(), ModelKey::Discounting);
    let before = swap.cashflow_schedule(market, as_of).unwrap();
    let before_json = serde_json::to_value(&before).unwrap();
    let value = swap.value(market, as_of).unwrap();
    assert_eq!(value.currency(), Currency::USD);
    close("control PV", value.amount(), oracle.pv(), oracle.pv_bound);
    assert_eq!(
        before_json,
        serde_json::to_value(swap.cashflow_schedule(market, as_of).unwrap()).unwrap()
    );
    assert_eq!(
        before.get_meta().representation,
        CashflowRepresentation::Projected
    );
    assert_eq!(
        before.get_flows().len(),
        oracle.fixed_rows.len() + oracle.coupons.len()
    );
    let floating: Vec<_> = before
        .get_flows()
        .iter()
        .filter(|flow| flow.kind == CFKind::FloatReset)
        .collect();
    assert_eq!(floating.len(), oracle.coupons.len());
    let spread = swap.float_leg.spread_bp.to_f64().unwrap() * 1e-4;
    for (flow, expected) in floating.iter().zip(&oracle.coupons) {
        assert_eq!(flow.date, expected.grid.payment);
        assert_eq!(flow.amount.currency(), Currency::USD);
        close(
            "control floating accrual",
            flow.accrual_factor,
            expected.grid.alpha,
            1e-15,
        );
        close(
            "control coupon amount",
            flow.amount.amount(),
            expected.amount,
            oracle.notional.abs() * FACTOR_BOUND,
        );
        close(
            "control coupon rate",
            flow.rate.unwrap(),
            expected.rate,
            FACTOR_BOUND,
        );
        close(
            "control compound factor",
            1.0 + (flow.rate.unwrap() - spread) * flow.accrual_factor,
            expected.factor,
            FACTOR_BOUND,
        );
    }
    let fixed: Vec<_> = before
        .get_flows()
        .iter()
        .filter(|flow| matches!(flow.kind, CFKind::Fixed | CFKind::Stub))
        .collect();
    assert_eq!(fixed.len(), oracle.fixed_rows.len());
    for (flow, (payment, alpha, amount, kind)) in fixed.iter().zip(&oracle.fixed_rows) {
        assert_eq!(flow.date, *payment);
        assert_eq!(flow.kind, *kind);
        assert_eq!(flow.amount.currency(), Currency::USD);
        close("control fixed accrual", flow.accrual_factor, *alpha, 1e-15);
        close(
            "control signed fixed amount",
            flow.amount.amount(),
            -*amount,
            FIXED_PV_BOUND,
        );
        close("control fixed rate", flow.rate.unwrap(), 0.04, FACTOR_BOUND);
    }
    let expected_fixings = oracle.fixings();
    assert_eq!(
        before.get_meta().projected_fixings.len(),
        expected_fixings.len()
    );
    for (actual, expected) in before
        .get_meta()
        .projected_fixings
        .iter()
        .zip(&expected_fixings)
    {
        assert_eq!(actual.series_id, expected.series_id);
        assert_eq!(actual.date, expected.date);
        close(
            "control raw fixing",
            actual.value.unwrap(),
            expected.value.unwrap(),
            RAW_RATE_BOUND,
        );
    }
    assert_eq!(
        swap.dated_cashflows(market, as_of).unwrap(),
        before.dated_flows().unwrap()
    );
    let curve = market.get_discount("USD-OIS").unwrap();
    let exported_pv = before
        .get_flows()
        .iter()
        .map(|flow| {
            flow.amount.amount() * curve.df_on_date(flow.date).unwrap()
                / curve.df_on_date(as_of).unwrap()
        })
        .sum::<f64>();
    close(
        "discounted public export",
        value.amount(),
        exported_pv,
        oracle.pv_bound,
    );
    let record = json!({"value": value, "model": swap.default_model(), "reference": oracle.record(), "cashflows": before});
    (before, record)
}

fn history_market(
    swap: &InterestRateSwap,
    market: &MarketContext,
    as_of: Date,
    fixing_calendar_id: &str,
) -> MarketContext {
    let calendar = calendar_by_id_strict(fixing_calendar_id).unwrap();
    let float = &swap.float_leg;
    let mut observations = BTreeMap::new();
    for period in
        build_periods(float.schedule_params(float.calendar_or_weekends(), None, false)).unwrap()
    {
        let mut current = adjust(
            period.accrual_start,
            float.business_day_convention,
            calendar,
        )
        .unwrap();
        let end = adjust(period.accrual_end, float.business_day_convention, calendar).unwrap();
        while current < as_of && current < end {
            observations.insert(current, 0.05);
            current = current.add_business_days(1, calendar).unwrap();
        }
    }
    market.clone().insert_series(
        ScalarTimeSeries::new(
            format!("FIXING:{}", float.forward_curve_id),
            observations.into_iter().collect(),
            None,
        )
        .unwrap(),
    )
}

fn metrics_and_dv01(
    swap: &InterestRateSwap,
    market: &MarketContext,
    as_of: Date,
    oracle: &Oracle,
    forward_id: Option<&str>,
    calendar_id: &str,
    denominator: f64,
) -> Value {
    let requested = [
        MetricId::PvFixed,
        MetricId::PvFloat,
        MetricId::ParRate,
        MetricId::Dv01,
    ];
    let result = swap
        .price_with_metrics(market, as_of, &requested, PricingOptions::default())
        .unwrap();
    assert_eq!(result.value.currency(), Currency::USD);
    close(
        "metric request PV",
        result.value.amount(),
        oracle.pv(),
        oracle.pv_bound,
    );
    for metric in &requested {
        assert!(result.measures[metric].is_finite());
    }
    for (metric, expected, bound) in [
        (MetricId::PvFixed, oracle.fixed_pv, FIXED_PV_BOUND),
        (MetricId::PvFloat, oracle.float_pv, oracle.pv_bound),
        (MetricId::ParRate, oracle.par(), oracle.par_bound()),
    ] {
        close(metric.as_str(), result.measures[&metric], expected, bound);
    }
    let annuity_result = swap
        .price_with_metrics(
            market,
            as_of,
            &[MetricId::Annuity],
            PricingOptions::default(),
        )
        .unwrap();
    close(
        "separately requested annuity",
        annuity_result.measures[&MetricId::Annuity],
        oracle.annuity,
        ANNUITY_BOUND,
    );
    let mut ids = vec!["USD-OIS"];
    if let Some(id) = forward_id {
        assert_ne!(id, "USD-OIS");
        ids.push(id);
    }
    let bump = |width| {
        market
            .bump(ids.iter().map(|id| MarketBump::Curve {
                id: (*id).into(),
                spec: BumpSpec::parallel_bp(width),
            }))
            .unwrap()
    };
    let up = bump(1.0);
    let down = bump(-1.0);
    let up_oracle = full_oracle(swap, &up, as_of, forward_id, calendar_id, denominator);
    let down_oracle = full_oracle(swap, &down, as_of, forward_id, calendar_id, denominator);
    let (_, up_record) = check_price(swap, &up, as_of, &up_oracle);
    let (_, down_record) = check_price(swap, &down, as_of, &down_oracle);
    let expected = (up_oracle.pv() - down_oracle.pv()) / 2.0;
    let bound = (up_oracle.pv_bound + down_oracle.pv_bound) / 2.0;
    close(
        "control combined DV01",
        result.measures[&MetricId::Dv01],
        expected,
        bound,
    );
    json!({"requested": requested, "measures": result.measures,
        "separate_annuity_request": annuity_result.measures,
        "bump_curve_ids": ids, "bump_width_bp": 1.0, "calibration": null,
        "dv01_reference": expected, "dv01_bound": bound,
        "up": {"market": up, "result": up_record},
        "down": {"market": down, "result": down_record}})
}

fn suffix_oracle(
    swap: &InterestRateSwap,
    market: &MarketContext,
    as_of: Date,
    calendar_id: &str,
    denominator: f64,
) -> Oracle {
    let daily = full_oracle(swap, market, as_of, None, calendar_id, denominator);
    let curve = market.get_discount("USD-OIS").unwrap();
    let mut coupons = daily.coupons;
    for coupon in &mut coupons {
        let historical = coupon
            .grid
            .observations
            .iter()
            .take_while(|observation| observation.start < as_of);
        let mut factor = historical.fold(1.0, |product, observation| {
            product * (1.0 + raw_rate(market, as_of, None, observation) * observation.factor_alpha)
        });
        if let Some(first_future) = coupon
            .grid
            .observations
            .iter()
            .find(|observation| observation.start >= as_of)
        {
            factor *= curve.df_on_date(first_future.start).unwrap()
                / curve.df_on_date(coupon.grid.end).unwrap();
        }
        close(
            "daily versus historical-prefix/DF-suffix factor",
            coupon.factor,
            factor,
            FACTOR_BOUND,
        );
        coupon.factor = factor;
        let spread = swap.float_leg.spread_bp.to_f64().unwrap() * 1e-4;
        coupon.rate = (factor - 1.0) / coupon.grid.alpha + spread;
        coupon.amount = swap.notional.amount() * ((factor - 1.0) + spread * coupon.grid.alpha);
    }
    assemble_oracle(swap, market, as_of, coupons)
}

#[test]
fn ois_control_lookback_shift_and_cutoff_match_manual_products() {
    let as_of = date!(2025 - 01 - 02);
    let shifted_pairs = [
        (date!(2024 - 12 - 31), date!(2025 - 01 - 01)),
        (date!(2025 - 01 - 01), date!(2025 - 01 - 02)),
        (date!(2025 - 01 - 02), date!(2025 - 01 - 03)),
        (date!(2025 - 01 - 03), date!(2025 - 01 - 06)),
        (date!(2025 - 01 - 06), date!(2025 - 01 - 07)),
        (date!(2025 - 01 - 07), date!(2025 - 01 - 08)),
    ];
    let cutoff_pairs = [
        (date!(2025 - 01 - 02), date!(2025 - 01 - 03)),
        (date!(2025 - 01 - 03), date!(2025 - 01 - 06)),
        (date!(2025 - 01 - 06), date!(2025 - 01 - 07)),
        (date!(2025 - 01 - 07), date!(2025 - 01 - 08)),
        (date!(2025 - 01 - 07), date!(2025 - 01 - 08)),
        (date!(2025 - 01 - 07), date!(2025 - 01 - 08)),
    ];
    let shifted_dates = vec![
        date!(2024 - 12 - 31),
        date!(2025 - 01 - 01),
        date!(2025 - 01 - 02),
        date!(2025 - 01 - 03),
        date!(2025 - 01 - 06),
        date!(2025 - 01 - 07),
    ];
    let cutoff_dates = vec![
        date!(2025 - 01 - 02),
        date!(2025 - 01 - 03),
        date!(2025 - 01 - 06),
        date!(2025 - 01 - 07),
    ];
    let base_weights = [1.0, 3.0, 1.0, 1.0, 1.0, 1.0];
    let shift_weights = [1.0, 1.0, 1.0, 3.0, 1.0, 1.0];
    let mut factors = Vec::new();
    for (label, compounding, pairs, weights, expected_dates) in [
        (
            "lookback_2bd",
            FloatingLegCompounding::CompoundedInArrears { lookback_days: 2 },
            shifted_pairs,
            base_weights,
            &shifted_dates,
        ),
        (
            "observation_shift_2bd",
            FloatingLegCompounding::CompoundedWithObservationShift { shift_days: 2 },
            shifted_pairs,
            shift_weights,
            &shifted_dates,
        ),
        (
            "cutoff_2bd",
            FloatingLegCompounding::CompoundedWithRateCutoff { cutoff_days: 2 },
            cutoff_pairs,
            base_weights,
            &cutoff_dates,
        ),
    ] {
        let mut swap = compact_swap();
        swap.float_leg.compounding = compounding;
        let market = if label == "cutoff_2bd" {
            compact_market()
        } else {
            compact_market().insert_series(
                ScalarTimeSeries::new(
                    "FIXING:USD-OIS",
                    vec![
                        (date!(2024 - 12 - 31), 0.031),
                        (date!(2025 - 01 - 01), 0.047),
                    ],
                    None,
                )
                .unwrap(),
            )
        };
        let observations = pairs
            .iter()
            .zip(weights)
            .map(|(&(start, end), days)| Observation {
                start,
                end,
                rate_alpha: (end - start).whole_days() as f64 / 360.0,
                factor_alpha: days / 360.0,
            })
            .collect();
        let coupon = coupon_oracle(
            &swap,
            &market,
            as_of,
            None,
            CouponGrid {
                start: as_of,
                end: date!(2025 - 01 - 10),
                payment: date!(2025 - 01 - 14),
                alpha: 8.0 / 360.0,
                observations,
            },
        );
        factors.push(coupon.factor);
        let oracle = assemble_oracle(&swap, &market, as_of, vec![coupon]);
        let (schedule, record) = check_price(&swap, &market, as_of, &oracle);
        assert_eq!(
            schedule
                .get_meta()
                .projected_fixings
                .iter()
                .map(|fixing| fixing.date)
                .collect::<Vec<_>>(),
            *expected_dates
        );
        let missing_error = if label == "cutoff_2bd" {
            None
        } else {
            let missing = market.clone().insert_series(
                ScalarTimeSeries::new("FIXING:USD-OIS", vec![(date!(2025 - 01 - 01), 0.047)], None)
                    .unwrap(),
            );
            let expected = "Missing fixing for 'USD-OIS' on 2024-12-31 (valuation date: 2025-01-02). The fixing series exists but has no exact observation: Requested item not found: series 'FIXING:USD-OIS' observation on 2024-12-31";
            for error in [
                swap.value(&missing, as_of).unwrap_err(),
                swap.cashflow_schedule(&missing, as_of).unwrap_err(),
            ] {
                let finstack_quant_core::Error::Validation(message) = error else {
                    panic!("unexpected shifted-fixing error {error:?}")
                };
                assert_eq!(message, expected);
            }
            Some(expected)
        };
        emit(
            &format!("ois_p3/{label}"),
            json!({
                "fixture": {"instrument": swap, "market": market, "as_of": as_of.to_string(), "source": "discount", "fixing_calendar": "weekends_only"},
                "result": record, "missing_shifted_fixing_error": missing_error,
            }),
        );
    }
    assert!(
        (factors[0] - factors[1]).abs() > FACTOR_BOUND,
        "unequal historical rates must distinguish shifted and unshifted weights"
    );
}

#[test]
fn ois_control_explicit_forward_sources_match_manual_products() {
    let as_of = date!(2025 - 01 - 01);
    let base = benchmark_market();
    let mut swap = benchmark_swap(5);
    swap.float_leg.forward_curve_id = "USD-SOFR-OIS".into();
    let explicit = ForwardCurve::builder("USD-SOFR-OIS", 1.0 / 360.0)
        .base_date(as_of)
        .day_count(DayCount::Act360)
        .knots([
            (0.0, 0.035),
            (1.0, 0.038),
            (2.0, 0.040),
            (5.0, 0.045),
            (10.0, 0.050),
        ])
        .interp(InterpStyle::Linear)
        .build()
        .unwrap();
    let derived = base
        .get_discount("USD-OIS")
        .unwrap()
        .to_forward_curve("USD-SOFR-OIS", 1.0 / 365.0, Some(InterpStyle::Linear))
        .unwrap();
    for (label, forward) in [
        ("explicit_forward", explicit),
        ("discount_derived_forward", derived),
    ] {
        let market = base.clone().insert(forward);
        assert!(market.get_forward("USD-OIS").is_err());
        assert_eq!(
            market.get_forward("USD-SOFR-OIS").unwrap().base_date(),
            as_of
        );
        let oracle = full_oracle(&swap, &market, as_of, Some("USD-SOFR-OIS"), "usny", 360.0);
        assert_eq!(oracle.coupons.len(), 5);
        let (_, record) = check_price(&swap, &market, as_of, &oracle);
        let risk = if label == "explicit_forward" {
            Some(metrics_and_dv01(
                &swap,
                &market,
                as_of,
                &oracle,
                Some("USD-SOFR-OIS"),
                "usny",
                360.0,
            ))
        } else {
            None
        };
        let forward_only = market
            .bump([MarketBump::Curve {
                id: "USD-SOFR-OIS".into(),
                spec: BumpSpec::parallel_bp(1.0),
            }])
            .unwrap();
        let shifted_oracle = full_oracle(
            &swap,
            &forward_only,
            as_of,
            Some("USD-SOFR-OIS"),
            "usny",
            360.0,
        );
        let (_, source_record) = check_price(&swap, &forward_only, as_of, &shifted_oracle);
        let expected_change = shifted_oracle.pv() - oracle.pv();
        let actual_change = swap.value(&forward_only, as_of).unwrap().amount()
            - swap.value(&market, as_of).unwrap().amount();
        let change_bound = oracle.pv_bound + shifted_oracle.pv_bound;
        assert!(
            expected_change.abs() > change_bound,
            "explicit forward must affect coupon economics"
        );
        close(
            "forward-only source sensitivity",
            actual_change,
            expected_change,
            change_bound,
        );
        emit(
            &format!("ois_p3/{label}"),
            json!({
                "fixture": {"instrument": swap, "market": market, "as_of": as_of.to_string(), "source": "forward", "forward_id": "USD-SOFR-OIS", "fixing_calendar": "usny", "payment_calendar": "weekends_only"},
                "result": record, "risk": risk,
                "forward_only_bump": {"width_bp": 1.0, "expected_change_usd": expected_change, "actual_change_usd": actual_change, "bound_usd": change_bound, "result": source_record},
            }),
        );
    }
}

#[test]
fn ois_control_seasoned_and_arithmetic_spread_match_references() {
    let base = benchmark_market();
    let swap = benchmark_swap(5);
    let seasoned_as_of = date!(2025 - 07 - 01);
    let seasoned = history_market(&swap, &base, seasoned_as_of, "usny");
    let oracle = suffix_oracle(&swap, &seasoned, seasoned_as_of, "usny", 360.0);
    let (_, record) = check_price(&swap, &seasoned, seasoned_as_of, &oracle);
    let risk = metrics_and_dv01(
        &swap,
        &seasoned,
        seasoned_as_of,
        &oracle,
        None,
        "usny",
        360.0,
    );
    emit(
        "ois_p3/seasoned_5Y",
        json!({
            "fixture": {"instrument": swap, "market": seasoned, "as_of": seasoned_as_of.to_string(), "source": "discount", "fixing_calendar": "usny", "payment_calendar": "weekends_only"},
            "result": record, "risk": risk,
        }),
    );

    let as_of = date!(2025 - 01 - 01);
    let zero_oracle = full_oracle(&swap, &base, as_of, None, "usny", 360.0);
    let (zero_schedule, zero_record) = check_price(&swap, &base, as_of, &zero_oracle);
    let mut spread_swap = swap.clone();
    spread_swap.float_leg.spread_bp = dec!(10);
    let spread_oracle = full_oracle(&spread_swap, &base, as_of, None, "usny", 360.0);
    let (spread_schedule, spread_record) = check_price(&spread_swap, &base, as_of, &spread_oracle);
    assert_eq!(
        zero_schedule.get_meta().projected_fixings,
        spread_schedule.get_meta().projected_fixings
    );
    let extra_pv = zero_oracle
        .coupons
        .iter()
        .map(|coupon| NOTIONAL * 0.001 * coupon.grid.alpha * coupon.discount)
        .sum::<f64>();
    close(
        "arithmetic spread PV increment",
        swap.value(&base, as_of).unwrap().amount() + extra_pv,
        spread_swap.value(&base, as_of).unwrap().amount(),
        zero_oracle.pv_bound + spread_oracle.pv_bound,
    );
    emit(
        "ois_p3/arithmetic_spread_10bp",
        json!({
            "fixture": {"instrument": spread_swap, "market": base, "as_of": as_of.to_string(), "source": "discount", "fixing_calendar": "usny", "payment_calendar": "weekends_only"},
            "zero_spread": zero_record, "spread": spread_record, "expected_extra_pv_usd": extra_pv,
        }),
    );
}

fn projection_record(projection: &OvernightCouponProjection) -> Value {
    json!({
        "factor": projection.compound_factor, "rate": projection.rate,
        "alpha": projection.accrual_year_fraction, "fixing_date": projection.fixing_date.to_string(),
        "parallel_forward_sensitivity": projection.parallel_forward_sensitivity,
        "parallel_forward_second_sensitivity": projection.parallel_forward_second_sensitivity,
        "observations": projection.observation_exposures.iter().map(|observation| json!({
            "start": observation.observation_start.to_string(), "end": observation.observation_end.to_string(),
            "raw_rate": observation.projected_rate, "rate_alpha": observation.rate_accrual_year_fraction,
            "factor_alpha": observation.factor_accrual_year_fraction, "derivative": observation.coupon_forward_derivative,
        })).collect::<Vec<_>>(),
    })
}

#[test]
fn ois_control_compounded_spread_stays_on_daily_projection() {
    let as_of = date!(2025 - 01 - 01);
    let market = benchmark_market();
    let swap = benchmark_swap(1);
    let oracle = full_oracle(&swap, &market, as_of, None, "usny", 360.0);
    let grid = &oracle.coupons[0].grid;
    let (expected_factor, expected_fixings) =
        product(&market, as_of, None, &grid.observations, 0.001);
    let expected_rate = (expected_factor - 1.0) / grid.alpha;
    let curve = market.get_discount("USD-OIS").unwrap();
    let calendar = calendar_by_id_strict("usny").unwrap();
    let compounding = FloatingLegCompounding::CompoundedInArrears { lookback_days: 0 };
    let project = |need_observation_exposures| {
        project_overnight_coupon(OvernightCouponProjectionInput {
            curve: OvernightProjectionCurve::Discount(curve.as_ref()),
            fixings: None,
            fixing_id: "USD-OIS",
            as_of,
            accrual_start: grid.start,
            accrual_end: grid.end,
            day_count: DayCount::Act360,
            coupon_frequency: Some(Tenor::annual()),
            coupon_period: (grid.start, grid.end),
            compounding: &compounding,
            fixing_calendar: calendar,
            compounded_spread: 0.001,
            need_observation_exposures,
        })
        .unwrap()
    };
    let economics = project(false);
    let diagnostics = project(true);
    for projection in [&economics, &diagnostics] {
        close(
            "compounded-spread factor",
            projection.compound_factor,
            expected_factor,
            FACTOR_BOUND,
        );
        close(
            "compounded-spread rate",
            projection.rate,
            expected_rate,
            FACTOR_BOUND,
        );
        close(
            "compounded-spread accrual",
            projection.accrual_year_fraction,
            grid.alpha,
            1e-15,
        );
        assert_eq!(
            projection.fixing_date,
            grid.observations.last().unwrap().start
        );
        assert!(projection.parallel_forward_sensitivity.is_finite());
        assert!(projection.parallel_forward_second_sensitivity.is_finite());
    }
    assert_eq!(
        economics.accrual_year_fraction,
        diagnostics.accrual_year_fraction
    );
    assert_eq!(economics.fixing_date, diagnostics.fixing_date);
    close(
        "compounded-spread modes factor",
        economics.compound_factor,
        diagnostics.compound_factor,
        FACTOR_BOUND,
    );
    close(
        "compounded-spread modes rate",
        economics.rate,
        diagnostics.rate,
        FACTOR_BOUND,
    );
    assert_eq!(
        diagnostics.observation_exposures.len(),
        grid.observations.len()
    );
    for ((actual, expected), fixing) in diagnostics
        .observation_exposures
        .iter()
        .zip(&grid.observations)
        .zip(&expected_fixings)
    {
        assert_eq!(actual.observation_start, expected.start);
        assert_eq!(actual.observation_end, expected.end);
        close(
            "compounded-spread raw rate",
            actual.projected_rate,
            fixing.value.unwrap(),
            RAW_RATE_BOUND,
        );
        close(
            "compounded-spread rate weight",
            actual.rate_accrual_year_fraction,
            expected.rate_alpha,
            1e-15,
        );
        close(
            "compounded-spread factor weight",
            actual.factor_accrual_year_fraction,
            expected.factor_alpha,
            1e-15,
        );
    }
    emit(
        "ois_p3/compounded_spread_10bp",
        json!({
            "fixture": {"market": market, "as_of": as_of.to_string(), "source": "discount", "start": grid.start.to_string(), "end": grid.end.to_string(), "day_count": "act_360", "coupon_frequency": "1Y", "fixing_calendar": "usny", "compounded_spread": 0.001},
            "reference": {"factor": expected_factor, "rate": expected_rate, "alpha": grid.alpha, "observations": expected_fixings, "factor_bound": FACTOR_BOUND, "rate_bound": FACTOR_BOUND, "raw_rate_bound": RAW_RATE_BOUND},
            "economics": projection_record(&economics), "diagnostics": projection_record(&diagnostics),
        }),
    );
}

#[test]
fn ois_control_holiday_adjusted_spread_matches_full_export() {
    let as_of = date!(2024 - 12 - 02);
    let mut swap = benchmark_swap(1);
    swap.id = "OIS-HOLIDAY-SPREAD".into();
    swap.notional = Money::new(1_000_000.0, Currency::USD).unwrap();
    swap.fixed_leg.start = date!(2025 - 01 - 02);
    swap.fixed_leg.end = date!(2025 - 07 - 04);
    swap.fixed_leg.calendar_id = Some("usny".into());
    swap.fixed_leg.business_day_convention = BusinessDayConvention::ModifiedFollowing;
    swap.fixed_leg.stub = StubKind::ShortFront;
    swap.float_leg.start = swap.fixed_leg.start;
    swap.float_leg.end = swap.fixed_leg.end;
    swap.float_leg.calendar_id = Some("usny".into());
    swap.float_leg.fixing_calendar_id = Some("usny".into());
    swap.float_leg.business_day_convention = BusinessDayConvention::ModifiedFollowing;
    swap.float_leg.stub = StubKind::ShortFront;
    swap.float_leg.spread_bp = dec!(100);
    swap.validate().unwrap();
    let market = MarketContext::new().insert(
        DiscountCurve::builder("USD-OIS")
            .base_date(as_of)
            .day_count(DayCount::Act365F)
            .knots([(0.0, 1.0), (2.0, 1.0)])
            .interp(InterpStyle::Linear)
            .build()
            .unwrap(),
    );
    let oracle = full_oracle(&swap, &market, as_of, None, "usny", 360.0);
    assert_eq!(oracle.coupons.len(), 1);
    assert_eq!(oracle.fixed_rows.len(), 1);
    let coupon = &oracle.coupons[0];
    assert_eq!(coupon.grid.end, date!(2025 - 07 - 07));
    assert_eq!(coupon.grid.payment, date!(2025 - 07 - 09));
    assert_eq!(oracle.fixed_rows[0].0, date!(2025 - 07 - 09));
    close(
        "holiday fixed contractual accrual",
        oracle.fixed_rows[0].1,
        183.0 / 360.0,
        1e-15,
    );
    close(
        "holiday floating adjusted accrual",
        coupon.grid.alpha,
        186.0 / 360.0,
        1e-15,
    );
    close(
        "holiday zero-index factor",
        coupon.factor,
        1.0,
        FACTOR_BOUND,
    );
    close(
        "holiday spread cash",
        coupon.amount,
        1_000_000.0 * 0.01 * 186.0 / 360.0,
        1e-9,
    );
    let (schedule, record) = check_price(&swap, &market, as_of, &oracle);
    let floating = schedule
        .get_flows()
        .iter()
        .find(|flow| flow.kind == CFKind::FloatReset)
        .unwrap();
    close(
        "holiday actual spread cash",
        floating.amount.amount(),
        1_000_000.0 * 0.01 * 186.0 / 360.0,
        1e-9,
    );
    emit(
        "ois_p3/holiday_adjusted_spread",
        json!({
            "fixture": {"instrument": swap, "market": market, "as_of": as_of.to_string(), "source": "discount", "fixing_calendar": "usny", "payment_calendar": "usny"},
            "result": record,
        }),
    );
}

#[test]
fn ois_control_act365l_preserves_full_coupon_context() {
    let origin = date!(2023 - 07 - 03);
    let as_of = date!(2024 - 03 - 04);
    let end = date!(2024 - 07 - 03);
    let mut swap = benchmark_swap(1);
    swap.id = "OIS-ACT365L-FULL-COUPON".into();
    swap.fixed_leg.start = origin;
    swap.fixed_leg.end = end;
    swap.fixed_leg.calendar_id = Some("weekends_only".into());
    swap.float_leg.start = origin;
    swap.float_leg.end = end;
    swap.float_leg.calendar_id = Some("weekends_only".into());
    swap.float_leg.fixing_calendar_id = Some("weekends_only".into());
    swap.float_leg.day_count = DayCount::Act365L;
    swap.validate().unwrap();
    let base = MarketContext::new().insert(
        DiscountCurve::builder("USD-OIS")
            .base_date(origin)
            .day_count(DayCount::Act365F)
            .knots([(0.0, 1.0), (1.0, 0.96), (5.0, 0.80)])
            .interp(InterpStyle::Linear)
            .build()
            .unwrap(),
    );
    let full_grid = daily_grid(origin, end, "weekends_only", 366.0);
    assert_eq!(full_grid.len(), 262);
    assert_eq!(
        full_grid.iter().filter(|item| item.start < as_of).count(),
        175
    );
    assert_eq!(
        full_grid
            .iter()
            .filter(|item| (item.end - item.start).whole_days() == 3)
            .count(),
        52
    );
    let historical_factor =
        (1.0_f64 + 0.05 / 366.0).powi(140) * (1.0_f64 + 3.0 * 0.05 / 366.0).powi(35);
    for forward_id in [None, Some("USD-SOFR-OIS")] {
        let mut variant = swap.clone();
        let market = if let Some(id) = forward_id {
            variant.float_leg.forward_curve_id = id.into();
            base.clone().insert(
                ForwardCurve::builder(id, 1.0 / 365.0)
                    .base_date(origin)
                    .day_count(DayCount::Act365F)
                    .knots([(0.0, 0.05 * 365.0 / 366.0), (5.0, 0.05 * 365.0 / 366.0)])
                    .interp(InterpStyle::Linear)
                    .build()
                    .unwrap(),
            )
        } else {
            base.clone()
        };
        let market = history_market(&variant, &market, as_of, "weekends_only");
        let oracle = if forward_id.is_none() {
            suffix_oracle(&variant, &market, as_of, "weekends_only", 366.0)
        } else {
            full_oracle(&variant, &market, as_of, forward_id, "weekends_only", 366.0)
        };
        assert_eq!(oracle.coupons.len(), 1);
        assert_eq!(oracle.fixed_rows.len(), 1);
        let coupon = &oracle.coupons[0];
        assert_eq!(coupon.grid.start, origin);
        assert_eq!(coupon.grid.end, end);
        assert_eq!(coupon.grid.payment, date!(2024 - 07 - 05));
        // The complete annual coupon has 366 actual days over denominator 366.
        close(
            "ACT365L full coupon denominator",
            coupon.grid.alpha,
            1.0,
            1e-15,
        );
        close(
            "ACT360 fixed coupon",
            oracle.fixed_rows[0].1,
            366.0 / 360.0,
            1e-15,
        );
        let count_reference = if forward_id.is_some() {
            for fixing in &coupon.fixings {
                close(
                    "ACT365L converted raw index",
                    fixing.value.unwrap(),
                    0.05,
                    RAW_RATE_BOUND,
                );
            }
            (1.0_f64 + 0.05 / 366.0).powi(210) * (1.0_f64 + 3.0 * 0.05 / 366.0).powi(52)
        } else {
            let curve = market.get_discount("USD-OIS").unwrap();
            historical_factor * curve.df_on_date(as_of).unwrap() / curve.df_on_date(end).unwrap()
        };
        close(
            "ACT365L counted independent factors",
            coupon.factor,
            count_reference,
            FACTOR_BOUND,
        );
        let (_, record) = check_price(&variant, &market, as_of, &oracle);
        emit(
            if forward_id.is_some() {
                "ois_p3/act365l_explicit_forward"
            } else {
                "ois_p3/act365l_discount"
            },
            json!({
                "fixture": {"instrument": variant, "market": market, "as_of": as_of.to_string(), "source": if forward_id.is_some() { "forward" } else { "discount" }, "forward_id": forward_id, "fixing_calendar": "weekends_only", "payment_calendar": "weekends_only", "coupon_period": [origin.to_string(), end.to_string()]},
                "historical_factor_reference": historical_factor, "counted_factor_reference": count_reference,
                "historical_observations": 175, "full_observations": 262, "result": record,
            }),
        );
    }
}

#[test]
fn ois_control_nonpositive_daily_accrual_preserves_errors() {
    let month_end = (
        date!(2025 - 01 - 29),
        date!(2025 - 02 - 03),
        date!(2025 - 01 - 30),
        date!(2025 - 01 - 31),
    );
    let leap_day = (
        date!(2024 - 02 - 27),
        date!(2024 - 03 - 01),
        date!(2024 - 02 - 28),
        date!(2024 - 02 - 29),
    );
    let calendar = calendar_by_id_strict("weekends_only").unwrap();
    for (label, day_count, (start, end, zero_start, zero_end)) in [
        ("thirty360", DayCount::Thirty360, month_end),
        ("thirty_e_360", DayCount::ThirtyE360, month_end),
        ("thirty_e_360_isda", DayCount::ThirtyE360Isda, month_end),
        ("thirty_360_it", DayCount::Thirty360It, month_end),
        ("nl_365", DayCount::Nl365, leap_day),
    ] {
        let mut swap = compact_swap();
        swap.id = format!("OIS-ZERO-DAILY-ACCRUAL-{label}").into();
        swap.fixed_leg.start = start;
        swap.fixed_leg.end = end;
        swap.float_leg.start = start;
        swap.float_leg.end = end;
        swap.float_leg.day_count = day_count;
        swap.validate().unwrap();
        assert_eq!(swap.default_model(), ModelKey::Discounting);
        let context = DayCountContext {
            calendar: Some(calendar),
            frequency: Some(Tenor::annual()),
            coupon_period: Some((start, end)),
            ..DayCountContext::default()
        };
        let full_alpha = day_count.year_fraction(start, end, context).unwrap();
        assert!(full_alpha.is_finite() && full_alpha > 0.0);
        assert_eq!(
            day_count
                .year_fraction(zero_start, zero_end, context)
                .unwrap(),
            0.0,
        );
        let base = MarketContext::new().insert(
            DiscountCurve::builder("USD-OIS")
                .base_date(start)
                .day_count(DayCount::Act365F)
                .knots([(0.0, 1.0), (1.0, 0.96)])
                .interp(InterpStyle::Linear)
                .build()
                .unwrap(),
        );
        let expected_error = format!(
            "Overnight observation has non-positive day-count fraction for {zero_start} -> {zero_end}"
        );
        for (state, as_of) in [("future", start), ("seasoned", zero_start)] {
            let market = if as_of == start {
                base.clone()
            } else {
                base.clone().insert_series(
                    ScalarTimeSeries::new("FIXING:USD-OIS", vec![(start, 0.05)], None).unwrap(),
                )
            };
            assert_eq!(market.get_discount("USD-OIS").unwrap().base_date(), start);
            let mut errors = Vec::new();
            for (operation, error) in [
                ("value", swap.value(&market, as_of).unwrap_err()),
                (
                    "cashflow_schedule",
                    swap.cashflow_schedule(&market, as_of).unwrap_err(),
                ),
            ] {
                let finstack_quant_core::Error::Validation(message) = error else {
                    panic!("unexpected nonpositive-daily-accrual error {error:?}")
                };
                assert_eq!(message, expected_error);
                errors.push(
                    json!({"operation": operation, "error_kind": "validation", "message": message}),
                );
            }
            emit(
                &format!("ois_p3/nonpositive_daily_accrual/{label}/{state}"),
                json!({
                    "fixture": {"instrument": swap, "market": market,
                        "as_of": as_of.to_string(), "model": swap.default_model(),
                        "source": "discount", "fixing_calendar": "weekends_only",
                        "payment_calendar": "weekends_only"},
                    "reference": {"full_coupon_alpha": full_alpha,
                        "zero_interval": [zero_start.to_string(), zero_end.to_string()],
                        "zero_interval_alpha": 0.0, "expected_error": expected_error},
                    "errors": errors,
                }),
            );
        }
    }
}

#[test]
fn ois_control_calendar_range_exhaustion_preserves_errors() {
    let end = Date::MAX;
    let start = end.previous_day().unwrap();
    let mut swap = compact_swap();
    swap.id = "OIS-CALENDAR-RANGE-EXHAUSTION".into();
    swap.fixed_leg.start = start;
    swap.fixed_leg.end = end;
    swap.fixed_leg.business_day_convention = BusinessDayConvention::Unadjusted;
    swap.fixed_leg.payment_lag_days = 0;
    swap.float_leg.start = start;
    swap.float_leg.end = end;
    swap.float_leg.business_day_convention = BusinessDayConvention::Unadjusted;
    swap.float_leg.payment_lag_days = 0;
    swap.float_leg.reset_lag_days = 0;
    swap.float_leg.fixing_calendar_id = Some("jpto".into());
    swap.validate().unwrap();
    assert_eq!(swap.default_model(), ModelKey::Discounting);

    // ShortFront rolls backward from the supported end date. Unadjusted
    // boundaries and zero payment lag make both public schedules valid, so
    // the failure below belongs to the overnight observation advance.
    for periods in [
        build_periods(swap.fixed_leg.schedule_params(false)).unwrap(),
        build_periods(swap.float_leg.schedule_params("weekends_only", None, false)).unwrap(),
    ] {
        assert_eq!(periods.len(), 1);
        assert_eq!(periods[0].accrual_start, start);
        assert_eq!(periods[0].accrual_end, end);
        assert_eq!(periods[0].payment_date, end);
        close(
            "date-range-edge accrual",
            periods[0].accrual_year_fraction,
            1.0 / 360.0,
            1e-15,
        );
    }

    let calendar = calendar_by_id_strict("jpto").unwrap();
    // JPTO's equinox holidays come from a table that ends in 2150, so its
    // membership in year 9999 is unavailable (CORE-CONVENTIONS-004 item 7):
    // an error, never a silent "business day". The daily algorithm must ask
    // for the next business day before clamping it to the coupon end; derive
    // the exact error independently from the public date API, without using
    // either pricing route as the oracle, and require both routes to
    // propagate it unchanged.
    assert!(calendar.is_business_day(end).is_err());
    let expected_error = start.add_business_days(1, calendar).unwrap_err();
    assert!(matches!(
        &expected_error,
        finstack_quant_core::Error::Input(finstack_quant_core::InputError::NotFound { id })
            if id.contains("equinox") && id.contains("9999")
    ));
    let market = MarketContext::new().insert(
        DiscountCurve::builder("USD-OIS")
            .base_date(start)
            .day_count(DayCount::Act365F)
            .knots([(0.0, 1.0), (1.0, 0.96)])
            .interp(InterpStyle::Linear)
            .build()
            .unwrap(),
    );
    assert!(market.get_forward("USD-OIS").is_err());
    let value_error = swap.value(&market, start).unwrap_err();
    let schedule_error = swap.cashflow_schedule(&market, start).unwrap_err();
    assert_eq!(value_error, expected_error);
    assert_eq!(schedule_error, expected_error);
    emit(
        "ois_p3/calendar_range_exhaustion",
        json!({
            "fixture": {"instrument": swap, "market": market,
                "as_of": start.to_string(), "model": swap.default_model(),
                "source": "discount", "fixing_calendar": "jpto",
                "payment_calendar": "weekends_only"},
            "reference": {"advance_start": start.to_string(), "business_days": 1,
                "coupon_end": end.to_string(), "expected_error": expected_error},
            "errors": {"value": value_error, "cashflow_schedule": schedule_error},
        }),
    );
}

#[test]
fn ois_control_nonbusiness_minimum_date_preserves_success() {
    // USNY is rule-only (no lunar/equinox tables or Easter), so it is
    // defined at Date::MIN, where its New Year's Day closure applies.
    let calendar = calendar_by_id_strict("usny").unwrap();
    let start = Date::MIN;
    assert!(!calendar.is_business_day(start).expect("calendar lookup"));
    let end = start.add_business_days(1, calendar).unwrap();
    assert!(end > start && calendar.is_business_day(end).expect("calendar lookup"));
    let alpha = (end - start).whole_days() as f64 / 360.0;
    let mut swap = compact_swap();
    swap.id = "OIS-NONBUSINESS-MINIMUM-DATE".into();
    swap.fixed_leg.start = start;
    swap.fixed_leg.end = end;
    swap.fixed_leg.stub = StubKind::ShortBack;
    swap.fixed_leg.business_day_convention = BusinessDayConvention::Unadjusted;
    swap.fixed_leg.payment_lag_days = 0;
    swap.float_leg.start = start;
    swap.float_leg.end = end;
    swap.float_leg.stub = StubKind::ShortBack;
    swap.float_leg.business_day_convention = BusinessDayConvention::Unadjusted;
    swap.float_leg.payment_lag_days = 0;
    swap.float_leg.reset_lag_days = 0;
    swap.float_leg.fixing_calendar_id = Some("usny".into());
    swap.validate().unwrap();
    assert_eq!(swap.default_model(), ModelKey::Discounting);
    assert_eq!(swap.side, PayReceive::Pay);
    assert_eq!(swap.fixed_leg.rate, dec!(0.04));
    assert_eq!(swap.float_leg.spread_bp, dec!(0));
    assert_eq!(swap.fixed_leg.day_count, DayCount::Act360);
    assert_eq!(swap.float_leg.day_count, DayCount::Act360);
    assert_eq!(
        swap.float_leg.compounding,
        FloatingLegCompounding::CompoundedInArrears { lookback_days: 0 }
    );
    assert_eq!(swap.fixed_leg.discount_curve_id.as_str(), "USD-OIS");
    assert_eq!(swap.float_leg.discount_curve_id.as_str(), "USD-OIS");
    assert_eq!(swap.float_leg.forward_curve_id.as_str(), "USD-OIS");

    // ShortBack rolls forward from Date::MIN, so neither schedule generation
    // nor the fixed stub classification needs an unsupported earlier date.
    for periods in [
        build_periods(swap.fixed_leg.schedule_params(false)).unwrap(),
        build_periods(swap.float_leg.schedule_params("weekends_only", None, false)).unwrap(),
    ] {
        assert_eq!(periods.len(), 1);
        assert_eq!(periods[0].accrual_start, start);
        assert_eq!(periods[0].accrual_end, end);
        assert_eq!(periods[0].payment_date, end);
        close(
            "minimum-date coupon accrual",
            periods[0].accrual_year_fraction,
            alpha,
            1e-15,
        );
    }
    assert_eq!(
        adjust(start, BusinessDayConvention::Unadjusted, calendar).unwrap(),
        start
    );
    assert_eq!(
        adjust(end, BusinessDayConvention::Unadjusted, calendar).unwrap(),
        end
    );

    let market = MarketContext::new().insert(
        DiscountCurve::builder("USD-OIS")
            .base_date(start)
            .day_count(DayCount::Act365F)
            .knots([(0.0, 1.0), (1.0, 0.96)])
            .interp(InterpStyle::Linear)
            .build()
            .unwrap(),
    );
    assert!(market.get_forward("USD-OIS").is_err());
    let curve = market.get_discount("USD-OIS").unwrap();
    assert_eq!(curve.base_date(), start);
    let factor = curve.df_on_date(start).unwrap() / curve.df_on_date(end).unwrap();
    let discount = curve.df_on_date(end).unwrap() / curve.df_on_date(start).unwrap();
    let rate = (factor - 1.0) / alpha;
    let floating_amount = NOTIONAL * (factor - 1.0);
    let fixed_amount = NOTIONAL * 0.04 * alpha;
    let oracle = Oracle {
        notional: NOTIONAL,
        fixed_pv: fixed_amount * discount,
        float_pv: floating_amount * discount,
        annuity: alpha * discount,
        pv_bound: (NOTIONAL * FACTOR_BOUND * discount.abs()).max(1e-8),
        fixed_rows: vec![(end, alpha, fixed_amount, CFKind::Stub)],
        coupons: vec![CouponOracle {
            grid: CouponGrid {
                start,
                end,
                payment: end,
                alpha,
                observations: vec![Observation {
                    start,
                    end,
                    rate_alpha: alpha,
                    factor_alpha: alpha,
                }],
            },
            factor,
            rate,
            amount: floating_amount,
            discount,
            fixings: vec![ProjectedFixing {
                series_id: "FIXING:USD-OIS".into(),
                date: start,
                value: Some(rate),
            }],
        }],
    };
    // This reference is a single dated discount ratio. It never searches
    // backward for a fixing before the supported date range. check_price
    // also checks exact export stability across the intervening value call.
    let (schedule, record) = check_price(&swap, &market, start, &oracle);
    assert_eq!(schedule.get_meta().projected_fixings.len(), 1);
    assert_eq!(schedule.get_meta().projected_fixings[0].date, start);
    emit(
        "ois_p3/nonbusiness_minimum_date",
        json!({
            "fixture": {"instrument": swap, "market": market,
                "as_of": start.to_string(), "model": swap.default_model(),
                "source": "discount", "fixing_calendar": "usny",
                "payment_calendar": "weekends_only"},
            "reference": {"start_is_business_day": false,
                "first_business_day": end.to_string(), "observation_count": 1,
                "factor": factor, "alpha": alpha, "raw_rate_decimal": rate,
                "float_amount_usd": floating_amount, "fixed_amount_usd": fixed_amount,
                "discount_to_as_of": discount, "payer_pv_usd": oracle.pv()},
            "result": record,
        }),
    );
}
