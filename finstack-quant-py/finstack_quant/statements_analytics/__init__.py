"""Statement analysis: sensitivity, variance, scenarios, backtesting, and more.

Goal seek, DCF, corporate analysis, reports, and introspection.
Bindings for the ``finstack-quant-statements-analytics`` Rust crate.

Examples:
--------
>>> from finstack_quant.statements_analytics import backtest_forecast
>>> backtest_forecast([1.0, 2.0], [1.0, 2.5]).n
2

"""

from finstack_quant.finstack_quant import statements_analytics as _sa

SensitivityConfig = _sa.SensitivityConfig
VarianceConfig = _sa.VarianceConfig
ScenarioSet = _sa.ScenarioSet
SensitivityResult = _sa.SensitivityResult
TornadoEntry = _sa.TornadoEntry
VarianceRow = _sa.VarianceRow
VarianceReport = _sa.VarianceReport
ScenarioResults = _sa.ScenarioResults
ScenarioDiff = _sa.ScenarioDiff
BridgeStep = _sa.BridgeStep
BridgeChart = _sa.BridgeChart
run_sensitivity = _sa.run_sensitivity
generate_tornado_entries = _sa.generate_tornado_entries
run_variance = _sa.run_variance
evaluate_scenario_set = _sa.evaluate_scenario_set
scenario_diff = _sa.scenario_diff
variance_bridge = _sa.variance_bridge
backtest_forecast = _sa.backtest_forecast
goal_seek = _sa.goal_seek
evaluate_dcf = _sa.evaluate_dcf
dcf_sensitivity = _sa.dcf_sensitivity
evaluate_lbo = _sa.evaluate_lbo
wacc = _sa.wacc
run_corporate_analysis = _sa.run_corporate_analysis
pl_summary_report_text = _sa.pl_summary_report_text
credit_assessment_report_text = _sa.credit_assessment_report_text
credit_assessment = _sa.credit_assessment
DependencyTracer = _sa.DependencyTracer
DependencyTree = _sa.DependencyTree
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
add_property_operating_statement = _sa.add_property_operating_statement

# Typed results, specs and mappings
CompanyMetrics = _sa.CompanyMetrics
CorporateAnalysis = _sa.CorporateAnalysis
CorporateValuationResult = _sa.CorporateValuationResult
CreditAssessment = _sa.CreditAssessment
CreditAssessmentPoint = _sa.CreditAssessmentPoint
CreditMapping = _sa.CreditMapping
DcfSensitivityResult = _sa.DcfSensitivityResult
DimensionScore = _sa.DimensionScore
EclBucket = _sa.EclBucket
EclResult = _sa.EclResult
EquityBridge = _sa.EquityBridge
Explanation = _sa.Explanation
ExplanationStep = _sa.ExplanationStep
ForecastMetrics = _sa.ForecastMetrics
GoalSeekResult = _sa.GoalSeekResult
LboCheckMappings = _sa.LboCheckMappings
LboResult = _sa.LboResult
PLSummaryReport = _sa.PLSummaryReport
ParameterSpec = _sa.ParameterSpec
PeerFilter = _sa.PeerFilter
PeerSet = _sa.PeerSet
PeerStats = _sa.PeerStats
QualitativeFlags = _sa.QualitativeFlags
RegressionResult = _sa.RegressionResult
RelativeValueResult = _sa.RelativeValueResult
ScoringDimension = _sa.ScoringDimension
Stage = _sa.Stage
StageResult = _sa.StageResult
StagingConfig = _sa.StagingConfig
TerminalValueSpec = _sa.TerminalValueSpec
ThreeStatementMapping = _sa.ThreeStatementMapping
ValuationDiscounts = _sa.ValuationDiscounts
WeightedEclResult = _sa.WeightedEclResult
pl_summary_report = _sa.pl_summary_report

__all__: list[str] = [
    "AccountType",
    "BridgeChart",
    "BridgeStep",
    "CompanyMetrics",
    "CorkscrewAccount",
    "CorkscrewConfig",
    "CorkscrewExtension",
    "CorkscrewReport",
    "CorporateAnalysis",
    "CorporateValuationResult",
    "CreditAssessment",
    "CreditAssessmentPoint",
    "CreditMapping",
    "CreditScorecardExtension",
    "DcfSensitivityResult",
    "DependencyTracer",
    "DependencyTree",
    "DimensionScore",
    "EclBucket",
    "EclResult",
    "EquityBridge",
    "Explanation",
    "ExplanationStep",
    "Exposure",
    "ForecastMetrics",
    "FreeRentWindowSpec",
    "GoalSeekResult",
    "LboCheckMappings",
    "LboResult",
    "LeaseGrowthConvention",
    "LeaseSpec",
    "ManagementFeeBase",
    "ManagementFeeSpec",
    "PLSummaryReport",
    "ParameterSpec",
    "PeerFilter",
    "PeerSet",
    "PeerStats",
    "PropertyTemplateNodes",
    "QualitativeFlags",
    "RegressionResult",
    "RelativeValueResult",
    "RenewalSpec",
    "RentRollOutputNodes",
    "RentStepSpec",
    "ScenarioDiff",
    "ScenarioResults",
    "ScenarioSet",
    "ScorecardConfig",
    "ScorecardMetric",
    "ScorecardReport",
    "ScoringDimension",
    "SensitivityConfig",
    "SensitivityResult",
    "Stage",
    "StageResult",
    "StagingConfig",
    "TerminalValueSpec",
    "ThreeStatementMapping",
    "TornadoEntry",
    "ValuationDiscounts",
    "VarianceConfig",
    "VarianceReport",
    "VarianceRow",
    "WeightedEclResult",
    "add_ncf_buildup",
    "add_noi_buildup",
    "add_property_operating_statement",
    "add_rent_roll",
    "add_roll_forward",
    "add_roll_forward_with_opening",
    "add_vintage_buildup",
    "backtest_forecast",
    "classify_stage",
    "compute_ecl",
    "compute_ecl_weighted",
    "compute_multiple",
    "credit_assessment",
    "credit_assessment_report_text",
    "dcf_sensitivity",
    "evaluate_dcf",
    "evaluate_lbo",
    "evaluate_scenario_set",
    "explain_formula",
    "explain_formula_text",
    "generate_tornado_entries",
    "goal_seek",
    "peer_stats",
    "percentile_rank",
    "pl_summary_report",
    "pl_summary_report_text",
    "regression_fair_value",
    "render_check_report_html",
    "render_check_report_text",
    "run_checks",
    "run_corporate_analysis",
    "run_credit_underwriting_checks",
    "run_sensitivity",
    "run_three_statement_checks",
    "run_variance",
    "scenario_diff",
    "score_relative_value",
    "validate_scorecard_config",
    "variance_bridge",
    "wacc",
    "z_score",
]
