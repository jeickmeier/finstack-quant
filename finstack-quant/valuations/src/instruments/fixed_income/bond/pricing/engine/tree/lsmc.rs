//! Least-squares Monte Carlo for option-bearing credit-risky bonds.
//!
//! Training and pricing use disjoint Philox seed domains. The regression
//! policy is fitted on standardized rate, hazard, balance, locked-coupon,
//! accrued-interest, and cumulative-distribution state, then frozen before
//! the pricing paths are sampled.

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

#[derive(Clone)]
struct CashEvent {
    amount_at_step: f64,
    event_minus_step: f64,
}

#[derive(Clone, Copy)]
struct BalanceEvent {
    delta: f64,
}

#[derive(Clone)]
struct AccrualClaim {
    start: Date,
    end: Date,
    payment: Date,
    day_count: finstack_quant_core::dates::DayCount,
    amount: f64,
    pik: bool,
}

#[derive(Clone, Copy)]
struct DistributionEvent {
    date: Date,
    amount: f64,
}

#[derive(Clone, Copy)]
struct ReturnFloorTemplate {
    kind: ReturnFloorKind,
    issue_price: f64,
    issue_date: Date,
    day_count: DayCount,
}

impl ReturnFloorTemplate {
    fn redemption(
        self,
        date: Date,
        outstanding: f64,
        cumulative_cash: f64,
        cumulative_target_pv: f64,
        accrued: f64,
    ) -> Result<f64> {
        let required = match self.kind {
            ReturnFloorKind::Moic(multiple) => multiple * self.issue_price - cumulative_cash,
            ReturnFloorKind::Xirr(rate) => {
                let target = rate.as_decimal();
                let year_fraction = self.day_count.year_fraction(
                    self.issue_date,
                    date,
                    DayCountContext::default(),
                )?;
                (self.issue_price - cumulative_target_pv) * (1.0 + target).powf(year_fraction)
            }
        };
        if !required.is_finite() {
            return Err(Error::Validation(format!(
                "return-floor redemption at {date} is not finite"
            )));
        }
        Ok((required - accrued).max(outstanding).max(0.0))
    }

    fn target_pv(self, date: Date, amount: f64) -> Result<f64> {
        match self.kind {
            ReturnFloorKind::Moic(_) => Ok(0.0),
            ReturnFloorKind::Xirr(rate) => {
                let year_fraction = self.day_count.year_fraction(
                    self.issue_date,
                    date,
                    DayCountContext::default(),
                )?;
                Ok(amount / (1.0 + rate.as_decimal()).powf(year_fraction))
            }
        }
    }
}

#[derive(Clone)]
struct FloatingCoupon {
    reset_step: usize,
    accrual_start_step: usize,
    payment_step: usize,
    compiled: CompiledFloatingCoupon,
    rate_model: FloatingRateModel,
    initial_notional: Option<f64>,
    initial_term_rate: Option<f64>,
}

#[derive(Clone)]
enum FloatingRateModel {
    Term(ObservedRateSource),
    Overnight(OvernightCoupon),
}

#[derive(Clone)]
struct ConditionalRate {
    observation_step: usize,
    accrual: f64,
    base_index_rate: f64,
    base_discount_forward: f64,
    conditional_discount_factors: Vec<f64>,
}

impl ConditionalRate {
    fn rate(&self, path: &[RatesCreditPathState]) -> Result<f64> {
        let first_step = path
            .first()
            .map(|state| state.step)
            .ok_or_else(|| Error::internal("bond hazard LSMC conditional rate path is empty"))?;
        let offset = self.observation_step.checked_sub(first_step).ok_or_else(|| {
            Error::internal(format!(
                "bond hazard LSMC conditional rate step {} precedes sampled segment start {first_step}",
                self.observation_step
            ))
        })?;
        let state = path
            .get(offset)
            .filter(|state| state.step == self.observation_step)
            .ok_or_else(|| {
                Error::internal(
                    "bond hazard LSMC conditional rate step is outside the sampled path",
                )
            })?;
        let node_df = self
            .conditional_discount_factors
            .get(state.rate_node)
            .copied()
            .ok_or_else(|| {
                Error::internal(format!(
                    "bond hazard LSMC rate node {} is outside conditional-forward slice {}",
                    state.rate_node, self.observation_step
                ))
            })?;
        if !node_df.is_finite() || node_df <= 0.0 {
            return Err(Error::internal(
                "bond hazard LSMC conditional discount factor is not positive and finite",
            ));
        }
        let node_forward = (1.0 / node_df - 1.0) / self.accrual;
        let rate = self.base_index_rate + node_forward - self.base_discount_forward;
        if rate.is_finite() {
            Ok(rate)
        } else {
            Err(Error::internal(
                "bond hazard LSMC produced a non-finite conditional index rate",
            ))
        }
    }
}

#[derive(Clone)]
enum OvernightRateSource {
    Fixed(f64),
    Conditional(ConditionalRate),
}

type ObservedRateSource = OvernightRateSource;

impl OvernightRateSource {
    fn rate(&self, path: &[RatesCreditPathState]) -> Result<f64> {
        match self {
            Self::Fixed(rate) => Ok(*rate),
            Self::Conditional(rate) => rate.rate(path),
        }
    }
}

#[derive(Clone)]
struct OvernightCoupon {
    sources: BTreeMap<(Date, u32), OvernightRateSource>,
    max_history_steps: usize,
}

#[derive(Clone)]
struct ExerciseDate {
    date: Date,
    calls: Vec<ExerciseCall>,
    puts: Vec<CallPut>,
    return_floor: bool,
}

#[derive(Clone)]
struct ExerciseCall {
    price_pct_of_par: f64,
    make_whole: Option<MakeWholeExercise>,
}

#[derive(Clone)]
enum MakeWholeExercise {
    Deterministic(f64),
    Conditional(usize),
}

#[derive(Clone)]
struct MakeWholeClaim {
    exercise_step: usize,
    decision_index: usize,
    basis_index: usize,
}

#[derive(Clone)]
struct MakeWholeBasis {
    interval_adjustments: Vec<f64>,
}

impl MakeWholeBasis {
    #[cfg(test)]
    fn realized_reference_value(
        &self,
        exercise_step: usize,
        path: &[RatesCreditPathState],
        cash: &[f64],
    ) -> Result<f64> {
        if path.len() != cash.len() || path.len() != self.interval_adjustments.len() + 1 {
            return Err(Error::internal(
                "bond hazard LSMC make-whole replay does not match its reference grid",
            ));
        }
        let terminal = path.len() - 1;
        if exercise_step >= terminal {
            return Ok(0.0);
        }
        let mut value = cash[terminal];
        for step in (exercise_step..terminal).rev() {
            let discount = path[step].discount_to_next * self.interval_adjustments[step];
            if !discount.is_finite() || discount <= 0.0 {
                return Err(Error::internal(
                    "bond hazard LSMC make-whole reference discount is invalid",
                ));
            }
            value *= discount;
            if step > exercise_step {
                value += cash[step];
            }
        }
        if value.is_finite() && value >= 0.0 {
            Ok(value)
        } else {
            Err(Error::internal(
                "bond hazard LSMC make-whole reference value is invalid",
            ))
        }
    }
}

#[derive(Clone)]
struct ReplayTemplate {
    times: Vec<f64>,
    step_dates: Vec<Option<Date>>,
    static_cash: Vec<Vec<CashEvent>>,
    balance_events: Vec<Vec<BalanceEvent>>,
    floating: Vec<FloatingCoupon>,
    floating_reset_ids: Vec<Vec<usize>>,
    floating_accrual_start_ids: Vec<Vec<usize>>,
    floating_payment_ids: Vec<Vec<usize>>,
    static_accruals: Vec<AccrualClaim>,
    static_distributions: Vec<Vec<DistributionEvent>>,
    exercise: Vec<Vec<ExerciseDate>>,
    decision_steps: Vec<usize>,
    initial_outstanding: f64,
    redemption_step: Option<usize>,
    call_friction_cents: f64,
    recovery_rate: f64,
    return_floor: Option<ReturnFloorTemplate>,
    historical_distribution_cash: f64,
    historical_distribution_target_pv: f64,
    make_whole_claims: Vec<MakeWholeClaim>,
    make_whole_bases: Vec<MakeWholeBasis>,
    max_rate_history_steps: usize,
}

#[derive(Default)]
struct FloatingBuild {
    reset: Option<Date>,
    start: Option<Date>,
    end: Option<Date>,
    payment: Option<Date>,
    day_count: Option<finstack_quant_core::dates::DayCount>,
    accrual: f64,
    base_index_rate: Option<f64>,
}

type FloatingRuntimeState = FloatingCouponReplayState;

#[derive(Clone)]
struct LiveFloatingCheckpoint {
    coupon_id: usize,
    state: FloatingRuntimeState,
}

#[derive(Clone)]
struct ProductCheckpoint {
    outstanding: f64,
    cumulative_distribution_cash: f64,
    cumulative_distribution_target_pv: f64,
    live_floating: SmallVec<[LiveFloatingCheckpoint; 1]>,
}

#[derive(Clone)]
struct TrainingCheckpoint {
    factor: RatesCreditPathCheckpoint,
    product: ProductCheckpoint,
}

struct BlockPathRecord {
    snapshots: Vec<DecisionSnapshot>,
    terminal: Option<DecisionSnapshot>,
    step_states: Vec<StepReplay>,
}

#[derive(Clone)]
struct ReplayCursor {
    outstanding: f64,
    cumulative_distribution_cash: f64,
    cumulative_distribution_target_pv: f64,
    floating: Vec<FloatingRuntimeState>,
}

#[derive(Clone, Copy, Default)]
struct StepReplay {
    current_cash: f64,
    reference_cash: f64,
    outstanding: f64,
    cumulative_distribution_cash: f64,
    cumulative_distribution_target_pv: f64,
}

struct ExerciseInputs<'a> {
    bond: &'a Bond,
    step: usize,
    path: &'a [RatesCreditPathState],
    outstanding: f64,
    locked_coupon: f64,
    coupon_states: &'a [FloatingRuntimeState],
    cumulative_distribution_cash: f64,
    cumulative_distribution_target_pv: f64,
    provider: Option<&'a dyn BondLsmcExerciseProvider>,
    make_whole_policies: Option<&'a [RegressionPolicy]>,
}

impl ReplayCursor {
    fn new(template: &ReplayTemplate) -> Result<Self> {
        let mut floating = Vec::with_capacity(template.floating.len());
        for coupon in &template.floating {
            let mut state = coupon.compiled.replay_state();
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

    fn from_checkpoint(template: &ReplayTemplate, checkpoint: &ProductCheckpoint) -> Result<Self> {
        let mut floating = vec![FloatingRuntimeState::default(); template.floating.len()];
        for live in &checkpoint.live_floating {
            let slot = floating.get_mut(live.coupon_id).ok_or_else(|| {
                Error::internal("bond hazard LSMC checkpoint has an invalid floating coupon id")
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

    fn checkpoint(&self, template: &ReplayTemplate) -> ProductCheckpoint {
        let live_floating = self
            .floating
            .iter()
            .enumerate()
            .filter(|(coupon_id, state)| template.floating[*coupon_id].compiled.is_live(state))
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

    fn advance_overnight(
        &mut self,
        template: &ReplayTemplate,
        path: &[RatesCreditPathState],
        coupon_id: usize,
        end: Date,
    ) -> Result<()> {
        let coupon = template.floating.get(coupon_id).ok_or_else(|| {
            Error::internal("bond hazard LSMC overnight coupon id is outside the replay template")
        })?;
        let FloatingRateModel::Overnight(overnight) = &coupon.rate_model else {
            return Ok(());
        };
        let state = self.floating.get_mut(coupon_id).ok_or_else(|| {
            Error::internal("bond hazard LSMC overnight coupon state is outside the replay cursor")
        })?;
        coupon.compiled.advance_overnight(state, end, |slice| {
            overnight
                .sources
                .get(&(slice.observation_date, slice.rate_tenor_days))
                .ok_or_else(|| {
                    Error::internal(format!(
                        "bond hazard LSMC has no overnight source for {} / {} day(s)",
                        slice.observation_date, slice.rate_tenor_days
                    ))
                })?
                .rate(path)
        })?;
        Ok(())
    }

    fn advance_step(
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
                if coupon.compiled.is_live(&self.floating[coupon_id])
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
        let mut pik_to_add = 0.0;
        for &coupon_id in &template.floating_payment_ids[step] {
            let coupon = &template.floating[coupon_id];
            let settlement = coupon.compiled.settle(&self.floating[coupon_id])?;
            let cash_coupon = settlement.cash_amount;
            current_cash += cash_coupon;
            reference_cash += cash_coupon;
            self.cumulative_distribution_cash += cash_coupon.max(0.0);
            if let Some(floor) = template.return_floor {
                self.cumulative_distribution_target_pv +=
                    floor.target_pv(coupon.compiled.period().payment_date, cash_coupon.max(0.0))?;
            }
            pik_to_add += settlement.pik_amount;
            self.floating[coupon_id] = FloatingRuntimeState::default();
        }
        for event in &template.balance_events[step] {
            self.outstanding += event.delta;
        }
        // Canonical cashflow emission applies same-day amortization before PIK
        // capitalization.  The next coupon captures this after-event balance.
        self.outstanding += pik_to_add;
        if self.outstanding < -1.0e-8 || !self.outstanding.is_finite() {
            return Err(Error::Validation(format!(
                "Bond '{}' hazard LSMC replay produced invalid outstanding principal {} at step {step}",
                bond.id.as_str(), self.outstanding
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
                    return Err(Error::internal(
                        "bond hazard LSMC overnight coupon has a term reset event",
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

impl ReplayTemplate {
    fn new(
        tree: &RatesCreditTree,
        bond: &Bond,
        market: &MarketContext,
        as_of: Date,
    ) -> Result<Self> {
        let times = tree.time_grid()?.to_vec();
        if times.len() < 2 || !times.windows(2).all(|window| window[1] > window[0]) {
            return Err(Error::Validation(
                "bond hazard LSMC requires a strictly increasing calibrated tree grid".to_string(),
            ));
        }
        let terminal_step = times.len() - 1;
        let step_dates = times
            .iter()
            .map(|time| {
                let day_position = time * 365.0;
                let rounded = day_position.round();
                if (day_position - rounded).abs() <= 1.0e-10 {
                    as_of
                        .checked_add(Duration::days(rounded as i64))
                        .map(Some)
                        .ok_or_else(|| {
                            Error::Validation(
                                "bond hazard LSMC grid date exceeds the supported range"
                                    .to_string(),
                            )
                        })
                } else {
                    Ok(None)
                }
            })
            .collect::<Result<Vec<_>>>()?;
        let discount = market.get_discount(&bond.discount_curve_id)?;
        let schedule = bond.full_cashflow_schedule(market)?;
        let grid_day_count = DayCount::Act365F;
        let final_redemption_date = schedule
            .get_flows()
            .iter()
            .filter(|flow| flow.kind == CFKind::Notional)
            .map(|flow| flow.date)
            .max();
        let floating_spec = floating_coupon_spec(bond);
        let return_floor = bond
            .return_floor
            .as_ref()
            .map(|spec| {
                spec.validate()?;
                Ok::<_, Error>(ReturnFloorTemplate {
                    kind: spec.kind,
                    issue_price: spec.issue_price.resolve(bond.notional)?,
                    issue_date: bond.issue_date,
                    day_count: spec.day_count.unwrap_or(DayCount::Act365F),
                })
            })
            .transpose()?;
        let include_as_of_option_cash = has_exercise_claim_on_date(bond, as_of)?;
        let mut historical_distribution_cash = 0.0;
        let mut historical_distribution_target_pv = 0.0;
        if let Some(floor) = return_floor {
            for point in realized_distributions(bond, market, bond.issue_date)? {
                if point.date >= as_of {
                    break;
                }
                if !point.outstanding.is_finite() || point.outstanding < 0.0 {
                    return Err(Error::Validation(format!(
                        "Bond '{}' has invalid historical outstanding {} at {}",
                        bond.id.as_str(),
                        point.outstanding,
                        point.date
                    )));
                }
                historical_distribution_cash = point.cum_before + point.coupon;
                historical_distribution_target_pv += floor.target_pv(point.date, point.coupon)?;
            }
        }

        // Start immediately before the origin's contractual events. The replay
        // then applies same-day PIK/amortization before exercise while leaving
        // a same-day final redemption available as the holder's terminal claim.
        let mut initial_outstanding = bond.notional.amount();
        for flow in schedule.get_flows().iter().filter(|flow| flow.date < as_of) {
            if flow.kind == CFKind::Notional
                && flow.date == bond.issue_date
                && flow.amount.amount() < 0.0
            {
                continue;
            }
            let terminal_redemption = flow.kind == CFKind::Notional
                && final_redemption_date.is_some_and(|date| flow.date == date);
            if let Some(delta) = static_balance_delta(flow, terminal_redemption) {
                initial_outstanding += delta;
            }
        }
        if !initial_outstanding.is_finite() || initial_outstanding < -1.0e-8 {
            return Err(Error::Validation(format!(
                "Bond '{}' has invalid principal immediately before {as_of}: {initial_outstanding}",
                bond.id.as_str()
            )));
        }
        initial_outstanding = initial_outstanding.max(0.0);

        let mut dynamic_groups = BTreeMap::<(Date, Date, Date), FloatingBuild>::new();
        let mut dynamic_flow_indices = BTreeSet::new();
        if let Some(spec) = floating_spec {
            for (flow_index, flow) in schedule.get_flows().iter().enumerate() {
                if !matches!(flow.kind, CFKind::FloatReset | CFKind::Pik) || flow.date < as_of {
                    continue;
                }
                let Some(accrual) = flow.accrual else {
                    continue;
                };
                if accrual.projected_index_rate.is_none() {
                    continue;
                }
                let reset = flow
                    .reset_date
                    .unwrap_or(contractual_reset_date(accrual.start, spec)?);
                let dynamic = flow.date > as_of || include_as_of_option_cash;
                if !dynamic {
                    continue;
                }
                let key = (accrual.start, accrual.end, flow.date);
                let entry = dynamic_groups.entry(key).or_default();
                entry.reset = Some(reset);
                entry.start = Some(accrual.start);
                entry.end = Some(accrual.end);
                entry.payment = Some(flow.date);
                entry.day_count = Some(accrual.day_count);
                entry.accrual = flow.accrual_factor;
                entry.base_index_rate = accrual.projected_index_rate;
                dynamic_flow_indices.insert(flow_index);
            }
        }

        let (cash_fraction, pik_fraction) = floating_spec
            .map(|spec| coupon_fractions(spec.coupon_type))
            .transpose()?
            .unwrap_or((0.0, 0.0));
        let mut params = floating_spec
            .map(|spec| params_from_spec(&spec.rate_spec))
            .unwrap_or_default();
        if floating_spec.is_some_and(|spec| spec.rate_spec.overnight_compounding.is_some()) {
            params.index_floor_bp = None;
            params.index_cap_bp = None;
        }
        params.validate()?;

        let mut floating = Vec::with_capacity(dynamic_groups.len());
        for group in dynamic_groups.into_values() {
            let reset = required_date(group.reset, "reset")?;
            let start = required_date(group.start, "accrual start")?;
            let end = required_date(group.end, "accrual end")?;
            let payment = required_date(group.payment, "payment")?;
            let day_count = group.day_count.ok_or_else(|| {
                Error::Validation("floating coupon is missing day-count metadata".to_string())
            })?;
            if !group.accrual.is_finite() || group.accrual <= 0.0 {
                return Err(Error::Validation(format!(
                    "Bond '{}' floating coupon {} to {} has invalid accrual factor {}",
                    bond.id.as_str(),
                    start,
                    end,
                    group.accrual
                )));
            }
            let reset_time = if reset <= as_of {
                0.0
            } else {
                grid_day_count.year_fraction(as_of, reset, DayCountContext::default())?
            };
            let start_time = if start <= as_of {
                0.0
            } else {
                grid_day_count.year_fraction(as_of, start, DayCountContext::default())?
            };
            let reset_step = nearest_step(&times, reset_time);
            let accrual_start_step = nearest_step(&times, start_time);
            let payment_step = exact_grid_step(&times, as_of, payment)?;
            if accrual_start_step >= payment_step {
                return Err(Error::Validation(format!(
                    "Bond '{}' floating coupon accruing {} to {} (paid {}) maps accrual start and payment to step {}; use a daily rates-credit grid through the adjusted payment date",
                    bond.id.as_str(), start, end, payment, accrual_start_step
                )));
            }
            let spec = floating_spec.ok_or_else(|| {
                Error::internal("bond hazard LSMC dynamic floating coupon has no specification")
            })?;
            let base_index_rate = group.base_index_rate.ok_or_else(|| {
                Error::Validation(format!(
                    "Bond '{}' floating coupon {} to {} is missing projected_index_rate metadata required by hazard LSMC",
                    bond.id.as_str(), start, end
                ))
            })?;
            let (rate_model, observation) = if spec.rate_spec.overnight_compounding.is_some() {
                let (overnight_model, overnight_observation) = build_overnight_coupon(
                    tree,
                    market,
                    discount.as_ref(),
                    as_of,
                    &times,
                    start,
                    end,
                    spec,
                )?;
                (
                    FloatingRateModel::Overnight(overnight_model),
                    overnight_observation,
                )
            } else {
                let forward = market.get_forward(&spec.rate_spec.index_id).ok();
                let tenor_years = forward.as_ref().map_or_else(
                    || {
                        spec.rate_spec
                            .index_tenor
                            .unwrap_or(spec.rate_spec.reset_frequency)
                            .to_years()
                    },
                    |curve| curve.tenor(),
                );
                let fixing_id = fixing_series_id(spec.rate_spec.index_id.as_str());
                let published_same_day = reset == as_of
                    && market
                        .get_series(&fixing_id)
                        .ok()
                        .is_some_and(|series| series.value_on_exact(reset).is_ok());
                let known_reset = reset < as_of || published_same_day;
                let source = if known_reset || forward.is_none() {
                    ObservedRateSource::Fixed(base_index_rate)
                } else {
                    let observation_end_time = reset_time + tenor_years;
                    if observation_end_time > times[terminal_step] + 1.0e-12 {
                        return Err(Error::Validation(format!(
                                "Bond '{}' term-index observation at {} with tenor {:.12}y ends beyond the calibrated rates-credit horizon {:.12}y",
                                bond.id.as_str(), reset, tenor_years, times[terminal_step]
                            )));
                    }
                    let observation_end_step = nearest_step(&times, observation_end_time);
                    if observation_end_step <= reset_step {
                        return Err(Error::Validation(format!(
                                "Bond '{}' term-index observation at {} with tenor {:.12}y maps to no positive rates-credit interval",
                                bond.id.as_str(), reset, tenor_years
                            )));
                    }
                    let index_accrual = times[observation_end_step] - times[reset_step];
                    let reset_grid_date = step_dates[reset_step].ok_or_else(|| {
                        Error::Validation(format!(
                            "Bond '{}' term-index reset step {} has no calendar date",
                            bond.id.as_str(),
                            reset_step
                        ))
                    })?;
                    let observation_end_date =
                        step_dates[observation_end_step].ok_or_else(|| {
                            Error::Validation(format!(
                                "Bond '{}' term-index observation-end step {} has no calendar date",
                                bond.id.as_str(),
                                observation_end_step
                            ))
                        })?;
                    let base_df =
                        discount.df_between_dates(reset_grid_date, observation_end_date)?;
                    if !base_df.is_finite() || base_df <= 0.0 {
                        return Err(Error::Validation(format!(
                                "Bond '{}' has an invalid reference discount factor over the term-index observation [{:.12}, {:.12}]",
                                bond.id.as_str(), times[reset_step], times[observation_end_step]
                            )));
                    }
                    ObservedRateSource::Conditional(ConditionalRate {
                        observation_step: reset_step,
                        accrual: index_accrual,
                        base_index_rate,
                        base_discount_forward: (1.0 / base_df - 1.0) / index_accrual,
                        conditional_discount_factors: tree.conditional_discount_factors(
                            reset_step,
                            observation_end_step,
                            times[terminal_step],
                        )?,
                    })
                };
                (
                    FloatingRateModel::Term(source),
                    FloatingRateObservation::Term {
                        reset_date: reset,
                        tenor_years,
                    },
                )
            };
            let compiled = CompiledFloatingCoupon::compile(
                FloatingCouponPeriod {
                    accrual_start: start,
                    accrual_end: end,
                    payment_date: payment,
                    day_count,
                    accrual_factor: group.accrual,
                },
                FloatingCouponEconomics {
                    cash_fraction,
                    pik_fraction,
                    rate_params: params.clone(),
                },
                observation,
            )?;
            let initial_notional = (start < as_of)
                .then(|| {
                    scheduled_outstanding_after(
                        bond,
                        schedule.get_flows(),
                        start,
                        final_redemption_date,
                    )
                })
                .transpose()?;
            let initial_term_rate = match &rate_model {
                FloatingRateModel::Term(ObservedRateSource::Fixed(rate)) if reset <= as_of => {
                    Some(*rate)
                }
                _ => None,
            };
            floating.push(FloatingCoupon {
                reset_step,
                accrual_start_step,
                payment_step,
                compiled,
                rate_model,
                initial_notional,
                initial_term_rate,
            });
        }

        let mut exercise_by_date = BTreeMap::<Date, ExerciseDate>::new();
        let mut make_whole_claims = Vec::new();
        let mut make_whole_bases = Vec::<MakeWholeBasis>::new();
        if let Some(call_put) = &bond.call_put {
            for call in &call_put.calls {
                for date in exercise_dates(call, as_of, bond.maturity) {
                    let entry = exercise_by_date
                        .entry(date)
                        .or_insert_with(|| ExerciseDate {
                            date,
                            calls: Vec::new(),
                            puts: Vec::new(),
                            return_floor: false,
                        });
                    let has_later_reference_cash = schedule
                        .get_flows()
                        .iter()
                        .any(|flow| flow.date > date && is_cash_settlement_kind(flow.kind));
                    let make_whole = if call.make_whole.is_none() {
                        None
                    } else if date == bond.maturity && !has_later_reference_cash {
                        // Exercise occurs after same-day holder cash. At the
                        // terminal decision there are no later reference-basis
                        // cashflows to regress; the clean contractual call
                        // floor remains authoritative. A rolled payment after
                        // contractual maturity remains a live reference claim.
                        Some(MakeWholeExercise::Deterministic(0.0))
                    } else if tree.config.rate_vol > 0.0 {
                        let claim_id = make_whole_claims.len();
                        let (exercise_step, basis) = build_make_whole_claim(
                            call,
                            date,
                            discount.as_ref(),
                            market,
                            as_of,
                            &times,
                        )?;
                        let basis_index = make_whole_bases
                            .iter()
                            .position(|existing| {
                                existing.interval_adjustments == basis.interval_adjustments
                            })
                            .unwrap_or_else(|| {
                                let index = make_whole_bases.len();
                                make_whole_bases.push(basis);
                                index
                            });
                        make_whole_claims.push(MakeWholeClaim {
                            exercise_step,
                            decision_index: usize::MAX,
                            basis_index,
                        });
                        Some(MakeWholeExercise::Conditional(claim_id))
                    } else {
                        make_whole_value(call, date, schedule.get_flows(), market)?
                            .map(MakeWholeExercise::Deterministic)
                    };
                    entry.calls.push(ExerciseCall {
                        price_pct_of_par: call.price_pct_of_par,
                        make_whole,
                    });
                }
            }
            for put in &call_put.puts {
                for date in exercise_dates(put, as_of, bond.maturity) {
                    let entry = exercise_by_date
                        .entry(date)
                        .or_insert_with(|| ExerciseDate {
                            date,
                            calls: Vec::new(),
                            puts: Vec::new(),
                            return_floor: false,
                        });
                    entry.puts.push(put.clone());
                }
            }
        }
        if let Some(spec) = bond.return_floor.as_ref() {
            for date in return_floor_dates(spec.window, bond.issue_date, bond.maturity, as_of)? {
                exercise_by_date
                    .entry(date)
                    .or_insert_with(|| ExerciseDate {
                        date,
                        calls: Vec::new(),
                        puts: Vec::new(),
                        return_floor: false,
                    })
                    .return_floor = true;
            }
        }
        let include_as_of_cash = exercise_by_date.contains_key(&as_of);
        let entitled_cashflows = if include_as_of_cash {
            bond.pricing_dated_cashflows_from_schedule_inclusive(&schedule, as_of, as_of)?
        } else {
            bond.pricing_dated_cashflows_from_schedule(&schedule, as_of, as_of)?
        };
        let mut entitled_counts = BTreeMap::<(Date, u64), usize>::new();
        for (date, amount) in entitled_cashflows {
            *entitled_counts
                .entry((date, amount.amount().to_bits()))
                .or_default() += 1;
        }

        let mut static_cash = vec![Vec::new(); times.len()];
        let mut balance_events = vec![Vec::new(); times.len()];
        let mut static_accruals = Vec::new();
        let mut static_distributions = vec![Vec::new(); times.len()];
        for (flow_index, flow) in schedule.get_flows().iter().enumerate() {
            if flow.date < as_of || dynamic_flow_indices.contains(&flow_index) {
                continue;
            }
            if flow.kind == CFKind::Notional
                && flow.date == bond.issue_date
                && flow.amount.amount() < 0.0
            {
                // The replay starts with issued principal already outstanding;
                // the negative inception exchange is an investor cashflow,
                // not a second draw into the modeled balance.
                continue;
            }
            let event_time =
                grid_day_count.year_fraction(as_of, flow.date, DayCountContext::default())?;
            let step = nearest_step(&times, event_time);
            let terminal_redemption = flow.kind == CFKind::Notional
                && final_redemption_date.is_some_and(|date| flow.date == date);
            if is_cash_settlement_kind(flow.kind) && !terminal_redemption {
                let entitlement_key = (flow.date, flow.amount.amount().to_bits());
                let entitled = entitled_counts
                    .get_mut(&entitlement_key)
                    .is_some_and(|count| {
                        if *count == 0 {
                            false
                        } else {
                            *count -= 1;
                            true
                        }
                    });
                if entitled {
                    let event_minus_step = event_time - times[step];
                    if event_minus_step.abs() > 1.0e-10 {
                        return Err(Error::Validation(format!(
                            "Bond '{}' cashflow date {} is not an exact node on the daily ACT/365F rates-credit grid (offset {event_minus_step}); rebuild calibration with build_daily_bond_rates_credit_targets",
                            bond.id.as_str(), flow.date
                        )));
                    }
                    static_cash[step].push(CashEvent {
                        amount_at_step: flow.amount.amount(),
                        event_minus_step,
                    });
                }
            }
            if flow.date >= as_of {
                if let Some(delta) = static_balance_delta(flow, terminal_redemption) {
                    balance_events[step].push(BalanceEvent { delta });
                }
            }
            if is_holder_distribution(flow) {
                static_distributions[step].push(DistributionEvent {
                    date: flow.date,
                    amount: flow.amount.amount().max(0.0),
                });
            }
            if let Some(accrual) = flow
                .accrual
                .filter(|_| matches!(flow.kind, CFKind::Fixed | CFKind::FloatReset | CFKind::Pik))
            {
                static_accruals.push(AccrualClaim {
                    start: accrual.start,
                    end: accrual.end,
                    payment: flow.date,
                    day_count: accrual.day_count,
                    amount: flow.amount.amount(),
                    pik: flow.kind == CFKind::Pik,
                });
            }
        }

        let mut exercise = vec![Vec::new(); times.len()];
        for item in exercise_by_date.into_values() {
            let event_time =
                grid_day_count.year_fraction(as_of, item.date, DayCountContext::default())?;
            exercise[nearest_step(&times, event_time)].push(item);
        }
        let mut decision_steps = vec![0, terminal_step];
        decision_steps.extend(
            exercise
                .iter()
                .enumerate()
                .filter(|(_, entries)| !entries.is_empty())
                .map(|(step, _)| step),
        );
        decision_steps.sort_unstable();
        decision_steps.dedup();
        let redemption_step = final_redemption_date
            .filter(|date| *date > as_of || (*date == as_of && include_as_of_cash))
            .map(|date| exact_grid_step(&times, as_of, date))
            .transpose()?;
        if let Some(step) = redemption_step {
            if decision_steps.binary_search(&step).is_err() {
                decision_steps.push(step);
                decision_steps.sort_unstable();
            }
        }
        for claim in &mut make_whole_claims {
            claim.decision_index =
                decision_steps
                    .binary_search(&claim.exercise_step)
                    .map_err(|_| {
                        Error::internal(
                            "bond hazard LSMC make-whole claim has no matching decision step",
                        )
                    })?;
        }

        let mut floating_reset_ids = vec![Vec::new(); times.len()];
        let mut floating_accrual_start_ids = vec![Vec::new(); times.len()];
        let mut floating_payment_ids = vec![Vec::new(); times.len()];
        for (id, coupon) in floating.iter().enumerate() {
            if matches!(coupon.rate_model, FloatingRateModel::Term(_))
                && coupon.initial_term_rate.is_none()
            {
                floating_reset_ids[coupon.reset_step].push(id);
            }
            if coupon.initial_notional.is_none() {
                floating_accrual_start_ids[coupon.accrual_start_step].push(id);
            }
            floating_payment_ids[coupon.payment_step].push(id);
        }

        let max_rate_history_steps = floating
            .iter()
            .filter_map(|coupon| match &coupon.rate_model {
                FloatingRateModel::Overnight(overnight) => Some(overnight.max_history_steps),
                FloatingRateModel::Term(_) => None,
            })
            .max()
            .unwrap_or(0);

        Ok(Self {
            times,
            step_dates,
            static_cash,
            balance_events,
            floating,
            floating_reset_ids,
            floating_accrual_start_ids,
            floating_payment_ids,
            static_accruals,
            static_distributions,
            exercise,
            decision_steps,
            initial_outstanding,
            redemption_step,
            call_friction_cents: bond
                .instrument_pricing_overrides
                .model_config
                .call_friction_cents
                .unwrap_or(0.0),
            recovery_rate: tree.recovery_rate(),
            return_floor,
            historical_distribution_cash,
            historical_distribution_target_pv,
            make_whole_claims,
            make_whole_bases,
            max_rate_history_steps,
        })
    }

    #[allow(clippy::too_many_arguments)]
    fn decision_snapshot(
        &self,
        bond: &Bond,
        step: usize,
        path: &[RatesCreditPathState],
        state: StepReplay,
        floating: &[FloatingRuntimeState],
        exercise_provider: Option<&dyn BondLsmcExerciseProvider>,
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
                && step < coupon.payment_step
                && coupon.compiled.captured_notional(&floating[id]).is_some()
                && coupon
                    .compiled
                    .locked_term_index_rate(&floating[id])
                    .is_some()
            {
                locked_coupon += coupon.compiled.settle(&floating[id])?.total_amount;
            }
        }
        let factor = path_state(path, step)?;
        let (call, put) = self.exercise_amounts(ExerciseInputs {
            bond,
            step,
            path,
            outstanding: state.outstanding,
            locked_coupon,
            coupon_states: floating,
            cumulative_distribution_cash: state.cumulative_distribution_cash,
            cumulative_distribution_target_pv: state.cumulative_distribution_target_pv,
            provider: exercise_provider,
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
                state.outstanding
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

    #[allow(clippy::too_many_arguments)]
    fn replay_block(
        &self,
        tree: &RatesCreditTree,
        bond: &Bond,
        config: &BondLsmcConfig,
        seed: u64,
        path_index: u64,
        antithetic: bool,
        checkpoint: &TrainingCheckpoint,
        low_decision: usize,
        high_decision: usize,
        exercise_provider: Option<&dyn BondLsmcExerciseProvider>,
        make_whole_policies: Option<&[RegressionPolicy]>,
        path: &mut Vec<RatesCreditPathState>,
    ) -> Result<BlockPathRecord> {
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
        let mut step_states = Vec::with_capacity(high_step - low_step + 1);
        let mut decision_snapshots = Vec::with_capacity(high_decision - low_decision + 1);
        let mut next_decision = low_decision;
        for step in low_step..=high_step {
            let state = cursor.advance_step(self, bond, path, config, step)?;
            step_states.push(state);
            if next_decision <= high_decision && self.decision_steps[next_decision] == step {
                decision_snapshots.push(self.decision_snapshot(
                    bond,
                    step,
                    path,
                    state,
                    &cursor.floating,
                    exercise_provider,
                    make_whole_policies,
                )?);
                next_decision += 1;
            }
        }
        if decision_snapshots.len() != high_decision - low_decision + 1 {
            return Err(Error::internal(
                "bond hazard LSMC block replay missed a decision snapshot",
            ));
        }
        let oas = config.oas_bp / 10_000.0;
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
                let state = step_states[step - low_step];
                let dt = self.times[step + 1] - self.times[step];
                let interval_df = factor.discount_to_next * (-oas * dt).exp();
                let recovery = self.recovery_rate
                    * state.outstanding
                    * continuous_frp_weight(interval_df, factor.survival_to_next)?;
                let cash = if step > from { state.current_cash } else { 0.0 };
                b += a * (recovery + cash);
                a *= interval_df * factor.survival_to_next;
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

    fn replay(
        &self,
        _tree: &RatesCreditTree,
        bond: &Bond,
        path: &[RatesCreditPathState],
        config: &BondLsmcConfig,
        exercise_provider: Option<&dyn BondLsmcExerciseProvider>,
        make_whole_policies: Option<&[RegressionPolicy]>,
    ) -> Result<PathRecord> {
        if path.len() != self.times.len() {
            return Err(Error::Validation(format!(
                "bond hazard LSMC path has {} states but calibrated grid has {}",
                path.len(),
                self.times.len()
            )));
        }
        let oas = config.oas_bp / 10_000.0;
        let mut cursor = ReplayCursor::new(self)?;
        let mut step_cash = vec![0.0; self.times.len()];
        let mut outstanding_after_events = vec![0.0; self.times.len()];
        let mut snapshots = Vec::with_capacity(self.decision_steps.len());
        let mut next_decision = 0_usize;

        for step in 0..self.times.len() {
            let state = cursor.advance_step(self, bond, path, config, step)?;
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
                    exercise_provider,
                    make_whole_policies,
                )?);
                next_decision += 1;
            }
        }

        if snapshots.len() != self.decision_steps.len() {
            return Err(Error::internal(
                "bond hazard LSMC full replay missed a decision snapshot",
            ));
        }

        for decision in 0..snapshots.len().saturating_sub(1) {
            let from = snapshots[decision].step;
            let to = snapshots[decision + 1].step;
            let mut a = 1.0;
            let mut b = 0.0;
            for step in (from..to).rev() {
                let dt = self.times[step + 1] - self.times[step];
                let interval_df = path[step].discount_to_next * (-oas * dt).exp();
                let q = interval_df * path[step].survival_to_next;
                let recovery = self.recovery_rate
                    * outstanding_after_events[step]
                    * continuous_frp_weight(interval_df, path[step].survival_to_next)?;
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

    fn accrued_state(
        &self,
        date: Date,
        coupon_states: &[FloatingRuntimeState],
    ) -> Result<(f64, f64)> {
        let mut cash = 0.0;
        let mut pik = 0.0;
        for claim in &self.static_accruals {
            if date <= claim.start || date >= claim.payment {
                continue;
            }
            let elapsed_end = date.min(claim.end);
            let elapsed = claim.day_count.year_fraction(
                claim.start,
                elapsed_end,
                DayCountContext::default(),
            )?;
            let total = claim.day_count.year_fraction(
                claim.start,
                claim.end,
                DayCountContext::default(),
            )?;
            let amount = claim.amount * safe_accrual_ratio(elapsed, total);
            if claim.pik {
                pik += amount;
            } else {
                cash += amount;
            }
        }
        for (id, coupon) in self.floating.iter().enumerate() {
            let period = coupon.compiled.period();
            if date <= period.accrual_start || date >= period.payment_date {
                continue;
            }
            let accrued_amount = coupon.compiled.accrued_amount(&coupon_states[id], date)?;
            cash += accrued_amount * coupon.compiled.economics().cash_fraction;
            pik += accrued_amount * coupon.compiled.economics().pik_fraction;
        }
        Ok((cash, pik))
    }

    fn exercise_amounts(&self, inputs: ExerciseInputs<'_>) -> Result<(Option<f64>, Option<f64>)> {
        let ExerciseInputs {
            bond,
            step,
            path,
            outstanding,
            locked_coupon,
            coupon_states,
            cumulative_distribution_cash,
            cumulative_distribution_target_pv,
            provider,
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
                            Error::internal(
                                "bond hazard LSMC return-floor date has no floor specification",
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
                                    Error::internal(format!(
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
            let amounts = if let Some(provider) = provider {
                let state = BondLsmcExerciseState {
                    bond,
                    date: entry.date,
                    step,
                    short_rate: factor.short_rate,
                    hazard_rate: factor.hazard_rate,
                    outstanding,
                    locked_coupon,
                    accrued_cash,
                    accrued_pik,
                    cumulative_distribution_cash,
                    cumulative_distribution_target_pv,
                    default_call,
                    default_put,
                };
                state.validate()?;
                provider.exercise_amounts(&state)?
            } else {
                BondLsmcExerciseAmounts {
                    call: default_call,
                    put: default_put,
                }
            };
            validate_exercise_amount("call", amounts.call)?;
            validate_exercise_amount("put", amounts.put)?;
            if let Some(value) = amounts.call {
                call = Some(call.map_or(value, |existing| existing.min(value)));
            }
            if let Some(value) = amounts.put {
                put = Some(put.map_or(value, |existing| existing.max(value)));
            }
        }
        if let (Some(call), Some(put)) = (call, put) {
            if put > call + 1.0e-10 {
                return Err(Error::Validation(format!(
                    "Bond '{}' has incompatible path-dependent barriers at step {step}: holder put {put} exceeds issuer call {call}",
                    bond.id.as_str()
                )));
            }
        }
        Ok((call, put))
    }
}

fn required_date(value: Option<Date>, label: &str) -> Result<Date> {
    value.ok_or_else(|| Error::Validation(format!("floating coupon is missing {label} metadata")))
}

fn floating_coupon_spec(bond: &Bond) -> Option<&FloatingCouponSpec> {
    match &bond.cashflow_spec {
        CashflowSpec::Floating(spec) => Some(spec),
        CashflowSpec::Amortizing { base, .. } => match base.as_ref() {
            CashflowSpec::Floating(spec) => Some(spec),
            _ => None,
        },
        _ => None,
    }
}

fn contractual_reset_date(start: Date, spec: &FloatingCouponSpec) -> Result<Date> {
    let calendar_id = spec
        .rate_spec
        .fixing_calendar_id
        .as_deref()
        .unwrap_or(&spec.schedule.calendar_id);
    let calendar = resolve_calendar_strict(calendar_id)?;
    if spec.rate_spec.reset_lag_days == 0 {
        return Ok(start);
    }
    let reset = start.add_business_days(-spec.rate_spec.reset_lag_days, calendar)?;
    adjust(reset, spec.schedule.business_day_convention, calendar)
}

#[allow(clippy::too_many_arguments)]
fn build_overnight_coupon(
    tree: &RatesCreditTree,
    market: &MarketContext,
    discount: &dyn Discounting,
    as_of: Date,
    times: &[f64],
    start: Date,
    end: Date,
    spec: &FloatingCouponSpec,
) -> Result<(OvernightCoupon, FloatingRateObservation)> {
    let method = spec.rate_spec.overnight_compounding.ok_or_else(|| {
        Error::internal("bond hazard LSMC overnight replay has no compounding method")
    })?;
    let day_count = spec
        .rate_spec
        .overnight_basis
        .unwrap_or(spec.schedule.day_count);
    let day_count_basis = match day_count {
        DayCount::Act360 => 360.0,
        DayCount::Act365F => 365.0,
        other => {
            return Err(Error::Validation(format!(
                "bond hazard LSMC overnight basis must be Act360 or Act365F, got {other:?}"
            )))
        }
    };
    let calendar_id = spec
        .rate_spec
        .fixing_calendar_id
        .as_deref()
        .unwrap_or(&spec.schedule.calendar_id);
    let calendar = resolve_calendar_strict(calendar_id)?;
    let observations = OvernightObservationSchedule::compile(start, end, method, calendar)?;
    if observations.observations().is_empty() && start < end {
        return Err(Error::Validation(format!(
            "bond hazard LSMC overnight accrual [{start}, {end}) contains no observations on calendar '{calendar_id}'"
        )));
    }

    let constraints = OvernightRateConstraints {
        application: spec.rate_spec.overnight_index_constraints,
        index_floor_bp: decimal_option_to_f64(
            spec.rate_spec.index_floor_bp,
            "overnight index floor",
        )?,
        index_cap_bp: decimal_option_to_f64(spec.rate_spec.index_cap_bp, "overnight index cap")?,
    };
    let index_id = spec.rate_spec.index_id.as_str();
    let fixing_id = fixing_series_id(index_id);
    let fixings = market.get_series(&fixing_id).ok();
    let forward = market.get_forward(index_id).ok();
    let horizon = *times.last().ok_or_else(|| {
        Error::internal("bond hazard LSMC overnight replay has an empty tree grid")
    })?;
    let mut sources = BTreeMap::new();

    for observation in observations.observations() {
        let max_tenor = observation
            .rate_tenor_days
            .max(observation.weight_days)
            .max(1);
        for tenor_days in 1..=max_tenor {
            let key = (observation.observation_date, tenor_days);
            if sources.contains_key(&key) {
                continue;
            }
            let observation_date = observation.observation_date;
            let published_same_day = observation_date == as_of
                && fixings.is_some_and(|series| series.value_on_exact(observation_date).is_ok());
            let source = if observation_date < as_of || published_same_day {
                match require_fixing_value_exact(fixings, index_id, observation_date, as_of) {
                    Ok(rate) => OvernightRateSource::Fixed(rate),
                    Err(error) => OvernightRateSource::Fixed(fallback_index_rate(
                        &spec.rate_spec.fallback,
                        error,
                    )?),
                }
            } else if let Some(forward) = forward.as_ref() {
                if observation_date < forward.base_date() {
                    OvernightRateSource::Fixed(fallback_index_rate(
                        &spec.rate_spec.fallback,
                        Error::Validation(format!(
                            "overnight observation {observation_date} for index '{index_id}' precedes forward-curve base {}",
                            forward.base_date()
                        )),
                    )?)
                } else {
                    let observation_end = observation_date
                        .checked_add(Duration::days(i64::from(tenor_days)))
                        .ok_or_else(|| {
                            Error::Validation(
                                "bond hazard LSMC overnight observation end overflows the supported date range"
                                    .to_string(),
                            )
                        })?;
                    let curve_time = forward.day_count().year_fraction(
                        forward.base_date(),
                        observation_date,
                        DayCountContext::default(),
                    )?;
                    let accrual = f64::from(tenor_days) / day_count_basis;
                    let base_index_rate = forward.rate_period(curve_time, curve_time + accrual);
                    let base_df = discount.df_between_dates(observation_date, observation_end)?;
                    if !base_df.is_finite() || base_df <= 0.0 {
                        return Err(Error::Validation(format!(
                            "bond hazard LSMC has an invalid discount factor for overnight observation {observation_date}"
                        )));
                    }
                    let observation_step = exact_grid_step(times, as_of, observation_date)?;
                    let end_step = exact_grid_step(times, as_of, observation_end)?;
                    if observation_step >= end_step {
                        return Err(Error::Validation(format!(
                            "bond hazard LSMC overnight observation {observation_date} to {observation_end} maps to one tree step"
                        )));
                    }
                    OvernightRateSource::Conditional(ConditionalRate {
                        observation_step,
                        accrual,
                        base_index_rate,
                        base_discount_forward: (1.0 / base_df - 1.0) / accrual,
                        conditional_discount_factors: tree.conditional_discount_factors(
                            observation_step,
                            end_step,
                            horizon,
                        )?,
                    })
                }
            } else {
                OvernightRateSource::Fixed(fallback_index_rate(
                    &spec.rate_spec.fallback,
                    Error::Validation(format!(
                        "forward curve '{index_id}' is required for stochastic overnight observation {observation_date}"
                    )),
                )?)
            };
            sources.insert(key, source);
        }
    }

    let mut max_history_steps = 0_usize;
    for observation in observations.observations() {
        let Some(OvernightRateSource::Conditional(source)) =
            sources.get(&(observation.observation_date, observation.rate_tenor_days))
        else {
            continue;
        };
        let weight_end_step = exact_grid_step(times, as_of, observation.weight_end)?;
        max_history_steps =
            max_history_steps.max(weight_end_step.saturating_sub(source.observation_step));
    }

    Ok((
        OvernightCoupon {
            sources,
            max_history_steps,
        },
        FloatingRateObservation::Overnight {
            schedule: observations,
            day_count_basis,
            constraints,
        },
    ))
}

fn decimal_option_to_f64(value: Option<rust_decimal::Decimal>, label: &str) -> Result<Option<f64>> {
    value
        .map(|value| {
            value.to_f64().ok_or_else(|| {
                Error::Validation(format!(
                    "bond hazard LSMC {label} cannot be represented as f64"
                ))
            })
        })
        .transpose()
}

fn fallback_index_rate(fallback: &FloatingRateFallback, error: Error) -> Result<f64> {
    match fallback {
        FloatingRateFallback::Error => Err(error),
        FloatingRateFallback::SpreadOnly => Ok(0.0),
        FloatingRateFallback::FixedRate(rate) => rate.to_f64().ok_or_else(|| {
            Error::Validation(
                "bond hazard LSMC floating fallback rate cannot be represented as f64".to_string(),
            )
        }),
    }
}

fn exact_grid_step(times: &[f64], as_of: Date, date: Date) -> Result<usize> {
    let time = DayCount::Act365F.year_fraction(as_of, date, DayCountContext::default())?;
    let step = nearest_step(times, time.max(0.0));
    let offset = times[step] - time.max(0.0);
    if offset.abs() > 1.0e-10 {
        return Err(Error::Validation(format!(
            "bond hazard LSMC date {date} is not an exact node on the daily ACT/365F grid (offset {offset})"
        )));
    }
    Ok(step)
}

fn coupon_fractions(coupon_type: CouponType) -> Result<(f64, f64)> {
    let (cash, pik) = match coupon_type {
        CouponType::Cash => (1.0, 0.0),
        CouponType::Pik => (0.0, 1.0),
        CouponType::Split { cash_pct, pik_pct } => (
            cash_pct.to_f64().ok_or_else(|| {
                Error::Validation("floating cash fraction cannot be represented as f64".to_string())
            })?,
            pik_pct.to_f64().ok_or_else(|| {
                Error::Validation("floating PIK fraction cannot be represented as f64".to_string())
            })?,
        ),
    };
    if !cash.is_finite()
        || !pik.is_finite()
        || cash < 0.0
        || pik < 0.0
        || (cash + pik - 1.0).abs() > 1.0e-9
    {
        return Err(Error::Validation(format!(
            "floating coupon cash/PIK fractions must be non-negative and sum to one, got cash={cash}, pik={pik}"
        )));
    }
    Ok((cash, pik))
}

#[allow(clippy::too_many_arguments)]
fn build_make_whole_claim(
    call: &CallPut,
    exercise_date: Date,
    discount: &dyn Discounting,
    market: &MarketContext,
    as_of: Date,
    times: &[f64],
) -> Result<(usize, MakeWholeBasis)> {
    let spec = call.make_whole.as_ref().ok_or_else(|| {
        Error::internal("bond hazard LSMC attempted to build an absent make-whole claim")
    })?;
    let reference = market.get_discount(&spec.reference_curve_id)?;
    let base_grid = relative_curve_grid(discount, as_of, times, 0.0)?;
    let reference_grid =
        relative_curve_grid(reference.as_ref(), as_of, times, spec.spread_bp / 10_000.0)?;
    let mut interval_adjustments = Vec::with_capacity(times.len().saturating_sub(1));
    for step in 0..times.len().saturating_sub(1) {
        let base_interval = base_grid[step + 1] / base_grid[step];
        let reference_interval = reference_grid[step + 1] / reference_grid[step];
        let adjustment = reference_interval / base_interval;
        if !adjustment.is_finite() || adjustment <= 0.0 {
            return Err(Error::Validation(format!(
                "make-whole reference-basis adjustment at step {step} is not positive and finite"
            )));
        }
        interval_adjustments.push(adjustment);
    }
    Ok((
        exact_grid_step(times, as_of, exercise_date)?,
        MakeWholeBasis {
            interval_adjustments,
        },
    ))
}

fn relative_curve_grid(
    curve: &dyn Discounting,
    origin: Date,
    times: &[f64],
    spread: f64,
) -> Result<Vec<f64>> {
    if !spread.is_finite() {
        return Err(Error::Validation(format!(
            "make-whole spread must be finite, got {spread}"
        )));
    }
    times
        .iter()
        .map(|time| {
            let day_position = time * 365.0;
            let lower_days = day_position.floor() as i64;
            let upper_days = day_position.ceil() as i64;
            let weight = day_position - lower_days as f64;
            let lower_date = origin
                .checked_add(Duration::days(lower_days))
                .ok_or_else(|| {
                    Error::Validation("make-whole grid date underflow or overflow".to_string())
                })?;
            let upper_date = origin
                .checked_add(Duration::days(upper_days))
                .ok_or_else(|| {
                    Error::Validation("make-whole grid date underflow or overflow".to_string())
                })?;
            let lower_df = curve.df_between_dates(origin, lower_date)?;
            let upper_df = curve.df_between_dates(origin, upper_date)?;
            if !lower_df.is_finite() || !upper_df.is_finite() || lower_df <= 0.0 || upper_df <= 0.0
            {
                return Err(Error::Validation(
                    "make-whole reference curve returned an invalid discount factor".to_string(),
                ));
            }
            let curve_df = ((1.0 - weight) * lower_df.ln() + weight * upper_df.ln()).exp();
            let lower_tau =
                curve
                    .day_count()
                    .year_fraction(origin, lower_date, DayCountContext::default())?;
            let upper_tau =
                curve
                    .day_count()
                    .year_fraction(origin, upper_date, DayCountContext::default())?;
            let tau = (1.0 - weight) * lower_tau + weight * upper_tau;
            let adjusted = curve_df * (-spread * tau).exp();
            if adjusted.is_finite() && adjusted > 0.0 {
                Ok(adjusted)
            } else {
                Err(Error::Validation(
                    "make-whole spread-adjusted reference discount is invalid".to_string(),
                ))
            }
        })
        .collect()
}

fn make_whole_value(
    call: &CallPut,
    exercise_date: Date,
    flows: &[CashFlow],
    market: &MarketContext,
) -> Result<Option<f64>> {
    let Some(spec) = &call.make_whole else {
        return Ok(None);
    };
    let reference = market.get_discount(&spec.reference_curve_id)?;
    let spread = spec.spread_bp / 10_000.0;
    let mut value = 0.0;
    for flow in flows
        .iter()
        .filter(|flow| flow.date > exercise_date && is_cash_settlement_kind(flow.kind))
    {
        let tau = reference.day_count().year_fraction(
            exercise_date,
            flow.date,
            DayCountContext::default(),
        )?;
        value += flow.amount.amount()
            * reference.df_between_dates(exercise_date, flow.date)?
            * (-spread * tau).exp();
    }
    if value.is_finite() && value >= 0.0 {
        Ok(Some(value))
    } else {
        Err(Error::Validation(format!(
            "make-whole value at {exercise_date} must be non-negative and finite, got {value}"
        )))
    }
}

fn is_holder_distribution(flow: &CashFlow) -> bool {
    flow.amount.amount() > 0.0
        && matches!(
            flow.kind,
            CFKind::Fixed | CFKind::FloatReset | CFKind::Stub | CFKind::Amortization
        )
}

fn static_balance_delta(flow: &CashFlow, terminal_redemption: bool) -> Option<f64> {
    if terminal_redemption {
        return None;
    }
    match flow.kind {
        CFKind::Pik => Some(flow.amount.amount()),
        CFKind::Amortization
        | CFKind::PrePayment
        | CFKind::DefaultedNotional
        | CFKind::Notional
        | CFKind::RevolvingDraw
        | CFKind::RevolvingRepayment => Some(-flow.amount.amount()),
        _ => None,
    }
}

fn scheduled_outstanding_after(
    bond: &Bond,
    flows: &[CashFlow],
    date: Date,
    final_redemption_date: Option<Date>,
) -> Result<f64> {
    let mut outstanding = bond.notional.amount();
    for flow in flows.iter().filter(|flow| flow.date <= date) {
        if flow.kind == CFKind::Notional
            && flow.date == bond.issue_date
            && flow.amount.amount() < 0.0
        {
            continue;
        }
        let terminal_redemption = flow.kind == CFKind::Notional
            && final_redemption_date.is_some_and(|redemption| flow.date == redemption);
        if let Some(delta) = static_balance_delta(flow, terminal_redemption) {
            outstanding += delta;
        }
    }
    if outstanding.is_finite() && outstanding >= -1.0e-8 {
        Ok(outstanding.max(0.0))
    } else {
        Err(Error::Validation(format!(
            "Bond '{}' has invalid scheduled outstanding {outstanding} after {date}",
            bond.id.as_str()
        )))
    }
}

fn has_exercise_claim_on_date(bond: &Bond, date: Date) -> Result<bool> {
    if date > bond.maturity {
        return Ok(false);
    }
    if bond.call_put.as_ref().is_some_and(|rights| {
        rights
            .calls
            .iter()
            .chain(&rights.puts)
            .any(|right| right.start_date <= date && date <= right.end_date)
    }) {
        return Ok(true);
    }
    let Some(floor) = bond.return_floor.as_ref() else {
        return Ok(false);
    };
    Ok(
        return_floor_dates(floor.window, bond.issue_date, bond.maturity, date)?
            .first()
            .is_some_and(|claim_date| *claim_date == date),
    )
}

fn exercise_dates(option: &CallPut, as_of: Date, maturity: Date) -> Vec<Date> {
    let first = option.start_date.max(as_of);
    let last = option.end_date.min(maturity);
    if first > last {
        return Vec::new();
    }
    let mut result = Vec::with_capacity((last - first).whole_days().max(0) as usize + 1);
    let mut date = first;
    loop {
        result.push(date);
        if date == last {
            break;
        }
        let Some(next) = date.next_day() else {
            break;
        };
        date = next;
    }
    result
}

fn return_floor_dates(
    window: ProtectionWindow,
    issue_date: Date,
    maturity: Date,
    as_of: Date,
) -> Result<Vec<Date>> {
    let first_life_date = issue_date.next_day().ok_or_else(|| {
        Error::Validation("return-floor issue date has no following day".to_string())
    })?;
    let last_life_date = maturity.previous_day().ok_or_else(|| {
        Error::Validation("return-floor maturity has no preceding day".to_string())
    })?;
    let (window_start, window_end) = match window {
        ProtectionWindow::Full => (first_life_date, last_life_date),
        ProtectionWindow::From(start) => (start.max(first_life_date), last_life_date),
        ProtectionWindow::Between { start, end } => {
            (start.max(first_life_date), end.min(last_life_date))
        }
    };
    let first = window_start.max(as_of);
    if first > window_end {
        return Ok(Vec::new());
    }
    let mut dates = Vec::with_capacity((window_end - first).whole_days().max(0) as usize + 1);
    let mut date = first;
    loop {
        dates.push(date);
        if date == window_end {
            break;
        }
        date = date.next_day().ok_or_else(|| {
            Error::Validation(
                "return-floor protection window exceeds the supported date range".to_string(),
            )
        })?;
    }
    Ok(dates)
}

fn nearest_step(times: &[f64], time: f64) -> usize {
    let last = times.len() - 1;
    if time <= times[0] {
        return 0;
    }
    if time >= times[last] {
        return last;
    }
    let upper = times.partition_point(|&candidate| candidate <= time);
    let lower = upper - 1;
    if time - times[lower] <= times[upper] - time {
        lower
    } else {
        upper
    }
}

fn safe_accrual_ratio(elapsed: f64, total: f64) -> f64 {
    if total > 0.0 && elapsed.is_finite() && total.is_finite() {
        (elapsed / total).clamp(0.0, 1.0)
    } else {
        0.0
    }
}

fn path_state(path: &[RatesCreditPathState], step: usize) -> Result<&RatesCreditPathState> {
    let first = path
        .first()
        .map(|state| state.step)
        .ok_or_else(|| Error::internal("bond hazard LSMC sampled path is empty"))?;
    let offset = step.checked_sub(first).ok_or_else(|| {
        Error::internal(format!(
            "bond hazard LSMC step {step} precedes sampled segment start {first}"
        ))
    })?;
    path.get(offset)
        .filter(|state| state.step == step)
        .ok_or_else(|| {
            Error::internal(format!(
                "bond hazard LSMC step {step} is outside sampled segment"
            ))
        })
}

fn validate_exercise_amount(label: &str, value: Option<f64>) -> Result<()> {
    if value.is_none_or(|amount| amount.is_finite() && amount >= 0.0) {
        Ok(())
    } else {
        Err(Error::Validation(format!(
            "bond hazard LSMC {label} amount must be non-negative and finite"
        )))
    }
}

#[derive(Clone)]
struct DecisionSnapshot {
    step: usize,
    features: [f64; FEATURE_COUNT],
    current_cash: f64,
    hold_redemption: f64,
    call: Option<f64>,
    put: Option<f64>,
    friction: f64,
    a_to_next: f64,
    b_to_next: f64,
}

impl DecisionSnapshot {
    fn requires_policy(&self) -> bool {
        self.hold_redemption == 0.0 && (self.call.is_some() || self.put.is_some())
    }

    fn exercise_value(&self, continuation_for_decision: f64, realized_hold: f64) -> f64 {
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
struct PathRecord {
    snapshots: Vec<DecisionSnapshot>,
}

type RegressionPolicy = StandardizedPolynomialPolicy<FEATURE_COUNT>;

#[allow(clippy::too_many_arguments)]
fn fit_make_whole_policies(
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
fn fit_policies(
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

fn block_requires_policy(
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

fn value_with_policy(path: &PathRecord, policies: &[Option<RegressionPolicy>]) -> Result<f64> {
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

fn fit_policy(features: &[[f64; FEATURE_COUNT]], responses: &[f64]) -> Result<RegressionPolicy> {
    RegressionPolicy::fit(features, responses, PolynomialDegree::Quadratic)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::instruments::common_impl::traits::Attributes;
    use crate::instruments::fixed_income::bond::{CallPutSchedule, CashflowSpec};
    use crate::instruments::InstrumentPricingOverrides;
    use finstack_quant_core::currency::Currency;
    use finstack_quant_core::dates::Tenor;
    use finstack_quant_core::market_data::term_structures::{DiscountCurve, ForwardCurve};
    use finstack_quant_core::math::interp::InterpStyle;
    use finstack_quant_core::money::Money;
    use finstack_quant_core::types::CurveId;
    use finstack_quant_models::trees::two_factor_rates_credit::{
        RatesCreditCalibrationTargets, RatesCreditConfig,
    };
    use rust_decimal::Decimal;

    fn synthetic_daily_template(days: usize, with_daily_make_whole: bool) -> ReplayTemplate {
        let states = days + 1;
        let make_whole_claims = if with_daily_make_whole {
            (0..days)
                .map(|decision_index| MakeWholeClaim {
                    exercise_step: decision_index,
                    decision_index,
                    basis_index: 0,
                })
                .collect()
        } else {
            Vec::new()
        };
        let make_whole_bases = if with_daily_make_whole {
            vec![MakeWholeBasis {
                interval_adjustments: vec![1.0; days],
            }]
        } else {
            Vec::new()
        };
        ReplayTemplate {
            times: (0..=days).map(|day| day as f64 / 365.0).collect(),
            step_dates: vec![None; states],
            static_cash: vec![Vec::new(); states],
            balance_events: vec![Vec::new(); states],
            floating: Vec::new(),
            floating_reset_ids: vec![Vec::new(); states],
            floating_accrual_start_ids: vec![Vec::new(); states],
            floating_payment_ids: vec![Vec::new(); states],
            static_accruals: Vec::new(),
            static_distributions: vec![Vec::new(); states],
            exercise: vec![Vec::new(); states],
            decision_steps: (0..=days).collect(),
            initial_outstanding: 100.0,
            redemption_step: Some(days),
            call_friction_cents: 0.0,
            recovery_rate: 0.4,
            return_floor: None,
            historical_distribution_cash: 0.0,
            historical_distribution_target_pv: 0.0,
            make_whole_claims,
            make_whole_bases,
            max_rate_history_steps: 0,
        }
    }

    fn stochastic_test_market(as_of: Date) -> MarketContext {
        let discount = DiscountCurve::builder("USD-OIS")
            .base_date(as_of)
            .knots([
                (0.0, 1.0),
                (1.0, (-0.03_f64).exp()),
                (2.0, (-0.06_f64).exp()),
            ])
            .interp(InterpStyle::LogLinear)
            .build()
            .expect("test discount curve");
        let term_forward = ForwardCurve::builder("USD-SOFR-3M", 0.25)
            .base_date(as_of)
            .knots([(0.0, 0.04), (2.0, 0.04)])
            .interp(InterpStyle::Linear)
            .build()
            .expect("term forward curve");
        let overnight_forward = ForwardCurve::builder("USD-SOFR", 1.0 / 360.0)
            .base_date(as_of)
            .knots([(0.0, 0.04), (2.0, 0.04)])
            .interp(InterpStyle::Linear)
            .build()
            .expect("overnight forward curve");
        MarketContext::new()
            .insert(discount)
            .insert(term_forward)
            .insert(overnight_forward)
    }

    fn stochastic_test_bond(as_of: Date, maturity: Date) -> Bond {
        Bond::builder()
            .id("LSMC_OPTION_ORDERING".into())
            .notional(Money::from((100_i64, Currency::USD)))
            .issue_date(as_of)
            .maturity(maturity)
            .cashflow_spec(
                CashflowSpec::fixed(0.08, Tenor::quarterly(), DayCount::Act365F)
                    .expect("finite coupon"),
            )
            .discount_curve_id(CurveId::new("USD-OIS"))
            .credit_curve_id_opt(Some(CurveId::new("USD-CREDIT")))
            .instrument_pricing_overrides(InstrumentPricingOverrides::default())
            .attributes(Attributes::new())
            .build()
            .expect("test bond")
    }

    fn term_pik_test_bond(as_of: Date, maturity: Date) -> Bond {
        let mut bond = stochastic_test_bond(as_of, maturity);
        bond.id = "LSMC_TERM_PIK".into();
        bond.cashflow_spec = CashflowSpec::floating_with_reset_lag(
            CurveId::new("USD-SOFR-3M"),
            200.0,
            Tenor::semi_annual(),
            DayCount::Act360,
            2,
        )
        .expect("term floating specification");
        let CashflowSpec::Floating(spec) = &mut bond.cashflow_spec else {
            unreachable!("floating constructor returned a non-floating specification");
        };
        spec.coupon_type = CouponType::Pik;
        spec.rate_spec.index_floor_bp = Some(Decimal::from(100));
        spec.rate_spec.all_in_cap_bp = Some(Decimal::from(800));
        spec.rate_spec.fallback = FloatingRateFallback::FixedRate(Decimal::new(4, 2));
        bond
    }

    fn overnight_pik_test_bond(as_of: Date, maturity: Date) -> Bond {
        let mut bond = stochastic_test_bond(as_of, maturity);
        bond.id = "LSMC_OVERNIGHT_PIK".into();
        bond.cashflow_spec = CashflowSpec::floating_with_reset_lag(
            CurveId::new("USD-SOFR"),
            200.0,
            Tenor::quarterly(),
            DayCount::Act360,
            0,
        )
        .expect("overnight floating specification");
        let CashflowSpec::Floating(spec) = &mut bond.cashflow_spec else {
            unreachable!("floating constructor returned a non-floating specification");
        };
        spec.coupon_type = CouponType::Pik;
        spec.rate_spec.overnight_compounding =
            Some(crate::cashflow::builder::OvernightCompoundingMethod::CompoundedInArrears);
        spec.rate_spec.overnight_basis = Some(DayCount::Act360);
        spec.rate_spec.fixing_calendar_id = Some("weekends_only".to_string());
        spec.rate_spec.index_floor_bp = Some(Decimal::from(100));
        spec.rate_spec.index_cap_bp = Some(Decimal::from(300));
        spec.rate_spec.fallback = FloatingRateFallback::FixedRate(Decimal::new(4, 2));
        bond
    }

    fn stochastic_test_tree(days: usize) -> RatesCreditTree {
        let times = (0..=days).map(|day| day as f64 / 365.0).collect::<Vec<_>>();
        let discount_factors = times
            .iter()
            .map(|time| (-0.03 * time).exp())
            .collect::<Vec<_>>();
        let survival_probabilities = times
            .iter()
            .map(|time| (-0.02 * time).exp())
            .collect::<Vec<_>>();
        let mut tree = RatesCreditTree::new(RatesCreditConfig {
            steps: days,
            rate_vol: 0.02,
            hazard_vol: 0.03,
            correlation: -0.25,
            rate_mean_reversion: 0.05,
            hazard_mean_reversion: 0.03,
        });
        tree.calibrate(&RatesCreditCalibrationTargets {
            times,
            discount_factors,
            survival_probabilities,
            recovery_rate: 0.4,
        })
        .expect("rates-credit calibration");
        tree
    }

    fn path_state(
        step: usize,
        short_rate: f64,
        hazard_rate: f64,
        discount_to_next: f64,
    ) -> RatesCreditPathState {
        RatesCreditPathState {
            step,
            time: step as f64,
            rate_node: 0,
            hazard_node: 0,
            short_rate,
            hazard_rate,
            discount_to_next,
            survival_to_next: (-hazard_rate).exp(),
            default_to_next: 1.0 - (-hazard_rate).exp(),
        }
    }

    #[test]
    fn quadratic_interaction_policy_recovers_surface() {
        let mut features = Vec::new();
        let mut responses = Vec::new();
        for left in -3..=3 {
            for right in -3..=3 {
                let x = left as f64 / 2.0;
                let y = right as f64 / 3.0;
                let mut row = [0.0; FEATURE_COUNT];
                row[0] = x;
                row[1] = y;
                features.push(row);
                responses.push(4.0 + 2.0 * x - 3.0 * y + 0.5 * x * x + 1.25 * x * y);
            }
        }
        let policy = fit_policy(&features, &responses).expect("quadratic policy");
        let mut probe = [0.0; FEATURE_COUNT];
        probe[0] = 0.7;
        probe[1] = -0.4;
        let expected = 4.0 + 2.0 * 0.7 - 3.0 * -0.4 + 0.5 * 0.7_f64.powi(2) + 1.25 * 0.7 * -0.4;
        let actual = policy.predict(&probe).expect("finite prediction");
        assert!((actual - expected).abs() < 1.0e-10);
    }

    #[test]
    fn bond_policy_cubic_refinement_recovers_nonquadratic_continuation() {
        let mut features = Vec::new();
        let mut responses = Vec::new();
        for left in -3..=3 {
            for right in -3..=3 {
                let x = left as f64 / 2.0;
                let y = right as f64 / 2.5;
                let mut row = [0.0; FEATURE_COUNT];
                row[0] = x;
                row[1] = y;
                features.push(row);
                responses.push(10.0 + 0.7 * x.powi(3) - 0.4 * x * y.powi(2) + 0.2 * y);
            }
        }
        let quadratic = RegressionPolicy::fit(&features, &responses, PolynomialDegree::Quadratic)
            .expect("quadratic bond policy");
        let cubic = RegressionPolicy::fit(&features, &responses, PolynomialDegree::Cubic)
            .expect("cubic bond policy refinement");
        let mut quadratic_error = 0.0;
        let mut cubic_error = 0.0;
        for (x, y) in [
            (0.35_f64, -0.55_f64),
            (0.9, 0.45),
            (-1.1, 0.2),
            (0.15, 1.05),
        ] {
            let mut row = [0.0; FEATURE_COUNT];
            row[0] = x;
            row[1] = y;
            let expected = 10.0 + 0.7 * x.powi(3) - 0.4 * x * y.powi(2) + 0.2 * y;
            quadratic_error +=
                (quadratic.predict(&row).expect("quadratic prediction") - expected).powi(2);
            cubic_error += (cubic.predict(&row).expect("cubic prediction") - expected).powi(2);
        }
        assert!(cubic_error < 1.0e-20, "cubic error={cubic_error}");
        assert!(
            cubic_error < quadratic_error * 1.0e-8,
            "cubic refinement must improve the same bond continuation sample: cubic={cubic_error}, quadratic={quadratic_error}"
        );
    }

    #[test]
    fn constant_features_drop_to_intercept_without_realized_fallback() {
        let features = vec![[1.0; FEATURE_COUNT]; 8];
        let responses = vec![2.0, 4.0, 3.0, 5.0, 1.0, 7.0, 6.0, 4.0];
        let policy = fit_policy(&features, &responses).expect("intercept policy");
        assert_eq!(policy.num_terms(), 1);
        assert!((policy.predict(&[1.0; FEATURE_COUNT]).expect("prediction") - 4.0).abs() < 1e-12);
    }

    #[test]
    fn put_precedes_call_and_incompatible_barriers_are_rejected_upstream() {
        let snapshot = DecisionSnapshot {
            step: 1,
            features: [0.0; FEATURE_COUNT],
            current_cash: 5.0,
            hold_redemption: 0.0,
            call: Some(105.0),
            put: Some(100.0),
            friction: 0.0,
            a_to_next: 1.0,
            b_to_next: 0.0,
        };
        assert_eq!(snapshot.exercise_value(90.0, 90.0), 100.0);
        assert_eq!(snapshot.exercise_value(110.0, 110.0), 105.0);
        assert_eq!(snapshot.exercise_value(102.0, 102.0), 102.0);
    }

    #[test]
    fn antithetic_budget_counts_independent_estimators() {
        assert_eq!(simulated_path_count(20_000, true).expect("count"), 40_000);
        assert_eq!(simulated_path_count(20_000, false).expect("count"), 20_000);
    }

    #[test]
    fn default_daily_training_budget_is_time_blocked_without_path_truncation() {
        let template = synthetic_daily_template(3_650, true);
        let physical_paths = simulated_path_count(DEFAULT_BOND_LSMC_PATHS, true).expect("count");
        let block_len = training_block_len(&template, physical_paths, 3_650)
            .expect("ten-year daily training plan must fit its memory bound");
        let boundaries = training_boundaries(&template, physical_paths).expect("boundaries");
        assert!(block_len > 0 && block_len < 3_650);
        assert_eq!(boundaries.first(), Some(&0));
        assert_eq!(boundaries.last(), Some(&3_650));
        assert_eq!(physical_paths, 40_000);
    }

    #[test]
    fn stochastic_option_ordering_is_reproducible_with_exact_stage_counts() {
        let as_of = time::macros::date!(2025 - 01 - 01);
        let maturity = time::macros::date!(2025 - 04 - 01);
        let exercise = time::macros::date!(2025 - 02 - 15);
        let market = stochastic_test_market(as_of);
        let tree = stochastic_test_tree(90);
        let bullet = stochastic_test_bond(as_of, maturity);
        let mut callable = bullet.clone();
        callable.call_put = Some(CallPutSchedule {
            calls: vec![CallPut {
                start_date: exercise,
                end_date: exercise,
                price_pct_of_par: 80.0,
                make_whole: None,
            }],
            puts: Vec::new(),
        });
        let mut puttable = bullet.clone();
        puttable.call_put = Some(CallPutSchedule {
            calls: Vec::new(),
            puts: vec![CallPut {
                start_date: exercise,
                end_date: exercise,
                price_pct_of_par: 120.0,
                make_whole: None,
            }],
        });
        let config = BondLsmcConfig {
            paths: 128,
            antithetic: true,
            seed: 0x5eed_cafe,
            oas_bp: 17.0,
            target_ci_half_width: None,
        };
        let bullet_result =
            price_bond_lsmc(&tree, &bullet, &market, as_of, &config, None).expect("bullet");
        let call_result =
            price_bond_lsmc(&tree, &callable, &market, as_of, &config, None).expect("callable");
        let repeated_call = price_bond_lsmc(&tree, &callable, &market, as_of, &config, None)
            .expect("repeated callable");
        let put_result =
            price_bond_lsmc(&tree, &puttable, &market, as_of, &config, None).expect("puttable");
        let bullet_value = bullet_result.estimate.mean.amount();
        let call_value = call_result.estimate.mean.amount();
        let put_value = put_result.estimate.mean.amount();
        assert!(
            call_value < bullet_value,
            "issuer call must lower value: call={call_value}, bullet={bullet_value}"
        );
        assert!(
            bullet_value < put_value,
            "holder put must raise value: bullet={bullet_value}, put={put_value}"
        );
        assert_eq!(
            call_value,
            repeated_call.estimate.mean.amount(),
            "fixed seed and grid must reproduce the fitted-policy estimate"
        );
        assert_eq!(bullet_result.training_paths, 0);
        assert_eq!(call_result.training_paths, 128);
        assert_eq!(call_result.training_simulated_paths, 256);
        assert_eq!(call_result.pricing_simulated_paths, 256);
        assert_eq!(put_result.training_paths, 128);
    }

    #[test]
    fn issue_date_initial_exchange_does_not_double_replay_outstanding() {
        let as_of = time::macros::date!(2025 - 01 - 01);
        let maturity = time::macros::date!(2025 - 04 - 01);
        let market = stochastic_test_market(as_of);
        let tree = stochastic_test_tree(90);
        let bond = stochastic_test_bond(as_of, maturity);
        let template = ReplayTemplate::new(&tree, &bond, &market, as_of).expect("template");
        assert_eq!(template.initial_outstanding, 100.0);
        let config = BondLsmcConfig {
            paths: 1,
            antithetic: false,
            seed: 1,
            oas_bp: 0.0,
            target_ci_half_width: None,
        };
        let mut path = Vec::new();
        tree.sample_path_into(1, 0, false, &mut path)
            .expect("factor path");
        let replay = template
            .replay(&tree, &bond, &path, &config, None, None)
            .expect("product replay");
        assert_eq!(replay.snapshots[0].features[2], 100.0);
    }

    #[test]
    fn term_reset_base_forward_uses_discount_curve_date_axis() {
        let curve_base = time::macros::date!(2024 - 01 - 01);
        let as_of = time::macros::date!(2025 - 01 - 01);
        let maturity = time::macros::date!(2026 - 01 - 01);
        let discount = DiscountCurve::builder("USD-OIS")
            .base_date(curve_base)
            .day_count(DayCount::Act360)
            .knots([
                (0.0, 1.0),
                (1.0, (-0.02_f64).exp()),
                (2.0, (-0.08_f64).exp()),
                (3.0, (-0.17_f64).exp()),
            ])
            .interp(InterpStyle::LogLinear)
            .build()
            .expect("shifted-base test discount curve");
        let term_forward = ForwardCurve::builder("USD-SOFR-3M", 0.25)
            .base_date(as_of)
            .knots([(0.0, 0.04), (2.0, 0.04)])
            .interp(InterpStyle::Linear)
            .build()
            .expect("term forward curve");
        let market = MarketContext::new()
            .insert(discount.clone())
            .insert(term_forward);
        let tree = stochastic_test_tree(366);
        let bond = term_pik_test_bond(as_of, maturity);
        let template = ReplayTemplate::new(&tree, &bond, &market, as_of).expect("template");
        let coupon = template
            .floating
            .iter()
            .find(|coupon| coupon.reset_step > 0)
            .expect("future term coupon");
        let FloatingRateModel::Term(ObservedRateSource::Conditional(rate)) = &coupon.rate_model
        else {
            panic!("future reset should carry a conditional rate");
        };
        let FloatingRateObservation::Term { tenor_years, .. } = coupon.compiled.observation()
        else {
            panic!("expected term observation");
        };
        let observation_end_step = nearest_step(
            &template.times,
            template.times[coupon.reset_step] + tenor_years,
        );
        let reset_date = template.step_dates[coupon.reset_step].expect("reset grid date");
        let observation_end_date =
            template.step_dates[observation_end_step].expect("observation-end grid date");
        let expected_df = discount
            .df_between_dates(reset_date, observation_end_date)
            .expect("date-based forward discount factor");
        let expected_forward = (1.0 / expected_df - 1.0) / rate.accrual;
        assert!(
            (rate.base_discount_forward - expected_forward).abs() < 1.0e-14,
            "base forward must use discount-curve dates: actual={}, expected={expected_forward}",
            rate.base_discount_forward
        );

        let old_time_axis_df = discount.df(template.times[observation_end_step])
            / discount.df(template.times[coupon.reset_step]);
        let old_time_axis_forward = (1.0 / old_time_axis_df - 1.0) / rate.accrual;
        assert!(
            (rate.base_discount_forward - old_time_axis_forward).abs() > 0.02,
            "fixture must distinguish curve-relative time from the valuation grid"
        );
    }

    #[test]
    fn lagged_term_reset_uses_curve_tenor_and_captures_post_pik_start_balance() {
        let as_of = time::macros::date!(2025 - 01 - 01);
        let maturity = time::macros::date!(2026 - 01 - 01);
        let market = stochastic_test_market(as_of);
        // New Year's Day adjusts the final payment to 2026-01-02, so the
        // calibrated daily grid must extend one day past contractual maturity.
        let tree = stochastic_test_tree(366);
        let bullet = term_pik_test_bond(as_of, maturity);
        let template = ReplayTemplate::new(&tree, &bullet, &market, as_of).expect("template");
        let (coupon_id, coupon) = template
            .floating
            .iter()
            .enumerate()
            .find(|(_, coupon)| coupon.accrual_start_step > 0)
            .expect("future term coupon");
        assert!(coupon.reset_step < coupon.accrual_start_step);
        let FloatingRateObservation::Term { tenor_years, .. } = coupon.compiled.observation()
        else {
            panic!("expected term observation");
        };
        assert!((*tenor_years - 0.25).abs() < 1.0e-12);
        let times = tree.time_grid().expect("tree times");
        let observation_end = nearest_step(times, times[coupon.reset_step] + tenor_years);
        assert!(
            observation_end < coupon.payment_step,
            "3M curve tenor must not be replaced by the 6M coupon/payment interval"
        );
        let FloatingRateModel::Term(ObservedRateSource::Conditional(rate)) = &coupon.rate_model
        else {
            panic!("future reset should carry a conditional rate");
        };
        assert!((rate.accrual - (times[observation_end] - times[coupon.reset_step])).abs() < 1e-14);
        assert_eq!(
            rate.conditional_discount_factors,
            tree.conditional_discount_factors(
                coupon.reset_step,
                observation_end,
                *times.last().expect("terminal time")
            )
            .expect("conditional discount factors")
        );

        let schedule = bullet
            .full_cashflow_schedule(&market)
            .expect("deterministic schedule");
        let first_pik = schedule
            .get_flows()
            .iter()
            .filter(|flow| flow.kind == CFKind::Pik)
            .min_by_key(|flow| flow.date)
            .expect("first PIK coupon")
            .amount
            .amount();
        let expected_start_balance = 100.0 + first_pik;
        let mut sampled = Vec::new();
        tree.sample_path_into(23, 0, false, &mut sampled)
            .expect("factor path");
        let config = BondLsmcConfig {
            paths: 64,
            antithetic: true,
            seed: 23,
            oas_bp: 0.0,
            target_ci_half_width: None,
        };
        let mut cursor = ReplayCursor::new(&template).expect("cursor");
        for step in 0..=coupon.accrual_start_step {
            cursor
                .advance_step(&template, &bullet, &sampled, &config, step)
                .expect("forward replay");
        }
        let captured = coupon
            .compiled
            .captured_notional(&cursor.floating[coupon_id])
            .expect("second coupon notional");
        assert!(
            (captured - expected_start_balance).abs() < 1.0e-10,
            "the prior PIK coupon must capitalize before the next coupon captures notional: captured={captured}, expected={expected_start_balance}"
        );

        let mut callable = bullet.clone();
        callable.call_put = Some(CallPutSchedule {
            calls: vec![CallPut {
                start_date: time::macros::date!(2025 - 08 - 15),
                end_date: time::macros::date!(2025 - 08 - 15),
                price_pct_of_par: 80.0,
                make_whole: None,
            }],
            puts: Vec::new(),
        });
        let bullet_value = price_bond_lsmc(&tree, &bullet, &market, as_of, &config, None)
            .expect("term PIK bullet")
            .estimate
            .mean
            .amount();
        let call_value = price_bond_lsmc(&tree, &callable, &market, as_of, &config, None)
            .expect("callable term PIK bond")
            .estimate
            .mean
            .amount();
        assert!(
            call_value < bullet_value,
            "mid-period call must reduce stochastic term PIK value: call={call_value}, bullet={bullet_value}"
        );
    }

    #[test]
    fn overnight_pik_replays_daily_caps_and_mid_accrual_exercise_state() {
        let as_of = time::macros::date!(2025 - 01 - 01);
        let maturity = time::macros::date!(2025 - 07 - 01);
        let market = stochastic_test_market(as_of);
        let tree = stochastic_test_tree(181);
        let capped = overnight_pik_test_bond(as_of, maturity);
        let mut uncapped = capped.clone();
        let CashflowSpec::Floating(uncapped_spec) = &mut uncapped.cashflow_spec else {
            unreachable!("expected floating coupon");
        };
        uncapped_spec.rate_spec.index_cap_bp = None;
        let config = BondLsmcConfig {
            paths: 64,
            antithetic: true,
            seed: 0x0a11_ce55,
            oas_bp: 0.0,
            target_ci_half_width: None,
        };
        let capped_template =
            ReplayTemplate::new(&tree, &capped, &market, as_of).expect("capped template");
        let uncapped_template =
            ReplayTemplate::new(&tree, &uncapped, &market, as_of).expect("uncapped template");
        let mut sampled = Vec::new();
        tree.sample_path_into(41, 0, false, &mut sampled)
            .expect("factor path");
        let capped_path = capped_template
            .replay(&tree, &capped, &sampled, &config, None, None)
            .expect("capped overnight replay");
        let uncapped_path = uncapped_template
            .replay(&tree, &uncapped, &sampled, &config, None, None)
            .expect("uncapped overnight replay");
        assert!(
            capped_path
                .snapshots
                .last()
                .expect("terminal")
                .hold_redemption
                < uncapped_path
                    .snapshots
                    .last()
                    .expect("terminal")
                    .hold_redemption,
            "daily index cap must reduce PIK capitalization on the same overnight path"
        );

        let mut callable = capped.clone();
        let exercise = time::macros::date!(2025 - 02 - 14);
        callable.call_put = Some(CallPutSchedule {
            calls: vec![CallPut {
                start_date: exercise,
                end_date: exercise,
                price_pct_of_par: 90.0,
                make_whole: None,
            }],
            puts: Vec::new(),
        });
        let callable_template =
            ReplayTemplate::new(&tree, &callable, &market, as_of).expect("callable template");
        let callable_path = callable_template
            .replay(&tree, &callable, &sampled, &config, None, None)
            .expect("callable overnight replay");
        let exercise_step = exact_grid_step(tree.time_grid().expect("times"), as_of, exercise)
            .expect("exercise step");
        let exercise_snapshot = callable_path
            .snapshots
            .iter()
            .find(|snapshot| snapshot.step == exercise_step)
            .expect("mid-accrual snapshot");
        assert!(
            exercise_snapshot.features[5] > 0.0,
            "overnight PIK must carry partial daily accrual into the exercise state"
        );
        let capped_value = price_bond_lsmc(&tree, &capped, &market, as_of, &config, None)
            .expect("overnight PIK bullet")
            .estimate
            .mean
            .amount();
        let call_value = price_bond_lsmc(&tree, &callable, &market, as_of, &config, None)
            .expect("callable overnight PIK")
            .estimate
            .mean
            .amount();
        assert!(
            call_value < capped_value,
            "mid-accrual call must reduce stochastic overnight PIK value: call={call_value}, bullet={capped_value}"
        );
    }

    #[test]
    fn fixed_budget_rejects_unmet_final_confidence_target() {
        let as_of = time::macros::date!(2025 - 01 - 01);
        let maturity = time::macros::date!(2025 - 04 - 01);
        let market = stochastic_test_market(as_of);
        let tree = stochastic_test_tree(90);
        let bond = stochastic_test_bond(as_of, maturity);
        let config = BondLsmcConfig {
            paths: 32,
            antithetic: true,
            seed: 11,
            oas_bp: 0.0,
            target_ci_half_width: Some(f64::MIN_POSITIVE),
        };
        let error = price_bond_lsmc(&tree, &bond, &market, as_of, &config, None)
            .expect_err("a finite stochastic sample cannot satisfy a near-zero CI target");
        assert!(
            error.to_string().contains("exhausted its fixed budget"),
            "unexpected error: {error}"
        );
    }

    #[test]
    fn state_dependent_right_on_nonfirst_path_still_requires_a_policy() {
        let snapshot = |step, call| DecisionSnapshot {
            step,
            features: [0.0; FEATURE_COUNT],
            current_cash: 0.0,
            hold_redemption: 0.0,
            call,
            put: None,
            friction: 0.0,
            a_to_next: 1.0,
            b_to_next: 0.0,
        };
        // Path-major layout: path 0 has no right at either local decision;
        // path 1 has a state-dependent call only at local decision 0.
        let snapshots = vec![
            snapshot(1, None),
            snapshot(2, None),
            snapshot(1, Some(95.0)),
            snapshot(2, None),
        ];
        assert!(
            block_requires_policy(&snapshots, 2, 2, 0).expect("policy scan"),
            "a right on any training path must trigger regression"
        );
        assert!(!block_requires_policy(&snapshots, 2, 2, 1).expect("policy scan"));
    }

    #[test]
    fn stochastic_maturity_make_whole_uses_contractual_floor_without_training_target() {
        let as_of = time::macros::date!(2025 - 01 - 01);
        let maturity = time::macros::date!(2025 - 04 - 01);
        let market = stochastic_test_market(as_of);
        let tree = stochastic_test_tree(90);
        let mut bond = stochastic_test_bond(as_of, maturity);
        bond.call_put = Some(CallPutSchedule {
            calls: vec![CallPut {
                start_date: maturity,
                end_date: maturity,
                price_pct_of_par: 100.0,
                make_whole: Some(crate::instruments::fixed_income::bond::MakeWholeSpec {
                    reference_curve_id: CurveId::new("USD-OIS"),
                    spread_bp: 25.0,
                }),
            }],
            puts: Vec::new(),
        });
        let template = ReplayTemplate::new(&tree, &bond, &market, as_of).expect("template");
        assert!(
            template.make_whole_claims.is_empty(),
            "terminal make-whole has no later reference cashflows to regress"
        );
        let config = BondLsmcConfig {
            paths: 32,
            antithetic: true,
            seed: 91,
            oas_bp: 0.0,
            target_ci_half_width: None,
        };
        let result = price_bond_lsmc(&tree, &bond, &market, as_of, &config, None)
            .expect("maturity make-whole price");
        assert!(result.estimate.mean.amount().is_finite());
        assert_eq!(result.make_whole_training_paths, 0);
    }

    #[test]
    fn rolled_maturity_make_whole_retains_later_reference_cash() {
        let as_of = time::macros::date!(2025 - 01 - 01);
        // Saturday contractual maturity rolls the holder payment to Monday.
        let maturity = time::macros::date!(2025 - 03 - 01);
        let market = stochastic_test_market(as_of);
        let tree = stochastic_test_tree(61);
        let mut bond = stochastic_test_bond(as_of, maturity);
        bond.call_put = Some(CallPutSchedule {
            calls: vec![CallPut {
                start_date: maturity,
                end_date: maturity,
                price_pct_of_par: 100.0,
                make_whole: Some(crate::instruments::fixed_income::bond::MakeWholeSpec {
                    reference_curve_id: CurveId::new("USD-OIS"),
                    spread_bp: 25.0,
                }),
            }],
            puts: Vec::new(),
        });
        let template = ReplayTemplate::new(&tree, &bond, &market, as_of).expect("template");
        assert_eq!(template.make_whole_claims.len(), 1);
        let redemption_step = template.redemption_step.expect("rolled redemption step");
        assert!(template.make_whole_claims[0].exercise_step < redemption_step);

        let config = BondLsmcConfig {
            paths: 16,
            antithetic: true,
            seed: 92,
            oas_bp: 0.0,
            target_ci_half_width: None,
        };
        let result = price_bond_lsmc(&tree, &bond, &market, as_of, &config, None)
            .expect("rolled-maturity make-whole price");
        assert!(result.estimate.mean.amount().is_finite());
        assert_eq!(result.make_whole_training_paths, 16);
        assert_eq!(result.make_whole_training_simulated_paths, 32);
    }

    #[test]
    fn option_bearing_custom_cashflows_are_rejected_before_replay() {
        let as_of = time::macros::date!(2025 - 01 - 01);
        let maturity = time::macros::date!(2025 - 04 - 01);
        let market = stochastic_test_market(as_of);
        let tree = stochastic_test_tree(90);
        let mut bond = stochastic_test_bond(as_of, maturity);
        bond.call_put = Some(CallPutSchedule {
            calls: vec![CallPut {
                start_date: time::macros::date!(2025 - 02 - 15),
                end_date: time::macros::date!(2025 - 02 - 15),
                price_pct_of_par: 100.0,
                make_whole: None,
            }],
            puts: Vec::new(),
        });
        bond.custom_cashflows = Some(
            bond.full_cashflow_schedule(&market)
                .expect("canonical schedule"),
        );
        let config = BondLsmcConfig {
            paths: 8,
            antithetic: true,
            seed: 5,
            oas_bp: 0.0,
            target_ci_half_width: None,
        };
        let error = price_bond_lsmc(&tree, &bond, &market, as_of, &config, None)
            .expect_err("custom optional schedule must be rejected");
        assert!(
            error.to_string().contains("custom cashflows"),
            "unexpected error: {error}"
        );
    }

    #[test]
    fn two_time_blocks_match_full_replay_at_cash_amortization_exercise_boundary() {
        let as_of = time::macros::date!(2025 - 01 - 01);
        let maturity = time::macros::date!(2025 - 01 - 05);
        let bond = stochastic_test_bond(as_of, maturity);
        let tree = stochastic_test_tree(4);
        let mut template = synthetic_daily_template(4, false);
        template.step_dates = (0_i64..=4)
            .map(|offset| as_of.checked_add(Duration::days(offset)))
            .collect();
        template.decision_steps = vec![0, 2, 4];
        template.static_cash[2].push(CashEvent {
            amount_at_step: 15.0,
            event_minus_step: 0.0,
        });
        template.static_cash[4].push(CashEvent {
            amount_at_step: 4.0,
            event_minus_step: 0.0,
        });
        template.balance_events[2].push(BalanceEvent { delta: -10.0 });
        template.exercise[2].push(ExerciseDate {
            date: time::macros::date!(2025 - 01 - 03),
            calls: vec![ExerciseCall {
                price_pct_of_par: 90.0,
                make_whole: None,
            }],
            puts: Vec::new(),
            return_floor: false,
        });
        let config = BondLsmcConfig {
            paths: 1,
            antithetic: false,
            seed: 77,
            oas_bp: 125.0,
            target_ci_half_width: None,
        };
        let seed = 0x1234_5678;
        let mut full_path = Vec::new();
        tree.sample_path_into(seed, 0, false, &mut full_path)
            .expect("full factor path");
        let full = template
            .replay(&tree, &bond, &full_path, &config, None, None)
            .expect("full product replay");

        let initial_cursor = ReplayCursor::new(&template).expect("initial cursor");
        let checkpoint_zero = TrainingCheckpoint {
            factor: RatesCreditPathCheckpoint::from(&full_path[0]),
            product: initial_cursor.checkpoint(&template),
        };
        let mut boundary_cursor = ReplayCursor::new(&template).expect("boundary cursor");
        for step in 0..2 {
            boundary_cursor
                .advance_step(&template, &bond, &full_path, &config, step)
                .expect("prefix replay");
        }
        let checkpoint_two = TrainingCheckpoint {
            factor: RatesCreditPathCheckpoint::from(&full_path[2]),
            product: boundary_cursor.checkpoint(&template),
        };
        let mut segment = Vec::new();
        let low = template
            .replay_block(
                &tree,
                &bond,
                &config,
                seed,
                0,
                false,
                &checkpoint_zero,
                0,
                1,
                None,
                None,
                &mut segment,
            )
            .expect("low block");
        let high = template
            .replay_block(
                &tree,
                &bond,
                &config,
                seed,
                0,
                false,
                &checkpoint_two,
                1,
                2,
                None,
                None,
                &mut segment,
            )
            .expect("high block");
        let low_snapshot = &low.snapshots[0];
        let boundary_snapshot = &high.snapshots[0];
        let terminal = high.terminal.as_ref().expect("terminal snapshot");
        for (blocked, complete) in [
            (low_snapshot, &full.snapshots[0]),
            (boundary_snapshot, &full.snapshots[1]),
            (terminal, &full.snapshots[2]),
        ] {
            assert!((blocked.current_cash - complete.current_cash).abs() < 1.0e-14);
            assert!((blocked.hold_redemption - complete.hold_redemption).abs() < 1.0e-14);
            assert!((blocked.a_to_next - complete.a_to_next).abs() < 1.0e-14);
            assert!((blocked.b_to_next - complete.b_to_next).abs() < 1.0e-14);
            assert_eq!(blocked.call, complete.call);
        }
        assert_eq!(boundary_snapshot.current_cash, 15.0);
        assert_eq!(boundary_snapshot.features[2], 90.0);
        assert_eq!(boundary_snapshot.call, Some(81.0));

        let policy = fit_policy(
            &[full.snapshots[1].features, full.snapshots[1].features],
            &[100.0, 100.0],
        )
        .expect("boundary policy");
        let full_value = value_with_policy(&full, &[None, Some(policy.clone()), None])
            .expect("full replay value");
        let mut blocked_value = terminal.current_cash
            + terminal.exercise_value(terminal.hold_redemption, terminal.hold_redemption);
        let boundary_continuation =
            boundary_snapshot.a_to_next * blocked_value + boundary_snapshot.b_to_next;
        blocked_value = boundary_snapshot.current_cash
            + boundary_snapshot.exercise_value(
                policy
                    .predict(&boundary_snapshot.features)
                    .expect("policy prediction"),
                boundary_continuation,
            );
        let low_continuation = low_snapshot.a_to_next * blocked_value + low_snapshot.b_to_next;
        blocked_value = low_snapshot.current_cash
            + low_snapshot.exercise_value(low_continuation, low_continuation);
        assert!(
            (blocked_value - full_value).abs() < 1.0e-12,
            "blocked={blocked_value}, full={full_value}"
        );
    }

    #[test]
    fn return_floor_subtracts_partial_accrual_before_adding_it_once() {
        let floor = ReturnFloorTemplate {
            kind: ReturnFloorKind::Moic(2.0),
            issue_price: 100.0,
            issue_date: time::macros::date!(2025 - 01 - 01),
            day_count: DayCount::Act365F,
        };
        let clean = floor
            .redemption(time::macros::date!(2025 - 07 - 01), 100.0, 50.0, 0.0, 10.0)
            .expect("floor redemption");
        assert_eq!(clean, 140.0);
        assert_eq!(clean + 10.0, 150.0);
    }

    #[test]
    fn make_whole_reference_target_uses_rates_but_not_hazard_or_oas() {
        let basis = MakeWholeBasis {
            interval_adjustments: vec![1.0, 1.0],
        };
        let cash = [0.0, 10.0, 100.0];
        let low_hazard = [
            path_state(0, 0.02, 0.01, 0.9),
            path_state(1, 0.02, 0.01, 0.8),
            path_state(2, 0.02, 0.01, 1.0),
        ];
        let high_hazard = [
            path_state(0, 0.02, 0.50, 0.9),
            path_state(1, 0.02, 0.50, 0.8),
            path_state(2, 0.02, 0.50, 1.0),
        ];
        let lower_rates = [
            path_state(0, 0.01, 0.01, 0.95),
            path_state(1, 0.01, 0.01, 0.9),
            path_state(2, 0.01, 0.01, 1.0),
        ];
        let base = basis
            .realized_reference_value(0, &low_hazard, &cash)
            .expect("reference target");
        let hazard_changed = basis
            .realized_reference_value(0, &high_hazard, &cash)
            .expect("reference target");
        let rate_changed = basis
            .realized_reference_value(0, &lower_rates, &cash)
            .expect("reference target");
        assert!((base - 81.0).abs() < 1.0e-12);
        assert_eq!(base, hazard_changed);
        assert!(rate_changed > base);
    }

    #[test]
    fn same_day_terminal_claim_has_no_next_day_credit_or_rate_exposure() {
        let origin = time::macros::date!(2025 - 01 - 15);
        assert_eq!(
            exact_grid_step(&[0.0, 1.0 / 365.0], origin, origin).expect("same-day grid step"),
            0
        );
        let path = PathRecord {
            snapshots: vec![
                DecisionSnapshot {
                    step: 0,
                    features: [0.0; FEATURE_COUNT],
                    current_cash: 5.0,
                    hold_redemption: 100.0,
                    call: None,
                    put: None,
                    friction: 0.0,
                    a_to_next: 0.01,
                    b_to_next: 1_000.0,
                },
                DecisionSnapshot {
                    step: 1,
                    features: [0.0; FEATURE_COUNT],
                    current_cash: 0.0,
                    hold_redemption: 0.0,
                    call: None,
                    put: None,
                    friction: 0.0,
                    a_to_next: 0.0,
                    b_to_next: 0.0,
                },
            ],
        };
        let value = value_with_policy(&path, &[None, None]).expect("same-day terminal value");
        assert_eq!(value, 105.0);

        let terminal = &path.snapshots[0];
        let holder_put = DecisionSnapshot {
            put: Some(110.0),
            ..terminal.clone()
        };
        assert_eq!(holder_put.exercise_value(-999.0, 999.0), 110.0);
        let issuer_call = DecisionSnapshot {
            call: Some(90.0),
            ..terminal.clone()
        };
        assert_eq!(issuer_call.exercise_value(-999.0, 999.0), 90.0);
    }
}
