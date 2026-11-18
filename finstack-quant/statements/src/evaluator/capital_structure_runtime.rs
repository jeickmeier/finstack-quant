//! Capital-structure-specific evaluator runtime helpers.
//!
//! Statements consume raw classified schedules so historical periods and PIK
//! capitalization remain visible; public valuation schedules filter past cash
//! and normalize PIK into non-cash principal rows.

use super::engine::PeriodEval;
use super::{EvaluationContext, Evaluator, PeriodHistory};
use crate::error::Result;
use crate::evaluator::{DependencyGraph, EvalWarning};
use crate::types::{FinancialModelSpec, NodeId};
use finstack_quant_core::dates::{Date, Period, PeriodId};
use finstack_quant_core::money::Money;
use indexmap::IndexMap;
use std::collections::HashSet;
use std::sync::Arc;

type Instruments = IndexMap<String, Arc<dyn finstack_quant_valuations::instruments::Instrument>>;
type DynamicPeriodEvaluation = (
    IndexMap<String, crate::evaluator::ResultCell>,
    crate::evaluator::engine::PeriodRow,
    Vec<EvalWarning>,
    crate::capital_structure::CapitalStructureCashflows,
);

/// Capital-structure inputs of [`Evaluator::evaluate_period_dynamic`].
pub(crate) struct CapitalStructureRun<'a> {
    /// Market the debt instruments' contractual flows are projected on.
    pub(crate) market_ctx: &'a finstack_quant_core::market_data::context::MarketContext,
    /// Valuation date of the contractual flows.
    pub(crate) as_of: Date,
    /// Debt instruments keyed by id.
    pub(crate) instruments: &'a Instruments,
    /// Balances carried across periods, updated by this period.
    pub(crate) state: &'a mut crate::capital_structure::CapitalStructureState,
    /// Nodes re-evaluated after a waterfall replaces the contractual flows.
    pub(crate) affected_nodes: &'a HashSet<NodeId>,
}

impl Evaluator {
    /// Build instruments from model specifications.
    pub(crate) fn build_instruments(
        &self,
        model: &FinancialModelSpec,
    ) -> Result<Option<Instruments>> {
        use crate::capital_structure::integration;

        let Some(cs_spec) = &model.capital_structure else {
            return Ok(None);
        };
        // Once per run: the cash pool may not depend, through any chain of
        // formulas and masks, on a same-period output of its own waterfall.
        if let Some(waterfall) = &cs_spec.waterfall {
            for (setting, expression) in waterfall.sizing_expressions() {
                crate::capital_structure::reject_pool_output_dependence(
                    &model.nodes,
                    setting,
                    expression,
                )?;
            }
        }
        let mut instruments = Instruments::new();

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
    pub(crate) fn evaluate_period_dynamic(
        &mut self,
        eval: PeriodEval<'_>,
        historical: &Arc<PeriodHistory>,
        historical_cs: &Arc<
            IndexMap<PeriodId, crate::capital_structure::CapitalStructureCashflows>,
        >,
        cs: CapitalStructureRun<'_>,
        mut mc: Option<(
            crate::forecast::stream::PathId,
            &mut super::engine::McZCache,
        )>,
    ) -> Result<DynamicPeriodEvaluation> {
        let PeriodEval { model, period, .. } = eval;
        let CapitalStructureRun {
            market_ctx,
            as_of,
            instruments,
            state: committed_state,
            affected_nodes: cs_affected_nodes,
        } = cs;
        let period_id = period.id;
        // The period works on a staged copy of the debt state. It replaces
        // the committed state only after the statement cells that read the
        // final capital amounts have evaluated, so a period that fails
        // anywhere leaves the state of the last committed period.
        let mut staged_state = committed_state.clone();
        let cs_state = &mut staged_state;

        // The period is validated before any schedule is read.
        crate::capital_structure::period_flows::validate_period(period)?;
        let ContractualFlows {
            flows: contractual_flows,
            distress: distress_cash,
            warnings: mut contractual_warnings,
        } = compute_contractual_flows(instruments, cs_state, period, market_ctx, as_of)?;

        let fx_ctx = build_fx_context(model, market_ctx, period)?;
        let mut cs_cashflows = crate::capital_structure::CapitalStructureCashflows::new();
        merge_updated_flows(&mut cs_cashflows, &contractual_flows, period_id);
        if !distress_cash.is_empty() {
            cs_cashflows.distress_cash.insert(period_id, distress_cash);
        }
        recompute_cs_totals(&mut cs_cashflows, period_id, fx_ctx.as_ref())?;

        let mut context = EvaluationContext::new_with_history(
            period_id,
            Arc::clone(historical),
            Arc::clone(historical_cs),
        );
        context.set_capital_structure_cashflows(cs_cashflows.clone());

        self.evaluate_nodes_in_order(
            eval,
            &mut context,
            mc.as_ref().map(|(seed, _)| *seed),
            None,
            mc.as_mut().map(|(_, cache)| &mut **cache),
        )?;

        let mut waterfall_replaced_flows = false;
        if let Some(cs_spec) = &model.capital_structure
            && let Some(waterfall_spec) = &cs_spec.waterfall
        {
            // The debt ledger and the schedules the owners rebuild from
            // this period's principal changes commit together.
            let waterfall_result = crate::capital_structure::waterfall::execute_waterfall_then(
                &context,
                waterfall_spec,
                cs_state,
                &contractual_flows,
                model.capital_structure_currency()?,
                |staged| {
                    settle_postings(
                        staged,
                        instruments,
                        waterfall_spec.pik_toggle.as_ref(),
                        period,
                        market_ctx,
                        as_of,
                    )
                },
            )?;

            merge_updated_flows(&mut cs_cashflows, &waterfall_result.flows, period_id);
            recompute_cs_totals(&mut cs_cashflows, period_id, fx_ctx.as_ref())?;
            if let Some(equity) = waterfall_result.equity_distribution {
                cs_cashflows.equity_distribution.insert(period_id, equity);
            }
            if !waterfall_result.prepayments.is_empty() {
                cs_cashflows
                    .prepayments
                    .insert(period_id, waterfall_result.prepayments);
            }
            if let Some(retained) = waterfall_result.retained_cash {
                cs_cashflows.retained_cash.insert(period_id, retained);
            }
            if let Some(cash) = waterfall_result.available_cash {
                cs_cashflows.available_cash.insert(period_id, cash);
            }
            contractual_warnings.extend(waterfall_result.warnings);
            context.set_capital_structure_cashflows(cs_cashflows);
            waterfall_replaced_flows = true;
        }

        if waterfall_replaced_flows && !cs_affected_nodes.is_empty() {
            self.evaluate_nodes_in_order(
                eval,
                &mut context,
                mc.as_ref().map(|(seed, _)| *seed),
                Some(cs_affected_nodes),
                mc.as_mut().map(|(_, cache)| &mut **cache),
            )?;
        }

        let period_cs_cashflows = context
            .capital_structure_cashflows
            .take()
            .unwrap_or_default();
        let row = (
            context.current_values.clone(),
            context.current_cells.clone(),
        );
        let (values, mut warnings) = context.into_results();
        // The second `evaluate_nodes_in_order` pass re-evaluates cs-affected
        // nodes into the same context, so node-level warnings (e.g.
        // DivisionByZero) can be pushed twice for the same node/period.
        // Deduplicate while preserving first-occurrence order.
        let mut seen: HashSet<String> = HashSet::with_capacity(warnings.len());
        warnings.retain(|w| seen.insert(format!("{w:?}")));
        warnings.append(&mut contractual_warnings);
        *committed_state = staged_state;
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

/// Rebuild the schedule of every instrument whose principal the waterfall
/// changed this period or whose coupon election the PIK toggle changed.
///
/// Each principal change is appended to the instrument's posting ledger,
/// effective the day after the period close. In a forecast period the PIK
/// toggle's request becomes a dated coupon election at the start of each
/// authorized loan's next coupon period; an election lasts at least its
/// minimum number of coupon periods, counted from the election dates, so a
/// period evaluated again decides the same thing. The owner recompiles the
/// schedule from the contract and both ledgers. A schedule is accepted only
/// when the principal it carries into the next period equals the ledger's
/// closing balance; nothing is stored unless every instrument rebuilds.
fn settle_postings(
    state: &mut crate::capital_structure::CapitalStructureState,
    instruments: &Instruments,
    pik_toggle: Option<&crate::capital_structure::PikToggleSpec>,
    period: &Period,
    market_ctx: &finstack_quant_core::market_data::context::MarketContext,
    as_of: Date,
) -> Result<()> {
    use crate::capital_structure::period_flows::scheduled_opening_principal;
    use finstack_quant_cashflows::builder::CouponType;
    use finstack_quant_valuations::instruments::fixed_income::loan_terms::{
        CouponElection, PrincipalPosting,
    };
    use finstack_quant_valuations::instruments::fixed_income::term_loan::TermLoan;
    use rust_decimal::Decimal;

    let find = |instrument_id: &str| {
        instruments.get(instrument_id).ok_or_else(|| {
            crate::error::Error::capital_structure(format!(
                "'{instrument_id}' is not an instrument of the capital structure"
            ))
        })
    };

    let mut postings: IndexMap<String, Vec<PrincipalPosting>> = IndexMap::new();
    for (instrument_id, delta) in std::mem::take(&mut state.period_postings) {
        let mut ledger = state
            .postings
            .get(&instrument_id)
            .cloned()
            .unwrap_or_default();
        ledger.push(PrincipalPosting {
            effective_date: period.end,
            delta,
        });
        postings.insert(instrument_id, ledger);
    }

    let mut elections: IndexMap<String, Vec<CouponElection>> = IndexMap::new();
    let request = state.period_pik_request.take();
    if let (Some(pik), Some(request), false) = (pik_toggle, request, period.is_actual) {
        for election in &pik.elections {
            let instrument = find(&election.instrument_id)?;
            let loan = instrument
                .as_any()
                .downcast_ref::<TermLoan>()
                .ok_or_else(|| {
                    crate::error::Error::capital_structure(format!(
                        "`pik_toggle.elections` names '{}', which is not a term loan and has \
                         no PIK coupon terms to elect",
                        election.instrument_id
                    ))
                })?;
            // The election takes effect at the start of the next coupon
            // period; a loan with none left has nothing to elect.
            let starts = loan.coupon_period_starts()?;
            let Some(effective) = starts.iter().copied().find(|start| *start >= period.end) else {
                continue;
            };
            let current = state
                .coupon_elections
                .get(&election.instrument_id)
                .cloned()
                .unwrap_or_default();
            // Elections are made in period order, so nothing is dated after
            // this one; deciding from the dates alone means a period
            // evaluated again reaches the same decision.
            let mut ledger: Vec<CouponElection> = current
                .iter()
                .copied()
                .filter(|made| made.effective_date <= effective)
                .collect();
            let in_pik_since = ledger
                .last()
                .filter(|made| made.coupon_type != loan.coupon_type)
                .map(|made| made.effective_date);
            let elected = match (request, in_pik_since) {
                (true, None) => Some(if election.pik_fraction == Decimal::ONE {
                    CouponType::Pik
                } else {
                    CouponType::Split {
                        cash_fraction: Decimal::ONE - election.pik_fraction,
                        pik_fraction: election.pik_fraction,
                    }
                }),
                // A PIK election ends only after its minimum number of
                // coupon periods, counting the first elected one.
                (false, Some(since)) => {
                    let elapsed = starts
                        .iter()
                        .filter(|start| **start >= since && **start < effective)
                        .count();
                    (elapsed >= election.min_coupon_periods).then_some(loan.coupon_type)
                }
                _ => None,
            };
            if let Some(coupon_type) = elected {
                // One election per coupon period: a later request replaces
                // an election that has not started, and an election that
                // restores the terms already in force is no election.
                ledger.retain(|made| made.effective_date != effective);
                let in_force = ledger
                    .last()
                    .map_or(loan.coupon_type, |made| made.coupon_type);
                if coupon_type != in_force {
                    ledger.push(CouponElection {
                        effective_date: effective,
                        coupon_type,
                    });
                }
            }
            if ledger != current {
                elections.insert(election.instrument_id.clone(), ledger);
            }
        }
    }

    let changed: Vec<String> = postings
        .keys()
        .chain(elections.keys().filter(|id| !postings.contains_key(*id)))
        .cloned()
        .collect();
    let mut rebuilt = Vec::with_capacity(changed.len());
    for instrument_id in changed {
        let instrument = find(&instrument_id)?;
        let posting_ledger = postings
            .get(&instrument_id)
            .or_else(|| state.postings.get(&instrument_id))
            .map_or(&[][..], Vec::as_slice);
        let election_ledger = elections
            .get(&instrument_id)
            .or_else(|| state.coupon_elections.get(&instrument_id))
            .map_or(&[][..], Vec::as_slice);
        let schedule = crate::capital_structure::residual_schedule::rebuild_from_postings(
            instrument.as_ref(),
            market_ctx,
            as_of,
            posting_ledger,
            election_ledger,
        )?;
        let carried = scheduled_opening_principal(&schedule, period.end)?;
        let closing = state
            .closing_balances
            .get(&instrument_id)
            .copied()
            .unwrap_or(carried);
        // Half a minor unit is the dust bound the ledger itself uses.
        let bound = crate::capital_structure::period_flows::half_minor_unit(carried.currency());
        if (carried.amount() - closing.amount()).abs() > bound {
            return Err(crate::error::Error::capital_structure(format!(
                "instrument '{instrument_id}', period {}: the rebuilt schedule carries {} into \
                 the next period but the debt ledger closes at {}",
                period.id, carried, closing
            )));
        }
        rebuilt.push((instrument_id, schedule));
    }
    for (instrument_id, schedule) in rebuilt {
        state.residual_schedules.insert(instrument_id, schedule);
    }
    state.postings.extend(postings);
    state.coupon_elections.extend(elections);
    Ok(())
}

pub(crate) fn resolve_opening_balance(
    instrument: &(dyn finstack_quant_cashflows::CashflowProvider + Send + Sync),
    market_ctx: &finstack_quant_core::market_data::context::MarketContext,
    as_of: Date,
    period_start: Date,
) -> Result<Money> {
    let schedule = instrument.raw_cashflow_schedule(market_ctx, as_of)?;
    crate::capital_structure::period_flows::scheduled_opening_principal(&schedule, period_start)
}

/// One period's contractual capital-structure reading.
struct ContractualFlows {
    /// Debt-service breakdown per instrument.
    flows: IndexMap<String, crate::capital_structure::CashflowBreakdown>,
    /// Distress cash rows of the period, outside debt service.
    distress: Vec<crate::capital_structure::DistressCashflow>,
    /// Warnings raised while reading the schedules.
    warnings: Vec<EvalWarning>,
}

fn compute_contractual_flows(
    instruments: &Instruments,
    cs_state: &mut crate::capital_structure::CapitalStructureState,
    period: &Period,
    market_ctx: &finstack_quant_core::market_data::context::MarketContext,
    as_of: Date,
) -> Result<ContractualFlows> {
    use crate::capital_structure::period_flows::{
        SuppliedSchedule, calculate_period_flows, is_debt_kind, period_distress_cash,
    };

    crate::capital_structure::period_flows::validate_period(period)?;
    let mut flows = IndexMap::new();
    let mut distress = Vec::new();
    let mut warnings = Vec::new();
    for (instrument_id, instrument) in instruments {
        let opening_balance =
            if let Some(balance) = cs_state.opening_balances.get(instrument_id).copied() {
                balance
            } else {
                let schedule = instrument.raw_cashflow_schedule(market_ctx, as_of)?;
                Money::from((0_i64, schedule.get_notional().initial.currency()))
            };

        if crate::capital_structure::period_flows::is_debt_kind(instrument.key())?
            && !cs_state.residual_schedules.contains_key(instrument_id)
        {
            let schedule = instrument.raw_cashflow_schedule(market_ctx, as_of)?;
            cs_state
                .residual_schedules
                .insert(instrument_id.clone(), schedule);
        }
        // A residual schedule is rebuilt from the instrument's own rows and
        // keeps their holder perspective.
        let residual = cs_state
            .residual_schedules
            .get(instrument_id.as_str())
            .map(SuppliedSchedule::holder);
        let (breakdown, closing_balance, _, period_warnings) = calculate_period_flows(
            instrument.as_ref(),
            period,
            opening_balance,
            market_ctx,
            as_of,
            residual,
        )?;
        let is_debt = is_debt_kind(instrument.key())?;
        let raw;
        let source = match residual {
            Some(supplied) => supplied,
            None => {
                raw = instrument.raw_cashflow_schedule(market_ctx, as_of)?;
                SuppliedSchedule::holder(&raw)
            }
        };
        distress.extend(period_distress_cash(
            instrument_id.as_str(),
            is_debt,
            source,
            period,
        )?);
        let residual = residual.map(|supplied| supplied.schedule);
        // Every instrument has dated principal claims for the period: a
        // borrowing's scheduled repayments, and none for a hedge, whose
        // notional is not borrowed principal.
        let principal_claims = residual.map_or_else(Vec::new, |schedule| {
            crate::capital_structure::period_flows::period_principal_claims(schedule, period)
        });
        cs_state.set_period_principal_flows(instrument_id.clone(), period, principal_claims)?;
        warnings.extend(period_warnings);

        flows.insert(instrument_id.to_string(), breakdown.clone());
        cs_state.set_closing_balance(instrument_id.to_string(), closing_balance);
    }
    Ok(ContractualFlows {
        flows,
        distress,
        warnings,
    })
}

/// Build the reporting-FX context for one period's `cs.*` totals.
///
/// Aggregated `cs.*` cash items and balances convert on the inclusive
/// period-end snapshot (`period.end - 1 day` under half-open `[start, end)`)
/// with `CapitalStructureSpec.fx_policy`. There is no default policy: a
/// cross-currency conversion without one is an error. Per-flow FX is not
/// applied here.
fn build_fx_context<'a>(
    model: &FinancialModelSpec,
    market_ctx: &'a finstack_quant_core::market_data::context::MarketContext,
    period: &Period,
) -> crate::error::Result<Option<CsTotalsContext<'a>>> {
    let Some(cs_spec) = model.capital_structure.as_ref() else {
        return Ok(None);
    };
    let reporting_currency = model.capital_structure_currency()?;
    let fx_matrix = market_ctx.fx();
    let fx_policy = cs_spec.fx_policy;
    let snapshot_date = crate::capital_structure::period_flows::period_snapshot_date(period);
    Ok(Some(CsTotalsContext {
        reporting_currency,
        fx_matrix,
        fx_policy,
        snapshot_date,
    }))
}

struct CsTotalsContext<'a> {
    reporting_currency: Option<finstack_quant_core::currency::Currency>,
    fx_matrix: Option<&'a std::sync::Arc<finstack_quant_core::money::fx::FxMatrix>>,
    fx_policy: Option<crate::types::CapitalFxPolicy>,
    snapshot_date: finstack_quant_core::dates::Date,
}

fn recompute_cs_totals(
    cashflows: &mut crate::capital_structure::CapitalStructureCashflows,
    period_id: PeriodId,
    fx_ctx: Option<&CsTotalsContext<'_>>,
) -> crate::error::Result<()> {
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

        accumulate(entry, breakdown, Ok)?;
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

    cashflows.fx_conversions.shift_remove(&period_id);
    cashflows.consolidation_unavailable.shift_remove(&period_id);
    let Some(ctx) = fx_ctx else {
        return Ok(());
    };
    let Some(rc) = ctx.reporting_currency else {
        // More than one native currency and no target currency: only the
        // per-instrument and native-currency scopes exist.
        cashflows.consolidation_unavailable.insert(
            period_id,
            "the instruments are in more than one currency and no reporting currency is set"
                .to_string(),
        );
        return Ok(());
    };
    cashflows.reporting_currency = Some(rc);

    // One period-close rate per native currency. A missing quote leaves the
    // consolidated scope unavailable; the native scopes are already stored.
    let mut rates: IndexMap<Currency, f64> = IndexMap::new();
    let mut records = Vec::new();
    for &currency in totals_by_currency.keys() {
        if currency == rc {
            continue;
        }
        if ctx.fx_policy.is_none() {
            return Err(crate::error::Error::capital_structure(format!(
                "Cannot convert {currency} to reporting currency {rc}: \
                 capital_structure.fx_policy is not set. Choose `period_end` explicitly."
            )));
        }
        match crate::capital_structure::integration::period_close_rate(
            currency,
            rc,
            ctx.snapshot_date,
            ctx.fx_matrix,
        ) {
            Ok(record) => {
                rates.insert(currency, record.rate);
                records.push(record);
            }
            Err(reason) => {
                cashflows
                    .consolidation_unavailable
                    .insert(period_id, reason);
                return Ok(());
            }
        }
    }
    let mut converted_total = crate::capital_structure::CashflowBreakdown::with_currency(rc);
    for (currency, breakdown) in &totals_by_currency {
        match rates.get(currency) {
            None => accumulate(&mut converted_total, breakdown, Ok)?,
            Some(&rate) => accumulate(&mut converted_total, breakdown, |money| {
                Ok(Money::new(money.amount() * rate, rc)?)
            })?,
        }
    }
    cashflows.totals.insert(period_id, converted_total);
    if !records.is_empty() {
        cashflows.fx_conversions.insert(period_id, records);
    }

    Ok(())
}

/// Add every component of `source` into `total`, passing each amount through
/// `convert` first (identity for same-currency sums, FX for reporting totals).
fn accumulate(
    total: &mut crate::capital_structure::CashflowBreakdown,
    source: &crate::capital_structure::CashflowBreakdown,
    convert: impl Fn(Money) -> crate::error::Result<Money>,
) -> crate::error::Result<()> {
    total.interest_expense_cash = total
        .interest_expense_cash
        .checked_add(convert(source.interest_expense_cash)?)?;
    total.interest_expense_pik = total
        .interest_expense_pik
        .checked_add(convert(source.interest_expense_pik)?)?;
    total.principal_payment = total
        .principal_payment
        .checked_add(convert(source.principal_payment)?)?;
    total.fees = total.fees.checked_add(convert(source.fees)?)?;
    total.debt_balance = total
        .debt_balance
        .checked_add(convert(source.debt_balance)?)?;
    total.accrued_interest = total
        .accrued_interest
        .checked_add(convert(source.accrued_interest)?)?;
    let mut income = total.interest_income_cash_or_zero();
    income = income.checked_add(convert(source.interest_income_cash_or_zero())?)?;
    total.interest_income_cash = Some(income);
    // Opening balance and principal components; one the source does not
    // record leaves the total untouched.
    for (total, part) in total
        .audit_detail_mut()
        .into_iter()
        .zip(source.audit_detail())
    {
        crate::capital_structure::add_optional_money(total, part.map(&convert).transpose()?)?;
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
    use finstack_quant_core::cashflow::CFKind;
    use finstack_quant_core::cashflow::CashFlowParams;
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
            schedule: CashFlowSchedule::from_flows(
                vec![
                    {
                        let cash = Money::from((-1_000_000_i64, Currency::USD));
                        CashFlowParams::new(issue, None, cash, CFKind::Notional, 0.0, None)
                            .with_principal_delta(finstack_quant_core::money::Money::zero(
                                cash.currency(),
                            ))
                            .with_principal_date(issue)
                            .build()
                            .expect("valid cashflow")
                    },
                    CashFlowParams::new(
                        coupon_date,
                        None,
                        Money::from((-20_000_i64, Currency::USD)),
                        CFKind::Fixed,
                        0.25,
                        Some(0.08),
                    )
                    .build()
                    .expect("valid cashflow"),
                    {
                        let cash = Money::from((100_000_i64, Currency::USD));
                        CashFlowParams::new(
                            coupon_date,
                            None,
                            cash,
                            CFKind::Amortization,
                            0.0,
                            None,
                        )
                        .with_principal_delta(cash.checked_neg())
                        .with_principal_date(coupon_date)
                        .build()
                        .expect("valid cashflow")
                    },
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

    #[test]
    fn first_period_opening_without_funding_uses_pre_event_face() {
        let period_start = Date::from_calendar_date(2025, Month::January, 1).expect("valid date");
        let maturity = Date::from_calendar_date(2027, Month::January, 1).expect("valid date");
        for (issue, expected) in [
            (
                Date::from_calendar_date(2024, Month::January, 1).expect("issue"),
                100.0,
            ),
            (period_start, 100.0),
            (
                Date::from_calendar_date(2025, Month::January, 15).expect("issue"),
                0.0,
            ),
        ] {
            let instrument = ScheduleInstrument {
                schedule: CashFlowSchedule::from_flows(
                    vec![{
                        let cash = Money::from((100_i64, Currency::USD));
                        CashFlowParams::new(maturity, None, cash, CFKind::Notional, 0.0, None)
                            .with_principal_delta(cash.checked_neg())
                            .with_principal_date(maturity)
                            .build()
                            .expect("valid cashflow")
                    }],
                    Notional::par(100.0, Currency::USD).expect("face"),
                    DayCount::Act365F,
                    CashFlowMeta {
                        issue_date: Some(issue),
                        ..Default::default()
                    },
                ),
            };
            let opening = resolve_opening_balance(
                &instrument,
                &MarketContext::new(),
                period_start,
                period_start,
            )
            .expect("opening principal");
            assert_eq!(opening.amount(), expected, "issue={issue}");
        }
    }

    #[test]
    fn first_period_opening_without_funding_does_not_apply_boundary_amortization() {
        let period_start = Date::from_calendar_date(2025, Month::January, 1).expect("valid date");
        let instrument = ScheduleInstrument {
            schedule: CashFlowSchedule::from_flows(
                vec![{
                    let cash = Money::from((20_i64, Currency::USD));
                    CashFlowParams::new(period_start, None, cash, CFKind::Amortization, 0.0, None)
                        .with_principal_delta(cash.checked_neg())
                        .with_principal_date(period_start)
                        .build()
                        .expect("valid cashflow")
                }],
                Notional::par(100.0, Currency::USD).expect("face"),
                DayCount::Act365F,
                CashFlowMeta {
                    issue_date: Some(
                        Date::from_calendar_date(2024, Month::January, 1).expect("issue"),
                    ),
                    ..Default::default()
                },
            ),
        };
        let opening = resolve_opening_balance(
            &instrument,
            &MarketContext::new(),
            period_start,
            period_start,
        )
        .expect("opening principal");
        assert_eq!(opening.amount(), 100.0);
    }
}

#[cfg(test)]
mod fx_policy_tests {
    use super::build_fx_context;
    use crate::types::{CapitalStructureSpec, FinancialModelSpec};
    use finstack_quant_core::dates::{Date, Period, PeriodId};
    use finstack_quant_core::market_data::context::MarketContext;
    use time::Month;

    #[test]
    fn omitted_fx_policy_has_no_default() {
        let start = Date::from_calendar_date(2025, Month::January, 1).expect("valid date");
        let end = Date::from_calendar_date(2025, Month::April, 1).expect("valid date");
        let period = Period {
            id: PeriodId::quarter(2025, 1).expect("valid period fixture"),
            start,
            end,
            is_actual: false,
        };
        let mut model = FinancialModelSpec::new(
            "fx-default",
            finstack_quant_core::dates::PeriodPlan::try_new(
                finstack_quant_core::dates::PeriodCalendar::Gregorian,
                vec![period.clone()],
            )
            .expect("valid period plan"),
        );
        model.capital_structure = Some(CapitalStructureSpec {
            debt_instruments: vec![],
            meta: indexmap::IndexMap::new(),
            reporting_currency: None,
            fx_policy: None,
            waterfall: None,
        });

        let market_ctx = MarketContext::new();
        let ctx = build_fx_context(&model, &market_ctx, &period)
            .expect("valid currencies")
            .expect("capital structure is present");
        assert_eq!(
            ctx.fx_policy, None,
            "an omitted fx_policy is not replaced by PeriodEnd"
        );
    }
}

#[cfg(test)]
mod election_tests {
    use super::{Instruments, settle_postings};
    use crate::capital_structure::{CapitalStructureState, PikElectionSpec, PikToggleSpec};
    use finstack_quant_cashflows::builder::CouponType;
    use finstack_quant_core::dates::{Date, Period, PeriodId};
    use finstack_quant_core::market_data::context::MarketContext;
    use finstack_quant_valuations::instruments::fixed_income::term_loan::TermLoan;
    use std::sync::Arc;
    use time::macros::date;

    /// An election's duration is counted from its dates, so settling a
    /// period again, in any number of retries, counts each coupon period
    /// once and reaches the same decision.
    #[test]
    fn a_period_settled_again_makes_the_same_election() {
        // The example loan pays quarterly from 1 January 2024.
        let mut loan = TermLoan::example().expect("term loan");
        loan.id = "TL".into();
        let mut instruments = Instruments::new();
        instruments.insert("TL".into(), Arc::new(loan));
        let toggle = PikToggleSpec {
            liquidity_metric: "liquidity".into(),
            threshold: 1.0,
            elections: vec![PikElectionSpec {
                instrument_id: "TL".into(),
                pik_fraction: rust_decimal::Decimal::ONE,
                min_coupon_periods: 2,
            }],
        };
        let market = MarketContext::new();
        let mut state = CapitalStructureState::new();
        let mut settle = |quarter: u8, start: Date, end: Date, request: bool| {
            let period = Period {
                id: PeriodId::quarter(2024, quarter).expect("quarter"),
                start,
                end,
                is_actual: false,
            };
            for _ in 0..3 {
                state.period_pik_request = Some(request);
                settle_postings(
                    &mut state,
                    &instruments,
                    Some(&toggle),
                    &period,
                    &market,
                    date!(2024 - 01 - 01),
                )
                .expect("settlement");
            }
            state
                .coupon_elections
                .get("TL")
                .map(|ledger| {
                    ledger
                        .iter()
                        .map(|made| (made.effective_date, made.coupon_type))
                        .collect::<Vec<_>>()
                })
                .unwrap_or_default()
        };
        let pik = (date!(2024 - 04 - 01), CouponType::Pik);
        // A request for PIK in the first quarter covers periods from 1 April.
        assert_eq!(
            settle(1, date!(2024 - 01 - 01), date!(2024 - 04 - 01), true),
            vec![pik]
        );
        // One coupon period has run by 1 July: the minimum of two holds.
        assert_eq!(
            settle(2, date!(2024 - 04 - 01), date!(2024 - 07 - 01), false),
            vec![pik]
        );
        // Two have run by 1 October: the election ends there.
        assert_eq!(
            settle(3, date!(2024 - 07 - 01), date!(2024 - 10 - 01), false),
            vec![pik, (date!(2024 - 10 - 01), CouponType::Cash)]
        );
    }
}

#[cfg(test)]
mod consolidation_tests {
    use super::{CsTotalsContext, recompute_cs_totals};
    use crate::capital_structure::{
        CapitalStructureCashflows, CashflowBreakdown, CsComponent, CsScope,
    };
    use crate::types::CapitalFxPolicy;
    use finstack_quant_core::currency::Currency;
    use finstack_quant_core::dates::PeriodId;
    use finstack_quant_core::money::Money;
    use finstack_quant_core::money::fx::{FxMatrix, SimpleFxProvider};
    use std::sync::Arc;
    use time::macros::date;

    fn money(amount: i64, currency: Currency) -> Money {
        Money::from((amount, currency))
    }

    /// EUR 40 of scheduled and EUR 10 of sweep principal on one loan, USD 20
    /// of scheduled principal on another.
    fn two_currency_period(period: PeriodId) -> CapitalStructureCashflows {
        let mut cashflows = CapitalStructureCashflows::new();
        let mut euro = CashflowBreakdown::with_currency(Currency::EUR);
        euro.principal_payment = money(50, Currency::EUR);
        euro.scheduled_principal = Some(money(40, Currency::EUR));
        euro.sweep_prepayment = Some(money(10, Currency::EUR));
        let mut dollar = CashflowBreakdown::with_currency(Currency::USD);
        dollar.principal_payment = money(20, Currency::USD);
        dollar.scheduled_principal = Some(money(20, Currency::USD));
        dollar.sweep_prepayment = Some(money(0, Currency::USD));
        for (id, breakdown) in [("EUR-TL", euro), ("USD-TL", dollar)] {
            cashflows
                .by_instrument
                .entry(id.into())
                .or_default()
                .insert(period, breakdown);
        }
        cashflows
    }

    fn matrix(quotes: &[(Currency, Currency, f64)]) -> Arc<FxMatrix> {
        let provider = SimpleFxProvider::new();
        provider.set_quotes(quotes).expect("quotes");
        Arc::new(FxMatrix::new(Arc::new(provider), None))
    }

    /// The record's example: at USD 1.20 per EUR the structure consolidates
    /// to USD 80 of principal, USD 68 scheduled and USD 12 of sweep, and the
    /// conversion is retained with its pair, rate, date and source.
    #[test]
    fn native_totals_consolidate_at_the_period_close_rate() {
        let period = PeriodId::quarter(2025, 1).expect("quarter");
        let close = date!(2025 - 03 - 31);
        let fx = matrix(&[(Currency::EUR, Currency::USD, 1.2)]);
        let mut cashflows = two_currency_period(period);
        recompute_cs_totals(
            &mut cashflows,
            period,
            Some(&CsTotalsContext {
                reporting_currency: Some(Currency::USD),
                fx_matrix: Some(&fx),
                fx_policy: Some(CapitalFxPolicy::PeriodEnd),
                snapshot_date: close,
            }),
        )
        .expect("consolidation");

        let read = |component, scope: CsScope| {
            cashflows
                .get_money(component, &scope, &period)
                .expect("reported amount")
        };
        // Per-instrument and native-currency scopes are in their own currency.
        assert_eq!(
            read(
                CsComponent::PrincipalPayment,
                CsScope::Instrument("EUR-TL".into())
            ),
            money(50, Currency::EUR)
        );
        assert_eq!(
            read(
                CsComponent::SweepPrepayment,
                CsScope::NativeTotal(Currency::EUR)
            ),
            money(10, Currency::EUR)
        );
        assert_eq!(
            read(
                CsComponent::PrincipalPayment,
                CsScope::NativeTotal(Currency::USD)
            ),
            money(20, Currency::USD)
        );
        // The consolidated scope reconciles: 68 + 12 = 80.
        assert_eq!(
            read(CsComponent::PrincipalPayment, CsScope::Consolidated),
            money(80, Currency::USD)
        );
        assert_eq!(
            read(CsComponent::ScheduledPrincipal, CsScope::Consolidated),
            money(68, Currency::USD)
        );
        assert_eq!(
            read(CsComponent::SweepPrepayment, CsScope::Consolidated),
            money(12, Currency::USD)
        );
        // A component no breakdown recorded is unknown, not zero.
        assert!(
            cashflows
                .get_money(
                    CsComponent::MandatoryPrepayment,
                    &CsScope::Consolidated,
                    &period
                )
                .is_err()
        );
        let records = &cashflows.fx_conversions[&period];
        assert_eq!(records.len(), 1);
        assert_eq!(
            (
                records[0].from,
                records[0].to,
                records[0].rate,
                records[0].date
            ),
            (Currency::EUR, Currency::USD, 1.2, close)
        );
        assert_eq!(records[0].source, "direct");
        assert!(cashflows.consolidation_unavailable.is_empty());
    }

    /// A missing quote leaves the consolidated scope unavailable, with the
    /// reason, while the native scopes are reported as usual.
    #[test]
    fn a_missing_quote_makes_only_the_consolidated_scope_unavailable() {
        let period = PeriodId::quarter(2025, 1).expect("quarter");
        let fx = matrix(&[(Currency::GBP, Currency::USD, 1.3)]);
        let mut cashflows = two_currency_period(period);
        recompute_cs_totals(
            &mut cashflows,
            period,
            Some(&CsTotalsContext {
                reporting_currency: Some(Currency::USD),
                fx_matrix: Some(&fx),
                fx_policy: Some(CapitalFxPolicy::PeriodEnd),
                snapshot_date: date!(2025 - 03 - 31),
            }),
        )
        .expect("native totals still form");
        assert!(cashflows.totals.is_empty());
        assert!(cashflows.fx_conversions.is_empty());
        assert_eq!(cashflows.reporting_currency, Some(Currency::USD));
        let reason = &cashflows.consolidation_unavailable[&period];
        assert!(reason.contains("EUR/USD"), "{reason}");
        let error = cashflows
            .get_money(
                CsComponent::PrincipalPayment,
                &CsScope::Consolidated,
                &period,
            )
            .expect_err("unavailable consolidated total")
            .to_string();
        assert!(error.contains("unavailable"), "{error}");
        assert_eq!(
            cashflows
                .get_money(
                    CsComponent::PrincipalPayment,
                    &CsScope::NativeTotal(Currency::EUR),
                    &period
                )
                .expect("native total"),
            money(50, Currency::EUR)
        );

        // Without an explicit policy a cross-currency consolidation is an
        // error, never a default.
        let mut cashflows = two_currency_period(period);
        let error = recompute_cs_totals(
            &mut cashflows,
            period,
            Some(&CsTotalsContext {
                reporting_currency: Some(Currency::USD),
                fx_matrix: Some(&fx),
                fx_policy: None,
                snapshot_date: date!(2025 - 03 - 31),
            }),
        )
        .expect_err("no policy")
        .to_string();
        assert!(error.contains("fx_policy is not set"), "{error}");
    }
}
