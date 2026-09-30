//! Theta for seasoned FX barrier and touch options.
//!
//! Theta holds the market fixed, so the observed barrier state over the roll
//! is the one the as-of spot implies: breached only when the current spot is
//! at or beyond the barrier. These tests pin theta to the explicit reprice of
//! the instrument with that state set, and pin that ordinary pricing of the
//! rolled instrument without observed state still fails.

use finstack_quant_core::currency::Currency;
use finstack_quant_core::dates::Date;
use finstack_quant_core::market_data::context::MarketContext;
use finstack_quant_core::money::fx::FxQuery;
use finstack_quant_core::money::Money;
use finstack_quant_core::types::{BarrierDirection, CurveId};
use finstack_quant_valuations::instruments::fx::fx_touch_option::{
    FxTouchOption, PayoutTiming, TouchType,
};
use finstack_quant_valuations::instruments::json_loader::{InstrumentEnvelope, InstrumentJson};
use finstack_quant_valuations::instruments::{FxBarrierOption, Instrument, PricingOptions};
use finstack_quant_valuations::metrics::MetricId;

const GOLDEN: &str = include_str!(
    "../../golden/data/pricing/quantlib/fx_barrier_option/eurusd_up_out_call_3m_quantlib.json"
);

/// The QuantLib up-and-out golden: instrument, market and valuation date.
/// Its monitoring starts on the valuation date, so any theta roll enters the
/// monitored window with no observed barrier state.
fn golden() -> (FxBarrierOption, MarketContext, Date) {
    let fixture: serde_json::Value = serde_json::from_str(GOLDEN).expect("golden JSON");
    let market: MarketContext =
        serde_json::from_value(fixture["market"]["data"].clone()).expect("golden market");
    let envelope: InstrumentEnvelope =
        serde_json::from_value(fixture["instrument"].clone()).expect("golden instrument");
    let InstrumentJson::FxBarrierOption(option) = envelope.instrument else {
        panic!("golden instrument is an FX barrier option");
    };
    let as_of = finstack_quant_core::dates::parse_iso_date(
        fixture["metadata"]["valuation_date"]
            .as_str()
            .expect("valuation date"),
    )
    .expect("ISO valuation date");
    (option, market, as_of)
}

fn eurusd_spot(market: &MarketContext, as_of: Date) -> f64 {
    market
        .fx()
        .expect("FX matrix")
        .rate(FxQuery::new(Currency::EUR, Currency::USD, as_of))
        .expect("EURUSD quote")
        .rate
}

fn theta(instrument: &dyn Instrument, market: &MarketContext, as_of: Date) -> f64 {
    let result = instrument
        .price_with_metrics(market, as_of, &[MetricId::Theta], PricingOptions::default())
        .expect("theta prices");
    result
        .measures
        .get(&MetricId::Theta)
        .copied()
        .expect("theta in measures")
}

/// `PV(rolled instrument, rolled market, as_of + 1D) - PV(instrument, market, as_of)`:
/// theta for a one-day roll with no period cash.
fn one_day_reprice(
    instrument: &dyn Instrument,
    rolled_instrument: &dyn Instrument,
    market: &MarketContext,
    as_of: Date,
) -> f64 {
    let rolled_market = market.roll_forward(1).expect("roll market");
    let rolled = rolled_instrument
        .value(&rolled_market, as_of + time::Duration::days(1))
        .expect("rolled reprice")
        .amount();
    rolled - instrument.value(market, as_of).expect("base PV").amount()
}

fn assert_close(actual: f64, expected: f64, label: &str) {
    assert!(actual.is_finite(), "{label}: theta must be finite");
    assert!(
        (actual - expected).abs() <= 1e-9 * expected.abs().max(1.0),
        "{label}: theta {actual} must equal the spot-held reprice {expected}"
    );
}

#[test]
fn fx_barrier_golden_theta_rolls_observed_state_with_spot_held() {
    let (option, market, as_of) = golden();
    let spot = eurusd_spot(&market, as_of);
    assert!(
        spot < option.barrier,
        "golden spot sits below the up barrier"
    );

    let rolled_market = market.roll_forward(1).expect("roll market");
    let rolled_date = as_of + time::Duration::days(1);
    assert!(
        option.value(&rolled_market, rolled_date).is_err(),
        "ordinary seasoned pricing still requires observed_barrier_breached"
    );

    let mut observed = option.clone();
    observed.observed_barrier_breached = Some(false);
    let expected = one_day_reprice(&option, &observed, &market, as_of);
    let actual = theta(&option, &market, as_of);
    assert_close(actual, expected, "FX up-and-out call");
    assert!(
        actual < 0.0,
        "a long up-and-out call far from its barrier decays, got {actual}"
    );
}

#[test]
fn fx_barrier_theta_with_spot_beyond_barrier_rolls_as_breached() {
    let (mut option, market, as_of) = golden();
    let spot = eurusd_spot(&market, as_of);
    option.barrier = spot * 1.02;
    option.barrier_type = finstack_quant_core::types::BarrierType::DownAndIn;

    let mut observed = option.clone();
    observed.observed_barrier_breached = Some(true);
    let expected = one_day_reprice(&option, &observed, &market, as_of);
    assert_close(
        theta(&option, &market, as_of),
        expected,
        "FX down-and-in call with spot at or below its barrier",
    );
}

/// One-touch EURUSD option on the golden market, monitored from `as_of`.
fn touch(market_as_of: Date, barrier: f64, direction: BarrierDirection) -> FxTouchOption {
    let mut option = FxTouchOption::example().expect("example touch option");
    option.barrier = barrier;
    option.barrier_direction = direction;
    option.touch_type = TouchType::OneTouch;
    option.payout_timing = PayoutTiming::AtExpiry;
    option.payout_amount = Money::new(1_000_000.0, Currency::USD).expect("payout");
    option.monitoring_start_date = Some(market_as_of);
    option.expiry = market_as_of + time::Duration::days(91);
    option.vol_surface_id = CurveId::new("EURUSD-BARRIER-VOL-QL");
    option
}

#[test]
fn fx_touch_theta_rolls_observed_state_with_spot_held() {
    let (_, market, as_of) = golden();
    let spot = eurusd_spot(&market, as_of);

    for (barrier, direction, touched, label) in [
        (1.25, BarrierDirection::Up, false, "up one-touch above spot"),
        (spot, BarrierDirection::Down, true, "down one-touch at spot"),
    ] {
        let option = touch(as_of, barrier, direction);
        let rolled_market = market.roll_forward(1).expect("roll market");
        assert!(
            option
                .value(&rolled_market, as_of + time::Duration::days(1))
                .is_err(),
            "{label}: ordinary seasoned pricing still requires observed_barrier_breached"
        );
        let mut observed = option.clone();
        observed.observed_barrier_breached = Some(touched);
        let expected = one_day_reprice(&option, &observed, &market, as_of);
        assert_close(theta(&option, &market, as_of), expected, label);
    }
}
