"""Structural credit model bindings.

Mirrors ``finstack_quant_models::credit``.

Examples:
--------
>>> from finstack_quant.models.credit import MertonModel
>>> round(MertonModel(100.0, 0.25, 80.0, 0.05).default_probability(1.0), 6)
0.166629

"""

import sys as _sys

from finstack_quant.finstack_quant import models as _models

MertonModel = _models.credit.MertonModel
AssetDynamics = _models.credit.AssetDynamics
MertonBarrierType = _models.credit.MertonBarrierType
SimulatedPaths = _models.credit.SimulatedPaths
DynamicRecoverySpec = _models.credit.DynamicRecoverySpec
EndogenousHazardSpec = _models.credit.EndogenousHazardSpec
CreditState = _models.credit.CreditState
ToggleExerciseModel = _models.credit.ToggleExerciseModel
RatingFactorTable = _models.credit.RatingFactorTable
moodys_warf_factor = _models.credit.moodys_warf_factor
lgd = _models.credit.lgd
liability_management = _models.credit.liability_management
migration = _models.credit.migration
pd = _models.credit.pd
recovery_waterfall = _models.credit.recovery_waterfall
scoring = _models.credit.scoring

for _name, _module in {
    "lgd": lgd,
    "liability_management": liability_management,
    "migration": migration,
    "pd": pd,
    "recovery_waterfall": recovery_waterfall,
    "scoring": scoring,
}.items():
    _sys.modules.setdefault(f"finstack_quant.models.credit.{_name}", _module)

__all__ = [
    "AssetDynamics",
    "CreditState",
    "DynamicRecoverySpec",
    "EndogenousHazardSpec",
    "MertonBarrierType",
    "MertonModel",
    "RatingFactorTable",
    "SimulatedPaths",
    "ToggleExerciseModel",
    "lgd",
    "liability_management",
    "migration",
    "moodys_warf_factor",
    "pd",
    "recovery_waterfall",
    "scoring",
]
