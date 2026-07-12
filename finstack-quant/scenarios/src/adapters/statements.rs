//! Statement shock and rate binding adapters.

use crate::adapters::traits::ScenarioEffect;
use crate::error::{Error, Result};
use crate::spec::{Compounding, RateBindingSpec};
use crate::utils::tenor_years_from_binding;
use crate::warning::Warning;
use finstack_quant_core::dates::{BusinessDayConvention, HolidayCalendar, Tenor};
use finstack_quant_core::market_data::context::MarketContext;
use finstack_quant_core::math::Compounding as CoreCompounding;
use finstack_quant_statements::evaluator::Evaluator;
use finstack_quant_statements::types::{AmountOrScalar, NodeId};
use finstack_quant_statements::FinancialModelSpec;

/// Generate effect for a forecast-percent statement op.
pub(crate) fn stmt_forecast_percent_effects(node_id: &NodeId, pct: f64) -> Vec<ScenarioEffect> {
    vec![ScenarioEffect::StmtForecastPercent {
        node_id: node_id.clone(),
        pct,
    }]
}

/// Generate effect for a forecast-assign statement op.
pub(crate) fn stmt_forecast_assign_effects(node_id: &NodeId, value: f64) -> Vec<ScenarioEffect> {
    vec![ScenarioEffect::StmtForecastAssign {
        node_id: node_id.clone(),
        value,
    }]
}

/// Generate effect for a rate-binding op.
pub(crate) fn rate_binding_effects(binding: &RateBindingSpec) -> Vec<ScenarioEffect> {
    vec![ScenarioEffect::RateBinding {
        binding: binding.clone(),
    }]
}

fn with_node_values_mut<F>(model: &mut FinancialModelSpec, node_id: &str, mut f: F) -> Result<bool>
where
    F: FnMut(&mut AmountOrScalar),
{
    let node = model
        .get_node_mut(node_id)
        .ok_or_else(|| Error::NodeNotFound {
            node_id: node_id.to_string(),
        })?;

    match node.values.as_mut() {
        Some(values) => {
            for val in values.values_mut() {
                f(val);
            }
            Ok(true)
        }
        None => Ok(false),
    }
}

/// Apply a percentage change to a statement node's explicit forecast values.
pub fn apply_forecast_percent(
    model: &mut FinancialModelSpec,
    node_id: &str,
    pct: f64,
) -> Result<bool> {
    let factor = 1.0 + (pct / 100.0);

    with_node_values_mut(model, node_id, |val| match val {
        AmountOrScalar::Scalar(s) => *s *= factor,
        AmountOrScalar::Amount(money) => {
            *money =
                finstack_quant_core::money::Money::new(money.amount() * factor, money.currency());
        }
    })
}

/// Assign a scalar value to explicit forecasts in a node, optionally filtering periods.
pub fn apply_forecast_assign(
    model: &mut FinancialModelSpec,
    node_id: &str,
    value: f64,
    period_filter: Option<(
        finstack_quant_core::dates::Date,
        finstack_quant_core::dates::Date,
    )>,
) -> Result<bool> {
    let allowed_period_ids = period_filter.as_ref().map(|(start, end)| {
        model
            .periods
            .iter()
            .filter(|period| period.start >= *start && period.end <= *end)
            .map(|period| period.id)
            .collect::<std::collections::HashSet<_>>()
    });

    let node = model
        .get_node_mut(node_id)
        .ok_or_else(|| Error::NodeNotFound {
            node_id: node_id.to_string(),
        })?;

    match node.values.as_mut() {
        Some(values) => {
            for (period_id, val) in values.iter_mut() {
                if allowed_period_ids
                    .as_ref()
                    .is_none_or(|allowed| allowed.contains(period_id))
                {
                    *val = AmountOrScalar::Scalar(value);
                }
            }
            Ok(true)
        }
        None => Ok(false),
    }
}

/// Update a statement rate node using a full [`RateBindingSpec`].
///
/// # Curve lookup order
///
/// The binding's `curve_id` is resolved against the **discount** collection
/// first, then the **forward** collection. If the same identifier exists in
/// both, the discount curve wins and the forward curve is never consulted —
/// use distinct identifiers per collection when both must be addressable.
pub fn update_rate_from_binding(
    binding: &RateBindingSpec,
    model: &mut FinancialModelSpec,
    market: &MarketContext,
    calendar: Option<&dyn HolidayCalendar>,
) -> Result<bool> {
    let curve_id = binding.curve_id.as_str();

    if let Ok(curve) = market.get_discount(curve_id) {
        let (tenor_years, _) = tenor_years_from_binding(
            binding,
            curve.base_date(),
            curve.day_count(),
            calendar,
            BusinessDayConvention::ModifiedFollowing,
        )?;

        if let Some(&max_t) = curve.knots().last() {
            if tenor_years > max_t + 1e-8 {
                return Err(Error::Validation(format!(
                    "Tenor {} ({tenor_years:.4}y) is out of range for discount curve {curve_id} (max {max_t:.4}y)",
                    binding.tenor
                )));
            }
        }

        let zero = curve.zero(tenor_years);
        let converted = convert_continuous_rate(zero, binding.compounding, tenor_years)?;
        return apply_forecast_assign(model, binding.node_id.as_str(), converted, None);
    }

    if let Ok(curve) = market.get_forward(curve_id) {
        let (start_years, effective_dc) = tenor_years_from_binding(
            binding,
            curve.base_date(),
            curve.day_count(),
            calendar,
            BusinessDayConvention::ModifiedFollowing,
        )?;

        if let Some(&max_t) = curve.knots().last() {
            if start_years > max_t + 1e-8 {
                return Err(Error::Validation(format!(
                    "Tenor {} ({start_years:.4}y) is out of range for forward curve {curve_id} (max {max_t:.4}y)",
                    binding.tenor
                )));
            }
        }

        let forward_start = Tenor::parse(&binding.tenor)
            .map_err(|e| Error::InvalidTenor(e.to_string()))?
            .add_to_date(
                curve.base_date(),
                calendar,
                BusinessDayConvention::ModifiedFollowing,
            )?;

        let accrual_years = Tenor::from_years(curve.tenor(), effective_dc)?.to_years_with_context(
            forward_start,
            calendar,
            BusinessDayConvention::ModifiedFollowing,
            effective_dc,
        )?;
        if !accrual_years.is_finite() || accrual_years <= 0.0 {
            return Err(Error::Validation(format!(
                "Forward curve '{curve_id}' has non-positive accrual period ({accrual_years:.6}y); \
                 cannot convert simple forward rate"
            )));
        }

        let forward_simple = curve.rate(start_years);
        let converted = if matches!(binding.compounding, Compounding::Simple) {
            forward_simple
        } else {
            let forward_continuous = CoreCompounding::Simple.convert_rate(
                forward_simple,
                accrual_years,
                &CoreCompounding::Continuous,
            );
            convert_continuous_rate(forward_continuous, binding.compounding, accrual_years)?
        };
        return apply_forecast_assign(model, binding.node_id.as_str(), converted, None);
    }

    Err(Error::MarketDataNotFound {
        id: curve_id.to_string(),
    })
}

fn convert_continuous_rate(
    continuous_rate: f64,
    comp: Compounding,
    year_fraction: f64,
) -> Result<f64> {
    if !year_fraction.is_finite() || year_fraction <= 0.0 {
        return Err(Error::Validation(format!(
            "Year fraction must be positive for rate conversion, got {year_fraction}"
        )));
    }

    let to: CoreCompounding = match comp {
        Compounding::Continuous => return Ok(continuous_rate),
        Compounding::Simple => CoreCompounding::Simple,
        Compounding::Annual => CoreCompounding::Annual,
        Compounding::SemiAnnual => CoreCompounding::SEMI_ANNUAL,
        Compounding::Quarterly => CoreCompounding::QUARTERLY,
        Compounding::Monthly => CoreCompounding::MONTHLY,
    };

    Ok(CoreCompounding::Continuous.convert_rate(continuous_rate, year_fraction, &to))
}

/// Re-evaluate the financial model to propagate scenario changes.
///
/// Returns structured [`Warning`]s for any evaluator notes encountered.
pub fn reevaluate_model(model: &mut FinancialModelSpec) -> Result<Vec<Warning>> {
    let mut evaluator = Evaluator::new();
    let results = evaluator.evaluate(model)?;
    let warnings: Vec<Warning> = results
        .meta
        .warnings
        .iter()
        .map(|w| Warning::ModelEvaluation {
            detail: format!("{w:?}"),
        })
        .collect();
    Ok(warnings)
}

#[cfg(test)]
mod tests {
    use super::*;
    use finstack_quant_core::dates::build_periods;
    use finstack_quant_statements::types::{NodeSpec, NodeType};
    use indexmap::IndexMap;

    #[test]
    fn test_apply_forecast_assign_updates_only_selected_periods() {
        let period_plan = build_periods("2025Q1..Q4", None).expect("periods should build");
        let periods = period_plan.periods;
        let mut model = FinancialModelSpec::new("test", periods.clone());

        let mut values = IndexMap::new();
        for (i, period) in periods.iter().enumerate() {
            values.insert(period.id, AmountOrScalar::Scalar(100.0 * (i as f64 + 1.0)));
        }

        model.add_node(NodeSpec::new("Revenue", NodeType::Value).with_values(values));

        apply_forecast_assign(
            &mut model,
            "Revenue",
            500.0,
            Some((periods[1].start, periods[1].end)),
        )
        .expect("filtered assign should succeed");

        let shocked_values: Vec<f64> = model
            .get_node("Revenue")
            .expect("node should exist")
            .values
            .as_ref()
            .expect("values should exist")
            .values()
            .map(|v| match v {
                AmountOrScalar::Scalar(s) => *s,
                AmountOrScalar::Amount(_) => 0.0,
            })
            .collect();

        assert_eq!(shocked_values, vec![100.0, 500.0, 300.0, 400.0]);
    }
}
