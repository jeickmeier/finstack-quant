//! Theta for seasoned equity barrier options.
//!
//! Theta holds the market fixed, so observation dates inside the roll see the
//! as-of spot: the barrier is breached over the roll only when that spot is
//! at or beyond it. Theta must equal the explicit reprice of the option with
//! that observed state set.

use super::helpers::*;
use finstack_quant_core::dates::{Date, DayCount};
use finstack_quant_core::market_data::context::MarketContext;
use finstack_quant_core::types::BarrierType;
use finstack_quant_valuations::instruments::exotics::barrier_option::BarrierOption;
use finstack_quant_valuations::instruments::{Instrument, Monitoring, PricingOptions};
use finstack_quant_valuations::metrics::MetricId;
use time::macros::date;

const SPOT: f64 = 100.0;

fn market(as_of: Date) -> MarketContext {
    build_market_with_day_count(as_of, SPOT, 0.2, 0.05, 0.01, DayCount::Act365F)
}

fn theta(option: &BarrierOption, market: &MarketContext, as_of: Date) -> f64 {
    option
        .price_with_metrics(market, as_of, &[MetricId::Theta], PricingOptions::default())
        .expect("theta prices")
        .measures
        .get(&MetricId::Theta)
        .copied()
        .expect("theta in measures")
}

/// Theta over `days` days (capped at expiry) when there is no period cash.
fn reprice(option: &BarrierOption, observed: &BarrierOption, as_of: Date, rolled: Date) -> f64 {
    let base = market(as_of);
    let rolled_market = base
        .roll_forward((rolled - as_of).whole_days())
        .expect("roll market");
    observed
        .value(&rolled_market, rolled)
        .expect("rolled reprice")
        .amount()
        - option.value(&base, as_of).expect("base PV").amount()
}

fn assert_close(actual: f64, expected: f64, label: &str) {
    assert!(actual.is_finite(), "{label}: theta must be finite");
    assert!(
        (actual - expected).abs() <= 1e-9 * expected.abs().max(1.0),
        "{label}: theta {actual} must equal the spot-held reprice {expected}"
    );
}

#[test]
fn discrete_barrier_theta_observes_the_held_spot_on_dates_inside_the_roll() {
    let as_of = date!(2024 - 01 - 02);
    let expiry = date!(2024 - 04 - 02);

    for (barrier_type, barrier, breached) in [
        (BarrierType::UpAndOut, 120.0, false),
        (BarrierType::DownAndIn, SPOT, true),
    ] {
        let mut option = create_down_and_out_call(expiry, 100.0, barrier, DayCount::Act365F);
        option.barrier_type = barrier_type;
        option.monitoring = Monitoring::Discrete {
            observation_dates: vec![as_of, date!(2024 - 02 - 02), date!(2024 - 03 - 04), expiry],
        };
        option.instrument_pricing_overrides.model_config.mc_paths = Some(2_000);
        let rolled = as_of + time::Duration::days(1);
        assert!(
            option.value(&market(as_of).roll_forward(1).expect("roll"), rolled).is_err(),
            "{barrier_type:?}: ordinary pricing past an observation date still requires observed_barrier_breached"
        );

        let mut observed = option.clone();
        observed.observed_barrier_breached = Some(breached);
        let expected = reprice(&option, &observed, as_of, rolled);
        let label = format!("discrete {barrier_type:?}");
        let actual = theta(&option, &market(as_of), as_of);
        assert_close(actual, expected, &label);
    }
}

#[test]
fn continuous_barrier_theta_capped_at_expiry_uses_the_held_spot() {
    let expiry = date!(2024 - 04 - 02);
    let as_of = date!(2024 - 04 - 01);
    let option = create_down_and_out_call(expiry, 95.0, 80.0, DayCount::Act365F);

    let mut observed = option.clone();
    observed.observed_barrier_breached = Some(false);
    let expected = reprice(&option, &observed, as_of, expiry);
    let actual = theta(&option, &market(as_of), as_of);
    assert_close(
        actual,
        expected,
        "continuous down-and-out call rolled to expiry",
    );
    assert!(
        actual < 0.0,
        "an in-the-money down-and-out call loses its remaining time value, got {actual}"
    );
}
