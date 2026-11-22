//! Live path cursor stepping and checkpointing for bond hazard LSMC.

use super::super::*;

impl FloatingCoupon {
    /// Resolve full settlement on a copy, preserving the live accrual endpoint.
    pub(in super::super) fn settlement(
        &self,
        state: &FloatingCouponReplayState,
        path: &[RatesCreditPathState],
    ) -> Result<finstack_quant_cashflows::builder::FloatingCouponSettlement> {
        let mut settlement_state = state.clone();
        if let FloatingRateModel::Overnight(overnight) = &self.rate_model {
            self.compiled.advance_overnight(
                &mut settlement_state,
                self.compiled.period().accrual_end,
                |slice| {
                    overnight
                        .sources
                        .get(&slice.observation_date)
                        .ok_or_else(|| {
                            Error::Internal(format!(
                                "bond hazard LSMC has no overnight source for {}",
                                slice.observation_date
                            ))
                        })?
                        .rate(path)
                },
            )?;
        }
        self.compiled.settle(&settlement_state)
    }
}

impl ReplayCursor {
    pub(in super::super) fn new(template: &ReplayTemplate) -> Result<Self> {
        let mut floating = Vec::with_capacity(template.floating.len());
        for coupon in &template.floating {
            let mut state = FloatingCouponReplayState::default();
            if let Some(rate) = coupon.initial_term_rate {
                coupon.compiled.observe_term(&mut state, rate)?;
            }
            if let Some(notional) = coupon.initial_notional {
                coupon.compiled.capture_notional(&mut state, notional)?;
            }
            floating.push(state);
        }
        Ok(Self {
            outstanding: template.initial_outstanding,
            cumulative_distribution_cash: template.historical_distribution_cash,
            cumulative_distribution_target_pv: template.historical_distribution_target_pv,
            floating,
        })
    }

    pub(in super::super) fn from_checkpoint(
        template: &ReplayTemplate,
        checkpoint: &ProductCheckpoint,
    ) -> Result<Self> {
        let mut floating = vec![FloatingCouponReplayState::default(); template.floating.len()];
        for live in &checkpoint.live_floating {
            let slot = floating.get_mut(live.coupon_id).ok_or_else(|| {
                Error::Internal(
                    "bond hazard LSMC checkpoint has an invalid floating coupon id".into(),
                )
            })?;
            *slot = live.state.clone();
        }
        Ok(Self {
            outstanding: checkpoint.outstanding,
            cumulative_distribution_cash: checkpoint.cumulative_distribution_cash,
            cumulative_distribution_target_pv: checkpoint.cumulative_distribution_target_pv,
            floating,
        })
    }

    pub(in super::super) fn checkpoint(&self) -> ProductCheckpoint {
        let live_floating = self
            .floating
            .iter()
            .enumerate()
            .filter(|(_, state)| state.is_live())
            .map(|(coupon_id, state)| LiveFloatingCheckpoint {
                coupon_id,
                state: state.clone(),
            })
            .collect::<SmallVec<_>>();
        ProductCheckpoint {
            outstanding: self.outstanding,
            cumulative_distribution_cash: self.cumulative_distribution_cash,
            cumulative_distribution_target_pv: self.cumulative_distribution_target_pv,
            live_floating,
        }
    }

    pub(in super::super) fn advance_overnight(
        &mut self,
        template: &ReplayTemplate,
        path: &[RatesCreditPathState],
        coupon_id: usize,
        end: Date,
    ) -> Result<()> {
        let coupon = template.floating.get(coupon_id).ok_or_else(|| {
            Error::Internal(
                "bond hazard LSMC overnight coupon id is outside the replay template".into(),
            )
        })?;
        let FloatingRateModel::Overnight(overnight) = &coupon.rate_model else {
            return Ok(());
        };
        let state = self.floating.get_mut(coupon_id).ok_or_else(|| {
            Error::Internal(
                "bond hazard LSMC overnight coupon state is outside the replay cursor".into(),
            )
        })?;
        coupon.compiled.advance_overnight(state, end, |slice| {
            overnight
                .sources
                .get(&slice.observation_date)
                .ok_or_else(|| {
                    Error::Internal(format!(
                        "bond hazard LSMC has no overnight source for {}",
                        slice.observation_date
                    ))
                })?
                .rate(path)
        })?;
        Ok(())
    }

    /// Settle the locked full coupon without changing its cash or balance dates.
    fn floating_settlement(
        &self,
        template: &ReplayTemplate,
        path: &[RatesCreditPathState],
        coupon_id: usize,
    ) -> Result<finstack_quant_cashflows::builder::FloatingCouponSettlement> {
        let coupon = &template.floating[coupon_id];
        // Early payment requires every remaining fixing to be known, which
        // template construction checks before replay starts.
        coupon.settlement(&self.floating[coupon_id], path)
    }

    pub(in super::super) fn advance_step(
        &mut self,
        template: &ReplayTemplate,
        bond: &Bond,
        path: &[RatesCreditPathState],
        config: &BondLsmcConfig,
        step: usize,
    ) -> Result<StepReplay> {
        let oas = config.oas_bp / 10_000.0;
        if let Some(date) = template.step_dates.get(step).copied().flatten() {
            for coupon_id in 0..template.floating.len() {
                let coupon = &template.floating[coupon_id];
                if self.floating[coupon_id].is_live()
                    && matches!(coupon.rate_model, FloatingRateModel::Overnight(_))
                {
                    self.advance_overnight(
                        template,
                        path,
                        coupon_id,
                        date.min(coupon.compiled.period().accrual_end),
                    )?;
                }
            }
        }

        let mut current_cash = 0.0;
        let mut reference_cash = 0.0;
        for event in &template.static_cash[step] {
            current_cash += event.amount_at_step * (-oas * event.event_minus_step).exp();
            reference_cash += event.amount_at_step;
        }
        for distribution in &template.static_distributions[step] {
            self.cumulative_distribution_cash += distribution.amount;
            if let Some(floor) = template.return_floor {
                self.cumulative_distribution_target_pv +=
                    floor.target_pv(distribution.date, distribution.amount)?;
            }
        }
        for &coupon_id in &template.floating_payment_ids[step] {
            let settlement = self.floating_settlement(template, path, coupon_id)?;
            let coupon = &template.floating[coupon_id];
            let cash_coupon = settlement.cash_amount;
            current_cash += cash_coupon;
            reference_cash += cash_coupon;
            self.cumulative_distribution_cash += cash_coupon.max(0.0);
            if let Some(floor) = template.return_floor {
                self.cumulative_distribution_target_pv +=
                    floor.target_pv(coupon.compiled.period().payment_date, cash_coupon.max(0.0))?;
            }
        }
        let mut pik_to_add = 0.0;
        for &coupon_id in &template.floating_balance_ids[step] {
            let settlement = self.floating_settlement(template, path, coupon_id)?;
            if settlement.pik_amount < 0.0 {
                return Err(Error::Validation(format!(
                    "Bond '{}' hazard LSMC cannot de-capitalize negative PIK on {}",
                    bond.id.as_str(),
                    template.floating[coupon_id].compiled.period().accrual_end
                )));
            }
            pik_to_add += settlement.pik_amount;
        }
        // Capitalize before same-day principal movements and next-period capture.
        self.outstanding += pik_to_add;
        for event in &template.balance_events[step] {
            self.outstanding += event.delta;
        }
        for &coupon_id in template.floating_payment_ids[step]
            .iter()
            .chain(&template.floating_balance_ids[step])
        {
            let coupon = &template.floating[coupon_id];
            let final_step = if coupon.capitalizes {
                coupon.payment_step.max(coupon.effective_end_step)
            } else {
                coupon.payment_step
            };
            if step == final_step {
                self.floating[coupon_id] = FloatingCouponReplayState::default();
            }
        }
        if self.outstanding < -1.0e-8 || !self.outstanding.is_finite() {
            return Err(Error::Validation(format!(
                "Bond '{}' hazard LSMC replay produced invalid outstanding principal {} at step {step}",
                bond.id.as_str(),
                self.outstanding
            )));
        }
        self.outstanding = self.outstanding.max(0.0);
        for &coupon_id in &template.floating_reset_ids[step] {
            let coupon = &template.floating[coupon_id];
            let state = &mut self.floating[coupon_id];
            match &coupon.rate_model {
                FloatingRateModel::Term(source) => {
                    let index_rate = source.rate(path)?;
                    coupon.compiled.observe_term(state, index_rate)?;
                }
                FloatingRateModel::Overnight(_) => {
                    return Err(Error::Internal(
                        "bond hazard LSMC overnight coupon has a term reset event".into(),
                    ));
                }
            }
        }
        for &coupon_id in &template.floating_accrual_start_ids[step] {
            let coupon = &template.floating[coupon_id];
            coupon
                .compiled
                .capture_notional(&mut self.floating[coupon_id], self.outstanding)?;
        }
        if let Some(date) = template.step_dates.get(step).copied().flatten() {
            for &coupon_id in &template.floating_accrual_start_ids[step] {
                let coupon = &template.floating[coupon_id];
                if matches!(coupon.rate_model, FloatingRateModel::Overnight(_)) {
                    self.advance_overnight(
                        template,
                        path,
                        coupon_id,
                        date.min(coupon.compiled.period().accrual_end),
                    )?;
                }
            }
        }
        Ok(StepReplay {
            current_cash,
            reference_cash,
            outstanding: self.outstanding,
            cumulative_distribution_cash: self.cumulative_distribution_cash,
            cumulative_distribution_target_pv: self.cumulative_distribution_target_pv,
        })
    }
}
