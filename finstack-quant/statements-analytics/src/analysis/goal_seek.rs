//! Qualified point-in-time goal seek over an editable forecast assumption.

use finstack_quant_core::dates::{Date, PeriodId};
use finstack_quant_core::market_data::context::MarketContext;
use finstack_quant_core::math::solver::BrentSolver;
use finstack_quant_statements::evaluator::{CellSource, Evaluator, RunProvenance, StatementResult};
use finstack_quant_statements::types::{AmountOrScalar, FinancialModelSpec};
use finstack_quant_statements::{Error, Result};

/// Explicit cell address for the goal-seek driver or target.
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
#[cfg_attr(feature = "json-schema", derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields)]
pub struct GoalSeekCell {
    /// Exact node identifier in the prepared financial statement.
    pub node: String,
    /// Existing row of the statement forecast.
    pub period: PeriodId,
}

/// Caller-elected policy for finding the root bracket in driver units.
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
#[cfg_attr(feature = "json-schema", derive(schemars::JsonSchema))]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum GoalSeekSearch {
    /// One preselected bracket with finite ordered endpoints.
    Bracket {
        /// Lower end of the driver range.
        lower: f64,
        /// Upper end of the driver range.
        upper: f64,
    },
    /// Bounded expansion with explicitly approved endpoints and budget.
    Bounded {
        /// Lowest driver value the solver may evaluate.
        lower: f64,
        /// Highest driver value the solver may evaluate.
        upper: f64,
        /// Starting driver value in the admitted domain.
        start: f64,
        /// Initial positive bracket half-width in driver units.
        initial_half_width: f64,
        /// Maximum number of bracket expansions.
        max_expansions: u32,
    },
}

/// Dated, typed goal-seek inputs from a single statement snapshot.
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
#[cfg_attr(feature = "json-schema", derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields)]
pub struct GoalSeekSpec {
    /// Editable, selected explicit assumption in a forecast period.
    pub driver: GoalSeekCell,
    /// Target cell in the same or a later forecast period.
    pub target: GoalSeekCell,
    /// Desired amount or scalar, in the target cell's native units.
    pub target_value: AmountOrScalar,
    /// Absolute residual tolerance in the exact type and units of the target.
    pub target_tolerance: AmountOrScalar,
    /// Finite positive root tolerance in the driver cell's units.
    pub driver_tolerance: f64,
    /// Budget for the Brent iterations following the bracket check or search.
    pub max_iterations: u32,
    /// Selected range or bounded search in the driver dimension.
    pub search: GoalSeekSearch,
    /// Market pricing day for all trial and final statement evaluations.
    #[cfg_attr(
        feature = "json-schema",
        schemars(with = "finstack_quant_core::wire::DateWire")
    )]
    pub as_of: Date,
    /// Inclusive publication cutoff held fixed throughout the solve.
    #[cfg_attr(
        feature = "json-schema",
        schemars(with = "finstack_quant_core::wire::DateWire")
    )]
    pub observed_through: Date,
    /// Return the validated trial model with the verified driver installed.
    pub update_model: bool,
}

impl GoalSeekSpec {
    /// Validate the explicit search geometry, tolerances and information date.
    ///
    /// # Errors
    /// Rejects invalid, nonfinite or inconsistent dates, target controls and search domains.
    pub fn validate(&self) -> Result<()> {
        if self.observed_through > self.as_of
            || !self.target_value.value().is_finite()
            || !self.target_tolerance.value().is_finite()
            || self.target_tolerance.value() < 0.0
            || self.target_value.currency() != self.target_tolerance.currency()
        {
            return Err(Error::invalid_input(
                "Goal seek requires a valid cutoff, finite target and nonnegative absolute tolerance in target units",
            ));
        }
        BrentSolver::new(self.driver_tolerance, self.max_iterations as usize)?;
        let (lower, upper) = match self.search {
            GoalSeekSearch::Bracket { lower, upper } => (lower, upper),
            GoalSeekSearch::Bounded {
                lower,
                upper,
                start,
                initial_half_width,
                max_expansions,
            } => {
                if !(lower..=upper).contains(&start)
                    || !initial_half_width.is_finite()
                    || initial_half_width <= 0.0
                    || max_expansions == 0
                {
                    return Err(Error::invalid_input(
                        "Goal seek expansion requires an admitted start, positive width and nonzero budget",
                    ));
                }
                (lower, upper)
            }
        };
        if !lower.is_finite() || !upper.is_finite() || lower >= upper {
            return Err(Error::invalid_input(
                "Goal seek bounds must be finite and strictly increasing",
            ));
        }
        Ok(())
    }
}

/// Verified typed result and statement provenance from a goal-seek run.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
#[cfg_attr(feature = "json-schema", derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields)]
pub struct GoalSeekResult {
    /// Exact reported driver in its original scalar or monetary type.
    pub solved_value: AmountOrScalar,
    /// Requested amount or scalar in target units.
    pub target_value: AmountOrScalar,
    /// Target observed in a fresh run at the returned driver.
    pub achieved_value: AmountOrScalar,
    /// Achieved target less requested target, in target units.
    pub residual: AmountOrScalar,
    /// Elected absolute tolerance in target units.
    pub target_tolerance: AmountOrScalar,
    /// Root tolerance in driver units.
    pub driver_tolerance: f64,
    /// Total statement evaluations, including original admission and fresh verification.
    #[serde(with = "finstack_quant_core::wire::count")]
    #[cfg_attr(feature = "json-schema", schemars(with = "u32"))]
    pub evaluations: usize,
    /// Provenance for the input statement run.
    pub original_run: RunProvenance,
    /// Provenance for the fresh run at the reported typed driver.
    pub solved_run: RunProvenance,
    /// Fingerprint of the held market used for all evaluation calls.
    pub market_hash: String,
    /// Verified runnable copy when requested.
    pub model: Option<FinancialModelSpec>,
}

impl GoalSeekResult {
    /// Load a canonical result and admit its optional solved model.
    ///
    /// # Arguments
    /// * `json` - A serialized result with typed residual, target and original and solved run identities.
    ///
    /// # Errors
    /// Rejects malformed data or inadmissible model contents.
    pub fn from_json(json: &str) -> Result<Self> {
        let mut result: Self = serde_json::from_str(json)
            .map_err(|err| Error::invalid_input(format!("Invalid GoalSeekResult JSON: {err}")))?;
        if let Some(model) = result.model.as_mut() {
            model.validate_semantics()?;
        }
        Ok(result)
    }
}

fn selected_value(
    run: &StatementResult,
    key: &GoalSeekCell,
    assumption: bool,
) -> Result<AmountOrScalar> {
    let cell = run.get_cell(&key.node, &key.period).ok_or_else(|| {
        Error::eval(format!(
            "Goal seek cannot read '{}' at {}",
            key.node, key.period
        ))
    })?;
    if assumption && cell.source != Some(CellSource::Value) {
        return Err(Error::eval(format!(
            "Goal seek driver '{}' at {} must be a visible selected assumption",
            key.node, key.period
        )));
    }
    cell.value.ok_or_else(|| {
        Error::eval(format!(
            "Goal seek cell '{}' at {} is {:?}: {:?}",
            key.node, key.period, cell.status, cell.reason
        ))
    })
}

fn checked_difference(
    actual: AmountOrScalar,
    target: AmountOrScalar,
) -> finstack_quant_core::Result<AmountOrScalar> {
    match (actual, target) {
        (AmountOrScalar::Amount(actual), AmountOrScalar::Amount(target)) => {
            actual.checked_sub(target).map(AmountOrScalar::Amount)
        }
        (AmountOrScalar::Scalar(actual), AmountOrScalar::Scalar(target))
            if (actual - target).is_finite() =>
        {
            Ok(AmountOrScalar::Scalar(actual - target))
        }
        _ => Err(finstack_quant_core::Error::Validation(
            "Goal seek target type, currency or numeric domain changed".into(),
        )),
    }
}

fn accepts(residual: AmountOrScalar, tolerance: AmountOrScalar) -> bool {
    match (residual, tolerance) {
        (AmountOrScalar::Amount(residual), AmountOrScalar::Amount(tolerance)) => {
            residual.currency() == tolerance.currency()
                && residual.amount_decimal().abs() <= tolerance.amount_decimal()
        }
        (AmountOrScalar::Scalar(residual), AmountOrScalar::Scalar(tolerance)) => {
            tolerance.is_finite() && residual.abs() <= tolerance
        }
        _ => false,
    }
}

/// Reuse a prepared graph to verify the reported typed root on one held snapshot.
///
/// # Arguments
/// * `model` - Statement model with a source snapshot and the existing editable forecast cell.
/// * `spec` - Dated driver, target and absolute acceptance tolerances with a bounded search policy.
/// * `market` - Held capital-structure market data or an empty market for unlinked models.
///
/// # Errors
/// Rejects unavailable or masked assumptions, unit or currency mismatches,
/// invalid search domains, failed trials, and verified residuals outside the
/// elected target tolerance.
pub fn goal_seek(
    model: &FinancialModelSpec,
    spec: &GoalSeekSpec,
    market: &MarketContext,
) -> Result<GoalSeekResult> {
    spec.validate()?;
    let driver_period = model
        .periods
        .iter()
        .position(|p| p.id == spec.driver.period && !p.is_actual)
        .ok_or_else(|| {
            Error::invalid_input("Goal seek driver must be in an existing forecast period")
        })?;
    let target_period = model
        .periods
        .iter()
        .position(|p| p.id == spec.target.period)
        .ok_or_else(|| Error::invalid_input("Goal seek target period is absent"))?;
    if target_period < driver_period {
        return Err(Error::invalid_input(
            "Goal seek target must be in the same or a later period than the editable assumption",
        ));
    }
    let mut evaluator = Evaluator::new();
    let prepared = evaluator.prepare(model)?;
    let initial = evaluator.evaluate_prepared_at(
        model,
        &prepared,
        market,
        spec.as_of,
        spec.observed_through,
    )?;
    let original_run = initial
        .provenance
        .clone()
        .ok_or_else(|| Error::eval("Missing original goal-seek run provenance"))?;
    let market_hash = original_run
        .market_content_hash
        .clone()
        .ok_or_else(|| Error::eval("Goal seek requires a coherent captured market snapshot"))?;
    let initial_assumption = selected_value(&initial, &spec.driver, true)?;
    checked_difference(
        selected_value(&initial, &spec.target, false)?,
        spec.target_value,
    )?;
    let mut working = model.clone();
    let mut evaluations = 1;
    let mut trial = |candidate| -> Result<(AmountOrScalar, AmountOrScalar, StatementResult)> {
        let driver = if let Some(currency) = initial_assumption.currency() {
            AmountOrScalar::amount(candidate, currency)?
        } else {
            AmountOrScalar::Scalar(candidate)
        };
        let slot = working
            .nodes
            .get_mut(spec.driver.node.as_str())
            .and_then(|node| node.values.as_mut())
            .and_then(|values| values.get_mut(&spec.driver.period))
            .ok_or_else(|| Error::eval("Admitted goal-seek assumption slot disappeared"))?;
        *slot = driver;
        evaluations += 1;
        let result = evaluator.evaluate_prepared_at(
            &working,
            &prepared,
            market,
            spec.as_of,
            spec.observed_through,
        )?;
        if result
            .provenance
            .as_ref()
            .and_then(|run| run.market_content_hash.as_ref())
            != Some(&market_hash)
        {
            return Err(Error::eval(
                "Goal seek trial used a different or unavailable market snapshot",
            ));
        }
        if selected_value(&result, &spec.driver, true)? != driver {
            return Err(Error::eval(
                "Goal seek evaluation did not select the supplied typed driver",
            ));
        }
        let achieved = selected_value(&result, &spec.target, false)?;
        Ok((driver, achieved, result))
    };
    let mut objective = |candidate| -> finstack_quant_core::Result<f64> {
        let (_, achieved, _) = trial(candidate).map_err(|err| {
            finstack_quant_core::Error::Validation(format!(
                "Goal seek trial at driver {candidate} failed: {err}"
            ))
        })?;
        Ok(checked_difference(achieved, spec.target_value)?.value())
    };
    let solver = BrentSolver::new(spec.driver_tolerance, spec.max_iterations as usize)?;
    let solution = match spec.search {
        GoalSeekSearch::Bracket { lower, upper } => {
            solver.solve_in_bracket(&mut objective, lower, upper)?
        }
        GoalSeekSearch::Bounded {
            lower,
            upper,
            start,
            initial_half_width,
            max_expansions,
        } => solver.solve(
            &mut objective,
            start,
            lower,
            upper,
            initial_half_width,
            max_expansions as usize,
        )?,
    };
    let (driver, achieved, verified) = trial(solution)?;
    let residual = checked_difference(achieved, spec.target_value)?;
    if !accepts(residual, spec.target_tolerance) {
        return Err(Error::eval(format!(
            "Goal-seek verified target miss at {solution}: residual {} exceeds absolute tolerance {}",
            residual.value(),
            spec.target_tolerance.value()
        )));
    }
    let solved_run = verified
        .provenance
        .ok_or_else(|| Error::eval("Missing solved goal-seek run provenance"))?;
    Ok(GoalSeekResult {
        solved_value: driver,
        target_value: spec.target_value,
        achieved_value: achieved,
        residual,
        target_tolerance: spec.target_tolerance,
        driver_tolerance: spec.driver_tolerance,
        evaluations,
        original_run,
        solved_run,
        market_hash,
        model: spec.update_model.then_some(working),
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use finstack_quant_statements::builder::ModelBuilder;
    use time::macros::date;

    #[test]
    fn checked_goal_seek_verifies_a_typed_forecast_cell_without_mutating_input() {
        let first = PeriodId::quarter(2025, 1).unwrap();
        let last = PeriodId::quarter(2025, 2).unwrap();
        let mut builder = ModelBuilder::new("held-forecast");
        builder.periods("2025Q1..Q2", None).unwrap();
        builder.source_snapshot("forecast-2024-12-31");
        builder.value_scalar("driver", &[(first, 6.0), (last, 7.0)]);
        builder.compute("target", "driver * 3.0").unwrap();
        let model = builder.build().unwrap();
        let original_hash = model.content_hash().unwrap();
        let spec = GoalSeekSpec {
            driver: GoalSeekCell {
                node: "driver".into(),
                period: last,
            },
            target: GoalSeekCell {
                node: "target".into(),
                period: last,
            },
            target_value: AmountOrScalar::Scalar(27.0),
            target_tolerance: AmountOrScalar::Scalar(1e-9),
            driver_tolerance: 1e-12,
            max_iterations: 64,
            search: GoalSeekSearch::Bracket {
                lower: 0.0,
                upper: 12.0,
            },
            as_of: date!(2024 - 12 - 31),
            observed_through: date!(2024 - 12 - 31),
            update_model: true,
        };
        let result = goal_seek(&model, &spec, &MarketContext::new()).unwrap();
        assert!((result.solved_value.value() - 9.0).abs() < 1e-12);
        assert!(result.residual.value().abs() <= spec.target_tolerance.value());
        assert_eq!(result.original_run.model_content_hash, original_hash);
        assert_ne!(result.solved_run.model_content_hash, original_hash);
        assert_eq!(model.content_hash().unwrap(), original_hash);
        assert_eq!(
            result.model.unwrap().content_hash().unwrap(),
            result.solved_run.model_content_hash
        );
    }
}
