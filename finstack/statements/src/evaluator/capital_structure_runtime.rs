//! Capital-structure-specific evaluator runtime helpers.

use super::{EvaluationContext, Evaluator};
use crate::error::Result;
use crate::evaluator::{DependencyGraph, EvalWarning};
use crate::types::{FinancialModelSpec, NodeId};
use finstack_core::dates::{Date, Period, PeriodId};
use finstack_core::money::Money;
use indexmap::IndexMap;
use std::collections::HashSet;
use std::sync::Arc;

type Instruments = IndexMap<String, Arc<dyn finstack_cashflows::CashflowProvider + Send + Sync>>;

impl Evaluator {
    /// Build instruments from model specifications.
    #[cfg(feature = "valuation-integration")]
    pub(crate) fn build_instruments(
        &self,
        model: &FinancialModelSpec,
    ) -> Result<Option<Instruments>> {
        use crate::capital_structure::integration;
        use finstack_cashflows::CashflowProvider;

        let cs_spec = match &model.capital_structure {
            Some(cs) => cs,
            None => return Ok(None),
        };

        let mut instruments: IndexMap<String, Arc<dyn CashflowProvider + Send + Sync>> =
            IndexMap::new();

        for debt_spec in &cs_spec.debt_instruments {
            let instrument = integration::build_any_instrument_from_spec(debt_spec)?;
            instruments.insert(debt_spec.id.clone(), instrument);
        }

        Ok(Some(instruments))
    }

    /// Build instruments from model specifications.
    #[cfg(not(feature = "valuation-integration"))]
    pub(crate) fn build_instruments(
        &self,
        model: &FinancialModelSpec,
    ) -> Result<Option<Instruments>> {
        let cs_spec = match &model.capital_structure {
            Some(cs) => cs,
            None => return Ok(None),
        };

        if cs_spec.debt_instruments.is_empty() {
            return Ok(Some(IndexMap::new()));
        }

        Err(crate::error::Error::capital_structure(
            "capital-structure debt instruments require the `valuation-integration` feature",
        ))
    }

    /// Evaluate a period with dynamic capital structure support.
    #[allow(clippy::too_many_arguments)]
    pub(crate) fn evaluate_period_dynamic(
        &mut self,
        model: &FinancialModelSpec,
        period: &Period,
        is_actual: bool,
        explicit_values_visible: bool,
        eval_order: &[crate::types::NodeId],
        node_to_column: &std::sync::Arc<IndexMap<crate::types::NodeId, usize>>,
        historical: &Arc<IndexMap<PeriodId, IndexMap<String, f64>>>,
        historical_cs: &Arc<
            IndexMap<PeriodId, crate::capital_structure::CapitalStructureCashflows>,
        >,
        market_ctx: &finstack_core::market_data::context::MarketContext,
        as_of: Date,
        instruments: &Instruments,
        cs_state: &mut crate::capital_structure::CapitalStructureState,
        cs_affected_nodes: &HashSet<NodeId>,
    ) -> Result<(
        IndexMap<String, f64>,
        Vec<EvalWarning>,
        crate::capital_structure::CapitalStructureCashflows,
    )> {
        let period_id = period.id;

        let (contractual_flows, mut contractual_warnings) =
            compute_contractual_flows(instruments, cs_state, period, market_ctx, as_of)?;

        let fx_ctx = build_fx_context(model, market_ctx, period);
        let mut cs_cashflows = build_cs_cashflows_from_contractual(&contractual_flows, period_id);
        recompute_cs_totals(&mut cs_cashflows, period_id, fx_ctx.as_ref())?;

        let mut context = EvaluationContext::new_with_history(
            period_id,
            std::sync::Arc::clone(node_to_column),
            Arc::clone(historical),
            Arc::clone(historical_cs),
        );
        context.capital_structure_cashflows = Some(cs_cashflows.clone());

        self.evaluate_nodes_in_order(
            model,
            &period_id,
            is_actual,
            explicit_values_visible,
            eval_order,
            &mut context,
            None,
            None,
            None,
        )?;

        if let Some(cs_spec) = &model.capital_structure {
            if let Some(waterfall_spec) = &cs_spec.waterfall {
                let waterfall_result = crate::capital_structure::waterfall::execute_waterfall(
                    &period_id,
                    &context,
                    waterfall_spec,
                    cs_state,
                    &contractual_flows,
                )?;

                merge_updated_flows(&mut cs_cashflows, &waterfall_result.flows, period_id);
                recompute_cs_totals(&mut cs_cashflows, period_id, fx_ctx.as_ref())?;
                if let Some(equity) = waterfall_result.equity_distribution {
                    cs_cashflows.equity_distribution.insert(period_id, equity);
                }
                contractual_warnings.extend(waterfall_result.warnings);
                context.capital_structure_cashflows = Some(cs_cashflows);
            }
        }

        if context.capital_structure_cashflows.is_some() && !cs_affected_nodes.is_empty() {
            self.evaluate_nodes_in_order(
                model,
                &period_id,
                is_actual,
                explicit_values_visible,
                eval_order,
                &mut context,
                None,
                Some(cs_affected_nodes),
                None,
            )?;
        }

        let period_cs_cashflows = context
            .capital_structure_cashflows
            .take()
            .unwrap_or_default();
        let (values, mut warnings) = context.into_results();
        // The second `evaluate_nodes_in_order` pass re-evaluates cs-affected
        // nodes into the same context, so node-level warnings (e.g.
        // DivisionByZero) can be pushed twice for the same node/period.
        // Deduplicate while preserving first-occurrence order.
        let mut seen: HashSet<String> = HashSet::with_capacity(warnings.len());
        warnings.retain(|w| seen.insert(format!("{w:?}")));
        warnings.append(&mut contractual_warnings);
        Ok((values, warnings, period_cs_cashflows))
    }
}

pub(crate) fn dependent_closure(
    graph: &DependencyGraph,
    seeds: &HashSet<NodeId>,
) -> HashSet<NodeId> {
    let mut visited: HashSet<NodeId> = seeds.iter().cloned().collect();
    let mut stack: Vec<NodeId> = seeds.iter().cloned().collect();

    while let Some(node) = stack.pop() {
        if let Some(dependents) = graph.dependents.get(node.as_str()) {
            for dependent in dependents {
                if visited.insert(dependent.clone()) {
                    stack.push(dependent.clone());
                }
            }
        }
    }

    visited
}

pub(crate) fn resolve_opening_balance(
    instrument: &(dyn finstack_cashflows::CashflowProvider + Send + Sync),
    market_ctx: &finstack_core::market_data::context::MarketContext,
    as_of: Date,
    period_start: Date,
) -> Result<Money> {
    let schedule = instrument.cashflow_schedule(market_ctx, as_of)?;
    let outstanding_path = schedule.outstanding_by_date()?;

    let abs_money = |m: &Money| -> Money {
        if m.amount() < 0.0 {
            Money::new(-m.amount(), m.currency())
        } else {
            *m
        }
    };

    if let Some((_, m)) = outstanding_path.iter().rfind(|(d, _)| *d <= period_start) {
        return Ok(abs_money(m));
    }

    // No outstanding entry at or before the period start. A forward-dated /
    // delayed-draw instrument (issue date after the period start) carries a
    // zero balance pre-issuance — falling back to the first *future* entry
    // would report the full notional before the debt exists. Only use the
    // first entry for issued instruments whose first flow lands later.
    let pre_issuance = schedule
        .meta
        .issue_date
        .is_some_and(|issue| issue > period_start)
        && outstanding_path
            .first()
            .is_some_and(|(d, _)| *d > period_start);
    if pre_issuance {
        return Ok(Money::new(0.0, schedule.notional.initial.currency()));
    }

    if let Some((_, m)) = outstanding_path.first() {
        return Ok(abs_money(m));
    }

    let currency = schedule
        .flows
        .first()
        .map(|cf| cf.amount.currency())
        .unwrap_or(finstack_core::currency::Currency::USD);
    Ok(Money::new(0.0, currency))
}

fn compute_contractual_flows(
    instruments: &Instruments,
    cs_state: &mut crate::capital_structure::CapitalStructureState,
    period: &Period,
    market_ctx: &finstack_core::market_data::context::MarketContext,
    as_of: Date,
) -> Result<(
    IndexMap<String, crate::capital_structure::CashflowBreakdown>,
    Vec<EvalWarning>,
)> {
    use crate::capital_structure::integration;

    let mut flows = IndexMap::new();
    let mut warnings = Vec::new();
    for (instrument_id, instrument) in instruments {
        let opening_balance =
            if let Some(balance) = cs_state.opening_balances.get(instrument_id).copied() {
                balance
            } else {
                let schedule = instrument.cashflow_schedule(market_ctx, as_of)?;
                Money::new(0.0, schedule.notional.initial.currency())
            };

        // Toggle-driven PIK capitalization accumulated in state is excluded
        // from the scale-clamp basis so PIK compounding is not frozen.
        let toggled_pik = cs_state
            .cumulative_toggled_pik
            .get(instrument_id.as_str())
            .copied()
            .unwrap_or_else(|| Money::new(0.0, opening_balance.currency()));
        let (breakdown, closing_balance, period_warnings) = integration::calculate_period_flows(
            instrument.as_ref(),
            period,
            opening_balance,
            toggled_pik,
            market_ctx,
            as_of,
        )?;
        warnings.extend(period_warnings);

        flows.insert(instrument_id.to_string(), breakdown.clone());
        cs_state.set_closing_balance(instrument_id.to_string(), closing_balance);
    }
    Ok((flows, warnings))
}

fn build_cs_cashflows_from_contractual(
    contractual_flows: &IndexMap<String, crate::capital_structure::CashflowBreakdown>,
    period_id: PeriodId,
) -> crate::capital_structure::CapitalStructureCashflows {
    let mut cs = crate::capital_structure::CapitalStructureCashflows::new();
    for (inst_id, breakdown) in contractual_flows {
        let mut period_map = IndexMap::new();
        period_map.insert(period_id, breakdown.clone());
        cs.by_instrument.insert(inst_id.clone(), period_map);
    }
    cs
}

fn build_fx_context<'a>(
    model: &FinancialModelSpec,
    market_ctx: &'a finstack_core::market_data::context::MarketContext,
    period: &Period,
) -> Option<CsTotalsContext<'a>> {
    let cs_spec = model.capital_structure.as_ref()?;
    let reporting_currency = cs_spec
        .reporting_currency
        .or_else(|| market_ctx.fx().map(|fx| fx.config().pivot_currency));
    let fx_matrix = market_ctx.fx();
    let fx_policy = cs_spec
        .fx_policy
        .unwrap_or(finstack_core::money::fx::FxConversionPolicy::CashflowDate);
    let snapshot_date = if period.end > period.start {
        period.end - time::Duration::days(1)
    } else {
        period.start
    };
    Some(CsTotalsContext {
        reporting_currency,
        fx_matrix,
        fx_policy,
        snapshot_date,
    })
}

struct CsTotalsContext<'a> {
    reporting_currency: Option<finstack_core::currency::Currency>,
    fx_matrix: Option<&'a std::sync::Arc<finstack_core::money::fx::FxMatrix>>,
    fx_policy: finstack_core::money::fx::FxConversionPolicy,
    snapshot_date: finstack_core::dates::Date,
}

fn recompute_cs_totals(
    cashflows: &mut crate::capital_structure::CapitalStructureCashflows,
    period_id: PeriodId,
    fx_ctx: Option<&CsTotalsContext<'_>>,
) -> crate::error::Result<()> {
    use crate::capital_structure::integration::convert_to_reporting;
    use finstack_core::currency::Currency;

    let mut totals_by_currency: IndexMap<Currency, crate::capital_structure::CashflowBreakdown> =
        IndexMap::new();
    cashflows.totals.clear();
    cashflows.totals_by_currency.clear();
    cashflows.reporting_currency = None;

    for breakdown in cashflows
        .by_instrument
        .values()
        .filter_map(|pm| pm.get(&period_id))
    {
        let currency = breakdown.interest_expense_cash.currency();
        let entry = totals_by_currency.entry(currency).or_insert_with(|| {
            crate::capital_structure::CashflowBreakdown::with_currency(currency)
        });

        entry.interest_expense_cash += breakdown.interest_expense_cash;
        entry.interest_expense_pik += breakdown.interest_expense_pik;
        entry.principal_payment += breakdown.principal_payment;
        entry.fees += breakdown.fees;
        entry.debt_balance += breakdown.debt_balance;
        entry.accrued_interest += breakdown.accrued_interest;
    }

    for (currency, breakdown) in &totals_by_currency {
        let mut period_map = IndexMap::new();
        period_map.insert(period_id, breakdown.clone());
        cashflows.totals_by_currency.insert(*currency, period_map);
    }

    if totals_by_currency.len() == 1 {
        if let Some((&currency, breakdown)) = totals_by_currency.iter().next() {
            cashflows.reporting_currency = Some(currency);
            cashflows.totals.insert(period_id, breakdown.clone());
        }
        return Ok(());
    }

    if let Some(ctx) = fx_ctx {
        if let Some(rc) = ctx.reporting_currency {
            let mut converted_total =
                crate::capital_structure::CashflowBreakdown::with_currency(rc);
            let mut all_converted = true;
            for (_, breakdown) in &totals_by_currency {
                let fields = [
                    breakdown.interest_expense_cash,
                    breakdown.interest_expense_pik,
                    breakdown.principal_payment,
                    breakdown.fees,
                    breakdown.debt_balance,
                    breakdown.accrued_interest,
                ];
                let mut converted_fields = Vec::with_capacity(6);
                for money in &fields {
                    match convert_to_reporting(
                        *money,
                        ctx.snapshot_date,
                        Some(rc),
                        ctx.fx_matrix,
                        ctx.fx_policy,
                    ) {
                        Ok(Some(m)) => converted_fields.push(m),
                        Ok(None) => {
                            all_converted = false;
                            break;
                        }
                        Err(e) => return Err(e),
                    }
                }
                if !all_converted {
                    break;
                }
                converted_total.interest_expense_cash += converted_fields[0];
                converted_total.interest_expense_pik += converted_fields[1];
                converted_total.principal_payment += converted_fields[2];
                converted_total.fees += converted_fields[3];
                converted_total.debt_balance += converted_fields[4];
                converted_total.accrued_interest += converted_fields[5];
            }
            if all_converted {
                cashflows.reporting_currency = Some(rc);
                cashflows.totals.insert(period_id, converted_total);
            }
        }
    }

    Ok(())
}

fn merge_updated_flows(
    cs_cashflows: &mut crate::capital_structure::CapitalStructureCashflows,
    updated_flows: &IndexMap<String, crate::capital_structure::CashflowBreakdown>,
    period_id: PeriodId,
) {
    for (inst_id, breakdown) in updated_flows {
        cs_cashflows
            .by_instrument
            .entry(inst_id.clone())
            .or_default()
            .insert(period_id, breakdown.clone());
    }
}
