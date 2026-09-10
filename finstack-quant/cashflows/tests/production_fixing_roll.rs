//! Crossed fixings preserve source observations and fail atomically.
use finstack_quant_cashflows::{
    builder::{CashFlowMeta, CashFlowSchedule, Notional},
    fixings::{materialize_fixings, ProjectedFixing},
};
use finstack_quant_core::{
    currency::Currency,
    dates::DayCount,
    market_data::{
        context::MarketContext,
        scalars::{ScalarTimeSeries, SeriesInterpolation},
    },
};
use time::macros::date;

fn schedule(values: &[Option<f64>]) -> CashFlowSchedule {
    CashFlowSchedule::from_parts(
        vec![],
        Notional::par(1.0, Currency::USD).unwrap(),
        DayCount::Act360,
        CashFlowMeta {
            projected_fixings: values
                .iter()
                .map(|value| ProjectedFixing {
                    series_id: "FIXING:USD-SOFR".into(),
                    date: date!(2025 - 01 - 03),
                    value: *value,
                })
                .collect(),
            ..Default::default()
        },
    )
}

#[test]
fn production_roll_keeps_existing_fixing_values_and_series_metadata() {
    let market = MarketContext::new().insert_series(
        ScalarTimeSeries::new(
            "FIXING:USD-SOFR",
            vec![(date!(2025 - 01 - 03), 0.07)],
            Some(Currency::USD),
        )
        .unwrap()
        .with_interpolation(SeriesInterpolation::Linear),
    );
    let projected = schedule(&[Some(0.03), None, Some(f64::NAN)]);
    let rolled = materialize_fixings(
        &market,
        [&projected],
        date!(2025 - 01 - 02),
        date!(2025 - 01 - 06),
    )
    .unwrap();
    let series = rolled.get_series("FIXING:USD-SOFR").unwrap();
    assert_eq!(series.value_on_exact(date!(2025 - 01 - 03)).unwrap(), 0.07);
    assert_eq!(series.currency(), Some(Currency::USD));
    assert_eq!(series.interpolation(), SeriesInterpolation::Linear);
    assert_eq!(
        market.get_series("FIXING:USD-SOFR").unwrap().observations(),
        series.observations()
    );
}

#[test]
fn production_roll_materializes_once_and_reports_conflicting_or_missing_projections() {
    let market = MarketContext::new();
    let projected = schedule(&[None, Some(0.03), Some(0.03)]);
    let rolled = materialize_fixings(
        &market,
        [&projected],
        date!(2025 - 01 - 02),
        date!(2025 - 01 - 06),
    )
    .unwrap();
    let repeated = materialize_fixings(
        &rolled,
        [&projected],
        date!(2025 - 01 - 02),
        date!(2025 - 01 - 06),
    )
    .unwrap();
    assert_eq!(
        repeated
            .get_series("FIXING:USD-SOFR")
            .unwrap()
            .observations(),
        vec![(date!(2025 - 01 - 03), 0.03)]
    );
    assert!(market.get_series("FIXING:USD-SOFR").is_err());
    for (values, message) in [
        (vec![None], "missing pre-roll projection"),
        (
            vec![Some(0.03), Some(0.04)],
            "conflicting pre-roll projections",
        ),
        (vec![Some(f64::INFINITY)], "non-finite pre-roll projection"),
    ] {
        let invalid = schedule(&values);
        let error = materialize_fixings(
            &market,
            [&invalid],
            date!(2025 - 01 - 02),
            date!(2025 - 01 - 06),
        )
        .unwrap_err();
        assert!(error.to_string().contains(message), "{error}");
        assert!(error.to_string().contains("FIXING:USD-SOFR"), "{error}");
        assert!(market.get_series("FIXING:USD-SOFR").is_err());
    }
}

#[test]
fn production_roll_uses_exclusive_origin_and_inclusive_horizon() {
    let market = MarketContext::new();
    let unavailable = schedule(&[None]);
    assert!(materialize_fixings(
        &market,
        [&unavailable],
        date!(2025 - 01 - 01),
        date!(2025 - 01 - 02)
    )
    .is_ok());
    assert!(materialize_fixings(
        &market,
        [&unavailable],
        date!(2025 - 01 - 03),
        date!(2025 - 01 - 04)
    )
    .is_ok());
    assert!(materialize_fixings(
        &market,
        [&unavailable],
        date!(2025 - 01 - 02),
        date!(2025 - 01 - 03)
    )
    .is_err());
    assert!(materialize_fixings(
        &market,
        [&unavailable],
        date!(2025 - 01 - 04),
        date!(2025 - 01 - 03)
    )
    .is_err());
}
