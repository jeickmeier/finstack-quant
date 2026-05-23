"""Factor-model primitives, calibration, and decomposition.

Bindings for the ``finstack-factor-model`` Rust crate. Credit hierarchy
calibration lives under :mod:`finstack.factor_model.credit`; root aliases are
kept for parity with Rust crate-root re-exports.
"""

from __future__ import annotations

from finstack.factor_model import credit as credit
from finstack.finstack import factor_model as _factor_model

CreditFactorModel = _factor_model.CreditFactorModel
CreditCalibrator = _factor_model.CreditCalibrator
LevelsAtDate = _factor_model.LevelsAtDate
PeriodDecomposition = _factor_model.PeriodDecomposition
FactorCovarianceForecast = _factor_model.FactorCovarianceForecast
decompose_levels = _factor_model.decompose_levels
decompose_period = _factor_model.decompose_period

__all__: list[str] = [
    "CreditCalibrator",
    "CreditFactorModel",
    "FactorCovarianceForecast",
    "LevelsAtDate",
    "PeriodDecomposition",
    "credit",
    "decompose_levels",
    "decompose_period",
]
