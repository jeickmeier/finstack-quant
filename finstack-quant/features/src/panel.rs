//! JSON and typed-spec orchestration for panel transform pipelines.
//!
//! [`transform_panel_json`] accepts a UTF-8 JSON [`PanelTransformSpec`] and returns
//! a JSON object mapping operation names to output columns.
//! [`transform_panel`] is the typed Rust entry point and preserves
//! operation order in [`PanelTransformResult`].

use crate::{transform_cross_sectional, transform_timeseries, CrossSectionalOp, TimeSeriesOp};
use finstack_quant_core::{Error, Result};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::BTreeSet;

/// Apply a list of named panel transforms from a JSON specification.
///
/// Operations run sequentially. Each op reads the previous column by default;
/// set `input` to `"values"` or an earlier operation name to select a source.
///
/// # Arguments
///
/// * `spec_json` - UTF-8 JSON document encoding a [`PanelTransformSpec`],
///   including values, required partition columns, and named operations. Each
///   operation may set optional `input` (`None` default: previous column, or
///   raw `values` for the first op).
///
/// # Errors
///
/// Returns a validation error when the specification is malformed or an
/// operation cannot be evaluated.
pub fn transform_panel_json(spec_json: &str) -> Result<String> {
    let spec: PanelTransformSpec = serde_json::from_str(spec_json)
        .map_err(|err| Error::Validation(format!("invalid panel transform JSON: {err}")))?;
    let result = transform_panel(&spec)?;
    serde_json::to_string(&result)
        .map_err(|err| Error::Internal(format!("failed to serialize panel transform: {err}")))
}

/// Apply a list of named panel transforms from a typed specification.
///
/// Operations run sequentially. Each op reads the previous column by default;
/// set `input` to `"values"` or an earlier operation name to select a source.
///
/// # Arguments
///
/// * `spec` - Typed panel-transform specification whose operations reference
///   row-aligned values and the partition columns they require.
///
/// # Errors
///
/// Returns a validation error when the specification is malformed, an
/// operation name is reserved or duplicated, `input` names an unknown or
/// not-yet-evaluated column, or an operation cannot be evaluated.
pub fn transform_panel(spec: &PanelTransformSpec) -> Result<PanelTransformResult> {
    validate_operation_names(&spec.operations)?;
    let mut columns = Vec::with_capacity(spec.operations.len());
    for operation in &spec.operations {
        let source = resolve_input(spec, &columns, operation)?;
        let output = match operation {
            PanelOperation::Timeseries { op, params, .. } => {
                let entity = spec.entity.as_ref().ok_or_else(|| {
                    Error::Validation(
                        "panel transform entity is required for time-series operations".to_string(),
                    )
                })?;
                let order = spec.order.as_ref().ok_or_else(|| {
                    Error::Validation(
                        "panel transform order is required for time-series operations".to_string(),
                    )
                })?;
                transform_timeseries(source, entity, order, *op, params.as_ref())?
            }
            PanelOperation::CrossSectional { op, params, .. } => {
                let time_key = spec.time_key.as_ref().ok_or_else(|| {
                    Error::Validation(
                        "panel transform time_key is required for cross-sectional operations"
                            .to_string(),
                    )
                })?;
                transform_cross_sectional(source, time_key, *op, params.as_ref())?
            }
        };
        columns.push(PanelTransformColumn {
            name: operation.name().to_string(),
            values: output,
        });
    }
    Ok(PanelTransformResult { columns })
}

fn resolve_input<'a>(
    spec: &'a PanelTransformSpec,
    columns: &'a [PanelTransformColumn],
    operation: &PanelOperation,
) -> Result<&'a [Option<f64>]> {
    let Some(requested) = operation.input() else {
        return Ok(columns
            .last()
            .map_or(spec.values.as_slice(), |column| column.values.as_slice()));
    };
    if requested == "values" {
        return Ok(&spec.values);
    }
    columns
        .iter()
        .find(|column| column.name == requested)
        .map(|column| column.values.as_slice())
        .ok_or_else(|| {
            Error::Validation(format!(
                "panel transform operation '{}' input '{requested}' is unknown",
                operation.name()
            ))
        })
}

fn validate_operation_names(operations: &[PanelOperation]) -> Result<()> {
    let mut names = BTreeSet::new();
    for operation in operations {
        let name = operation.name();
        if name.trim().is_empty() {
            return Err(Error::Validation(
                "panel transform operation name must not be empty".to_string(),
            ));
        }
        if name == "values" {
            return Err(Error::Validation(
                "panel transform operation name must not be the reserved name 'values'".to_string(),
            ));
        }
        if !names.insert(name) {
            return Err(Error::Validation(format!(
                "duplicate panel transform operation name '{name}'"
            )));
        }
    }
    Ok(())
}

/// Specification for a panel transform pipeline.
#[derive(Clone, Debug, Deserialize, Serialize)]
#[cfg_attr(feature = "json-schema", derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields)]
pub struct PanelTransformSpec {
    /// Input numeric value column. `None` represents missing data.
    pub values: Vec<Option<f64>>,
    /// Entity key for time-series operations.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub entity: Option<Vec<String>>,
    /// Lexicographic order key for time-series operations.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub order: Option<Vec<String>>,
    /// Partition key for cross-sectional operations.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub time_key: Option<Vec<String>>,
    /// Ordered operations evaluated sequentially; each reads the previous
    /// column unless `input` selects `values` or an earlier named column.
    pub operations: Vec<PanelOperation>,
}

/// A named panel transform operation.
#[derive(Clone, Debug, Deserialize, Serialize)]
#[cfg_attr(feature = "json-schema", derive(schemars::JsonSchema))]
#[serde(tag = "family", rename_all = "snake_case", deny_unknown_fields)]
pub enum PanelOperation {
    /// Time-series operation evaluated within each entity.
    Timeseries {
        /// Output column name. Must not be the reserved name `values`.
        name: String,
        /// Operation to evaluate.
        op: TimeSeriesOp,
        /// Optional operation parameters.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        params: Option<Value>,
        /// Source column. `None` (default) uses the previous operation output,
        /// or the raw `values` column for the first operation. May name
        /// `values` or an already evaluated column; forward references fail.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        input: Option<String>,
    },
    /// Cross-sectional operation evaluated within each time partition.
    CrossSectional {
        /// Output column name. Must not be the reserved name `values`.
        name: String,
        /// Operation to evaluate.
        op: CrossSectionalOp,
        /// Optional operation parameters.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        params: Option<Value>,
        /// Source column. `None` (default) uses the previous operation output,
        /// or the raw `values` column for the first operation. May name
        /// `values` or an already evaluated column; forward references fail.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        input: Option<String>,
    },
}

impl PanelOperation {
    /// Return the output column name.
    #[must_use]
    pub fn name(&self) -> &str {
        match self {
            Self::Timeseries { name, .. } | Self::CrossSectional { name, .. } => name,
        }
    }

    /// Return the requested source column name, if any.
    fn input(&self) -> Option<&str> {
        match self {
            Self::Timeseries { input, .. } | Self::CrossSectional { input, .. } => input.as_deref(),
        }
    }
}

/// A named output column from a panel transform pipeline.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "json-schema", derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields)]
pub struct PanelTransformColumn {
    /// Output column name.
    pub name: String,
    /// Output values aligned to the input `values` column.
    pub values: Vec<Option<f64>>,
}

/// Ordered result columns from a panel transform pipeline.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "json-schema", derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields)]
pub struct PanelTransformResult {
    /// Output columns in the same order as requested operations.
    pub columns: Vec<PanelTransformColumn>,
}

impl PanelTransformResult {
    /// Look up an output column by name.
    ///
    /// # Arguments
    ///
    /// * `name` - Exact operation output name as supplied in the panel
    ///   specification; lookup is case-sensitive and returns `None` when no
    ///   column matches.
    #[must_use]
    pub fn get_column(&self, name: &str) -> Option<&[Option<f64>]> {
        self.columns
            .iter()
            .find(|column| column.name == name)
            .map(|column| column.values.as_slice())
    }
}
