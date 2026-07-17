//! Scenario application for portfolios.
//!
//! Applies [`ScenarioSpec`](finstack_quant_scenarios::spec::ScenarioSpec) to a cloned
//! portfolio and market context, then optionally re-values with the stressed data.

use crate::error::{Error, Result};
use crate::portfolio::Portfolio;
use finstack_quant_core::market_data::context::MarketContext;
use finstack_quant_scenarios::engine::{ApplicationReport, ExecutionContext, ScenarioEngine};
use finstack_quant_scenarios::spec::ScenarioSpec;
use finstack_quant_valuations::instruments::Instrument;
use std::sync::Arc;

/// JSON envelope returned by scenario-and-revalue binding surfaces.
#[derive(Debug, Clone, serde::Serialize)]
pub struct ScenarioRevalueEnvelope {
    /// Stressed portfolio valuation.
    pub valuation: crate::valuation::PortfolioValuation,
    /// Scenario application report.
    pub report: ApplicationReport,
}

/// Apply a scenario to a portfolio.
///
/// This function:
/// 1. Clones the portfolio (scenarios create modified copies)
/// 2. Extracts instruments into a mutable vector for the scenario engine
/// 3. Applies the scenario using the engine
/// 4. Returns the modified portfolio and market data
///
/// # Aliasing contract
///
/// The input `portfolio` and `market` references are **never mutated**: the
/// function works on owned clones. The values returned in the result tuple
/// are **fresh `Portfolio` and `MarketContext` instances** distinct from the
/// inputs — re-using the original `market` after the call therefore continues
/// to see pre-scenario data, and re-using the returned `MarketContext` sees
/// post-scenario data. Callers that want to chain multiple scenario passes
/// must thread the *returned* market into the next call; threading the
/// original is silently a no-op for stacked scenarios.
///
/// # Arguments
///
/// * `portfolio` - Portfolio to clone and mutate within the scenario engine.
/// * `scenario` - Scenario specification describing desired transformations.
/// * `market` - Market data context subject to the scenario operations.
///
/// # Returns
///
/// [`Result`] containing the modified portfolio, the modified market data
/// (a fresh clone — see "Aliasing contract" above), and the application report.
///
/// # Errors
///
/// Returns [`Error::ScenarioError`] when the scenario engine reports a failure.
///
/// # Examples
///
/// ```ignore
/// use finstack_quant_core::market_data::context::MarketContext;
/// use finstack_quant_portfolio::scenarios::apply_and_revalue;
/// use finstack_quant_scenarios::spec::ScenarioSpec;
///
/// # fn main() -> finstack_quant_portfolio::Result<()> {
/// # let portfolio: finstack_quant_portfolio::Portfolio = unimplemented!("Provide a portfolio");
/// # let market: MarketContext = unimplemented!("Provide market data");
/// # let scenario: ScenarioSpec = unimplemented!("Provide a scenario");
/// let (_valuation, report) = apply_and_revalue(&portfolio, &scenario, &market, &Default::default())?;
/// println!("Applied {} operations", report.operations_applied);
/// # Ok(())
/// # }
/// ```
pub(crate) fn apply_scenario(
    portfolio: &Portfolio,
    scenario: &ScenarioSpec,
    market: &MarketContext,
) -> Result<(Portfolio, MarketContext, ApplicationReport)> {
    let mut market_copy = market.clone();
    let mut portfolio_copy = portfolio.clone();

    // Extract instruments into a mutable vector
    let mut instruments: Vec<Box<dyn Instrument>> = portfolio_copy
        .positions
        .iter()
        .map(|pos| {
            // Clone the instrument via its Arc
            pos.instrument.clone_box()
        })
        .collect();

    // Build execution context
    let mut ctx = ExecutionContext {
        market: &mut market_copy,
        model: None,
        instruments: Some(&mut instruments),
        rate_bindings: None,
        calendar: None,
        as_of: portfolio.as_of,
    };

    // Apply scenario
    let engine = ScenarioEngine::default();
    let report = engine
        .apply(scenario, &mut ctx)
        .map_err(|e| Error::ScenarioError(e.to_string()))?;

    replace_portfolio_instruments(&mut portfolio_copy, instruments)?;

    Ok((portfolio_copy, market_copy, report))
}

fn replace_portfolio_instruments(
    portfolio: &mut Portfolio,
    instruments: Vec<Box<dyn Instrument>>,
) -> Result<()> {
    let position_count = portfolio.positions.len();
    let instrument_count = instruments.len();
    if position_count != instrument_count {
        return Err(Error::ScenarioError(format!(
            "MO-16: scenario engine returned {instrument_count} instruments for {position_count} portfolio positions"
        )));
    }

    // Update portfolio positions with modified instruments (move boxes into `Arc`, no extra clone).
    for (position, modified_inst) in portfolio.positions.iter_mut().zip(instruments.into_iter()) {
        position.instrument = Arc::from(modified_inst);
    }

    Ok(())
}

/// Apply a scenario and re-value the portfolio.
///
/// Convenience function that applies a scenario and immediately
/// re-values the portfolio with the modified market data.
///
/// # Arguments
///
/// * `portfolio` - Original portfolio used as the base case.
/// * `scenario` - Scenario specification to apply.
/// * `market` - Market data context to mutate.
/// * `config` - Configuration forwarded to [`value_portfolio`](crate::valuation::value_portfolio).
///
/// # Returns
///
/// [`Result`] containing the re-valued [`PortfolioValuation`](crate::valuation::PortfolioValuation)
/// along with the scenario [`ApplicationReport`].
///
/// # Errors
///
/// Propagates errors from the internal `apply_scenario` helper and [`value_portfolio`](crate::valuation::value_portfolio).
///
/// # Examples
///
/// ```ignore
/// use finstack_quant_core::config::FinstackConfig;
/// use finstack_quant_core::market_data::context::MarketContext;
/// use finstack_quant_portfolio::scenarios::apply_and_revalue;
/// use finstack_quant_scenarios::spec::ScenarioSpec;
///
/// # fn main() -> finstack_quant_portfolio::Result<()> {
/// # let portfolio: finstack_quant_portfolio::Portfolio = unimplemented!("Provide a portfolio");
/// # let market: MarketContext = unimplemented!("Provide market data");
/// # let scenario: ScenarioSpec = unimplemented!("Provide a scenario");
/// let config = FinstackConfig::default();
/// let (valuation, _report) = apply_and_revalue(&portfolio, &scenario, &market, &config)?;
/// println!("Stressed total: {}", valuation.total_base_ccy);
/// # Ok(())
/// # }
/// ```
pub fn apply_and_revalue(
    portfolio: &Portfolio,
    scenario: &ScenarioSpec,
    market: &MarketContext,
    config: &finstack_quant_core::config::FinstackConfig,
) -> Result<(crate::valuation::PortfolioValuation, ApplicationReport)> {
    let (modified_portfolio, modified_market, report) =
        apply_scenario(portfolio, scenario, market)?;

    let valuation = crate::valuation::value_portfolio(
        &modified_portfolio,
        &modified_market,
        config,
        &Default::default(),
    )?;

    Ok((valuation, report))
}

/// Apply a scenario and return the canonical JSON envelope shape.
///
/// # Errors
///
/// Returns any scenario-application or valuation error raised by
/// [`apply_and_revalue`].
///
/// # Arguments
///
/// * `portfolio` - Base portfolio whose positions and base currency are
///   copied before scenario operations are applied.
/// * `scenario` - Ordered shock and transformation specification to apply.
/// * `market` - Unshocked market snapshot used as the scenario-application
///   source and subsequent valuation context.
/// * `config` - Library valuation configuration, including market-data and
///   convention resolution policy.
pub fn apply_and_revalue_envelope(
    portfolio: &Portfolio,
    scenario: &ScenarioSpec,
    market: &MarketContext,
    config: &finstack_quant_core::config::FinstackConfig,
) -> Result<ScenarioRevalueEnvelope> {
    let (valuation, report) = apply_and_revalue(portfolio, scenario, market, config)?;
    Ok(ScenarioRevalueEnvelope { valuation, report })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::builder::PortfolioBuilder;
    use crate::position::{Position, PositionUnit};
    use crate::test_utils::build_test_market;
    use crate::types::Entity;
    use finstack_quant_core::config::FinstackConfig;
    use finstack_quant_core::currency::Currency;
    use finstack_quant_core::money::Money;
    use finstack_quant_scenarios::spec::{CurveKind, OperationSpec};
    use finstack_quant_valuations::instruments::rates::deposit::Deposit;
    use std::sync::Arc;
    use time::macros::date;

    #[test]
    fn test_apply_scenario_basic() {
        let as_of = date!(2024 - 01 - 01);

        let deposit = Deposit::builder()
            .id("DEP_1M".into())
            .notional(Money::new(1_000_000.0, Currency::USD))
            .start_date(as_of)
            .maturity(date!(2024 - 02 - 01))
            .day_count(finstack_quant_core::dates::DayCount::Act360)
            .discount_curve_id("USD".into())
            .quote_rate_opt(Some(
                rust_decimal::Decimal::try_from(0.045).expect("valid literal"),
            ))
            .build()
            .expect("test should succeed");

        let position = Position::new(
            "POS_001",
            "ENTITY_A",
            "DEP_1M",
            Arc::new(deposit),
            1.0,
            PositionUnit::Units,
        )
        .expect("test should succeed");

        let portfolio = PortfolioBuilder::new("TEST")
            .base_ccy(Currency::USD)
            .as_of(as_of)
            .entity(Entity::new("ENTITY_A"))
            .position(position)
            .build()
            .expect("test should succeed");

        let market = build_test_market();

        let scenario = ScenarioSpec {
            id: "test_scenario".to_string(),
            name: Some("Test Scenario".to_string()),
            description: None,
            operations: vec![OperationSpec::CurveParallelBp {
                curve_kind: CurveKind::Discount,
                curve_id: "USD".into(),
                discount_curve_id: None,
                bp: 50.0,
            }],
            priority: 0,
            resolution_mode: Default::default(),
        };

        let result = apply_scenario(&portfolio, &scenario, &market);
        assert!(result.is_ok());

        let (_modified_portfolio, _modified_market, report) = result.expect("test should succeed");
        assert!(report.operations_applied > 0);
    }

    #[test]
    fn test_apply_and_revalue() {
        let as_of = date!(2024 - 01 - 01);

        let deposit = Deposit::builder()
            .id("DEP_1M".into())
            .notional(Money::new(1_000_000.0, Currency::USD))
            .start_date(as_of)
            .maturity(date!(2024 - 02 - 01))
            .day_count(finstack_quant_core::dates::DayCount::Act360)
            .discount_curve_id("USD".into())
            .quote_rate_opt(Some(
                rust_decimal::Decimal::try_from(0.045).expect("valid literal"),
            ))
            .build()
            .expect("test should succeed");

        let position = Position::new(
            "POS_001",
            "ENTITY_A",
            "DEP_1M",
            Arc::new(deposit),
            1.0,
            PositionUnit::Units,
        )
        .expect("test should succeed");

        let portfolio = PortfolioBuilder::new("TEST")
            .base_ccy(Currency::USD)
            .as_of(as_of)
            .entity(Entity::new("ENTITY_A"))
            .position(position)
            .build()
            .expect("test should succeed");

        let market = build_test_market();
        let config = FinstackConfig::default();

        let scenario = ScenarioSpec {
            id: "test_scenario".to_string(),
            name: None,
            description: None,
            operations: vec![],
            priority: 0,
            resolution_mode: Default::default(),
        };

        let result = apply_and_revalue(&portfolio, &scenario, &market, &config);
        assert!(result.is_ok());
    }

    #[test]
    fn mo16_replace_portfolio_instruments_rejects_length_mismatch() {
        let as_of = date!(2024 - 01 - 01);

        let deposit = Deposit::builder()
            .id("DEP_1M".into())
            .notional(Money::new(1_000_000.0, Currency::USD))
            .start_date(as_of)
            .maturity(date!(2024 - 02 - 01))
            .day_count(finstack_quant_core::dates::DayCount::Act360)
            .discount_curve_id("USD".into())
            .quote_rate_opt(Some(
                rust_decimal::Decimal::try_from(0.045).expect("valid literal"),
            ))
            .build()
            .expect("test should succeed");

        let position = Position::new(
            "POS_001",
            "ENTITY_A",
            "DEP_1M",
            Arc::new(deposit),
            1.0,
            PositionUnit::Units,
        )
        .expect("test should succeed");

        let mut portfolio = PortfolioBuilder::new("TEST")
            .base_ccy(Currency::USD)
            .as_of(as_of)
            .entity(Entity::new("ENTITY_A"))
            .position(position)
            .build()
            .expect("test should succeed");

        let err = replace_portfolio_instruments(&mut portfolio, Vec::new())
            .expect_err("MO-16: mismatched scenario result length must fail");
        assert!(
            matches!(err, Error::ScenarioError(_)),
            "unexpected error: {err}"
        );
    }
}
