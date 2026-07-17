//! JSON orchestration for panel transform pipelines.

use crate::{
    transform_cross_sectional_with_op, transform_timeseries_with_op, CrossSectionalOp, TimeSeriesOp,
};
use finstack_quant_core::{Error, Result};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::{BTreeMap, BTreeSet};

/// Apply a list of named panel transforms from a JSON specification.
///
/// # Arguments
///
/// * `spec_json` - UTF-8 JSON document encoding a [`PanelTransformSpec`],
///   including values, required partition columns, and named operations.
///
/// # Errors
///
/// Returns a validation error when the specification is malformed or an
/// operation cannot be evaluated.
pub fn transform_panel(spec_json: &str) -> Result<String> {
    let spec: PanelTransformSpec = serde_json::from_str(spec_json)
        .map_err(|err| Error::Validation(format!("invalid panel transform JSON: {err}")))?;
    let result = transform_panel_spec(&spec)?;
    serde_json::to_string(&LegacyPanelTransformResult::from(&result))
        .map_err(|err| Error::Internal(format!("failed to serialize panel transform: {err}")))
}

/// Apply a list of named panel transforms from a typed specification.
///
/// # Arguments
///
/// * `spec` - Typed panel-transform specification whose operations reference
///   row-aligned values and the partition columns they require.
///
/// # Errors
///
/// Returns a validation error when the specification is malformed or an
/// operation cannot be evaluated.
pub fn transform_panel_spec(spec: &PanelTransformSpec) -> Result<PanelTransformResult> {
    validate_operation_names(&spec.operations)?;
    let mut columns = Vec::with_capacity(spec.operations.len());
    for operation in &spec.operations {
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
                transform_timeseries_with_op(&spec.values, entity, order, *op, params.as_ref())?
            }
            PanelOperation::CrossSectional { op, params, .. } => {
                let time_key = spec.time_key.as_ref().ok_or_else(|| {
                    Error::Validation(
                        "panel transform time_key is required for cross-sectional operations"
                            .to_string(),
                    )
                })?;
                transform_cross_sectional_with_op(&spec.values, time_key, *op, params.as_ref())?
            }
        };
        columns.push(PanelTransformColumn {
            name: operation.name().to_string(),
            values: output,
        });
    }
    Ok(PanelTransformResult { columns })
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
        if !names.insert(name) {
            return Err(Error::Validation(format!(
                "duplicate panel transform operation name '{name}'"
            )));
        }
    }
    Ok(())
}

/// Specification for a panel transform pipeline.
#[derive(Clone, Debug, Deserialize, Serialize, JsonSchema)]
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
    /// Ordered operations to evaluate against `values`.
    pub operations: Vec<PanelOperation>,
}

/// A named panel transform operation.
#[derive(Clone, Debug, Deserialize, Serialize, JsonSchema)]
#[serde(tag = "family", rename_all = "snake_case", deny_unknown_fields)]
pub enum PanelOperation {
    /// Time-series operation evaluated within each entity.
    Timeseries {
        /// Output column name.
        name: String,
        /// Operation to evaluate.
        op: TimeSeriesOp,
        /// Optional operation parameters.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        params: Option<Value>,
    },
    /// Cross-sectional operation evaluated within each time partition.
    CrossSectional {
        /// Output column name.
        name: String,
        /// Operation to evaluate.
        op: CrossSectionalOp,
        /// Optional operation parameters.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        params: Option<Value>,
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
}

/// A named output column from a panel transform pipeline.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct PanelTransformColumn {
    /// Output column name.
    pub name: String,
    /// Output values aligned to the input `values` column.
    pub values: Vec<Option<f64>>,
}

/// Ordered result columns from a panel transform pipeline.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct PanelTransformResult {
    /// Output columns in the same order as requested operations.
    pub columns: Vec<PanelTransformColumn>,
}

impl PanelTransformResult {
    /// Look up an output column by name.
    #[must_use]
    pub fn get_column(&self, name: &str) -> Option<&[Option<f64>]> {
        self.columns
            .iter()
            .find(|column| column.name == name)
            .map(|column| column.values.as_slice())
    }
}

#[derive(Debug, Serialize)]
struct LegacyPanelTransformResult {
    columns: BTreeMap<String, Vec<Option<f64>>>,
}

impl From<&PanelTransformResult> for LegacyPanelTransformResult {
    fn from(result: &PanelTransformResult) -> Self {
        let columns = result
            .columns
            .iter()
            .map(|column| (column.name.clone(), column.values.clone()))
            .collect();
        Self { columns }
    }
}
