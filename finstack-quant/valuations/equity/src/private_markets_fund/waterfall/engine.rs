//! Waterfall tier distribution execution (European, American, and per-distribution allocation).

use super::ledger::{AllocationLedger, AllocationRow, FundEvent, FundEventKind};
use super::spec::{CatchUpMode, PeFundWaterfallSpec, PeFundWaterfallTranche, WaterfallStyle};
use finstack_quant_core::config::{FinstackConfig, results_meta};
use finstack_quant_core::currency::Currency;
use finstack_quant_core::dates::Date;
use finstack_quant_core::money::Money;
use indexmap::IndexMap;
use std::sync::Arc;

pub(super) struct AllocationParams<'e> {
    pub(super) total_amount: Money,
    pub(super) initial_lp_unreturned: f64,
    pub(super) initial_gp_carry: f64,
    pub(super) lp_distributed_cum_before: f64,
    pub(super) all_events: &'e [FundEvent],
    /// Ledger rows persisted before this distribution call. LP IRR and
    /// preferred-return solves are computed from contributions plus these
    /// rows' `to_lp` amounts — the LP's actual net receipts — not from gross
    /// fund events .
    pub(super) prior_rows: &'e [AllocationRow],
    pub(super) allocation_date: Date,
    pub(super) currency: Currency,
}

/// Equity waterfall calculation engine.
pub struct EquityWaterfallEngine<'a> {
    pub(super) spec: &'a PeFundWaterfallSpec,
}

impl<'a> EquityWaterfallEngine<'a> {
    /// Create a new waterfall engine.
    pub fn new(spec: &'a PeFundWaterfallSpec) -> Self {
        Self { spec }
    }

    /// Run the waterfall allocation on the given events.
    pub fn run(&self, events: &[FundEvent]) -> finstack_quant_core::Result<AllocationLedger> {
        // Validate and sort events. Same-date ties order contributions before
        // distributions/proceeds (the standard treatment: capital called on a
        // date is at risk before cash distributed on that date), so the
        // allocation is deterministic regardless of caller input order. The
        // sort is stable, so same-date events of the same kind keep input
        // order.
        let mut sorted_events = events.to_vec();
        sorted_events.sort_by_key(|event| (event.date, event.kind != FundEventKind::Contribution));

        // Currency-safety: the waterfall performs raw f64 arithmetic across event
        // amounts, so every event must share a single currency. Reject mixed
        // currencies up front rather than silently summing across them. (Empty
        // input is left to the per-style handlers, which return `TooFewPoints`.)
        if let Some(first) = sorted_events.first() {
            let expected = first.amount.currency();
            if let Some(bad) = sorted_events
                .iter()
                .find(|e| e.amount.currency() != expected)
            {
                return Err(finstack_quant_core::Error::CurrencyMismatch {
                    expected,
                    actual: bad.amount.currency(),
                });
            }
        }

        let contributions: Vec<(Date, Money)> = sorted_events
            .iter()
            .filter(|e| e.kind == FundEventKind::Contribution)
            .map(|e| (e.date, e.amount.checked_neg()))
            .collect();

        let mut ledger_rows = Vec::new();

        let lp_unreturned = match self.spec.style {
            WaterfallStyle::European => self.run_european(&sorted_events, &mut ledger_rows)?,
            WaterfallStyle::American => self.run_american(&sorted_events, &mut ledger_rows)?,
        };

        if self.spec.clawback.is_some()
            && let Some(last_event_date) = sorted_events.last().map(|event| event.date)
        {
            self.settle_clawback_as_of(
                &sorted_events,
                &mut ledger_rows,
                last_event_date,
                lp_unreturned,
            )?;
        }

        let config = FinstackConfig::default();
        let meta = results_meta(&config);

        Ok(AllocationLedger {
            rows: ledger_rows,
            meta,
            contributions,
        })
    }

    /// Run European-style waterfall (fund-level aggregation).
    pub(super) fn run_european(
        &self,
        events: &[FundEvent],
        ledger_rows: &mut Vec<AllocationRow>,
    ) -> finstack_quant_core::Result<f64> {
        // Aggregate all events at fund level
        let mut lp_unreturned = 0.0;
        let mut gp_carry_cum = 0.0;
        let currency = events
            .first()
            .ok_or(finstack_quant_core::InputError::TooFewPoints)?
            .amount
            .currency();

        for event in events {
            if event.kind == FundEventKind::Distribution || event.kind == FundEventKind::Proceeds {
                let lp_distributed_so_far: f64 = ledger_rows.iter().map(|r| r.to_lp.amount()).sum();
                let allocations = self.allocate_distribution(AllocationParams {
                    total_amount: event.amount,
                    initial_lp_unreturned: lp_unreturned,
                    initial_gp_carry: gp_carry_cum,
                    lp_distributed_cum_before: lp_distributed_so_far,
                    all_events: events,
                    prior_rows: ledger_rows,
                    allocation_date: event.date,
                    currency,
                })?;

                for alloc in allocations {
                    lp_unreturned = alloc.lp_unreturned.amount();
                    gp_carry_cum = alloc.gp_carry_cum.amount();
                    ledger_rows.push(alloc);
                }
            } else if event.kind == FundEventKind::Contribution {
                // Track LP capital
                lp_unreturned += event.amount.amount();
            }
        }
        Ok(lp_unreturned)
    }

    /// Run American-style waterfall (deal-by-deal, chronological).
    ///
    /// Every event must identify its deal. Capital, LP return history,
    /// preferred-return hurdles, and GP carry are maintained independently for
    /// each deal. Rows from all deals remain globally date-ordered because
    /// `run` sorts the input events before dispatching here.
    fn run_american(
        &self,
        events: &[FundEvent],
        ledger_rows: &mut Vec<AllocationRow>,
    ) -> finstack_quant_core::Result<f64> {
        #[derive(Default)]
        struct DealState {
            lp_unreturned: f64,
            gp_carry_cum: f64,
            rows: Vec<AllocationRow>,
        }

        let currency = events
            .first()
            .ok_or(finstack_quant_core::InputError::TooFewPoints)?
            .amount
            .currency();

        let mut events_by_deal: IndexMap<String, Vec<FundEvent>> = IndexMap::new();
        for event in events {
            let deal_id = event
                .deal_id
                .as_deref()
                .filter(|id| !id.trim().is_empty())
                .ok_or_else(|| {
                    finstack_quant_core::Error::Validation(format!(
                        "American waterfall requires a non-empty deal_id on every event; \
                         missing on {:?} event dated {}",
                        event.kind, event.date
                    ))
                })?;
            events_by_deal
                .entry(deal_id.to_owned())
                .or_default()
                .push(event.clone());
        }

        let mut states: IndexMap<String, DealState> = IndexMap::new();
        for event in events {
            let deal_id = event.deal_id.as_deref().ok_or_else(|| {
                finstack_quant_core::Error::Validation(
                    "American waterfall event lost its validated deal_id".to_string(),
                )
            })?;
            let deal_events = events_by_deal.get(deal_id).ok_or_else(|| {
                finstack_quant_core::Error::Validation(format!(
                    "American waterfall has no event ledger for deal '{deal_id}'"
                ))
            })?;
            let state = states.entry(deal_id.to_owned()).or_default();

            if event.kind == FundEventKind::Contribution {
                state.lp_unreturned += event.amount.amount();
                continue;
            }

            let lp_distributed_so_far: f64 = state.rows.iter().map(|row| row.to_lp.amount()).sum();
            let allocations = self.allocate_distribution(AllocationParams {
                total_amount: event.amount,
                initial_lp_unreturned: state.lp_unreturned,
                initial_gp_carry: state.gp_carry_cum,
                lp_distributed_cum_before: lp_distributed_so_far,
                all_events: deal_events,
                prior_rows: &state.rows,
                allocation_date: event.date,
                currency,
            })?;

            for mut allocation in allocations {
                allocation.deal_id = Some(Arc::from(deal_id));
                state.lp_unreturned = allocation.lp_unreturned.amount();
                state.gp_carry_cum = allocation.gp_carry_cum.amount();
                state.rows.push(allocation.clone());
                ledger_rows.push(allocation);
            }
        }

        Ok(states.values().map(|state| state.lp_unreturned).sum())
    }

    /// Allocate a single distribution through the waterfall.
    pub(super) fn allocate_distribution(
        &self,
        params: AllocationParams,
    ) -> finstack_quant_core::Result<Vec<AllocationRow>> {
        let mut remaining_amount = params.total_amount.amount();
        let mut lp_unreturned = params.initial_lp_unreturned;
        let mut gp_carry_cum = params.initial_gp_carry;
        let mut allocations = Vec::new();

        // Track LP allocated within this distribution call (prior tranches)
        let mut lp_allocated_in_call_so_far: f64 = 0.0;

        // LP-net cashflow history (contributions + ledger to_lp rows) used by
        // every IRR-based solve in this call . Same-date
        // allocations from earlier tranches in this call are handled via
        // `lp_allocated_in_call_so_far`.
        let lp_history =
            self.lp_net_history(params.all_events, params.prior_rows, params.allocation_date)?;

        // Precompute holdback percent (0.0 if no clawback or no holdback)
        let holdback_decimal: f64 = (match &self.spec.clawback {
            Some(c) => c.holdback_decimal.unwrap_or(0.0),
            _ => 0.0,
        })
        .clamp(0.0, 1.0);

        for (idx, tranche) in self.spec.tranches.iter().enumerate() {
            if remaining_amount <= 1e-6 {
                break;
            }

            let (to_lp, to_gp_paid, tranche_name, gp_carry_cum_after) = match tranche {
                PeFundWaterfallTranche::ReturnOfCapital => {
                    let allocation = remaining_amount.min(lp_unreturned);
                    lp_unreturned -= allocation;
                    remaining_amount -= allocation;
                    lp_allocated_in_call_so_far += allocation;
                    (
                        allocation,
                        0.0,
                        Arc::from("Return of Capital"),
                        gp_carry_cum,
                    )
                }

                PeFundWaterfallTranche::PreferredIrr { hurdle_irr: irr } => {
                    // `calculate_preferred_amount` returns the total LP amount needed at
                    // `allocation_date` to reach `target_irr`, based on the LP-net
                    // ledger history.  Earlier tranches within this same distribution
                    // (captured in `lp_allocated_in_call_so_far`) are invisible to the
                    // IRR solve because they haven't been persisted to the ledger yet.
                    // We subtract that same-date offset so the preferred tier only awards
                    // the incremental cash still needed on top of what the LP already
                    // received from earlier tranches in this distribution call.
                    let gross_required =
                        self.calculate_preferred_amount(*irr, &lp_history, params.allocation_date)?;
                    let required = (gross_required - lp_allocated_in_call_so_far).max(0.0);
                    let allocation = remaining_amount.min(required);
                    remaining_amount -= allocation;
                    lp_allocated_in_call_so_far += allocation;
                    (
                        allocation,
                        0.0,
                        format!("Preferred Return {:.1}%", irr * 100.0).into(),
                        gp_carry_cum,
                    )
                }

                PeFundWaterfallTranche::CatchUp { gp_share } => {
                    // Determine target cumulative GP share from the next promote tier if available
                    let mut target_gp_share: f64 = *gp_share; // fallback
                    for next in self.spec.tranches.iter().skip(idx + 1) {
                        if let PeFundWaterfallTranche::PromoteTier { gp_share, .. } = next {
                            target_gp_share = *gp_share;
                            break;
                        }
                    }

                    let total_contributions_to_date: f64 = params
                        .all_events
                        .iter()
                        .filter(|e| {
                            e.kind == FundEventKind::Contribution
                                && e.date <= params.allocation_date
                        })
                        .map(|e| e.amount.amount())
                        .sum();

                    // Profit to date before this catch-up tranche (gross basis)
                    let mut profit_excl = (params.lp_distributed_cum_before
                        + lp_allocated_in_call_so_far
                        + gp_carry_cum)
                        - total_contributions_to_date;
                    if !profit_excl.is_finite() || profit_excl.is_sign_negative() {
                        profit_excl = 0.0;
                    }

                    let catchup_share = match self.spec.catch_up_mode {
                        CatchUpMode::Full => 1.0,
                        CatchUpMode::Partial => *gp_share,
                    };
                    let gap = (target_gp_share * profit_excl - gp_carry_cum).max(0.0);
                    let catchup_amount = if gap == 0.0 {
                        0.0
                    } else if catchup_share > target_gp_share {
                        (gap / (catchup_share - target_gp_share)).min(remaining_amount)
                    } else {
                        // This split cannot reach the target; all remaining cash
                        // stays in the contractual catch-up tranche.
                        remaining_amount
                    };
                    let to_gp_gross = catchup_amount * catchup_share;
                    let to_lp = catchup_amount - to_gp_gross;

                    // Holdback flow: `gp_carry_cum` and `remaining_amount`
                    // both move by the GROSS carry, but only `to_gp_paid`
                    // (post-holdback) is recorded on the ledger row. The
                    // held-back portion (`to_gp_gross - to_gp_paid`) is not
                    // an on-balance line item here; it is implicitly released
                    // to GP at fund-end clawback settlement, where
                    // `apply_clawback` reconciles via
                    //   delta_gp = allowed_gp_total(as_of) − Σ ledger.to_gp
                    // using dated lifetime entitlement and separately tracking
                    // the implied escrow balance. Excess escrow goes to LP;
                    // any remaining shortfall is recovered from paid GP carry.
                    let to_gp_paid = to_gp_gross * (1.0 - holdback_decimal);
                    gp_carry_cum += to_gp_gross;
                    remaining_amount -= catchup_amount;
                    (to_lp, to_gp_paid, Arc::from("Catch-Up"), gp_carry_cum)
                }

                PeFundWaterfallTranche::PromoteTier {
                    lp_share,
                    gp_share,
                    hurdle_irr: hurdle_rate,
                } => {
                    // Gate the promote split on the LP actually reaching the
                    // tier's hurdle IRR : pay the LP at
                    // 100% until the hurdle is met, then split at
                    // lp_share/gp_share, cascading to the next tier once the
                    // next hurdle is reached.

                    // 1) 100% to LP until this tier's hurdle IRR is met.
                    let gross_required = self.calculate_preferred_amount(
                        *hurdle_rate,
                        &lp_history,
                        params.allocation_date,
                    )?;
                    let pre_split = (gross_required - lp_allocated_in_call_so_far)
                        .max(0.0)
                        .min(remaining_amount);
                    remaining_amount -= pre_split;
                    lp_allocated_in_call_so_far += pre_split;

                    // 2) Split at lp/gp shares, capped so the LP does not get
                    //    pushed past the *next* promote tier's hurdle — any
                    //    excess cascades to that tier.
                    let next_hurdle_rate =
                        self.spec
                            .tranches
                            .iter()
                            .skip(idx + 1)
                            .find_map(|t| match t {
                                PeFundWaterfallTranche::PromoteTier { hurdle_irr, .. } => {
                                    Some(*hurdle_irr)
                                }
                                _ => None,
                            });
                    let split_amount = match next_hurdle_rate {
                        Some(next_rate) if *lp_share > 1e-12 => {
                            let next_required = self.calculate_preferred_amount(
                                next_rate,
                                &lp_history,
                                params.allocation_date,
                            )?;
                            let lp_needed = (next_required - lp_allocated_in_call_so_far).max(0.0);
                            (lp_needed / lp_share).min(remaining_amount)
                        }
                        _ => remaining_amount,
                    };

                    let to_lp_split = split_amount * lp_share;
                    let to_gp_gross = split_amount * gp_share;
                    let to_gp_paid = to_gp_gross * (1.0 - holdback_decimal);
                    gp_carry_cum += to_gp_gross;
                    remaining_amount -= split_amount;
                    lp_allocated_in_call_so_far += to_lp_split;

                    let tranche_name: Arc<str> = format!(
                        "Promote {:.1}%+ ({}%/{}%)",
                        hurdle_rate * 100.0,
                        lp_share * 100.0,
                        gp_share * 100.0
                    )
                    .into();

                    (
                        pre_split + to_lp_split,
                        to_gp_paid,
                        tranche_name,
                        gp_carry_cum,
                    )
                }
            };

            // Calculate current LP IRR from the LP-net ledger history plus
            // what this call has allocated to the LP so far .
            let lp_irr_to_date = {
                let mut flows = lp_history.clone();
                if lp_allocated_in_call_so_far > 1e-9 {
                    flows.push((
                        params.allocation_date,
                        Money::from_model_value(lp_allocated_in_call_so_far, params.currency)?,
                    ));
                }
                self.calculate_lp_irr_to_date(&flows)?
            };

            allocations.push(AllocationRow {
                date: params.allocation_date,
                deal_id: None, // Set by caller for American style
                tranche: tranche_name,
                to_lp: Money::from_model_value(to_lp, params.currency)?,
                to_gp: Money::from_model_value(to_gp_paid, params.currency)?,
                lp_unreturned: Money::from_model_value(lp_unreturned, params.currency)?,
                gp_carry_cum: Money::from_model_value(gp_carry_cum_after, params.currency)?,
                lp_irr_to_date,
                note: None,
            });
        }

        // Exhaustive-allocation invariant: a canonical waterfall ends in a
        // residual-splitting tier (e.g. the carried-interest promote), so a
        // distribution is always fully allocated. Cash the configured tiers
        // cannot place must FAIL here — silently discarding it would
        // understate LP economics (cashflows, IRR, MOIC, DPI, TVPI) with no
        // diagnostic. Tolerance matches the loop's early-exit epsilon.
        if remaining_amount > 1e-6 {
            return Err(finstack_quant_core::Error::Validation(format!(
                "waterfall spec cannot fully allocate the {} distribution on {}: {:.6} \
                 remains unallocated after the last tranche. End the spec with a \
                 residual-splitting tier (e.g. `promote_tier(hurdle, lp_share, gp_share)`) \
                 so profits above the configured tiers are not dropped",
                params.total_amount.amount(),
                params.allocation_date,
                remaining_amount
            )));
        }

        Ok(allocations)
    }
}
