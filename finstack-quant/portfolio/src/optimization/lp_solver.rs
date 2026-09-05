//! Problem, decision-space, constraint, and solver types for portfolio optimization.
//!
use super::constraints::{Constraint, Inequality};
use super::decision::{
    build_decision_space, quantity_from_scale, DecisionFeatures, DecisionItem,
    OptimizationDenominators,
};
use super::problem::PortfolioOptimizationProblem;
use super::result::{OptimizationStatus, PortfolioOptimizationResult};
use super::types::{
    MetricExpr, MissingMetricPolicy, PerPositionMetric, WeightingScheme, PV_PER_UNIT_TOL,
};
use crate::error::{Error, Result};
use crate::types::PositionId;
use finstack_quant_core::config::FinstackConfig;
use finstack_quant_core::market_data::context::MarketContext;
use finstack_quant_core::math::summation::NeumaierAccumulator;
use finstack_quant_valuations::metrics::MetricId;
use good_lp::{constraint, default_solver, variable, Expression, Solution, SolverModel};
use indexmap::IndexMap;

/// LP‑based optimizer using the `good_lp` crate as backend.
#[derive(Default)]
pub struct DefaultLpOptimizer;

/// Filtered weight sums with absolute value at or below this are treated as
/// zero when reporting `ValueWeightedAverage` bound slacks: the achieved
/// average `Σ_F wᵢ·mᵢ / Σ_F wᵢ` is undefined (the bound holds vacuously) and
/// the slack is reported as `NaN`. Matches the MO-18 post-solve tolerance on
/// the filtered weight sum.
const VWA_SLACK_DENOMINATOR_TOL: f64 = 1e-9;

/// Linear constraint: `coefficients · w (<=,>=,=) rhs`.
#[derive(Clone, Debug)]
struct LpConstraint {
    coefficients: Vec<f64>,
    relation: Inequality,
    rhs: f64,
    /// Optional name (constraint label) for diagnostics.
    name: Option<String>,
    /// Whether this is a turnover placeholder to be expanded with auxiliary variables.
    is_turnover_placeholder: bool,
    /// For `ValueWeightedAverage` bounds: which decision items matched the
    /// filter. The `Σ_F wᵢ(mᵢ − rhs) OP 0` linearization is only equivalent
    /// to `average OP rhs` when `Σ_F wᵢ > 0`, so the solution is checked
    /// post-solve against this mask.
    vwa_filter_mask: Option<Vec<bool>>,
    /// For `ValueWeightedAverage` bounds: the caller-facing bound `rhs` in
    /// metric units. The row itself is lowered to `Σ_F wᵢ(mᵢ − rhs) OP 0`
    /// (so `LpConstraint::rhs` is 0), and this value is used post-solve to
    /// report the slack back in metric units.
    vwa_rhs: Option<f64>,
}

impl DefaultLpOptimizer {
    fn reconstruction_denominator(
        weighting: WeightingScheme,
        denominators: OptimizationDenominators,
    ) -> f64 {
        match weighting {
            WeightingScheme::ValueWeight => denominators.gross_pv_base,
            WeightingScheme::NotionalWeight => denominators.gross_notional,
            WeightingScheme::UnitScaling => 1.0,
        }
    }

    /// Collect all `MetricId`s required by the problem's `PerPositionMetric`s.
    fn required_metrics(problem: &PortfolioOptimizationProblem) -> Vec<MetricId> {
        let mut metrics = Vec::new();

        let mut add_metric = |ppm: &PerPositionMetric| {
            if let PerPositionMetric::Metric(id) = ppm {
                if !metrics.contains(id) {
                    metrics.push(id.clone());
                }
            }
        };

        match &problem.objective {
            super::types::Objective::Maximize(expr) | super::types::Objective::Minimize(expr) => {
                match expr {
                    MetricExpr::WeightedSum { metric, .. }
                    | MetricExpr::ValueWeightedAverage { metric, .. } => {
                        add_metric(metric);
                    }
                }
            }
        }

        for constraint in &problem.constraints {
            match constraint {
                Constraint::MetricBound { metric, .. } => match metric {
                    MetricExpr::WeightedSum { metric, .. }
                    | MetricExpr::ValueWeightedAverage { metric, .. } => {
                        add_metric(metric);
                    }
                },
                Constraint::WeightBounds { .. }
                | Constraint::MaxTurnover { .. }
                | Constraint::Budget { .. } => {}
            }
        }

        metrics
    }

    /// Resolve a `PerPositionMetric` to its raw per‑decision value, if present.
    ///
    /// Returns `None` when the metric/attribute is missing for the position;
    /// callers apply the [`MissingMetricPolicy`] on top of this.
    fn per_position_metric_raw(ppm: &PerPositionMetric, feat: &DecisionFeatures) -> Option<f64> {
        match ppm {
            PerPositionMetric::Metric(id) => feat.measures.get(id.as_str()).copied(),
            PerPositionMetric::CustomKey(key) => feat.measures.get(key).copied(),
            PerPositionMetric::PvBase => Some(feat.pv_base),
            PerPositionMetric::PvNative => Some(feat.pv_native),
            PerPositionMetric::Attribute(key) => {
                feat.attributes.get(key).and_then(|v| v.as_number())
            }
            PerPositionMetric::AttributeIndicator(test) => {
                Some(if test.evaluate(&feat.attributes) {
                    1.0
                } else {
                    0.0
                })
            }
            PerPositionMetric::Constant(c) => Some(*c),
        }
    }

    /// Lower a `PerPositionMetric` to a per‑decision value `m_i`.
    fn per_position_metric_value(
        ppm: &PerPositionMetric,
        feat: &DecisionFeatures,
        missing_policy: MissingMetricPolicy,
    ) -> Result<f64> {
        match (Self::per_position_metric_raw(ppm, feat), missing_policy) {
            (Some(v), _) => Ok(v),
            (None, MissingMetricPolicy::Zero | MissingMetricPolicy::Exclude) => Ok(0.0),
            (None, MissingMetricPolicy::Strict) => {
                Err(Error::invalid_input("required metric missing for position"))
            }
        }
    }

    /// Build coefficient vector `a` for a `MetricExpr`.
    fn build_metric_coefficients(
        expr: &MetricExpr,
        feats: &[DecisionFeatures],
        missing_policy: MissingMetricPolicy,
        items: &[DecisionItem],
    ) -> Result<Vec<f64>> {
        let mut coeffs = Vec::with_capacity(feats.len());
        match expr {
            MetricExpr::ValueWeightedAverage {
                filter: Some(_), ..
            } => {
                return Err(Error::invalid_input(
                    "MO-6: filtered ValueWeightedAverage objectives are unsupported because \
                     the filtered denominator is decision-dependent; use a MetricBound \
                     or an unfiltered average objective",
                ));
            }
            MetricExpr::WeightedSum { metric, filter }
            | MetricExpr::ValueWeightedAverage { metric, filter } => {
                for (item, feat) in items.iter().zip(feats) {
                    if let Some(f) = filter {
                        if !f.matches(&item.entity_id, &item.position_id, &feat.attributes) {
                            coeffs.push(0.0);
                            continue;
                        }
                    }
                    // Under `MissingMetricPolicy::Exclude` a position with a
                    // missing metric keeps its current weight and is dropped
                    // from constraint / objective evaluation. `WeightedSum`
                    // and unfiltered `ValueWeightedAverage` share this
                    // coefficient builder; skip here the same way VWA bounds
                    // drop the name from both numerator and denominator.
                    if matches!(missing_policy, MissingMetricPolicy::Exclude)
                        && Self::per_position_metric_raw(metric, feat).is_none()
                    {
                        coeffs.push(0.0);
                        continue;
                    }
                    // Aggregated objectives sum across positions, so the per-
                    // position metric must be expressed in a common numeraire.
                    // `PvNative` is per-position native currency and is not
                    // commensurable across multi-currency portfolios, so reject
                    // it explicitly instead of silently substituting `PvBase`.
                    if matches!(metric, PerPositionMetric::PvNative) {
                        return Err(Error::invalid_input(
                            "PvNative is not valid in aggregated objectives \
                             (WeightedSum / ValueWeightedAverage); values in \
                             different native currencies are not commensurable. \
                             Use PerPositionMetric::PvBase instead.",
                        ));
                    }
                    // Candidates carry `pv_base = 0` (no held value), so a
                    // PvBase coefficient would silently ignore their actual
                    // value in the objective/constraint — fail closed instead.
                    if matches!(metric, PerPositionMetric::PvBase) && !item.is_existing {
                        return Err(Error::invalid_input(format!(
                            "PvBase is not valid in aggregated expressions when candidate \
                             positions are in scope: candidate '{}' has no held base value \
                             (pv_base = 0) and would be silently ignored. Filter candidates \
                             out of the expression or use a per-position metric/attribute.",
                            item.position_id
                        )));
                    }
                    let m_i = Self::per_position_metric_value(metric, feat, missing_policy)?;
                    coeffs.push(m_i);
                }
            }
        }

        Ok(coeffs)
    }

    /// Build the linearized row for `ValueWeightedAverage(metric) OP rhs`.
    ///
    /// The bound `average(metric over F) OP rhs` is represented as
    /// `Σ_F w_i * (m_i - rhs) OP 0`, avoiding the earlier dimensionally wrong
    /// `Σ_F w_i * m_i OP rhs` encoding when the filtered weight share is not 1.
    fn build_value_weighted_average_bound_coefficients(
        metric: &PerPositionMetric,
        filter: Option<&super::universe::PositionFilter>,
        rhs: f64,
        feats: &[DecisionFeatures],
        missing_policy: MissingMetricPolicy,
        items: &[DecisionItem],
    ) -> Result<(Vec<f64>, Vec<bool>)> {
        let mut coeffs = Vec::with_capacity(feats.len());
        let mut mask = Vec::with_capacity(feats.len());
        for (item, feat) in items.iter().zip(feats) {
            if let Some(f) = filter {
                if !f.matches(&item.entity_id, &item.position_id, &feat.attributes) {
                    coeffs.push(0.0);
                    mask.push(false);
                    continue;
                }
            }
            // Under `MissingMetricPolicy::Exclude` a position with a missing
            // metric keeps its current weight and is excluded from constraint
            // evaluation. Leaving it in the average with `m_i = 0` would
            // contribute `−wᵢ·rhs` to the lowered row and distort the bound,
            // so drop it from both numerator and denominator.
            if matches!(missing_policy, MissingMetricPolicy::Exclude)
                && Self::per_position_metric_raw(metric, feat).is_none()
            {
                coeffs.push(0.0);
                mask.push(false);
                continue;
            }
            mask.push(true);
            if matches!(metric, PerPositionMetric::PvNative) {
                return Err(Error::invalid_input(
                    "PvNative is not valid in ValueWeightedAverage constraints; \
                     use PerPositionMetric::PvBase instead.",
                ));
            }
            // Candidates carry `pv_base = 0`; see the identical guard in
            // `build_metric_coefficients`.
            if matches!(metric, PerPositionMetric::PvBase) && !item.is_existing {
                return Err(Error::invalid_input(format!(
                    "PvBase is not valid in aggregated expressions when candidate \
                     positions are in scope: candidate '{}' has no held base value \
                     (pv_base = 0) and would be silently ignored. Filter candidates \
                     out of the expression or use a per-position metric/attribute.",
                    item.position_id
                )));
            }
            let m_i = Self::per_position_metric_value(metric, feat, missing_policy)?;
            coeffs.push(m_i - rhs);
        }
        Ok((coeffs, mask))
    }
}

/// Objective coefficients plus the assembled LP constraint rows for a problem.
struct LpRows {
    coeffs_objective: Vec<f64>,
    lp_constraints: Vec<LpConstraint>,
}

/// A solved LP model together with the bookkeeping needed to reconstruct the
/// portfolio-level result (decision variables and turnover
/// auxiliary variables).
struct AssembledModel {
    solution: Box<dyn Solution>,
    w_vars: Vec<good_lp::Variable>,
}

impl DefaultLpOptimizer {
    /// Build objective coefficients and the LP constraint rows for `problem`.
    ///
    /// This covers Steps 4 and 5 of [`Self::optimize`]: lowering the objective
    /// expression and each [`Constraint`] into coefficient vectors. A synthetic
    /// budget row is appended when the problem declares none, except under
    /// [`WeightingScheme::UnitScaling`] where a sum-of-multipliers budget is
    /// dimensionally meaningless and must be caller-explicit (MO-9).
    fn build_lp_rows(
        problem: &PortfolioOptimizationProblem,
        decision_features: &[DecisionFeatures],
        decision_items: &[DecisionItem],
        n_vars: usize,
        has_budget: bool,
    ) -> Result<LpRows> {
        let turnover_constraint_count = problem
            .constraints
            .iter()
            .filter(|c| matches!(c, Constraint::MaxTurnover { .. }))
            .count();
        if turnover_constraint_count > 1 {
            return Err(Error::invalid_input(
                "M-9: duplicate MaxTurnover constraints are ambiguous; combine them into one cap",
            ));
        }

        let budget_constraints: Vec<f64> = problem
            .constraints
            .iter()
            .filter_map(|c| match c {
                Constraint::Budget { rhs } => Some(*rhs),
                _ => None,
            })
            .collect();
        if budget_constraints.len() > 1 {
            return Err(Error::invalid_input(
                "MO-21: duplicate Budget constraints are ambiguous (two Σwᵢ = rhs equalities); \
                 combine them into one",
            ));
        }

        // MO-19: an unfiltered ValueWeightedAverage objective is lowered to
        // the plain linear form Σ wᵢ·mᵢ, which equals the true average
        // Σ wᵢ·mᵢ / Σ wᵢ only when the weights sum to 1. Any other budget
        // scales the reported objective by Σw — and sign-flips the
        // optimization direction when Σw < 0 — so fail closed unless the
        // problem guarantees Σw = 1 (no explicit budget, in which case the
        // synthesized Σw = 1 row applies, or an explicit Budget { rhs: 1.0 }).
        let objective_is_unfiltered_vwa = matches!(
            &problem.objective,
            super::types::Objective::Maximize(MetricExpr::ValueWeightedAverage {
                filter: None,
                ..
            }) | super::types::Objective::Minimize(MetricExpr::ValueWeightedAverage {
                filter: None,
                ..
            })
        );
        if objective_is_unfiltered_vwa {
            if let Some(&rhs) = budget_constraints.first() {
                // Exact comparison is deliberate: the Σw = 1 guarantee holds
                // only for a budget rhs that is exactly 1.0 (a caller-typed
                // literal); any other value — however close — scales the
                // lowered objective.
                #[allow(clippy::float_cmp)]
                if rhs != 1.0 {
                    return Err(Error::invalid_input(format!(
                        "MO-19: a ValueWeightedAverage objective is lowered to Σ wᵢ·mᵢ, which \
                         equals the true average only when the weights sum to 1; the explicit \
                         Budget rhs is {rhs}, so the reported objective would be scaled by \
                         Σw = {rhs} (and the optimization direction flipped for a negative \
                         budget). Use Budget {{ rhs: 1.0 }} or a WeightedSum objective."
                    )));
                }
            }
        }

        let objective_expr = &problem.objective;
        let coeffs_objective = match objective_expr {
            super::types::Objective::Maximize(expr) | super::types::Objective::Minimize(expr) => {
                Self::build_metric_coefficients(
                    expr,
                    decision_features,
                    problem.missing_metric_policy,
                    decision_items,
                )?
            }
        };

        let mut lp_constraints: Vec<LpConstraint> = Vec::new();

        for constraint in &problem.constraints {
            match constraint {
                Constraint::MetricBound {
                    label,
                    metric,
                    op,
                    rhs,
                } => {
                    let (a, lowered_rhs, vwa_filter_mask, vwa_rhs) = match metric {
                        MetricExpr::ValueWeightedAverage { metric, filter } => {
                            let (coeffs, mask) =
                                Self::build_value_weighted_average_bound_coefficients(
                                    metric,
                                    filter.as_ref(),
                                    *rhs,
                                    decision_features,
                                    problem.missing_metric_policy,
                                    decision_items,
                                )?;
                            (coeffs, 0.0, Some(mask), Some(*rhs))
                        }
                        MetricExpr::WeightedSum { .. } => (
                            Self::build_metric_coefficients(
                                metric,
                                decision_features,
                                problem.missing_metric_policy,
                                decision_items,
                            )?,
                            *rhs,
                            None,
                            None,
                        ),
                    };
                    lp_constraints.push(LpConstraint {
                        coefficients: a,
                        relation: *op,
                        rhs: lowered_rhs,
                        name: label.clone(),
                        is_turnover_placeholder: false,
                        vwa_filter_mask,
                        vwa_rhs,
                    });
                }
                Constraint::WeightBounds { .. } => {
                    // Already applied to `DecisionFeatures::min_weight/max_weight`.
                }
                Constraint::MaxTurnover {
                    label,
                    max_turnover,
                } => {
                    lp_constraints.push(LpConstraint {
                        coefficients: vec![0.0; n_vars],
                        relation: Inequality::Le,
                        rhs: *max_turnover,
                        name: label.clone().or_else(|| Some("turnover".to_string())),
                        is_turnover_placeholder: true,
                        vwa_filter_mask: None,
                        vwa_rhs: None,
                    });
                }
                Constraint::Budget { rhs } => {
                    let coefficients = vec![1.0; n_vars];
                    lp_constraints.push(LpConstraint {
                        coefficients,
                        relation: Inequality::Eq,
                        rhs: *rhs,
                        name: Some("budget".to_string()),
                        is_turnover_placeholder: false,
                        vwa_filter_mask: None,
                        vwa_rhs: None,
                    });
                }
            }
        }

        if !has_budget {
            if matches!(problem.weighting, WeightingScheme::UnitScaling) {
                return Err(Error::invalid_input(
                    "MO-9: UnitScaling requires an explicit budget or other bounding constraints; \
                     the optimizer will not synthesize sum(multipliers) = 1",
                ));
            }
            lp_constraints.push(LpConstraint {
                coefficients: vec![1.0; n_vars],
                relation: Inequality::Eq,
                rhs: 1.0,
                name: Some("budget".to_string()),
                is_turnover_placeholder: false,
                vwa_filter_mask: None,
                vwa_rhs: None,
            });
        }

        Ok(LpRows {
            coeffs_objective,
            lp_constraints,
        })
    }

    /// Declare `good_lp` variables, assemble the objective and constraint rows
    /// into a solver model, and solve it.
    ///
    /// On solver failure the error is mapped to a structured
    /// [`OptimizationStatus`] and surfaced as `Ok(Err)` so the caller can
    /// build the failure-result envelope.
    #[allow(clippy::type_complexity)]
    fn assemble_and_solve(
        problem: &PortfolioOptimizationProblem,
        decision_items: &[DecisionItem],
        decision_features: &[DecisionFeatures],
        current_weights: &IndexMap<PositionId, f64>,
        rows: &LpRows,
        n_vars: usize,
    ) -> Result<std::result::Result<AssembledModel, OptimizationStatus>> {
        let maximise = matches!(problem.objective, super::types::Objective::Maximize(_));
        let mut vars = good_lp::variables!();

        let mut w_vars = Vec::with_capacity(n_vars);
        for (item, feat) in decision_items.iter().zip(decision_features) {
            let current_weight = current_weights
                .get(&item.position_id)
                .copied()
                .unwrap_or(0.0);
            let (min_w, max_w) = if item.is_held {
                (current_weight, current_weight)
            } else {
                (feat.min_weight, feat.max_weight)
            };
            if max_w < min_w {
                return Err(Error::invalid_input(format!(
                    "inconsistent weight bounds for '{}': min {} > max {}",
                    item.position_id, min_w, max_w
                )));
            }

            let (var_min, var_max) = (min_w, max_w);

            w_vars.push(vars.add(variable().min(var_min).max(var_max)));
        }

        let has_turnover_constraint = problem
            .constraints
            .iter()
            .any(|c| matches!(c, Constraint::MaxTurnover { .. }));

        let mut t_vars: Vec<Option<good_lp::Variable>> = vec![None; n_vars];

        if has_turnover_constraint {
            for t_var in t_vars.iter_mut().take(n_vars) {
                *t_var = Some(vars.add(variable().min(0.0)));
            }
        }

        let mut objective_expr: Expression = 0.0.into();
        for (var, coef) in w_vars.iter().zip(&rows.coeffs_objective) {
            objective_expr += (*coef) * *var;
        }

        let mut problem_model = if maximise {
            vars.maximise(objective_expr)
        } else {
            vars.minimise(objective_expr)
        }
        .using(default_solver);

        for lc in &rows.lp_constraints {
            if lc.is_turnover_placeholder {
                continue;
            }

            let mut lhs: Expression = 0.0.into();
            for (var, coef) in w_vars.iter().zip(&lc.coefficients) {
                lhs += (*coef) * *var;
            }

            problem_model = match lc.relation {
                Inequality::Le => problem_model.with(constraint!(lhs <= lc.rhs)),
                Inequality::Ge => problem_model.with(constraint!(lhs >= lc.rhs)),
                Inequality::Eq => problem_model.with(constraint!(lhs == lc.rhs)),
            };
        }

        if let Some(Constraint::MaxTurnover { max_turnover, .. }) = problem
            .constraints
            .iter()
            .find(|c| matches!(c, Constraint::MaxTurnover { .. }))
        {
            for (idx, w_var) in w_vars.iter().enumerate() {
                let Some(t_var) = t_vars[idx] else {
                    continue;
                };
                let w0 = current_weights
                    .get(&decision_items[idx].position_id)
                    .copied()
                    .unwrap_or(0.0);

                let lhs1: Expression = t_var - *w_var;
                problem_model = problem_model.with(constraint!(lhs1 >= -w0));

                let lhs2: Expression = t_var + *w_var;
                problem_model = problem_model.with(constraint!(lhs2 >= w0));
            }

            let mut lhs_turnover: Expression = 0.0.into();
            for tv in t_vars.iter().flatten() {
                lhs_turnover += *tv;
            }
            problem_model = problem_model.with(constraint!(lhs_turnover <= *max_turnover));
        }

        // Solver failures become structured `OptimizationStatus` (`Ok(Err)`):
        // `objective_value = NaN`, empty weight maps, and empty
        // `conflicting_constraints` because `good_lp` does not expose an IIS.
        match problem_model.solve() {
            Ok(sol) => Ok(Ok(AssembledModel {
                solution: Box::new(sol),
                w_vars,
            })),
            Err(e) => {
                let status = match &e {
                    good_lp::ResolutionError::Infeasible => OptimizationStatus::Infeasible {
                        conflicting_constraints: Vec::new(),
                    },
                    good_lp::ResolutionError::Unbounded => OptimizationStatus::Unbounded,
                    _ => OptimizationStatus::Error {
                        message: e.to_string(),
                    },
                };
                Ok(Err(status))
            }
        }
    }

    /// Reconstruct the portfolio-level result from a solved LP model.
    ///
    /// This covers the post-solve steps of [`Self::optimize`]: optimal weights,
    /// weight deltas, implied quantities, the objective value, metric values,
    /// and constraint slacks.
    #[allow(clippy::too_many_arguments)]
    fn reconstruct_result(
        problem: &PortfolioOptimizationProblem,
        decision_items: &[DecisionItem],
        decision_features: &[DecisionFeatures],
        current_weights: IndexMap<PositionId, f64>,
        denominators: OptimizationDenominators,
        rows: &LpRows,
        model: &AssembledModel,
        config: &FinstackConfig,
    ) -> PortfolioOptimizationResult {
        let AssembledModel { solution, w_vars } = model;
        let solution = solution.as_ref();

        let mut optimal_weights: IndexMap<PositionId, f64> = IndexMap::new();
        let mut weight_deltas: IndexMap<PositionId, f64> = IndexMap::new();

        for (item, w_var) in decision_items.iter().zip(w_vars) {
            let w_star = solution.value(*w_var);
            let w0 = current_weights
                .get(&item.position_id)
                .copied()
                .unwrap_or(0.0);
            optimal_weights.insert(item.position_id.clone(), w_star);
            weight_deltas.insert(item.position_id.clone(), w_star - w0);
        }

        let mut implied_quantities: IndexMap<PositionId, f64> = IndexMap::new();
        let reconstruction_denominator =
            Self::reconstruction_denominator(problem.weighting, denominators);
        for (item, feat) in decision_items.iter().zip(decision_features) {
            let w_star = optimal_weights
                .get(&item.position_id)
                .copied()
                .unwrap_or(0.0);
            let qty = match problem.weighting {
                WeightingScheme::NotionalWeight => {
                    if feat.deal_notional_abs > PV_PER_UNIT_TOL {
                        quantity_from_scale(
                            item.unit,
                            (w_star * reconstruction_denominator) / feat.deal_notional_abs,
                        )
                    } else {
                        0.0
                    }
                }
                WeightingScheme::ValueWeight => {
                    if feat.pv_per_unit.abs() > PV_PER_UNIT_TOL {
                        quantity_from_scale(
                            item.unit,
                            (w_star * reconstruction_denominator) / feat.pv_per_unit,
                        )
                    } else {
                        0.0
                    }
                }
                WeightingScheme::UnitScaling => {
                    if item.is_existing {
                        item.current_quantity * w_star
                    } else {
                        w_star
                    }
                }
            };

            implied_quantities.insert(item.position_id.clone(), qty);
        }

        let mut objective_acc = NeumaierAccumulator::new();
        for (coef, w_var) in rows.coeffs_objective.iter().zip(w_vars) {
            let w_star = solution.value(*w_var);
            objective_acc.add(*coef * w_star);
        }
        let objective_value = objective_acc.current();

        let mut metric_values: IndexMap<String, f64> = IndexMap::new();
        metric_values.insert("objective".to_string(), objective_value);

        let mut constraint_slacks: IndexMap<String, f64> = IndexMap::new();
        for lc in &rows.lp_constraints {
            if lc.is_turnover_placeholder {
                continue;
            }
            if let Some(name) = &lc.name {
                let slack = if let (Some(mask), Some(bound_rhs)) = (&lc.vwa_filter_mask, lc.vwa_rhs)
                {
                    // ValueWeightedAverage bounds are lowered to
                    // `Σ_F wᵢ(mᵢ − rhs) OP 0`, whose raw row slack is in
                    // weight×metric units. Report the slack in metric units
                    // instead: recompute the achieved filtered average
                    // (`coef + rhs` recovers `mᵢ`) and compare against the
                    // caller-facing rhs. When the filtered weight sum is ~0
                    // the average is undefined (the bound holds vacuously),
                    // so the slack is reported as NaN.
                    let mut numerator = 0.0;
                    let mut filtered_weight_sum = 0.0;
                    for ((var, coef), matched) in w_vars.iter().zip(&lc.coefficients).zip(mask) {
                        if !matched {
                            continue;
                        }
                        let w = solution.value(*var);
                        numerator += w * (*coef + bound_rhs);
                        filtered_weight_sum += w;
                    }
                    if filtered_weight_sum.abs() <= VWA_SLACK_DENOMINATOR_TOL {
                        f64::NAN
                    } else {
                        let achieved_average = numerator / filtered_weight_sum;
                        match lc.relation {
                            Inequality::Le => bound_rhs - achieved_average,
                            Inequality::Ge => achieved_average - bound_rhs,
                            Inequality::Eq => (achieved_average - bound_rhs).abs(),
                        }
                    }
                } else {
                    let mut lhs_val = 0.0;
                    for (var, coef) in w_vars.iter().zip(&lc.coefficients) {
                        lhs_val += *coef * solution.value(*var);
                    }

                    match lc.relation {
                        Inequality::Le => lc.rhs - lhs_val,
                        Inequality::Ge => lhs_val - lc.rhs,
                        Inequality::Eq => (lhs_val - lc.rhs).abs(),
                    }
                };
                constraint_slacks.insert(name.clone(), slack);
            }
        }
        if let Some(Constraint::MaxTurnover {
            label,
            max_turnover,
        }) = problem
            .constraints
            .iter()
            .find(|c| matches!(c, Constraint::MaxTurnover { .. }))
        {
            let actual_turnover: f64 = decision_items
                .iter()
                .map(|item| {
                    let w0 = current_weights
                        .get(&item.position_id)
                        .copied()
                        .unwrap_or(0.0);
                    let w_star = optimal_weights
                        .get(&item.position_id)
                        .copied()
                        .unwrap_or(0.0);
                    (w_star - w0).abs()
                })
                .sum();
            constraint_slacks.insert(
                label.clone().unwrap_or_else(|| "turnover".to_string()),
                *max_turnover - actual_turnover,
            );
        }

        let status = OptimizationStatus::Optimal;

        let meta = finstack_quant_core::config::results_meta_now(config);

        PortfolioOptimizationResult {
            problem: problem.clone(),
            current_weights,
            optimal_weights,
            weight_deltas,
            implied_quantities,
            objective_value,
            metric_values,
            status,
            constraint_slacks,
            meta,
        }
    }

    /// Optimize the portfolio for the given problem and market/config context.
    ///
    /// # Errors
    ///
    /// Returns [`Error`] when:
    /// - Portfolio validation fails
    /// - Required metrics cannot be priced
    /// - The LP backend fails or returns an invalid solution
    pub fn optimize(
        &self,
        problem: &PortfolioOptimizationProblem,
        market: &MarketContext,
        config: &FinstackConfig,
    ) -> Result<PortfolioOptimizationResult> {
        problem.portfolio.validate()?;

        let has_budget = problem
            .constraints
            .iter()
            .any(|c| matches!(c, Constraint::Budget { .. }));

        let required_metrics = Self::required_metrics(problem);
        let options = crate::valuation::PortfolioValuationOptions {
            strict_risk: matches!(problem.missing_metric_policy, MissingMetricPolicy::Strict),
            metrics: if required_metrics.is_empty() {
                crate::valuation::RequestedMetrics::Standard
            } else {
                crate::valuation::RequestedMetrics::StandardPlus(required_metrics.clone())
            },
        };

        let valuation =
            crate::valuation::value_portfolio(&problem.portfolio, market, config, &options)?;

        let (decision_items, mut decision_features, current_weights, denominators) =
            build_decision_space(problem, &valuation, &required_metrics, market, config)?;

        if decision_items.is_empty() {
            return Err(Error::invalid_input(
                "no decision variables in optimization problem",
            ));
        }

        let n_vars = decision_items.len();

        for constraint in &problem.constraints {
            if let Constraint::WeightBounds {
                filter, min, max, ..
            } = constraint
            {
                for (item, feat) in decision_items.iter().zip(decision_features.iter_mut()) {
                    let is_match = if item.is_existing {
                        if let Some(position) =
                            problem.portfolio.get_position(item.position_id.as_str())
                        {
                            filter.matches(
                                &position.entity_id,
                                &position.position_id,
                                &position.attributes,
                            )
                        } else {
                            false
                        }
                    } else {
                        problem
                            .trade_universe
                            .candidates
                            .iter()
                            .find(|candidate| candidate.id == item.position_id)
                            .is_some_and(|candidate| {
                                filter.matches(
                                    &candidate.entity_id,
                                    &candidate.id,
                                    &candidate.attributes,
                                )
                            })
                    };

                    if is_match {
                        feat.min_weight = feat.min_weight.max(*min);
                        feat.max_weight = feat.max_weight.min(*max);
                    }
                }
            }
        }

        let rows = Self::build_lp_rows(
            problem,
            &decision_features,
            &decision_items,
            n_vars,
            has_budget,
        )?;

        let model = match Self::assemble_and_solve(
            problem,
            &decision_items,
            &decision_features,
            &current_weights,
            &rows,
            n_vars,
        )? {
            Ok(model) => model,
            Err(status) => {
                let meta = finstack_quant_core::config::results_meta_now(config);
                return Ok(PortfolioOptimizationResult {
                    problem: problem.clone(),
                    current_weights,
                    optimal_weights: IndexMap::new(),
                    weight_deltas: IndexMap::new(),
                    implied_quantities: IndexMap::new(),
                    objective_value: f64::NAN,
                    metric_values: IndexMap::new(),
                    status,
                    constraint_slacks: IndexMap::new(),
                    meta,
                });
            }
        };

        // Post-solve validation: the `Σ_F wᵢ(mᵢ − rhs) OP 0` linearization of a
        // ValueWeightedAverage bound is the multiply-through form of
        // `average OP rhs` and preserves the inequality sense only when the
        // filtered weight sum is non-negative. A zero sum is fine — holding
        // none of the filtered basket satisfies an average bound vacuously —
        // but a *negative* sum enforces the opposite of what the caller asked
        // (dividing by a negative quantity flips the inequality), so fail
        // loudly instead of reporting Optimal.
        for lc in &rows.lp_constraints {
            let Some(mask) = &lc.vwa_filter_mask else {
                continue;
            };
            let filtered_weight_sum: f64 = model
                .w_vars
                .iter()
                .zip(mask)
                .filter(|(_, matched)| **matched)
                .map(|(w, _)| model.solution.value(*w))
                .sum();
            if filtered_weight_sum < -1e-9 {
                return Err(Error::invalid_input(format!(
                    "MO-18: ValueWeightedAverage bound '{}' has a negative filtered weight \
                     sum ({filtered_weight_sum:.3e}) at the solution, which flips the \
                     inequality sense of the average linearization — the reported optimum \
                     enforces the opposite of the requested bound. Keep the filtered \
                     positions' net weight non-negative (e.g. WeightBounds) or express \
                     the bound as a WeightedSum.",
                    lc.name.as_deref().unwrap_or("<unnamed>")
                )));
            }
        }

        Ok(Self::reconstruct_result(
            problem,
            &decision_items,
            &decision_features,
            current_weights,
            denominators,
            &rows,
            &model,
            config,
        ))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::AttributeValue;

    #[test]
    fn pv_native_metric_uses_native_value_not_base_value() {
        let feat = DecisionFeatures {
            pv_base: 125.0,
            pv_native: 100.0,
            pv_per_unit: 125.0,
            deal_notional_abs: 0.0,
            measures: IndexMap::new(),
            attributes: IndexMap::new(),
            min_weight: 0.0,
            max_weight: 1.0,
        };

        let value = DefaultLpOptimizer::per_position_metric_value(
            &PerPositionMetric::PvNative,
            &feat,
            MissingMetricPolicy::Strict,
        )
        .expect("native PV should be available");

        assert_eq!(value, 100.0);
    }

    #[test]
    fn attribute_indicator_metric_evaluates_correctly() {
        let mut attributes = IndexMap::new();
        attributes.insert(
            "rating".to_string(),
            AttributeValue::Text("CCC".to_string()),
        );
        let feat = DecisionFeatures {
            pv_base: 100.0,
            pv_native: 100.0,
            pv_per_unit: 100.0,
            deal_notional_abs: 0.0,
            measures: IndexMap::new(),
            attributes,
            min_weight: 0.0,
            max_weight: 1.0,
        };

        let matching = DefaultLpOptimizer::per_position_metric_value(
            &PerPositionMetric::AttributeIndicator(crate::types::AttributeTest::text_eq(
                "rating", "CCC",
            )),
            &feat,
            MissingMetricPolicy::Zero,
        )
        .expect("should resolve");
        assert_eq!(matching, 1.0);

        let non_matching = DefaultLpOptimizer::per_position_metric_value(
            &PerPositionMetric::AttributeIndicator(crate::types::AttributeTest::text_eq(
                "rating", "BBB",
            )),
            &feat,
            MissingMetricPolicy::Zero,
        )
        .expect("should resolve");
        assert_eq!(non_matching, 0.0);
    }

    #[test]
    fn numeric_attribute_metric_resolves() {
        let mut attributes = IndexMap::new();
        attributes.insert("score".to_string(), AttributeValue::Number(650.0));
        let feat = DecisionFeatures {
            pv_base: 100.0,
            pv_native: 100.0,
            pv_per_unit: 100.0,
            deal_notional_abs: 0.0,
            measures: IndexMap::new(),
            attributes,
            min_weight: 0.0,
            max_weight: 1.0,
        };

        let value = DefaultLpOptimizer::per_position_metric_value(
            &PerPositionMetric::Attribute("score".to_string()),
            &feat,
            MissingMetricPolicy::Zero,
        )
        .expect("should resolve");
        assert_eq!(value, 650.0);
    }

    #[test]
    fn weighted_sum_exclude_skips_missing_metric_like_vwa() {
        use crate::position::PositionUnit;
        use crate::types::{EntityId, PositionId};

        let scored = DecisionFeatures {
            pv_base: 50.0,
            pv_native: 50.0,
            pv_per_unit: 50.0,
            deal_notional_abs: 0.0,
            measures: IndexMap::from([("ytm".to_string(), 10.0)]),
            attributes: IndexMap::new(),
            min_weight: 0.0,
            max_weight: 1.0,
        };
        let missing = DecisionFeatures {
            pv_base: 50.0,
            pv_native: 50.0,
            pv_per_unit: 50.0,
            deal_notional_abs: 0.0,
            measures: IndexMap::new(),
            attributes: IndexMap::new(),
            min_weight: 0.0,
            max_weight: 1.0,
        };
        let items = vec![
            DecisionItem {
                position_id: PositionId::from("SCORED"),
                entity_id: EntityId::from("ENT"),
                is_existing: true,
                is_held: false,
                current_quantity: 1.0,
                unit: PositionUnit::Units,
            },
            DecisionItem {
                position_id: PositionId::from("MISSING"),
                entity_id: EntityId::from("ENT"),
                is_existing: true,
                is_held: true,
                current_quantity: 1.0,
                unit: PositionUnit::Units,
            },
        ];
        let expr = MetricExpr::WeightedSum {
            metric: PerPositionMetric::Metric(MetricId::Ytm),
            filter: None,
        };

        let zero_coeffs = DefaultLpOptimizer::build_metric_coefficients(
            &expr,
            &[scored.clone(), missing.clone()],
            MissingMetricPolicy::Zero,
            &items,
        )
        .expect("Zero policy should resolve missing as 0");
        assert_eq!(zero_coeffs, vec![10.0, 0.0]);

        let exclude_coeffs = DefaultLpOptimizer::build_metric_coefficients(
            &expr,
            &[scored, missing],
            MissingMetricPolicy::Exclude,
            &items,
        )
        .expect("Exclude policy should skip missing names");
        assert_eq!(exclude_coeffs, vec![10.0, 0.0]);
    }
}
