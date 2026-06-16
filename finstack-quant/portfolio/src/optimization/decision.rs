use super::problem::PortfolioOptimizationProblem;
use super::tolerances::{GROSS_BASE_TOL, MIN_WEIGHT_TOL, PV_PER_UNIT_TOL};
use super::types::{MissingMetricPolicy, WeightingScheme};
use crate::error::{Error, Result};
use crate::types::{AttributeValue, EntityId, PositionId};
use finstack_quant_core::config::FinstackConfig;
use finstack_quant_core::market_data::context::MarketContext;
use finstack_quant_valuations::metrics::MetricId;
use indexmap::IndexMap;

/// Result of building the decision space: items, features, current weights, and total PV.
type DecisionSpaceResult = (
    Vec<DecisionItem>,
    Vec<DecisionFeatures>,
    IndexMap<PositionId, f64>,
    OptimizationDenominators,
);

#[derive(Clone, Copy, Debug)]
pub(crate) struct OptimizationDenominators {
    pub gross_pv_base: f64,
    pub gross_notional: f64,
}

/// Internal representation of a decision variable.
#[derive(Clone, Debug)]
pub(crate) struct DecisionItem {
    pub position_id: PositionId,
    pub entity_id: EntityId,
    /// Whether this represents an existing portfolio position.
    pub is_existing: bool,
    /// Whether this position is held (weight locked to current).
    pub is_held: bool,
    /// Current quantity held (if any).
    ///
    /// This is only used when reconstructing `WeightingScheme::UnitScaling`
    /// solutions, where the optimized weight is a dimensionless multiplier on
    /// the live quantity rather than a PV share.
    pub current_quantity: f64,
}

/// Per‑decision‑variable features used to build linear forms.
#[derive(Clone, Debug)]
pub(crate) struct DecisionFeatures {
    /// Current base‑currency PV (scaled by quantity; 0 for candidates).
    pub pv_base: f64,
    /// Current native-currency PV (scaled by quantity; 0 for candidates).
    pub pv_native: f64,
    /// Base‑currency PV per unit of quantity (for implied quantities).
    pub pv_per_unit: f64,
    /// Metric measures by string key (as in `ValuationResult::measures`).
    pub measures: IndexMap<String, f64>,
    /// Attributes used by attribute‑based constraints and filters.
    pub attributes: IndexMap<String, AttributeValue>,
    /// Minimum weight bound.
    pub min_weight: f64,
    /// Maximum weight bound.
    pub max_weight: f64,
}

fn metrics_to_strings(
    metrics: &IndexMap<finstack_quant_valuations::metrics::MetricId, f64>,
) -> IndexMap<String, f64> {
    metrics
        .iter()
        .map(|(id, v)| (id.as_str().to_string(), *v))
        .collect()
}

fn is_missing_required_metrics(
    measures: &IndexMap<String, f64>,
    required_metrics: &[MetricId],
) -> bool {
    required_metrics
        .iter()
        .any(|metric| !measures.contains_key(metric.as_str()))
}

/// Build decision items and associated features from the portfolio and trade universe.
#[allow(clippy::too_many_arguments)]
pub(crate) fn build_decision_space(
    problem: &PortfolioOptimizationProblem,
    valuation: &crate::valuation::PortfolioValuation,
    required_metrics: &[MetricId],
    market: &MarketContext,
    config: &FinstackConfig,
) -> Result<DecisionSpaceResult> {
    let mut items = Vec::new();
    let mut features = Vec::new();
    let mut current_weights = IndexMap::new();

    // Gross market value: sum of absolute PVs (for weight calculation with hedged portfolios)
    let mut gross_pv_base = 0.0_f64;
    // Track notional values for NotionalWeight scheme
    let mut position_notionals: IndexMap<PositionId, f64> = IndexMap::new();
    let mut gross_notional = 0.0_f64;

    for position in &problem.portfolio.positions {
        // Pull valuation for this position
        let pv_entry = valuation
            .position_values
            .get(&position.position_id)
            .ok_or_else(|| {
                Error::invalid_input(format!(
                    "missing valuation for position '{}'",
                    position.position_id
                ))
            })?;

        let pv_base = pv_entry.value_base.amount();
        let pv_native = pv_entry.value_native.amount();
        gross_pv_base += pv_base.abs();
        // For NotionalWeight: use the same unit-aware scale factor that
        // portfolio valuation and factor risk use, so Percentage positions
        // contribute 50% as 0.5 instead of raw 50.0.
        let notional_proxy = position.scale_factor();
        position_notionals.insert(position.position_id.clone(), notional_proxy);
        gross_notional += notional_proxy.abs();

        // Extract measures
        let mut measures = IndexMap::new();
        if let Some(val_result) = &pv_entry.valuation_result {
            measures = metrics_to_strings(&val_result.measures);
        } else if !required_metrics.is_empty()
            && matches!(problem.missing_metric_policy, MissingMetricPolicy::Strict)
        {
            return Err(Error::valuation(
                position.position_id.clone(),
                "valuation result missing required metrics",
            ));
        }

        let missing_required_metrics = !required_metrics.is_empty()
            && is_missing_required_metrics(&measures, required_metrics);

        // Decide if position is held / tradeable
        let explicit_hold = if let Some(ref held) = problem.trade_universe.held_filter {
            held.matches(
                &position.entity_id,
                &position.position_id,
                &position.attributes,
            )
        } else {
            false
        };

        let is_tradeable = problem.trade_universe.tradeable_filter.matches(
            &position.entity_id,
            &position.position_id,
            &position.attributes,
        );
        let is_excluded = !is_tradeable && !explicit_hold;
        let freeze_for_missing_metrics = missing_required_metrics
            && matches!(problem.missing_metric_policy, MissingMetricPolicy::Exclude);
        let is_held = explicit_hold || is_excluded || freeze_for_missing_metrics;

        let pv_per_unit = if position.quantity != 0.0 {
            pv_base / position.quantity
        } else {
            0.0
        };
        if matches!(problem.weighting, WeightingScheme::ValueWeight)
            && !is_held
            && position.quantity.abs() > PV_PER_UNIT_TOL
            && pv_per_unit.abs() < PV_PER_UNIT_TOL
        {
            return Err(Error::invalid_input(format!(
                "MO-8: existing position {} has zero base-currency PV per unit under ValueWeight; \
                 cannot reconstruct a non-zero optimized quantity",
                position.position_id
            )));
        }

        items.push(DecisionItem {
            position_id: position.position_id.clone(),
            entity_id: position.entity_id.clone(),
            is_existing: true,
            is_held,
            current_quantity: position.quantity,
        });

        // M-7: Preserve the sign of existing positions by default. Longs remain
        // long-only, shorts remain short-only; explicit WeightBounds constraints
        // can further tighten these ranges.
        let (min_weight, max_weight) = if pv_base < 0.0 {
            (-1.0, 0.0)
        } else {
            (0.0, 1.0)
        };
        features.push(DecisionFeatures {
            pv_base,
            pv_native,
            // When quantity == 0, treat pv_per_unit as 0 to avoid division by zero.
            pv_per_unit,
            measures,
            attributes: position.attributes.clone(),
            min_weight,
            max_weight,
        });
    }

    // Batch price candidates using a temporary portfolio
    let candidate_valuation = if !problem.trade_universe.candidates.is_empty() {
        let mut builder = crate::builder::PortfolioBuilder::new("CANDIDATES")
            .base_ccy(problem.portfolio.base_ccy)
            .as_of(problem.portfolio.as_of);

        // Use a dummy entity for all candidates
        builder = builder.entity(crate::types::Entity::new("CANDIDATE_POOL"));

        for candidate in &problem.trade_universe.candidates {
            let mut pos = crate::position::Position::new(
                candidate.id.clone(),
                "CANDIDATE_POOL",
                candidate.instrument.id(),
                std::sync::Arc::clone(&candidate.instrument),
                1.0, // Quantity 1.0 for unit pricing
                candidate.unit,
            )
            .map_err(|e| Error::invalid_input(e.to_string()))?;

            // Copy attributes directly from candidate
            for (k, v) in &candidate.attributes {
                pos.attributes.insert(k.clone(), v.clone());
            }

            builder = builder.position(pos);
        }

        let candidate_portfolio = builder
            .build()
            .map_err(|e| Error::invalid_input(e.to_string()))?;

        let options = crate::valuation::PortfolioValuationOptions {
            strict_risk: matches!(problem.missing_metric_policy, MissingMetricPolicy::Strict),
            metrics: if required_metrics.is_empty() {
                crate::valuation::RequestedMetrics::Standard
            } else {
                crate::valuation::RequestedMetrics::StandardPlus(required_metrics.to_vec())
            },
        };

        Some(crate::valuation::value_portfolio(
            &candidate_portfolio,
            market,
            config,
            &options,
        )?)
    } else {
        None
    };

    for candidate in &problem.trade_universe.candidates {
        let val_entry = candidate_valuation
            .as_ref()
            .and_then(|v| v.position_values.get(&candidate.id))
            .ok_or_else(|| Error::valuation(candidate.id.clone(), "failed to value candidate"))?;

        let pv_unit = val_entry.value_base.amount();
        // ValueWeight reconstructs implied quantities as
        // `(w_star * gross_pv_base) / pv_per_unit`. A zero `pv_unit`
        // candidate would silently collapse to zero quantity for any
        // non-zero target weight, producing a meaningless trade. Reject
        // up-front so the caller can either re-price or remove the
        // candidate rather than discovering the no-op after solving.
        if matches!(problem.weighting, WeightingScheme::ValueWeight)
            && pv_unit.abs() < PV_PER_UNIT_TOL
            && candidate.max_weight.abs() > PV_PER_UNIT_TOL
        {
            return Err(Error::invalid_input(format!(
                "candidate {} has zero base-currency PV under the current market; \
                 ValueWeight optimisation cannot reconstruct a quantity from a target weight. \
                 Remove the candidate, fix its market data, or switch weighting scheme.",
                candidate.id
            )));
        }

        let measures = val_entry
            .valuation_result
            .as_ref()
            .map(|r| metrics_to_strings(&r.measures))
            .unwrap_or_default();
        let missing_required_metrics = !required_metrics.is_empty()
            && is_missing_required_metrics(&measures, required_metrics);

        items.push(DecisionItem {
            position_id: candidate.id.clone(),
            entity_id: candidate.entity_id.clone(),
            is_existing: false,
            is_held: missing_required_metrics
                && matches!(problem.missing_metric_policy, MissingMetricPolicy::Exclude),
            current_quantity: 0.0,
        });

        let candidate_min_weight = if problem.trade_universe.allow_short_candidates
            && candidate.min_weight.abs() < MIN_WEIGHT_TOL
        {
            -candidate.max_weight
        } else {
            candidate.min_weight
        };

        features.push(DecisionFeatures {
            pv_base: 0.0, // Currently held value is 0
            pv_native: val_entry.value_native.amount(),
            pv_per_unit: pv_unit,
            measures,
            attributes: candidate.attributes.clone(),
            min_weight: candidate_min_weight,
            max_weight: candidate.max_weight, // Respect candidate constraints
        });
    }

    // Populate current weights based on weighting scheme.
    match problem.weighting {
        WeightingScheme::NotionalWeight => {
            // For NotionalWeight: use signed quantity / gross absolute quantity
            // This gives weights based on notional exposure, not PV
            if gross_notional.abs() > GROSS_BASE_TOL {
                for item in &items {
                    if item.is_existing {
                        let notional = position_notionals
                            .get(&item.position_id)
                            .copied()
                            .unwrap_or(0.0);
                        current_weights.insert(item.position_id.clone(), notional / gross_notional);
                    } else {
                        current_weights.insert(item.position_id.clone(), 0.0);
                    }
                }
            } else {
                for item in &items {
                    current_weights.insert(item.position_id.clone(), 0.0);
                }
            }
        }
        WeightingScheme::ValueWeight | WeightingScheme::UnitScaling => {
            // For ValueWeight/UnitScaling: use signed PV / gross market value
            // This handles hedged portfolios where net PV is ~0 but we still want meaningful weights.
            if gross_pv_base.abs() > GROSS_BASE_TOL {
                for (item, feat) in items.iter().zip(&features) {
                    if item.is_existing {
                        current_weights
                            .insert(item.position_id.clone(), feat.pv_base / gross_pv_base);
                    } else {
                        current_weights.insert(item.position_id.clone(), 0.0);
                    }
                }
            } else {
                for item in &items {
                    current_weights.insert(item.position_id.clone(), 0.0);
                }
            }
        }
    }

    Ok((
        items,
        features,
        current_weights,
        OptimizationDenominators {
            gross_pv_base,
            gross_notional,
        },
    ))
}
