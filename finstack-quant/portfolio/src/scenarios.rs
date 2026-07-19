//! Scenario application for portfolios.
//!
//! Applies [`ScenarioSpec`](finstack_quant_scenarios::spec::ScenarioSpec) to a
//! cloned market context and a borrowed-or-owned portfolio state, then
//! optionally re-values with the stressed data.

use crate::error::{Error, Result};
use crate::portfolio::Portfolio;
use crate::types::PositionId;
use crate::{evaluation::PositionInvalidation, MarketFactorKey};
use finstack_quant_core::market_data::context::MarketContext;
use finstack_quant_core::money::Money;
use finstack_quant_scenarios::engine::{
    ApplicationReport, ExecutionContext, ScenarioChangeManifest, ScenarioEngine,
    ScenarioMarketTarget,
};
use finstack_quant_scenarios::spec::{CurveKind, ScenarioSpec};
use finstack_quant_valuations::instruments::{Instrument, RatesCurveKind};
use indexmap::IndexMap;
use std::borrow::Cow;
use std::sync::Arc;

const SCENARIO_BATCH_MAX_ACTIVE_STATES: usize = 8;

/// JSON envelope returned by scenario-and-revalue binding surfaces.
#[derive(Debug, Clone, serde::Serialize)]
pub struct ScenarioRevalueEnvelope {
    /// Stressed portfolio valuation.
    pub valuation: crate::valuation::PortfolioValuation,
    /// Scenario application report.
    pub report: ApplicationReport,
}

struct AppliedScenarioState<'a> {
    portfolio: Cow<'a, Portfolio>,
    market: MarketContext,
    as_of: finstack_quant_core::dates::Date,
    report: ApplicationReport,
}

fn selective_invalidation(
    portfolio: &Portfolio,
    changes: &ScenarioChangeManifest,
) -> Option<PositionInvalidation> {
    if changes.as_of_changed
        || changes.portfolio_shape_changed
        || changes.all_dirty
        || portfolio.dependency_index().indexed_position_count() != portfolio.positions.len()
        || changes
            .changed_instrument_indices
            .iter()
            .any(|index| *index >= portfolio.positions.len())
    {
        return None;
    }

    let mut changed_factors = Vec::with_capacity(changes.market_targets.len());
    let mut refresh_base_currency = false;
    for target in &changes.market_targets {
        let key = match target {
            ScenarioMarketTarget::Curve {
                curve_kind,
                curve_id,
            } => {
                let kind = match curve_kind {
                    CurveKind::Discount => RatesCurveKind::Discount,
                    CurveKind::Forward | CurveKind::Commodity => RatesCurveKind::Forward,
                    CurveKind::ParCDS => RatesCurveKind::Credit,
                    CurveKind::Inflation => RatesCurveKind::Inflation,
                };
                MarketFactorKey::curve(curve_id.clone(), kind)
            }
            ScenarioMarketTarget::VolSurface { surface_id, .. } => {
                MarketFactorKey::vol_surface(surface_id.as_str())
            }
            ScenarioMarketTarget::EquityPrice { price_id } => {
                MarketFactorKey::spot(price_id.as_str())
            }
            ScenarioMarketTarget::Fx { base, quote } => {
                refresh_base_currency = true;
                MarketFactorKey::fx(*base, *quote)
            }
            ScenarioMarketTarget::VolatilityIndex { .. }
            | ScenarioMarketTarget::BaseCorrelation { .. } => return None,
        };
        changed_factors.push(key);
    }

    let mut reprice_indices = if changed_factors.is_empty() {
        Vec::new()
    } else {
        portfolio
            .dependency_index()
            .affected_positions(&changed_factors)
    };
    reprice_indices.extend(changes.changed_instrument_indices.iter().copied());

    let invalidation = PositionInvalidation::new(reprice_indices, refresh_base_currency);
    Some(if changes.changed_instrument_indices.is_empty() {
        invalidation
    } else {
        invalidation.with_authoritative_portfolio_change()
    })
}

/// Apply a scenario to a portfolio.
///
/// This function:
/// 1. Borrows the portfolio for market-only scenarios
/// 2. Clones instruments only when the scenario requires instrument access
/// 3. Applies the scenario using the engine
/// 4. Owns a modified portfolio only when instruments changed
///
/// # Aliasing contract
///
/// The input `portfolio` and `market` references are **never mutated**.
/// Market-only scenarios borrow the portfolio and own only the cloned,
/// stressed market. Instrument-mutating scenarios additionally own a cloned
/// portfolio with rebuilt indexes. The returned effective date comes from the
/// scenario execution context, including time rolls.
///
/// # Arguments
///
/// * `portfolio` - Portfolio to borrow or clone according to scenario effects.
/// * `scenario` - Scenario specification describing desired transformations.
/// * `market` - Market data context subject to the scenario operations.
///
/// # Returns
///
/// [`Result`] containing the prepared portfolio/market/date state and the
/// application report.
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
fn apply_scenario<'a>(
    portfolio: &'a Portfolio,
    scenario: &ScenarioSpec,
    market: &MarketContext,
) -> Result<AppliedScenarioState<'a>> {
    let mut market_copy = market.clone();
    let mut instruments = scenario.requires_instruments().then(|| {
        portfolio
            .positions
            .iter()
            .map(|position| position.instrument.clone_box())
            .collect::<Vec<Box<dyn Instrument>>>()
    });

    // Build execution context
    let mut ctx = ExecutionContext {
        market: &mut market_copy,
        model: None,
        instruments: instruments.as_mut(),
        rate_bindings: None,
        calendar: None,
        as_of: portfolio.as_of,
    };

    // Apply scenario
    let engine = ScenarioEngine::default();
    let report = engine
        .apply(scenario, &mut ctx)
        .map_err(|e| Error::ScenarioError(e.to_string()))?;
    let as_of = ctx.as_of;
    drop(ctx);

    let portfolio = if scenario.mutates_instruments() {
        let mut portfolio_copy = portfolio.clone();
        let instruments = instruments.ok_or_else(|| {
            Error::ScenarioError(
                "scenario classified as instrument-mutating without instrument state".to_string(),
            )
        })?;
        replace_portfolio_instruments(&mut portfolio_copy, instruments)?;
        portfolio_copy.as_of = as_of;
        Cow::Owned(portfolio_copy)
    } else {
        Cow::Borrowed(portfolio)
    };

    Ok(AppliedScenarioState {
        portfolio,
        market: market_copy,
        as_of,
        report,
    })
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
    portfolio.rebuild_index();

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
    let AppliedScenarioState {
        portfolio,
        market,
        as_of,
        report,
    } = apply_scenario(portfolio, scenario, market)?;
    let mut plan = crate::evaluation::PortfolioEvaluationPlan::new(config);
    let market_state = plan.register_owned_market(market, as_of);
    let portfolio_state = match portfolio {
        Cow::Borrowed(portfolio) => plan.register_portfolio(portfolio),
        Cow::Owned(portfolio) => plan.register_owned_portfolio(portfolio),
    };
    let profile = crate::evaluation::EvaluationProfile::from_options(&Default::default());
    let evaluation = plan.register_evaluation(market_state, portfolio_state, profile)?;
    let valuation = plan.execute().into_valuation(evaluation)?;

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

/// One ordered result from [`scenario_pnl_batch`].
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct ScenarioPnlBatchItem {
    /// Identifier copied from the input scenario.
    pub scenario_id: String,
    /// Scenario-attributable portfolio P&L.
    pub pnl: ScenarioPnl,
    /// Application provenance and warnings for this scenario.
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
/// Registers an unstressed state and the scenario-applied state with the
/// request-scoped portfolio evaluation engine, requests PV only for both, and
/// reports the difference per position and in total. This is the standard
/// "what does this shock cost me" measure a risk desk runs before sizing a
/// hedge. Unlike [`apply_and_revalue`], this function deliberately does not
/// compute the standard risk set.
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
/// * `config` - Configuration used by both registered PV-only jobs, so the
///   base and stressed legs share one rounding and convention policy while
///   retaining separate immutable market states and calibration caches.
///
/// # Returns
///
/// [`Result`] containing the [`ScenarioPnl`] ladder together with the scenario
/// [`ApplicationReport`], so the P&L is never separated from its provenance.
///
/// # Errors
///
/// Returns [`Error::ScenarioError`] when the scenario engine fails, any
/// valuation error raised by the shared evaluation executor on either leg,
/// and [`Error::Core`] when a position's stressed and base values carry
/// different currencies.
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
    let mut results =
        scenario_pnl_batch(portfolio, std::slice::from_ref(scenario), market, config)?;
    let result = results
        .pop()
        .ok_or_else(|| Error::ScenarioError("scenario batch returned no result".to_string()))?;
    Ok((result.pnl, result.report))
}

/// Compute ordered P&L results for multiple scenarios while sharing one base valuation.
///
/// The base portfolio is valued once with a PV-only profile. Stressed states
/// are evaluated in bounded waves, so retained memory is proportional to the
/// active wave plus the returned P&L ladders rather than the full
/// positions-by-scenarios valuation cube.
///
/// # Arguments
///
/// * `portfolio` - Portfolio to value for the shared base and every scenario.
/// * `scenarios` - Ordered scenario specifications. An empty slice returns an
///   empty vector without performing valuation.
/// * `market` - Unstressed market snapshot used for the shared base and as the
///   source for each independently applied scenario.
/// * `config` - Valuation configuration used by every executor state.
///
/// # Returns
///
/// Ordered results matching `scenarios`, including each scenario identifier,
/// P&L ladder, and application report.
///
/// # Errors
///
/// Returns the first scenario-application or valuation error in input order,
/// or a currency error while differencing base and stressed values.
pub fn scenario_pnl_batch(
    portfolio: &Portfolio,
    scenarios: &[ScenarioSpec],
    market: &MarketContext,
    config: &finstack_quant_core::config::FinstackConfig,
) -> Result<Vec<ScenarioPnlBatchItem>> {
    if scenarios.is_empty() {
        return Ok(Vec::new());
    }

    let options = crate::valuation::PortfolioValuationOptions {
        strict_risk: false,
        metrics: crate::valuation::RequestedMetrics::Only(Vec::new()),
    };
    let base = crate::valuation::value_portfolio(portfolio, market, config, &options)?;
    let profile = crate::evaluation::EvaluationProfile::from_options(&options);
    let mut results = Vec::with_capacity(scenarios.len());

    for wave in scenarios.chunks(SCENARIO_BATCH_MAX_ACTIVE_STATES) {
        let mut applied = Vec::with_capacity(wave.len());
        let mut application_error = None;
        for scenario in wave {
            match apply_scenario(portfolio, scenario, market) {
                Ok(state) => applied.push((scenario.id.clone(), state)),
                Err(error) => {
                    application_error = Some(error);
                    break;
                }
            }
        }

        let mut plan = crate::evaluation::PortfolioEvaluationPlan::new(config);
        let shared_portfolio = plan.register_portfolio(portfolio);
        let mut jobs = Vec::with_capacity(applied.len());
        for (
            scenario_id,
            AppliedScenarioState {
                portfolio: stressed_portfolio,
                market: stressed_market,
                as_of,
                report,
            },
        ) in applied
        {
            let invalidation = selective_invalidation(stressed_portfolio.as_ref(), &report.changes);
            let market_state = plan.register_owned_market(stressed_market, as_of);
            let portfolio_state = match stressed_portfolio {
                Cow::Borrowed(_) => shared_portfolio,
                Cow::Owned(portfolio) => plan.register_owned_portfolio(portfolio),
            };
            let evaluation = if let Some(invalidation) = invalidation {
                plan.register_selective_evaluation(
                    market_state,
                    portfolio_state,
                    profile.clone(),
                    crate::evaluation::ParentResult::External(&base),
                    invalidation,
                )?
            } else {
                plan.register_evaluation(market_state, portfolio_state, profile.clone())?
            };
            jobs.push((scenario_id, report, evaluation));
        }

        let outcome = plan.execute();
        for (scenario_id, report, evaluation) in jobs {
            let stressed = outcome.get(evaluation)?;
            results.push(ScenarioPnlBatchItem {
                scenario_id,
                pnl: diff_valuations(&base, stressed)?,
                report,
            });
        }
        if let Some(error) = application_error {
            return Err(error);
        }
    }

    Ok(results)
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
    use finstack_quant_valuations::instruments::{Attributes, Instrument};
    use finstack_quant_valuations::pricer::InstrumentType;
    use std::any::Any;
    use std::sync::{
        atomic::{AtomicUsize, Ordering},
        Arc,
    };
    use time::macros::date;

    #[derive(Clone)]
    struct CountingPvInstrument {
        id: String,
        calls: Arc<AtomicUsize>,
        attributes: Attributes,
    }

    finstack_quant_valuations::impl_empty_cashflow_provider!(
        CountingPvInstrument,
        finstack_quant_cashflows::builder::CashflowRepresentation::NoResidual
    );

    impl Instrument for CountingPvInstrument {
        fn id(&self) -> &str {
            &self.id
        }

        fn key(&self) -> InstrumentType {
            InstrumentType::Basket
        }

        fn as_any(&self) -> &dyn Any {
            self
        }

        fn as_any_mut(&mut self) -> &mut dyn Any {
            self
        }

        fn attributes(&self) -> &Attributes {
            &self.attributes
        }

        fn attributes_mut(&mut self) -> &mut Attributes {
            &mut self.attributes
        }

        fn clone_box(&self) -> Box<dyn Instrument> {
            Box::new(self.clone())
        }

        fn base_value(
            &self,
            _market: &MarketContext,
            _as_of: finstack_quant_core::dates::Date,
        ) -> finstack_quant_core::Result<Money> {
            self.calls.fetch_add(1, Ordering::SeqCst);
            Ok(Money::new(100.0, Currency::USD))
        }
    }

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

        let applied = result.expect("test should succeed");
        assert!(applied.report.operations_applied > 0);
        assert!(matches!(applied.portfolio, Cow::Borrowed(_)));
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
    fn scenario_batch_values_the_base_once_and_reuses_no_op_children() {
        let calls = Arc::new(AtomicUsize::new(0));
        let instrument = CountingPvInstrument {
            id: "COUNTING_PV".to_string(),
            calls: Arc::clone(&calls),
            attributes: Attributes::new(),
        };
        let position = Position::new(
            "POS_COUNT",
            "ENTITY_A",
            "COUNTING_PV",
            Arc::new(instrument),
            1.0,
            PositionUnit::Units,
        )
        .expect("counting position");
        let portfolio = PortfolioBuilder::new("COUNTING_SCENARIO_BATCH")
            .base_ccy(Currency::USD)
            .as_of(date!(2024 - 01 - 01))
            .entity(Entity::new("ENTITY_A"))
            .position(position)
            .build()
            .expect("counting portfolio");
        let scenarios: Vec<ScenarioSpec> = (0..10)
            .map(|index| ScenarioSpec {
                id: format!("NO_OP_{index}"),
                name: None,
                description: None,
                operations: Vec::new(),
                priority: 0,
                resolution_mode: Default::default(),
            })
            .collect();

        let results = scenario_pnl_batch(
            &portfolio,
            &scenarios,
            &build_test_market(),
            &FinstackConfig::default(),
        )
        .expect("no-op batch");

        assert_eq!(results.len(), scenarios.len());
        assert_eq!(
            calls.load(Ordering::SeqCst),
            1,
            "the shared base should be the only PV call for no-op children"
        );
        assert!(results.iter().all(|item| item.pnl.total.amount() == 0.0));
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
            provenance: None,
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
