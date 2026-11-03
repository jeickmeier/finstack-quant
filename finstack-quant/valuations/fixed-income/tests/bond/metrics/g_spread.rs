//! G-spread: the bond's yield over the par yield of a named government curve.

use finstack_quant_core::currency::Currency;
use finstack_quant_core::dates::StubKind;
use finstack_quant_core::market_data::context::MarketContext;
use finstack_quant_core::market_data::term_structures::DiscountCurve;
use finstack_quant_core::math::interp::InterpStyle;
use finstack_quant_core::money::Money;
use finstack_quant_core::types::Rate;
use finstack_quant_valuations_core::instruments::pricing_overrides::InstrumentPricingOverrides;
use finstack_quant_valuations_core::instruments::{Instrument, PricingOptions};
use finstack_quant_valuations_core::metrics::MetricId;
use finstack_quant_valuations_fixed_income::bond::{Bond, CallPut, CallPutSchedule};
use time::macros::date;

fn flat_curve(id: &str, zero_rate: f64) -> DiscountCurve {
    DiscountCurve::builder(id)
        .base_date(date!(2025 - 01 - 01))
        .day_count(finstack_quant_core::dates::DayCount::Act365F)
        .knots([(0.0, 1.0), (10.0, (-zero_rate * 10.0).exp())])
        .interp(InterpStyle::LogLinear)
        .build()
        .expect("flat curve")
}

/// Five-year 5% semi-annual 30/360 bullet, settling on the valuation date.
fn bullet() -> Bond {
    let mut bond = Bond::fixed(
        "G-SPREAD-BULLET",
        Money::new(1_000_000.0, Currency::USD).expect("money"),
        Rate::from_decimal(0.05).expect("rate"),
        date!(2025 - 01 - 01),
        date!(2030 - 01 - 01),
        StubKind::ShortFront,
        "USD-OIS",
    )
    .expect("bond");
    bond.settlement_convention = None;
    bond
}

/// Callable at 101 from 2027, quoted above the call so the worst exit is a call.
fn quoted_callable() -> Bond {
    let mut bond = bullet();
    bond.call_put = Some(CallPutSchedule {
        calls: vec![CallPut {
            start: date!(2027 - 01 - 01),
            end: date!(2027 - 01 - 01),
            price_pct_of_par: 101.0,
            make_whole: None,
        }],
        puts: vec![],
    });
    bond.instrument_pricing_overrides =
        InstrumentPricingOverrides::default().with_quoted_clean_price_pct(106.0);
    bond
}

fn metrics(bond: &Bond, market: &MarketContext, ids: &[MetricId]) -> Vec<f64> {
    let result = bond
        .price_with_metrics(
            market,
            date!(2025 - 01 - 01),
            ids,
            PricingOptions::default(),
        )
        .expect("metrics");
    ids.iter().map(|id| result.measures[id.as_str()]).collect()
}

fn named(mut bond: Bond, curve_id: &str) -> Bond {
    bond.instrument_pricing_overrides = bond
        .instrument_pricing_overrides
        .with_government_curve_id(curve_id);
    bond
}

#[test]
fn g_spread_on_the_swap_curve_is_i_spread_bit_for_bit() {
    let market = MarketContext::new().insert(flat_curve("USD-OIS", 0.04));
    for bond in [bullet(), quoted_callable()] {
        let values = metrics(
            &named(bond, "USD-OIS"),
            &market,
            &[MetricId::GSpread, MetricId::ISpread],
        );
        assert_eq!(values[0].to_bits(), values[1].to_bits());
    }
}

#[test]
fn the_callable_case_is_measured_to_the_workout_date() {
    // Pins that the bit-for-bit case above runs on the workout branch: at 106
    // the yield to the 2027 call at 101 is the worst, so the par rate is the
    // two-year one.
    let market = MarketContext::new().insert(flat_curve("USD-OIS", 0.04));
    let callable = metrics(
        &named(quoted_callable(), "USD-OIS"),
        &market,
        &[MetricId::GSpread, MetricId::Ytm, MetricId::Ytw],
    );
    assert!(callable[2] < callable[1], "worst exit is the call");
    let par_to_workout = callable[2] - callable[0];
    let two_year_par = flat_par_rate(0.04, 2);
    assert!((par_to_workout - two_year_par).abs() < 1e-12);
}

/// Par rate of a semi-annual 30/360 leg of `years` from 2025-01-01 on a flat
/// continuous zero curve with Act/365F times: `(1 − DF(T)) / Σ 0.5 DF(tᵢ)`.
fn flat_par_rate(zero_rate: f64, years: i32) -> f64 {
    let start = date!(2025 - 01 - 01);
    let df = |date: time::Date| (-zero_rate * (date - start).whole_days() as f64 / 365.0).exp();
    let mut annuity = 0.0;
    let mut last = start;
    for half_year in 1..=(2 * years) {
        let (year, month) = (
            2025 + half_year / 2,
            if half_year % 2 == 0 {
                time::Month::January
            } else {
                time::Month::July
            },
        );
        last = time::Date::from_calendar_date(year, month, 1).expect("date");
        annuity += 0.5 * df(last);
    }
    (1.0 - df(last)) / annuity
}

#[test]
fn a_curve_50bp_lower_widens_the_spread_by_its_par_rate_difference() {
    // Swap curve flat 4% and government curve flat 3.5%, continuous Act/365F.
    // Five-year semi-annual 30/360 par rates are 4.042099% and 3.532447%, so
    // the spread widens by 50.9652bp: the compounding basis takes it above 50.
    let market = MarketContext::new()
        .insert(flat_curve("USD-OIS", 0.04))
        .insert(flat_curve("UST", 0.035));
    let values = metrics(
        &named(bullet(), "UST"),
        &market,
        &[MetricId::GSpread, MetricId::ISpread],
    );
    let widening = values[0] - values[1];
    assert!((widening - (flat_par_rate(0.04, 5) - flat_par_rate(0.035, 5))).abs() < 1e-12);
    assert!((widening - 0.005_096_519_727).abs() < 1e-11, "{widening}");
}

#[test]
fn no_government_curve_is_an_error_and_never_the_discount_curve() {
    let market = MarketContext::new().insert(flat_curve("USD-OIS", 0.04));
    let error = bullet()
        .price_with_metrics(
            &market,
            date!(2025 - 01 - 01),
            &[MetricId::GSpread],
            PricingOptions::default(),
        )
        .expect_err("no curve named");
    assert!(error.to_string().contains("government_curve_id"), "{error}");
}

#[test]
fn a_named_curve_missing_from_the_market_is_not_found() {
    let market = MarketContext::new().insert(flat_curve("USD-OIS", 0.04));
    let error = named(bullet(), "UST")
        .price_with_metrics(
            &market,
            date!(2025 - 01 - 01),
            &[MetricId::GSpread],
            PricingOptions::default(),
        )
        .expect_err("curve missing");
    assert!(error.to_string().contains("UST"), "{error}");
    assert!(
        error.to_string().to_lowercase().contains("not found"),
        "{error}"
    );
}

#[test]
fn naming_a_government_curve_moves_no_price_or_parallel_dv01() {
    let market = MarketContext::new()
        .insert(flat_curve("USD-OIS", 0.04))
        .insert(flat_curve("UST", 0.035));
    let plain = metrics(
        &bullet(),
        &market,
        &[MetricId::Dv01, MetricId::Ytm, MetricId::ISpread],
    );
    let with_curve = metrics(
        &named(bullet(), "UST"),
        &market,
        &[MetricId::Dv01, MetricId::Ytm, MetricId::ISpread],
    );
    assert_eq!(plain, with_curve);
    let as_of = date!(2025 - 01 - 01);
    assert_eq!(
        bullet().value(&market, as_of).expect("pv"),
        named(bullet(), "UST").value(&market, as_of).expect("pv")
    );
}

#[test]
fn government_curve_id_is_strict_serde_and_omitted_when_unset() {
    let json = serde_json::to_value(bullet()).expect("json");
    assert!(json.to_string().find("government_curve_id").is_none());

    let with_curve = serde_json::to_value(named(bullet(), "UST")).expect("json");
    let back: Bond = serde_json::from_value(with_curve).expect("round trip");
    assert_eq!(
        back.instrument_pricing_overrides
            .model_config
            .government_curve_id
            .as_ref()
            .map(|id| id.as_str()),
        Some("UST")
    );

    let mut typo = serde_json::to_value(named(bullet(), "UST")).expect("json");
    typo["instrument_pricing_overrides"]["model_config"]["government_curve"] = "UST".into();
    assert!(serde_json::from_value::<Bond>(typo).is_err());
}

/// A call on the quote date below the price makes `ytw` negative infinity. Its
/// horizon is the quote date, so no par rate exists: both spreads are the same
/// error, never a non-finite value.
#[test]
fn a_workout_on_the_quote_date_is_the_same_error_as_i_spread() {
    let as_of = date!(2025 - 01 - 01);
    let market = MarketContext::new().insert(flat_curve("USD-OIS", 0.04));
    let mut bond = named(bullet(), "USD-OIS");
    bond.call_put = Some(CallPutSchedule {
        calls: vec![CallPut {
            start: as_of,
            end: as_of,
            price_pct_of_par: 99.0,
            make_whole: None,
        }],
        puts: vec![],
    });
    bond.instrument_pricing_overrides = bond
        .instrument_pricing_overrides
        .with_quoted_clean_price_pct(100.0);
    assert_eq!(
        metrics(&bond, &market, &[MetricId::Ytw])[0],
        f64::NEG_INFINITY
    );
    let wire = serde_json::to_value(
        bond.price_with_metrics(&market, as_of, &[MetricId::Ytw], PricingOptions::default())
            .expect("ytw"),
    )
    .expect("json");
    assert!(wire["measures"]["ytw"].is_null(), "{}", wire["measures"]);
    let error = |id: MetricId| {
        bond.price_with_metrics(&market, as_of, &[id], PricingOptions::default())
            .expect_err("no par rate to the quote date")
            .to_string()
    };
    let (g, i) = (error(MetricId::GSpread), error(MetricId::ISpread));
    assert!(g.contains("Invalid date range"), "{g}");
    assert!(i.contains("Invalid date range"), "{i}");
}

/// What naming a curve does to DV01: the curve is a dependency of the bond, so
/// bucketed DV01 reports it at zero, and parallel DV01 ignores it when missing.
#[test]
fn a_named_curve_is_a_dependency_with_no_dv01_of_its_own() {
    let as_of = date!(2025 - 01 - 01);
    let market = MarketContext::new()
        .insert(flat_curve("USD-OIS", 0.04))
        .insert(flat_curve("UST", 0.035));
    let bond = named(bullet(), "UST");
    assert!(
        bond.market_dependencies()
            .expect("dependencies")
            .curves
            .discount_curves
            .iter()
            .any(|id| id.as_str() == "UST")
    );
    let bucketed = bond
        .price_with_metrics(
            &market,
            as_of,
            &[MetricId::BucketedDv01],
            PricingOptions::default(),
        )
        .expect("bucketed dv01");
    let rows: Vec<(&str, f64)> = bucketed
        .measures
        .iter()
        .map(|(key, value)| (key.as_str(), *value))
        .collect();
    let ust: Vec<f64> = rows
        .iter()
        .filter(|(key, _)| key.contains("UST"))
        .map(|(_, value)| *value)
        .collect();
    assert!(
        !ust.is_empty() && ust.iter().all(|value| *value == 0.0),
        "{rows:?}"
    );

    let without_curve = MarketContext::new().insert(flat_curve("USD-OIS", 0.04));
    assert_eq!(
        metrics(&bond, &without_curve, &[MetricId::Dv01]),
        metrics(&bullet(), &without_curve, &[MetricId::Dv01]),
    );
}
