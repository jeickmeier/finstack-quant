"""Structural credit model bindings."""

from __future__ import annotations

from finstack_quant.finstack_quant import valuations as _valuations

MertonModel = _valuations.credit.MertonModel
DynamicRecoverySpec = _valuations.credit.DynamicRecoverySpec
EndogenousHazardSpec = _valuations.credit.EndogenousHazardSpec
CreditState = _valuations.credit.CreditState
ToggleExerciseModel = _valuations.credit.ToggleExerciseModel

__all__ = [
    "CreditState",
    "DynamicRecoverySpec",
    "EndogenousHazardSpec",
    "MertonModel",
    "ToggleExerciseModel",
]
