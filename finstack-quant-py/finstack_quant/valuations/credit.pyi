"""Type stubs for ``finstack_quant.valuations.credit``."""

from __future__ import annotations

__all__ = [
    "MertonModel",
    "DynamicRecoverySpec",
    "EndogenousHazardSpec",
    "CreditState",
    "ToggleExerciseModel",
]

class MertonModel:
    def __init__(
        self,
        asset_value: float,
        asset_vol: float,
        debt_barrier: float,
        risk_free_rate: float,
    ) -> None: ...
    @staticmethod
    def credit_grades(
        equity_value: float,
        equity_vol: float,
        total_debt: float,
        risk_free_rate: float,
        barrier_uncertainty: float,
        mean_recovery: float,
    ) -> MertonModel: ...
    @staticmethod
    def from_json(json: str) -> MertonModel: ...
    def to_json(self) -> str: ...
    def distance_to_default(self, horizon: float) -> float: ...
    def default_probability(self, horizon: float) -> float: ...
    def implied_spread(self, horizon: float, recovery: float) -> float: ...

class DynamicRecoverySpec:
    @staticmethod
    def constant(recovery: float) -> DynamicRecoverySpec: ...
    @staticmethod
    def from_json(json: str) -> DynamicRecoverySpec: ...
    def to_json(self) -> str: ...
    def recovery_at_notional(self, notional: float) -> float: ...

class EndogenousHazardSpec:
    @staticmethod
    def power_law(
        base_hazard: float,
        base_leverage: float,
        exponent: float,
    ) -> EndogenousHazardSpec: ...
    @staticmethod
    def from_json(json: str) -> EndogenousHazardSpec: ...
    def to_json(self) -> str: ...
    def hazard_at_leverage(self, leverage: float) -> float: ...
    def hazard_after_pik_accrual(
        self,
        accreted_notional: float,
        asset_value: float,
    ) -> float: ...

class CreditState:
    def __init__(
        self,
        hazard_rate: float = 0.0,
        distance_to_default: float | None = None,
        leverage: float = 0.0,
        accreted_notional: float = 0.0,
        coupon_due: float = 0.0,
        asset_value: float | None = None,
    ) -> None: ...
    def to_json(self) -> str: ...

class ToggleExerciseModel:
    @staticmethod
    def threshold(
        variable: str,
        threshold: float,
        direction: str,
    ) -> ToggleExerciseModel: ...
    @staticmethod
    def optimal(
        nested_paths: int,
        equity_discount_rate: float,
        asset_vol: float,
        risk_free_rate: float,
        horizon: float,
    ) -> ToggleExerciseModel: ...
    @staticmethod
    def from_json(json: str) -> ToggleExerciseModel: ...
    def to_json(self) -> str: ...
