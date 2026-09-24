//! Least-squares Monte Carlo for option-bearing credit-risky bonds.
//!
//! Training and pricing use disjoint Philox seed domains. The regression
//! policy is fitted on standardized rate, hazard, balance, locked-coupon,
//! accrued-interest, and cumulative-distribution state, then frozen before
//! the pricing paths are sampled.

use super::bond_valuator::BondValuator;
use crate::cashflow::builder::calendar::resolve_calendar_strict;
use crate::cashflow::builder::specs::{CouponType, FloatingCouponSpec, FloatingRateFallback};
use crate::cashflow::builder::{
    CompiledFloatingCoupon, FloatingCouponEconomics, FloatingCouponPeriod,
    FloatingCouponReplayState, FloatingRateObservation, OvernightObservationSchedule,
    OvernightRateConstraints,
};
use crate::cashflow::primitives::{is_cash_settlement_kind, CFKind, CashFlow};
use crate::instruments::common_impl::helpers::resolve_mc_paths;
use crate::instruments::common_impl::pricing::floating_reset_descriptors::params_from_spec;
use crate::instruments::common_impl::pricing::rates_credit::continuous_frp_weight;
use crate::instruments::fixed_income::bond::pricing::return_floor::realized_distributions;
use crate::instruments::fixed_income::bond::{
    Bond, CallPut, CashflowSpec, ProtectionWindow, ReturnFloorKind,
};
use finstack_quant_core::dates::{adjust, Date, DateExt, DayCount, DayCountContext, Duration};
use finstack_quant_core::market_data::context::MarketContext;
use finstack_quant_core::market_data::fixings::{fixing_series_id, require_fixing_value_exact};
use finstack_quant_core::market_data::traits::Discounting;
use finstack_quant_core::math::stats::OnlineStats;
use finstack_quant_core::{Error, Result};
use finstack_quant_models::monte_carlo::pricer::polynomial::{
    PolynomialDegree, StandardizedPolynomialPolicy,
};
use finstack_quant_models::monte_carlo::results::MoneyEstimate;
use finstack_quant_models::monte_carlo::seed::derive_seed;
use finstack_quant_models::trees::two_factor_rates_credit::{
    RatesCreditPathCheckpoint, RatesCreditPathState, RatesCreditTree,
};
use rust_decimal::prelude::ToPrimitive;
use smallvec::SmallVec;
use std::collections::{BTreeMap, BTreeSet};
use std::mem::size_of;

mod build;
mod policy;
mod replay;
mod state;
#[cfg(test)]
mod tests;

use build::*;
use policy::*;
use state::*;

/// Default independent-estimator budget in each training or pricing stage.
/// With antithetic sampling each estimator averages two physical factor paths,
/// so the default produces 40,000 unique factor paths per stage.
pub(crate) const DEFAULT_BOND_LSMC_PATHS: usize = 20_000;

const TRAINING_DOMAIN: &str = "bond_hazard_lsmc_training";
const MAKE_WHOLE_TRAINING_DOMAIN: &str = "bond_hazard_lsmc_make_whole_training";
const PRICING_DOMAIN: &str = "bond_hazard_lsmc_pricing";
const FEATURE_COUNT: usize = 8;
const MAX_TRAINING_BYTES: usize = 1_073_741_824;
const QUADRATIC_TERM_COUNT: usize =
    1 + FEATURE_COUNT + FEATURE_COUNT + FEATURE_COUNT * (FEATURE_COUNT - 1) / 2;
const REGRESSION_WORKSPACE_COPIES: usize = 4;

/// Runtime controls for bond LSMC.
#[derive(Debug, Clone, Copy)]
pub(crate) struct BondLsmcConfig {
    /// Independent estimators sampled exactly in each stage. Each estimator
    /// uses two physical factor paths when `antithetic` is true.
    pub(crate) paths: usize,
    /// Whether consecutive factor paths form one antithetic estimator.
    pub(crate) antithetic: bool,
    /// Root seed used only as a caller-visible reproducibility input.
    pub(crate) seed: u64,
    /// Continuously compounded option-adjusted spread in basis points.
    pub(crate) oas_bp: f64,
    /// Optional required final 95% confidence-interval half-width in currency units.
    pub(crate) target_ci_half_width: Option<f64>,
}

impl BondLsmcConfig {
    /// Resolve bond-owned Monte Carlo controls and a stable default seed.
    pub(crate) fn for_bond(bond: &Bond, oas_bp: f64) -> Result<Self> {
        let paths = resolve_mc_paths(
            bond.instrument_pricing_overrides.model_config.mc_paths,
            DEFAULT_BOND_LSMC_PATHS,
        )?;
        if !oas_bp.is_finite() {
            return Err(Error::Validation(format!(
                "bond hazard LSMC OAS must be finite, got {oas_bp}"
            )));
        }
        Ok(Self {
            paths,
            antithetic: bond
                .instrument_pricing_overrides
                .model_config
                .mc_antithetic
                .unwrap_or(true),
            seed: derive_seed(
                &bond.id,
                bond.metric_pricing_overrides
                    .mc_seed_scenario
                    .as_deref()
                    .unwrap_or("bond_hazard_lsmc"),
            ),
            oas_bp,
            target_ci_half_width: bond
                .instrument_pricing_overrides
                .model_config
                .mc_target_ci_half_width,
        })
    }

    fn validate(self) -> Result<()> {
        if self.paths == 0 {
            return Err(Error::Validation(
                "bond hazard LSMC requires at least one simulated factor path".to_string(),
            ));
        }
        if !self.oas_bp.is_finite() {
            return Err(Error::Validation(format!(
                "bond hazard LSMC OAS must be finite, got {}",
                self.oas_bp
            )));
        }
        if self
            .target_ci_half_width
            .is_some_and(|target| !target.is_finite() || target <= 0.0)
        {
            return Err(Error::Validation(format!(
                "bond hazard LSMC target 95% CI half-width must be positive and finite, got {:?}",
                self.target_ci_half_width
            )));
        }
        Ok(())
    }
}

/// Price estimate plus stage-level simulation diagnostics.
#[derive(Debug, Clone)]
pub(crate) struct BondLsmcResult {
    /// Price, standard error, and 95% confidence interval. The standard error
    /// is the sampling uncertainty of the estimate under the frozen fitted
    /// policy; it excludes regression, time-grid, and model error.
    pub(crate) estimate: MoneyEstimate,
    /// Unique independent estimator identities in exercise-policy training.
    pub(crate) training_paths: usize,
    /// Unique primary plus antithetic physical paths in exercise-policy training.
    pub(crate) training_simulated_paths: usize,
    /// Unique independent estimator identities in make-whole training.
    pub(crate) make_whole_training_paths: usize,
    /// Unique primary plus antithetic physical paths in make-whole training.
    pub(crate) make_whole_training_simulated_paths: usize,
    /// Unique primary plus antithetic physical paths in out-of-sample pricing.
    pub(crate) pricing_simulated_paths: usize,
    /// Root seed used to derive independent training and pricing streams.
    pub(crate) seed: u64,
    /// Calibrated simulation times in ACT/365F year fractions.
    pub(crate) time_grid: Vec<f64>,
    /// Whether each independent estimator averaged an antithetic path pair.
    pub(crate) antithetic: bool,
}

/// Path state supplied to state-dependent exercise-price callbacks.
pub(crate) struct BondLsmcExerciseState<'a> {
    /// Bond being valued.
    pub(crate) bond: &'a Bond,
    /// Contractual exercise date represented by this decision.
    pub(crate) date: Date,
    /// Rates-credit lattice step carrying the decision state.
    pub(crate) step: usize,
    /// Calibrated short rate at the sampled node, in annual decimal units.
    pub(crate) short_rate: f64,
    /// Effective sampled hazard rate, in annual decimal units.
    pub(crate) hazard_rate: f64,
    /// Principal outstanding after same-step capitalization and amortization.
    pub(crate) outstanding: f64,
    /// Total amount of coupons already fixed but not yet paid.
    pub(crate) locked_coupon: f64,
    /// Accrued cash-pay coupon amount at the exercise date.
    pub(crate) accrued_cash: f64,
    /// Accrued PIK coupon amount at the exercise date.
    pub(crate) accrued_pik: f64,
    /// Holder cash distributions accumulated through the exercise date.
    pub(crate) cumulative_distribution_cash: f64,
    /// Return-floor target-present-value accumulator through the exercise date.
    pub(crate) cumulative_distribution_target_pv: f64,
    /// Static contractual call amount, including accrued coupon, when active.
    pub(crate) default_call: Option<f64>,
    /// Static contractual put amount, including accrued coupon, when active.
    pub(crate) default_put: Option<f64>,
}

impl BondLsmcExerciseState<'_> {
    fn validate(&self) -> Result<()> {
        let finite_state = [
            self.short_rate,
            self.hazard_rate,
            self.outstanding,
            self.locked_coupon,
            self.accrued_cash,
            self.accrued_pik,
            self.cumulative_distribution_cash,
            self.cumulative_distribution_target_pv,
        ]
        .into_iter()
        .all(f64::is_finite);
        if !finite_state || self.hazard_rate < 0.0 || self.outstanding < 0.0 {
            return Err(Error::Validation(format!(
                "Bond '{}' has invalid hazard LSMC exercise state at {} (step {})",
                self.bond.id.as_str(),
                self.date,
                self.step
            )));
        }
        validate_exercise_amount("default call", self.default_call)?;
        validate_exercise_amount("default put", self.default_put)
    }
}

/// Exercise barriers returned by a state-dependent provider.
#[derive(Debug, Clone, Copy)]
pub(crate) struct BondLsmcExerciseAmounts {
    /// Issuer call amount or `None` when no call is active.
    pub(crate) call: Option<f64>,
    /// Holder put amount or `None` when no put is active.
    pub(crate) put: Option<f64>,
}

/// Supplies state-dependent make-whole or return-floor exercise amounts.
pub(crate) trait BondLsmcExerciseProvider: Send + Sync {
    /// Return exercise amounts for one sampled decision state.
    fn exercise_amounts(
        &self,
        state: &BondLsmcExerciseState<'_>,
    ) -> Result<BondLsmcExerciseAmounts>;
}

impl<F> BondLsmcExerciseProvider for F
where
    F: for<'a> Fn(&BondLsmcExerciseState<'a>) -> Result<BondLsmcExerciseAmounts> + Send + Sync,
{
    fn exercise_amounts(
        &self,
        state: &BondLsmcExerciseState<'_>,
    ) -> Result<BondLsmcExerciseAmounts> {
        self(state)
    }
}

/// Fit an exercise policy and value it on an independent factor-path stream.
///
/// The `tree` must already be calibrated.  Ordinary fixed-price call and put
/// schedules and return floors need no callback. The provider is an optional
/// product hook for overriding an otherwise resolved exercise amount.
pub(crate) fn price_bond_lsmc(
    tree: &RatesCreditTree,
    bond: &Bond,
    market: &MarketContext,
    as_of: Date,
    config: &BondLsmcConfig,
    exercise_provider: Option<&dyn BondLsmcExerciseProvider>,
) -> Result<BondLsmcResult> {
    config.validate()?;
    let has_embedded_options = bond.return_floor.is_some()
        || bond
            .call_put
            .as_ref()
            .is_some_and(|schedule| schedule.has_options());
    if has_embedded_options && bond.custom_cashflows.is_some() {
        return Err(Error::Validation(format!(
            "Bond '{}' cannot use custom cashflows with rates-credit LSMC options because the custom schedule does not preserve the contractual reset, accrual, PIK, and exercise-state decomposition",
            bond.id.as_str()
        )));
    }
    let template = ReplayTemplate::new(tree, bond, market, as_of)?;
    let train_seed = mix_seed(config.seed, derive_seed(&bond.id, TRAINING_DOMAIN));
    let make_whole_seed = mix_seed(
        config.seed,
        derive_seed(&bond.id, MAKE_WHOLE_TRAINING_DOMAIN),
    );
    let pricing_seed = mix_seed(config.seed, derive_seed(&bond.id, PRICING_DOMAIN));
    if train_seed == pricing_seed
        || train_seed == make_whole_seed
        || make_whole_seed == pricing_seed
    {
        return Err(Error::internal("bond hazard LSMC seed domains collided"));
    }

    let pricing_estimators = config.paths;
    let pricing_simulated_paths = simulated_path_count(pricing_estimators, config.antithetic)?;
    let mut sampled = Vec::new();
    let needs_training = template.exercise.iter().any(|entries| !entries.is_empty());
    let training_estimators = usize::from(needs_training) * config.paths;
    let training_simulated_paths = if needs_training {
        simulated_path_count(training_estimators, config.antithetic)?
    } else {
        0
    };
    let make_whole_training_paths =
        usize::from(!template.make_whole_claims.is_empty()) * config.paths;
    let make_whole_training_simulated_paths = if make_whole_training_paths > 0 {
        simulated_path_count(make_whole_training_paths, config.antithetic)?
    } else {
        0
    };
    let make_whole_policies = fit_make_whole_policies(
        tree,
        &template,
        bond,
        config,
        make_whole_seed,
        make_whole_training_paths,
        make_whole_training_simulated_paths,
    )?;
    let policies = if needs_training {
        fit_policies(
            tree,
            &template,
            bond,
            config,
            exercise_provider,
            train_seed,
            training_estimators,
            training_simulated_paths,
            &make_whole_policies,
        )?
    } else {
        vec![None; template.decision_steps.len()]
    };

    let mut stats = OnlineStats::new();
    for path_index in 0..pricing_estimators {
        tree.sample_path_into(pricing_seed, path_index as u64, false, &mut sampled)?;
        let primary = value_with_policy(
            &template.replay(
                tree,
                bond,
                &sampled,
                config,
                exercise_provider,
                Some(&make_whole_policies),
            )?,
            &policies,
        )?;
        if config.antithetic {
            tree.sample_path_into(pricing_seed, path_index as u64, true, &mut sampled)?;
            let antithetic = value_with_policy(
                &template.replay(
                    tree,
                    bond,
                    &sampled,
                    config,
                    exercise_provider,
                    Some(&make_whole_policies),
                )?,
                &policies,
            )?;
            stats.update(0.5 * (primary + antithetic));
        } else {
            stats.update(primary);
        }
    }

    if let Some(target) = config.target_ci_half_width {
        let actual = stats.ci_half_width();
        if !actual.is_finite() || actual > target {
            return Err(Error::Validation(format!(
                "Bond '{}' hazard LSMC exhausted its fixed budget of {} independent pricing estimators with 95% CI half-width {actual}, above target {target}",
                bond.id.as_str(), pricing_estimators
            )));
        }
    }
    let estimate = finstack_quant_models::monte_carlo::estimate::Estimate::new(
        stats.mean(),
        stats.stderr(),
        stats.confidence_interval(0.05),
        pricing_estimators,
    )
    .with_num_simulated_paths(pricing_simulated_paths)
    .with_std_dev(stats.std_dev());
    Ok(BondLsmcResult {
        estimate: MoneyEstimate::from_estimate(estimate, bond.notional.currency())?,
        training_paths: training_estimators,
        training_simulated_paths,
        make_whole_training_paths,
        make_whole_training_simulated_paths,
        pricing_simulated_paths,
        seed: config.seed,
        time_grid: tree.time_grid()?.to_vec(),
        antithetic: config.antithetic,
    })
}

fn simulated_path_count(estimators: usize, antithetic: bool) -> Result<usize> {
    if antithetic {
        estimators.checked_mul(2).ok_or_else(|| {
            Error::Validation("bond hazard LSMC factor-path count overflow".to_string())
        })
    } else {
        Ok(estimators)
    }
}

fn training_block_len(
    template: &ReplayTemplate,
    simulated_paths: usize,
    decision_count: usize,
) -> Result<usize> {
    let decision_count = decision_count.max(1);
    let max_live = template
        .decision_steps
        .iter()
        .map(|&step| {
            template
                .floating
                .iter()
                .filter(|coupon| {
                    let live_from = match &coupon.rate_model {
                        FloatingRateModel::Term(_) => {
                            coupon.reset_step.min(coupon.accrual_start_step)
                        }
                        FloatingRateModel::Overnight(_) => coupon.accrual_start_step,
                    };
                    live_from < step && step <= coupon.payment_step
                })
                .count()
        })
        .max()
        .unwrap_or(0);
    let heap_live_slots = if max_live <= 1 {
        0
    } else {
        max_live.checked_next_power_of_two().unwrap_or(usize::MAX)
    };
    let heap_live_bytes = heap_live_slots.saturating_mul(size_of::<LiveFloatingCheckpoint>());
    let checkpoint_bytes = size_of::<TrainingCheckpoint>()
        .saturating_add(heap_live_bytes)
        .saturating_add(size_of::<usize>() * 2);
    let snapshot_bytes = size_of::<DecisionSnapshot>();
    let checkpoint_build_scratch = template
        .times
        .len()
        .saturating_mul(size_of::<RatesCreditPathState>())
        .saturating_add(
            template
                .floating
                .len()
                .saturating_mul(size_of::<FloatingRuntimeState>()),
        );
    let regression_workspace = simulated_paths
        .saturating_mul(QUADRATIC_TERM_COUNT)
        .saturating_mul(size_of::<f64>())
        .saturating_mul(REGRESSION_WORKSPACE_COPIES);
    let carried_make_whole = template
        .make_whole_bases
        .len()
        .saturating_mul(simulated_paths)
        .saturating_mul(size_of::<f64>())
        .saturating_add(
            template
                .make_whole_bases
                .len()
                .saturating_mul(size_of::<Vec<f64>>()),
        );
    // Each fitted policy retains two heap vectors (terms and coefficients).
    // The models-owned basis term is private, so three indices conservatively
    // bound its per-term storage here.
    let policy_bytes = size_of::<RegressionPolicy>().saturating_add(
        QUADRATIC_TERM_COUNT
            .saturating_mul(size_of::<f64>().saturating_add(size_of::<[usize; 3]>())),
    );
    let make_whole_policy_storage = template
        .make_whole_claims
        .len()
        .saturating_mul(policy_bytes);
    let exercise_policy_storage = decision_count.saturating_mul(policy_bytes);
    let mut make_whole_claim_prefix = vec![0_usize; decision_count.saturating_add(1)];
    for claim in &template.make_whole_claims {
        if claim.decision_index < decision_count {
            make_whole_claim_prefix[claim.decision_index + 1] =
                make_whole_claim_prefix[claim.decision_index + 1].saturating_add(1);
        }
    }
    for decision in 0..decision_count {
        make_whole_claim_prefix[decision + 1] =
            make_whole_claim_prefix[decision + 1].saturating_add(make_whole_claim_prefix[decision]);
    }
    let mut best = None;
    for block_len in 1..=decision_count {
        let boundary_count = decision_count.div_ceil(block_len).saturating_add(1);
        let checkpoint_total = boundary_count
            .saturating_mul(simulated_paths)
            .saturating_mul(checkpoint_bytes)
            .saturating_add(boundary_count.saturating_mul(size_of::<Vec<TrainingCheckpoint>>()));
        let mut max_owned = 0_usize;
        let mut max_claims = 0_usize;
        let mut max_step_span = 0_usize;
        let mut low = 0_usize;
        while low < decision_count {
            let high = (low + block_len).min(decision_count);
            max_owned = max_owned.max(high - low);
            max_claims = max_claims
                .max(make_whole_claim_prefix[high].saturating_sub(make_whole_claim_prefix[low]));
            max_step_span = max_step_span.max(
                template.decision_steps[high]
                    .saturating_sub(template.decision_steps[low])
                    .saturating_add(template.max_rate_history_steps),
            );
            low = high;
        }
        let path_scratch = max_step_span
            .saturating_add(1)
            .saturating_mul(
                size_of::<RatesCreditPathState>().saturating_add(size_of::<StepReplay>()),
            )
            .saturating_add(max_owned.saturating_add(1).saturating_mul(snapshot_bytes))
            .saturating_add(
                template
                    .floating
                    .len()
                    .saturating_mul(size_of::<FloatingRuntimeState>()),
            );
        let exercise_workspace = simulated_paths
            .saturating_mul(
                max_owned
                    .saturating_mul(snapshot_bytes)
                    .saturating_add(size_of::<f64>() * 2)
                    .saturating_add(size_of::<[f64; FEATURE_COUNT]>()),
            )
            .saturating_add(regression_workspace)
            .saturating_add(exercise_policy_storage)
            .saturating_add(path_scratch);
        let make_whole_workspace =
            carried_make_whole
                .saturating_add(make_whole_policy_storage)
                .saturating_add(max_claims.saturating_mul(simulated_paths).saturating_mul(
                    size_of::<[f64; FEATURE_COUNT]>().saturating_add(size_of::<f64>()),
                ))
                .saturating_add(
                    max_claims
                        .saturating_mul(2)
                        .saturating_mul(size_of::<Vec<f64>>()),
                )
                .saturating_add(max_owned.saturating_mul(size_of::<Vec<usize>>()))
                .saturating_add(regression_workspace)
                .saturating_add(path_scratch);
        let total = checkpoint_total.saturating_add(
            checkpoint_build_scratch.max(exercise_workspace.max(make_whole_workspace)),
        );
        if best.is_none_or(|(_, best_total)| total < best_total) {
            best = Some((block_len, total));
        }
    }
    let (block_len, estimated_bytes) =
        best.ok_or_else(|| Error::internal("bond hazard LSMC could not size its training blocks"))?;
    if estimated_bytes > MAX_TRAINING_BYTES {
        return Err(Error::Validation(format!(
            "bond hazard LSMC checkpoint and regression workspace requires an estimated {estimated_bytes} bytes for {simulated_paths} physical paths and {decision_count} decisions, above the fixed {MAX_TRAINING_BYTES}-byte training-memory bound; reduce mc_paths or exercise dates"
        )));
    }
    Ok(block_len)
}

fn training_boundaries(template: &ReplayTemplate, simulated_paths: usize) -> Result<Vec<usize>> {
    let transitions = template
        .decision_steps
        .len()
        .checked_sub(1)
        .ok_or_else(|| {
            Error::internal("bond hazard LSMC requires at least one decision transition")
        })?;
    let block_len = training_block_len(template, simulated_paths, transitions)?;
    let mut boundaries = Vec::with_capacity(transitions.div_ceil(block_len) + 1);
    boundaries.push(0);
    let mut decision = 0;
    while decision < transitions {
        decision = (decision + block_len).min(transitions);
        boundaries.push(decision);
    }
    Ok(boundaries)
}

#[allow(clippy::too_many_arguments)]
fn build_training_checkpoints(
    tree: &RatesCreditTree,
    template: &ReplayTemplate,
    bond: &Bond,
    config: &BondLsmcConfig,
    seed: u64,
    estimators: usize,
    simulated_paths: usize,
    boundaries: &[usize],
) -> Result<Vec<Vec<TrainingCheckpoint>>> {
    let mut checkpoints = boundaries
        .iter()
        .map(|_| Vec::with_capacity(simulated_paths))
        .collect::<Vec<_>>();
    let mut sampled = Vec::new();
    for path_index in 0..estimators {
        for antithetic in [false, true]
            .into_iter()
            .take(if config.antithetic { 2 } else { 1 })
        {
            tree.sample_path_into(seed, path_index as u64, antithetic, &mut sampled)?;
            let mut cursor = ReplayCursor::new(template)?;
            let mut boundary_index = 0_usize;
            for step in 0..template.times.len() {
                while boundary_index < boundaries.len()
                    && template.decision_steps[boundaries[boundary_index]] == step
                {
                    let prefix_step = step.saturating_sub(template.max_rate_history_steps);
                    let factor = sampled.get(prefix_step).ok_or_else(|| {
                        Error::internal(
                            "bond hazard LSMC checkpoint prefix is outside sampled path",
                        )
                    })?;
                    checkpoints[boundary_index].push(TrainingCheckpoint {
                        factor: RatesCreditPathCheckpoint::from(factor),
                        product: cursor.checkpoint(template),
                    });
                    boundary_index += 1;
                }
                cursor.advance_step(template, bond, &sampled, config, step)?;
            }
            if boundary_index != boundaries.len() {
                return Err(Error::internal(
                    "bond hazard LSMC failed to capture every training boundary",
                ));
            }
        }
    }
    if checkpoints
        .iter()
        .any(|boundary| boundary.len() != simulated_paths)
    {
        return Err(Error::internal(
            "bond hazard LSMC checkpoint count does not match physical path count",
        ));
    }
    Ok(checkpoints)
}

fn physical_path_identity(physical: usize, antithetic: bool) -> (u64, bool) {
    if antithetic {
        ((physical / 2) as u64, physical % 2 == 1)
    } else {
        (physical as u64, false)
    }
}

#[inline]
fn mix_seed(left: u64, right: u64) -> u64 {
    left.rotate_left(17) ^ right.wrapping_mul(0x9e37_79b9_7f4a_7c15)
}
