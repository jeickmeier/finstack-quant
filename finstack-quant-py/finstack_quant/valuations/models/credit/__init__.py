"""Structural credit model bindings.

Mirrors ``finstack_quant_valuations::models::credit``.

Examples:
--------
>>> import finstack_quant.valuations.models.credit as credit
>>> credit.__name__
'finstack_quant.valuations.models.credit'
"""

from __future__ import annotations

from finstack_quant.finstack_quant import valuations as _valuations

MertonModel = _valuations.models.credit.MertonModel
DynamicRecoverySpec = _valuations.models.credit.DynamicRecoverySpec
EndogenousHazardSpec = _valuations.models.credit.EndogenousHazardSpec
CreditState = _valuations.models.credit.CreditState
ToggleExerciseModel = _valuations.models.credit.ToggleExerciseModel

__all__ = [
    "CreditState",
    "DynamicRecoverySpec",
    "EndogenousHazardSpec",
    "MertonModel",
    "ToggleExerciseModel",
]
