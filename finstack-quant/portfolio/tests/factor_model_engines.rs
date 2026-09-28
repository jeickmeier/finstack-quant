//! Factor-model sensitivity integration tests.

use finstack_quant_core::currency::Currency;
use finstack_quant_core::dates::Date;
use finstack_quant_core::market_data::bumps::BumpUnits;
use finstack_quant_core::market_data::context::MarketContext;
use finstack_quant_core::market_data::term_structures::DiscountCurve;
use finstack_quant_core::math::interp::InterpStyle;
use finstack_quant_core::money::fx::{FxMatrix, SimpleFxProvider};
use finstack_quant_core::money::Money;
use finstack_quant_core::types::CurveId;
use finstack_quant_core::{InputError, Result};
use finstack_quant_models::factor::{
    BumpSizeConfig, FactorCovarianceMatrix, FactorDefinition, FactorId, FactorModelConfig,
    FactorType, MarketMapping, PricingMode, RiskMeasure, UnmatchedPolicy,
};
use finstack_quant_portfolio::factor_model::FactorModelBuilder;
use finstack_quant_portfolio::position::{Position, PositionUnit};
use finstack_quant_portfolio::sensitivity::{
    DeltaBasedEngine, FactorSensitivityEngine, FullRepricingEngine,
};
use finstack_quant_portfolio::types::DUMMY_ENTITY_ID;
use finstack_quant_portfolio::Portfolio;
use finstack_quant_valuations::instruments::fixed_income::bond::Bond;
use finstack_quant_valuations::instruments::Instrument;
use finstack_quant_valuations::metrics::MetricId;
use std::sync::Arc;
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
        Money::new(1_000_000.0, Currency::USD).expect("valid money fixture"),
        finstack_quant_core::types::Rate::from_decimal(0.05).expect("valid rate fixture"),
        issue,
        maturity,
        finstack_quant_core::dates::StubKind::ShortFront,
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
        finstack_quant_valuations::instruments::PricingOptions::default(),
    )?;
    let expected_dv01 = metric_result.measures[MetricId::Dv01.as_str()];

    let positions = vec![("bond-pos".to_string(), &bond as &dyn Instrument, 1.0)];
    let factors = vec![rates_factor()];
    let matrix = DeltaBasedEngine::new(BumpSizeConfig::default()).compute_sensitivities(
        &positions,
        &factors,
        &market,
        as_of,
        Currency::USD,
    )?;

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
        finstack_quant_valuations::instruments::PricingOptions::default(),
    )?;
    let expected_dv01 = metric_result.measures[MetricId::Dv01.as_str()];

    let positions = vec![("bond-pos".to_string(), &bond as &dyn Instrument, 1.0)];
    let factors = vec![rates_factor()];
    let matrix = FullRepricingEngine::new(BumpSizeConfig::default(), 5)?.compute_sensitivities(
        &positions,
        &factors,
        &market,
        as_of,
        Currency::USD,
    )?;

    let actual_dv01 = matrix.delta(0, 0);
    assert!(
        (actual_dv01 - expected_dv01).abs() < dv01_tolerance(expected_dv01),
        "full repricing DV01 {} should match bond metric {}",
        actual_dv01,
        expected_dv01
    );
    Ok(())
}

fn create_eur_bond() -> Result<Bond> {
    let issue = make_date(2025, Month::January, 15)?;
    let maturity = make_date(2030, Month::January, 15)?;

    Bond::fixed(
        "BOND-FACTOR-MODEL-EUR",
        Money::new(1_000_000.0, Currency::EUR).expect("valid money fixture"),
        finstack_quant_core::types::Rate::from_decimal(0.04).expect("valid rate fixture"),
        issue,
        maturity,
        finstack_quant_core::dates::StubKind::ShortFront,
        "EUR-OIS",
    )
}

fn create_two_currency_market(base_date: Date) -> Result<MarketContext> {
    let eur_curve = DiscountCurve::builder("EUR-OIS")
        .base_date(base_date)
        .interp(InterpStyle::MonotoneConvex)
        .knots([
            (0.0, 1.0),
            (1.0, 0.99),
            (2.0, 0.97),
            (5.0, 0.90),
            (10.0, 0.75),
        ])
        .build()?;

    Ok(create_test_market(base_date)?.insert(eur_curve))
}

fn create_two_currency_market_with_fx(base_date: Date) -> Result<MarketContext> {
    let provider = Arc::new(SimpleFxProvider::new());
    provider.set_quotes(&[(Currency::EUR, Currency::USD, 1.10)])?;
    Ok(create_two_currency_market(base_date)?.insert_fx(FxMatrix::new(provider)))
}

fn fx_factor() -> FactorDefinition {
    FactorDefinition {
        id: FactorId::new("eur-usd"),
        factor_type: FactorType::Fx,
        market_mapping: MarketMapping::FxRate {
            pair: (Currency::EUR, Currency::USD),
        },
        description: Some("EURUSD spot".to_string()),
    }
}

#[test]
fn delta_based_engine_eur_in_usd_book_has_nonzero_fx_factor() -> Result<()> {
    let eur_bond = create_eur_bond()?;
    let as_of = make_date(2025, Month::January, 15)?;
    let market = create_two_currency_market_with_fx(as_of)?;
    let positions = vec![("eur-pos".to_string(), &eur_bond as &dyn Instrument, 2.0)];
    let factors = vec![fx_factor()];

    let matrix = DeltaBasedEngine::new(BumpSizeConfig::default()).compute_sensitivities(
        &positions,
        &factors,
        &market,
        as_of,
        Currency::USD,
    )?;
    let one_lot = DeltaBasedEngine::new(BumpSizeConfig::default()).compute_sensitivities(
        &[("eur-pos".to_string(), &eur_bond as &dyn Instrument, 1.0)],
        &factors,
        &market,
        as_of,
        Currency::USD,
    )?;

    assert!(
        matrix.delta(0, 0).abs() > 1e-8,
        "EUR translation through bumped USD spot must be a non-zero FX factor"
    );
    assert!(
        (matrix.delta(0, 0) - 2.0 * one_lot.delta(0, 0)).abs() < 1e-8,
        "engine weight must scale the FX-factor column"
    );
    Ok(())
}

#[test]
fn full_repricing_engine_eur_in_usd_book_has_nonzero_fx_factor() -> Result<()> {
    let eur_bond = create_eur_bond()?;
    let as_of = make_date(2025, Month::January, 15)?;
    let market = create_two_currency_market_with_fx(as_of)?;
    let positions = vec![("eur-pos".to_string(), &eur_bond as &dyn Instrument, 1.0)];
    let factors = vec![fx_factor()];

    let matrix = FullRepricingEngine::new(BumpSizeConfig::default(), 5)?.compute_sensitivities(
        &positions,
        &factors,
        &market,
        as_of,
        Currency::USD,
    )?;

    assert!(
        matrix.delta(0, 0).abs() > 1e-8,
        "full reprice must convert grid PVs through bumped spot FX"
    );
    Ok(())
}

#[test]
fn delta_based_engine_fails_closed_when_cross_currency_fx_is_missing() -> Result<()> {
    let eur_bond = create_eur_bond()?;
    let as_of = make_date(2025, Month::January, 15)?;
    let market = create_two_currency_market(as_of)?;
    let positions = vec![("eur-pos".to_string(), &eur_bond as &dyn Instrument, 1.0)];
    let factors = vec![fx_factor()];

    let error = DeltaBasedEngine::new(BumpSizeConfig::default())
        .compute_sensitivities(&positions, &factors, &market, as_of, Currency::USD)
        .expect_err("missing FX must fail closed");
    let message = error.to_string();
    assert!(
        message.contains("FX matrix") || message.contains("FX conversion"),
        "unexpected error: {message}"
    );
    Ok(())
}

#[test]
fn portfolio_wrap_uses_scale_factor_weight_for_eur_fx_factor() -> Result<()> {
    let eur_bond = create_eur_bond()?;
    let as_of = make_date(2025, Month::January, 15)?;
    let market = create_two_currency_market_with_fx(as_of)?;
    let factor = fx_factor();
    let covariance = FactorCovarianceMatrix::new(vec![factor.id.clone()], vec![0.01])?;
    let model = FactorModelBuilder::new()
        .config(FactorModelConfig {
            factors: vec![factor],
            covariance,
            matching: finstack_quant_models::factor::MatchingConfig::MappingTable(vec![]),
            pricing_mode: PricingMode::DeltaBased,
            risk_measure: RiskMeasure::Variance,
            bump_config: None,
            unmatched_policy: Some(UnmatchedPolicy::Residual),
        })
        .build()?;

    let two_lot = Position::new(
        "eur-pos",
        DUMMY_ENTITY_ID,
        "BOND-FACTOR-MODEL-EUR",
        Arc::new(eur_bond.clone()),
        2.0,
        PositionUnit::Units,
    )?;
    let one_lot = Position::new(
        "eur-pos",
        DUMMY_ENTITY_ID,
        "BOND-FACTOR-MODEL-EUR",
        Arc::new(eur_bond),
        1.0,
        PositionUnit::Units,
    )?;
    let two_lot_book = Portfolio::builder("usd-book-two")
        .base_currency(Currency::USD)
        .as_of(as_of)
        .position(two_lot)
        .build()?;
    let one_lot_book = Portfolio::builder("usd-book-one")
        .base_currency(Currency::USD)
        .as_of(as_of)
        .position(one_lot)
        .build()?;

    let two = model.compute_sensitivities(&two_lot_book, &market, as_of)?;
    let one = model.compute_sensitivities(&one_lot_book, &market, as_of)?;
    assert!(
        two.delta(0, 0).abs() > 1e-8,
        "EUR instrument in a USD book must have a non-zero FX-factor column"
    );
    assert!(
        (two.delta(0, 0) - 2.0 * one.delta(0, 0)).abs() < 1e-8,
        "Portfolio wrap weight must be scale_factor()"
    );

    let stressed = model.factor_stress(
        &two_lot_book,
        &market,
        as_of,
        &[(FactorId::new("eur-usd"), 1.0)],
    )?;
    assert!(
        stressed.total_pnl.abs() > 1e-8,
        "FX factor stress must convert through the bumped spot matrix"
    );
    Ok(())
}

// ------------------------------------------------------------------
// Volatility factors
// ------------------------------------------------------------------

const VOL_SPOT: f64 = 100.0;
const VOL_LEVEL: f64 = 0.20;
const VOL_RATE: f64 = 0.03;
const VOL_DIV: f64 = 0.01;

fn equity_vol_market(as_of: Date) -> Result<MarketContext> {
    use finstack_quant_core::market_data::scalars::MarketScalar;
    use finstack_quant_core::market_data::surfaces::VolSurface;

    let discount = DiscountCurve::builder("USD-OIS")
        .base_date(as_of)
        .knots([(0.0, 1.0), (5.0, (-VOL_RATE * 5.0).exp())])
        .build()?;
    let strikes = [80.0, 90.0, 100.0, 110.0, 120.0];
    let expiries = [0.25, 0.5, 1.0, 2.0];
    let mut surface = VolSurface::builder("EQUITY-VOL")
        .expiries(&expiries)
        .strikes(&strikes);
    for _ in expiries {
        surface = surface.row(&[VOL_LEVEL; 5]);
    }
    Ok(MarketContext::new()
        .insert(discount)
        .insert_surface(surface.build()?)
        .insert_price("EQUITY-SPOT", MarketScalar::Unitless(VOL_SPOT))
        .insert_price("EQUITY-DIVYIELD", MarketScalar::Unitless(VOL_DIV)))
}

fn equity_vol_factor() -> FactorDefinition {
    FactorDefinition {
        id: FactorId::new("spx-vol"),
        factor_type: FactorType::Volatility,
        market_mapping: MarketMapping::VolShift {
            vol_surface_ids: vec!["EQUITY-VOL".to_string()],
            units: BumpUnits::Percent,
        },
        description: Some("SPX implied vol parallel shift".to_string()),
    }
}

/// A volatility factor resolves to a one-vol-point bump (`vol_points = 1.0`,
/// canonical unit `BumpUnits::Percent`, i.e. +0.01 absolute vol), so its
/// delta is P&L per vol point and must equal the instrument's Vega.
///
/// Independent reference: the option's `Vega` metric (per one vol point) and
/// the Black-Scholes closed form `N·S·e^{-qT}·φ(d1)·√T·0.01`. The engine's
/// central difference of width one vol point differs from the analytic
/// derivative by `O(h²)·∂³V/∂σ³/6`, which is below 1e-3 relative for a
/// 6-month ATM option at σ = 20%, so both comparisons use 1e-3 relative.
#[test]
fn vol_factor_delta_matches_equity_option_vega() -> Result<()> {
    use finstack_quant_core::math::special_functions::norm_pdf;
    use finstack_quant_valuations::instruments::{EquityOption, OptionType};

    let as_of = make_date(2025, Month::January, 2)?;
    let expiry = make_date(2025, Month::July, 2)?;
    let option = EquityOption::european(
        "EQ-VOL-FACTOR",
        "SPX",
        100.0,
        expiry,
        100.0,
        Currency::USD,
        OptionType::Call,
    )?;
    let market = equity_vol_market(as_of)?;

    let metric = option.price_with_metrics(
        &market,
        as_of,
        &[MetricId::Vega],
        finstack_quant_valuations::instruments::PricingOptions::default(),
    )?;
    let vega = metric.measures[MetricId::Vega.as_str()];

    let t = (expiry - as_of).whole_days() as f64 / 365.0;
    let d1 = ((VOL_SPOT / 100.0).ln() + (VOL_RATE - VOL_DIV + 0.5 * VOL_LEVEL * VOL_LEVEL) * t)
        / (VOL_LEVEL * t.sqrt());
    let bs_vega_per_point =
        100.0 * VOL_SPOT * (-VOL_DIV * t).exp() * norm_pdf(d1) * t.sqrt() * 0.01;

    let positions = vec![("opt-pos".to_string(), &option as &dyn Instrument, 1.0)];
    let factors = vec![equity_vol_factor()];
    let delta_matrix = DeltaBasedEngine::new(BumpSizeConfig::default()).compute_sensitivities(
        &positions,
        &factors,
        &market,
        as_of,
        Currency::USD,
    )?;
    let repricing_matrix = FullRepricingEngine::new(BumpSizeConfig::default(), 5)?
        .compute_sensitivities(&positions, &factors, &market, as_of, Currency::USD)?;

    for (label, actual) in [
        ("delta engine", delta_matrix.delta(0, 0)),
        ("full repricing", repricing_matrix.delta(0, 0)),
    ] {
        for (reference_label, reference) in [("Vega metric", vega), ("BS vega", bs_vega_per_point)]
        {
            assert!(
                (actual - reference).abs() <= 1e-3 * reference.abs(),
                "{label} vol-factor delta {actual} should match {reference_label} {reference}"
            );
        }
    }
    Ok(())
}
