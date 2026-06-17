//! Lightweight strategy-level allocation schemes.

use crate::error::{Error, Result};
use finstack_quant_core::math::summation::NeumaierAccumulator;
use serde::{Deserialize, Serialize};

const WEIGHT_TOLERANCE: f64 = 1e-9;
const SOLVER_TOLERANCE: f64 = 1e-10;
const MAX_SOLVER_ITERATIONS: usize = 10_000;

/// Strategy allocation scheme.
#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum AllocationScheme {
    /// Equal weights across all strategies.
    Equal,
    /// Caller-supplied fixed weights.
    Fixed,
    /// Inverse sample-volatility weights from strategy returns.
    InverseVolatility,
    /// Long-only target risk contribution weights from covariance and budgets.
    RiskBudget,
}

/// JSON specification for strategy-level allocation.
#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct WeightAllocationSpec {
    /// Allocation scheme to apply.
    pub scheme: AllocationScheme,
    /// Total capital to allocate across strategies.
    pub total_capital: f64,
    /// Strategy input rows.
    pub strategies: Vec<StrategyAllocationInput>,
    /// Optional covariance matrix for `risk_budget`, row-major as nested lists.
    pub covariance: Option<Vec<Vec<f64>>>,
    /// Optional target volatility. Unsupported in v1 because weights are fully invested.
    pub target_volatility: Option<f64>,
    /// Number of decimal places for capital rounding.
    #[serde(default = "default_money_decimal_places")]
    pub money_decimal_places: u32,
}

/// Per-strategy allocation input row.
#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct StrategyAllocationInput {
    /// Stable strategy identifier.
    pub id: String,
    /// Fixed weight for `fixed` scheme.
    pub fixed_weight: Option<f64>,
    /// Historical returns for `inverse_volatility`.
    #[serde(default)]
    pub returns: Vec<f64>,
    /// Target risk budget fraction for `risk_budget`.
    pub risk_budget: Option<f64>,
}

/// Strategy allocation result.
#[derive(Clone, Debug, Serialize)]
pub struct WeightAllocationResult {
    /// Allocation scheme applied.
    pub scheme: AllocationScheme,
    /// Per-strategy allocation rows.
    pub allocations: Vec<StrategyAllocation>,
    /// Portfolio-level diagnostics.
    pub diagnostics: AllocationDiagnostics,
}

/// Per-strategy allocation output row.
#[derive(Clone, Debug, Serialize)]
pub struct StrategyAllocation {
    /// Strategy identifier.
    pub id: String,
    /// Fully invested allocation weight.
    pub weight: f64,
    /// Rounded capital allocation.
    pub capital: f64,
    /// Sample volatility used by inverse-volatility allocation, when applicable.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub volatility: Option<f64>,
    /// Risk contribution fraction, when covariance diagnostics are available.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub risk_contribution: Option<f64>,
}

/// Portfolio-level allocation diagnostics.
#[derive(Clone, Debug, Serialize)]
pub struct AllocationDiagnostics {
    /// Sum of allocation weights.
    pub weights_sum: f64,
    /// Gross leverage, equal to sum of absolute weights.
    pub leverage: f64,
}

/// Allocate strategy weights from a JSON specification.
///
/// # Errors
///
/// Returns [`Error::ValidationFailed`] when inputs violate scheme invariants.
pub fn allocate_weights(spec_json: &str) -> Result<String> {
    let spec = parse_allocation_spec(spec_json)?;
    let result = spec.execute()?;
    serde_json::to_string(&result)
        .map_err(|err| Error::InvalidInput(format!("failed to serialize allocation result: {err}")))
}

/// Validate a strategy allocation JSON specification.
///
/// # Errors
///
/// Returns [`Error::ValidationFailed`] when inputs violate scheme invariants.
pub fn validate_allocation_json(spec_json: &str) -> Result<()> {
    let spec = parse_allocation_spec(spec_json)?;
    spec.execute().map(|_| ())
}

fn default_money_decimal_places() -> u32 {
    10
}

fn parse_allocation_spec(spec_json: &str) -> Result<WeightAllocationSpec> {
    serde_json::from_str(spec_json)
        .map_err(|err| Error::validation(format!("invalid allocation JSON: {err}")))
}

impl WeightAllocationSpec {
    fn execute(&self) -> Result<WeightAllocationResult> {
        self.validate_common()?;
        let covariance = if self.scheme == AllocationScheme::RiskBudget {
            Some(self.covariance_matrix()?)
        } else {
            None
        };
        let mut volatilities = vec![None; self.strategies.len()];
        let weights = match self.scheme {
            AllocationScheme::Equal => equal_weights(self.strategies.len()),
            AllocationScheme::Fixed => fixed_weights(&self.strategies)?,
            AllocationScheme::InverseVolatility => {
                let (weights, vols) = inverse_volatility_weights(&self.strategies)?;
                volatilities = vols;
                weights
            }
            AllocationScheme::RiskBudget => {
                let covariance = covariance.as_ref().ok_or_else(|| {
                    Error::validation("allocation covariance is required for risk_budget scheme")
                })?;
                risk_budget_weights(&self.strategies, covariance)?
            }
        };

        let risk_contributions = if let Some(covariance) = covariance.as_ref() {
            Some(risk_contribution_fractions(&weights, covariance)?)
        } else {
            None
        };
        let capitals = rounded_capitals(self.total_capital, &weights, self.money_decimal_places)?;

        let allocations = self
            .strategies
            .iter()
            .enumerate()
            .map(|(idx, strategy)| StrategyAllocation {
                id: strategy.id.clone(),
                weight: weights[idx],
                capital: capitals[idx],
                volatility: volatilities[idx],
                risk_contribution: risk_contributions
                    .as_ref()
                    .map(|contributions| contributions[idx]),
            })
            .collect();

        Ok(WeightAllocationResult {
            scheme: self.scheme,
            allocations,
            diagnostics: AllocationDiagnostics {
                weights_sum: neumaier_total(weights.iter().copied()),
                leverage: neumaier_total(weights.iter().map(|weight| weight.abs())),
            },
        })
    }

    fn validate_common(&self) -> Result<()> {
        if !self.total_capital.is_finite() {
            return Err(Error::validation("allocation total_capital must be finite"));
        }
        if self.strategies.is_empty() {
            return Err(Error::validation(
                "allocation requires at least one strategy",
            ));
        }
        if self.money_decimal_places > 12 {
            return Err(Error::validation(
                "allocation money_decimal_places must be <= 12",
            ));
        }
        if let Some(target_volatility) = self.target_volatility {
            if !target_volatility.is_finite() || target_volatility <= 0.0 {
                return Err(Error::validation(
                    "allocation target_volatility must be positive and finite",
                ));
            }
            return Err(Error::validation(
                "allocation target_volatility leverage scaling is unsupported in v1",
            ));
        }

        let mut ids = std::collections::BTreeSet::new();
        for strategy in &self.strategies {
            if strategy.id.trim().is_empty() {
                return Err(Error::validation(
                    "allocation strategy id must not be empty",
                ));
            }
            if !ids.insert(strategy.id.clone()) {
                return Err(Error::validation(format!(
                    "allocation strategy id '{}' is duplicated",
                    strategy.id
                )));
            }
        }
        Ok(())
    }

    fn covariance_matrix(&self) -> Result<Vec<f64>> {
        let covariance = self.covariance.as_ref().ok_or_else(|| {
            Error::validation("allocation covariance is required for risk_budget scheme")
        })?;
        flatten_and_validate_covariance(covariance, self.strategies.len())
    }
}

fn equal_weights(len: usize) -> Vec<f64> {
    vec![1.0 / len as f64; len]
}

fn fixed_weights(strategies: &[StrategyAllocationInput]) -> Result<Vec<f64>> {
    let mut weights = Vec::with_capacity(strategies.len());
    for strategy in strategies {
        let weight = strategy.fixed_weight.ok_or_else(|| {
            Error::validation(format!(
                "allocation strategy '{}' missing fixed_weight",
                strategy.id
            ))
        })?;
        if !weight.is_finite() || weight < 0.0 {
            return Err(Error::validation(format!(
                "allocation strategy '{}' fixed_weight must be finite and nonnegative",
                strategy.id
            )));
        }
        weights.push(weight);
    }
    validate_weights_sum_to_one(&weights, "fixed weights")?;
    Ok(weights)
}

fn inverse_volatility_weights(
    strategies: &[StrategyAllocationInput],
) -> Result<(Vec<f64>, Vec<Option<f64>>)> {
    if strategies.len() == 1 {
        return Ok((vec![1.0], vec![sample_volatility(&strategies[0]).ok()]));
    }

    let mut inverse_vols = Vec::with_capacity(strategies.len());
    let mut volatilities = Vec::with_capacity(strategies.len());
    for strategy in strategies {
        let volatility = sample_volatility(strategy)?;
        if volatility <= WEIGHT_TOLERANCE {
            return Err(Error::validation(format!(
                "allocation strategy '{}' has zero volatility",
                strategy.id
            )));
        }
        volatilities.push(volatility);
        inverse_vols.push(1.0 / volatility);
    }

    let denominator = neumaier_total(inverse_vols.iter().copied());
    if denominator <= WEIGHT_TOLERANCE {
        return Err(Error::validation(
            "allocation inverse volatility denominator is zero",
        ));
    }
    Ok((
        inverse_vols
            .into_iter()
            .map(|inverse_vol| inverse_vol / denominator)
            .collect(),
        volatilities.into_iter().map(Some).collect(),
    ))
}

fn sample_volatility(strategy: &StrategyAllocationInput) -> Result<f64> {
    let finite_returns = strategy
        .returns
        .iter()
        .copied()
        .filter(|value| value.is_finite())
        .collect::<Vec<_>>();
    if finite_returns.len() < 2 {
        return Err(Error::validation(format!(
            "allocation strategy '{}' requires at least two finite returns",
            strategy.id
        )));
    }
    let mean = neumaier_total(finite_returns.iter().copied()) / finite_returns.len() as f64;
    let variance = finite_returns
        .iter()
        .map(|value| {
            let centered = *value - mean;
            centered * centered
        })
        .sum::<f64>()
        / (finite_returns.len() - 1) as f64;
    Ok(variance.sqrt())
}

fn risk_budget_weights(
    strategies: &[StrategyAllocationInput],
    covariance: &[f64],
) -> Result<Vec<f64>> {
    let budgets = risk_budgets(strategies)?;
    if strategies.len() == 1 {
        return Ok(vec![1.0]);
    }

    let mut weights = budgets.clone();
    normalize_weights(&mut weights)?;

    for _ in 0..MAX_SOLVER_ITERATIONS {
        let contributions = risk_contribution_fractions(&weights, covariance)?;
        let max_error = contributions
            .iter()
            .zip(budgets.iter())
            .map(|(actual, target)| (actual - target).abs())
            .fold(0.0_f64, f64::max);
        if max_error <= SOLVER_TOLERANCE {
            return Ok(weights);
        }

        for (idx, weight) in weights.iter_mut().enumerate() {
            let actual = contributions[idx];
            if actual <= WEIGHT_TOLERANCE {
                return Err(Error::validation(format!(
                    "allocation risk contribution for strategy '{}' is nonpositive",
                    strategies[idx].id
                )));
            }
            *weight *= (budgets[idx] / actual).sqrt();
        }
        normalize_weights(&mut weights)?;
    }

    Err(Error::validation(
        "allocation risk_budget solver failed to converge",
    ))
}

fn risk_budgets(strategies: &[StrategyAllocationInput]) -> Result<Vec<f64>> {
    let mut budgets = Vec::with_capacity(strategies.len());
    for strategy in strategies {
        let budget = strategy.risk_budget.ok_or_else(|| {
            Error::validation(format!(
                "allocation strategy '{}' missing risk_budget",
                strategy.id
            ))
        })?;
        if !budget.is_finite() || budget < 0.0 {
            return Err(Error::validation(format!(
                "allocation strategy '{}' risk_budget must be finite and nonnegative",
                strategy.id
            )));
        }
        budgets.push(budget);
    }
    validate_weights_sum_to_one(&budgets, "risk budgets")?;
    Ok(budgets)
}

fn flatten_and_validate_covariance(covariance: &[Vec<f64>], n: usize) -> Result<Vec<f64>> {
    if covariance.len() != n {
        return Err(Error::validation(format!(
            "allocation covariance must have {n} rows"
        )));
    }
    let mut flat = Vec::with_capacity(n * n);
    for row in covariance {
        if row.len() != n {
            return Err(Error::validation(format!(
                "allocation covariance must be {n}x{n}"
            )));
        }
        for value in row {
            if !value.is_finite() {
                return Err(Error::validation(
                    "allocation covariance entries must be finite",
                ));
            }
            flat.push(*value);
        }
    }
    for row in 0..n {
        for col in 0..n {
            let left = flat[row * n + col];
            let right = flat[col * n + row];
            if (left - right).abs() > 1e-10 {
                return Err(Error::validation("allocation covariance must be symmetric"));
            }
        }
    }
    finstack_quant_core::math::linalg::cholesky_correlation(&flat, n).map_err(|err| {
        Error::validation(format!(
            "allocation covariance must be positive semidefinite: {err}"
        ))
    })?;
    Ok(flat)
}

fn risk_contribution_fractions(weights: &[f64], covariance: &[f64]) -> Result<Vec<f64>> {
    let n = weights.len();
    let mut sigma_w = vec![0.0; n];
    for row in 0..n {
        let mut acc = NeumaierAccumulator::new();
        for col in 0..n {
            acc.add(covariance[row * n + col] * weights[col]);
        }
        sigma_w[row] = acc.total();
    }

    let variance = weights
        .iter()
        .zip(sigma_w.iter())
        .map(|(weight, marginal)| weight * marginal)
        .sum::<f64>();
    if variance <= WEIGHT_TOLERANCE {
        return Err(Error::validation(
            "allocation portfolio variance must be positive",
        ));
    }

    Ok(weights
        .iter()
        .zip(sigma_w.iter())
        .map(|(weight, marginal)| weight * marginal / variance)
        .collect())
}

fn rounded_capitals(total_capital: f64, weights: &[f64], decimal_places: u32) -> Result<Vec<f64>> {
    let mut capitals = weights
        .iter()
        .map(|weight| round_to(total_capital * weight, decimal_places))
        .collect::<Vec<_>>();
    let target_total = round_to(total_capital, decimal_places);
    let current_total = neumaier_total(capitals.iter().copied());
    let residual = round_to(target_total - current_total, decimal_places);
    if residual.abs() > 0.0 {
        let idx = largest_weight_index(weights).ok_or_else(|| {
            Error::validation("allocation requires at least one weight for capital rounding")
        })?;
        capitals[idx] = round_to(capitals[idx] + residual, decimal_places);
    }
    Ok(capitals)
}

fn largest_weight_index(weights: &[f64]) -> Option<usize> {
    let mut best: Option<(usize, f64)> = None;
    for (idx, weight) in weights.iter().enumerate() {
        let magnitude = weight.abs();
        match best {
            Some((_, best_magnitude)) if magnitude <= best_magnitude => {}
            _ => best = Some((idx, magnitude)),
        }
    }
    best.map(|(idx, _)| idx)
}

fn round_to(value: f64, decimal_places: u32) -> f64 {
    let scale = 10_f64.powi(decimal_places as i32);
    (value * scale).round() / scale
}

fn validate_weights_sum_to_one(weights: &[f64], label: &str) -> Result<()> {
    let sum = neumaier_total(weights.iter().copied());
    if (sum - 1.0).abs() > WEIGHT_TOLERANCE {
        return Err(Error::validation(format!(
            "allocation {label} must sum to 1.0 (got {sum})"
        )));
    }
    Ok(())
}

fn normalize_weights(weights: &mut [f64]) -> Result<()> {
    let sum = neumaier_total(weights.iter().copied());
    if sum <= WEIGHT_TOLERANCE {
        return Err(Error::validation("allocation weights sum must be positive"));
    }
    for weight in weights {
        *weight /= sum;
    }
    Ok(())
}

fn neumaier_total<I>(values: I) -> f64
where
    I: IntoIterator<Item = f64>,
{
    let mut acc = NeumaierAccumulator::new();
    for value in values {
        acc.add(value);
    }
    acc.total()
}
