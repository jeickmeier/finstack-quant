//! Shared helpers for statements-analytics template bindings.
//!
//! Template bindings rebuild a Rust [`ModelBuilder`] from a serialized
//! [`FinancialModelSpec`], apply the template, then restore `meta` and
//! `capital_structure` on the rebuilt spec.

use crate::errors::display_to_py;
use finstack_quant_statements::builder::{ModelBuilder, Ready};
use finstack_quant_statements::types::{CapitalStructureSpec, FinancialModelSpec};
use indexmap::IndexMap;
use pyo3::prelude::*;
use serde_json::Value;

/// Rebuilt model builder with preserved metadata.
pub(crate) type RebuiltBuilder = (
    ModelBuilder<Ready>,
    IndexMap<String, Value>,
    Option<CapitalStructureSpec>,
);

/// Reconstruct a ready [`ModelBuilder`] from a serialized model spec.
pub(crate) fn rebuild_builder(spec: FinancialModelSpec) -> PyResult<RebuiltBuilder> {
    let meta = spec.meta.clone();
    let capital_structure = spec.capital_structure.clone();
    let id = spec.id.clone();
    let periods = spec.periods.clone();
    let nodes = spec.nodes;

    let mut builder = ModelBuilder::new(id)
        .periods_explicit(periods)
        .map_err(display_to_py)?;
    for (node_id, node_spec) in nodes {
        builder.insert_node(node_id, node_spec);
    }
    Ok((builder, meta, capital_structure))
}

/// Build a spec from a template-transformed builder, restoring metadata.
pub(crate) fn finalize_spec(
    builder: ModelBuilder<Ready>,
    meta: IndexMap<String, Value>,
    capital_structure: Option<CapitalStructureSpec>,
) -> PyResult<FinancialModelSpec> {
    let mut new_spec = builder.build().map_err(display_to_py)?;
    new_spec.meta = meta;
    new_spec.capital_structure = capital_structure;
    Ok(new_spec)
}

/// Serialize the finalized spec to JSON.
pub(crate) fn finalize_json(
    builder: ModelBuilder<Ready>,
    meta: IndexMap<String, Value>,
    capital_structure: Option<CapitalStructureSpec>,
) -> PyResult<String> {
    let spec = finalize_spec(builder, meta, capital_structure)?;
    serde_json::to_string(&spec).map_err(display_to_py)
}
