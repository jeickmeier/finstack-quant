use finstack_quant_cashflows::builder::{
    OvernightCompoundingMethod, OvernightIndexConstraintApplication, OvernightObservationSchedule,
    OvernightRateConstraints,
};
use finstack_quant_core::dates::{Date, DateExt, WEEKENDS_ONLY};
use finstack_quant_core::Result;
use time::Month;

fn january(day: u8) -> Date {
    Date::from_calendar_date(2025, Month::January, day).expect("valid January 2025 date")
}

fn constant_rate(_: &finstack_quant_cashflows::builder::OvernightObservationSlice) -> Result<f64> {
    Ok(0.05)
}

#[test]
fn in_arrears_compiles_business_observations_with_weekend_weight() {
    let start = january(6); // Monday
    let end = january(13); // Monday
    let schedule = OvernightObservationSchedule::compile(
        start,
        end,
        OvernightCompoundingMethod::CompoundedInArrears,
        &WEEKENDS_ONLY,
    )
    .expect("in-arrears observations compile");

    let observations = schedule.observations();
    assert_eq!(observations.len(), 5);
    assert_eq!(
        observations
            .iter()
            .map(|slice| slice.observation_date.day())
            .collect::<Vec<_>>(),
        vec![6, 7, 8, 9, 10]
    );
    assert_eq!(
        observations
            .iter()
            .map(|slice| slice.weight_days)
            .collect::<Vec<_>>(),
        vec![1, 1, 1, 1, 3]
    );
    assert!(observations.iter().all(|slice| slice.rate_tenor_days == 1));

    let replay = schedule
        .replay(
            end,
            360.0,
            OvernightRateConstraints::default(),
            constant_rate,
        )
        .expect("full replay");
    let expected =
        ((1.0_f64 + 0.05 / 360.0).powi(4) * (1.0 + 0.05 * 3.0 / 360.0) - 1.0) * 360.0 / 7.0;
    assert!((replay.projected_rate - expected).abs() < 1e-12);
    assert_eq!(replay.projected_rate, replay.constrained_rate);
    assert_eq!(replay.weight_days, 7);
}

#[test]
fn lookback_moves_observations_and_keeps_accrual_weights() {
    let start = january(6);
    let end = january(13);
    let schedule = OvernightObservationSchedule::compile(
        start,
        end,
        OvernightCompoundingMethod::CompoundedWithLookback { lookback_days: 2 },
        &WEEKENDS_ONLY,
    )
    .expect("lookback observations compile");

    assert_eq!(
        schedule
            .observations()
            .iter()
            .map(|slice| slice.observation_date.day())
            .collect::<Vec<_>>(),
        vec![2, 3, 6, 7, 8]
    );
    assert_eq!(
        schedule
            .observations()
            .iter()
            .map(|slice| (slice.weight_start.day(), slice.weight_days))
            .collect::<Vec<_>>(),
        vec![(6, 1), (7, 1), (8, 1), (9, 1), (10, 3)]
    );
}

#[test]
fn lockout_freezes_only_from_the_contractual_cutoff() {
    let start = january(6);
    let end = january(13);
    let schedule = OvernightObservationSchedule::compile(
        start,
        end,
        OvernightCompoundingMethod::CompoundedWithLockout { lockout_days: 2 },
        &WEEKENDS_ONLY,
    )
    .expect("lockout observations compile");

    assert_eq!(
        schedule
            .observations()
            .iter()
            .map(|slice| slice.observation_date.day())
            .collect::<Vec<_>>(),
        vec![6, 7, 8, 9, 9]
    );

    let mut seen = Vec::new();
    schedule
        .replay(
            january(9),
            360.0,
            OvernightRateConstraints::default(),
            |slice| {
                seen.push(slice.observation_date.day());
                Ok(0.05)
            },
        )
        .expect("pre-cutoff partial replay");
    assert_eq!(seen, vec![6, 7, 8]);
}

#[test]
fn observation_shift_moves_rates_weights_and_partial_cutoff() {
    let start = january(13); // Monday
    let end = january(20); // Monday
    let shift_days = 2;
    let schedule = OvernightObservationSchedule::compile(
        start,
        end,
        OvernightCompoundingMethod::CompoundedWithObservationShift { shift_days },
        &WEEKENDS_ONLY,
    )
    .expect("observation-shift schedule compiles");

    assert_eq!(
        schedule.observations()[0].weight_start,
        start
            .add_business_days(-2, &WEEKENDS_ONLY)
            .expect("shift start")
    );
    assert_eq!(
        schedule
            .observations()
            .iter()
            .map(|slice| (slice.observation_date.day(), slice.weight_days))
            .collect::<Vec<_>>(),
        vec![(9, 1), (10, 3), (13, 1), (14, 1), (15, 1)]
    );

    let mut seen = Vec::new();
    let partial = schedule
        .replay(
            january(15),
            360.0,
            OvernightRateConstraints::default(),
            |slice| {
                seen.push((slice.observation_date.day(), slice.weight_days));
                Ok(0.05)
            },
        )
        .expect("shifted partial replay");
    assert_eq!(seen, vec![(9, 1), (10, 3)]);
    assert_eq!(partial.weight_days, 4);
}

#[test]
fn daily_and_period_index_bounds_apply_at_different_levels() {
    let start = january(6);
    let end = january(9);
    let schedule = OvernightObservationSchedule::compile(
        start,
        end,
        OvernightCompoundingMethod::SimpleAverage,
        &WEEKENDS_ONLY,
    )
    .expect("simple-average observations compile");
    let observed_rate = |slice: &finstack_quant_cashflows::builder::OvernightObservationSlice| {
        Ok(match slice.observation_date.day() {
            6 => 0.00,
            7 => 0.04,
            _ => 0.20,
        })
    };
    let bounds = |application| OvernightRateConstraints {
        application,
        index_floor_bp: Some(300.0),
        index_cap_bp: Some(500.0),
    };

    let daily = schedule
        .replay(
            end,
            360.0,
            bounds(OvernightIndexConstraintApplication::Daily),
            observed_rate,
        )
        .expect("daily constrained replay");
    let period = schedule
        .replay(
            end,
            360.0,
            bounds(OvernightIndexConstraintApplication::Period),
            observed_rate,
        )
        .expect("period constrained replay");

    assert!((daily.projected_rate - 0.08).abs() < 1e-12);
    assert!((period.projected_rate - 0.08).abs() < 1e-12);
    assert!((daily.constrained_rate - 0.04).abs() < 1e-12);
    assert!((period.constrained_rate - 0.05).abs() < 1e-12);
}

#[test]
fn partial_replay_clips_weekend_weight_without_changing_fixing_tenor() {
    let start = january(10); // Friday
    let end = january(14); // Tuesday
    let schedule = OvernightObservationSchedule::compile(
        start,
        end,
        OvernightCompoundingMethod::CompoundedInArrears,
        &WEEKENDS_ONLY,
    )
    .expect("in-arrears observations compile");
    assert_eq!(schedule.observations()[0].weight_days, 3);

    let mut seen = Vec::new();
    let replay = schedule
        .replay(
            january(12), // Sunday exercise
            360.0,
            OvernightRateConstraints::default(),
            |slice| {
                seen.push((
                    slice.observation_date,
                    slice.weight_days,
                    slice.rate_tenor_days,
                ));
                Ok(0.05)
            },
        )
        .expect("partial weekend replay");

    assert_eq!(seen, vec![(january(10), 2, 1)]);
    assert_eq!(replay.weight_days, 2);
    assert!((replay.projected_rate - 0.05).abs() < 1e-12);
}
