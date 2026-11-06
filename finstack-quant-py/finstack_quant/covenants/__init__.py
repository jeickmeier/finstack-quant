"""Typed covenant definitions, engine evaluation, templates and forecasting.

Examples:
--------
>>> from finstack_quant.covenants import CovenantEngine, CovenantTemplate, covenant_template
>>> engine = CovenantEngine.from_specs(covenant_template(CovenantTemplate.cov_lite(7.0, 4.5)))
>>> reports = engine.evaluate({"total_leverage": 5.0, "senior_leverage": 3.0}, "2026-03-31")
>>> reports["max_total_leverage"].passed
True
"""

from finstack_quant.finstack_quant import covenants as _covenants

Covenant = _covenants.Covenant
CovenantBreach = _covenants.CovenantBreach
CovenantConsequence = _covenants.CovenantConsequence
CovenantEngine = _covenants.CovenantEngine
CovenantForecast = _covenants.CovenantForecast
CovenantForecastConfig = _covenants.CovenantForecastConfig
CovenantReport = _covenants.CovenantReport
CovenantSpec = _covenants.CovenantSpec
CovenantTemplate = _covenants.CovenantTemplate
CovenantType = _covenants.CovenantType
CovenantWaiver = _covenants.CovenantWaiver
FutureBreach = _covenants.FutureBreach
SpringingCondition = _covenants.SpringingCondition
ThresholdSchedule = _covenants.ThresholdSchedule
breaches_to_dataframe = _covenants.breaches_to_dataframe
covenant_template = _covenants.covenant_template
evaluate_engine = _covenants.evaluate_engine
forecast_breaches = _covenants.forecast_breaches
forecast_covenant = _covenants.forecast_covenant
reports_to_dataframe = _covenants.reports_to_dataframe
validate_covenant_engine_json = _covenants.validate_covenant_engine_json
validate_covenant_report_json = _covenants.validate_covenant_report_json
validate_covenant_spec_json = _covenants.validate_covenant_spec_json

__all__ = [
    "Covenant",
    "CovenantBreach",
    "CovenantConsequence",
    "CovenantEngine",
    "CovenantForecast",
    "CovenantForecastConfig",
    "CovenantReport",
    "CovenantSpec",
    "CovenantTemplate",
    "CovenantType",
    "CovenantWaiver",
    "FutureBreach",
    "SpringingCondition",
    "ThresholdSchedule",
    "breaches_to_dataframe",
    "covenant_template",
    "evaluate_engine",
    "forecast_breaches",
    "forecast_covenant",
    "reports_to_dataframe",
    "validate_covenant_engine_json",
    "validate_covenant_report_json",
    "validate_covenant_spec_json",
]
