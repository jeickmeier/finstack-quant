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

/// Fit an exercise policy and value it on an independent factor-path stream.
///
/// The `tree` must already be calibrated.
pub(crate) fn price_bond_lsmc(
    tree: &RatesCreditTree,
    bond: &Bond,
    market: &MarketContext,
    as_of: Date,
    config: &BondLsmcConfig,
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
            train_seed,
            training_estimators,
            training_simulated_paths,
            &make_whole_policies,
        )?
    } else {
        vec![None; template.decision_steps.len()]
    };

    // Pricing estimators are independent: each draws its own factor path
    // stream. They run in parallel and are folded into the statistics in
    // path order, so the estimate is bit-identical to a serial run.
    let make_whole_policies = make_whole_policies.as_slice();
    let policies = policies.as_slice();
    let template = &template;
    let price_estimator = |(sampled, buffers): &mut (Vec<RatesCreditPathState>, ReplayBuffers),
                           path_index: usize|
     -> Result<f64> {
        tree.sample_path_into(pricing_seed, path_index as u64, false, sampled)?;
        let primary = value_with_policy(
            &template.replay(bond, sampled, config, Some(make_whole_policies), buffers)?,
            policies,
        )?;
        if !config.antithetic {
            return Ok(primary);
        }
        tree.sample_path_into(pricing_seed, path_index as u64, true, sampled)?;
        let antithetic = value_with_policy(
            &template.replay(bond, sampled, config, Some(make_whole_policies), buffers)?,
            policies,
        )?;
        Ok(0.5 * (primary + antithetic))
    };
    let init_buffers = || (Vec::new(), ReplayBuffers::default());
    #[cfg(not(target_arch = "wasm32"))]
    let estimators: Vec<Result<f64>> = {
        use rayon::prelude::*;
        (0..pricing_estimators)
            .into_par_iter()
            .map_init(init_buffers, price_estimator)
            .collect()
    };
    #[cfg(target_arch = "wasm32")]
    let estimators: Vec<Result<f64>> = {
        let mut buffers = init_buffers();
        (0..pricing_estimators)
            .map(|path_index| price_estimator(&mut buffers, path_index))
            .collect()
    };
    let mut stats = OnlineStats::new();
    for estimator in estimators {
        stats.update(estimator?);
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

/// Decisions per training block: `⌈√D⌉` for `D` decision transitions.
///
/// Training stores one factor/product checkpoint per path at each block
/// boundary and replays paths within a block, so `√D` balances checkpoint
/// storage (`D / block` boundaries) against replay length per block.
fn training_block_len(decision_count: usize) -> usize {
    ((decision_count.max(1) as f64).sqrt().ceil() as usize).max(1)
}

fn training_boundaries(template: &ReplayTemplate) -> Result<Vec<usize>> {
    let transitions = template
        .decision_steps
        .len()
        .checked_sub(1)
        .ok_or_else(|| {
            Error::internal("bond hazard LSMC requires at least one decision transition")
        })?;
    let block_len = training_block_len(transitions);
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
