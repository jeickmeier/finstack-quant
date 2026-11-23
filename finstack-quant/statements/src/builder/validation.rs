//! Node-identifier validation rules shared by model builders.

use crate::error::{Error, Result};

/// DSL keywords that must not be used as node IDs.
///
/// A node named `if`, `and`, `or`, `not`, `true`, or `false` would shadow the
/// language keyword: the parser treats these tokens specially (as control flow
/// or logical operators / boolean literals) and a bare identifier reference
/// cannot reach such a node. Reject at model build time instead of producing
/// confusing downstream parse errors.
///
/// Built-in function names (`lag`, `sum`, `median`, ...) are intentionally not
/// on this list: the DSL disambiguates call sites (`median(x)`) from bare
/// references (`median`), so a node named after a function is legal.
const RESERVED_DSL_IDENTIFIERS: &[&str] = &["if", "and", "or", "not", "true", "false"];

/// Validate that a node ID does not use reserved prefixes or shadow a DSL
/// keyword.
///
/// The `__cs__` prefix is reserved for internal capital structure references,
/// `__` prefix is reserved for other internal use, and identifiers that
/// collide with a DSL keyword (see `RESERVED_DSL_IDENTIFIERS`) would shadow the
/// language primitive. `period_id` is reserved for the timeline column in
/// wide result exports.
///
/// Shared by mutable model construction and complete-specification validation.
/// The same identifier rules apply at both boundaries.
///
/// # Arguments
///
/// * `node_id` - Candidate statement-node identifier to validate against
///   internal prefixes and reserved formula-language keywords.
///
/// # Errors
///
/// Returns an error if `node_id` is not a formula identifier (nonempty
/// dot-separated segments of letters, digits and underscores, starting with a
/// letter or underscore), uses a reserved prefix, is `period_id`, or collides
/// with a DSL keyword.
pub fn validate_node_id(node_id: &str) -> Result<()> {
    if !crate::dsl::signatures::is_valid_identifier(node_id) {
        return Err(Error::build(format!(
            "Node ID '{node_id}' is not a valid identifier: use nonempty dot-separated \
             segments of letters, digits and underscores, starting with a letter or underscore"
        )));
    }
    if node_id == "period_id" {
        return Err(Error::build(
            "Node ID 'period_id' is reserved for the timeline column in result exports",
        ));
    }
    if node_id.contains("__cs__") {
        return Err(Error::build(format!(
            "Node ID '{}' contains reserved prefix '__cs__'. \
             This prefix is used internally for capital structure references.",
            node_id
        )));
    }
    if node_id.starts_with("__") {
        return Err(Error::build(format!(
            "Node ID '{}' cannot start with '__' (reserved for internal use)",
            node_id
        )));
    }
    if RESERVED_DSL_IDENTIFIERS.contains(&node_id) {
        return Err(Error::build(format!(
            "Node ID '{}' collides with a reserved DSL keyword or built-in function; \
             rename the node to avoid shadowing formula primitives.",
            node_id
        )));
    }
    Ok(())
}
