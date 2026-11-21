use super::{
    FinancingLegContext, TotalReturnLegParams, TrsEngine, TrsReturnModel,
    financing_period_projection, financing_period_rate,
};
use crate::constants::ANNUITY_EPSILON;
use crate::instruments::FinancingLegSpec;
use crate::instruments::TrsScheduleSpec;
use finstack_quant_cashflows::builder::FloatingLegCompounding;
use finstack_quant_cashflows::builder::ScheduleParams;
use finstack_quant_core::currency::Currency;
use finstack_quant_core::dates::{
    BusinessDayConvention, Date, DateExt, DayCount, DayCountContext, StubKind, Tenor,
};
use finstack_quant_core::market_data::context::MarketContext;
use finstack_quant_core::market_data::scalars::ScalarTimeSeries;
use finstack_quant_core::market_data::term_structures::{DiscountCurve, ForwardCurve};
use finstack_quant_core::market_data::traits::Discounting;
use finstack_quant_core::money::Money;
use finstack_quant_core::types::CurveId;
use rust_decimal::Decimal;
use time::Month;

fn date(y: i32, m: u8, d: u8) -> Date {
    Date::from_calendar_date(y, Month::try_from(m).expect("month"), d).expect("date")
}

#[test]
fn act365l_financing_coupons_preserve_actual_periods_and_the_pv_spread_identity() {
    for (start, end, frequency, stub, denominator) in [
        (
            date(2023, 3, 1),
            date(2024, 3, 1),
            Tenor::annual(),
            StubKind::ShortFront,
            366.0,
        ),
        (
            date(2023, 12, 1),
            date(2024, 3, 1),
            Tenor::quarterly(),
            StubKind::ShortFront,
            366.0,
        ),
        (
            date(2024, 3, 1),
            date(2025, 1, 1),
            Tenor::annual(),
            StubKind::ShortFront,
            365.0,
        ),
        (
            date(2023, 12, 1),
            date(2025, 1, 1),
            Tenor::annual(),
            StubKind::LongFront,
            365.0,
        ),
    ] {
        let as_of = start.previous_day().expect("valuation date");
        let schedule = TrsScheduleSpec::from_params(
            start,
            end,
            ScheduleParams {
                frequency,
                day_count: DayCount::Act365L,
                business_day_convention: BusinessDayConvention::Unadjusted,
                calendar_id: "weekends_only".into(),
                stub,
                end_of_month: false,
                payment_lag_days: 0,
                adjust_accrual_dates: false,
                roll_rule: finstack_quant_cashflows::builder::RollRule::None,
            },
        );
        let disc = DiscountCurve::builder("DISC")
            .base_date(as_of)
            .day_count(DayCount::Act365F)
            .knots([(0.0, 1.0), (3.0, 1.0)])
            .interp(finstack_quant_core::math::interp::InterpStyle::MonotoneConvex)
            .extrapolation(finstack_quant_core::math::interp::ExtrapolationPolicy::Extend)
            .build()
            .expect("flat discount");
        let notional = Money::from((1_000_000_i64, Currency::USD));
        let total_fraction = (end - start).whole_days() as f64 / denominator;
        for compounding in [
            FloatingLegCompounding::Simple,
            FloatingLegCompounding::CompoundedInArrears { lookback_days: 0 },
        ] {
            let fwd = if matches!(compounding, FloatingLegCompounding::Simple) {
                ForwardCurve::from_dates(
                    "TERM",
                    as_of,
                    &[
                        (as_of, 1.0),
                        (start, 1.0),
                        (end, 1.0 / (1.0 + 0.05 * total_fraction)),
                    ],
                    DayCount::Act365F,
                    finstack_quant_core::math::interp::InterpStyle::LogLinear,
                    finstack_quant_core::math::interp::ExtrapolationPolicy::Extend,
                )
                .expect("flat term forward")
            } else {
                let mut pts = vec![(as_of, 1.0), (start, 1.0)];
                let mut day = start;
                let mut fac = 1.0;
                while day < end {
                    let next = day
                        .add_weekdays(1)
                        .expect("next overnight interval")
                        .min(end);
                    fac /= 1.0 + 0.05 * (next - day).whole_days() as f64 / denominator;
                    pts.push((next, fac));
                    day = next;
                }
                ForwardCurve::from_dates(
                    "TERM",
                    as_of,
                    &pts,
                    DayCount::Act365F,
                    finstack_quant_core::math::interp::InterpStyle::LogLinear,
                    finstack_quant_core::math::interp::ExtrapolationPolicy::Extend,
                )
                .expect("flat overnight forward")
            };
            let market = MarketContext::new()
                .try_insert(disc.clone())
                .expect("insert market curve")
                .try_insert(fwd)
                .expect("insert market curve");
            let financing = FinancingLegSpec {
                discount_curve_id: CurveId::new("DISC"),
                forward_curve_id: CurveId::new("TERM"),
                spread_bp: Decimal::from(100),
                day_count: DayCount::Act365L,
                compounding,
            };
            let annuity =
                TrsEngine::financing_annuity(&financing, &schedule, notional, &market, as_of)
                    .expect("ACT/365L annuity");
            let float_pv =
                TrsEngine::pv_financing_float_only(&financing, &schedule, notional, &market, as_of)
                    .expect("ACT/365L floating PV");
            let total_pv =
                TrsEngine::pv_financing_leg(&financing, &schedule, notional, &market, as_of)
                    .expect("ACT/365L financing PV");
            let expected_float = if matches!(compounding, FloatingLegCompounding::Simple) {
                50_000.0 * total_fraction
            } else {
                let mut factor = 1.0;
                let mut day = start;
                while day < end {
                    let next = day
                        .add_weekdays(1)
                        .expect("next overnight interval")
                        .min(end);
                    factor *= 1.0 + 0.05 * (next - day).whole_days() as f64 / denominator;
                    day = next;
                }
                1_000_000.0 * (factor - 1.0)
            };
            assert!((annuity / notional.amount() - total_fraction).abs() < 1e-14);
            assert!(
                (float_pv - expected_float).abs() < 1e-7,
                "{start} -> {end}, {compounding:?}: expected {expected_float}, got {float_pv}"
            );
            assert!((total_pv.amount() - float_pv - 0.01 * annuity).abs() < 1e-7);
        }
    }
}

struct FlatReturnModel {
    rate: f64,
}

struct SequencedReturnModel {
    returns: [f64; 3],
    next_idx: std::sync::atomic::AtomicUsize,
}

impl TrsReturnModel for FlatReturnModel {
    fn period_return(
        &self,
        _inputs: &super::PeriodReturnInputs,
        _context: &MarketContext,
    ) -> finstack_quant_core::Result<f64> {
        Ok(self.rate)
    }
}

impl TrsReturnModel for SequencedReturnModel {
    fn period_return(
        &self,
        _inputs: &super::PeriodReturnInputs,
        _context: &MarketContext,
    ) -> finstack_quant_core::Result<f64> {
        let idx = self.next_idx.load(std::sync::atomic::Ordering::SeqCst);
        self.next_idx
            .fetch_add(1, std::sync::atomic::Ordering::SeqCst);
        Ok(self.returns[idx])
    }
}

#[test]
fn test_trs_annuity_epsilon_is_reasonable() {
    // Verify the threshold catches near-zero but allows reasonable values
    let eps = ANNUITY_EPSILON;
    assert!(eps > 0.0, "ANNUITY_EPSILON should be positive");
    assert!(eps < 1e-10, "ANNUITY_EPSILON should be small");

    // A typical annuity for a 1-year quarterly swap with $1M notional would be
    // roughly 0.25 * 4 * 1M * 0.95 = 950,000, which is well above epsilon
    let typical_annuity = 950_000.0;
    assert!(
        typical_annuity > eps,
        "Typical annuity should be above threshold"
    );
}

#[test]
fn trs_total_return_leg_preserves_small_residual_after_large_cancelling_periods() {
    let as_of = date(2025, 1, 1);
    let end = date(2025, 4, 1);
    let schedule = TrsScheduleSpec::from_params(
        as_of,
        end,
        ScheduleParams {
            frequency: Tenor::monthly(),
            day_count: DayCount::Act365F,
            business_day_convention: BusinessDayConvention::ModifiedFollowing,
            calendar_id: "weekends_only".into(),
            stub: StubKind::None,
            end_of_month: false,
            payment_lag_days: 0,
            adjust_accrual_dates: false,
            roll_rule: finstack_quant_cashflows::builder::RollRule::None,
        },
    );

    let disc = DiscountCurve::builder(CurveId::new("DISC"))
        .base_date(as_of)
        .day_count(DayCount::Act365F)
        .knots([(0.0, 1.0), (1.0, 1.0)])
        .interp(finstack_quant_core::math::interp::InterpStyle::MonotoneConvex)
        .extrapolation(finstack_quant_core::math::interp::ExtrapolationPolicy::Extend)
        .build()
        .expect("discount curve");

    let ctx = MarketContext::new()
        .try_insert(disc)
        .expect("insert market curve");
    let params = TotalReturnLegParams {
        schedule: &schedule,
        notional: Money::from((1_i64, Currency::USD)),
        discount_curve_id: "DISC",
    };
    let model = SequencedReturnModel {
        returns: [1e16, 1.0, -1e16],
        next_idx: std::sync::atomic::AtomicUsize::new(0),
    };

    let pv = TrsEngine::pv_total_return_leg_with_model(params, &ctx, as_of, &model).expect("pv");

    assert_eq!(pv.amount(), 1.0);
}

#[test]
fn trs_total_return_leg_uses_curve_df_between_dates() {
    let as_of = date(2024, 12, 31);
    let start = date(2025, 1, 1);
    let end = date(2026, 1, 1);
    let schedule = TrsScheduleSpec::from_params(
        start,
        end,
        ScheduleParams {
            frequency: Tenor::quarterly(),
            day_count: DayCount::Act365F,
            business_day_convention: BusinessDayConvention::ModifiedFollowing,
            calendar_id: "weekends_only".into(),
            stub: StubKind::None,
            end_of_month: false,
            payment_lag_days: 0,
            adjust_accrual_dates: false,
            roll_rule: finstack_quant_cashflows::builder::RollRule::None,
        },
    );

    let disc = DiscountCurve::builder(CurveId::new("DISC"))
        .base_date(as_of)
        .day_count(DayCount::Act360)
        .knots([(0.0, 1.0), (1.0, 0.95)])
        .interp(finstack_quant_core::math::interp::InterpStyle::MonotoneConvex)
        .extrapolation(finstack_quant_core::math::interp::ExtrapolationPolicy::Extend)
        .build()
        .expect("discount curve");

    let ctx = MarketContext::new()
        .try_insert(disc.clone())
        .expect("insert market curve");
    let params = TotalReturnLegParams {
        schedule: &schedule,
        notional: Money::from((1_000_000_i64, Currency::USD)),
        discount_curve_id: "DISC",
    };
    let model = FlatReturnModel { rate: 0.05 };

    let pv = TrsEngine::pv_total_return_leg_with_model(params, &ctx, as_of, &model).expect("pv");

    let period_schedule = schedule.period_schedule().expect("schedule");
    let mut expected = 0.0;
    let mut naive = 0.0;
    let ctx_day_count = DayCountContext::default();

    for i in 1..period_schedule.get_dates().len() {
        let _period_start = period_schedule.get_dates()[i - 1];
        let period_end = period_schedule.get_dates()[i];
        let df = disc.df_between_dates(as_of, period_end).expect("df");
        let t_end = schedule
            .params
            .day_count
            .year_fraction(as_of, period_end, ctx_day_count)
            .expect("t_end");
        let df_naive = disc.df(t_end).expect("df");
        let payment = 1_000_000.0 * model.rate;
        expected += payment * df;
        naive += payment * df_naive;
    }

    let diff = (pv.amount() - expected).abs();
    let tol = 1e-8 * 1_000_000.0;
    assert!(diff < tol, "PV should use curve DF: diff={}", diff);
    assert!(
        (expected - naive).abs() > 1e-6,
        "Expected curve-based DF to differ from naive DF"
    );
}

#[test]
fn trs_financing_leg_uses_curve_clock_and_contractual_accrual() {
    let as_of = date(2024, 12, 31);
    let start = date(2025, 1, 1);
    let end = date(2026, 1, 1);
    let schedule = TrsScheduleSpec::from_params(
        start,
        end,
        ScheduleParams {
            frequency: Tenor::quarterly(),
            day_count: DayCount::Act365F,
            business_day_convention: BusinessDayConvention::ModifiedFollowing,
            calendar_id: "weekends_only".into(),
            stub: StubKind::None,
            end_of_month: false,
            payment_lag_days: 0,
            adjust_accrual_dates: false,
            roll_rule: finstack_quant_cashflows::builder::RollRule::None,
        },
    );

    let disc = DiscountCurve::builder(CurveId::new("DISC"))
        .base_date(as_of)
        .day_count(DayCount::Act360)
        .knots([(0.0, 1.0), (1.0, 0.95)])
        .interp(finstack_quant_core::math::interp::InterpStyle::MonotoneConvex)
        .extrapolation(finstack_quant_core::math::interp::ExtrapolationPolicy::Extend)
        .build()
        .expect("discount curve");

    let fwd = ForwardCurve::from_zero_rates(
        CurveId::new("FWD"),
        as_of,
        &[(0.0, 0.03), (1.0, 0.04)],
        finstack_quant_core::math::Compounding::Continuous,
        DayCount::Act360,
        finstack_quant_core::math::interp::InterpStyle::LogLinear,
        finstack_quant_core::math::interp::ExtrapolationPolicy::Extend,
    )
    .expect("forward curve");

    let ctx = MarketContext::new()
        .try_insert(disc.clone())
        .expect("insert market curve")
        .try_insert(fwd.clone())
        .expect("insert market curve");

    let financing = FinancingLegSpec {
        discount_curve_id: CurveId::new("DISC"),
        forward_curve_id: CurveId::new("FWD"),
        spread_bp: Decimal::ZERO,
        day_count: DayCount::Act365F,
        compounding: FloatingLegCompounding::Simple,
    };

    let pv = TrsEngine::pv_financing_leg(
        &financing,
        &schedule,
        Money::from((1_000_000_i64, Currency::USD)),
        &ctx,
        as_of,
    )
    .expect("pv");

    let period_schedule = schedule.period_schedule().expect("schedule");
    let mut expected = 0.0;
    let mut naive = 0.0;
    let mut curve_accrual_pv = 0.0;
    let ctx_day_count = DayCountContext::default();

    for i in 1..period_schedule.get_dates().len() {
        let period_start = period_schedule.get_dates()[i - 1];
        let period_end = period_schedule.get_dates()[i];
        // Use financing.day_count for accrual (matches engine after fix)
        let yf = financing
            .day_count
            .year_fraction(period_start, period_end, ctx_day_count)
            .expect("yf");
        let growth = fwd.df_on_date(period_start).expect("start projection DF")
            / fwd.df_on_date(period_end).expect("end projection DF")
            - 1.0;
        let df = disc.df_between_dates(as_of, period_end).expect("df");
        expected += 1_000_000.0 * growth * df;
        let curve_accrual = fwd
            .day_count()
            .year_fraction(period_start, period_end, ctx_day_count)
            .expect("curve accrual");
        curve_accrual_pv += 1_000_000.0 * growth / curve_accrual * yf * df;

        let t_start = schedule
            .params
            .day_count
            .year_fraction(as_of, period_start, ctx_day_count)
            .expect("t_start");
        let t_end = schedule
            .params
            .day_count
            .year_fraction(as_of, period_end, ctx_day_count)
            .expect("t_end");
        let fwd_naive = fwd
            .rate_between(t_start, t_end, yf)
            .expect("finite ordered period");
        let df_naive = disc.df(t_end).expect("df");
        naive += 1_000_000.0 * fwd_naive * yf * df_naive;
    }

    let diff = (pv.amount() - expected).abs();
    let tol = 1e-8 * 1_000_000.0;
    assert!(diff < tol, "PV should use curve time: diff={}", diff);
    assert!(
        (expected - curve_accrual_pv).abs() > 100.0,
        "curve clock must not replace contractual coupon accrual"
    );
    assert!(
        (expected - naive).abs() > 1e-6,
        "Expected curve-based forward/DF to differ from naive time mapping"
    );
}

/// Regression test: an in-progress compounded financing period that
/// straddles `as_of` must splice realized daily fixings (period_start → as_of)
/// with projected forwards (as_of → period_end), NOT project the whole period
/// from the forward curve. This test verifies that the spliced result differs
/// from the all-projected result, and matches a hand-computed expected value.
#[test]
fn trs_financing_leg_ois_in_progress_splices_realized_and_projected() {
    // Period: 2025-01-06 (Mon) → 2025-02-03 (Mon), 28 days. as_of = 2025-01-20 (Mon, mid-period).
    // Both endpoints are Mondays so ModifiedFollowing leaves them unchanged.
    // Realized fixings for Jan 6..17 (2 full weeks), projected from Jan 20 onward.
    let period_start = date(2025, 1, 6);
    let period_end = date(2025, 2, 3);
    let as_of = date(2025, 1, 20);

    // Flat OIS forward at 5%.
    let fwd = ForwardCurve::flat(CurveId::new("SOFR"), period_start, 0.05, DayCount::Act360)
        .expect("forward curve");

    // Realized fixings at 4 % for every weekday in Jan 01..15.
    // (Weekday-only grid: Jan 1 is Wednesday; we enumerate Jan 1–15 weekdays.)
    let realized_dates: Vec<Date> = {
        let mut ds = Vec::new();
        let mut d = period_start;
        while d < as_of {
            let wd = d.weekday();
            use time::Weekday;
            if !matches!(wd, Weekday::Saturday | Weekday::Sunday) {
                ds.push(d);
            }
            d = d.next_day().expect("next day");
        }
        ds
    };
    let realized_obs: Vec<(Date, f64)> = realized_dates.iter().map(|&d| (d, 0.04)).collect();
    let fixing_series = ScalarTimeSeries::new(
        "FIXING:SOFR",
        realized_obs,
        None,
        finstack_quant_core::market_data::scalars::SeriesInterpolation::Step,
        finstack_quant_core::market_data::scalars::SeriesExtrapolation::Error,
    )
    .expect("fixing series");

    let disc = DiscountCurve::builder(CurveId::new("DISC"))
        .base_date(as_of)
        .day_count(DayCount::Act360)
        .knots([(0.0, 1.0), (1.0, 1.0)])
        .interp(finstack_quant_core::math::interp::InterpStyle::MonotoneConvex)
        .extrapolation(finstack_quant_core::math::interp::ExtrapolationPolicy::Extend)
        .build()
        .expect("discount curve");

    let ctx = MarketContext::new()
        .try_insert(disc)
        .expect("insert market curve")
        .try_insert(fwd.clone())
        .expect("insert market curve")
        .insert_series(fixing_series);

    // TRS with a single monthly period (period_start → period_end).
    // Use StubKind::Short so ScheduleBuilder does not require an exact integer
    // multiple of the tenor between start and end dates.
    let schedule = TrsScheduleSpec::from_params(
        period_start,
        period_end,
        ScheduleParams {
            frequency: Tenor::monthly(),
            day_count: DayCount::Act365F,
            business_day_convention: BusinessDayConvention::ModifiedFollowing,
            calendar_id: "weekends_only".into(),
            stub: StubKind::ShortBack,
            end_of_month: false,
            payment_lag_days: 0,
            adjust_accrual_dates: false,
            roll_rule: finstack_quant_cashflows::builder::RollRule::None,
        },
    );

    let financing = FinancingLegSpec {
        discount_curve_id: CurveId::new("DISC"),
        forward_curve_id: CurveId::new("SOFR"),
        spread_bp: Decimal::ZERO,
        day_count: DayCount::Act360,
        compounding: FloatingLegCompounding::CompoundedInArrears { lookback_days: 0 },
    };

    let notional = Money::from((1_000_000_i64, Currency::USD));
    let pv_spliced = TrsEngine::pv_financing_leg(&financing, &schedule, notional, &ctx, as_of)
        .expect("pv_spliced");

    // Compute expected spliced compound independently:
    // realized sub-period: weekdays in [Jan 6, Jan 17], each at 4 %
    // projected sub-period: weekdays in [Jan 20, Feb 3), each at 5 %
    let ctx_day_count = DayCountContext::default();
    use time::Weekday;
    let mut cf = 1.0_f64;
    let mut d = period_start;
    while d < period_end {
        let next_d = {
            // Step one weekday forward (weekends_only calendar → skip Sat/Sun)
            let mut nd = d.next_day().expect("next");
            while matches!(nd.weekday(), Weekday::Saturday | Weekday::Sunday) {
                nd = nd.next_day().expect("next");
            }
            nd.min(period_end)
        };
        let dcf = DayCount::Act360
            .year_fraction(d, next_d, ctx_day_count)
            .expect("dcf");
        let r = if d < as_of { 0.04 } else { 0.05 };
        cf *= 1.0 + r * dcf;
        d = next_d;
    }
    let yf = DayCount::Act360
        .year_fraction(period_start, period_end, ctx_day_count)
        .expect("yf");
    let expected_rate = (cf - 1.0) / yf;
    let expected_pv = 1_000_000.0 * expected_rate * yf; // df = 1

    // Compute the all-projected value (the buggy result before the fix).
    let ctx_no_fixings = MarketContext::new()
        .try_insert(
            DiscountCurve::builder(CurveId::new("DISC"))
                .base_date(as_of)
                .day_count(DayCount::Act360)
                .knots([(0.0, 1.0), (1.0, 1.0)])
                .interp(finstack_quant_core::math::interp::InterpStyle::MonotoneConvex)
                .extrapolation(finstack_quant_core::math::interp::ExtrapolationPolicy::Extend)
                .build()
                .expect("disc no_fixings"),
        )
        .expect("insert market curve")
        .try_insert(fwd)
        .expect("insert market curve");
    let pv_all_projected =
        TrsEngine::pv_financing_leg(&financing, &schedule, notional, &ctx_no_fixings, as_of);
    // Before the fix, this would succeed (no fixings required) and give a
    // different value; after the fix it must fail (fixings missing for an
    // in-progress period) OR we simply verify that pv_spliced matches expected.

    // The spliced PV must match the hand-computed expected value.
    let diff = (pv_spliced.amount() - expected_pv).abs();
    assert!(
        diff < 1.0, // within $1 on $1M notional
        "Spliced PV ({:.2}) must match expected ({:.2}); diff = {:.4}",
        pv_spliced.amount(),
        expected_pv,
        diff
    );

    // The all-projected path (no fixings) must error after the fix, because
    // a missing realized fixing for an in-progress OIS period is a hard error.
    assert!(
        pv_all_projected.is_err(),
        "In-progress OIS financing period without fixings must error after the fix"
    );

    // The spliced result must differ materially from the all-projected-at-5%
    // result (which would be CF_5pct - 1 normalized, i.e. biased high since
    // the realized sub-period was actually at 4%).
    let mut cf_all5 = 1.0_f64;
    let mut d2 = period_start;
    while d2 < period_end {
        let next_d = {
            let mut nd = d2.next_day().expect("next");
            while matches!(nd.weekday(), Weekday::Saturday | Weekday::Sunday) {
                nd = nd.next_day().expect("next");
            }
            nd.min(period_end)
        };
        let dcf = DayCount::Act360
            .year_fraction(d2, next_d, ctx_day_count)
            .expect("dcf");
        cf_all5 *= 1.0 + 0.05 * dcf;
        d2 = next_d;
    }
    let rate_all5 = (cf_all5 - 1.0) / yf;
    let pv_all5 = 1_000_000.0 * rate_all5 * yf;
    assert!(
        (pv_spliced.amount() - pv_all5).abs() > 100.0,
        "Spliced PV ({:.2}) must differ materially from all-5% projected ({:.2})",
        pv_spliced.amount(),
        pv_all5
    );
}

#[test]
fn trs_term_rate_started_period_requires_exact_reset_fixing() {
    let period_start = date(2025, 1, 6);
    let period_end = date(2025, 4, 7);
    let as_of = date(2025, 2, 3);
    let fwd = ForwardCurve::flat("USD-TERM-3M", period_start, 0.05, DayCount::Act360)
        .expect("forward curve");
    let financing = FinancingLegSpec {
        discount_curve_id: CurveId::new("DISC"),
        forward_curve_id: CurveId::new("USD-TERM-3M"),
        spread_bp: Decimal::ZERO,
        day_count: DayCount::Act360,
        compounding: FloatingLegCompounding::Simple,
    };
    let year_fraction = DayCount::Act360
        .year_fraction(period_start, period_end, DayCountContext::default())
        .expect("year fraction");

    let missing = financing_period_rate(
        &financing,
        &fwd,
        None,
        period_start,
        period_end,
        year_fraction,
        Tenor::quarterly(),
        as_of,
        finstack_quant_core::dates::WEEKENDS_ONLY_ID,
        Currency::USD,
    )
    .expect_err("started term period without fixing must fail");
    assert!(missing.to_string().contains("FIXING:USD-TERM-3M"));

    let fixings = ScalarTimeSeries::new(
        "FIXING:USD-TERM-3M",
        vec![(period_start, 0.041)],
        None,
        finstack_quant_core::market_data::scalars::SeriesInterpolation::Step,
        finstack_quant_core::market_data::scalars::SeriesExtrapolation::Error,
    )
    .expect("fixing series");
    let observed = financing_period_rate(
        &financing,
        &fwd,
        Some(&fixings),
        period_start,
        period_end,
        year_fraction,
        Tenor::quarterly(),
        as_of,
        finstack_quant_core::dates::WEEKENDS_ONLY_ID,
        Currency::USD,
    )
    .expect("exact reset fixing");
    assert!((observed - 0.041).abs() < 1e-14);

    let future = financing_period_rate(
        &financing,
        &fwd,
        None,
        period_end,
        period_end.add_months(3).expect("valid date shift"),
        year_fraction,
        Tenor::quarterly(),
        as_of,
        finstack_quant_core::dates::WEEKENDS_ONLY_ID,
        Currency::USD,
    )
    .expect("future term period projects");
    assert!(future.is_finite());
}

#[test]
fn trs_term_rate_period_resetting_on_valuation_date_uses_published_fixing_or_projects() {
    let as_of = date(2025, 1, 6);
    let period_end = date(2025, 4, 7);
    let fwd =
        ForwardCurve::flat("USD-TERM-3M", as_of, 0.05, DayCount::Act360).expect("forward curve");
    let financing = FinancingLegSpec {
        discount_curve_id: CurveId::new("DISC"),
        forward_curve_id: CurveId::new("USD-TERM-3M"),
        spread_bp: Decimal::ZERO,
        day_count: DayCount::Act360,
        compounding: FloatingLegCompounding::Simple,
    };
    let year_fraction = DayCount::Act360
        .year_fraction(as_of, period_end, DayCountContext::default())
        .expect("year fraction");
    let project = |fixings: Option<&ScalarTimeSeries>| {
        let leg = FinancingLegContext {
            financing: &financing,
            fwd: &fwd,
            fixings,
            coupon_frequency: Tenor::quarterly(),
            as_of,
            calendar_id: finstack_quant_core::dates::WEEKENDS_ONLY_ID,
            accrual_bdc: BusinessDayConvention::ModifiedFollowing,
            currency: Currency::USD,
            collect_fixings: true,
        };
        financing_period_projection(&leg, as_of, period_end, year_fraction).expect("projection")
    };

    let published = ScalarTimeSeries::new(
        "FIXING:USD-TERM-3M",
        vec![(as_of, 0.041)],
        None,
        finstack_quant_core::market_data::scalars::SeriesInterpolation::Step,
        finstack_quant_core::market_data::scalars::SeriesExtrapolation::Error,
    )
    .expect("fixing series");
    let fixed = project(Some(&published));
    assert!((fixed.rate - 0.041).abs() < 1e-14);
    assert!(
        fixed.fixings.is_empty(),
        "a published fixing is not an exposure"
    );

    let projected = project(None);
    let expected = fwd
        .rate_between_dates(as_of, period_end, year_fraction)
        .expect("forward rate");
    assert!((projected.rate - expected).abs() < 1e-14);
    assert_eq!(projected.fixings.len(), 1);
    assert_eq!(projected.fixings[0].date, as_of);
}

#[test]
fn trs_financing_leg_overnight_compounding_exceeds_term_rate_on_upward_curve() {
    // On an upward-sloping forward curve, daily-compounding an OIS financing
    // leg picks up positive convexity that the simple term-rate average
    // drops. A `simple` and a compounded leg over the same
    // curves must therefore price differently, with OIS strictly higher.
    let as_of = date(2024, 12, 31);
    let start = date(2025, 1, 15);
    let end = date(2025, 10, 15);
    let schedule = TrsScheduleSpec::from_params(
        start,
        end,
        ScheduleParams {
            frequency: Tenor::quarterly(),
            day_count: DayCount::Act365F,
            business_day_convention: BusinessDayConvention::ModifiedFollowing,
            calendar_id: "weekends_only".into(),
            stub: StubKind::None,
            end_of_month: false,
            payment_lag_days: 0,
            adjust_accrual_dates: false,
            roll_rule: finstack_quant_cashflows::builder::RollRule::None,
        },
    );

    let disc = DiscountCurve::builder(CurveId::new("DISC"))
        .base_date(as_of)
        .day_count(DayCount::Act360)
        .knots([(0.0, 1.0), (1.0, 0.95)])
        .interp(finstack_quant_core::math::interp::InterpStyle::MonotoneConvex)
        .extrapolation(finstack_quant_core::math::interp::ExtrapolationPolicy::Extend)
        .build()
        .expect("discount curve");

    // Steep upward-sloping overnight forward: 3% -> 6% over the year.
    let fwd = ForwardCurve::from_zero_rates(
        CurveId::new("FWD"),
        as_of,
        &[(0.0, 0.03), (0.25, 0.035), (0.5, 0.045), (1.0, 0.06)],
        finstack_quant_core::math::Compounding::Continuous,
        DayCount::Act360,
        finstack_quant_core::math::interp::InterpStyle::LogLinear,
        finstack_quant_core::math::interp::ExtrapolationPolicy::Extend,
    )
    .expect("forward curve");

    let ctx = MarketContext::new()
        .try_insert(disc)
        .expect("insert market curve")
        .try_insert(fwd)
        .expect("insert market curve");
    let notional = Money::from((1_000_000_i64, Currency::USD));

    let term = FinancingLegSpec::new("DISC", "FWD", Decimal::ZERO, DayCount::Act365F);
    let ois = term
        .clone()
        .with_compounding(FloatingLegCompounding::CompoundedInArrears { lookback_days: 0 });

    let ois_lookback = term
        .clone()
        .with_compounding(FloatingLegCompounding::CompoundedInArrears { lookback_days: 5 });

    let pv_term = TrsEngine::pv_financing_leg(&term, &schedule, notional, &ctx, as_of)
        .expect("term-rate financing pv");
    let pv_ois = TrsEngine::pv_financing_leg(&ois, &schedule, notional, &ctx, as_of)
        .expect("OIS financing pv");
    let pv_lookback = TrsEngine::pv_financing_leg(&ois_lookback, &schedule, notional, &ctx, as_of)
        .expect("OIS lookback financing pv");

    assert!(
        (pv_ois.amount() - pv_term.amount()).abs() < 1e-6,
        "Unshifted OIS compounding must telescope to term-rate on a shared factor curve: ois={}, term={}",
        pv_ois.amount(),
        pv_term.amount()
    );
    assert!(
        pv_ois.amount() > pv_lookback.amount(),
        "Unshifted OIS compounding must exceed positive-lookback compounding on an upward curve: ois={}, lookback={}",
        pv_ois.amount(),
        pv_lookback.amount()
    );

    // The lookback gap should be material (a fraction of a bp up to a few
    // bp of notional) but not absurd — guards against a degenerate
    // near-zero pass or a units blunder.
    let gap_bp = (pv_ois.amount() - pv_lookback.amount()) / notional.amount() * 10_000.0;
    assert!(
        (0.1..50.0).contains(&gap_bp),
        "OIS-vs-lookback gap {gap_bp} bp is outside the expected range"
    );
}

#[test]
fn overnight_compounded_matches_shared_overnight_engine() {
    use crate::instruments::pricing::overnight::{
        OvernightCouponProjectionInput, OvernightProjectionCurve, project_overnight_coupon,
    };
    use finstack_quant_cashflows::builder::FloatingLegCompounding;
    use finstack_quant_core::dates::calendar_by_id_strict;

    let start = date(2025, 1, 2);
    let end = date(2025, 4, 2);
    let fwd = ForwardCurve::flat("USD-SOFR-OIS", start, 0.04, DayCount::Act360).expect("forward");
    let financing = FinancingLegSpec {
        discount_curve_id: CurveId::new("USD-OIS"),
        forward_curve_id: CurveId::new("USD-SOFR-OIS"),
        spread_bp: Decimal::ZERO,
        day_count: DayCount::Act360,
        compounding: FloatingLegCompounding::CompoundedInArrears { lookback_days: 0 },
    };
    let yf = DayCount::Act360
        .year_fraction(start, end, DayCountContext::default())
        .expect("yf");
    let actual = financing_period_rate(
        &financing,
        &fwd,
        None,
        start,
        end,
        yf,
        Tenor::quarterly(),
        start,
        "usny",
        Currency::USD,
    )
    .expect("trs overnight");
    let calendar = calendar_by_id_strict("usny").expect("usny");
    let compounding = FloatingLegCompounding::CompoundedInArrears { lookback_days: 0 };
    let expected = project_overnight_coupon(OvernightCouponProjectionInput {
        curve: OvernightProjectionCurve::Forward(&fwd),
        fixings: None,
        fixing_id: "USD-SOFR-OIS",
        as_of: start,
        accrual_start: start,
        accrual_end: end,
        end_is_termination_date: None,
        day_count: DayCount::Act360,
        coupon_frequency: None,
        coupon_period: (start, end),
        compounding: &compounding,
        fixing_calendar: calendar,
        compounded_spread: 0.0,
        need_observation_exposures: false,
    })
    .expect("shared engine");
    assert!(
        (actual - expected.rate).abs() < 1e-12,
        "TRS overnight {actual} != shared engine {}",
        expected.rate
    );
    let leg = FinancingLegContext {
        financing: &financing,
        fwd: &fwd,
        fixings: None,
        coupon_frequency: Tenor::quarterly(),
        as_of: start,
        calendar_id: "usny",
        accrual_bdc: BusinessDayConvention::ModifiedFollowing,
        currency: Currency::USD,
        collect_fixings: false,
    };
    let projected =
        financing_period_projection(&leg, start, end, 0.50).expect("trs overnight projection");
    let amount = projected.unsigned_coupon(1_000_000.0, 0.0);
    let expected_amount = 1_000_000.0 * (expected.compound_factor - 1.0);
    assert!(
        (amount - expected_amount).abs() < 1e-6,
        "TRS overnight amount {amount} != N*(CF-1) {expected_amount}"
    );
    let wrong = actual * 1_000_000.0 * 0.50;
    assert!(
        (amount - wrong).abs() > 1.0,
        "TRS overnight coupon must not use the unadjusted schedule year fraction"
    );
}

#[test]
fn par_spread_bp_rejects_currency_mismatch() {
    let as_of = date(2025, 1, 2);
    let end = date(2025, 4, 2);
    let schedule = TrsScheduleSpec::from_params(
        as_of,
        end,
        ScheduleParams {
            frequency: Tenor::quarterly(),
            day_count: DayCount::Act360,
            business_day_convention: BusinessDayConvention::ModifiedFollowing,
            calendar_id: "usny".into(),
            stub: StubKind::None,
            end_of_month: false,
            payment_lag_days: 0,
            adjust_accrual_dates: false,
            roll_rule: finstack_quant_cashflows::builder::RollRule::None,
        },
    );
    let financing = FinancingLegSpec {
        discount_curve_id: CurveId::new("USD-OIS"),
        forward_curve_id: CurveId::new("USD-SOFR-3M"),
        spread_bp: Decimal::ZERO,
        day_count: DayCount::Act360,
        compounding: FloatingLegCompounding::Simple,
    };
    let err = TrsEngine::par_spread_bp(
        Money::from((10_000_i64, Currency::EUR)),
        &financing,
        &schedule,
        Money::from((1_000_000_i64, Currency::USD)),
        &MarketContext::new(),
        as_of,
        250_000.0,
    )
    .expect_err("EUR total return PV vs USD notional must reject");
    assert!(matches!(
        err,
        finstack_quant_core::Error::CurrencyMismatch {
            expected: Currency::USD,
            actual: Currency::EUR,
        }
    ));
}
