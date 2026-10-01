//! SOFR fixing-calendar regression tests against New York Fed closure notices.

use finstack_quant_cashflows::builder::{
    CashFlowSchedule, CouponType, FloatingCouponSpec, FloatingRateSpec,
    OvernightObservationSchedule, OvernightRateConstraints, ScheduleParams,
};
use finstack_quant_core::cashflow::CFKind;
use finstack_quant_core::currency::Currency;
use finstack_quant_core::dates::{calendar_by_id_strict, Date, DayCount};
use finstack_quant_core::market_data::context::MarketContext;
use finstack_quant_core::market_data::scalars::ScalarTimeSeries;
use finstack_quant_core::market_data::term_structures::ForwardCurve;
use finstack_quant_core::money::Money;
use rust_decimal_macros::dec;
use time::macros::date;

#[test]
fn sofr_preset_carries_thursday_fixing_across_repo_market_holidays() {
    for (start, end) in [
        (date!(2025 - 04 - 17), date!(2025 - 04 - 21)),
        (date!(2026 - 04 - 02), date!(2026 - 04 - 06)),
        (date!(2026 - 07 - 02), date!(2026 - 07 - 06)),
    ] {
        let spec = FloatingRateSpec::sofr(dec!(0));
        let calendar_id = spec.fixing_calendar_id.as_ref().expect("fixing calendar");
        assert_eq!(calendar_id.as_str(), "sofr");
        let calendar = calendar_by_id_strict(calendar_id.as_str()).unwrap();
        let schedule = OvernightObservationSchedule::compile(
            start,
            end,
            spec.compounding.expect("daily compounding"),
            calendar,
        )
        .unwrap();
        assert_eq!(schedule.observations().len(), 1);
        assert_eq!(schedule.observations()[0].observation_date, start);
        assert_eq!(schedule.observations()[0].weight_days, 4);
        let replay = schedule
            .replay(end, 360.0, OvernightRateConstraints::default(), |slice| {
                assert_eq!(slice.observation_date, start);
                Ok(0.05)
            })
            .unwrap();
        assert!((replay.projected_rate - 0.05).abs() < 1e-12);

        let coupon = historical_sofr_coupon(start, end);
        assert!((coupon - 1_000_000.0 * 0.05 * 4.0 / 360.0).abs() < 1e-8);
    }
}

fn historical_sofr_coupon(start: Date, end: Date) -> f64 {
    // Only the Thursday fixing exists. The base date after the entire coupon
    // ensures every required observation must resolve from supplied history.
    let forward = ForwardCurve::builder("USD-SOFR", 1.0 / 360.0)
        .base_date(end)
        .day_count(DayCount::Act360)
        .knots([(0.0, 0.07), (1.0, 0.07)])
        .build()
        .unwrap();
    let fixings = ScalarTimeSeries::new("FIXING:USD-SOFR", vec![(start, 0.05)], None).unwrap();
    let market = MarketContext::new().insert(forward).insert_series(fixings);
    let spec = FloatingCouponSpec {
        rate_spec: FloatingRateSpec::sofr(dec!(0)),
        coupon_type: CouponType::Cash,
        schedule: ScheduleParams::quarterly_act360(),
    };
    let mut builder = CashFlowSchedule::builder();
    builder
        .principal(Money::new(1_000_000.0, Currency::USD).unwrap(), start, end)
        .floating_cf(spec)
        .build(Some(&market))
        .expect("SOFR coupon requires no nonexistent Friday fixing")
        .get_flows()
        .iter()
        .find(|flow| flow.kind == CFKind::FloatReset)
        .expect("one floating coupon")
        .amount
        .amount()
}
