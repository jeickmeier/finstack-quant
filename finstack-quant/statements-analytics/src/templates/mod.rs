//! Financial statement modeling templates.
//!
//! Each template lives in its own subdirectory. To add a new template, create a
//! directory under `templates/` with a `mod.rs` and register it here. Templates
//! are free functions that take and return a
//! [`ModelBuilder`](finstack_quant_statements::builder::ModelBuilder).
//!
//! - [`crate::templates::roll_forward`] — beginning + changes = ending balance pattern
//! - [`crate::templates::real_estate`] — NOI/NCF/rent-roll/property operating statement builders
//! - [`crate::templates::vintage`] — cohort/vintage buildup via convolution
//!
//! For property modeling, [`crate::templates::real_estate`] provides the richest public surface:
//! rent-roll, NOI, EGI, management-fee, and NCF builders that generate
//! statement nodes using consistent naming conventions.
//!
//! # Build-time vs Runtime
//!
//! These templates are **build-time** helpers that extend
//! [`ModelBuilder`](finstack_quant_statements::builder::ModelBuilder)
//! to create properly connected node structures. For **runtime validation** of these
//! structures after evaluation, see [`crate::extensions::CorkscrewExtension`].
//!
//! | Template | Build-time | Runtime Validation |
//! |----------|------------|-------------------|
//! | Roll-forward | [`crate::templates::roll_forward::add_roll_forward`] | [`crate::extensions::CorkscrewExtension`] |
//! | Vintage | [`crate::templates::vintage::add_vintage_buildup`] | N/A |
//! | Real estate | [`crate::templates::real_estate::add_property_operating_statement`] | Model-specific |
//!
//! ## Conventions
//!
//! - Template helpers mutate the model graph at build time; they do not add
//!   bespoke runtime behavior.
//! - Real-estate template amounts are expressed per model period, not annualized,
//!   unless a specific struct field states otherwise.
//! - Generated node ids are intended to be stable and report-friendly, so callers
//!   should pass explicit node names when integrating with reporting layers.
//! - Roll-forward `{name}` pairs with corkscrew as `node_id = "{name}_end"`,
//!   `beginning_balance_node = "{name}_beg"`, `changes = increases`,
//!   `decreases = disposals`. Decrease nodes stay positive; corkscrew subtracts
//!   them (`expected = beginning + Σ changes − Σ decreases`).
//! - Vintage `decay_curve[k]` is indexed in **model periods**, not calendar
//!   years. On a quarterly model, `k = 1` is the next quarter.
//!
//! # Example
//!
//! ```
//! use finstack_quant_statements::prelude::*;
//! use finstack_quant_statements_analytics::templates::roll_forward::add_roll_forward;
//!
//! # fn main() -> std::result::Result<(), Box<dyn std::error::Error>> {
//! # let values: &[(PeriodId, AmountOrScalar)] = &[];
//! let builder = ModelBuilder::new("demo")
//!     .periods("2025Q1..2025Q4", None)?
//!     .value("additions", values)
//!     .value("disposals", values);
//! let model = add_roll_forward(builder, "inventory", &["additions"], &["disposals"]).build()?;
//! # let _ = model;
//! # Ok(())
//! # }
//! ```

pub mod real_estate;
pub mod roll_forward;
pub mod vintage;

/// Format an `f64` for embedding in a generated formula at full
/// (shortest-roundtrip) precision, ensuring the literal still looks like a
/// float (e.g. `5` → `"5.0"`).
pub(crate) fn fmt_f64(value: f64) -> String {
    let s = format!("{}", value);
    if s.contains('.') || s.contains('e') || s.contains('E') {
        s
    } else {
        format!("{s}.0")
    }
}
