//! Theta for path-dependent equity products whose fixings fall inside the roll.
//!
//! Theta holds the market fixed, so a scheduled fixing between the valuation
//! date and the rolled date observes the as-of spot. Theta must equal the
//! explicit reprice of the product with those fixings (and, for a trade struck
//! at the valuation date, that strike-set level) recorded.

use finstack_quant_core::dates::{Date, DayCount};
use finstack_quant_core::market_data::context::MarketContext;
use finstack_quant_core::market_data::scalars::MarketScalar;
use finstack_quant_core::market_data::surfaces::VolSurface;
use finstack_quant_core::market_data::term_structures::DiscountCurve;
use finstack_quant_valuations::instruments::{
    AsianOption, Autocallable, CliquetOption, Instrument, PricingOptions,
};
use finstack_quant_valuations::metrics::MetricId;
use time::macros::date;

const SPOT: f64 = 100.0;

/// Flat 20% vol, 1% dividend yield and a 3% discount curve under the ids the
/// equity examples use.
fn market(as_of: Date) -> MarketContext {
    let curve = DiscountCurve::builder("USD-OIS")
        .base_date(as_of)
        .day_count(DayCount::Act365F)
        .knots([(0.0, 1.0), (1.0, 0.97), (2.0, 0.94), (5.0, 0.86)])
        .build()
        .expect("curve");
    let surface = VolSurface::builder("SPX-VOL")
        .expiries(&[0.25, 0.5, 1.0, 2.0])
        .strikes(&[60.0, 80.0, 100.0, 120.0, 140.0])
        .row(&[0.20; 5])
        .row(&[0.20; 5])
        .row(&[0.20; 5])
        .row(&[0.20; 5])
        .build()
        .expect("surface");
    MarketContext::new()
        .insert(curve)
        .insert_surface(surface)
        .insert_price("SPX-SPOT", MarketScalar::Unitless(SPOT))
        .insert_price("SPX-DIV", MarketScalar::Unitless(0.01))
}

fn theta(instrument: &dyn Instrument, as_of: Date) -> f64 {
    instrument
        .price_with_metrics(
            &market(as_of),
            as_of,
            &[MetricId::Theta],
            PricingOptions::default(),
        )
        .expect("theta prices")
        .measures
        .get(&MetricId::Theta)
        .copied()
        .expect("theta in measures")
}

/// One-day theta reprice with no period cash.
fn one_day_reprice(instrument: &dyn Instrument, observed: &dyn Instrument, as_of: Date) -> f64 {
    let base = market(as_of);
    let rolled_date = as_of + time::Duration::days(1);
    let rolled = observed
        .value(&base.roll_forward(1).expect("roll market"), rolled_date)
        .expect("rolled reprice")
        .amount();
    assert!(
        instrument
            .value(&base.roll_forward(1).expect("roll market"), rolled_date)
            .is_err(),
        "ordinary pricing past a fixing date still requires the recorded fixing"
    );
    rolled - instrument.value(&base, as_of).expect("base PV").amount()
}

fn assert_close(actual: f64, expected: f64, label: &str) {
    assert!(actual.is_finite(), "{label}: theta must be finite");
    assert!(
        (actual - expected).abs() <= 1e-9 * expected.abs().max(1.0),
        "{label}: theta {actual} must equal the spot-held reprice {expected}"
    );
}

#[test]
fn asian_theta_records_the_held_spot_for_a_fixing_inside_the_roll() {
    let as_of = date!(2024 - 01 - 30);
    let option = AsianOption::example().expect("example asian option");
    let mut observed = option.clone();
    observed.past_fixings = vec![(date!(2024 - 01 - 31), SPOT)];
    let expected = one_day_reprice(&option, &observed, as_of);
    assert_close(theta(&option, as_of), expected, "asian option");
}

#[test]
fn autocallable_theta_records_strike_level_and_fixing_inside_the_roll() {
    let as_of = date!(2024 - 03 - 28);
    let mut note = Autocallable::example().expect("example autocallable");
    note.initial_level = None;
    note.instrument_pricing_overrides.model_config.mc_paths = Some(2_000);
    let mut observed = note.clone();
    observed.initial_level = Some(SPOT);
    observed.past_fixings = vec![(date!(2024 - 03 - 29), SPOT)];
    let expected = one_day_reprice(&note, &observed, as_of);
    assert_close(theta(&note, as_of), expected, "autocallable");
}

#[test]
fn cliquet_theta_records_the_held_spot_for_a_reset_on_the_valuation_date() {
    let as_of = date!(2024 - 03 - 29);
    let mut option = CliquetOption::example().expect("example cliquet");
    option.initial_level = Some(95.0);
    option.past_fixings.clear();
    option.instrument_pricing_overrides.model_config.mc_paths = Some(2_000);
    let mut observed = option.clone();
    observed.past_fixings = vec![(as_of, SPOT)];
    let expected = one_day_reprice(&option, &observed, as_of);
    assert_close(theta(&option, as_of), expected, "cliquet option");
}
