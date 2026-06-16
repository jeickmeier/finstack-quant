use super::constraints::{Constraint, Inequality};
use super::decision::{
    build_decision_space, DecisionFeatures, DecisionItem, OptimizationDenominators,
};
use super::problem::PortfolioOptimizationProblem;
use super::result::{OptimizationStatus, PortfolioOptimizationResult};
use super::types::{
    MetricExpr, MissingMetricPolicy, PerPositionMetric, WeightingScheme, PV_PER_UNIT_TOL,
};
use crate::error::{Error, Result};
use crate::types::{EntityId, PositionId};
use finstack_quant_core::config::FinstackConfig;
use finstack_quant_core::market_data::context::MarketContext;
use finstack_quant_core::math::summation::NeumaierAccumulator;
use finstack_quant_valuations::metrics::MetricId;
use good_lp::{constraint, default_solver, variable, Expression, Solution, SolverModel};
use indexmap::IndexMap;

/// LP‑based optimizer using the `good_lp` crate as backend.
#[derive(Default)]
pub struct DefaultLpOptimizer;

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
}

#[derive(Clone, Copy, Debug)]
struct WeightVarSpec {
    var: good_lp::Variable,
    offset: f64,
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

        // Scan objective
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

        // Scan constraints
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

    /// Resolve the entity ID for a decision item.
    fn decision_entity_id(item: &DecisionItem) -> &EntityId {
        &item.entity_id
    }

    /// Lower a `PerPositionMetric` to a per‑decision value `m_i`.
    fn per_position_metric_value(
        ppm: &PerPositionMetric,
        feat: &DecisionFeatures,
        missing_policy: MissingMetricPolicy,
    ) -> Result<f64> {
        let val = match ppm {
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
        };

        match (val, missing_policy) {
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
                        if !f.matches(
                            Self::decision_entity_id(item),
                            &item.position_id,
                            &feat.attributes,
                        ) {
                            coeffs.push(0.0);
                            continue;
                        }
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
    ) -> Result<Vec<f64>> {
        let mut coeffs = Vec::with_capacity(feats.len());
        for (item, feat) in items.iter().zip(feats) {
            if let Some(f) = filter {
                if !f.matches(
                    Self::decision_entity_id(item),
                    &item.position_id,
                    &feat.attributes,
                ) {
                    coeffs.push(0.0);
                    continue;
                }
            }
            if matches!(metric, PerPositionMetric::PvNative) {
                return Err(Error::invalid_input(
                    "PvNative is not valid in ValueWeightedAverage constraints; \
                     use PerPositionMetric::PvBase instead.",
                ));
            }
            let m_i = Self::per_position_metric_value(metric, feat, missing_policy)?;
            coeffs.push(m_i - rhs);
        }
        Ok(coeffs)
    }
}

/// Objective coefficients plus the assembled LP constraint rows for a problem.
struct LpRows {
    coeffs_objective: Vec<f64>,
    lp_constraints: Vec<LpConstraint>,
}

/// A solved LP model together with the bookkeeping needed to reconstruct the
/// portfolio-level result (decision variables, their offsets, and turnover
/// auxiliary variables).
struct AssembledModel {
    solution: Box<dyn Solution>,
    w_vars: Vec<WeightVarSpec>,
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

        // Step 4: Build objective coefficients.
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

        // Step 5: Build constraints as LP rows.
        let mut lp_constraints: Vec<LpConstraint> = Vec::new();

        for constraint in &problem.constraints {
            match constraint {
                Constraint::MetricBound {
                    label,
                    metric,
                    op,
                    rhs,
                } => {
                    let (a, lowered_rhs) = match metric {
                        MetricExpr::ValueWeightedAverage { metric, filter } => (
                            Self::build_value_weighted_average_bound_coefficients(
                                metric,
                                filter.as_ref(),
                                *rhs,
                                decision_features,
                                problem.missing_metric_policy,
                                decision_items,
                            )?,
                            0.0,
                        ),
                        MetricExpr::WeightedSum { .. } => (
                            Self::build_metric_coefficients(
                                metric,
                                decision_features,
                                problem.missing_metric_policy,
                                decision_items,
                            )?,
                            *rhs,
                        ),
                    };
                    lp_constraints.push(LpConstraint {
                        coefficients: a,
                        relation: *op,
                        rhs: lowered_rhs,
                        name: label.clone(),
                        is_turnover_placeholder: false,
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
    /// This covers Step 6 of [`Self::optimize`]. On solver failure the error is
    /// mapped to a structured [`OptimizationStatus`] and surfaced as `Ok(Err)`
    /// so the caller can build the failure-result envelope.
    #[allow(clippy::type_complexity)]
    fn assemble_and_solve(
        problem: &PortfolioOptimizationProblem,
        decision_items: &[DecisionItem],
        decision_features: &[DecisionFeatures],
        current_weights: &IndexMap<PositionId, f64>,
        rows: &LpRows,
        n_vars: usize,
    ) -> Result<std::result::Result<AssembledModel, OptimizationStatus>> {
        // Step 6: Assemble LP model using good_lp.
        let maximise = matches!(problem.objective, super::types::Objective::Maximize(_));
        let mut vars = good_lp::variables!();

        // Decision variables w_i
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

            let (var_min, var_max, offset) = (min_w, max_w, 0.0);

            w_vars.push(WeightVarSpec {
                var: vars.add(variable().min(var_min).max(var_max)),
                offset,
            });
        }

        // Auxiliary variables for turnover t_i (|w_i - w0_i|) if needed.
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

        // Objective
        let mut objective_expr: Expression = 0.0.into();
        for (var, coef) in w_vars.iter().zip(&rows.coeffs_objective) {
            objective_expr += (*coef) * var.var;
            if var.offset != 0.0 {
                objective_expr += (*coef) * var.offset;
            }
        }

        let mut problem_model = if maximise {
            vars.maximise(objective_expr)
        } else {
            vars.minimise(objective_expr)
        }
        .using(default_solver);

        // NOTE: effective-weight bounds are implicit: each variable is
        // declared on `[var_min, var_max] = [0, max_w - min_w]` with
        // offset `min_w`, so `effective_weight = var + offset` always
        // lies in `[min_w, max_w]`. No additional constraints are
        // needed — they would be algebraically redundant.

        // Add primary constraints
        for lc in &rows.lp_constraints {
            if lc.is_turnover_placeholder {
                continue;
            }

            let mut lhs: Expression = 0.0.into();
            for (var, coef) in w_vars.iter().zip(&lc.coefficients) {
                lhs += (*coef) * var.var;
                if var.offset != 0.0 {
                    lhs += (*coef) * var.offset;
                }
            }

            problem_model = match lc.relation {
                Inequality::Le => problem_model.with(constraint!(lhs <= lc.rhs)),
                Inequality::Ge => problem_model.with(constraint!(lhs >= lc.rhs)),
                Inequality::Eq => problem_model.with(constraint!(lhs == lc.rhs)),
            };
        }

        // Turnover constraint with auxiliary variables: Σ t_i <= max_turnover.
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

                let lhs1: Expression = t_var - w_var.var;
                problem_model = problem_model.with(constraint!(lhs1 >= w_var.offset - w0));

                let lhs2: Expression = t_var + w_var.var;
                problem_model = problem_model.with(constraint!(lhs2 >= w0 - w_var.offset));
            }

            let mut lhs_turnover: Expression = 0.0.into();
            for tv in t_vars.iter().flatten() {
                lhs_turnover += *tv;
            }
            problem_model = problem_model.with(constraint!(lhs_turnover <= *max_turnover));
        }

        // Solve LP — map solver failures to structured OptimizationStatus
        // rather than opaque errors so callers can inspect the reason.
        //
        // On failure the result carries:
        //   - `objective_value = NaN` — callers must check `status.is_feasible()`
        //     before consuming this value.
        //   - Empty weight/delta/quantity maps — no phantom allocations.
        //   - `conflicting_constraints = []` for Infeasible — the `good_lp` crate
        //     does not expose irreducible infeasible set (IIS) information.
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

        // Extract weights
        let mut optimal_weights: IndexMap<PositionId, f64> = IndexMap::new();
        let mut weight_deltas: IndexMap<PositionId, f64> = IndexMap::new();

        for (item, w_var) in decision_items.iter().zip(w_vars) {
            let w_star = solution.value(w_var.var) + w_var.offset;
            let w0 = current_weights
                .get(&item.position_id)
                .copied()
                .unwrap_or(0.0);
            optimal_weights.insert(item.position_id.clone(), w_star);
            weight_deltas.insert(item.position_id.clone(), w_star - w0);
        }

        // Implied quantities
        let mut implied_quantities: IndexMap<PositionId, f64> = IndexMap::new();
        let reconstruction_denominator =
            Self::reconstruction_denominator(problem.weighting, denominators);
        for (item, feat) in decision_items.iter().zip(decision_features) {
            let w_star = optimal_weights
                .get(&item.position_id)
                .copied()
                .unwrap_or(0.0);
            let qty = match problem.weighting {
                WeightingScheme::NotionalWeight => w_star * reconstruction_denominator,
                WeightingScheme::ValueWeight => {
                    if feat.pv_per_unit.abs() > PV_PER_UNIT_TOL {
                        (w_star * reconstruction_denominator) / feat.pv_per_unit
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

        // Objective value at solution: a · w*
        let mut objective_acc = NeumaierAccumulator::new();
        for (coef, w_var) in rows.coeffs_objective.iter().zip(w_vars) {
            let w_star = solution.value(w_var.var) + w_var.offset;
            objective_acc.add(*coef * w_star);
        }
        let objective_value = objective_acc.current();

        // Evaluate additional metric expressions of interest (for now: just objective).
        let mut metric_values: IndexMap<String, f64> = IndexMap::new();
        metric_values.insert("objective".to_string(), objective_value);

        // Constraint slacks
        let mut constraint_slacks: IndexMap<String, f64> = IndexMap::new();
        for lc in &rows.lp_constraints {
            if lc.is_turnover_placeholder {
                continue;
            }
            if let Some(name) = &lc.name {
                let mut lhs_val = 0.0;
                for (var, coef) in w_vars.iter().zip(&lc.coefficients) {
                    lhs_val += *coef * (solution.value(var.var) + var.offset);
                }

                let slack = match lc.relation {
                    Inequality::Le => lc.rhs - lhs_val,
                    Inequality::Ge => lhs_val - lc.rhs,
                    Inequality::Eq => (lhs_val - lc.rhs).abs(),
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

        let dual_values: IndexMap<String, f64> = IndexMap::new();

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
            dual_values,
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
        // Step 0: Validate portfolio and problem basics.
        problem.portfolio.validate()?;

        // Ensure there is at least one budget constraint, or we will add one with rhs=1.0.
        let has_budget = problem
            .constraints
            .iter()
            .any(|c| matches!(c, Constraint::Budget { .. }));

        // Step 1: Discover required metrics and value portfolio.
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

        // Step 2: Build decision space.
        let (decision_items, mut decision_features, current_weights, denominators) =
            build_decision_space(problem, &valuation, &required_metrics, market, config)?;

        if decision_items.is_empty() {
            return Err(Error::invalid_input(
                "no decision variables in optimization problem",
            ));
        }

        let n_vars = decision_items.len();

        // Step 3: Apply weight bounds from WeightBounds constraints.
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

        // Steps 4-5: Build objective coefficients and LP constraint rows.
        let rows = Self::build_lp_rows(
            problem,
            &decision_features,
            &decision_items,
            n_vars,
            has_budget,
        )?;

        // Step 6: Declare variables, assemble the model, and solve.
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
                // On failure the result carries `objective_value = NaN` and
                // empty maps — callers must check `status.is_feasible()`.
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
                    dual_values: IndexMap::new(),
                    constraint_slacks: IndexMap::new(),
                    meta,
                });
            }
        };

        // Post-solve: reconstruct the portfolio-level result.
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
}
