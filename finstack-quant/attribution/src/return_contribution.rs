//! Single-period return-contribution attribution.

use finstack_quant_core::math::summation::NeumaierAccumulator;
use finstack_quant_core::{Error, Result};
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};

const WEIGHT_TOLERANCE: f64 = 1e-9;

/// Input weighting mode for market-value positions.
#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
#[derive(Default)]
pub enum ReturnContributionWeighting {
    /// Normalize by signed net market value.
    NetMarketValue,
    /// Normalize by gross absolute market value.
    #[default]
    Gross,
}

/// JSON specification for single-period return contribution attribution.
#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct ReturnContributionSpec {
    /// As-of date or timestamp label for the attribution run.
    pub as_of: String,
    /// Position rows to attribute.
    pub positions: Vec<ReturnContributionPosition>,
    /// Optional factor exposure rows.
    #[serde(default)]
    pub factors: Vec<ReturnContributionFactor>,
    /// Weighting mode used when positions carry market values.
    #[serde(default)]
    pub weighting: ReturnContributionWeighting,
}

/// Position input row for return contribution attribution.
#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct ReturnContributionPosition {
    /// Stable instrument identifier.
    pub id: String,
    /// Position market value. Mutually exclusive with `weight`.
    pub market_value: Option<f64>,
    /// Position weight. Mutually exclusive with `market_value`.
    pub weight: Option<f64>,
    /// Period arithmetic return.
    #[serde(rename = "return")]
    pub period_return: f64,
    /// Arbitrary grouping labels.
    #[serde(default)]
    pub groups: BTreeMap<String, String>,
    /// Optional benchmark weight for Brinson-Fachler output.
    pub benchmark_weight: Option<f64>,
    /// Optional benchmark return for Brinson-Fachler output.
    pub benchmark_return: Option<f64>,
}

/// Factor exposure input row.
#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct ReturnContributionFactor {
    /// Factor identifier.
    pub factor: String,
    /// Portfolio exposure to the factor.
    pub exposure: f64,
    /// Factor return over the attribution period.
    pub factor_return: f64,
}

/// Return-contribution attribution result.
#[derive(Clone, Debug, Serialize)]
pub struct ReturnContributionResult {
    /// Total portfolio return, equal to summed instrument contribution.
    pub portfolio_return: f64,
    /// Per-instrument contribution rows.
    pub instrument_contribution: Vec<InstrumentContribution>,
    /// Contributions by arbitrary group dimension.
    pub group_contribution: BTreeMap<String, Vec<GroupContribution>>,
    /// Factor contribution rows.
    pub factor_contribution: Vec<FactorContribution>,
    /// Benchmark-relative attribution, when benchmark fields are supplied.
    pub benchmark_relative: Option<BenchmarkRelativeContribution>,
}

/// Per-instrument contribution output row.
#[derive(Clone, Debug, Serialize)]
pub struct InstrumentContribution {
    /// Stable instrument identifier.
    pub id: String,
    /// Portfolio weight used for contribution.
    pub weight: f64,
    /// Period arithmetic return.
    #[serde(rename = "return")]
    pub period_return: f64,
    /// Weight times return.
    pub contribution: f64,
    /// Active contribution versus benchmark contribution, when benchmark is supplied.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub active_contribution: Option<f64>,
}

/// Group-level contribution row.
#[derive(Clone, Debug, Serialize)]
pub struct GroupContribution {
    /// Group bucket key.
    pub key: String,
    /// Sum of instrument contributions in the bucket.
    pub contribution: f64,
}

/// Factor contribution output row.
#[derive(Clone, Debug, Serialize)]
pub struct FactorContribution {
    /// Factor identifier.
    pub factor: String,
    /// Portfolio exposure to the factor.
    pub exposure: f64,
    /// Factor return over the attribution period.
    pub factor_return: f64,
    /// Exposure times factor return.
    pub contribution: f64,
}

/// Benchmark-relative return attribution.
#[derive(Clone, Debug, Serialize)]
pub struct BenchmarkRelativeContribution {
    /// Benchmark total return.
    pub benchmark_return: f64,
    /// Portfolio return minus benchmark return.
    pub active_return: f64,
    /// Brinson-Fachler allocation effect.
    pub allocation_effect: f64,
    /// Brinson-Fachler selection effect.
    pub selection_effect: f64,
    /// Brinson-Fachler interaction effect.
    pub interaction_effect: f64,
    /// Difference between active return and reconstructed Brinson effects.
    pub residual: f64,
}

#[derive(Clone, Debug)]
struct WeightedPosition<'a> {
    input: &'a ReturnContributionPosition,
    weight: f64,
    contribution: f64,
}

#[derive(Default)]
struct BrinsonGroup {
    portfolio_weight: NeumaierAccumulator,
    portfolio_contribution: NeumaierAccumulator,
    benchmark_weight: NeumaierAccumulator,
    benchmark_contribution: NeumaierAccumulator,
}

/// Compute return contribution attribution from a JSON specification.
///
/// # Errors
///
/// Returns a validation error when the JSON is malformed or violates the
/// weighting/benchmark invariants.
pub fn attribute_return_contribution(spec_json: &str) -> Result<String> {
    let spec = parse_return_contribution_spec(spec_json)?;
    let result = spec.execute()?;
    serde_json::to_string(&result)
        .map_err(|err| Error::Internal(format!("failed to serialize return contribution: {err}")))
}

/// Validate a return contribution JSON specification.
///
/// # Errors
///
/// Returns a validation error when the JSON is malformed or cannot be executed.
pub fn validate_return_contribution_json(spec_json: &str) -> Result<()> {
    let spec = parse_return_contribution_spec(spec_json)?;
    spec.execute().map(|_| ())
}

fn parse_return_contribution_spec(spec_json: &str) -> Result<ReturnContributionSpec> {
    serde_json::from_str(spec_json)
        .map_err(|err| Error::Validation(format!("invalid return contribution JSON: {err}")))
}

impl ReturnContributionSpec {
    fn execute(&self) -> Result<ReturnContributionResult> {
        self.validate_shape()?;
        let weighted = self.weighted_positions()?;
        let benchmark_mode = self.validate_benchmark_mode()?;

        let mut portfolio_return = NeumaierAccumulator::new();
        for position in &weighted {
            portfolio_return.add(position.contribution);
        }
        let portfolio_return = portfolio_return.total();

        let benchmark_return = if benchmark_mode {
            Some(total_benchmark_return(&weighted)?)
        } else {
            None
        };

        Ok(ReturnContributionResult {
            portfolio_return,
            instrument_contribution: instrument_contributions(&weighted, benchmark_return),
            group_contribution: group_contributions(&weighted),
            factor_contribution: factor_contributions(&self.factors)?,
            benchmark_relative: if benchmark_mode {
                Some(benchmark_relative(
                    &weighted,
                    portfolio_return,
                    benchmark_return.ok_or_else(|| {
                        Error::Internal("benchmark mode lost benchmark return".to_string())
                    })?,
                )?)
            } else {
                None
            },
        })
    }

    fn validate_shape(&self) -> Result<()> {
        if self.as_of.trim().is_empty() {
            return Err(Error::Validation(
                "return contribution as_of must not be empty".to_string(),
            ));
        }
        if self.positions.is_empty() {
            return Err(Error::Validation(
                "return contribution requires at least one position".to_string(),
            ));
        }

        for position in &self.positions {
            if position.id.trim().is_empty() {
                return Err(Error::Validation(
                    "return contribution position id must not be empty".to_string(),
                ));
            }
            if !position.period_return.is_finite() {
                return Err(Error::Validation(format!(
                    "return contribution position '{}' return must be finite",
                    position.id
                )));
            }
            if let Some(market_value) = position.market_value {
                if !market_value.is_finite() {
                    return Err(Error::Validation(format!(
                        "return contribution position '{}' market_value must be finite",
                        position.id
                    )));
                }
            }
            if let Some(weight) = position.weight {
                if !weight.is_finite() {
                    return Err(Error::Validation(format!(
                        "return contribution position '{}' weight must be finite",
                        position.id
                    )));
                }
            }
            if position.market_value.is_some() == position.weight.is_some() {
                return Err(Error::Validation(format!(
                    "return contribution position '{}' must supply exactly one of market_value or weight",
                    position.id
                )));
            }
            match (position.benchmark_weight, position.benchmark_return) {
                (Some(weight), Some(ret)) => {
                    if !weight.is_finite() || !ret.is_finite() {
                        return Err(Error::Validation(format!(
                            "return contribution benchmark fields for '{}' must be finite",
                            position.id
                        )));
                    }
                }
                (None, None) => {}
                _ => {
                    return Err(Error::Validation(format!(
                        "return contribution position '{}' must supply both benchmark_weight and benchmark_return or neither",
                        position.id
                    )));
                }
            }
        }

        let has_market_values = self
            .positions
            .iter()
            .any(|position| position.market_value.is_some());
        let has_weights = self
            .positions
            .iter()
            .any(|position| position.weight.is_some());
        if has_market_values && has_weights {
            return Err(Error::Validation(
                "return contribution positions must not mix market_value and weight inputs"
                    .to_string(),
            ));
        }

        for factor in &self.factors {
            if factor.factor.trim().is_empty() {
                return Err(Error::Validation(
                    "return contribution factor id must not be empty".to_string(),
                ));
            }
            if !factor.exposure.is_finite() || !factor.factor_return.is_finite() {
                return Err(Error::Validation(format!(
                    "return contribution factor '{}' exposure and factor_return must be finite",
                    factor.factor
                )));
            }
        }

        Ok(())
    }

    fn weighted_positions(&self) -> Result<Vec<WeightedPosition<'_>>> {
        let weights = if self
            .positions
            .iter()
            .all(|position| position.weight.is_some())
        {
            explicit_weights(&self.positions)?
        } else {
            market_value_weights(&self.positions, self.weighting)?
        };

        Ok(self
            .positions
            .iter()
            .zip(weights)
            .map(|(input, weight)| WeightedPosition {
                input,
                weight,
                contribution: weight * input.period_return,
            })
            .collect())
    }

    fn validate_benchmark_mode(&self) -> Result<bool> {
        let benchmark_count = self
            .positions
            .iter()
            .filter(|position| position.benchmark_weight.is_some())
            .count();
        if benchmark_count == 0 {
            return Ok(false);
        }
        if benchmark_count != self.positions.len() {
            return Err(Error::Validation(
                "return contribution benchmark fields must be present on every position or none"
                    .to_string(),
            ));
        }
        Ok(true)
    }
}

fn explicit_weights(positions: &[ReturnContributionPosition]) -> Result<Vec<f64>> {
    let mut weights = Vec::with_capacity(positions.len());
    for position in positions {
        weights.push(position.weight.ok_or_else(|| {
            Error::Internal("explicit weight mode encountered missing weight".to_string())
        })?);
    }
    Ok(weights)
}

fn market_value_weights(
    positions: &[ReturnContributionPosition],
    weighting: ReturnContributionWeighting,
) -> Result<Vec<f64>> {
    let mut denominator = NeumaierAccumulator::new();
    for position in positions {
        let market_value = position.market_value.ok_or_else(|| {
            Error::Internal("market value mode encountered missing market_value".to_string())
        })?;
        match weighting {
            ReturnContributionWeighting::Gross => denominator.add(market_value.abs()),
            ReturnContributionWeighting::NetMarketValue => denominator.add(market_value),
        }
    }

    let denominator = denominator.total();
    if denominator.abs() <= WEIGHT_TOLERANCE {
        return Ok(vec![0.0; positions.len()]);
    }

    positions
        .iter()
        .map(|position| {
            let market_value = position.market_value.ok_or_else(|| {
                Error::Internal("market value mode encountered missing market_value".to_string())
            })?;
            Ok(match weighting {
                ReturnContributionWeighting::Gross => market_value.abs() / denominator,
                ReturnContributionWeighting::NetMarketValue => market_value / denominator,
            })
        })
        .collect()
}

fn instrument_contributions(
    weighted: &[WeightedPosition<'_>],
    benchmark_return: Option<f64>,
) -> Vec<InstrumentContribution> {
    let mut rows = weighted
        .iter()
        .map(|position| {
            let active_contribution = match (
                benchmark_return,
                position.input.benchmark_weight,
                position.input.benchmark_return,
            ) {
                (Some(_), Some(benchmark_weight), Some(benchmark_period_return)) => {
                    Some(position.contribution - benchmark_weight * benchmark_period_return)
                }
                _ => None,
            };
            InstrumentContribution {
                id: position.input.id.clone(),
                weight: position.weight,
                period_return: position.input.period_return,
                contribution: position.contribution,
                active_contribution,
            }
        })
        .collect::<Vec<_>>();
    rows.sort_by(|left, right| left.id.cmp(&right.id));
    rows
}

fn group_contributions(
    weighted: &[WeightedPosition<'_>],
) -> BTreeMap<String, Vec<GroupContribution>> {
    let mut group_names = BTreeSet::new();
    for position in weighted {
        for group_name in position.input.groups.keys() {
            group_names.insert(group_name.clone());
        }
    }

    let mut result = BTreeMap::new();
    for group_name in group_names {
        let mut buckets: BTreeMap<String, NeumaierAccumulator> = BTreeMap::new();
        for position in weighted {
            let key = position
                .input
                .groups
                .get(&group_name)
                .cloned()
                .unwrap_or_else(|| "unknown".to_string());
            buckets.entry(key).or_default().add(position.contribution);
        }
        result.insert(
            group_name,
            buckets
                .into_iter()
                .map(|(key, contribution)| GroupContribution {
                    key,
                    contribution: contribution.total(),
                })
                .collect(),
        );
    }
    result
}

fn factor_contributions(factors: &[ReturnContributionFactor]) -> Result<Vec<FactorContribution>> {
    let mut rows = factors
        .iter()
        .map(|factor| FactorContribution {
            factor: factor.factor.clone(),
            exposure: factor.exposure,
            factor_return: factor.factor_return,
            contribution: factor.exposure * factor.factor_return,
        })
        .collect::<Vec<_>>();
    rows.sort_by(|left, right| left.factor.cmp(&right.factor));
    Ok(rows)
}

fn total_benchmark_return(weighted: &[WeightedPosition<'_>]) -> Result<f64> {
    let mut benchmark_return = NeumaierAccumulator::new();
    for position in weighted {
        let weight = position.input.benchmark_weight.ok_or_else(|| {
            Error::Internal("benchmark mode encountered missing benchmark_weight".to_string())
        })?;
        let ret = position.input.benchmark_return.ok_or_else(|| {
            Error::Internal("benchmark mode encountered missing benchmark_return".to_string())
        })?;
        benchmark_return.add(weight * ret);
    }
    Ok(benchmark_return.total())
}

fn benchmark_relative(
    weighted: &[WeightedPosition<'_>],
    portfolio_return: f64,
    benchmark_return: f64,
) -> Result<BenchmarkRelativeContribution> {
    validate_benchmark_weights(weighted)?;
    let group_dimension = brinson_group_dimension(weighted);
    let mut groups: BTreeMap<String, BrinsonGroup> = BTreeMap::new();
    for position in weighted {
        let key = match &group_dimension {
            Some(name) => position
                .input
                .groups
                .get(name)
                .cloned()
                .unwrap_or_else(|| "unknown".to_string()),
            None => "all".to_string(),
        };
        let group = groups.entry(key).or_default();
        group.portfolio_weight.add(position.weight);
        group.portfolio_contribution.add(position.contribution);
        group
            .benchmark_weight
            .add(position.input.benchmark_weight.ok_or_else(|| {
                Error::Internal("benchmark mode encountered missing benchmark_weight".to_string())
            })?);
        group.benchmark_contribution.add(
            position.input.benchmark_weight.ok_or_else(|| {
                Error::Internal("benchmark mode encountered missing benchmark_weight".to_string())
            })? * position.input.benchmark_return.ok_or_else(|| {
                Error::Internal("benchmark mode encountered missing benchmark_return".to_string())
            })?,
        );
    }

    let mut allocation = NeumaierAccumulator::new();
    let mut selection = NeumaierAccumulator::new();
    let mut interaction = NeumaierAccumulator::new();

    for group in groups.values() {
        let portfolio_weight = group.portfolio_weight.total();
        let benchmark_weight = group.benchmark_weight.total();
        let portfolio_group_return = if portfolio_weight.abs() <= WEIGHT_TOLERANCE {
            0.0
        } else {
            group.portfolio_contribution.total() / portfolio_weight
        };
        let benchmark_group_return = if benchmark_weight.abs() <= WEIGHT_TOLERANCE {
            0.0
        } else {
            group.benchmark_contribution.total() / benchmark_weight
        };

        let weight_delta = portfolio_weight - benchmark_weight;
        let return_delta = portfolio_group_return - benchmark_group_return;
        allocation.add(weight_delta * (benchmark_group_return - benchmark_return));
        selection.add(benchmark_weight * return_delta);
        interaction.add(weight_delta * return_delta);
    }

    let allocation_effect = allocation.total();
    let selection_effect = selection.total();
    let interaction_effect = interaction.total();
    let active_return = portfolio_return - benchmark_return;
    let residual = active_return - allocation_effect - selection_effect - interaction_effect;

    Ok(BenchmarkRelativeContribution {
        benchmark_return,
        active_return,
        allocation_effect,
        selection_effect,
        interaction_effect,
        residual,
    })
}

fn validate_benchmark_weights(weighted: &[WeightedPosition<'_>]) -> Result<()> {
    let mut portfolio_weight = NeumaierAccumulator::new();
    let mut benchmark_weight = NeumaierAccumulator::new();
    for position in weighted {
        portfolio_weight.add(position.weight);
        benchmark_weight.add(position.input.benchmark_weight.ok_or_else(|| {
            Error::Internal("benchmark mode encountered missing benchmark_weight".to_string())
        })?);
    }

    let portfolio_weight = portfolio_weight.total();
    if (portfolio_weight - 1.0).abs() > WEIGHT_TOLERANCE {
        return Err(Error::Validation(format!(
            "return contribution portfolio weights must sum to 1.0 for benchmark-relative attribution (got {portfolio_weight})"
        )));
    }

    let benchmark_weight = benchmark_weight.total();
    if (benchmark_weight - 1.0).abs() > WEIGHT_TOLERANCE {
        return Err(Error::Validation(format!(
            "return contribution benchmark weights must sum to 1.0 for benchmark-relative attribution (got {benchmark_weight})"
        )));
    }

    Ok(())
}

fn brinson_group_dimension(weighted: &[WeightedPosition<'_>]) -> Option<String> {
    let mut names = BTreeSet::new();
    let mut has_sector = false;
    for position in weighted {
        if position.input.groups.contains_key("sector") {
            has_sector = true;
        }
        for name in position.input.groups.keys() {
            names.insert(name.clone());
        }
    }
    if has_sector {
        Some("sector".to_string())
    } else {
        names.into_iter().next()
    }
}
