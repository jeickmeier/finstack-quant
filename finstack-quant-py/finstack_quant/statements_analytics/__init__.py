"""Statement analysis: sensitivity, variance, scenarios, backtesting, and more.

Goal seek, DCF, corporate analysis, Monte Carlo, reports, and introspection.
Bindings for the ``finstack-quant-statements-analytics`` Rust crate.
"""

from __future__ import annotations

from finstack_quant.finstack_quant import statements_analytics as _sa

run_sensitivity = _sa.run_sensitivity
generate_tornado_entries = _sa.generate_tornado_entries
run_variance = _sa.run_variance
evaluate_scenario_set = _sa.evaluate_scenario_set
run_monte_carlo = _sa.run_monte_carlo
backtest_forecast = _sa.backtest_forecast
goal_seek = _sa.goal_seek
evaluate_dcf = _sa.evaluate_dcf
run_corporate_analysis = _sa.run_corporate_analysis
pl_summary_report = _sa.pl_summary_report
credit_assessment_report = _sa.credit_assessment_report
credit_assessment = _sa.credit_assessment
DependencyTracer = _sa.DependencyTracer
direct_dependencies = _sa.direct_dependencies
all_dependencies = _sa.all_dependencies
dependents = _sa.dependents
explain_formula = _sa.explain_formula
explain_formula_text = _sa.explain_formula_text
run_checks = _sa.run_checks
run_three_statement_checks = _sa.run_three_statement_checks
run_credit_underwriting_checks = _sa.run_credit_underwriting_checks
render_check_report_text = _sa.render_check_report_text
render_check_report_html = _sa.render_check_report_html

# ECL / IFRS 9 / CECL
Exposure = _sa.Exposure
classify_stage = _sa.classify_stage
compute_ecl = _sa.compute_ecl
compute_ecl_weighted = _sa.compute_ecl_weighted

# Comparable-company analysis
percentile_rank = _sa.percentile_rank
z_score = _sa.z_score
peer_stats = _sa.peer_stats
regression_fair_value = _sa.regression_fair_value
compute_multiple = _sa.compute_multiple
score_relative_value = _sa.score_relative_value

# Credit scorecards
ScorecardMetric = _sa.ScorecardMetric
ScorecardConfig = _sa.ScorecardConfig
ScorecardReport = _sa.ScorecardReport
CreditScorecardExtension = _sa.CreditScorecardExtension
validate_scorecard_config = _sa.validate_scorecard_config

# Corkscrew roll-forwards
AccountType = _sa.AccountType
CorkscrewAccount = _sa.CorkscrewAccount
CorkscrewConfig = _sa.CorkscrewConfig
CorkscrewReport = _sa.CorkscrewReport
CorkscrewExtension = _sa.CorkscrewExtension

# Vintage cohort buildup
add_vintage_buildup = _sa.add_vintage_buildup

# Roll-forward template
add_roll_forward = _sa.add_roll_forward
add_roll_forward_with_opening = _sa.add_roll_forward_with_opening

# Real estate templates
SimpleLeaseSpec = _sa.SimpleLeaseSpec
RentStepSpec = _sa.RentStepSpec
FreeRentWindowSpec = _sa.FreeRentWindowSpec
RenewalSpec = _sa.RenewalSpec
LeaseGrowthConvention = _sa.LeaseGrowthConvention
LeaseSpec = _sa.LeaseSpec
RentRollOutputNodes = _sa.RentRollOutputNodes
ManagementFeeBase = _sa.ManagementFeeBase
ManagementFeeSpec = _sa.ManagementFeeSpec
PropertyTemplateNodes = _sa.PropertyTemplateNodes
add_noi_buildup = _sa.add_noi_buildup
add_ncf_buildup = _sa.add_ncf_buildup
add_rent_roll = _sa.add_rent_roll
add_rent_roll_rental_revenue = _sa.add_rent_roll_rental_revenue
add_property_operating_statement = _sa.add_property_operating_statement

__all__: list[str] = [
    "AccountType",
    "CorkscrewAccount",
    "CorkscrewConfig",
    "CorkscrewExtension",
    "CorkscrewReport",
    "CreditScorecardExtension",
    "DependencyTracer",
    "Exposure",
    "FreeRentWindowSpec",
    "LeaseGrowthConvention",
    "LeaseSpec",
    "ManagementFeeBase",
    "ManagementFeeSpec",
    "PropertyTemplateNodes",
    "RenewalSpec",
    "RentRollOutputNodes",
    "RentStepSpec",
    "ScorecardConfig",
    "ScorecardMetric",
    "ScorecardReport",
    "SimpleLeaseSpec",
    "add_ncf_buildup",
    "add_noi_buildup",
    "add_property_operating_statement",
    "add_rent_roll",
    "add_rent_roll_rental_revenue",
    "add_roll_forward",
    "add_roll_forward_with_opening",
    "add_vintage_buildup",
    "all_dependencies",
    "backtest_forecast",
    "classify_stage",
    "compute_ecl",
    "compute_ecl_weighted",
    "compute_multiple",
    "credit_assessment",
    "credit_assessment_report",
    "dependents",
    "direct_dependencies",
    "evaluate_dcf",
    "evaluate_scenario_set",
    "explain_formula",
    "explain_formula_text",
    "generate_tornado_entries",
    "goal_seek",
    "peer_stats",
    "percentile_rank",
    "pl_summary_report",
    "regression_fair_value",
    "render_check_report_html",
    "render_check_report_text",
    "run_checks",
    "run_corporate_analysis",
    "run_credit_underwriting_checks",
    "run_monte_carlo",
    "run_sensitivity",
    "run_three_statement_checks",
    "run_variance",
    "score_relative_value",
    "validate_scorecard_config",
    "z_score",
]
