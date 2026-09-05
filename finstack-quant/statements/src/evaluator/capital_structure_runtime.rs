//! Capital-structure-specific evaluator runtime helpers.

use super::{EvaluationContext, Evaluator, PeriodHistory};
use crate::error::Result;
use crate::evaluator::{DependencyGraph, EvalWarning};
use crate::types::{FinancialModelSpec, NodeId};
use finstack_quant_core::dates::{Date, Period, PeriodId};
use finstack_quant_core::money::Money;
use indexmap::IndexMap;
use std::collections::HashSet;
use std::sync::Arc;

type Instruments =
    IndexMap<String, Arc<dyn finstack_quant_cashflows::CashflowProvider + Send + Sync>>;
type DynamicPeriodEvaluation = (
    IndexMap<String, f64>,
    Vec<Option<f64>>,
    Vec<EvalWarning>,
    crate::capital_structure::CapitalStructureCashflows,
);

impl Evaluator {
    /// Build instruments from model specifications.
    pub(crate) fn build_instruments(
        &self,
        model: &FinancialModelSpec,
    ) -> Result<Option<Instruments>> {
        use crate::capital_structure::integration;
        use finstack_quant_cashflows::CashflowProvider;

        let Some(cs_spec) = &model.capital_structure else {
            return Ok(None);
        };
        let mut instruments: IndexMap<String, Arc<dyn CashflowProvider + Send + Sync>> =
            IndexMap::new();

        for debt_spec in &cs_spec.debt_instruments {
            if instruments.contains_key(&debt_spec.id) {
                return Err(crate::error::Error::build(format!(
                    "capital structure: duplicate debt instrument id '{}'. Instrument ids must \
                     be unique; a duplicate would silently overwrite the earlier definition and \
                     understate debt service.",
                    debt_spec.id
                )));
            }
            let instrument = integration::build_instrument_from_spec(debt_spec)?;
            instruments.insert(debt_spec.id.clone(), instrument);
        }

        Ok(Some(instruments))
    }

    /// Evaluate a period with dynamic capital structure support.
    #[allow(clippy::too_many_arguments)]
    pub(crate) fn evaluate_period_dynamic(
        &mut self,
        model: &FinancialModelSpec,
        period: &Period,
        is_actual: bool,
        eval_order: &[crate::types::NodeId],
        historical: &Arc<PeriodHistory>,
        historical_cs: &Arc<
            IndexMap<PeriodId, crate::capital_structure::CapitalStructureCashflows>,
        >,
        market_ctx: &finstack_quant_core::market_data::context::MarketContext,
        as_of: Date,
        instruments: &Instruments,
        cs_state: &mut crate::capital_structure::CapitalStructureState,
        cs_affected_nodes: &HashSet<NodeId>,
    ) -> Result<DynamicPeriodEvaluation> {
        let period_id = period.id;

        let (contractual_flows, mut contractual_warnings) =
            compute_contractual_flows(instruments, cs_state, period, market_ctx, as_of)?;

        let fx_ctx = build_fx_context(model, market_ctx, period);
        let mut cs_cashflows = build_cs_cashflows_from_contractual(&contractual_flows, period_id);
        recompute_cs_totals(&mut cs_cashflows, period_id, fx_ctx.as_ref())?;

        let mut context = EvaluationContext::new_with_history(
            period_id,
            Arc::clone(historical),
            Arc::clone(historical_cs),
        );
        context.set_capital_structure_cashflows(cs_cashflows.clone());

        self.evaluate_nodes_in_order(
            model,
            &period_id,
            is_actual,
            eval_order,
            &mut context,
            None,
            None,
            None,
        )?;

        if let Some(cs_spec) = &model.capital_structure {
            if let Some(waterfall_spec) = &cs_spec.waterfall {
                if let Some(node) = model.nodes.get(&crate::types::NodeId::new(
                    &waterfall_spec.available_cash_node,
                )) {
                    if let Some(formula) = &node.formula_text {
                        crate::capital_structure::reject_available_cash_debt_service(formula)?;
                    }
                }
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
                context.set_capital_structure_cashflows(cs_cashflows);
            }
            let from_date = crate::capital_structure::period_flows::period_snapshot_date(period);
            cs_state.rebuild_residuals(from_date)?;
        }

        if context.capital_structure_cashflows.is_some() && !cs_affected_nodes.is_empty() {
            self.evaluate_nodes_in_order(
                model,
                &period_id,
                is_actual,
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
        let row = context.current_values.clone();
        let (values, mut warnings) = context.into_results();
        // The second `evaluate_nodes_in_order` pass re-evaluates cs-affected
        // nodes into the same context, so node-level warnings (e.g.
        // DivisionByZero) can be pushed twice for the same node/period.
        // Deduplicate while preserving first-occurrence order.
        let mut seen: HashSet<String> = HashSet::with_capacity(warnings.len());
        warnings.retain(|w| seen.insert(format!("{w:?}")));
        warnings.append(&mut contractual_warnings);
        Ok((values, row, warnings, period_cs_cashflows))
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
    instrument: &(dyn finstack_quant_cashflows::CashflowProvider + Send + Sync),
    market_ctx: &finstack_quant_core::market_data::context::MarketContext,
    as_of: Date,
    period_start: Date,
) -> Result<Money> {
    let schedule = instrument.cashflow_schedule(market_ctx, as_of)?;
    let outstanding_path = schedule.outstanding_by_date()?;

    let abs_money = |m: &Money| -> Money {
        if m.amount() < 0.0 {
            m.checked_neg()
        } else {
            *m
        }
    };

    // Same half-open rule as `calculate_period_flows`: a coupon/amort dated
    // exactly on `period_start` belongs to this period, so opening is the
    // balance strictly before that date.
    if let Some((_, m)) = outstanding_path.iter().rfind(|(d, _)| *d < period_start) {
        return Ok(abs_money(m));
    }

    // No outstanding entry at or before the period start. A forward-dated /
    // delayed-draw instrument (issue date after the period start) carries a
    // zero balance pre-issuance — falling back to the first *future* entry
    // would report the full notional before the debt exists. Only use the
    // first entry for issued instruments whose first flow lands later.
    let pre_issuance = schedule
        .get_meta()
        .issue_date
        .is_some_and(|issue| issue > period_start)
        && outstanding_path
            .first()
            .is_some_and(|(d, _)| *d > period_start);
    if pre_issuance {
        return Ok(Money::from((
            0_i64,
            schedule.get_notional().initial.currency(),
        )));
    }

    if let Some((_, m)) = outstanding_path.first() {
        return Ok(abs_money(m));
    }

    // Use the schedule's own notional currency rather than guessing USD: an
    // empty-schedule non-USD instrument must not seed a USD zero balance (it can
    // later trip the waterfall's single-currency check with a confusing error).
    Ok(Money::from((
        0_i64,
        schedule.get_notional().initial.currency(),
    )))
}

fn compute_contractual_flows(
    instruments: &Instruments,
    cs_state: &mut crate::capital_structure::CapitalStructureState,
    period: &Period,
    market_ctx: &finstack_quant_core::market_data::context::MarketContext,
    as_of: Date,
) -> Result<(
    IndexMap<String, crate::capital_structure::CashflowBreakdown>,
    Vec<EvalWarning>,
)> {
    use crate::capital_structure::period_flows::calculate_period_flows;

    let mut flows = IndexMap::new();
    let mut warnings = Vec::new();
    for (instrument_id, instrument) in instruments {
        let opening_balance =
            if let Some(balance) = cs_state.opening_balances.get(instrument_id).copied() {
                balance
            } else {
                let schedule = instrument.cashflow_schedule(market_ctx, as_of)?;
                Money::from((0_i64, schedule.get_notional().initial.currency()))
            };

        // Toggle-driven PIK capitalization accumulated in state is excluded
        // from the scale-clamp basis so PIK compounding is not frozen.
        let toggled_pik = cs_state
            .cumulative_toggled_pik
            .get(instrument_id.as_str())
            .copied()
            .unwrap_or_else(|| Money::from((0_i64, opening_balance.currency())));
        if !cs_state.residual_schedules.contains_key(instrument_id) {
            let schedule = instrument.cashflow_schedule(market_ctx, as_of)?;
            cs_state
                .residual_schedules
                .insert(instrument_id.clone(), schedule);
        }
        let residual = cs_state.residual_schedules.get(instrument_id.as_str());
        let (breakdown, closing_balance, net_new_funding, period_warnings) =
            calculate_period_flows(
                instrument.as_ref(),
                period,
                opening_balance,
                toggled_pik,
                market_ctx,
                as_of,
                residual,
            )?;
        warnings.extend(period_warnings);

        flows.insert(instrument_id.to_string(), breakdown.clone());
        cs_state.set_closing_balance(instrument_id.to_string(), closing_balance);
        // Record the period's draws so the waterfall can recover the payable
        // balance and draw-aware closing (overwritten each period).
        cs_state
            .period_new_funding
            .insert(instrument_id.to_string(), net_new_funding);
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

/// Build the reporting-FX context for one period's `cs.*` totals.
///
/// When `CapitalStructureSpec.fx_policy` is omitted, conversion uses
/// [`finstack_quant_core::money::fx::FxConversionPolicy::PeriodEnd`]: cash
/// items and balances convert on the inclusive period-end snapshot
/// (`period.end - 1 day` under half-open `[start, end)`). Per-flow FX is not
/// applied here.
fn build_fx_context<'a>(
    model: &FinancialModelSpec,
    market_ctx: &'a finstack_quant_core::market_data::context::MarketContext,
    period: &Period,
) -> Option<CsTotalsContext<'a>> {
    let cs_spec = model.capital_structure.as_ref()?;
    let reporting_currency = cs_spec
        .reporting_currency
        .or_else(|| market_ctx.fx().map(|fx| fx.config().pivot_currency));
    let fx_matrix = market_ctx.fx();
    let fx_policy = cs_spec
        .fx_policy
        .unwrap_or(finstack_quant_core::money::fx::FxConversionPolicy::PeriodEnd);
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
    reporting_currency: Option<finstack_quant_core::currency::Currency>,
    fx_matrix: Option<&'a std::sync::Arc<finstack_quant_core::money::fx::FxMatrix>>,
    fx_policy: finstack_quant_core::money::fx::FxConversionPolicy,
    snapshot_date: finstack_quant_core::dates::Date,
}

fn recompute_cs_totals(
    cashflows: &mut crate::capital_structure::CapitalStructureCashflows,
    period_id: PeriodId,
    fx_ctx: Option<&CsTotalsContext<'_>>,
) -> crate::error::Result<()> {
    use crate::capital_structure::integration::convert_to_reporting;
    use finstack_quant_core::currency::Currency;

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
        let mut income = entry.interest_income_cash_or_zero();
        income += breakdown.interest_income_cash_or_zero();
        entry.interest_income_cash = Some(income);
    }

    for (currency, breakdown) in &totals_by_currency {
        let mut period_map = IndexMap::new();
        period_map.insert(period_id, breakdown.clone());
        cashflows.totals_by_currency.insert(*currency, period_map);
    }

    if totals_by_currency.len() == 1 && fx_ctx.and_then(|ctx| ctx.reporting_currency).is_none() {
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
                    breakdown.interest_income_cash_or_zero(),
                ];
                let mut converted_fields = Vec::with_capacity(fields.len());
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
                let mut income = converted_total.interest_income_cash_or_zero();
                income += converted_fields[6];
                converted_total.interest_income_cash = Some(income);
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

#[cfg(test)]
mod opening_tests {
    use super::resolve_opening_balance;
    use finstack_quant_cashflows::builder::{CashFlowMeta, CashFlowSchedule, Notional};
    use finstack_quant_cashflows::primitives::CFKind;
    use finstack_quant_core::cashflow::CashFlow;
    use finstack_quant_core::currency::Currency;
    use finstack_quant_core::dates::{Date, DayCount};
    use finstack_quant_core::market_data::context::MarketContext;
    use finstack_quant_core::money::Money;
    use time::Month;

    struct ScheduleInstrument {
        schedule: CashFlowSchedule,
    }

    impl finstack_quant_cashflows::CashflowScheduleSource for ScheduleInstrument {
        fn raw_cashflow_schedule(
            &self,
            _curves: &MarketContext,
            _as_of: Date,
        ) -> finstack_quant_core::Result<CashFlowSchedule> {
            Ok(self.schedule.clone())
        }
    }

    /// First-period opening must use the same half-open `< start` snapshot as
    /// period flows. A coupon/amort dated on the first period start belongs
    /// to that period, so opening is the pre-payment outstanding.
    #[test]
    fn first_period_opening_excludes_coupon_dated_on_period_start() {
        let issue = Date::from_calendar_date(2024, Month::October, 1).expect("valid date");
        let period_start = Date::from_calendar_date(2025, Month::January, 1).expect("valid date");
        let coupon_date = period_start;

        let instrument = ScheduleInstrument {
            schedule: CashFlowSchedule::from_parts(
                vec![
                    CashFlow::new(
                        issue,
                        None,
                        Money::from((-1_000_000_i64, Currency::USD)),
                        CFKind::Notional,
                        0.0,
                        None,
                    ),
                    CashFlow::new(
                        coupon_date,
                        None,
                        Money::from((-20_000_i64, Currency::USD)),
                        CFKind::Fixed,
                        0.25,
                        Some(0.08),
                    ),
                    CashFlow::new(
                        coupon_date,
                        None,
                        Money::from((100_000_i64, Currency::USD)),
                        CFKind::Amortization,
                        0.0,
                        None,
                    ),
                ],
                Notional::par(1_000_000.0, Currency::USD).expect("valid notional fixture"),
                DayCount::Act365F,
                CashFlowMeta {
                    issue_date: Some(issue),
                    ..CashFlowMeta::default()
                },
            ),
        };

        let market_ctx = MarketContext::new();
        let opening = resolve_opening_balance(&instrument, &market_ctx, issue, period_start)
            .expect("opening balance");

        assert_eq!(
            opening.amount(),
            1_000_000.0,
            "opening must be the pre-payment outstanding, not the post-amort snapshot on period.start"
        );
        assert_eq!(opening.currency(), Currency::USD);
    }
}

#[cfg(test)]
mod fx_policy_tests {
    use super::build_fx_context;
    use crate::types::{CapitalStructureSpec, FinancialModelSpec};
    use finstack_quant_core::dates::{Date, Period, PeriodId};
    use finstack_quant_core::market_data::context::MarketContext;
    use finstack_quant_core::money::fx::FxConversionPolicy;
    use time::Month;

    #[test]
    fn omitted_fx_policy_selects_period_end() {
        let start = Date::from_calendar_date(2025, Month::January, 1).expect("valid date");
        let end = Date::from_calendar_date(2025, Month::April, 1).expect("valid date");
        let period = Period {
            id: PeriodId::quarter(2025, 1).expect("valid period fixture"),
            start,
            end,
            is_actual: false,
        };
        let mut model = FinancialModelSpec::new("fx-default", vec![period.clone()]);
        model.capital_structure = Some(CapitalStructureSpec {
            debt_instruments: vec![],
            meta: indexmap::IndexMap::new(),
            reporting_currency: None,
            fx_policy: None,
            waterfall: None,
        });

        let market_ctx = MarketContext::new();
        let ctx =
            build_fx_context(&model, &market_ctx, &period).expect("capital structure is present");
        assert_eq!(
            ctx.fx_policy,
            FxConversionPolicy::PeriodEnd,
            "omitted fx_policy must convert cs.* period aggregates on the inclusive period-end date"
        );
    }
}
