use super::super::types::{DrawRepaySpec, RevolvingCredit};
use super::*;
use crate::revolving_credit::RevolvingCreditFees;
use finstack_quant_cashflows::builder::RateSpec;
use finstack_quant_core::cashflow::CFKind;
use finstack_quant_core::currency::Currency;
use finstack_quant_core::dates::{DayCount, Tenor};
use finstack_quant_core::market_data::context::MarketContext;
use finstack_quant_core::money::Money;
use time::Month;
use time::macros::date;

#[test]
fn term_projection_converts_curve_basis_and_preserves_fixings_and_path_shocks() {
    use crate::loan_terms::MarginStep;
    use crate::revolving_credit::{StochasticUtilizationSpec, UtilizationProcess};
    use finstack_quant_cashflows::builder::FloatingRateSpec;
    use finstack_quant_core::dates::BusinessDayConvention;
    use finstack_quant_core::market_data::term_structures::{DiscountCurve, ForwardCurve};
    use rust_decimal::Decimal;

    let start = date!(2025 - 01 - 02);
    let step = date!(2025 - 02 - 14);
    let end = date!(2025 - 04 - 02);
    let spec: FloatingRateSpec = serde_json::from_value(serde_json::json!({
        "index_id":"TERM",
        "forward_curve_id":"TERM", "spread_bp":"0",
        "reset_frequency":{"count":3,"unit":"months"}, "reset_lag_days":0
    }))
    .expect("term specification");
    let facility = RevolvingCredit::builder()
        .id("RC-TERM-BASIS".into())
        .commitment(Money::from((1_000_000_i64, Currency::USD)))
        .drawn(Money::from((1_000_000_i64, Currency::USD)))
        .issue_date(start)
        .maturity(end)
        .rate(RateSpec::Floating(spec))
        .margin_steps(vec![MarginStep {
            date: step,
            delta_bp: Decimal::from(100),
        }])
        .day_count(DayCount::Act360)
        .yield_basis(finstack_quant_valuations_core::metrics::returns::YieldBasis::Act360)
        .frequency(Tenor::quarterly())
        .quote_compounding(finstack_quant_core::math::Compounding::QUARTERLY)
        .business_day_convention(BusinessDayConvention::Unadjusted)
        .fees(RevolvingCreditFees::default())
        .draw_repay_spec(DrawRepaySpec::Deterministic(vec![]))
        .discount_curve_id("USD-OIS".into())
        .recovery_rate(0.0)
        .build()
        .expect("facility");
    let market = MarketContext::new()
        .try_insert(
            ForwardCurve::flat("TERM", start, 0.04, DayCount::Act365F).expect("forward curve"),
        )
        .expect("insert market curve")
        .try_insert(
            DiscountCurve::builder("USD-OIS")
                .base_date(start)
                .day_count(DayCount::Act365F)
                .knots([(0.0, 1.0), (1.0, (-0.04_f64).exp())])
                .interp(finstack_quant_core::math::interp::InterpStyle::MonotoneConvex)
                .extrapolation(finstack_quant_core::math::interp::ExtrapolationPolicy::Extend)
                .build()
                .expect("discount curve"),
        )
        .expect("insert market curve");
    let fixings = ScalarTimeSeries::new(
        "FIXING:TERM",
        vec![(start, 0.07)],
        None,
        finstack_quant_core::market_data::scalars::SeriesInterpolation::Step,
        finstack_quant_core::market_data::scalars::SeriesExtrapolation::Error,
    )
    .expect("observed index");
    let total_accrual = (end - start).whole_days() as f64 / 360.0;
    let stepped_accrual = (end - step).whole_days() as f64 / 360.0;
    for observed in [false, true] {
        let as_of = if observed {
            date!(2025 - 01 - 03)
        } else {
            start
        };
        let fixing_series = observed.then_some(&fixings);
        let contractual_index = if observed {
            0.07
        } else {
            (0.04 * (end - start).whole_days() as f64 / 365.0).exp_m1() / total_accrual
        };
        let expected = 1_000_000.0 * (contractual_index * total_accrual + 0.01 * stepped_accrual);
        let schedule = CashflowEngine::new(&facility, Some(&market), as_of, fixing_series)
            .expect("deterministic engine")
            .generate_deterministic()
            .expect("deterministic coupons")
            .schedule;
        let coupon = schedule
            .get_flows()
            .iter()
            .find(|flow| flow.get_kind() == CFKind::Floating)
            .expect("floating coupon");
        assert!((coupon.get_amount().amount() - expected).abs() < 1e-7);
        assert!(schedule.get_meta().projected_fixings.iter().all(|fixing| {
            (fixing.value.expect("recorded index") - contractual_index).abs() < 1e-14
        }));

        for stochastic_rates in [false, true] {
            let mut path_facility = facility.clone();
            path_facility.draw_repay_spec =
                DrawRepaySpec::Stochastic(Box::new(StochasticUtilizationSpec {
                    utilization_process: UtilizationProcess::MeanReverting {
                        theta: 1.0,
                        kappa: 0.1,
                        sigma: 0.0,
                        spread_sensitivity: 0.0,
                    },
                    mc_config: None,
                }));
            let engine = CashflowEngine::new(&path_facility, Some(&market), as_of, fixing_series)
                .expect("path engine");
            let dates = vec![start, step, end];
            for shock in [0.0, 0.007] {
                let path = ThreeFactorPathData {
                    utilization_path: vec![1.0; dates.len()],
                    short_rate_path: vec![0.04 + shock; dates.len()],
                    credit_spread_path: vec![0.0; dates.len()],
                    time_points: dates
                        .iter()
                        .map(|date| (*date - start).whole_days() as f64 / 365.0)
                        .collect(),
                    payment_dates: dates.clone(),
                    stochastic_rates,
                };
                let schedule = engine.generate_stochastic_path(path).expect("path coupon");
                let coupon = schedule
                    .schedule
                    .get_flows()
                    .iter()
                    .find(|flow| flow.get_kind() == CFKind::Floating)
                    .expect("floating path coupon");
                let expected = expected
                    + if observed {
                        0.0
                    } else {
                        1_000_000.0 * shock * total_accrual
                    };
                assert!(
                    (coupon.get_amount().amount() - expected).abs() < 1e-5,
                    "observed={observed}, stochastic_rates={stochastic_rates}, shock={shock}: {} vs {expected}",
                    coupon.get_amount().amount()
                );
            }
        }
    }
}

#[test]
fn act365l_slices_preserve_actual_coupon_denominators_in_both_engines() {
    use crate::loan_terms::MarginStep;
    use finstack_quant_cashflows::builder::FloatingRateSpec;
    use finstack_quant_core::dates::{BusinessDayConvention, StubKind};
    use finstack_quant_core::market_data::term_structures::ForwardCurve;
    use rust_decimal::Decimal;

    for (start, end, step, frequency, stub, denominator) in [
        (
            date!(2023 - 03 - 01),
            date!(2024 - 03 - 01),
            date!(2023 - 06 - 01),
            Tenor::annual(),
            StubKind::ShortFront,
            366.0,
        ),
        (
            date!(2023 - 12 - 01),
            date!(2024 - 03 - 01),
            date!(2023 - 12 - 15),
            Tenor::quarterly(),
            StubKind::ShortFront,
            366.0,
        ),
        (
            date!(2024 - 03 - 01),
            date!(2025 - 01 - 01),
            date!(2024 - 06 - 01),
            Tenor::annual(),
            StubKind::ShortFront,
            365.0,
        ),
        (
            date!(2023 - 12 - 01),
            date!(2025 - 01 - 01),
            date!(2024 - 01 - 15),
            Tenor::annual(),
            StubKind::LongFront,
            365.0,
        ),
    ] {
        for floating in [false, true] {
            let rate = if floating {
                RateSpec::Floating(
                    serde_json::from_value::<FloatingRateSpec>(serde_json::json!({
                        "index_id":"TERM",
                        "forward_curve_id":"TERM", "spread_bp":"0",
                        "reset_frequency":{"count":1,"unit":"months"}, "reset_lag_days":0
                    }))
                    .expect("term rate specification"),
                )
            } else {
                RateSpec::Fixed { rate: 0.05 }
            };
            let facility = RevolvingCredit::builder()
                .id("RC-ACT365L-SLICES".into())
                .commitment(Money::from((1_000_000_i64, Currency::USD)))
                .drawn(Money::from((1_000_000_i64, Currency::USD)))
                .issue_date(start)
                .maturity(end)
                .rate(rate)
                .margin_steps(vec![MarginStep {
                    date: step,
                    delta_bp: Decimal::from(100),
                }])
                .day_count(DayCount::Act365L)
                .yield_basis(finstack_quant_valuations_core::metrics::returns::YieldBasis::Act365F)
                .frequency(frequency)
                .quote_compounding(finstack_quant_core::math::Compounding::QUARTERLY)
                .stub(stub)
                .business_day_convention(BusinessDayConvention::Unadjusted)
                .fees(RevolvingCreditFees::default())
                .draw_repay_spec(DrawRepaySpec::Deterministic(vec![]))
                .discount_curve_id("USD-OIS".into())
                .recovery_rate(0.0)
                .build()
                .expect("facility");
            let mut reset_grid = crate::revolving_credit::utils::build_reset_dates(&facility)
                .expect("reset dates")
                .unwrap_or_else(|| vec![start]);
            reset_grid.push(end);
            let mut date_factors = Vec::with_capacity(reset_grid.len());
            let mut factor = 1.0_f64;
            date_factors.push((reset_grid[0], factor));
            for pair in reset_grid.windows(2) {
                let seg_days = (pair[1] - pair[0]).whole_days() as f64;
                factor /= 1.0 + 0.05 * seg_days / denominator;
                date_factors.push((pair[1], factor));
            }
            let term_fwd = ForwardCurve::from_dates(
                "TERM",
                start,
                &date_factors,
                DayCount::Act365F,
                finstack_quant_core::math::interp::InterpStyle::LogLinear,
                finstack_quant_core::math::interp::ExtrapolationPolicy::Extend,
            )
            .expect("flat term curve");
            let observations = crate::revolving_credit::utils::build_observation_dates(&facility)
                .expect("observation dates");
            let n = observations.len();
            let time_points = observations
                .iter()
                .map(|day| (*day - start).whole_days() as f64 / 365.0)
                .collect();
            let market = MarketContext::new()
                .try_insert(term_fwd)
                .expect("insert market curve");
            let engine =
                CashflowEngine::new(&facility, Some(&market), start, None).expect("engine");
            let deterministic = engine
                .generate_deterministic()
                .expect("deterministic schedule");
            let stochastic = engine
                .generate_stochastic_path(ThreeFactorPathData {
                    utilization_path: vec![1.0; n],
                    short_rate_path: vec![0.05; n],
                    credit_spread_path: vec![0.0; n],
                    time_points,
                    payment_dates: observations,
                    stochastic_rates: false,
                })
                .expect("constant path schedule");
            let first_days = (step - start).whole_days() as f64;
            let last_days = (end - step).whole_days() as f64;
            let expected = 1_000_000.0 * (0.05 * first_days + 0.06 * last_days) / denominator;
            for schedule in [&deterministic.schedule, &stochastic.schedule] {
                let coupons: Vec<_> = schedule
                    .get_flows()
                    .iter()
                    .filter(|flow| matches!(flow.get_kind(), CFKind::Fixed | CFKind::Floating))
                    .collect();
                assert_eq!(coupons.len(), 1);
                assert!(
                    (coupons[0].get_accrual_factor() - (first_days + last_days) / denominator)
                        .abs()
                        < 1e-14
                );
                assert!(
                    (coupons[0].get_amount().amount() - expected).abs() < 1e-7,
                    "floating={floating}, {start} -> {end}: expected {expected}, got {}",
                    coupons[0].get_amount().amount()
                );
            }
        }
    }
}

#[test]
fn terminal_principal_uses_adjusted_payment_date_without_extra_accrual() {
    let start = Date::from_calendar_date(2026, Month::January, 3).expect("date");
    let maturity = Date::from_calendar_date(2027, Month::January, 3).expect("date");
    let adjusted = Date::from_calendar_date(2027, Month::January, 4).expect("date");
    let facility = RevolvingCredit::builder()
        .id("RC-BDC-BOUNDARY".into())
        .commitment(Money::from((1_000_000_i64, Currency::USD)))
        .drawn(Money::from((1_000_000_i64, Currency::USD)))
        .issue_date(start)
        .maturity(maturity)
        .rate(RateSpec::Fixed { rate: 0.05 })
        .day_count(DayCount::Act365F)
        .yield_basis(finstack_quant_valuations_core::metrics::returns::YieldBasis::Act365F)
        .frequency(Tenor::annual())
        .quote_compounding(finstack_quant_core::math::Compounding::Periodic(
            std::num::NonZeroU32::MIN,
        ))
        .fees(RevolvingCreditFees::default())
        .draw_repay_spec(DrawRepaySpec::Deterministic(vec![]))
        .discount_curve_id("USD-OIS".into())
        .recovery_rate(0.0)
        .build()
        .expect("facility");

    let path = CashflowEngine::new(&facility, None, start, None)
        .expect("engine")
        .generate_deterministic()
        .expect("cashflows");
    let flows = path.schedule.get_flows();
    let terminal = flows
        .iter()
        .find(|flow| flow.get_kind() == CFKind::Notional && flow.get_amount().amount() > 0.0)
        .expect("terminal principal");
    let interest = flows
        .iter()
        .find(|flow| flow.get_kind() == CFKind::Fixed)
        .expect("interest");

    assert_eq!(terminal.get_date(), adjusted);
    assert_eq!(interest.get_date(), adjusted);
    assert!((interest.get_accrual_factor() - 1.0).abs() < 1e-12);
    assert!((interest.get_amount().amount() - 50_000.0).abs() < 1e-8);
}
