//! Roll-forward pattern implementation.

use super::fmt_f64;
use finstack_quant_statements::builder::ModelBuilder;
use finstack_quant_statements::error::Result;
use finstack_quant_statements::types::{NodeId, NodeSpec, NodeType};

/// Add a roll-forward structure to the model with a zero opening balance.
///
/// Equivalent to [`add_roll_forward_with_opening`] with `opening = 0.0`
/// (the first period opens at zero).
///
/// # Arguments
/// * `builder` - Typed model builder to which beginning- and ending-balance
///   nodes and their formulas are appended.
/// * `name` - Base name for the roll-forward (e.g., "arr")
/// * `increases` - List of node IDs that increase the balance
/// * `decreases` - List of node IDs that decrease the balance
///
/// # Errors
///
/// Propagates model-builder errors while creating the beginning and ending
/// balance nodes and their formulas.
pub fn add_roll_forward<State>(
    builder: ModelBuilder<State>,
    name: &str,
    increases: &[&str],
    decreases: &[&str],
) -> Result<ModelBuilder<State>> {
    add_roll_forward_with_opening(builder, name, increases, decreases, 0.0)
}

/// Add a roll-forward structure to the model with an explicit opening balance.
///
/// This creates:
/// - `{name}_beg`: Beginning balance (linked to previous period's ending
///   balance; `opening` in the first period)
/// - `{name}_end`: Ending balance (Begin + Increases - Decreases)
///
/// # Arguments
/// * `builder` - Typed model builder to which beginning- and ending-balance
///   nodes and their formulas are appended.
/// * `name` - Base name for the roll-forward (e.g., "arr")
/// * `increases` - List of node IDs that increase the balance
/// * `decreases` - List of node IDs that decrease the balance
/// * `opening` - Opening balance used in the first period (no prior ending
///   balance exists); emitted as `coalesce(lag({name}_end, 1), {opening})`
///
/// # Errors
///
/// Propagates model-builder errors for invalid/duplicate node IDs, referenced
/// input nodes, or generated formulas.
pub fn add_roll_forward_with_opening<State>(
    mut builder: ModelBuilder<State>,
    name: &str,
    increases: &[&str],
    decreases: &[&str],
    opening: f64,
) -> Result<ModelBuilder<State>> {
    let beg_node_id = format!("{}_beg", name);
    let end_node_id = format!("{}_end", name);

    // 1. Create Beginning Balance Node
    // Formula: lag(end_node, 1)
    // Use coalesce to handle the first period (defaults to `opening` if no
    // history). `{}` formatting is shortest-roundtrip, so the opening value
    // survives the formula round-trip at full f64 precision.
    let beg_formula = format!("coalesce(lag({}, 1), {})", end_node_id, fmt_f64(opening));
    let beg_node = NodeSpec::new(beg_node_id.as_str(), NodeType::Calculated)
        .with_name(format!("{} (Beginning)", name))
        .with_formula(beg_formula);

    // 2. Create Ending Balance Node
    // Formula: beg + sum(increases) - sum(decreases)
    let mut end_formula = beg_node_id.clone();

    if !increases.is_empty() {
        end_formula.push_str(" + ");
        end_formula.push_str(&increases.join(" + "));
    }

    if !decreases.is_empty() {
        end_formula.push_str(" - (");
        end_formula.push_str(&decreases.join(" + "));
        end_formula.push(')');
    }

    let end_node = NodeSpec::new(end_node_id.as_str(), NodeType::Calculated)
        .with_name(format!("{} (Ending)", name))
        .with_formula(end_formula);

    // 3. Add nodes to builder
    builder.insert_node(NodeId::from(beg_node_id), beg_node);
    builder.insert_node(NodeId::from(end_node_id), end_node);

    Ok(builder)
}
