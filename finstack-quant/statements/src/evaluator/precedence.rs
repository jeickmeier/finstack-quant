//! Precedence resolution: Value > Forecast > Formula.

use crate::error::{Error, Result};
use crate::types::{NodeSpec, NodeType};
use finstack_quant_core::dates::PeriodId;

/// Resolve the value for a node in a specific period using precedence rules.
///
/// Precedence: Value > Forecast > Formula
///
/// - If explicit value exists for this period (and is visible) → use Value
/// - Else if forecast is applicable → use Forecast
/// - Else if formula exists → use Formula
/// - Else, for a Value or Mixed node → Missing (the cell has no observation),
///   or Hidden when its observation exists but was published after the
///   observation cutoff
/// - A Calculated node without a formula → error
///
/// # Arguments
/// * `node_spec` - Node metadata that includes values, forecast config, and formula text
/// * `period_id` - Period being evaluated
/// * `is_actual_period` - Flag indicating whether the period is classified as actuals (forecasts are skipped)
/// * `hidden` - The node's explicit observation for this period was published
///   after the observation cutoff, so it is not read; a forecast or formula
///   may still supply the period
pub(crate) fn resolve_node_value(
    node_spec: &NodeSpec,
    period_id: &PeriodId,
    is_actual_period: bool,
    hidden: bool,
) -> Result<NodeValueSource> {
    // 1. Check for explicit value (highest precedence)
    if !hidden
        && let Some(values) = &node_spec.values
        && let Some(amount_or_scalar) = values.get(period_id)
    {
        return Ok(NodeValueSource::Value(*amount_or_scalar));
    }

    // 2. Check for forecast (only in forecast periods, not actuals)
    if !is_actual_period && node_spec.forecast.is_some() {
        return Ok(NodeValueSource::Forecast);
    }

    // 3. Check for formula (lowest precedence, always available as fallback)
    if node_spec.formula_text.is_some() {
        return Ok(NodeValueSource::Formula);
    }

    // 4. No resolution method available
    match node_spec.node_type {
        // A missing cell is represented by absence
        // (STATEMENTS-CONTRACTS-001 #3); a hidden one is unavailable.
        NodeType::Value | NodeType::Mixed => Ok(if hidden {
            NodeValueSource::Hidden
        } else {
            NodeValueSource::Missing
        }),
        NodeType::Calculated => Err(Error::eval(format!(
            "Calculated node '{}' has no formula",
            node_spec.node_id
        ))),
    }
}

/// Source of a node's value for a period.
#[derive(Debug, Clone, PartialEq)]
pub enum NodeValueSource {
    /// Explicit observation
    Value(crate::types::AmountOrScalar),

    /// Forecast (to be evaluated by the forecast engine)
    Forecast,

    /// Formula to evaluate (looked up from the compiled cache, not stored here)
    Formula,

    /// No observation, forecast or formula for this period
    Missing,

    /// The observation was published after the cutoff and nothing else
    /// supplies the period
    Hidden,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::AmountOrScalar;
    use indexmap::IndexMap;

    impl NodeValueSource {
        fn is_formula(&self) -> bool {
            matches!(self, Self::Formula)
        }
    }

    #[test]
    fn test_value_precedence() {
        let mut values = IndexMap::new();
        values.insert(
            PeriodId::quarter(2025, 1).expect("valid period fixture"),
            AmountOrScalar::scalar(100.0),
        );

        let node = NodeSpec::new("revenue", NodeType::Mixed)
            .with_values(values)
            .with_formula("revenue * 1.05");

        let source = resolve_node_value(
            &node,
            &PeriodId::quarter(2025, 1).expect("valid period fixture"),
            true,
            false,
        )
        .unwrap();

        // Should use explicit value, not formula
        assert_eq!(
            source,
            NodeValueSource::Value(AmountOrScalar::scalar(100.0))
        );
    }

    #[test]
    fn test_formula_fallback() {
        let node = NodeSpec::new("cogs", NodeType::Calculated).with_formula("revenue * 0.6");

        let source = resolve_node_value(
            &node,
            &PeriodId::quarter(2025, 1).expect("valid period fixture"),
            true,
            false,
        )
        .unwrap();

        // Should use formula
        assert!(source.is_formula());
    }

    #[test]
    fn value_or_mixed_node_without_an_observation_is_missing() {
        for node_type in [NodeType::Value, NodeType::Mixed] {
            let node = NodeSpec::new("revenue", node_type);
            let source = resolve_node_value(
                &node,
                &PeriodId::quarter(2025, 1).expect("valid period fixture"),
                true,
                false,
            )
            .expect("an absent observation is a missing cell, not an error");
            assert_eq!(source, NodeValueSource::Missing);
        }
    }

    #[test]
    fn hidden_observation_without_a_fallback_is_hidden_not_an_error() {
        let period = PeriodId::quarter(2025, 1).expect("valid period fixture");
        let values = IndexMap::from([(period, AmountOrScalar::scalar(100.0))]);
        let value_only = NodeSpec::new("revenue", NodeType::Value).with_values(values.clone());
        assert_eq!(
            resolve_node_value(&value_only, &period, true, true).expect("hidden"),
            NodeValueSource::Hidden
        );
        let with_formula = NodeSpec::new("revenue", NodeType::Mixed)
            .with_values(values)
            .with_formula("123");
        assert!(
            resolve_node_value(&with_formula, &period, true, true)
                .expect("the formula supplies the hidden period")
                .is_formula()
        );
    }

    #[test]
    fn test_forecast_in_forecast_period() {
        use crate::types::ForecastSpec;

        let node = NodeSpec::new("revenue", NodeType::Mixed)
            .with_formula("lag(revenue, 1)")
            .with_forecast(ForecastSpec::growth(0.05));

        // In forecast period, should prefer forecast over formula
        let source = resolve_node_value(
            &node,
            &PeriodId::quarter(2025, 3).expect("valid period fixture"),
            false,
            false,
        )
        .unwrap();
        assert_eq!(source, NodeValueSource::Forecast);
    }

    #[test]
    fn test_formula_in_actual_period() {
        use crate::types::ForecastSpec;

        let node = NodeSpec::new("revenue", NodeType::Mixed)
            .with_formula("lag(revenue, 1)")
            .with_forecast(ForecastSpec::growth(0.05));

        // In actual period, should use formula (not forecast)
        let source = resolve_node_value(
            &node,
            &PeriodId::quarter(2025, 1).expect("valid period fixture"),
            true,
            false,
        )
        .unwrap();
        assert!(source.is_formula());
    }
}
