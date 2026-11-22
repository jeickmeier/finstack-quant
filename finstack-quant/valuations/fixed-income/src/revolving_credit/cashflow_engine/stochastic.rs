//! Three-factor stochastic path cashflow schedule builder for [`CashflowEngine`].

use super::super::types::DrawRepaySpec;
use super::fees::{FeeAccrual, FeeBalances};
use super::{CashflowEngine, ThreeFactorPathData};
use finstack_quant_cashflows::builder::{CashFlowSchedule, RateSpec};
use finstack_quant_core::Result;
use finstack_quant_core::cashflow::{CFKind, CashFlowParams};
use finstack_quant_core::config::{RoundingContext, ZeroKind};
use finstack_quant_core::dates::{Date, DayCount, DayCountContext};
use finstack_quant_core::money::Money;

impl<'a> CashflowEngine<'a> {
    /// Build cashflow schedule from 3-factor path trajectory.
    ///
    /// The path is observed on the facility's observation grid (contractual
    /// accrual boundaries plus term-index reset dates). Utilization is read at
    /// each accrual boundary; a term-index coupon is re-fixed at every reset
    /// date inside the period, so reset frequencies shorter than the payment
    /// frequency are honoured exactly as in the deterministic engine.
    pub(super) fn build_path_schedule(
        &self,
        path: &ThreeFactorPathData,
    ) -> Result<CashFlowSchedule> {
        let observation_index = |date: Date| -> Result<usize> {
            path.payment_dates.binary_search(&date).map_err(|_| {
                finstack_quant_core::Error::Validation(format!(
                    "RevolvingCredit path observation grid does not contain the date {date}"
                ))
            })
        };
        let mut flows = Vec::new();
        let rc = RoundingContext::default();
        let ccy = self.facility.commitment.currency();

        // Overnight facilities compound the forward curve over the whole
        // period. Term-index facilities on a stochastic (Hull-White) short-rate
        // path rebuild the index fixing as `F_index(t) + (r_t − f_OIS(t))`: the
        // simulated rate is the OIS numeraire rate, so the deterministic
        // index-over-OIS basis must be added back or the index forward curve
        // is silently ignored.
        let (overnight_fwd, term_basis_curves) = match &self.facility.rate {
            RateSpec::Floating(spec) => {
                let overnight =
                    super::super::utils::resolved_overnight_compounding(spec)?.is_some();
                match self.market {
                    Some(market) if overnight => (
                        Some(market.get_forward(spec.forward_curve_id.as_str())?),
                        None,
                    ),
                    Some(market) if path.stochastic_rates => (
                        None,
                        Some((
                            market.get_forward(spec.forward_curve_id.as_str())?,
                            market.get_discount(self.facility.discount_curve_id.as_str())?,
                        )),
                    ),
                    None if path.stochastic_rates => {
                        return Err(finstack_quant_core::Error::Validation(format!(
                            "Market context required to project the '{}' index over a \
                             stochastic short-rate path",
                            spec.forward_curve_id
                        )));
                    }
                    Some(_) | None => (None, None),
                }
            }
            RateSpec::Fixed { .. } => (None, None),
        };
        let deterministic_forward_mode = !path.stochastic_rates
            && matches!(
                &self.facility.draw_repay_spec,
                DrawRepaySpec::Stochastic(spec)
                    if spec.mc_config.as_ref().and_then(|config| config.interest_rate_process.as_ref()).is_none()
            );
        let deterministic_fwd = match (&self.facility.rate, deterministic_forward_mode) {
            (RateSpec::Floating(spec), true) if overnight_fwd.is_none() => {
                let market = self.market.ok_or_else(|| {
                    finstack_quant_core::Error::Validation(
                        "Market context required for a deterministic-forward revolving-credit path"
                            .into(),
                    )
                })?;
                Some(market.get_forward(spec.forward_curve_id.as_str())?)
            }
            _ => None,
        };

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

        let anchor = self.facility.issue_date.max(self.as_of);
        self.check_anchor_capacity(anchor)?;

        // Running drawn balance at the last observation, for the principal
        // leg: `C(t) · u(t)` where `C` is the commitment in force. The first
        // observation on or before the valuation date records the t₀ state,
        // which the path generator seeds from `drawn`.
        let mut prev_balance = if path.payment_dates[0] <= self.as_of {
            self.facility.commitment_at(anchor).amount() * path.utilization_path[0].clamp(0.0, 1.0)
        } else {
            self.facility.drawn.amount()
        };

        // Process each contractual accrual period. Interest and fees accrue
        // per observation sub-interval (accrual boundaries, term-index reset
        // dates and commitment/margin/fee step dates) on the average of the
        // start and end utilization of that sub-interval, so every reset and
        // every dated step inside a period is honoured; the aggregates post
        // on the adjusted payment date. Payment adjustment changes
        // settlement, never the accrual interval.
        for period in self.payment_periods.iter() {
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
            let idx_start = observation_index(period_start)?;
            let idx_end = observation_index(period_end)?;

            let mut interest = Money::from((0_i64, ccy));
            let mut fees = FeeAccrual::new(ccy);
            let mut accrual = 0.0;
            let mut weighted_rate = 0.0;
            let mut first_fixing: Option<Date> = None;
            // Overnight all-in rate for the period and the margin it was
            // projected with.
            let mut overnight_rate: Option<(f64, f64)> = None;

            // Commitment steps inside the period move the balance on their own
            // date (they are observation dates); only the utilization-driven
            // change is booked at the period midpoint below.
            let mut step_legs = 0.0;
            for k in idx_start..idx_end {
                let sub_start = path.payment_dates[k];
                let sub_end = path.payment_dates[k + 1];
                let sub_dt = self
                    .day_count
                    .year_fraction(sub_start, sub_end, accrual_context)?;
                let utilization_start = path.utilization_path[k].clamp(0.0, 1.0);
                let utilization_end = path.utilization_path[k + 1].clamp(0.0, 1.0);
                let commitment_change = self.facility.commitment_at(sub_end).amount()
                    - self.facility.commitment_at(sub_start).amount();
                if commitment_change != 0.0 && sub_end > self.as_of {
                    let leg = commitment_change * utilization_end;
                    if leg.abs() > 0.0 {
                        // Draw (increase) is negative for lender, repay (decrease) is positive
                        flows.push({
                            // The par change equals the cash (record CORE-CONVENTIONS-015 #4).
                            let cash = Money::from_model_value(-leg, ccy)?;
                            CashFlowParams::new(sub_end, None, cash, CFKind::Notional, 0.0, None)
                                .with_principal_delta(cash.checked_neg())
                                .with_principal_date(sub_end)
                                .build()?
                        });
                        step_legs += leg;
                    }
                }
                // Average utilization over the sub-interval: the unbiased
                // accrual base for a change occurring uniformly within it.
                let avg_utilization = (utilization_start + utilization_end) / 2.0;
                let commitment = self.facility.commitment_at(sub_start);
                let lc_outstanding = self.facility.lc_outstanding_at(sub_start);
                let drawn_balance = commitment.checked_mul_f64(avg_utilization)?;
                // Letters of credit consume availability; the utilization
                // process is capped so this stays non-negative.
                let undrawn_balance = Money::from_model_value(
                    (commitment.amount() * (1.0 - avg_utilization) - lc_outstanding.amount())
                        .max(0.0),
                    ccy,
                )?;
                // Fee tiers see letters of credit as usage.
                let tier_utilization = if commitment.amount() > 0.0 {
                    (avg_utilization + lc_outstanding.amount() / commitment.amount()).min(1.0)
                } else {
                    avg_utilization
                };

                // Contractual fixings override the simulated short rate once
                // the fixing date has passed. Future reset dates remain
                // stochastic.
                let coupon_rate = match &self.facility.rate {
                    RateSpec::Fixed { rate } => {
                        super::super::utils::fixed_rate_at(self.facility, *rate, sub_start)
                    }
                    RateSpec::Floating(spec) => {
                        let reset_effective = self.reset_effective_at(sub_start);
                        let fixing_date = super::super::utils::floating_fixing_date(
                            spec,
                            reset_effective,
                            self.facility.calendar_id.as_deref(),
                        )?;
                        first_fixing.get_or_insert(fixing_date);
                        if let Some(fwd) = overnight_fwd.as_ref() {
                            // Overnight index: compound the forward over the
                            // whole accrual period once; only the margin in
                            // force differs between sub-intervals.
                            let margin_delta_bp = self.facility.margin_delta_bp_at(sub_start);
                            match overnight_rate {
                                Some((margin, rate))
                                    if margin.to_bits() == margin_delta_bp.to_bits() =>
                                {
                                    rate
                                }
                                _ => {
                                    let rate = super::super::utils::project_revolver_floating_rate(
                                        super::super::utils::RevolverFloatingProjection {
                                            accrual_start: period_start,
                                            accrual_end: period_end,
                                            as_of: self.as_of,
                                            spec,
                                            fwd: fwd.as_ref(),
                                            day_count: self.day_count,
                                            coupon_frequency: self.facility.frequency,
                                            coupon_period: (period_start, period_end),
                                            currency: ccy,
                                            calendar_id: self.facility.calendar_id.as_deref(),
                                            margin_delta_bp,
                                            fixings: self.fixing_series,
                                        },
                                        None,
                                    )?;
                                    overnight_rate = Some((margin_delta_bp, rate));
                                    rate
                                }
                            }
                        } else {
                            let params = super::super::utils::floating_params_at(
                                self.facility,
                                spec,
                                sub_start,
                            )?;
                            let base_rate = if fixing_date < self.as_of {
                                finstack_quant_core::market_data::fixings::require_fixing_value_exact(
                                    self.fixing_series,
                                    spec.index_id.as_ref(),
                                    fixing_date,
                                )?
                            } else {
                                let simulated =
                                    path.short_rate_path[observation_index(reset_effective)?];
                                match term_basis_curves.as_ref() {
                                    Some((fwd, disc)) => {
                                        let (obs_start, obs_end, obs_accrual) =
                                            self.term_reset_window(period, reset_effective)?;
                                        simulated
                                            + super::super::utils::index_basis_at(
                                                reset_effective,
                                                anchor,
                                                fwd.as_ref(),
                                                disc.as_ref(),
                                                obs_start,
                                                obs_end,
                                                obs_accrual,
                                            )?
                                    }
                                    None => match deterministic_fwd.as_ref() {
                                        Some(fwd) => {
                                            let (obs_start, obs_end, obs_accrual) =
                                                self.term_reset_window(period, reset_effective)?;
                                            let contractual_index =
                                                finstack_quant_cashflows::builder::project_index_rate(
                                                    reset_effective,
                                                    fwd.as_ref(),
                                                    obs_start,
                                                    obs_end,
                                                    obs_accrual,
                                                )?;
                                            let reset_time = fwd.day_count().signed_year_fraction(
                                                fwd.base_date(),
                                                reset_effective,
                                                fwd.day_count_context(),
                                            )?;
                                            // Replace the raw curve baseline while preserving
                                            // any additive contractual-rate shock on the path.
                                            simulated + contractual_index
                                                - fwd.instantaneous_forward(reset_time.max(0.0))?
                                        }
                                        None => simulated,
                                    },
                                }
                            };
                            finstack_quant_cashflows::builder::calculate_floating_rate(
                                base_rate, &params,
                            )
                        }
                    }
                };

                interest =
                    interest.checked_add(drawn_balance.checked_mul_f64(coupon_rate * sub_dt)?)?;
                weighted_rate += coupon_rate * sub_dt;
                accrual += sub_dt;

                // Fees on the same average balances; tiers evaluated on the
                // sub-interval's average utilization to avoid boundary
                // artifacts.
                fees.accrue(
                    self.facility,
                    tier_utilization,
                    sub_start,
                    &FeeBalances {
                        drawn: drawn_balance,
                        undrawn: undrawn_balance,
                        commitment,
                        lc_outstanding,
                    },
                    sub_dt,
                )?;
            }

            if payment_date > self.as_of {
                if !rc.is_effectively_zero(interest.amount(), ZeroKind::Money(ccy)) {
                    flows.push(
                        CashFlowParams::new(
                            payment_date,
                            first_fixing,
                            interest,
                            match &self.facility.rate {
                                RateSpec::Fixed { .. } => CFKind::Fixed,
                                RateSpec::Floating(_) => CFKind::Floating,
                            },
                            accrual,
                            (accrual > 0.0).then(|| weighted_rate / accrual),
                        )
                        .build()?,
                    );
                }
                fees.emit(&mut flows, payment_date, accrual, &rc)?;
            }

            // Principal leg: the change of the drawn balance `C(t) · u(t)`
            // over the period, booked at the midpoint of the simulated
            // interval (`[period start, period end]`, or `[as_of, period end]`
            // for the period containing the valuation date). Interest uses
            // average utilization, so the midpoint is the matching, unbiased
            // funding date for a change occurring uniformly within the
            // period; a commitment step inside the period moves the balance
            // through the same leg. This intentionally differs from the
            // deterministic engine, which posts principal on contractual
            // event dates.
            let end_balance = self.facility.commitment_at(period_end).amount()
                * path.utilization_path[idx_end].clamp(0.0, 1.0);
            if period_end <= self.as_of {
                // History: the anchor balance is the known state, nothing to book.
                prev_balance = self.facility.drawn.amount();
                continue;
            }
            let balance_change = end_balance - prev_balance - step_legs;
            let simulated_from = period_start.max(self.as_of);
            let half_days = ((period_end - simulated_from).whole_days() / 2).max(1);
            let principal_date = simulated_from + time::Duration::days(half_days);
            // Every draw and repayment is funded; a change below a cent is
            // carried into the next period rather than dropped, so the funding
            // leg nets exactly to the terminal repayment.
            if !rc.is_effectively_zero(balance_change, ZeroKind::Money(ccy)) {
                // Draw (increase) is negative for lender, repay (decrease) is positive
                flows.push({
                    // The par change equals the cash (record CORE-CONVENTIONS-015 #4).
                    let cash = Money::from_model_value(-balance_change, ccy)?;
                    CashFlowParams::new(principal_date, None, cash, CFKind::Notional, 0.0, None)
                        .with_principal_delta(cash.checked_neg())
                        .with_principal_date(principal_date)
                        .build()?
                });
                prev_balance = end_balance;
            } else {
                prev_balance += step_legs;
            }
        }

        self.push_scheduled_fees(&mut flows)?;

        // Terminal repayment of outstanding balance
        let final_balance = Money::from_model_value(prev_balance, ccy)?;

        let terminal_payment_date = self
            .payment_periods
            .last()
            .map(|period| period.payment_date)
            .ok_or(finstack_quant_core::InputError::TooFewPoints)?;
        if terminal_payment_date > self.as_of
            && !rc.is_effectively_zero(final_balance.amount(), ZeroKind::Money(ccy))
        {
            flows.push({
                // The par change equals the cash (record CORE-CONVENTIONS-015 #4).
                let cash = final_balance;
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
                notional_hint: Some(Money::from((0_i64, self.facility.commitment.currency()))),
                meta: self.schedule_meta(),
            },
        ))
    }
}
