//! Deterministic draw/repay cashflow schedule builder for [`CashflowEngine`].

use super::super::types::DrawRepaySpec;
use super::CashflowEngine;
use super::fees::{FeeAccrual, FeeBalances};
use finstack_quant_cashflows::builder::{CashFlowSchedule, RateSpec};
use finstack_quant_core::Result;
use finstack_quant_core::cashflow::{CFKind, CashFlowParams};
use finstack_quant_core::config::{RoundingContext, ZeroKind};
use finstack_quant_core::dates::{Date, DayCount, DayCountContext};
use finstack_quant_core::market_data::fixings::fixing_series_id;
use finstack_quant_core::money::Money;

impl<'a> CashflowEngine<'a> {
    /// Build deterministic cashflow schedule from draw/repay events.
    ///
    /// This is the core deterministic cashflow generation logic, migrated from
    /// the original `cashflows.rs::generate_deterministic_cashflows_internal`.
    pub(super) fn build_deterministic_schedule(&self) -> Result<CashFlowSchedule> {
        let mut projected_fixings = Vec::new();
        let draw_repay_events = match &self.facility.draw_repay_spec {
            DrawRepaySpec::Deterministic(events) => events.clone(),
            DrawRepaySpec::Stochastic(_) => {
                return Err(finstack_quant_core::Error::Validation(
                    "Deterministic cashflows require DrawRepaySpec::Deterministic".to_string(),
                ));
            }
        };
        // Postings move the drawn balance like the facility's own events; only
        // the facility's events are cash rows. A posting was made at the
        // prior day's close, so it precedes the facility's events of its
        // effective date: postings go first and the date sort is stable.
        let mut contract_events = draw_repay_events;
        let ccy = self.facility.commitment.currency();
        if !self.postings.is_empty() {
            // After a posting, a facility repayment repays what is left of
            // the drawn balance, as a term loan's later amortization does: a
            // sweep cannot turn a scheduled repayment into an overpayment.
            // The replay is in date order with a posting ahead of the
            // facility's events of its date; a posting that itself exceeds
            // the balance is still refused by the validated replay below.
            let mut dated: Vec<(finstack_quant_core::dates::Date, bool, usize)> = self
                .postings
                .iter()
                .enumerate()
                .map(|(index, posting)| (posting.effective_date, false, index))
                .chain(
                    contract_events
                        .iter()
                        .enumerate()
                        .map(|(index, event)| (event.date, true, index)),
                )
                .collect();
            dated.sort_by_key(|(date, is_contract, _)| (*date, *is_contract));
            let mut balance = self.facility.drawn.amount();
            for (_, is_contract, index) in dated {
                if !is_contract {
                    balance += self.postings[index].delta.amount();
                    continue;
                }
                let event = &mut contract_events[index];
                if event.is_draw {
                    balance += event.amount.amount();
                } else {
                    let capped = event.amount.amount().min(balance.max(0.0));
                    if capped < event.amount.amount() {
                        event.amount = Money::new(capped, ccy)?;
                    }
                    balance -= capped;
                }
            }
            contract_events.retain(|event| event.is_draw || event.amount.amount() > 0.0);
        }
        let mut draw_repay_events = Vec::with_capacity(self.postings.len() + contract_events.len());
        for posting in self.postings {
            if posting.delta.currency() != ccy {
                return Err(finstack_quant_core::Error::Validation(format!(
                    "RevolvingCredit '{}': a principal posting effective {} is in {}, not the \
                     facility currency {ccy}",
                    self.facility.id,
                    posting.effective_date,
                    posting.delta.currency()
                )));
            }
            draw_repay_events.push(super::super::types::DrawRepayEvent {
                date: posting.effective_date,
                amount: posting.delta.abs(),
                is_draw: posting.delta.amount() > 0.0,
            });
        }
        draw_repay_events.extend(contract_events.iter().cloned());
        draw_repay_events.sort_by_key(|event| event.date);

        // `drawn` is the balance at the simulation anchor (the later of
        // the commitment and valuation dates) in both engines, so events are
        // future-only. An event dated on or before the anchor would be
        // replayed on top of a balance that already includes it: interest
        // accrued on 2X and 2X repaid at maturity.
        let anchor = self.facility.issue_date.max(self.as_of);
        if let Some(event) = draw_repay_events.iter().find(|e| e.date <= anchor) {
            return Err(finstack_quant_core::Error::Validation(format!(
                "RevolvingCredit draw/repay event dated {} is on or before the simulation anchor \
                 {} (the later of the commitment date {} and the valuation date {}); the drawn \
                 balance at the anchor is defined by drawn, so date events strictly after it",
                event.date, anchor, self.facility.issue_date, self.as_of
            )));
        }

        self.check_anchor_capacity(anchor)?;
        let step_dates = self.facility.step_dates();
        let mut flows = Vec::new();
        let rc = RoundingContext::default();

        // Add initial draw at issue_date (from lender perspective: negative cashflow)
        if self.facility.issue_date > self.as_of
            && !rc.is_effectively_zero(self.facility.drawn.amount(), ZeroKind::Money(ccy))
        {
            flows.push({
                // The par change equals the cash (record CORE-CONVENTIONS-015 #4).
                let cash = self.facility.drawn.checked_neg();
                CashFlowParams::new(
                    self.facility.issue_date,
                    None,
                    cash,
                    CFKind::Notional,
                    0.0,
                    None,
                )
                .with_principal_delta(cash.checked_neg())
                .with_principal_date(self.facility.issue_date)
                .build()?
            });
        }

        // Generate interest and fee cashflows with intra-period event slicing
        flows.reserve(self.payment_periods.len() * 4 + draw_repay_events.len() + 2);

        // Resolve forward curve once if floating rate (required for rate projection)
        let fwd_curve = match &self.facility.rate {
            RateSpec::Floating(spec) => {
                if let Some(market) = self.market {
                    Some(market.get_forward(&spec.forward_curve_id)?)
                } else {
                    return Err(finstack_quant_core::Error::Validation(format!(
                        "Market context required for floating rate facility (index: {})",
                        spec.forward_curve_id
                    )));
                }
            }
            RateSpec::Fixed { .. } => None,
        };

        // Term-index facilities re-fix the coupon at every reset date, so the
        // sub-period timeline must also be sliced on resets that fall inside
        // an accrual period (reset frequency shorter than payment frequency).
        // Overnight facilities compound daily fixings over the whole window.
        let slice_on_resets = match &self.facility.rate {
            RateSpec::Floating(spec) => {
                super::super::schedule_projection::resolved_overnight_compounding(spec)?.is_none()
            }
            RateSpec::Fixed { .. } => false,
        };

        for (i, period) in self.payment_periods.iter().enumerate() {
            let period_start = period.accrual_start;
            let period_end = period.accrual_end;
            let accrual_context = if self.day_count == DayCount::Act365L {
                DayCountContext {
                    frequency: Some(self.facility.frequency),
                    coupon_period: Some((period_start, period_end)),
                    ..Default::default()
                }
            } else {
                DayCountContext::default()
            };
            let payment_date = period.payment_date;

            // Apply as_of filtering for non-principal cashflows
            if payment_date <= self.as_of {
                continue;
            }

            // Build sub-period timeline with events
            // Events at period_end are excluded - they happen AFTER interest calculation
            let mut timeline = vec![period_start];
            for event in draw_repay_events.iter() {
                if event.date > period_start && event.date < period_end {
                    timeline.push(event.date);
                }
            }
            if slice_on_resets && let Some(ref reset_grid) = self.reset_dates {
                timeline.extend(
                    reset_grid
                        .iter()
                        .copied()
                        .filter(|&reset| reset > period_start && reset < period_end),
                );
            }
            // Commitment, margin and fee steps change the accrual inputs.
            timeline.extend(
                step_dates
                    .iter()
                    .copied()
                    .filter(|&step| step > period_start && step < period_end),
            );
            timeline.push(period_end);
            timeline.sort();
            timeline.dedup();

            // Track balance through sub-periods. The replay routes every
            // event through `apply_draw_repay_event` so limit validation
            // (draw ≤ commitment, repay ≤ balance) also covers events dated
            // exactly on a period boundary — the in-period loop below only
            // sees strictly-interior events, so boundary-dated events would
            // otherwise bypass validation and let the balance exceed the
            // commitment (negative undrawn fees).
            let mut current_balance = if i == 0 {
                self.facility.drawn
            } else {
                self.facility
                    .drawn_balance_at(&draw_repay_events, anchor, period_start)?
            };

            // Accumulators for aggregated accruals
            let mut total_interest = Money::from((0_i64, ccy));
            let mut fees = FeeAccrual::new(ccy);
            let mut total_accrual = 0.0;
            let mut reset_date_opt: Option<Date> = None;

            // Track weighted average rates for this period
            let mut weighted_interest_rate = 0.0;
            // A posting inside the period closes an interest row: what was
            // earned before it is a row of its own, with its own accrual
            // dates, so accrued interest at a later date keeps it whole.
            let mut stretch_start = period_start;
            let mut emitted = (Money::from((0_i64, ccy)), 0.0_f64, 0.0_f64);
            let interest_kind = match &self.facility.rate {
                RateSpec::Fixed { .. } => CFKind::Fixed,
                RateSpec::Floating(_) => CFKind::Floating,
            };

            for window in timeline.windows(2) {
                let sub_start = window[0];
                let sub_end = window[1];

                let dt = self
                    .day_count
                    .year_fraction(sub_start, sub_end, accrual_context)?;
                total_accrual += dt;

                let commitment = self.facility.commitment_at(sub_start);
                let lc_outstanding = self.facility.lc_outstanding_at(sub_start);
                if current_balance.amount() + lc_outstanding.amount() > commitment.amount() + 1e-9 {
                    return Err(finstack_quant_core::Error::Validation(format!(
                        "RevolvingCredit drawn balance {} plus letters of credit {} exceeds the \
                         commitment {} in force on {sub_start}; schedule a repayment on or \
                         before the commitment step",
                        current_balance, lc_outstanding, commitment
                    )));
                }
                // Letters of credit consume availability and count as usage.
                let current_undrawn = commitment
                    .checked_sub(current_balance)?
                    .checked_sub(lc_outstanding)?;
                let utilization = if commitment.amount() > 0.0 {
                    (current_balance.amount() + lc_outstanding.amount()) / commitment.amount()
                } else {
                    0.0
                };

                // Determine reset date for floating rates
                let sub_reset_effective_date = match &self.facility.rate {
                    RateSpec::Floating(_) => {
                        if let Some(ref reset_grid) = self.reset_dates {
                            reset_grid
                                .iter()
                                .rev()
                                .find(|&&d| d <= sub_start)
                                .copied()
                                .or(Some(period_start))
                        } else {
                            Some(period_start)
                        }
                    }
                    RateSpec::Fixed { .. } => None,
                };

                if reset_date_opt.is_none() {
                    reset_date_opt = match (&self.facility.rate, sub_reset_effective_date) {
                        (RateSpec::Floating(spec), Some(date)) => {
                            Some(super::super::schedule_projection::floating_fixing_date(
                                spec,
                                date,
                                self.facility.calendar_id.as_deref(),
                            )?)
                        }
                        _ => None,
                    };
                }

                let interest_rate = match &self.facility.rate {
                    RateSpec::Fixed { rate } => {
                        let rate = super::super::schedule_projection::fixed_rate_at(
                            self.facility,
                            *rate,
                            sub_start,
                        );
                        let interest = current_balance.checked_mul_f64(rate * dt)?;
                        total_interest = total_interest.checked_add(interest)?;
                        rate
                    }
                    RateSpec::Floating(spec) => {
                        let params = super::super::schedule_projection::floating_params_at(
                            self.facility,
                            spec,
                            sub_start,
                        )?;
                        let reset_effective = sub_reset_effective_date.unwrap_or(period_start);
                        let fixing_date = super::super::schedule_projection::floating_fixing_date(
                            spec,
                            reset_effective,
                            self.facility.calendar_id.as_deref(),
                        )?;
                        let overnight =
                            super::super::schedule_projection::resolved_overnight_compounding(
                                spec,
                            )?;

                        let coupon_rate = if overnight.is_some() {
                            let fwd = fwd_curve.as_ref().ok_or_else(|| {
                                finstack_quant_core::Error::Validation(
                                    "forward curve required for floating rate".into(),
                                )
                            })?;
                            super::super::schedule_projection::project_revolver_floating_rate(
                                super::super::schedule_projection::RevolverFloatingProjection {
                                    accrual_start: sub_start,
                                    accrual_end: sub_end,
                                    as_of: self.as_of,
                                    spec,
                                    fwd: fwd.as_ref(),
                                    day_count: self.day_count,
                                    coupon_frequency: self.facility.frequency,
                                    coupon_period: (period_start, period_end),
                                    currency: ccy,
                                    calendar_id: self.facility.calendar_id.as_deref(),
                                    margin_delta_bp: self.facility.margin_delta_bp_at(sub_start),
                                    fixings: self.fixing_series,
                                },
                                Some(&mut projected_fixings),
                            )?
                        } else if fixing_date < self.as_of {
                            let fixing_rate =
                                finstack_quant_core::market_data::fixings::require_fixing_value_exact(
                                    self.fixing_series,
                                    spec.index_id.as_ref(),
                                    fixing_date,
                                )?;
                            projected_fixings.push(
                                finstack_quant_cashflows::fixings::ProjectedFixing {
                                    series_id: fixing_series_id(spec.index_id.as_str()),
                                    date: fixing_date,
                                    value: Some(fixing_rate),
                                },
                            );
                            finstack_quant_cashflows::builder::calculate_floating_rate(
                                fixing_rate,
                                &params,
                            )
                        } else {
                            let fwd = fwd_curve.as_ref().ok_or_else(|| {
                                finstack_quant_core::Error::Validation(
                                    "forward curve required for floating rate".into(),
                                )
                            })?;
                            let (obs_start, obs_end, obs_accrual) =
                                self.term_reset_window(period, reset_effective)?;
                            let index_rate = finstack_quant_cashflows::builder::project_index_rate(
                                reset_effective,
                                fwd.as_ref(),
                                obs_start,
                                obs_end,
                                obs_accrual,
                            )?;
                            projected_fixings.push(
                                finstack_quant_cashflows::fixings::ProjectedFixing {
                                    series_id: fixing_series_id(spec.index_id.as_str()),
                                    date: fixing_date,
                                    value: Some(index_rate),
                                },
                            );
                            finstack_quant_cashflows::builder::calculate_floating_rate(
                                index_rate, &params,
                            )
                        };

                        let interest = current_balance.checked_mul_f64(coupon_rate * dt)?;
                        total_interest = total_interest.checked_add(interest)?;
                        coupon_rate
                    }
                };
                weighted_interest_rate += interest_rate * dt;

                fees.accrue(
                    self.facility,
                    utilization,
                    sub_start,
                    &FeeBalances {
                        drawn: current_balance,
                        undrawn: current_undrawn,
                        commitment,
                        lc_outstanding,
                    },
                    dt,
                )?;

                // Apply events at sub_end (but not at period_end - those happen after interest)
                if sub_end != period_end {
                    for event in draw_repay_events.iter() {
                        if event.date == sub_end {
                            current_balance =
                                super::super::schedule_projection::apply_draw_repay_event(
                                    current_balance,
                                    event,
                                    self.facility.commitment_at(event.date),
                                )?;
                        }
                    }
                    if self
                        .postings
                        .iter()
                        .any(|posting| posting.effective_date == sub_end)
                    {
                        let interest = total_interest.checked_sub(emitted.0)?;
                        let accrual = total_accrual - emitted.1;
                        let weighted = weighted_interest_rate - emitted.2;
                        if !rc.is_effectively_zero(interest.amount(), ZeroKind::Money(ccy)) {
                            flows.push(
                                CashFlowParams::new(
                                    payment_date,
                                    reset_date_opt,
                                    interest,
                                    interest_kind,
                                    accrual,
                                    (accrual > 0.0).then(|| weighted / accrual),
                                )
                                .with_accrual(finstack_quant_core::cashflow::CashFlowAccrual::new(
                                    stretch_start,
                                    sub_end,
                                    self.day_count,
                                    None,
                                    accrual_context.try_into()?,
                                )?)
                                .build()?,
                            );
                        }
                        emitted = (total_interest, total_accrual, weighted_interest_rate);
                        stretch_start = sub_end;
                    }
                }
            }

            // The interest row of a period no posting split is unchanged.
            if stretch_start != period_start {
                let interest = total_interest.checked_sub(emitted.0)?;
                let accrual = total_accrual - emitted.1;
                let weighted = weighted_interest_rate - emitted.2;
                if !rc.is_effectively_zero(interest.amount(), ZeroKind::Money(ccy)) {
                    flows.push(
                        CashFlowParams::new(
                            payment_date,
                            reset_date_opt,
                            interest,
                            interest_kind,
                            accrual,
                            (accrual > 0.0).then(|| weighted / accrual),
                        )
                        .with_accrual(finstack_quant_core::cashflow::CashFlowAccrual::new(
                            stretch_start,
                            period_end,
                            self.day_count,
                            None,
                            accrual_context.try_into()?,
                        )?)
                        .build()?,
                    );
                }
                fees.emit(&mut flows, payment_date, total_accrual, &rc)?;
                continue;
            }

            // Post aggregated cashflows at period_end
            // For revolving credit with intra-period events, we use time-weighted average rates
            // The rate_base is the period start balance, so the formula amount = rate_base × rate × accrual
            // is approximate when there are draws/repays during the period
            let avg_interest_rate = if total_accrual > 0.0 {
                Some(weighted_interest_rate / total_accrual)
            } else {
                None
            };
            let money_zero = finstack_quant_core::config::ZeroKind::Money(ccy);
            if !rc.is_effectively_zero(total_interest.amount(), money_zero) {
                flows.push(
                    CashFlowParams::new(
                        payment_date,
                        reset_date_opt,
                        total_interest,
                        match &self.facility.rate {
                            RateSpec::Fixed { .. } => CFKind::Fixed,
                            RateSpec::Floating(_) => CFKind::Floating,
                        },
                        total_accrual,
                        avg_interest_rate,
                    )
                    .build()?,
                );
            }

            fees.emit(&mut flows, payment_date, total_accrual, &rc)?;
        }

        // A posting is a zero-cash principal row: its cash settled outside
        // the schedule.
        for posting in self.postings {
            flows.push(
                CashFlowParams::new(
                    posting.effective_date,
                    None,
                    Money::from((0_i64, ccy)),
                    CFKind::PrePayment,
                    0.0,
                    None,
                )
                .with_principal_delta(posting.delta)
                .with_principal_date(posting.effective_date)
                .build()?,
            );
        }

        // Add principal flows from draw/repay events
        for event in contract_events.iter() {
            if event.date > self.as_of {
                flows.push({
                    // The par change equals the cash (record CORE-CONVENTIONS-015 #4).
                    let cash = if event.is_draw {
                        event.amount.checked_neg()
                    } else {
                        event.amount
                    };
                    CashFlowParams::new(event.date, None, cash, CFKind::Notional, 0.0, None)
                        .with_principal_delta(cash.checked_neg())
                        .with_principal_date(event.date)
                        .build()?
                });
            }
        }

        self.push_scheduled_fees(&mut flows)?;

        // Add terminal repayment. Same validated replay as the period
        // balances — maturity-dated events are boundary events too.
        let final_balance_for_terminal =
            self.facility
                .drawn_balance_at(&draw_repay_events, anchor, self.facility.maturity)?;

        let terminal_payment_date = self
            .payment_periods
            .last()
            .map(|period| period.payment_date)
            .ok_or(finstack_quant_core::InputError::TooFewPoints)?;
        if terminal_payment_date > self.as_of
            && !rc.is_effectively_zero(final_balance_for_terminal.amount(), ZeroKind::Money(ccy))
        {
            flows.push({
                // The par change equals the cash (record CORE-CONVENTIONS-015 #4).
                let cash = final_balance_for_terminal;
                CashFlowParams::new(
                    terminal_payment_date,
                    None,
                    cash,
                    CFKind::Notional,
                    0.0,
                    None,
                )
                .with_principal_delta(cash.checked_neg())
                .with_principal_date(terminal_payment_date)
                .build()?
            });
        }

        Ok(finstack_quant_cashflows::schedule_from_classified_flows(
            flows,
            self.facility.day_count,
            finstack_quant_cashflows::ScheduleBuildOpts {
                notional_hint: Some(if self.facility.issue_date <= self.as_of {
                    self.facility.drawn
                } else {
                    Money::from((0_i64, self.facility.commitment.currency()))
                }),
                meta: finstack_quant_cashflows::builder::CashFlowMeta {
                    projected_fixings,
                    ..self.schedule_meta()
                },
            },
        ))
    }
}
