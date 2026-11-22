//! LP IRR hurdle accounting and fund-end GP carry clawback reconciliation.

use super::engine::{AllocationParams, EquityWaterfallEngine};
use super::ledger::{AllocationRow, FundEvent, FundEventKind};
use finstack_quant_core::dates::Date;
use finstack_quant_core::money::Money;
use std::sync::Arc;

impl<'a> EquityWaterfallEngine<'a> {
    /// Build the LP-net cashflow history from contributions and persisted
    /// ledger `to_lp` rows .
    ///
    /// Gross fund events overstate the LP's receipts by the GP carry, which
    /// inflates the LP IRR and deflates preferred-return entitlements.
    /// Contributions up to and including `allocation_date` enter as negative
    /// flows (same-date contributions are processed before distributions, so
    /// capital called on the allocation date is already at risk); persisted
    /// `to_lp` allocations up to and including `allocation_date` enter as
    /// positive flows. Allocations from earlier tranches within the *current*
    /// distribution call are not yet persisted and are handled by the caller
    /// via `lp_allocated_in_call_so_far`.
    pub(super) fn lp_net_history(
        &self,
        all_events: &[FundEvent],
        prior_rows: &[AllocationRow],
        allocation_date: Date,
    ) -> finstack_quant_core::Result<Vec<(Date, Money)>> {
        let mut flows: Vec<(Date, Money)> = Vec::new();
        for event in all_events {
            if event.kind == FundEventKind::Contribution && event.date <= allocation_date {
                flows.push((
                    event.date,
                    Money::new(-event.amount.amount(), event.amount.currency())?,
                ));
            }
        }
        for row in prior_rows {
            if row.date <= allocation_date && row.to_lp.amount().abs() > 1e-9 {
                flows.push((row.date, row.to_lp));
            }
        }
        flows.sort_by_key(|(d, _)| *d);
        Ok(flows)
    }

    /// Additional receipt at `current_date` that makes the dated cashflows'
    /// NPV zero at the contractual hurdle. At a fixed hurdle this is linear in
    /// the additional amount; solving for an IRR first is unnecessary and can
    /// select an unrelated root when a fund makes later capital calls.
    pub(super) fn calculate_preferred_amount(
        &self,
        target_irr: f64,
        lp_flows: &[(Date, Money)],
        current_date: Date,
    ) -> finstack_quant_core::Result<f64> {
        if !target_irr.is_finite() || target_irr <= -1.0 {
            return Err(finstack_quant_core::Error::Validation(
                "waterfall IRR hurdle must be finite and greater than -100%".into(),
            ));
        }
        let Some(base_date) = lp_flows.iter().map(|(date, _)| *date).min() else {
            return Ok(0.0);
        };
        let elapsed = |date| {
            self.spec.day_count.year_fraction(
                base_date,
                date,
                finstack_quant_core::dates::DayCountContext::default(),
            )
        };
        let terminal_time = elapsed(current_date)?;
        let mut future_value = 0.0;
        for (date, amount) in lp_flows {
            // Use one common anchor, matching the IRR convention even for
            // day counts whose independently measured subperiods are not additive.
            let years = terminal_time - elapsed(*date)?;
            future_value += amount.amount() * (1.0 + target_irr).powf(years);
        }
        if !future_value.is_finite() {
            return Err(finstack_quant_core::Error::Validation(
                "waterfall hurdle cashflow accumulation is non-finite".into(),
            ));
        }
        Ok((-future_value).max(0.0))
    }

    /// Calculate LP IRR to date from an LP-net cashflow history.
    ///
    /// Uses the standalone [`super::super::metrics::calculate_irr`] routine (same
    /// `(1 + r)^{-t}` discounting, day-count error propagation, and
    /// multiple-root guard) with the waterfall's `day_count`, anchoring year
    /// fractions at the first flow.
    ///
    /// Propagates configuration or corrupt-input errors (such as an unsupported
    /// `day_count` basis or non-finite cashflows) while returning `Ok(None)`
    /// when the intermediate cashflow stream has no unique well-defined IRR
    /// (fewer than two non-zero dated flows, all same-sign flows, multiple sign
    /// changes from follow-on capital calls, or an unbracketed extreme loss).
    pub(super) fn calculate_lp_irr_to_date(
        &self,
        lp_flows: &[(Date, Money)],
    ) -> finstack_quant_core::Result<Option<f64>> {
        let _basis = finstack_quant_valuations_core::metrics::returns::YieldBasis::try_from(
            self.spec.day_count,
        )?;
        if lp_flows.iter().any(|(_, m)| !m.amount().is_finite()) {
            return Err(finstack_quant_core::Error::Validation(
                "LP waterfall cashflows must be finite".into(),
            ));
        }
        if lp_flows.len() < 2 {
            return Ok(None);
        }

        match super::super::metrics::calculate_irr(lp_flows, self.spec.day_count) {
            Ok(irr) => Ok(Some(irr)),
            Err(finstack_quant_core::Error::Input(
                finstack_quant_core::InputError::TooFewPoints,
            )) => Ok(None),
            Err(err)
                if err.kind() == finstack_quant_core::ErrorKind::Validation
                    || err.kind() == finstack_quant_core::ErrorKind::Computation =>
            {
                Ok(None)
            }
            Err(err) => Err(err),
        }
    }

    /// Reconcile lifetime tiers without moving historical LP receipt dates.
    ///
    /// A receipt is retained on its actual date and offset nominally at the
    /// reconciliation date. Allocating total lifetime proceeds at that date then
    /// changes only the final cumulative LP/GP split: all earlier LP cashflows
    /// keep their original hurdle value. Capital, catch-up and promote amounts
    /// still use lifetime nominal totals. The dated European replay caps the
    /// entitlement so reconciliation cannot create carry absent from that
    /// fund-level allocation (including partial or missing catch-up).
    fn allowed_gp_total_as_of(
        &self,
        events: &[FundEvent],
        ledger_rows: &[AllocationRow],
        as_of: Date,
    ) -> finstack_quant_core::Result<f64> {
        let history: Vec<FundEvent> = events
            .iter()
            .filter(|event| event.date <= as_of)
            .cloned()
            .collect();
        let Some(currency) = history.first().map(|event| event.amount.currency()) else {
            return Ok(0.0);
        };
        let total_distributions: f64 = history
            .iter()
            .filter(|event| event.kind != FundEventKind::Contribution)
            .map(|event| event.amount.amount())
            .sum();
        if total_distributions <= 0.0 {
            return Ok(0.0);
        }
        let total_contributions: f64 = history
            .iter()
            .filter(|event| event.kind == FundEventKind::Contribution)
            .map(|event| event.amount.amount())
            .sum();

        let mut replay_spec = self.spec.clone();
        replay_spec.clawback = None;
        let replay_engine = EquityWaterfallEngine::new(&replay_spec);
        let mut chronological_rows = Vec::new();
        replay_engine.run_european(&history, &mut chronological_rows)?;
        let chronological_carry: f64 = chronological_rows
            .iter()
            .map(|row| row.to_gp.amount())
            .sum();

        let mut timing_rows: Vec<AllocationRow> = ledger_rows
            .iter()
            .filter(|row| row.date <= as_of && row.to_lp.amount() != 0.0)
            .cloned()
            .collect();
        if let Some(mut offset) = timing_rows.last().cloned() {
            let receipts: f64 = timing_rows.iter().map(|row| row.to_lp.amount()).sum();
            offset.date = as_of;
            offset.to_lp = Money::from_model_value(-receipts, currency)?;
            timing_rows.push(offset);
        }
        let reconciled = replay_engine.allocate_distribution(AllocationParams {
            total_amount: Money::new(total_distributions, currency)?,
            initial_lp_unreturned: total_contributions,
            initial_gp_carry: 0.0,
            lp_distributed_cum_before: 0.0,
            all_events: &history,
            prior_rows: &timing_rows,
            allocation_date: as_of,
            currency,
        })?;
        let reconciled_carry: f64 = reconciled.iter().map(|row| row.to_gp.amount()).sum();
        Ok(reconciled_carry.min(chronological_carry).max(0.0))
    }

    /// Settle from existing escrow first, then recover overpaid carry from GP.
    /// A positive GP release cannot exceed the cash actually held: this clause
    /// does not authorize recalling distributions already paid to LPs.
    pub(super) fn settle_clawback_as_of(
        &self,
        events: &[FundEvent],
        ledger_rows: &mut Vec<AllocationRow>,
        settlement_date: Date,
        lp_unreturned: f64,
    ) -> finstack_quant_core::Result<()> {
        let last_row = ledger_rows
            .iter()
            .enumerate()
            .filter(|(_, row)| row.date <= settlement_date)
            .max_by_key(|(index, row)| (row.date, *index))
            .map(|(_, row)| row.clone());
        let Some(last_row) = last_row else {
            return Ok(());
        };
        let currency = last_row.to_gp.currency();
        let paid_lp: f64 = ledger_rows
            .iter()
            .filter(|row| row.date <= settlement_date)
            .map(|row| row.to_lp.amount())
            .sum();
        let paid_gp: f64 = ledger_rows
            .iter()
            .filter(|row| row.date <= settlement_date)
            .map(|row| row.to_gp.amount())
            .sum();
        let proceeds: f64 = events
            .iter()
            .filter(|event| {
                event.date <= settlement_date && event.kind != FundEventKind::Contribution
            })
            .map(|event| event.amount.amount())
            .sum();
        let escrow = (proceeds - paid_lp - paid_gp).max(0.0);
        let allowed_gp = self
            .allowed_gp_total_as_of(events, ledger_rows, settlement_date)?
            .min(paid_gp + escrow);
        let delta_gp = allowed_gp - paid_gp;
        let delta_lp = escrow - delta_gp;
        if delta_gp.abs() <= 1e-9 && delta_lp.abs() <= 1e-9 {
            return Ok(());
        }
        let mut flows = self.lp_net_history(events, ledger_rows, settlement_date)?;
        flows.push((
            settlement_date,
            Money::from_model_value(delta_lp, currency)?,
        ));
        let settlement_row = AllocationRow {
            date: settlement_date,
            deal_id: None,
            tranche: Arc::from("Clawback Settlement"),
            to_lp: Money::from_model_value(delta_lp, currency)?,
            to_gp: Money::from_model_value(delta_gp, currency)?,
            lp_unreturned: Money::from_model_value((lp_unreturned - delta_lp).max(0.0), currency)?,
            gp_carry_cum: Money::from_model_value(allowed_gp, currency)?,
            lp_irr_to_date: self.calculate_lp_irr_to_date(&flows)?,
            note: Some(Arc::from("Fund-level clawback and escrow release")),
        };
        ledger_rows.push(settlement_row);
        Ok(())
    }
}
