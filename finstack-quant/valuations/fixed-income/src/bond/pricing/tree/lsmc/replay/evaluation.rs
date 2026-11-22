//! Decision snapshots, full/block path replay, accrual state, and exercise valuation.

use super::super::*;

impl ReplayTemplate {
    pub(in super::super) fn decision_snapshot(
        &self,
        bond: &Bond,
        step: usize,
        path: &[RatesCreditPathState],
        state: StepReplay,
        floating: &[FloatingCouponReplayState],
        make_whole_policies: Option<&[RegressionPolicy]>,
    ) -> Result<DecisionSnapshot> {
        let representative_date = self.exercise[step]
            .first()
            .map(|entry| entry.date)
            .or_else(|| self.step_dates.get(step).copied().flatten())
            .unwrap_or(bond.maturity);
        let accrued = self.accrued_state(representative_date, floating)?;
        let mut locked_coupon = 0.0;
        for (id, coupon) in self.floating.iter().enumerate() {
            if matches!(&coupon.rate_model, FloatingRateModel::Term(_))
                && coupon.reset_step <= step
                && step < coupon.payment_step.max(coupon.effective_end_step)
                && floating[id].notional().is_some()
                && floating[id].term_index_rate().is_some()
            {
                let settlement = coupon.compiled.settle(&floating[id])?;
                if step < coupon.payment_step {
                    locked_coupon += settlement.cash_amount;
                }
                if step < coupon.effective_end_step {
                    locked_coupon += settlement.pik_amount;
                }
            }
        }
        let mut redemption_outstanding = state.outstanding;
        if self.redemption_step == Some(step) {
            redemption_outstanding += self.static_redemption_pik;
            for (id, coupon) in self.floating.iter().enumerate() {
                if coupon.payment_step <= step && step < coupon.effective_end_step {
                    redemption_outstanding += coupon.settlement(&floating[id], path)?.pik_amount;
                }
            }
        }
        let factor = path_state(path, step)?;
        let (call, put) = self.exercise_amounts(ExerciseInputs {
            bond,
            step,
            path,
            outstanding: redemption_outstanding,
            locked_coupon,
            coupon_states: floating,
            cumulative_distribution_cash: state.cumulative_distribution_cash,
            cumulative_distribution_target_pv: state.cumulative_distribution_target_pv,
            make_whole_policies,
        })?;
        Ok(DecisionSnapshot {
            step,
            features: [
                factor.short_rate,
                factor.hazard_rate,
                state.outstanding,
                locked_coupon,
                accrued.0,
                accrued.1,
                state.cumulative_distribution_cash,
                state.cumulative_distribution_target_pv,
            ],
            current_cash: state.current_cash,
            hold_redemption: if self.redemption_step == Some(step) {
                redemption_outstanding
            } else {
                0.0
            },
            call,
            put,
            friction: state.outstanding * (self.call_friction_cents / 10_000.0),
            a_to_next: 0.0,
            b_to_next: 0.0,
        })
    }

    /// Replay decisions `low_decision..=high_decision` of one training path,
    /// identified by its stream index and antithetic flag, from `checkpoint`.
    pub(in super::super) fn replay_block(
        &self,
        run: TrainingRun<'_>,
        (path_index, antithetic): (u64, bool),
        checkpoint: &TrainingCheckpoint,
        (low_decision, high_decision): (usize, usize),
        make_whole_policies: Option<&[RegressionPolicy]>,
        path: &mut Vec<RatesCreditPathState>,
    ) -> Result<BlockPathRecord> {
        let TrainingRun {
            tree,
            bond,
            config,
            seed,
            terms,
        } = run;
        let low_step = self.decision_steps[low_decision];
        let high_step = self.decision_steps[high_decision];
        tree.sample_path_segment_into(
            seed,
            path_index,
            antithetic,
            checkpoint.factor,
            high_step,
            path,
        )?;
        let mut cursor = ReplayCursor::from_checkpoint(self, &checkpoint.product)?;
        let mut step_states = Vec::with_capacity(if terms.product_is_static() {
            0
        } else {
            high_step - low_step + 1
        });
        let mut decision_snapshots = Vec::with_capacity(high_decision - low_decision + 1);
        let mut next_decision = low_decision;
        for step in low_step..=high_step {
            let state = terms.advance(&mut cursor, self, bond, path, config, step)?;
            if !terms.product_is_static() {
                step_states.push(state);
            }
            if next_decision <= high_decision && self.decision_steps[next_decision] == step {
                decision_snapshots.push(self.decision_snapshot(
                    bond,
                    step,
                    path,
                    state,
                    &cursor.floating,
                    make_whole_policies,
                )?);
                next_decision += 1;
            }
        }
        if decision_snapshots.len() != high_decision - low_decision + 1 {
            return Err(Error::Internal(
                "bond hazard LSMC block replay missed a decision snapshot".into(),
            ));
        }
        for (local, snapshot) in decision_snapshots
            .iter_mut()
            .take(high_decision - low_decision)
            .enumerate()
        {
            let from = self.decision_steps[low_decision + local];
            let to = self.decision_steps[low_decision + local + 1];
            let mut a = 1.0;
            let mut b = 0.0;
            for step in from..to {
                let factor = path_state(path, step)?;
                let state = terms.step_state(&step_states, low_step, step);
                let interval = terms.interval(factor)?;
                let recovery = self.recovery_rate * state.outstanding * interval.frp_weight;
                let cash = if step > from { state.current_cash } else { 0.0 };
                b += a * (recovery + cash);
                a *= interval.risky_df;
            }
            snapshot.a_to_next = a;
            snapshot.b_to_next = b;
        }
        let terminal = (high_decision + 1 == self.decision_steps.len())
            .then(|| decision_snapshots.pop())
            .flatten();
        if terminal.is_none() {
            decision_snapshots.pop();
        }
        Ok(BlockPathRecord {
            snapshots: decision_snapshots,
            terminal,
            step_states,
        })
    }

    pub(in super::super) fn replay(
        &self,
        bond: &Bond,
        path: &[RatesCreditPathState],
        config: &BondLsmcConfig,
        make_whole_policies: Option<&[RegressionPolicy]>,
        terms: &ReplayTerms,
        buffers: &mut ReplayBuffers,
    ) -> Result<PathRecord> {
        if path.len() != self.times.len() {
            return Err(Error::Validation(format!(
                "bond hazard LSMC path has {} states but calibrated grid has {}",
                path.len(),
                self.times.len()
            )));
        }
        let mut cursor = ReplayCursor::new(self)?;
        let ReplayBuffers {
            step_cash,
            outstanding_after_events,
        } = buffers;
        step_cash.clear();
        step_cash.resize(self.times.len(), 0.0);
        outstanding_after_events.clear();
        outstanding_after_events.resize(self.times.len(), 0.0);
        let mut snapshots = Vec::with_capacity(self.decision_steps.len());
        let mut next_decision = 0_usize;

        for step in 0..self.times.len() {
            let state = terms.advance(&mut cursor, self, bond, path, config, step)?;
            step_cash[step] = state.current_cash;
            outstanding_after_events[step] = state.outstanding;
            if next_decision < self.decision_steps.len()
                && self.decision_steps[next_decision] == step
            {
                snapshots.push(self.decision_snapshot(
                    bond,
                    step,
                    path,
                    state,
                    &cursor.floating,
                    make_whole_policies,
                )?);
                next_decision += 1;
            }
        }

        if snapshots.len() != self.decision_steps.len() {
            return Err(Error::Internal(
                "bond hazard LSMC full replay missed a decision snapshot".into(),
            ));
        }

        for decision in 0..snapshots.len().saturating_sub(1) {
            let from = snapshots[decision].step;
            let to = snapshots[decision + 1].step;
            let mut a = 1.0;
            let mut b = 0.0;
            for step in (from..to).rev() {
                let interval = terms.interval(&path[step])?;
                let q = interval.risky_df;
                let recovery =
                    self.recovery_rate * outstanding_after_events[step] * interval.frp_weight;
                a *= q;
                b = q * b + recovery;
                if step > from {
                    b += step_cash[step];
                }
            }
            snapshots[decision].a_to_next = a;
            snapshots[decision].b_to_next = b;
        }
        Ok(PathRecord { snapshots })
    }

    pub(in super::super) fn accrued_state(
        &self,
        date: Date,
        coupon_states: &[FloatingCouponReplayState],
    ) -> Result<(f64, f64)> {
        let mut cash = 0.0;
        let mut pik = 0.0;
        let redemption_date = self
            .redemption_step
            .and_then(|step| self.step_dates.get(step).copied().flatten());
        for claim in &self.static_accruals {
            let settlement_date = if claim.pik {
                redemption_date.map_or(claim.end, |redemption| claim.end.min(redemption))
            } else {
                claim.payment
            };
            if date <= claim.start || date >= settlement_date {
                continue;
            }
            let elapsed_end = date.min(claim.end);
            let ratio = if claim.day_count == DayCount::Act365L {
                // Preserve the emitted coupon's contractual leap denominator.
                safe_accrual_ratio(
                    (elapsed_end - claim.start).whole_days() as f64,
                    (claim.end - claim.start).whole_days() as f64,
                )
            } else {
                let mut elapsed_context = claim.day_count_context;
                elapsed_context.end_is_termination_date = elapsed_context
                    .end_is_termination_date
                    .map(|terminal| terminal && elapsed_end == claim.end);
                let elapsed =
                    claim
                        .day_count
                        .year_fraction(claim.start, elapsed_end, elapsed_context)?;
                let total = claim.day_count.year_fraction(
                    claim.start,
                    claim.end,
                    claim.day_count_context,
                )?;
                safe_accrual_ratio(elapsed, total)
            };
            let amount = claim.amount * ratio;
            if claim.pik {
                pik += amount;
            } else {
                cash += amount;
            }
        }
        for (id, coupon) in self.floating.iter().enumerate() {
            let period = coupon.compiled.period();
            let cash_active = date < period.payment_date && coupon.pays_cash;
            let pik_active = date < period.accrual_end
                && redemption_date.is_none_or(|redemption| date < redemption)
                && coupon.capitalizes;
            if date <= period.accrual_start || (!cash_active && !pik_active) {
                continue;
            }
            let accrued_amount = coupon.compiled.accrued_amount(&coupon_states[id], date)?;
            if cash_active {
                cash += accrued_amount * coupon.compiled.economics().cash_fraction;
            }
            if pik_active {
                pik += accrued_amount * coupon.compiled.economics().pik_fraction;
            }
        }
        Ok((cash, pik))
    }

    pub(in super::super) fn exercise_amounts(
        &self,
        inputs: ExerciseInputs<'_>,
    ) -> Result<(Option<f64>, Option<f64>)> {
        let ExerciseInputs {
            bond,
            step,
            path,
            outstanding,
            locked_coupon,
            coupon_states,
            cumulative_distribution_cash,
            cumulative_distribution_target_pv,
            make_whole_policies,
        } = inputs;
        let mut call: Option<f64> = None;
        let mut put: Option<f64> = None;
        let factor = path_state(path, step)?;
        for entry in &self.exercise[step] {
            let (accrued_cash, accrued_pik) = self.accrued_state(entry.date, coupon_states)?;
            let accrued = accrued_cash + accrued_pik;
            let return_floor = if entry.return_floor {
                Some(
                    self.return_floor
                        .ok_or_else(|| {
                            Error::Internal(
                                "bond hazard LSMC return-floor date has no floor specification"
                                    .into(),
                            )
                        })?
                        .redemption(
                            entry.date,
                            outstanding,
                            cumulative_distribution_cash,
                            cumulative_distribution_target_pv,
                            accrued,
                        )?,
                )
            } else {
                None
            };
            let default_call = if entry.calls.is_empty() {
                return_floor.map(|floor| floor + accrued)
            } else {
                entry
                    .calls
                    .iter()
                    .map(|option| {
                        let contractual = outstanding * option.price_pct_of_par / 100.0;
                        let make_whole_dirty = match &option.make_whole {
                            None => contractual,
                            Some(MakeWholeExercise::Deterministic(value)) => *value,
                            Some(MakeWholeExercise::Conditional(claim_id)) => {
                                let Some(policies) = make_whole_policies else {
                                    return Ok(contractual);
                                };
                                let policy = policies.get(*claim_id).ok_or_else(|| {
                                    Error::Internal(format!(
                                        "bond hazard LSMC has no make-whole policy for claim {claim_id}"
                                    ))
                                })?;
                                policy.predict(&[
                                    factor.short_rate,
                                    0.0,
                                    outstanding,
                                    locked_coupon,
                                    accrued_cash,
                                    accrued_pik,
                                    cumulative_distribution_cash,
                                    cumulative_distribution_target_pv,
                                ])?
                            }
                        };
                        Ok(contractual
                            .max((make_whole_dirty - accrued).max(0.0))
                            .max(return_floor.unwrap_or(0.0))
                            + accrued)
                    })
                    .collect::<Result<Vec<_>>>()?
                    .into_iter()
                    .reduce(f64::min)
            };
            let default_put = entry
                .puts
                .iter()
                .map(|option| outstanding * option.price_pct_of_par / 100.0 + accrued)
                .reduce(f64::max);
            validate_exercise_amount("call", default_call)?;
            validate_exercise_amount("put", default_put)?;
            if let Some(value) = default_call {
                call = Some(call.map_or(value, |existing| existing.min(value)));
            }
            if let Some(value) = default_put {
                put = Some(put.map_or(value, |existing| existing.max(value)));
            }
        }
        if let (Some(call), Some(put)) = (call, put)
            && put > call + 1.0e-10
        {
            return Err(Error::Validation(format!(
                "Bond '{}' has incompatible path-dependent barriers at step {step}: holder put {put} exceeds issuer call {call}",
                bond.id.as_str()
            )));
        }
        Ok((call, put))
    }
}
