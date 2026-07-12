//! Validation for metric definitions.

use crate::dsl::parse_and_compile;
use crate::error::{Error, Result};
use crate::registry::schema::MetricDefinition;

/// Validate a metric definition.
///
/// # Arguments
/// * `metric` - Definition to validate
/// * `namespace` - Namespace used for error reporting
///
/// # Validation Rules
/// - ID must be non-empty and contain only `[a-zA-Z0-9_-]`
/// - Name must be non-empty
/// - Formula must be non-empty, parseable, and compilable
///
/// Returns `Ok(())` when the definition passes all checks.
pub fn validate_metric_definition(metric: &MetricDefinition, namespace: &str) -> Result<()> {
    // Validate ID
    if metric.id.is_empty() {
        return Err(Error::registry(
            "Metric ID cannot be empty. Provide a unique identifier (e.g., 'gross_margin').",
        ));
    }

    // Validate ID matches the DSL identifier grammar. Hyphens are intentionally
    // excluded: the formula parser reads `roi-ttm` as the subtraction
    // `roi - ttm`, so a hyphenated metric id would be unreferenceable (and
    // invisible to dependency extraction). Allow only `[A-Za-z_][A-Za-z0-9_]*`.
    let mut chars = metric.id.chars();
    let valid = chars
        .next()
        .is_some_and(|c| c.is_ascii_alphabetic() || c == '_')
        && chars.all(|c| c.is_ascii_alphanumeric() || c == '_');
    if !valid {
        return Err(Error::registry(format!(
            "Invalid metric ID '{}': must start with a letter or underscore and contain only \
             letters, digits, or underscores (matching the DSL identifier grammar). \
             Example valid IDs: 'gross_margin', 'debt_to_equity', 'roi_ttm'",
            metric.id
        )));
    }

    // Validate name
    if metric.name.is_empty() {
        return Err(Error::registry(format!(
            "Metric '{}' has empty name. Provide a human-readable name (e.g., 'Gross Margin %').",
            metric.id
        )));
    }

    // Validate formula
    if metric.formula.trim().is_empty() {
        return Err(Error::registry(format!(
            "Metric '{}' has empty formula. Provide a valid DSL expression (e.g., 'revenue - cogs').",
            metric.id
        )));
    }

    // Validate formula syntax and compilation in one pass.
    parse_and_compile(&metric.formula).map_err(|e| {
        Error::registry(format!(
            "Invalid formula for metric '{}.{}': {}",
            namespace, metric.id, e
        ))
    })?;

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use indexmap::IndexMap;

    fn create_metric(id: &str, name: &str, formula: &str) -> MetricDefinition {
        MetricDefinition {
            id: id.into(),
            name: name.into(),
            formula: formula.into(),
            description: None,
            category: None,
            unit_type: None,
            requires: vec![],
            tags: vec![],
            meta: IndexMap::new(),
        }
    }

    #[test]
    fn test_valid_metric() {
        let metric = create_metric("gross_margin", "Gross Margin", "gross_profit / revenue");
        assert!(validate_metric_definition(&metric, "fin").is_ok());
    }

    #[test]
    fn test_empty_id_error() {
        let metric = create_metric("", "Test", "a + b");
        assert!(validate_metric_definition(&metric, "fin").is_err());
    }

    #[test]
    fn test_empty_name_error() {
        let metric = create_metric("test", "", "a + b");
        assert!(validate_metric_definition(&metric, "fin").is_err());
    }

    #[test]
    fn test_empty_formula_error() {
        let metric = create_metric("test", "Test", "");
        assert!(validate_metric_definition(&metric, "fin").is_err());
    }

    #[test]
    fn test_invalid_formula_error() {
        let metric = create_metric("test", "Test", "a + + b");
        assert!(validate_metric_definition(&metric, "fin").is_err());
    }

    #[test]
    fn test_compile_time_formula_error() {
        let metric = create_metric("test", "Test", "sum()");
        assert!(validate_metric_definition(&metric, "fin").is_err());
    }

    #[test]
    fn test_invalid_id_characters() {
        let metric = create_metric("test.metric", "Test", "a + b");
        assert!(validate_metric_definition(&metric, "fin").is_err());
    }
}
