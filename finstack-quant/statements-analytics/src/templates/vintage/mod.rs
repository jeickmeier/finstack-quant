//! Vintage/Cohort pattern implementation.

use finstack_quant_core::math::ZERO_TOLERANCE;
use finstack_quant_statements::builder::{ModelBuilder, Ready};
use finstack_quant_statements::error::Result;
use finstack_quant_statements::types::{NodeId, NodeSpec, NodeType};

/// Add a vintage buildup (cohort analysis) structure.
///
/// This models a "stack" of layers (cohorts) where each layer is generated
/// by a "new volume" node and then decays/evolves according to a curve.
///
/// The total value is the sum of all active cohorts:
/// `Total[t] = Sum(NewVolume[t-k] * Curve[k])` for k = 0..curve_len
///
/// # Arguments
/// * `builder` - Model builder (must be Ready state to access periods)
/// * `name` - Name of the resulting total node (e.g., "revenue")
/// * `new_volume_node` - Node ID for the new volume per period (e.g., "new_sales")
/// * `decay_curve` - Multipliers for the vintage curve (index 0 = inception, 1 = next period, etc.)
///
/// # Errors
///
/// Propagates model-builder errors for invalid or duplicate node IDs, an
/// unavailable new-volume node, or generated convolution formulas.
pub fn add_vintage_buildup(
    mut builder: ModelBuilder<Ready>,
    name: &str,
    new_volume_node: &str,
    decay_curve: &[f64],
) -> Result<ModelBuilder<Ready>> {
    // We construct the total node using a convolution formula:
    // Total = New * c0 + lag(New, 1) * c1 + lag(New, 2) * c2 + ...

    let mut terms = Vec::new();

    for (lag, &rate) in decay_curve.iter().enumerate() {
        // Skip zero rates to keep formula clean
        if rate.abs() < ZERO_TOLERANCE {
            continue;
        }

        // Coefficients are embedded at full shortest-roundtrip precision
        // (`{}` formatting) — fixed-precision truncation (e.g. `{:.6}`) would
        // silently distort long-tail decay curves.
        let term = if lag == 0 {
            // Current period term: New * c0
            format!("{} * {}", new_volume_node, super::fmt_f64(rate))
        } else {
            // Lagged term: lag(New, k) * ck
            // We use coalesce(lag(...), 0) to handle boundaries gracefully
            format!(
                "coalesce(lag({}, {}), 0.0) * {}",
                new_volume_node,
                lag,
                super::fmt_f64(rate)
            )
        };

        terms.push(term);
    }

    // If curve is empty or all zeros, result is 0
    let formula = if terms.is_empty() {
        "0.0".to_string()
    } else {
        terms.join(" + ")
    };

    let node = NodeSpec::new(name, NodeType::Calculated)
        .with_name(format!("{} (Total)", name))
        .with_formula(formula);

    builder.insert_node(NodeId::from(name), node);

    Ok(builder)
}
