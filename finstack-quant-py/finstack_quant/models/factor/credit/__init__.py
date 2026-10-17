"""Credit factor hierarchy artifacts, calibration, and decomposition.

Examples:
--------
>>> from finstack_quant.models.factor.credit import CreditFactorModel
>>> try:
...     CreditFactorModel.from_json("{}")
... except ValueError as exc:
...     "missing field" in str(exc)
True
"""

from finstack_quant.finstack_quant import models as _models

_credit = _models.factor.credit

CreditFactorModel = _credit.CreditFactorModel
CreditCalibrator = _credit.CreditCalibrator
LevelsAtDate = _credit.LevelsAtDate
PeriodDecomposition = _credit.PeriodDecomposition
FactorCovarianceForecast = _credit.FactorCovarianceForecast
VolHorizon = _credit.VolHorizon
decompose_levels = _credit.decompose_levels
decompose_period = _credit.decompose_period

__all__: list[str] = [
    "CreditCalibrator",
    "CreditFactorModel",
    "FactorCovarianceForecast",
    "LevelsAtDate",
    "PeriodDecomposition",
    "VolHorizon",
    "decompose_levels",
    "decompose_period",
]
