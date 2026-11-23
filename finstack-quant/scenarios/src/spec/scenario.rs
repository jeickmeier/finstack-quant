//! Top-level [`ScenarioSpec`] definition and builder methods.

use super::operations::OperationSpec;
use finstack_quant_core::market_data::hierarchy::ResolutionMode;
use serde::{Deserialize, Serialize};

/// A complete scenario specification with metadata and ordered operations.
///
/// # Fields
/// - `id`: Stable identifier used for persistence and reporting.
/// - `name`: Optional display name for UI or logs.
/// - `description`: Optional text describing the intent of the scenario.
/// - `operations`: Ordered list of [`OperationSpec`] values to execute.
/// - `priority`: Used by [`ScenarioSpec::compose`](crate::spec::ScenarioSpec::compose)
///   to determine merge ordering (lower numbers run first).
/// - `resolution_mode`: Controls how hierarchy-targeted shocks at multiple tree
///   levels combine for a single curve. Defaults to [`ResolutionMode::MostSpecificWins`].
///
/// # Examples
///
/// ```rust
/// use finstack_quant_scenarios::{ScenarioSpec, OperationSpec, CurveKind};
/// use finstack_quant_core::market_data::hierarchy::ResolutionMode;
///
/// let scenario = ScenarioSpec {
///     id: "stress_test".into(),
///     name: Some("Q1 Stress".into()),
///     description: Some("Rate shock scenario".into()),
///     operations: vec![
///         OperationSpec::CurveParallelBp {
///             curve_kind: CurveKind::Discount,
///             curve_id: "USD_SOFR".into(),
///             discount_curve_id: None,
///             bp: 50.0,
///         },
///     ],
///     priority: 0,
///     resolution_mode: ResolutionMode::default(),
/// };
/// ```
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "json-schema", derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields)]
pub struct ScenarioSpec {
    /// Unique identifier for this scenario.
    pub id: String,

    /// Optional human-readable name.
    #[serde(default)]
    pub name: Option<String>,

    /// Optional description.
    #[serde(default)]
    pub description: Option<String>,

    /// List of operations to apply.
    pub operations: Vec<OperationSpec>,

    /// Priority for composition (lower value = higher priority).
    #[serde(default)]
    pub priority: i32,

    /// Resolution mode for hierarchy-targeted operations.
    ///
    /// Only relevant when operations use hierarchy targeting.
    /// Default: [`ResolutionMode::MostSpecificWins`].
    #[serde(default)]
    pub resolution_mode: ResolutionMode,
}

impl ScenarioSpec {
    /// Strict composition: merge scenarios into a single spec, sorted by
    /// ascending priority, returning an error at compose time when the
    /// concatenated operations would be rejected at apply time.
    ///
    /// Every input is validated first (see [`ScenarioSpec::validate`]), so
    /// the composed spec is built only from valid scenarios. Composition then
    /// rejects scenarios that together contain more than one
    /// [`OperationSpec::TimeRollForward`].
    ///
    /// # Merge rules
    ///
    /// Inputs are stably sorted by ascending `priority` (equal priorities keep
    /// input order); every rule below follows that execution order.
    ///
    /// | Field | Composed value |
    /// |-------|----------------|
    /// | `id` | Input ids joined with `+` (`"composed"` when empty) |
    /// | `name` | Input names (the id where a name is absent) joined with `" + "` |
    /// | `description` | Present input descriptions joined with `" + "`; `None` when no input has one |
    /// | `operations` | Input operations concatenated in execution order |
    /// | `priority` | The lowest input priority, so the composed spec sorts where its first-running input did (`0` when empty) |
    /// | `resolution_mode` | The shared mode (inputs must agree; [`ResolutionMode::default`] when empty) |
    ///
    /// # Arguments
    ///
    /// * `scenarios` - Scenario specifications to merge in ascending priority
    ///   order. Every non-empty input must use the same `resolution_mode`;
    ///   conflicting modes are rejected before composition.
    ///
    /// # Errors
    ///
    /// Returns a validation error, prefixed with the offending scenario id,
    /// when any input fails [`ScenarioSpec::validate`] (blank id, invalid or
    /// non-finite operation fields, more than one time roll), and a validation
    /// error when `scenarios` contain conflicting `resolution_mode` values or
    /// together contain more than one time-roll operation.
    pub fn compose(mut scenarios: Vec<ScenarioSpec>) -> crate::Result<Self> {
        for scenario in &scenarios {
            scenario.validate().map_err(|error| {
                crate::error::Error::validation(format!(
                    "Cannot compose scenario '{}': {error}",
                    scenario.id
                ))
            })?;
        }
        if let Some(first) = scenarios.first()
            && let Some(conflicting) = scenarios
                .iter()
                .skip(1)
                .find(|scenario| scenario.resolution_mode != first.resolution_mode)
        {
            return Err(crate::error::Error::validation(format!(
                "Cannot compose scenarios '{}' (resolution_mode '{}') and '{}' \
                     (resolution_mode '{}'): all scenarios must use the same resolution_mode.",
                first.id,
                finstack_quant_core::wire::serde_label(&first.resolution_mode)?,
                conflicting.id,
                finstack_quant_core::wire::serde_label(&conflicting.resolution_mode)?,
            )));
        }
        // Stable so equal-priority scenarios keep input order.
        scenarios.sort_by_key(|s| s.priority);

        let composed_id = if scenarios.is_empty() {
            "composed".to_string()
        } else {
            scenarios
                .iter()
                .map(|scenario| scenario.id.as_str())
                .collect::<Vec<_>>()
                .join("+")
        };
        let composed_name = if scenarios.is_empty() {
            Some("Composed Scenario".to_string())
        } else {
            Some(
                scenarios
                    .iter()
                    .map(|scenario| scenario.name.as_deref().unwrap_or(scenario.id.as_str()))
                    .collect::<Vec<_>>()
                    .join(" + "),
            )
        };
        let resolution_mode = scenarios
            .first()
            .map_or_else(ResolutionMode::default, |scenario| scenario.resolution_mode);
        let priority = scenarios.first().map_or(0, |scenario| scenario.priority);
        let descriptions: Vec<&str> = scenarios
            .iter()
            .filter_map(|scenario| scenario.description.as_deref())
            .collect();
        let description = (!descriptions.is_empty()).then(|| descriptions.join(" + "));

        let mut all_operations = Vec::new();
        for scenario in scenarios {
            all_operations.extend(scenario.operations);
        }

        let time_roll_count = all_operations
            .iter()
            .filter(|op| matches!(op, OperationSpec::TimeRollForward { .. }))
            .count();
        if time_roll_count > 1 {
            return Err(crate::error::Error::validation(format!(
                "Compose would produce {time_roll_count} TimeRollForward operations; only \
                 one is allowed per composed scenario. Merge the roll periods into a single \
                 `TimeRollForward` (preferred) or remove the duplicates before calling compose."
            )));
        }

        Ok(Self {
            id: composed_id,
            name: composed_name,
            description,
            operations: all_operations,
            priority,
            resolution_mode,
        })
    }

    /// Whether applying this scenario needs the portfolio's instruments.
    ///
    /// Instrument-mutating shocks and time rolls need instrument access.
    /// Market-only, statement-only, and rate-binding operations do not.
    ///
    /// # Returns
    ///
    /// `true` when the execution context must include instruments.
    pub fn requires_instruments(&self) -> bool {
        self.operations
            .iter()
            .any(OperationSpec::requires_instruments)
    }

    /// Whether applying this scenario can replace or mutate instruments.
    ///
    /// # Returns
    ///
    /// `true` for instrument price, spread, or correlation shocks. A time roll
    /// reads instruments for carry but does not mutate them and therefore
    /// returns `false`.
    pub fn mutates_instruments(&self) -> bool {
        self.operations
            .iter()
            .any(OperationSpec::mutates_instruments)
    }
}
