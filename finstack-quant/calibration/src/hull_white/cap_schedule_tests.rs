use super::*;
use crate::hull_white::pricing::{
    normal_caplet_price, scheduled_bachelier_cap_floor_price,
    scheduled_cap_floor_implied_normal_vol, scheduled_cap_floor_price,
};
use crate::hull_white::{
    bootstrap_hull_white_sigma_schedule_to_cap_floors_with_fn,
    calibrate_hull_white_to_cap_floors_with_fn, CapFloorCalibrationConfig, HullWhiteParams,
    PiecewiseSigmaCalibrationConfig,
};
use finstack_quant_core::math::piecewise::PiecewiseConstantCurve;

fn quarterly_act360_schedule(maturity: f64) -> CapFloorSchedule {
    let mut schedule = CapFloorSchedule::synthetic(maturity, SwapFrequency::Quarterly);
    for period in &mut schedule.periods {
        period.accrual *= 365.0 / 360.0;
    }
    schedule
}

fn config() -> CapFloorCalibrationConfig {
    CapFloorCalibrationConfig {
        fit_tolerance: 1e-9,
        frequency: SwapFrequency::Quarterly,
        fixed_kappa: Some(0.05),
        initial_guess: None,
    }
}

#[test]
fn contractual_act360_cap_reprices_in_production_term_kernel() {
    let df = |time: f64| (-0.03 * time).exp();
    let quote = CapFloorQuote::try_new(5.0, 0.03, 0.01, true, true).expect("valid quote");
    let schedule = quarterly_act360_schedule(5.0);
    let (params, report) = calibrate_hull_white_to_cap_floors_with_fn(
        &df,
        &df,
        &[quote],
        Some(std::slice::from_ref(&schedule)),
        config(),
    )
    .expect("contractual calibration");
    assert!(report.success);
    // Independent ACT/360 counterexample: the old synthetic-year-fraction
    // calibration returned 0.0107077225833 and missed this premium by 1.4%.
    assert!((params.sigma - 0.0108559308318).abs() < 1e-10);
    let model = HullWhiteParams::constant(params.kappa, params.sigma).expect("valid model");
    let production_price: f64 = schedule
        .periods
        .iter()
        .map(|period| {
            finstack_quant_models::rates::hull_white::hw1f_term_caplet_price_from_dfs_with_model(
                &model,
                df(period.start_time),
                df(period.end_time),
                df(period.payment_time),
                period.fixing_time,
                period.start_time,
                period.end_time,
                period.payment_time,
                period.accrual,
                quote.strike,
                true,
            )
            .expect("production caplet price")
        })
        .sum();
    assert!((production_price - 0.0256427899494).abs() < 1e-11);
}

#[test]
fn contractual_fixings_payments_and_stub_accruals_survive_calibration() {
    let discount_df = |time: f64| (-0.025 * time).exp();
    let forward_df = |time: f64| (-0.04 * time).exp();
    let endpoints = [0.2, 0.45, 0.7, 0.95, 1.1];
    let schedule = CapFloorSchedule {
        periods: endpoints
            .windows(2)
            .map(|period| CapletSchedule {
                fixing_time: period[0] - 2.0 / 365.0,
                start_time: period[0],
                end_time: period[1],
                payment_time: period[1] + 3.0 / 365.0,
                accrual: (period[1] - period[0]) * 365.0 / 360.0,
            })
            .collect(),
    };
    let normal_vol = 0.009;
    let strike = 0.035;
    let expected_market: f64 = schedule
        .periods
        .iter()
        .map(|period| {
            normal_caplet_price(
                (forward_df(period.start_time) / forward_df(period.end_time) - 1.0)
                    / period.accrual,
                strike,
                normal_vol,
                period.fixing_time,
                period.accrual,
                discount_df(period.payment_time),
                false,
            )
        })
        .sum();
    let quote = CapFloorQuote::try_new(1.1, strike, normal_vol, false, true).expect("valid quote");
    let (params, report) = calibrate_hull_white_to_cap_floors_with_fn(
        &discount_df,
        &forward_df,
        &[quote],
        Some(std::slice::from_ref(&schedule)),
        config(),
    )
    .expect("stub floor calibration");
    assert!(report.success);
    let model = HullWhiteParams::constant(params.kappa, params.sigma).expect("valid model");
    let price =
        scheduled_cap_floor_price(&model, &discount_df, &forward_df, &schedule, strike, false)
            .expect("model price");
    assert!((price - expected_market).abs() < 1e-11);

    // These contractual times are financially observable, not cosmetic metadata.
    for change_fixing in [true, false] {
        let mut altered = schedule.clone();
        for period in &mut altered.periods {
            if change_fixing {
                period.fixing_time = period.start_time;
            } else {
                period.payment_time = period.end_time;
            }
        }
        let altered_market = scheduled_bachelier_cap_floor_price(
            &discount_df,
            &forward_df,
            &altered,
            strike,
            normal_vol,
            false,
        );
        assert!((altered_market - expected_market).abs() > 1e-8);
    }
}

#[test]
fn piecewise_bootstrap_sorts_quotes_with_contractual_schedules() {
    let df = |time: f64| (-0.03 * time).exp();
    let mut short = quarterly_act360_schedule(1.0);
    let mut long = quarterly_act360_schedule(2.0);
    for period in short.periods.iter_mut().chain(&mut long.periods) {
        period.fixing_time -= 2.0 / 365.0;
        period.payment_time += 2.0 / 365.0;
    }
    let horizon = short.periods.last().expect("live periods").fixing_time;
    let source = HullWhiteParams::new(
        0.05,
        PiecewiseConstantCurve::new(vec![0.0, horizon], vec![0.008, 0.015])
            .expect("valid sigma schedule"),
    )
    .expect("valid model");
    // Deliberately reverse the input maturities; schedules must move with quotes.
    let schedules = [long, short];
    let quotes: Vec<_> = schedules
        .iter()
        .map(|schedule| {
            let price = scheduled_cap_floor_price(&source, &df, &df, schedule, 0.03, true)
                .expect("source price");
            let vol = scheduled_cap_floor_implied_normal_vol(price, &df, &df, schedule, 0.03, true)
                .expect("source normal quote");
            CapFloorQuote::try_new(
                schedule.periods.last().expect("live periods").end_time,
                0.03,
                vol,
                true,
                true,
            )
            .expect("valid quote")
        })
        .collect();
    let (fitted, report) = bootstrap_hull_white_sigma_schedule_to_cap_floors_with_fn(
        &df,
        &df,
        &quotes,
        Some(&schedules),
        PiecewiseSigmaCalibrationConfig {
            fit_tolerance: 1e-9,
            fixed_kappa: 0.05,
            sigma_min: 1e-5,
            sigma_max: 0.1,
            frequency: SwapFrequency::Quarterly,
        },
    )
    .expect("piecewise fit");
    assert!(report.success);
    for schedule in &schedules {
        let fitted_price = scheduled_cap_floor_price(&fitted, &df, &df, schedule, 0.03, true)
            .expect("fitted price");
        let source_price = scheduled_cap_floor_price(&source, &df, &df, schedule, 0.03, true)
            .expect("source price");
        assert!((fitted_price - source_price).abs() < 1e-10);
    }
}

#[test]
fn malformed_contractual_cap_schedules_fail_before_fitting() {
    let quote = CapFloorQuote::try_new(1.0, 0.03, 0.01, true, true).expect("valid quote");
    let valid = quarterly_act360_schedule(1.0);
    assert!(prepare_cap_floor_schedules(&[quote], Some(&[]), SwapFrequency::Quarterly).is_err());
    for case in 0..7 {
        let mut schedule = valid.clone();
        match case {
            0 => schedule.periods[0].fixing_time = 0.0,
            1 => schedule.periods[0].accrual = f64::NAN,
            2 => schedule.periods[0].payment_time = schedule.periods[0].start_time - 0.01,
            3 => schedule.periods[0].fixing_time = schedule.periods[0].start_time + 0.01,
            4 => schedule.periods[1].start_time = schedule.periods[0].end_time - 0.01,
            5 => schedule.periods[2].end_time = 1.1,
            _ => schedule.periods.clear(),
        }
        assert!(
            prepare_cap_floor_schedules(&[quote], Some(&[schedule]), SwapFrequency::Quarterly)
                .is_err()
        );
    }
}

#[test]
fn term_caplet_allows_payment_before_unadjusted_accrual_end() {
    let df = |time: f64| (-0.03 * time).exp();
    let schedule = CapFloorSchedule {
        periods: vec![CapletSchedule {
            fixing_time: 0.25 - 2.0 / 365.0,
            start_time: 0.25,
            end_time: 0.5,
            payment_time: 0.5 - 1.0 / 365.0,
            accrual: 0.25 * 365.0 / 360.0,
        }],
    };
    let source = HullWhiteParams::constant(0.05, 0.012).expect("valid model");
    let premium = scheduled_cap_floor_price(&source, &df, &df, &schedule, 0.03, true)
        .expect("term rate is fixed before its early payment");
    assert!(premium.is_finite() && premium > 0.0);
    let normal_vol =
        scheduled_cap_floor_implied_normal_vol(premium, &df, &df, &schedule, 0.03, true)
            .expect("normal quote");
    let quote = CapFloorQuote::try_new(0.5, 0.03, normal_vol, true, true).expect("quote");
    let (fitted, report) =
        calibrate_hull_white_to_cap_floors_with_fn(&df, &df, &[quote], Some(&[schedule]), config())
            .expect("valid early-payment calibration");
    assert!(report.success);
    assert!((fitted.sigma - 0.012).abs() < 1e-9);
}
