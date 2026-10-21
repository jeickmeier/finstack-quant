//! Historical VaR and expected shortfall through the metrics framework.

use finstack_quant_core::currency::Currency;
use finstack_quant_core::dates::{Date, DayCount};
use finstack_quant_core::market_data::context::MarketContext;
use finstack_quant_core::market_data::term_structures::DiscountCurve;
use finstack_quant_core::money::Money;
use finstack_quant_core::types::CurveId;
use finstack_quant_core::Result;
use finstack_quant_valuations::instruments::fixed_income::bond::Bond;
use finstack_quant_valuations::instruments::Instrument;
use finstack_quant_valuations::metrics::risk::var_calculator::{calculate_var, VarConfig};
use finstack_quant_valuations::metrics::risk::{
    MarketHistory, MarketScenario, RiskFactorShift, RiskFactorType,
};
use finstack_quant_valuations::metrics::MetricId;
use finstack_quant_valuations::pricer::PricingDispatch;
use std::sync::Arc;
use time::macros::date;
use time::Duration;

fn sample_as_of() -> Date {
    date!(2024 - 01 - 01)
}

fn usd_ois_curve(as_of: Date) -> Result<DiscountCurve> {
    DiscountCurve::builder("USD-OIS")
        .base_date(as_of)
        .day_count(DayCount::Act365F)
        .knots(vec![(0.0, 1.0), (5.0, 0.85), (10.0, 0.70)])
        .build()
}

fn usd_ois_market(as_of: Date) -> Result<MarketContext> {
    Ok(MarketContext::new().insert(usd_ois_curve(as_of)?))
}

/// Fixed-rate USD bond used by the VaR tests.
fn standard_bond(id: &str, as_of: Date, maturity: Date) -> Bond {
    Bond::fixed(
        id,
        Money::new(100_000.0, Currency::USD).expect("valid money fixture"),
        finstack_quant_core::types::Rate::from_decimal(0.05).expect("valid rate fixture"),
        as_of,
        maturity,
        finstack_quant_core::dates::StubKind::ShortFront,
        "USD-OIS",
    )
    .expect("standard_bond should build successfully")
}

/// Single-factor USD-OIS rate scenario.
fn usd_ois_rate_scenario(date: Date, shift: f64) -> MarketScenario {
    MarketScenario::new(
        date,
        vec![RiskFactorShift {
            factor: RiskFactorType::DiscountRate {
                curve_id: CurveId::from("USD-OIS"),
                tenor_years: 5.0,
            },
            shift,
        }],
    )
}

/// Market history from `(date, shift)` pairs on the USD-OIS 5y node.
fn history_from_rate_shifts(as_of: Date, shifts: &[(Date, f64)]) -> MarketHistory {
    let scenarios = shifts
        .iter()
        .map(|(date, shift)| usd_ois_rate_scenario(*date, *shift))
        .collect();
    MarketHistory::new(as_of, shifts.len() as u32, scenarios)
}

#[test]
fn test_hvar_via_metrics_framework() -> Result<()> {
    let as_of = sample_as_of();
    let maturity = as_of + Duration::days(365 * 5);
    let bond = standard_bond("TEST-BOND", as_of, maturity);

    // Use enough scenarios so ES != VaR at 95% (tail size >= 2).
    let mut shifts: Vec<(finstack_quant_core::dates::Date, f64)> = Vec::new();
    for i in 1..=25_i64 {
        let d = as_of - Duration::days(i);
        // Mix signs and magnitudes to ensure a non-degenerate tail.
        let shift = if i % 2 == 0 {
            0.0004 * (i as f64)
        } else {
            -0.0003 * (i as f64)
        };
        shifts.push((d, shift));
    }
    let history = Arc::new(history_from_rate_shifts(as_of, &shifts));

    let market = Arc::new(usd_ois_market(as_of)?);

    // Compute a reference result directly from the VaR engine.
    let expected = calculate_var(
        &[&bond as &dyn Instrument],
        market.as_ref(),
        history.as_ref(),
        as_of,
        &VarConfig::var_95(),
        PricingDispatch::InstrumentDefault,
        None,
    )?;

    // Calculate VaR + ES via metrics framework
    use finstack_quant_valuations::instruments::PricingOptions;
    let opts = PricingOptions::default().with_market_history(history);
    let result_ordered = bond.price_with_metrics(
        market.as_ref(),
        as_of,
        &[MetricId::HVar, MetricId::ExpectedShortfall],
        opts,
    )?;

    let var = *result_ordered
        .measures
        .get("hvar")
        .expect("VaR should be computed");
    let es = *result_ordered
        .measures
        .get("expected_shortfall")
        .expect("ES should be computed");
    assert!(
        var < 0.0,
        "VaR should be negative (losses-negative convention)"
    );
    assert!(es <= var, "ES should lie at or beyond VaR in the loss tail");
    assert!(
        (var - expected.var).abs() < 1e-10,
        "VaR mismatch: got {var}, expected {}",
        expected.var
    );
    assert!(
        (es - expected.expected_shortfall).abs() < 1e-10,
        "ES mismatch: got {es}, expected {}",
        expected.expected_shortfall
    );
    let n = *result_ordered
        .measures
        .get("hvar_num_scenarios")
        .expect("scenario count should be exposed");
    assert_eq!(n as usize, expected.num_scenarios);

    // Also verify reversed ordering doesn't break ES vs VaR wiring.
    let history2 = Arc::new(history_from_rate_shifts(as_of, &shifts));
    let opts2 = PricingOptions::default().with_market_history(history2);
    let result_reversed = bond.price_with_metrics(
        market.as_ref(),
        as_of,
        &[MetricId::ExpectedShortfall, MetricId::HVar],
        opts2,
    )?;
    let var2 = *result_reversed
        .measures
        .get("hvar")
        .expect("VaR should be computed");
    let es2 = *result_reversed
        .measures
        .get("expected_shortfall")
        .expect("ES should be computed");
    assert!((var2 - expected.var).abs() < 1e-10);
    assert!((es2 - expected.expected_shortfall).abs() < 1e-10);

    Ok(())
}

#[test]
fn request_config_applies_to_both_historical_risk_metrics_in_either_order() -> Result<()> {
    let as_of = sample_as_of();
    let mut bond = standard_bond("RISK-CONFIG", as_of, as_of + Duration::days(365 * 5));
    let config = VarConfig::var_99();
    bond.metric_pricing_overrides.var_config = Some(config.clone());
    let shifts: Vec<_> = (1..=101)
        .map(|i| (as_of - Duration::days(i), i as f64 * 0.0001))
        .collect();
    let history = Arc::new(history_from_rate_shifts(as_of, &shifts));
    let market = usd_ois_market(as_of)?;
    let expected = calculate_var(
        &[&bond as &dyn Instrument],
        &market,
        &history,
        as_of,
        &config,
        PricingDispatch::InstrumentDefault,
        None,
    )?;
    let default = calculate_var(
        &[&bond as &dyn Instrument],
        &market,
        &history,
        as_of,
        &VarConfig::var_95(),
        PricingDispatch::InstrumentDefault,
        None,
    )?;
    assert_ne!(expected.var, default.var);
    for metrics in [
        [MetricId::HVar, MetricId::ExpectedShortfall],
        [MetricId::ExpectedShortfall, MetricId::HVar],
    ] {
        let result = bond.price_with_metrics(
            &market,
            as_of,
            &metrics,
            finstack_quant_valuations::instruments::PricingOptions::default()
                .with_market_history(Arc::clone(&history)),
        )?;
        assert!((result.measures["hvar"] - expected.var).abs() < 1e-10);
        assert!(
            (result.measures["expected_shortfall"] - expected.expected_shortfall).abs() < 1e-10
        );
    }
    Ok(())
}
