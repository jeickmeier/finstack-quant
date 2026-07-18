//! Deterministic finite-pool credit-loss simulation.
//!
//! Losses are positive amounts:
//! `loss = Σ 1{default_i} × notional_i × LGD_i`.
//! VaR uses the nearest-rank empirical loss quantile at `confidence`, with
//! index `ceil(confidence × num_paths) - 1`. Expected shortfall is the mean
//! of that VaR observation and every worse loss.

use std::collections::HashSet;

use super::{Copula, CopulaSpec, RecoveryModel, RecoverySpec};
use crate::Result;
use finstack_quant_core::math::{standard_normal_inv_cdf, student_t_inv_cdf, NeumaierAccumulator};
use finstack_quant_monte_carlo::rng::philox::PhiloxRng;
use finstack_quant_monte_carlo::traits::RandomStream;

/// Maximum number of paths accepted by the portfolio-loss simulation.
///
/// One million paths provides ample precision for interactive Python and
/// library workflows while bounding the loss distribution and its sorting
/// workspace to roughly 16 MiB of raw `f64` storage.
pub const MAX_PORTFOLIO_LOSS_PATHS: usize = 1_000_000;

const FACTOR_NORM_REL_TOLERANCE: f64 = 64.0 * f64::EPSILON;

/// One name in a finite credit portfolio.
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize, schemars::JsonSchema)]
pub struct CreditExposure {
    /// Stable exposure identifier.
    pub id: String,
    /// Non-negative exposure notional in one caller-defined unit.
    pub notional: f64,
    /// Unconditional default probability in `[0, 1]`.
    pub default_probability: f64,
    /// Loss given default in `[0, 1]`.
    pub lgd: f64,
    /// Systematic-factor loadings `β`; the squared norm must not exceed one.
    pub factor_loadings: Vec<f64>,
}

/// Portfolio credit-loss simulation settings.
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize, schemars::JsonSchema)]
pub struct PortfolioLossConfig {
    /// Number of simulated paths in `1..=MAX_PORTFOLIO_LOSS_PATHS`.
    pub num_paths: usize,
    /// Root seed for path-indexed Philox streams.
    pub seed: u64,
    /// Loss-positive VaR and expected-shortfall confidence in `(0, 1)`.
    pub confidence: f64,
    /// Gaussian or Student-t copula specification.
    pub copula: CopulaSpec,
}

/// Portfolio credit-loss distribution and tail statistics.
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize, schemars::JsonSchema)]
pub struct PortfolioLossResult {
    /// Loss for each path in ascending path-index order.
    pub losses: Vec<f64>,
    /// Arithmetic mean path loss.
    pub expected_loss: f64,
    /// Loss-positive nearest-rank VaR at the configured confidence.
    pub var: f64,
    /// Mean loss from the VaR observation through the worst path.
    pub expected_shortfall: f64,
    /// Loss-positive confidence used for [`Self::var`] and
    /// [`Self::expected_shortfall`], in `(0, 1)`.
    pub confidence: f64,
}

/// Loss statistics for one attachment/detachment tranche over a simulated pool
/// loss distribution.
///
/// All `*_fraction` members are expressed as a share of the tranche's own
/// notional, so a fully written-down tranche has fraction `1.0`. All `*_amount`
/// members are in the same scalar unit as the pool notional supplied to
/// [`PortfolioLossResult::tranche_loss_statistics`].
///
/// # References
///
/// - O'Kane, D. (2008). *Modelling Single-name and Multi-name Credit
///   Derivatives*. Wiley Finance. Chapter 15 ("Modelling Tranches"), which
///   defines the tranche loss function
///   `min(max(L - A, 0), D - A) / (D - A)` used here.
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize, schemars::JsonSchema)]
pub struct TrancheLossStatistics {
    /// Tranche attachment point as a fraction of pool notional, in `[0, 1)`.
    pub attachment: f64,
    /// Tranche detachment point as a fraction of pool notional, in `(0, 1]`.
    pub detachment: f64,
    /// Tranche notional `(detachment - attachment) * pool_notional`.
    pub tranche_notional: f64,
    /// Mean tranche loss as a fraction of tranche notional, in `[0, 1]`.
    pub expected_loss_fraction: f64,
    /// Mean tranche loss in pool-notional units.
    pub expected_loss_amount: f64,
    /// Nearest-rank tranche loss fraction at the distribution's confidence.
    pub var_fraction: f64,
    /// Nearest-rank tranche loss amount at the distribution's confidence.
    pub var_amount: f64,
    /// Mean tranche loss fraction from the VaR observation through the worst path.
    pub expected_shortfall_fraction: f64,
    /// Mean tranche loss amount from the VaR observation through the worst path.
    pub expected_shortfall_amount: f64,
    /// Share of paths whose pool loss fraction strictly exceeds `attachment`.
    pub prob_attachment_breached: f64,
    /// Share of paths whose pool loss fraction reaches or exceeds `detachment`.
    pub prob_full_writedown: f64,
}

impl PortfolioLossResult {
    /// Aggregate a finite loss distribution under loss-positive conventions.
    ///
    /// # Errors
    ///
    /// Returns an error for an empty distribution, a non-finite or negative
    /// loss, or confidence outside `(0, 1)`.
    pub fn from_losses(losses: Vec<f64>, confidence: f64) -> Result<Self> {
        validate_confidence(confidence)?;
        if losses.is_empty() {
            return Err(validation_error(
                "portfolio loss distribution must not be empty",
            ));
        }
        if let Some((index, loss)) = losses
            .iter()
            .copied()
            .enumerate()
            .find(|(_, loss)| !loss.is_finite() || *loss < 0.0)
        {
            return Err(validation_error(format!(
                "portfolio loss at index {index} must be finite and non-negative, got {loss}"
            )));
        }

        let expected_loss = scaled_mean(&losses, "expected loss")?;
        let mut sorted = Vec::new();
        sorted.try_reserve_exact(losses.len()).map_err(|error| {
            allocation_error(format!(
                "could not reserve {} losses for tail-statistic sorting: {error}",
                losses.len()
            ))
        })?;
        sorted.extend_from_slice(&losses);
        sorted.sort_unstable_by(f64::total_cmp);
        let var_index = ((confidence * sorted.len() as f64).ceil() as usize)
            .saturating_sub(1)
            .min(sorted.len() - 1);
        let var = sorted[var_index];
        let tail = &sorted[var_index..];
        let expected_shortfall = scaled_mean(tail, "expected shortfall")?;
        if !var.is_finite() || !expected_loss.is_finite() || !expected_shortfall.is_finite() {
            return Err(validation_error("portfolio loss statistics must be finite"));
        }

        Ok(Self {
            losses,
            expected_loss,
            var,
            expected_shortfall,
            confidence,
        })
    }

    /// Tranche loss statistics for an attachment/detachment pair over this
    /// simulated pool loss distribution.
    ///
    /// Each path's pool loss is converted to a pool loss fraction
    /// `L = loss / pool_notional` and mapped through the standard tranche loss
    /// function
    ///
    /// ```text
    /// tranche_loss_fraction(L) = clamp(L - A, 0, D - A) / (D - A)
    /// ```
    ///
    /// so the tranche absorbs nothing until the pool loss breaches `A` and is
    /// fully written down once it reaches `D`. Expected loss, VaR, and expected
    /// shortfall are then aggregated from that per-path fraction distribution
    /// using exactly the same loss-positive nearest-rank conventions as
    /// [`PortfolioLossResult::from_losses`], evaluated at this result's own
    /// [`Self::confidence`].
    ///
    /// # Units
    ///
    /// `attachment` and `detachment` are **fractions of pool notional in
    /// `[0, 1]`** — a 0–3% equity tranche is `(0.0, 0.03)`, not `(0.0, 3.0)`.
    /// This deliberately differs from
    /// [`crate::instruments::fixed_income::structured_credit::Tranche::loss_allocation`],
    /// which is `Money`-based and carries its attachment points in percent; that
    /// documented unit inconsistency is not reused here.
    ///
    /// # Errors
    ///
    /// Returns `Error::Validation` when `attachment` or `detachment` is outside
    /// `[0, 1]`, when `attachment >= detachment`, when `pool_notional` is not
    /// finite and strictly positive, or when a derived tranche statistic is not
    /// finite.
    ///
    /// # Arguments
    ///
    /// * `attachment` - Lower tranche boundary as a fraction of pool notional;
    ///   losses below this point are absorbed by more junior tranches.
    /// * `detachment` - Upper tranche boundary as a fraction of pool notional;
    ///   losses above this point are absorbed by more senior tranches.
    /// * `pool_notional` - Total pool notional in the same scalar unit as the
    ///   simulated losses; used to convert path losses to loss fractions.
    ///
    /// # Returns
    ///
    /// [`TrancheLossStatistics`] with fraction-of-tranche-notional and absolute
    /// expected loss, VaR, expected shortfall, and the attachment-breach and
    /// full-write-down probabilities.
    ///
    /// # References
    ///
    /// - O'Kane, D. (2008). *Modelling Single-name and Multi-name Credit
    ///   Derivatives*. Wiley Finance, Chapter 15.
    /// - Gibson, M. S. (2004). "Understanding the Risk of Synthetic CDOs."
    ///   *Finance and Economics Discussion Series* 2004-36, Federal Reserve Board.
    pub fn tranche_loss_statistics(
        &self,
        attachment: f64,
        detachment: f64,
        pool_notional: f64,
    ) -> Result<TrancheLossStatistics> {
        validate_unit_interval("tranche attachment", attachment)?;
        validate_unit_interval("tranche detachment", detachment)?;
        if attachment >= detachment {
            return Err(validation_error(format!(
                "tranche attachment must be strictly below detachment, got {attachment} >= {detachment}"
            )));
        }
        if !pool_notional.is_finite() || pool_notional <= 0.0 {
            return Err(validation_error(format!(
                "tranche pool_notional must be finite and strictly positive, got {pool_notional}"
            )));
        }

        let width = detachment - attachment;
        let tranche_notional = width * pool_notional;
        let mut fractions = Vec::new();
        fractions
            .try_reserve_exact(self.losses.len())
            .map_err(|error| {
                allocation_error(format!(
                    "could not reserve {} tranche loss fractions: {error}",
                    self.losses.len()
                ))
            })?;
        let mut breached = 0_usize;
        let mut written_down = 0_usize;
        for loss in &self.losses {
            let pool_fraction = loss / pool_notional;
            if pool_fraction > attachment {
                breached += 1;
            }
            if pool_fraction >= detachment {
                written_down += 1;
            }
            fractions.push((pool_fraction - attachment).clamp(0.0, width) / width);
        }

        let path_count = self.losses.len() as f64;
        let aggregated = Self::from_losses(fractions, self.confidence)?;
        Ok(TrancheLossStatistics {
            attachment,
            detachment,
            tranche_notional,
            expected_loss_fraction: aggregated.expected_loss,
            expected_loss_amount: aggregated.expected_loss * tranche_notional,
            var_fraction: aggregated.var,
            var_amount: aggregated.var * tranche_notional,
            expected_shortfall_fraction: aggregated.expected_shortfall,
            expected_shortfall_amount: aggregated.expected_shortfall * tranche_notional,
            prob_attachment_breached: breached as f64 / path_count,
            prob_full_writedown: written_down as f64 / path_count,
        })
    }
}

/// Simulate portfolio losses using deterministic path-indexed Philox streams.
///
/// Native builds evaluate paths in parallel; path `i` always uses Philox
/// substream `i`, so output is bit-identical to [`simulate_portfolio_loss_serial`].
/// `wasm32` builds use the serial implementation.
///
/// Each path draws the configured Gaussian or Student-t copula, compares latent
/// variables with the exposures' unconditional default-probability thresholds,
/// and sums `notional * lgd` for defaults. Losses, expected loss, VaR, and
/// expected shortfall are therefore in the same scalar currency/unit as the
/// supplied notionals; callers must not mix currencies in one invocation.
/// `config.seed` and path indexing make the result reproducible across native
/// serial and parallel execution for the same inputs.
///
/// # Errors
///
/// Returns an error if the simulation configuration is invalid (including a
/// zero or excessive path count, invalid confidence, or unsupported copula),
/// any exposure has invalid/heterogeneous factor loadings, probability, LGD,
/// notional, or identifier data, copula thresholds cannot be calculated,
/// allocation fails, or a path loss overflows. No partial loss distribution is
/// returned.
///
/// # Arguments
///
/// * `exposures` - Same-currency credit exposures with default probabilities,
///   LGDs, notionals, and factor loadings used for every simulated path.
/// * `config` - Copula, confidence, path count, random seed, and simulation
///   policy; its seed produces deterministic path-indexed streams.
pub fn simulate_portfolio_loss(
    exposures: &[CreditExposure],
    config: &PortfolioLossConfig,
) -> Result<PortfolioLossResult> {
    simulate(exposures, config, None, Execution::Parallel)
}

/// Simulate portfolio losses serially with the canonical path-indexed streams.
///
/// This uses the same model, seed, path-to-Philox-substream assignment, and
/// loss conventions as [`simulate_portfolio_loss`], but evaluates paths in
/// index order on one thread. Use it for debugging or environments where
/// parallel execution is unavailable; given identical inputs it produces the
/// same loss distribution and tail statistics as the native parallel variant.
///
/// # Errors
///
/// Returns the same validation, copula, allocation, and finite-loss errors as
/// [`simulate_portfolio_loss`]. No partial result is returned.
///
/// # Arguments
///
/// * `exposures` - Same-currency credit exposures with default probabilities,
///   LGDs, notionals, and factor loadings used for every simulated path.
/// * `config` - Copula, confidence, path count, random seed, and simulation
///   policy; its seed produces deterministic path-indexed streams.
pub fn simulate_portfolio_loss_serial(
    exposures: &[CreditExposure],
    config: &PortfolioLossConfig,
) -> Result<PortfolioLossResult> {
    simulate(exposures, config, None, Execution::Serial)
}

/// Simulate losses using one shared recovery model instead of exposure LGDs.
///
/// The recovery model's conditional LGD is evaluated on the first systematic
/// factor, matching the one-factor recovery/correlation convention. This
/// variant therefore requires exactly one factor loading per exposure.
///
/// For defaulted names, `recovery.conditional_lgd(factor)` replaces each
/// exposure's stored `lgd`; notional, default probability, and all other
/// exposure validation still apply. Outputs remain in the scalar units of the
/// exposure notionals and use the parallel path-indexed Philox scheme.
///
/// # Errors
///
/// Returns an error if the recovery specification cannot build, the standard
/// simulation configuration or exposures are invalid, any exposure does not
/// have exactly one systematic factor loading, copula/threshold construction
/// fails, allocation fails, or a simulated path loss becomes non-finite.
///
/// # Arguments
///
/// * `exposures` - Same-currency one-factor credit exposures; stored LGDs are
///   replaced by conditional LGD from `recovery` for defaulted names.
/// * `config` - Copula, confidence, path count, random seed, and simulation
///   policy for deterministic parallel path simulation.
/// * `recovery` - Conditional recovery/LGD specification evaluated against the
///   first systematic factor on each simulated path.
pub fn simulate_portfolio_loss_with_recovery(
    exposures: &[CreditExposure],
    config: &PortfolioLossConfig,
    recovery: &RecoverySpec,
) -> Result<PortfolioLossResult> {
    let recovery = recovery.build();
    simulate(
        exposures,
        config,
        Some(recovery.as_ref()),
        Execution::Parallel,
    )
}

#[derive(Clone, Copy)]
enum Execution {
    Serial,
    Parallel,
}

fn simulate(
    exposures: &[CreditExposure],
    config: &PortfolioLossConfig,
    recovery: Option<&dyn RecoveryModel>,
    execution: Execution,
) -> Result<PortfolioLossResult> {
    let validated = ValidatedSimulation::new(exposures, config, recovery.is_some())?;
    let simulate_path =
        |path_index: usize| validated.path_loss(exposures, config.seed, path_index, recovery);

    let mut losses = allocate_loss_buffer(config.num_paths)?;
    match execution {
        Execution::Serial => {
            for (path_index, loss) in losses.iter_mut().enumerate() {
                *loss = simulate_path(path_index)?;
            }
        }
        Execution::Parallel => fill_parallel_losses(&mut losses, simulate_path)?,
    }
    PortfolioLossResult::from_losses(losses, config.confidence)
}

struct ValidatedSimulation {
    copula: Box<dyn Copula>,
    thresholds: Vec<f64>,
    loading_norms: Vec<f64>,
    factor_count: usize,
    is_student_t: bool,
}

impl ValidatedSimulation {
    fn new(
        exposures: &[CreditExposure],
        config: &PortfolioLossConfig,
        uses_recovery_model: bool,
    ) -> Result<Self> {
        if config.num_paths == 0 {
            return Err(validation_error(
                "portfolio loss num_paths must be positive",
            ));
        }
        if config.num_paths > MAX_PORTFOLIO_LOSS_PATHS {
            return Err(validation_error(format!(
                "portfolio loss num_paths must not exceed {MAX_PORTFOLIO_LOSS_PATHS}, got {}",
                config.num_paths
            )));
        }
        validate_confidence(config.confidence)?;
        let loading_norms = validate_exposures(exposures)?;

        let factor_count = exposures
            .first()
            .map_or(0, |exposure| exposure.factor_loadings.len());
        if uses_recovery_model && factor_count != 1 && !exposures.is_empty() {
            return Err(validation_error(
                "recovery-model portfolio loss simulation requires exactly one systematic factor",
            ));
        }

        let (is_student_t, student_t_df) = match &config.copula {
            CopulaSpec::Gaussian => (false, None),
            CopulaSpec::StudentT { degrees_of_freedom } => (true, Some(*degrees_of_freedom)),
            _ => {
                return Err(validation_error(
                    "portfolio loss simulation supports Gaussian and Student-t copulas only",
                ));
            }
        };
        let copula = config.copula.build()?;
        let mut thresholds = Vec::new();
        thresholds
            .try_reserve_exact(exposures.len())
            .map_err(|error| {
                allocation_error(format!(
                    "could not reserve {} portfolio default thresholds: {error}",
                    exposures.len()
                ))
            })?;
        for exposure in exposures {
            let threshold = match student_t_df {
                Some(df) => student_t_inv_cdf(exposure.default_probability, df),
                None => Ok(standard_normal_inv_cdf(exposure.default_probability)),
            }?;
            thresholds.push(threshold);
        }

        Ok(Self {
            copula,
            thresholds,
            loading_norms,
            factor_count,
            is_student_t,
        })
    }

    fn path_loss(
        &self,
        exposures: &[CreditExposure],
        seed: u64,
        path_index: usize,
        recovery: Option<&dyn RecoveryModel>,
    ) -> Result<f64> {
        let mut rng = PhiloxRng::with_stream(seed, path_index as u64);
        let mixing = if self.is_student_t {
            self.copula.sample_mixing(rng.next_u01())
        } else {
            1.0
        };
        let mut factors = Vec::new();
        factors
            .try_reserve_exact(self.factor_count)
            .map_err(|error| {
                allocation_error(format!(
                    "could not reserve {} systematic factors: {error}",
                    self.factor_count
                ))
            })?;
        factors.resize(self.factor_count, 0.0);
        rng.fill_std_normals(&mut factors);

        let mut total_loss = 0.0;
        for ((exposure, threshold), rho) in exposures
            .iter()
            .zip(self.thresholds.iter())
            .zip(self.loading_norms.iter().copied())
        {
            let systematic = if rho > 0.0 {
                let mut dot = NeumaierAccumulator::new();
                for (loading, factor) in exposure.factor_loadings.iter().zip(factors.iter()) {
                    dot.add(loading * factor);
                }
                dot.total() / rho.sqrt()
            } else {
                0.0
            };
            let latent =
                self.copula
                    .latent_variable(systematic, rng.next_std_normal(), mixing, rho);
            if latent <= *threshold {
                let lgd = recovery
                    .map(|model| model.conditional_lgd(factors[0]))
                    .unwrap_or(exposure.lgd);
                let contribution = exposure.notional * lgd;
                let next_loss = total_loss + contribution;
                if !contribution.is_finite() || !next_loss.is_finite() {
                    return Err(validation_error(
                        "portfolio path loss overflowed finite f64 range",
                    ));
                }
                total_loss = next_loss;
            }
        }
        Ok(total_loss)
    }
}

fn validate_exposures(exposures: &[CreditExposure]) -> Result<Vec<f64>> {
    let factor_count = exposures
        .first()
        .map_or(0, |exposure| exposure.factor_loadings.len());
    let mut loading_norms = Vec::new();
    loading_norms
        .try_reserve_exact(exposures.len())
        .map_err(|error| {
            allocation_error(format!(
                "could not reserve {} factor-loading norms: {error}",
                exposures.len()
            ))
        })?;
    let mut seen_ids = HashSet::new();
    seen_ids.try_reserve(exposures.len()).map_err(|error| {
        allocation_error(format!(
            "could not reserve {} credit exposure identifiers: {error}",
            exposures.len()
        ))
    })?;
    for exposure in exposures {
        if exposure.id.trim().is_empty() {
            return Err(validation_error("credit exposure id must not be empty"));
        }
        let trimmed_id = exposure.id.trim();
        if !seen_ids.insert(trimmed_id) {
            return Err(validation_error(format!(
                "duplicate credit exposure id after trimming: '{trimmed_id}'"
            )));
        }
        validate_non_negative_finite("notional", exposure.notional)?;
        validate_unit_interval("default_probability", exposure.default_probability)?;
        validate_unit_interval("lgd", exposure.lgd)?;
        if exposure.factor_loadings.is_empty() {
            return Err(validation_error(format!(
                "credit exposure '{}' must have at least one factor loading",
                exposure.id
            )));
        }
        if exposure.factor_loadings.len() != factor_count {
            return Err(validation_error(format!(
                "credit exposure '{}' has {} factor loadings; expected {factor_count}",
                exposure.id,
                exposure.factor_loadings.len()
            )));
        }
        if exposure
            .factor_loadings
            .iter()
            .any(|loading| !loading.is_finite())
        {
            return Err(validation_error(format!(
                "credit exposure '{}' factor loadings must be finite",
                exposure.id
            )));
        }
        let mut squared_norm_sum = NeumaierAccumulator::new();
        for loading in &exposure.factor_loadings {
            squared_norm_sum.add(loading * loading);
        }
        let squared_norm = squared_norm_sum.total();
        let tolerance = FACTOR_NORM_REL_TOLERANCE * squared_norm.abs().max(1.0);
        if !squared_norm.is_finite() || squared_norm > 1.0 + tolerance {
            return Err(validation_error(format!(
                "credit exposure '{}' squared factor-loading norm must be at most 1, got {squared_norm}",
                exposure.id
            )));
        }
        loading_norms.push(squared_norm.clamp(0.0, 1.0));
    }
    Ok(loading_norms)
}

fn validate_confidence(confidence: f64) -> Result<()> {
    if confidence.is_finite() && confidence > 0.0 && confidence < 1.0 {
        Ok(())
    } else {
        Err(validation_error(format!(
            "portfolio loss confidence must be finite and strictly between 0 and 1, got {confidence}"
        )))
    }
}

fn validate_non_negative_finite(field: &str, value: f64) -> Result<()> {
    if value.is_finite() && value >= 0.0 {
        Ok(())
    } else {
        Err(validation_error(format!(
            "{field} must be finite and non-negative, got {value}"
        )))
    }
}

fn validate_unit_interval(field: &str, value: f64) -> Result<()> {
    if value.is_finite() && (0.0..=1.0).contains(&value) {
        Ok(())
    } else {
        Err(validation_error(format!(
            "{field} must be finite and in [0, 1], got {value}"
        )))
    }
}

fn validation_error(message: impl Into<String>) -> crate::Error {
    finstack_quant_core::Error::Validation(message.into()).into()
}

fn allocation_error(message: impl Into<String>) -> crate::Error {
    finstack_quant_core::Error::Internal(message.into()).into()
}

fn scaled_mean(values: &[f64], statistic: &str) -> Result<f64> {
    let scale = values.iter().copied().fold(0.0, f64::max);
    if scale == 0.0 {
        return Ok(0.0);
    }

    let mut scaled_total = NeumaierAccumulator::new();
    for value in values {
        scaled_total.add(*value / scale);
    }
    let mean = scale * (scaled_total.total() / values.len() as f64);
    if mean.is_finite() {
        Ok(mean)
    } else {
        Err(validation_error(format!(
            "portfolio loss {statistic} must be finite"
        )))
    }
}

fn allocate_loss_buffer(num_paths: usize) -> Result<Vec<f64>> {
    let mut losses = Vec::new();
    losses.try_reserve_exact(num_paths).map_err(|error| {
        allocation_error(format!(
            "could not reserve {num_paths} portfolio loss paths: {error}"
        ))
    })?;
    losses.resize(num_paths, 0.0);
    Ok(losses)
}

#[cfg(not(target_arch = "wasm32"))]
fn fill_parallel_losses<F>(losses: &mut [f64], simulate_path: F) -> Result<()>
where
    F: Fn(usize) -> Result<f64> + Sync + Send,
{
    use rayon::prelude::*;
    losses
        .par_iter_mut()
        .enumerate()
        .try_for_each(|(path_index, loss)| {
            *loss = simulate_path(path_index)?;
            Ok(())
        })
}

#[cfg(target_arch = "wasm32")]
fn fill_parallel_losses<F>(losses: &mut [f64], simulate_path: F) -> Result<()>
where
    F: Fn(usize) -> Result<f64>,
{
    for (path_index, loss) in losses.iter_mut().enumerate() {
        *loss = simulate_path(path_index)?;
    }
    Ok(())
}
