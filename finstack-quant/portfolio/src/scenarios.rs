//! Scenario application for portfolios.
//!
//! Applies [`ScenarioSpec`](finstack_quant_scenarios::spec::ScenarioSpec) to a cloned
//! portfolio and market context, then optionally re-values with the stressed data.

use crate::error::{Error, Result};
use crate::portfolio::Portfolio;
use crate::types::PositionId;
use finstack_quant_core::market_data::context::MarketContext;
use finstack_quant_core::money::Money;
use finstack_quant_scenarios::engine::{ApplicationReport, ExecutionContext, ScenarioEngine};
use finstack_quant_scenarios::spec::ScenarioSpec;
use finstack_quant_valuations::instruments::Instrument;
use indexmap::IndexMap;
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

/// Scenario-attributable profit and loss, in the portfolio base currency.
///
/// Produced by [`scenario_pnl`] as the difference between the stressed and the
/// unstressed [`PortfolioValuation`](crate::valuation::PortfolioValuation).
/// Every amount is a [`Money`] in the portfolio's base currency — the
/// computation never leaves `Money`, so the Decimal rounding contract of
/// [`crate::valuation::value_portfolio`] carries through to the P&L unchanged.
///
/// # Reconciliation
///
/// `by_position` sums to `total` in the base currency: both sides are derived
/// from the same per-position `value_base` amounts, so a desk-style
/// "does the drill-down foot to the headline" check passes.
///
/// # Ordering
///
/// `by_position` is deterministically ordered: positions present in the
/// stressed valuation come first, in stressed-valuation order, followed by any
/// position that only exists in the base valuation, in base-valuation order.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct ScenarioPnl {
    /// Total scenario P&L in the portfolio base currency
    /// (`stressed.total_base_ccy - base.total_base_ccy`).
    pub total: Money,

    /// Per-position scenario P&L in the portfolio base currency.
    pub by_position: IndexMap<PositionId, Money>,
}

/// JSON envelope returned by scenario-P&L binding surfaces.
///
/// Mirrors [`ScenarioRevalueEnvelope`] so callers keep scenario provenance
/// (which operations were applied, which were skipped) alongside the P&L.
#[derive(Debug, Clone, serde::Serialize)]
pub struct ScenarioPnlEnvelope {
    /// Scenario-attributable profit and loss.
    pub pnl: ScenarioPnl,
    /// Scenario application report.
    pub report: ApplicationReport,
}

/// Difference two portfolio valuations into a per-position P&L ladder.
///
/// Positions present on only one side are **zero-filled** against the missing
/// side (see [`scenario_pnl`] for the rationale and the caller-visible
/// contract).
fn diff_valuations(
    base: &crate::valuation::PortfolioValuation,
    stressed: &crate::valuation::PortfolioValuation,
) -> Result<ScenarioPnl> {
    let mut by_position: IndexMap<PositionId, Money> =
        IndexMap::with_capacity(stressed.position_values.len());

    for (position_id, stressed_value) in &stressed.position_values {
        let stressed_base = stressed_value.value_base;
        let delta = match base.position_values.get(position_id) {
            Some(base_value) => stressed_base.checked_sub(base_value.value_base)?,
            // Position added by the scenario: the base leg contributes nothing.
            None => stressed_base,
        };
        by_position.insert(position_id.clone(), delta);
    }

    for (position_id, base_value) in &base.position_values {
        if stressed.position_values.contains_key(position_id) {
            continue;
        }
        // Position removed by the scenario: the stressed leg contributes nothing,
        // so the P&L is the negated base value.
        let zero = Money::new(0.0, base_value.value_base.currency());
        by_position.insert(
            position_id.clone(),
            zero.checked_sub(base_value.value_base)?,
        );
    }

    let total = stressed.total_base_ccy.checked_sub(base.total_base_ccy)?;

    Ok(ScenarioPnl { total, by_position })
}

/// Compute the profit and loss attributable to a scenario.
///
/// Values the portfolio twice — once against the unstressed `market` and once
/// against the scenario-stressed market produced by [`apply_and_revalue`] —
/// and reports the difference per position and in total. This is the standard
/// "what does this shock cost me" measure a risk desk runs before sizing a
/// hedge.
///
/// The arithmetic stays in [`Money`] end to end (Decimal-backed): position
/// deltas are currency-checked subtractions of the base-currency position
/// values, never f64 round-trips. A currency mismatch is therefore an error,
/// not a silent coercion.
///
/// # Positions present on only one side
///
/// The scenario engine may add or remove positions. Such positions are
/// **zero-filled against the missing side**, not rejected:
///
/// - added by the scenario → P&L = stressed value (full gain/loss of the new leg);
/// - removed by the scenario → P&L = negated base value (the leg's value drops out).
///
/// Zero-filling is chosen over an error because it preserves the reconciliation
/// identity `sum(by_position) == total`: a portfolio whose composition changes
/// still foots. Callers who require a fixed position set should compare
/// `by_position.len()` against their expected position count.
///
/// # Arguments
///
/// * `portfolio` - Portfolio to value on both the unstressed and stressed legs.
/// * `scenario` - Scenario specification describing the shocks to apply.
/// * `market` - Unstressed market snapshot; used as the base leg and as the
///   source the scenario operations are applied to.
/// * `config` - Configuration forwarded to
///   [`value_portfolio`](crate::valuation::value_portfolio) on both legs, so
///   the two valuations share one rounding and convention policy.
///
/// # Returns
///
/// [`Result`] containing the [`ScenarioPnl`] ladder together with the scenario
/// [`ApplicationReport`], so the P&L is never separated from its provenance.
///
/// # Errors
///
/// Returns [`Error::ScenarioError`] when the scenario engine fails, any
/// valuation error raised by [`value_portfolio`](crate::valuation::value_portfolio)
/// on either leg, and [`Error::Core`] when a position's stressed and base
/// values carry different currencies.
///
/// # Examples
///
/// ```ignore
/// use finstack_quant_core::config::FinstackConfig;
/// use finstack_quant_core::market_data::context::MarketContext;
/// use finstack_quant_portfolio::scenarios::scenario_pnl;
/// use finstack_quant_scenarios::spec::ScenarioSpec;
///
/// # fn main() -> finstack_quant_portfolio::Result<()> {
/// # let portfolio: finstack_quant_portfolio::Portfolio = unimplemented!("Provide a portfolio");
/// # let market: MarketContext = unimplemented!("Provide market data");
/// # let scenario: ScenarioSpec = unimplemented!("Provide a scenario");
/// let (pnl, report) = scenario_pnl(&portfolio, &scenario, &market, &FinstackConfig::default())?;
/// println!("Scenario P&L: {} over {} operations", pnl.total, report.operations_applied);
/// # Ok(())
/// # }
/// ```
pub fn scenario_pnl(
    portfolio: &Portfolio,
    scenario: &ScenarioSpec,
    market: &MarketContext,
    config: &finstack_quant_core::config::FinstackConfig,
) -> Result<(ScenarioPnl, ApplicationReport)> {
    let base = crate::valuation::value_portfolio(portfolio, market, config, &Default::default())?;
    let (stressed, report) = apply_and_revalue(portfolio, scenario, market, config)?;
    let pnl = diff_valuations(&base, &stressed)?;
    Ok((pnl, report))
}

/// Compute scenario P&L and return the canonical JSON envelope shape.
///
/// # Arguments
///
/// * `portfolio` - Portfolio to value on both the unstressed and stressed legs.
/// * `scenario` - Ordered shock and transformation specification to apply.
/// * `market` - Unshocked market snapshot used for the base leg and as the
///   scenario-application source.
/// * `config` - Library valuation configuration, including market-data and
///   convention resolution policy.
///
/// # Errors
///
/// Returns any scenario-application, valuation, or currency-mismatch error
/// raised by [`scenario_pnl`].
pub fn scenario_pnl_envelope(
    portfolio: &Portfolio,
    scenario: &ScenarioSpec,
    market: &MarketContext,
    config: &finstack_quant_core::config::FinstackConfig,
) -> Result<ScenarioPnlEnvelope> {
    let (pnl, report) = scenario_pnl(portfolio, scenario, market, config)?;
    Ok(ScenarioPnlEnvelope { pnl, report })
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

    // -----------------------------------------------------------------------
    // scenario_pnl
    // -----------------------------------------------------------------------

    /// Single-position USD portfolio used by the scenario-P&L tests.
    fn single_position_portfolio() -> Portfolio {
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

        PortfolioBuilder::new("TEST")
            .base_ccy(Currency::USD)
            .as_of(as_of)
            .entity(Entity::new("ENTITY_A"))
            .position(position)
            .build()
            .expect("test should succeed")
    }

    fn scenario_with(operations: Vec<OperationSpec>) -> ScenarioSpec {
        ScenarioSpec {
            id: "pnl_scenario".to_string(),
            name: None,
            description: None,
            operations,
            priority: 0,
            resolution_mode: Default::default(),
        }
    }

    #[test]
    fn scenario_pnl_no_op_scenario_is_exactly_zero() {
        let portfolio = single_position_portfolio();
        let market = build_test_market();
        let config = FinstackConfig::default();

        let (pnl, _report) = scenario_pnl(&portfolio, &scenario_with(Vec::new()), &market, &config)
            .expect("test should succeed");

        assert_eq!(pnl.total.amount(), 0.0);
        assert_eq!(pnl.by_position.len(), 1);
        for (position_id, delta) in &pnl.by_position {
            assert_eq!(delta.amount(), 0.0, "position '{position_id}' must be flat");
            assert_eq!(delta.currency(), Currency::USD);
        }
    }

    #[test]
    fn scenario_pnl_by_position_reconciles_to_total() {
        let portfolio = single_position_portfolio();
        let market = build_test_market();
        let config = FinstackConfig::default();

        let scenario = scenario_with(vec![OperationSpec::CurveParallelBp {
            curve_kind: CurveKind::Discount,
            curve_id: "USD".into(),
            discount_curve_id: None,
            bp: 50.0,
        }]);

        let (pnl, report) =
            scenario_pnl(&portfolio, &scenario, &market, &config).expect("test should succeed");

        assert!(report.operations_applied > 0);

        let drilldown = pnl
            .by_position
            .values()
            .try_fold(Money::new(0.0, Currency::USD), |acc, delta| {
                acc.checked_add(*delta)
            })
            .expect("all deltas share the base currency");

        assert!(
            (drilldown.amount() - pnl.total.amount()).abs() < 1e-6,
            "drill-down {} must foot to total {}",
            drilldown.amount(),
            pnl.total.amount()
        );
    }

    #[test]
    fn scenario_pnl_envelope_serializes_for_binding_surfaces() {
        let portfolio = single_position_portfolio();
        let market = build_test_market();

        let envelope = scenario_pnl_envelope(
            &portfolio,
            &scenario_with(vec![OperationSpec::CurveParallelBp {
                curve_kind: CurveKind::Discount,
                curve_id: "USD".into(),
                discount_curve_id: None,
                bp: 25.0,
            }]),
            &market,
            &FinstackConfig::default(),
        )
        .expect("test should succeed");

        let json = serde_json::to_string(&envelope).expect("envelope must serialize");
        assert!(json.contains("\"pnl\""), "missing pnl key: {json}");
        assert!(json.contains("\"report\""), "missing report key: {json}");
        assert!(
            json.contains("\"by_position\""),
            "missing by_position key: {json}"
        );

        // The ladder round-trips so Python/WASM callers can rehydrate it.
        let reparsed: ScenarioPnl =
            serde_json::from_str(&serde_json::to_string(&envelope.pnl).expect("pnl serializes"))
                .expect("pnl round-trips");
        assert_eq!(reparsed.by_position.len(), envelope.pnl.by_position.len());
        assert_eq!(reparsed.total.amount(), envelope.pnl.total.amount());
    }

    /// Build a one-position valuation whose only position value is `amount`.
    fn valuation_with(position_id: &str, amount: f64) -> crate::valuation::PortfolioValuation {
        let value = Money::new(amount, Currency::USD);
        let position_value = crate::valuation::PositionValue {
            position_id: PositionId::from(position_id),
            entity_id: crate::types::EntityId::from("ENTITY_A"),
            value_native: value,
            value_base: value,
            metric_scale: 1.0,
            risk_metrics_complete: true,
            risk_error: None,
            valuation_result: None,
        };
        let mut position_values = IndexMap::new();
        position_values.insert(PositionId::from(position_id), position_value);

        crate::valuation::PortfolioValuation {
            as_of: date!(2024 - 01 - 01),
            position_values,
            total_base_ccy: value,
            by_entity: IndexMap::new(),
            degraded_positions: Vec::new(),
            fx_collapse_policy: finstack_quant_core::money::fx::FxConversionPolicy::CashflowDate,
        }
    }

    #[test]
    fn diff_valuations_zero_fills_positions_added_by_the_scenario() {
        let base = valuation_with("POS_OLD", 100.0);
        let stressed = valuation_with("POS_NEW", 30.0);

        let pnl = diff_valuations(&base, &stressed).expect("test should succeed");

        // Added leg contributes its full stressed value; removed leg contributes
        // the negated base value. Stressed-side positions are ordered first.
        let deltas: Vec<(String, f64)> = pnl
            .by_position
            .iter()
            .map(|(id, money)| (id.to_string(), money.amount()))
            .collect();
        assert_eq!(
            deltas,
            vec![
                ("POS_NEW".to_string(), 30.0),
                ("POS_OLD".to_string(), -100.0)
            ]
        );

        // Reconciliation still holds across a composition change.
        assert_eq!(pnl.total.amount(), -70.0);
        assert_eq!(deltas.iter().map(|(_, v)| v).sum::<f64>(), -70.0);
    }

    #[test]
    fn diff_valuations_rejects_currency_mismatch() {
        let base = valuation_with("POS_001", 100.0);
        let mut stressed = valuation_with("POS_001", 120.0);
        let eur = Money::new(120.0, Currency::EUR);
        if let Some(pv) = stressed.position_values.get_mut("POS_001") {
            pv.value_base = eur;
        }

        let err = diff_valuations(&base, &stressed)
            .expect_err("cross-currency position deltas must not be silently coerced");
        assert!(matches!(err, Error::Core(_)), "unexpected error: {err}");
    }
}
