//! Roll-forward pattern implementation.

use super::fmt_f64;
use finstack_quant_statements::builder::ModelBuilder;
use finstack_quant_statements::types::{NodeSpec, NodeType};

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
/// Duplicate node identifiers and invalid formulas or references are rejected
/// when the completed model is built.
pub fn add_roll_forward(
    builder: &mut ModelBuilder,
    name: &str,
    increases: &[&str],
    decreases: &[&str],
) {
    add_roll_forward_with_opening(builder, name, increases, decreases, 0.0);
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
/// * `opening` - First-period beginning balance, in the same units as the
///   increase and decrease flows.
///
/// Duplicate node identifiers and invalid formulas or references are rejected
/// when the completed model is built.
pub fn add_roll_forward_with_opening(
    builder: &mut ModelBuilder,
    name: &str,
    increases: &[&str],
    decreases: &[&str],
    opening: f64,
) {
    let beg_node_id = format!("{}_beg", name);
    let end_node_id = format!("{}_end", name);

    // coalesce handles the first period (defaults to `opening` if no
    // history). `{}` formatting is shortest-roundtrip, so the opening value
    // survives the formula round-trip at full f64 precision.
    let beg_formula = format!("coalesce(lag({}, 1), {})", end_node_id, fmt_f64(opening));
    let beg_node = NodeSpec::new(beg_node_id.as_str(), NodeType::Calculated)
        .with_name(format!("{} (Beginning)", name))
        .with_formula(beg_formula);

    let mut end_formula = beg_node_id;

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

    builder.insert_node(beg_node);
    builder.insert_node(end_node);
}
