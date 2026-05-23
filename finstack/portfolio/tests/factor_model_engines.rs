//! Factor-model sensitivity integration tests.

use finstack_core::currency::Currency;
use finstack_core::dates::Date;
use finstack_core::market_data::bumps::BumpUnits;
use finstack_core::market_data::context::MarketContext;
use finstack_core::market_data::term_structures::DiscountCurve;
use finstack_core::math::interp::InterpStyle;
use finstack_core::money::Money;
use finstack_core::types::CurveId;
use finstack_core::{InputError, Result};
use finstack_factor_model::{
    BumpSizeConfig, FactorDefinition, FactorId, FactorType, MarketMapping,
};
use finstack_portfolio::sensitivity::{
    DeltaBasedEngine, FactorSensitivityEngine, FullRepricingEngine,
};
use finstack_valuations::instruments::fixed_income::bond::Bond;
use finstack_valuations::instruments::Instrument;
use finstack_valuations::metrics::MetricId;
use time::Month;

fn make_date(year: i32, month: Month, day: u8) -> Result<Date> {
    Date::from_calendar_date(year, month, day).map_err(|_| {
        InputError::InvalidDate {
            year,
            month: month as u8,
            day,
        }
        .into()
    })
}

fn create_test_bond() -> Result<Bond> {
    let issue = make_date(2025, Month::January, 15)?;
    let maturity = make_date(2030, Month::January, 15)?;

    Bond::fixed(
        "BOND-FACTOR-MODEL",
        Money::new(1_000_000.0, Currency::USD),
        0.05,
        issue,
        maturity,
        "USD-OIS",
    )
}

fn create_test_market(base_date: Date) -> Result<MarketContext> {
    let curve = DiscountCurve::builder("USD-OIS")
        .base_date(base_date)
        .interp(InterpStyle::MonotoneConvex)
        .knots([
            (0.0, 1.0),
            (1.0, 0.98),
            (2.0, 0.96),
            (5.0, 0.88),
            (10.0, 0.70),
        ])
        .build()?;

    Ok(MarketContext::new().insert(curve))
}

fn rates_factor() -> FactorDefinition {
    FactorDefinition {
        id: FactorId::new("usd-rates"),
        factor_type: FactorType::Rates,
        market_mapping: MarketMapping::CurveParallel {
            curve_ids: vec![CurveId::new("USD-OIS")],
            units: BumpUnits::RateBp,
        },
        description: Some("USD discount curve parallel shift".to_string()),
    }
}

fn dv01_tolerance(expected: f64) -> f64 {
    expected.abs().max(1.0) * 1e-4
}

#[test]
fn delta_based_engine_matches_bond_dv01_metric() -> Result<()> {
    let bond = create_test_bond()?;
    let as_of = make_date(2025, Month::January, 15)?;
    let market = create_test_market(as_of)?;

    let metric_result = bond.price_with_metrics(
        &market,
        as_of,
        &[MetricId::Dv01],
        finstack_valuations::instruments::PricingOptions::default(),
    )?;
    let expected_dv01 = metric_result.measures[MetricId::Dv01.as_str()];

    let positions = vec![("bond-pos".to_string(), &bond as &dyn Instrument, 1.0)];
    let factors = vec![rates_factor()];
    let matrix = DeltaBasedEngine::new(BumpSizeConfig::default())
        .compute_sensitivities(&positions, &factors, &market, as_of)?;

    let actual_dv01 = matrix.delta(0, 0);
    assert!(
        (actual_dv01 - expected_dv01).abs() < dv01_tolerance(expected_dv01),
        "delta engine DV01 {} should match bond metric {}",
        actual_dv01,
        expected_dv01
    );
    Ok(())
}

#[test]
fn full_repricing_engine_matches_bond_dv01_metric() -> Result<()> {
    let bond = create_test_bond()?;
    let as_of = make_date(2025, Month::January, 15)?;
    let market = create_test_market(as_of)?;

    let metric_result = bond.price_with_metrics(
        &market,
        as_of,
        &[MetricId::Dv01],
        finstack_valuations::instruments::PricingOptions::default(),
    )?;
    let expected_dv01 = metric_result.measures[MetricId::Dv01.as_str()];

    let positions = vec![("bond-pos".to_string(), &bond as &dyn Instrument, 1.0)];
    let factors = vec![rates_factor()];
    let matrix = FullRepricingEngine::new(BumpSizeConfig::default(), 5)
        .compute_sensitivities(&positions, &factors, &market, as_of)?;

    let actual_dv01 = matrix.delta(0, 0);
    assert!(
        (actual_dv01 - expected_dv01).abs() < dv01_tolerance(expected_dv01),
        "full repricing DV01 {} should match bond metric {}",
        actual_dv01,
        expected_dv01
    );
    Ok(())
}
