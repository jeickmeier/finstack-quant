//! Named, atomic edits to a financial model.

use crate::error::{Error, Result};
use crate::types::{AmountOrScalar, FinancialModelSpec, ForecastSpec, NodeId, NodeSpec, NodeType};
use finstack_quant_core::dates::{Date, PeriodId};
use serde::{Deserialize, Serialize};

/// One named change to a [`FinancialModelSpec`].
///
/// Edits are applied together by [`FinancialModelSpec::apply_edits`]: either
/// every edit commits and the edited model passes runnable-model validation,
/// or the model is left unchanged.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "json-schema", derive(schemars::JsonSchema))]
#[serde(tag = "edit", rename_all = "snake_case", deny_unknown_fields)]
pub enum ModelEdit {
    /// Add a new node. The node's id must not already be defined.
    AddNode {
        /// Complete node to add under its own `node_id`.
        node: NodeSpec,
    },
    /// Replace an existing node with a complete new definition.
    ReplaceNode {
        /// Complete replacement, stored in the original node's position.
        node: NodeSpec,
    },
    /// Remove an existing node.
    RemoveNode {
        /// Identifier of the node to remove.
        node_id: NodeId,
    },
    /// Set one explicit observation on a Value or Mixed node.
    SetObservation {
        /// Node that takes the observation.
        node_id: NodeId,
        /// Period of the model timeline the observation belongs to.
        #[cfg_attr(feature = "json-schema", schemars(with = "String"))]
        period: PeriodId,
        /// Finite scalar, or `Money` in the node's currency.
        value: AmountOrScalar,
        /// Date the observation was published. `None` clears a stored date.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        #[cfg_attr(
            feature = "json-schema",
            schemars(with = "Option<finstack_quant_core::wire::DateWire>")
        )]
        availability_date: Option<Date>,
    },
    /// Remove one explicit observation and its availability date, leaving the
    /// cell missing.
    RemoveObservation {
        /// Node that holds the observation.
        node_id: NodeId,
        /// Period whose observation is removed.
        #[cfg_attr(feature = "json-schema", schemars(with = "String"))]
        period: PeriodId,
    },
    /// Set the formula of a Calculated or Mixed node.
    SetFormula {
        /// Node that takes the formula.
        node_id: NodeId,
        /// Statements DSL formula text.
        formula: String,
    },
    /// Attach a forecast, preserving the node's formula and observations. A
    /// Value or Calculated node becomes Mixed.
    SetForecast {
        /// Node that takes the forecast.
        node_id: NodeId,
        /// Forecast method and parameters.
        forecast: ForecastSpec,
    },
    /// Remove a node's forecast. The node stays Mixed.
    RemoveForecast {
        /// Node whose forecast is removed.
        node_id: NodeId,
    },
    /// Attach a `where` predicate to a node.
    SetWhere {
        /// Node the predicate masks.
        node_id: NodeId,
        /// Scalar DSL predicate.
        predicate: String,
    },
    /// Remove a node's `where` predicate.
    RemoveWhere {
        /// Node whose predicate is removed.
        node_id: NodeId,
    },
}

impl FinancialModelSpec {
    /// Apply named edits atomically.
    ///
    /// Edits apply in order to a copy of the model, which then passes
    /// [`validate_semantics`](Self::validate_semantics). Only then does the
    /// copy replace `self`, so a rejected edit leaves the model unchanged.
    ///
    /// # Arguments
    ///
    /// * `edits` - Changes to apply, in order. Later edits see earlier ones.
    ///
    /// # Errors
    ///
    /// Returns a build error when an edit adds an id that is already defined,
    /// names a node or observation that does not exist, or when the edited
    /// model fails runnable-model validation (for example a formula set on a
    /// Value node, an observation outside the timeline, or a reference to a
    /// removed node).
    pub fn apply_edits(&mut self, edits: &[ModelEdit]) -> Result<()> {
        let mut edited = self.clone();
        for edit in edits {
            edited.apply_edit(edit)?;
        }
        edited.validate_semantics()?;
        *self = edited;
        Ok(())
    }

    fn apply_edit(&mut self, edit: &ModelEdit) -> Result<()> {
        match edit {
            ModelEdit::AddNode { node } => {
                if self.nodes.contains_key(&node.node_id) {
                    return Err(Error::build(format!(
                        "Cannot add node '{}': it is already defined",
                        node.node_id
                    )));
                }
                self.nodes.insert(node.node_id.clone(), node.clone());
            }
            ModelEdit::ReplaceNode { node } => {
                *self.existing_node(&node.node_id, "replace")? = node.clone();
            }
            ModelEdit::RemoveNode { node_id } => {
                self.existing_node(node_id, "remove")?;
                self.nodes.shift_remove(node_id);
            }
            ModelEdit::SetObservation {
                node_id,
                period,
                value,
                availability_date,
            } => {
                let node = self.existing_node(node_id, "set an observation on")?;
                node.values
                    .get_or_insert_with(Default::default)
                    .insert(*period, *value);
                match availability_date {
                    Some(date) => node.availability_dates.insert(*period, *date),
                    None => node.availability_dates.shift_remove(period),
                };
            }
            ModelEdit::RemoveObservation { node_id, period } => {
                let node = self.existing_node(node_id, "remove an observation from")?;
                let removed = node
                    .values
                    .as_mut()
                    .and_then(|values| values.shift_remove(period));
                if removed.is_none() {
                    return Err(Error::build(format!(
                        "Node '{node_id}' has no observation for period {period} to remove"
                    )));
                }
                node.availability_dates.shift_remove(period);
            }
            ModelEdit::SetFormula { node_id, formula } => {
                self.existing_node(node_id, "set a formula on")?
                    .formula_text = Some(formula.clone());
            }
            ModelEdit::SetForecast { node_id, forecast } => {
                let node = self.existing_node(node_id, "attach a forecast to")?;
                node.forecast = Some(forecast.clone());
                node.node_type = NodeType::Mixed;
            }
            ModelEdit::RemoveForecast { node_id } => {
                self.existing_node(node_id, "remove a forecast from")?
                    .forecast = None;
            }
            ModelEdit::SetWhere { node_id, predicate } => {
                self.existing_node(node_id, "attach a where clause to")?
                    .where_text = Some(predicate.clone());
            }
            ModelEdit::RemoveWhere { node_id } => {
                self.existing_node(node_id, "remove a where clause from")?
                    .where_text = None;
            }
        }
        Ok(())
    }

    fn existing_node(&mut self, node_id: &NodeId, action: &str) -> Result<&mut NodeSpec> {
        self.nodes
            .get_mut(node_id)
            .ok_or_else(|| Error::build(format!("Cannot {action} unknown node '{node_id}'")))
    }
}
