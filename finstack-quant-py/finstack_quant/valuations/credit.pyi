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
    """Structural credit model based on asset value, asset volatility, and debt barrier."""

    def __init__(
        self,
        asset_value: float,
        asset_vol: float,
        debt_barrier: float,
        risk_free_rate: float,
    ) -> None:
        """Create a Merton-style structural credit model."""
        ...

    @staticmethod
    def credit_grades(
        equity_value: float,
        equity_vol: float,
        total_debt: float,
        risk_free_rate: float,
        barrier_uncertainty: float,
        mean_recovery: float,
    ) -> MertonModel:
        """Build the CreditGrades-style structural model from equity inputs."""
        ...

    @staticmethod
    def from_json(json: str) -> MertonModel:
        """Deserialize a structural credit model from JSON."""
        ...

    def to_json(self) -> str:
        """Serialize this model to canonical JSON."""
        ...

    def distance_to_default(self, horizon: float) -> float:
        """Return distance to default over ``horizon`` years."""
        ...

    def default_probability(self, horizon: float) -> float:
        """Return risk-neutral default probability over ``horizon`` years."""
        ...

    def implied_spread(self, horizon: float, recovery: float) -> float:
        """Return an implied credit spread for ``horizon`` and recovery rate."""
        ...

class DynamicRecoverySpec:
    """Recovery model used by credit instruments with notional-dependent recovery."""

    @staticmethod
    def constant(recovery: float) -> DynamicRecoverySpec:
        """Create a constant recovery-rate specification."""
        ...

    @staticmethod
    def from_json(json: str) -> DynamicRecoverySpec:
        """Deserialize a recovery specification from JSON."""
        ...

    def to_json(self) -> str:
        """Serialize this recovery specification to canonical JSON."""
        ...

    def recovery_at_notional(self, notional: float) -> float:
        """Return recovery rate for the supplied notional."""
        ...

class EndogenousHazardSpec:
    """Hazard-rate model driven by leverage or PIK-accreted notional."""

    @staticmethod
    def power_law(
        base_hazard: float,
        base_leverage: float,
        exponent: float,
    ) -> EndogenousHazardSpec:
        """Create a power-law hazard model around a base leverage point."""
        ...

    @staticmethod
    def from_json(json: str) -> EndogenousHazardSpec:
        """Deserialize an endogenous hazard specification from JSON."""
        ...

    def to_json(self) -> str:
        """Serialize this hazard specification to canonical JSON."""
        ...

    def hazard_at_leverage(self, leverage: float) -> float:
        """Return hazard rate at the supplied leverage."""
        ...

    def hazard_after_pik_accrual(
        self,
        accreted_notional: float,
        asset_value: float,
    ) -> float:
        """Return hazard rate after PIK accrual changes leverage."""
        ...

class CreditState:
    """Point-in-time credit state used by toggle and path-dependent credit logic."""

    def __init__(
        self,
        hazard_rate: float = 0.0,
        distance_to_default: float | None = None,
        leverage: float = 0.0,
        accreted_notional: float = 0.0,
        coupon_due: float = 0.0,
        asset_value: float | None = None,
    ) -> None:
        """Create a credit-state snapshot."""
        ...

    def to_json(self) -> str:
        """Serialize this state to canonical JSON."""
        ...

class ToggleExerciseModel:
    """Exercise model for credit toggle features."""

    @staticmethod
    def threshold(
        variable: str,
        threshold: float,
        direction: str,
    ) -> ToggleExerciseModel:
        """Create a threshold exercise rule.

        ``variable`` selects the state variable and ``direction`` is typically
        ``"above"`` or ``"below"``.
        """
        ...

    @staticmethod
    def optimal(
        nested_paths: int,
        equity_discount_rate: float,
        asset_vol: float,
        risk_free_rate: float,
        horizon: float,
    ) -> ToggleExerciseModel:
        """Create an optimal exercise model from nested-path parameters."""
        ...

    @staticmethod
    def from_json(json: str) -> ToggleExerciseModel:
        """Deserialize a toggle exercise model from JSON."""
        ...

    def to_json(self) -> str:
        """Serialize this exercise model to canonical JSON."""
        ...
