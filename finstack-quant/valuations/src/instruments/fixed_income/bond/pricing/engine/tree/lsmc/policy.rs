//! Regression policies: decision snapshots and least-squares continuation fits.

use super::*;

#[derive(Clone)]
pub(super) struct DecisionSnapshot {
    pub(super) step: usize,
    pub(super) features: [f64; FEATURE_COUNT],
    pub(super) current_cash: f64,
    pub(super) hold_redemption: f64,
    pub(super) call: Option<f64>,
    pub(super) put: Option<f64>,
    pub(super) friction: f64,
    pub(super) a_to_next: f64,
    pub(super) b_to_next: f64,
}

impl DecisionSnapshot {
    pub(super) fn requires_policy(&self) -> bool {
        self.hold_redemption == 0.0 && (self.call.is_some() || self.put.is_some())
    }

    pub(super) fn exercise_value(&self, continuation_for_decision: f64, realized_hold: f64) -> f64 {
        let continuation_for_decision = if self.hold_redemption > 0.0 {
            self.hold_redemption
        } else {
            continuation_for_decision
        };
        let realized_hold = if self.hold_redemption > 0.0 {
            self.hold_redemption
        } else {
            realized_hold
        };
        let put_exercised = self.put.is_some_and(|put| put > continuation_for_decision);
        let decision_after_put = self.put.map_or(continuation_for_decision, |put| {
            continuation_for_decision.max(put)
        });
        if let Some(call) = self.call {
            if decision_after_put > call + self.friction {
                return call;
            }
        }
        if put_exercised {
            self.put.unwrap_or(realized_hold)
        } else {
            realized_hold
        }
    }
}

#[derive(Clone)]
pub(super) struct PathRecord {
    pub(super) snapshots: Vec<DecisionSnapshot>,
}

pub(super) type RegressionPolicy = StandardizedPolynomialPolicy<FEATURE_COUNT>;

#[allow(clippy::too_many_arguments)]
pub(super) fn fit_make_whole_policies(
    tree: &RatesCreditTree,
    template: &ReplayTemplate,
    bond: &Bond,
    config: &BondLsmcConfig,
    seed: u64,
    estimators: usize,
    simulated_paths: usize,
) -> Result<Vec<RegressionPolicy>> {
    if template.make_whole_claims.is_empty() {
        return Ok(Vec::new());
    }
    let boundaries = training_boundaries(template, simulated_paths)?;
    let checkpoints = build_training_checkpoints(
        tree,
        template,
        bond,
        config,
        seed,
        estimators,
        simulated_paths,
        &boundaries,
    )?;
    let mut policies = vec![None; template.make_whole_claims.len()];
    let mut carried_reference_values =
        vec![vec![0.0; simulated_paths]; template.make_whole_bases.len()];
    let mut sampled = Vec::new();
    for block_index in (0..boundaries.len() - 1).rev() {
        let low = boundaries[block_index];
        let high = boundaries[block_index + 1];
        let owned = high - low;
        let claim_indices = template
            .make_whole_claims
            .iter()
            .enumerate()
            .filter(|(_, claim)| low <= claim.decision_index && claim.decision_index < high)
            .map(|(index, _)| index)
            .collect::<Vec<_>>();
        let mut features = claim_indices
            .iter()
            .map(|_| Vec::with_capacity(simulated_paths))
            .collect::<Vec<_>>();
        let mut responses = claim_indices
            .iter()
            .map(|_| Vec::with_capacity(simulated_paths))
            .collect::<Vec<_>>();
        let mut buffers_by_local = vec![Vec::new(); owned];
        for (buffer_index, &claim_index) in claim_indices.iter().enumerate() {
            let local = template.make_whole_claims[claim_index].decision_index - low;
            buffers_by_local[local].push(buffer_index);
        }

        for physical in 0..simulated_paths {
            let (path_index, antithetic) = physical_path_identity(physical, config.antithetic);
            let record = template.replay_block(
                tree,
                bond,
                config,
                seed,
                path_index,
                antithetic,
                &checkpoints[block_index][physical],
                low,
                high,
                None,
                None,
                &mut sampled,
            )?;
            let low_step = template.decision_steps[low];
            let high_step = template.decision_steps[high];
            let mut local_at_step = vec![None; high_step - low_step];
            for local in 0..owned {
                local_at_step[template.decision_steps[low + local] - low_step] = Some(local);
            }
            for (basis_index, basis) in template.make_whole_bases.iter().enumerate() {
                let mut value = carried_reference_values[basis_index][physical];
                for step in (low_step..high_step).rev() {
                    let next_state = record.step_states[step + 1 - low_step];
                    let redemption = if template.redemption_step == Some(step + 1) {
                        next_state.outstanding
                    } else {
                        0.0
                    };
                    let factor = path_state(&sampled, step)?;
                    value = factor.discount_to_next
                        * basis.interval_adjustments[step]
                        * (next_state.reference_cash + redemption + value);
                    if let Some(local) = local_at_step[step - low_step] {
                        for &buffer_index in &buffers_by_local[local] {
                            let claim = &template.make_whole_claims[claim_indices[buffer_index]];
                            if claim.basis_index != basis_index {
                                continue;
                            }
                            let mut reference_features = record.snapshots[local].features;
                            reference_features[1] = 0.0;
                            features[buffer_index].push(reference_features);
                            responses[buffer_index].push(value);
                        }
                    }
                }
                carried_reference_values[basis_index][physical] = value;
            }
        }
        for (buffer_index, &claim_index) in claim_indices.iter().enumerate() {
            policies[claim_index] = Some(fit_policy(
                &features[buffer_index],
                &responses[buffer_index],
            )?);
        }
    }
    policies
        .into_iter()
        .enumerate()
        .map(|(index, policy)| {
            policy.ok_or_else(|| {
                Error::internal(format!(
                    "bond hazard LSMC did not fit make-whole claim {index}"
                ))
            })
        })
        .collect()
}

#[allow(clippy::too_many_arguments)]
pub(super) fn fit_policies(
    tree: &RatesCreditTree,
    template: &ReplayTemplate,
    bond: &Bond,
    config: &BondLsmcConfig,
    exercise_provider: Option<&dyn BondLsmcExerciseProvider>,
    seed: u64,
    estimators: usize,
    simulated_paths: usize,
    make_whole_policies: &[RegressionPolicy],
) -> Result<Vec<Option<RegressionPolicy>>> {
    let decisions = template.decision_steps.len();
    let last = decisions
        .checked_sub(1)
        .ok_or_else(|| Error::internal("bond hazard LSMC replay produced no decision snapshots"))?;
    let boundaries = training_boundaries(template, simulated_paths)?;
    let checkpoints = build_training_checkpoints(
        tree,
        template,
        bond,
        config,
        seed,
        estimators,
        simulated_paths,
        &boundaries,
    )?;
    let mut policies = vec![None; decisions];
    let mut values = vec![0.0; simulated_paths];
    let mut sampled = Vec::new();
    for block_index in (0..boundaries.len() - 1).rev() {
        let low = boundaries[block_index];
        let high = boundaries[block_index + 1];
        let owned = high - low;
        let capacity = simulated_paths.checked_mul(owned).ok_or_else(|| {
            Error::Validation("bond hazard LSMC block snapshot count overflow".to_string())
        })?;
        let mut block_snapshots = Vec::with_capacity(capacity);
        let block_checkpoints = checkpoints.get(block_index).ok_or_else(|| {
            Error::internal("bond hazard LSMC block checkpoint boundary is missing")
        })?;
        if block_checkpoints.len() != simulated_paths {
            return Err(Error::internal(
                "bond hazard LSMC block checkpoint count is inconsistent",
            ));
        }
        for (physical, checkpoint) in block_checkpoints.iter().enumerate() {
            let (path_index, antithetic) = physical_path_identity(physical, config.antithetic);
            let record = template.replay_block(
                tree,
                bond,
                config,
                seed,
                path_index,
                antithetic,
                checkpoint,
                low,
                high,
                exercise_provider,
                Some(make_whole_policies),
                &mut sampled,
            )?;
            if record.snapshots.len() != owned {
                return Err(Error::internal(
                    "bond hazard LSMC block snapshot count is inconsistent",
                ));
            }
            if high == last {
                let terminal = record.terminal.ok_or_else(|| {
                    Error::internal("bond hazard LSMC final block has no terminal snapshot")
                })?;
                values[physical] = terminal.current_cash
                    + terminal.exercise_value(terminal.hold_redemption, terminal.hold_redemption);
            } else if record.terminal.is_some() {
                return Err(Error::internal(
                    "bond hazard LSMC non-terminal block produced a terminal snapshot",
                ));
            }
            block_snapshots.extend(record.snapshots);
        }
        for local in (0..owned).rev() {
            let decision = low + local;
            let realized = (0..simulated_paths)
                .map(|physical| {
                    let snapshot = &block_snapshots[physical * owned + local];
                    snapshot.a_to_next * values[physical] + snapshot.b_to_next
                })
                .collect::<Vec<_>>();
            let requires_policy =
                block_requires_policy(&block_snapshots, simulated_paths, owned, local)?;
            if requires_policy {
                let features = (0..simulated_paths)
                    .map(|physical| block_snapshots[physical * owned + local].features)
                    .collect::<Vec<_>>();
                let policy = fit_policy(&features, &realized)?;
                for physical in 0..simulated_paths {
                    let snapshot = &block_snapshots[physical * owned + local];
                    values[physical] = snapshot.current_cash
                        + snapshot.exercise_value(
                            policy.predict(&snapshot.features)?,
                            realized[physical],
                        );
                }
                policies[decision] = Some(policy);
            } else {
                for physical in 0..simulated_paths {
                    let snapshot = &block_snapshots[physical * owned + local];
                    values[physical] = snapshot.current_cash
                        + snapshot.exercise_value(realized[physical], realized[physical]);
                }
            }
        }
    }
    Ok(policies)
}

pub(super) fn block_requires_policy(
    snapshots: &[DecisionSnapshot],
    simulated_paths: usize,
    decisions_in_block: usize,
    local_decision: usize,
) -> Result<bool> {
    for physical in 0..simulated_paths {
        let index = physical
            .checked_mul(decisions_in_block)
            .and_then(|offset| offset.checked_add(local_decision))
            .ok_or_else(|| Error::internal("bond hazard LSMC block index overflow"))?;
        let snapshot = snapshots.get(index).ok_or_else(|| {
            Error::internal("bond hazard LSMC block snapshot is missing for a training path")
        })?;
        if snapshot.requires_policy() {
            return Ok(true);
        }
    }
    Ok(false)
}

pub(super) fn value_with_policy(
    path: &PathRecord,
    policies: &[Option<RegressionPolicy>],
) -> Result<f64> {
    if path.snapshots.len() != policies.len() || path.snapshots.is_empty() {
        return Err(Error::internal(
            "bond hazard LSMC pricing replay does not match the trained decision grid",
        ));
    }
    let last = path.snapshots.len() - 1;
    let terminal = &path.snapshots[last];
    let mut value = terminal.current_cash
        + terminal.exercise_value(terminal.hold_redemption, terminal.hold_redemption);
    for decision in (0..last).rev() {
        let snapshot = &path.snapshots[decision];
        let continuation = snapshot.a_to_next * value + snapshot.b_to_next;
        if snapshot.requires_policy() {
            let policy = policies[decision].as_ref().ok_or_else(|| {
                Error::internal(format!(
                    "bond hazard LSMC has no fitted policy for exercise decision {decision}"
                ))
            })?;
            value = snapshot.current_cash
                + snapshot.exercise_value(policy.predict(&snapshot.features)?, continuation);
        } else {
            value = snapshot.current_cash + snapshot.exercise_value(continuation, continuation);
        }
    }
    Ok(value)
}

pub(super) fn fit_policy(
    features: &[[f64; FEATURE_COUNT]],
    responses: &[f64],
) -> Result<RegressionPolicy> {
    RegressionPolicy::fit(features, responses, PolynomialDegree::Quadratic)
}
