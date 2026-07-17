"""Credit factor hierarchy artifacts, calibration, and decomposition.

Examples:
--------
>>> import finstack_quant.factor_model.credit as credit
>>> credit.__name__
'finstack_quant.factor_model.credit'
"""

from __future__ import annotations

from finstack_quant.finstack_quant import factor_model as _factor_model

_credit = _factor_model.credit

CreditFactorModel = _credit.CreditFactorModel
CreditCalibrator = _credit.CreditCalibrator
LevelsAtDate = _credit.LevelsAtDate
PeriodDecomposition = _credit.PeriodDecomposition
FactorCovarianceForecast = _credit.FactorCovarianceForecast
decompose_levels = _credit.decompose_levels
decompose_period = _credit.decompose_period

__all__: list[str] = [
    "CreditCalibrator",
    "CreditFactorModel",
    "FactorCovarianceForecast",
    "LevelsAtDate",
    "PeriodDecomposition",
    "decompose_levels",
    "decompose_period",
]
