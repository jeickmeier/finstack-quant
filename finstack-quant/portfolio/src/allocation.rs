//! Lightweight strategy-level allocation schemes.

use crate::error::{Error, Result};
use finstack_quant_core::math::stats::variance;
use finstack_quant_core::math::summation::{neumaier_sum, NeumaierAccumulator};
use rust_decimal::prelude::{FromPrimitive, ToPrimitive};
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};

const WEIGHT_TOLERANCE: f64 = 1e-9;
const SOLVER_TOLERANCE: f64 = 1e-10;
const MAX_SOLVER_ITERATIONS: usize = 10_000;

/// Strategy allocation scheme.
#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[cfg_attr(feature = "json-schema", derive(schemars::JsonSchema))]
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
#[cfg_attr(feature = "json-schema", derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields)]
pub struct WeightAllocationSpec {
    /// Allocation scheme to apply.
    pub scheme: AllocationScheme,
    /// Finite signed capital to allocate across strategies. Rounded allocations
    /// retain this sign and sum to the total rounded at `money_decimal_places`.
    pub total_capital: f64,
    /// Strategy input rows.
    pub strategies: Vec<StrategyAllocationInput>,
    /// Optional covariance matrix for `risk_budget`, row-major as nested lists.
    pub covariance: Option<Vec<Vec<f64>>>,
    /// Number of decimal places for capital rounding, default 10 and at most 12.
    /// Rounded minor units must remain finite and representable by the
    /// floating-point output.
    #[serde(default = "default_money_decimal_places")]
    pub money_decimal_places: u32,
}

/// Per-strategy allocation input row.
#[derive(Clone, Debug, Deserialize, Serialize)]
#[cfg_attr(feature = "json-schema", derive(schemars::JsonSchema))]
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
#[derive(Clone, Debug, Deserialize, Serialize)]
#[cfg_attr(feature = "json-schema", derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields)]
pub struct WeightAllocationResult {
    /// Allocation scheme applied.
    pub scheme: AllocationScheme,
    /// Per-strategy allocation rows.
    pub allocations: Vec<StrategyAllocation>,
    /// Portfolio-level diagnostics.
    pub diagnostics: AllocationDiagnostics,
}

/// Per-strategy allocation output row.
#[derive(Clone, Debug, Deserialize, Serialize)]
#[cfg_attr(feature = "json-schema", derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields)]
pub struct StrategyAllocation {
    /// Strategy identifier.
    pub id: String,
    /// Fully invested allocation weight.
    pub weight: f64,
    /// Rounded capital allocation with the sign of total capital. Fractional
    /// minor units are assigned by largest remainder, breaking ties by input order.
    pub capital: f64,
    /// Sample volatility used by inverse-volatility allocation, when applicable.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub volatility: Option<f64>,
    /// Risk contribution fraction, when covariance diagnostics are available.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub risk_contribution: Option<f64>,
}

/// Portfolio-level allocation diagnostics.
#[derive(Clone, Debug, Deserialize, Serialize)]
#[cfg_attr(feature = "json-schema", derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields)]
pub struct AllocationDiagnostics {
    /// Sum of allocation weights.
    pub weights_sum: f64,
    /// Gross leverage, equal to sum of absolute weights.
    pub leverage: f64,
}

/// Allocate strategy weights from a typed specification.
///
/// Capital retains the sign of `total_capital` and sums to its rounded total.
/// Minor units are assigned by largest fractional remainder, with ties awarded
/// in strategy input order; zero-weight strategies receive zero capital.
///
/// # Errors
///
/// Returns [`Error::InvalidInput`] when inputs violate scheme invariants or
/// capital cannot be represented at the requested rounding precision.
///
/// # Arguments
///
/// * `spec` - [`WeightAllocationSpec`] selecting the allocation scheme,
///   strategy inputs, and any required covariance data.
pub fn allocate_weights(spec: &WeightAllocationSpec) -> Result<WeightAllocationResult> {
    spec.validate_common()?;
    let covariance = if spec.scheme == AllocationScheme::RiskBudget {
        Some(spec.covariance_matrix()?)
    } else {
        None
    };
    let mut volatilities = vec![None; spec.strategies.len()];
    let weights = match spec.scheme {
        AllocationScheme::Equal => equal_weights(spec.strategies.len()),
        AllocationScheme::Fixed => fixed_weights(&spec.strategies)?,
        AllocationScheme::InverseVolatility => {
            let (weights, vols) = inverse_volatility_weights(&spec.strategies)?;
            volatilities = vols;
            weights
        }
        AllocationScheme::RiskBudget => {
            let covariance = covariance.as_ref().ok_or_else(|| {
                Error::invalid_input("allocation covariance is required for risk_budget scheme")
            })?;
            risk_budget_weights(&spec.strategies, covariance)?
        }
    };

    let risk_contributions = if let Some(covariance) = covariance.as_ref() {
        Some(risk_contribution_fractions(&weights, covariance)?)
    } else {
        None
    };
    let capitals = rounded_capitals(spec.total_capital, &weights, spec.money_decimal_places)?;

    let allocations = spec
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
        scheme: spec.scheme,
        allocations,
        diagnostics: AllocationDiagnostics {
            weights_sum: neumaier_sum(weights.iter().copied()),
            leverage: neumaier_sum(weights.iter().map(|weight| weight.abs())),
        },
    })
}

fn default_money_decimal_places() -> u32 {
    10
}

impl WeightAllocationSpec {
    /// Parse an allocation specification from its JSON wire form.
    ///
    /// # Arguments
    ///
    /// * `spec_json` - UTF-8 JSON object with the [`WeightAllocationSpec`]
    ///   fields; unknown fields are rejected.
    ///
    /// # Errors
    ///
    /// Returns [`Error::InvalidInput`] when the text is not valid JSON or
    /// does not match the specification shape.
    pub fn from_json(spec_json: &str) -> Result<Self> {
        serde_json::from_str(spec_json)
            .map_err(|err| Error::invalid_input(format!("invalid allocation JSON: {err}")))
    }

    fn validate_common(&self) -> Result<()> {
        if !self.total_capital.is_finite() {
            return Err(Error::invalid_input(
                "allocation total_capital must be finite",
            ));
        }
        if self.strategies.is_empty() {
            return Err(Error::invalid_input(
                "allocation requires at least one strategy",
            ));
        }
        if self.money_decimal_places > 12 {
            return Err(Error::invalid_input(
                "allocation money_decimal_places must be <= 12",
            ));
        }
        let mut ids = std::collections::BTreeSet::new();
        for strategy in &self.strategies {
            if strategy.id.trim().is_empty() {
                return Err(Error::invalid_input(
                    "allocation strategy id must not be empty",
                ));
            }
            if !ids.insert(strategy.id.clone()) {
                return Err(Error::invalid_input(format!(
                    "allocation strategy id '{}' is duplicated",
                    strategy.id
                )));
            }
        }
        Ok(())
    }

    fn covariance_matrix(&self) -> Result<Vec<f64>> {
        let covariance = self.covariance.as_ref().ok_or_else(|| {
            Error::invalid_input("allocation covariance is required for risk_budget scheme")
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
            Error::invalid_input(format!(
                "allocation strategy '{}' missing fixed_weight",
                strategy.id
            ))
        })?;
        if !weight.is_finite() || weight < 0.0 {
            return Err(Error::invalid_input(format!(
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
            return Err(Error::invalid_input(format!(
                "allocation strategy '{}' has zero volatility",
                strategy.id
            )));
        }
        volatilities.push(volatility);
        inverse_vols.push(1.0 / volatility);
    }

    let denominator = neumaier_sum(inverse_vols.iter().copied());
    if denominator <= WEIGHT_TOLERANCE {
        return Err(Error::invalid_input(
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
    // A non-finite return is a data fault, not a value to skip: silently
    // filtering it would compute a volatility from a different sample than
    // the caller supplied (see the finite-sensitivity precedent in
    // `parametric::validate_finite_sensitivities`).
    if let Some((index, value)) = strategy
        .returns
        .iter()
        .enumerate()
        .find(|(_, value)| !value.is_finite())
    {
        return Err(Error::invalid_input(format!(
            "allocation strategy '{}' has non-finite return at index {index} ({value})",
            strategy.id
        )));
    }
    if strategy.returns.len() < 2 {
        return Err(Error::invalid_input(format!(
            "allocation strategy '{}' requires at least two returns",
            strategy.id
        )));
    }
    Ok(variance(&strategy.returns).sqrt())
}

fn risk_budget_weights(
    strategies: &[StrategyAllocationInput],
    covariance: &[f64],
) -> Result<Vec<f64>> {
    let budgets = risk_budgets(strategies)?;
    let n = strategies.len();
    if n == 1 {
        return Ok(vec![1.0]);
    }

    // A zero risk budget is a valid request ("allocate no risk to this
    // sleeve"): assign weight 0 up front and solve on the positive-budget
    // subset. A zero-weight strategy has zero risk contribution, which the
    // fixed-point update below cannot iterate on. Budgets are non-negative
    // and sum to 1, so the active subset is non-empty and still sums to 1.
    let active: Vec<usize> = (0..n).filter(|&idx| budgets[idx] > 0.0).collect();
    let mut full_weights = vec![0.0; n];
    if active.len() == 1 {
        full_weights[active[0]] = 1.0;
        return Ok(full_weights);
    }
    let m = active.len();
    let sub_budgets: Vec<f64> = active.iter().map(|&idx| budgets[idx]).collect();
    let mut sub_covariance = vec![0.0; m * m];
    for (row, &i) in active.iter().enumerate() {
        for (col, &j) in active.iter().enumerate() {
            sub_covariance[row * m + col] = covariance[i * n + j];
        }
    }

    let mut weights = sub_budgets.clone();
    normalize_weights(&mut weights)?;

    for _ in 0..MAX_SOLVER_ITERATIONS {
        let contributions = risk_contribution_fractions(&weights, &sub_covariance)?;
        let max_error = contributions
            .iter()
            .zip(sub_budgets.iter())
            .map(|(actual, target)| (actual - target).abs())
            .fold(0.0_f64, f64::max);
        if max_error <= SOLVER_TOLERANCE {
            for (row, &idx) in active.iter().enumerate() {
                full_weights[idx] = weights[row];
            }
            return Ok(full_weights);
        }

        for (row, weight) in weights.iter_mut().enumerate() {
            let actual = contributions[row];
            if actual <= WEIGHT_TOLERANCE {
                // Zero-budget strategies are excluded above, so a
                // nonpositive contribution here is a genuinely negative
                // (or vanishing) risk contribution — typically strongly
                // negative correlations — which this long-only fixed-point
                // scheme cannot solve.
                return Err(Error::invalid_input(format!(
                    "allocation risk contribution for strategy '{}' is nonpositive; the \
                     long-only risk_budget solver requires strictly positive risk \
                     contributions (check for strongly negative correlations)",
                    strategies[active[row]].id
                )));
            }
            *weight *= (sub_budgets[row] / actual).sqrt();
        }
        normalize_weights(&mut weights)?;
    }

    Err(Error::invalid_input(
        "allocation risk_budget solver failed to converge",
    ))
}

fn risk_budgets(strategies: &[StrategyAllocationInput]) -> Result<Vec<f64>> {
    let mut budgets = Vec::with_capacity(strategies.len());
    for strategy in strategies {
        let budget = strategy.risk_budget.ok_or_else(|| {
            Error::invalid_input(format!(
                "allocation strategy '{}' missing risk_budget",
                strategy.id
            ))
        })?;
        if !budget.is_finite() || budget < 0.0 {
            return Err(Error::invalid_input(format!(
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
        return Err(Error::invalid_input(format!(
            "allocation covariance must have {n} rows"
        )));
    }
    let mut flat = Vec::with_capacity(n * n);
    for row in covariance {
        if row.len() != n {
            return Err(Error::invalid_input(format!(
                "allocation covariance must be {n}x{n}"
            )));
        }
        for value in row {
            if !value.is_finite() {
                return Err(Error::invalid_input(
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
                return Err(Error::invalid_input(
                    "allocation covariance must be symmetric",
                ));
            }
        }
    }
    finstack_quant_core::math::linalg::cholesky_correlation(&flat, n).map_err(|err| {
        Error::invalid_input(format!(
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
        return Err(Error::invalid_input(
            "allocation portfolio variance must be positive",
        ));
    }

    Ok(weights
        .iter()
        .zip(sigma_w.iter())
        .map(|(weight, marginal)| weight * marginal / variance)
        .collect())
}

/// Splits `total_capital` across `weights` in minor units with the
/// largest-remainder rule, conserving the rounded budget exactly.
///
/// Minor units are counted in `Decimal`: at the default ten decimal places a
/// capital of 1e6 is already 1e16 minor units, past f64's 2^53 integer range,
/// where f64 floors lose their remainders and their sum drifts off the budget.
fn rounded_capitals(total_capital: f64, weights: &[f64], decimal_places: u32) -> Result<Vec<f64>> {
    let not_representable = || {
        Error::invalid_input(
            "allocation capital rounding requires finite minor units and nonnegative weights",
        )
    };
    if weights
        .iter()
        .any(|weight| !weight.is_finite() || *weight < 0.0)
    {
        return Err(not_representable());
    }
    let scale = Decimal::from(10_u64.pow(decimal_places));
    let target_units = Decimal::from_f64(total_capital.abs())
        .and_then(|capital| capital.checked_mul(scale))
        .ok_or_else(not_representable)?
        .round();
    let weights = weights
        .iter()
        .map(|weight| Decimal::from_f64(*weight).ok_or_else(not_representable))
        .collect::<Result<Vec<_>>>()?;
    let weights_sum = weights
        .iter()
        .try_fold(Decimal::ZERO, |sum, weight| sum.checked_add(*weight))
        .ok_or_else(not_representable)?;
    if weights_sum <= Decimal::ZERO {
        return Err(not_representable());
    }

    let mut units = Vec::with_capacity(weights.len());
    let mut remainders = Vec::with_capacity(weights.len());
    for (idx, weight) in weights.iter().enumerate() {
        // Normalize the accepted sum-to-one tolerance before rounding so the
        // minor-unit budget is conserved even for caller-supplied fixed weights.
        let exact_units = (*weight / weights_sum)
            .checked_mul(target_units)
            .ok_or_else(not_representable)?;
        let whole_units = exact_units.floor();
        units.push(whole_units);
        if *weight > Decimal::ZERO {
            remainders.push((idx, exact_units - whole_units));
        }
    }
    let residual = target_units - units.iter().copied().sum::<Decimal>();
    // Each positive weight contributes a remainder in [0, 1), so the floors
    // fall short of the budget by fewer units than there are positive
    // weights; Decimal's 28-digit normalization keeps that bound.
    let residual = usize::try_from(residual.to_i64().unwrap_or(-1))
        .ok()
        .filter(|residual| *residual <= remainders.len())
        .ok_or_else(|| {
            Error::invalid_input(
                "allocation capital is not representable at the requested rounding precision",
            )
        })?;
    remainders.sort_by(|left, right| right.1.cmp(&left.1).then_with(|| left.0.cmp(&right.0)));
    for (idx, _) in remainders.into_iter().take(residual) {
        units[idx] += Decimal::ONE;
    }

    let sign = if total_capital.is_sign_negative() {
        -Decimal::ONE
    } else {
        Decimal::ONE
    };
    units
        .into_iter()
        .map(|units| {
            (sign * units / scale)
                .to_f64()
                .filter(|capital| capital.is_finite())
                .ok_or_else(|| Error::invalid_input("allocation rounded capital must be finite"))
        })
        .collect()
}

fn validate_weights_sum_to_one(weights: &[f64], label: &str) -> Result<()> {
    let sum = neumaier_sum(weights.iter().copied());
    if (sum - 1.0).abs() > WEIGHT_TOLERANCE {
        return Err(Error::invalid_input(format!(
            "allocation {label} must sum to 1.0 (got {sum})"
        )));
    }
    Ok(())
}

fn normalize_weights(weights: &mut [f64]) -> Result<()> {
    let sum = neumaier_sum(weights.iter().copied());
    if sum <= WEIGHT_TOLERANCE {
        return Err(Error::invalid_input(
            "allocation weights sum must be positive",
        ));
    }
    for weight in weights {
        *weight /= sum;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn budget_strategy(id: &str, risk_budget: f64) -> StrategyAllocationInput {
        StrategyAllocationInput {
            id: id.to_string(),
            fixed_weight: None,
            returns: Vec::new(),
            risk_budget: Some(risk_budget),
        }
    }

    #[test]
    fn unsupported_target_volatility_is_rejected_at_deserialization() {
        let error = serde_json::from_value::<WeightAllocationSpec>(serde_json::json!({
            "scheme": "equal", "total_capital": 100.0, "strategies": [],
            "covariance": null, "target_volatility": null
        }))
        .expect_err("unsupported allocation control");
        assert!(error.to_string().contains("target_volatility"));
    }

    #[test]
    fn risk_budget_scheme_drops_zero_budget_strategies() {
        // A zero risk budget is a valid request ("no risk to this sleeve")
        // and must yield weight 0, not a solver failure: with weight 0 the
        // strategy's risk contribution is 0, which the fixed-point iteration
        // used to reject as "nonpositive" on its first update step. The
        // correlated a/b block forces at least one iteration.
        let spec = WeightAllocationSpec {
            scheme: AllocationScheme::RiskBudget,
            total_capital: 1000.0,
            strategies: vec![
                budget_strategy("a", 0.5),
                budget_strategy("b", 0.5),
                budget_strategy("c", 0.0),
            ],
            covariance: Some(vec![
                vec![0.04, 0.01, 0.0],
                vec![0.01, 0.09, 0.0],
                vec![0.0, 0.0, 0.04],
            ]),
            money_decimal_places: 2,
        };

        let result = allocate_weights(&spec).expect("zero-budget strategy must be allowed");
        // Two-asset equal-risk-contribution solution is inverse-vol:
        // w_a / w_b = sigma_b / sigma_a = 0.3 / 0.2.
        assert!(
            (result.allocations[0].weight - 0.6).abs() < 1e-6,
            "weight a = {}",
            result.allocations[0].weight
        );
        assert!(
            (result.allocations[1].weight - 0.4).abs() < 1e-6,
            "weight b = {}",
            result.allocations[1].weight
        );
        assert_eq!(result.allocations[2].weight, 0.0);
        assert_eq!(result.allocations[2].capital, 0.0);
    }

    #[test]
    fn capital_rounding_conserves_budget_beyond_f64_integer_precision() {
        // 1e7 at 10 decimal places is 1e17 minor units, past 2^53: f64 minor
        // units lose their fractional remainders and their sum drifts from the
        // budget, so allocation used to fail depending on the weights' last bits.
        let weights = [
            0.19330633454364168,
            0.15538965898731819,
            0.08594177409210618,
            0.1996966265689855,
            0.3656656058079483,
        ];
        let spec = WeightAllocationSpec {
            scheme: AllocationScheme::Fixed,
            total_capital: 10_000_000.0,
            strategies: weights
                .iter()
                .enumerate()
                .map(|(idx, weight)| StrategyAllocationInput {
                    id: format!("s{idx}"),
                    fixed_weight: Some(*weight),
                    returns: Vec::new(),
                    risk_budget: None,
                })
                .collect(),
            covariance: None,
            money_decimal_places: 10,
        };

        let result = allocate_weights(&spec).expect("large capital must allocate");
        for (allocation, weight) in result.allocations.iter().zip(weights) {
            assert!(
                (allocation.capital - 10_000_000.0 * weight).abs() <= 1e-8,
                "{}: capital {} for weight {weight}",
                allocation.id,
                allocation.capital
            );
        }
        let total = neumaier_sum(result.allocations.iter().map(|a| a.capital));
        assert!(
            (total - 10_000_000.0).abs() <= 1e-8,
            "capitals sum to {total}"
        );
    }

    #[test]
    fn inverse_volatility_rejects_non_finite_returns() {
        // Silently filtering a NaN return computes a volatility from a
        // different sample than the caller supplied; surface it instead.
        let spec = WeightAllocationSpec {
            scheme: AllocationScheme::InverseVolatility,
            total_capital: 1000.0,
            strategies: vec![
                StrategyAllocationInput {
                    id: "a".to_string(),
                    fixed_weight: None,
                    returns: vec![0.01, f64::NAN, 0.02, 0.01],
                    risk_budget: None,
                },
                StrategyAllocationInput {
                    id: "b".to_string(),
                    fixed_weight: None,
                    returns: vec![0.05, -0.05, 0.05, -0.05],
                    risk_budget: None,
                },
            ],
            covariance: None,
            money_decimal_places: 2,
        };

        let err = allocate_weights(&spec).expect_err("non-finite return must error");
        let message = err.to_string();
        assert!(
            message.contains("'a'") && message.contains("index 1"),
            "error must name the strategy and offending index, got: {message}"
        );
    }
}
