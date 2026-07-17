"""
Structural credit models and path-dependent credit specifications.

Bindings for Merton-style structural models, dynamic recovery, endogenous hazard
rates, credit-state snapshots, and toggle exercise rules used by PIK/toggle
bonds and similar instruments.

Examples
--------
>>> import finstack_quant.valuations.models.credit as credit
>>> credit.__name__
'finstack_quant.valuations.models.credit'
"""

from __future__ import annotations

__all__ = [
    "MertonModel",
    "DynamicRecoverySpec",
    "EndogenousHazardSpec",
    "CreditState",
    "ToggleExerciseModel",
]

class MertonModel:
    """
    Merton (1974) structural credit model with optional CreditGrades calibration.

    Firm value follows geometric Brownian motion under the risk-neutral measure;
    default occurs when asset value crosses a debt barrier at horizon. Spreads
    and default probabilities are risk-neutral.

    Examples
    --------
    >>> from finstack_quant.valuations.models.credit import MertonModel
    >>> model = MertonModel(100.0, 0.25, 80.0, 0.05)
    >>> model.default_probability(1.0)  # doctest: +SKIP
    """

    def __init__(
        self,
        asset_value: float,
        asset_vol: float,
        debt_barrier: float,
        risk_free_rate: float,
    ) -> None:
        """
        Construct a Merton structural model from firm asset inputs.

        Parameters
        ----------
        asset_value : float
            Firm asset value (positive, finite).
        asset_vol : float
            Annualized asset volatility as a decimal (e.g. ``0.30`` for 30%).
        debt_barrier : float
            Default barrier, typically total debt face value.
        risk_free_rate : float
            Continuously compounded risk-free rate as a decimal.

        Raises
        ------
        ValueError
            If inputs are non-finite or out of range.

        Sources
        -------
        See ``docs/REFERENCES.md#merton-1974``.
        """
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
        """
        Build a CreditGrades-style model calibrated from equity inputs.

        Inverts the structural mapping from observable equity value and volatility
        to implied firm asset value and asset volatility, with barrier uncertainty
        and mean recovery governing the default boundary.

        Parameters
        ----------
        equity_value : float
            Market equity value (positive, finite).
        equity_vol : float
            Equity volatility as a decimal.
        total_debt : float
            Total debt face used as the reference barrier scale.
        risk_free_rate : float
            Continuously compounded risk-free rate as a decimal.
        barrier_uncertainty : float
            Barrier uncertainty parameter (CreditGrades ``alpha`` scale).
        mean_recovery : float
            Expected recovery rate as a decimal in ``[0, 1]``.

        Returns
        -------
        MertonModel
            Calibrated structural model.

        Raises
        ------
        ValueError
            If inputs are non-finite or violate model constraints.

        Sources
        -------
        See ``docs/REFERENCES.md#merton-1974`` and ``docs/REFERENCES.md#o-kane-2008``.

        Examples
        --------
        >>> from finstack_quant.valuations.models.credit import MertonModel
        >>> callable(MertonModel.credit_grades)
        True
        """
        ...

    @staticmethod
    def from_json(json: str) -> MertonModel:
        """
        Deserialize a structural credit model from JSON.

        Parameters
        ----------
        json : str
            Canonical JSON payload for ``MertonModel``.

        Returns
        -------
        MertonModel
            Parsed model instance.

        Raises
        ------
        ValueError
            If JSON is malformed or fails validation.

        Examples
        --------
        >>> from finstack_quant.valuations.models.credit import MertonModel
        >>> callable(MertonModel.from_json)
        True
        """
        ...

    def to_json(self) -> str:
        """
        Serialize this model to pretty-printed canonical JSON.

        Returns
        -------
        str
            JSON string.
        """
        ...

    def distance_to_default(self, horizon: float) -> float:
        """
        Return risk-neutral distance to default at ``horizon`` years.

        Parameters
        ----------
        horizon : float
            Horizon in years (positive, finite).

        Returns
        -------
        float
            Distance-to-default statistic (standard-deviation units).

        Raises
        ------
        ValueError
            If supplied inputs violate the documented type, shape, finite-value, or domain constraints.
        """
        ...

    def default_probability(self, horizon: float) -> float:
        """
        Return risk-neutral default probability over ``horizon`` years.

        Parameters
        ----------
        horizon : float
            Horizon in years (positive, finite).

        Returns
        -------
        float
            Default probability in ``[0, 1]``.

        Raises
        ------
        ValueError
            If supplied inputs violate the documented type, shape, finite-value, or domain constraints.
        """
        ...

    def implied_spread(self, horizon: float, recovery: float) -> float:
        """
        Return implied CDS par spread for ``horizon`` and ``recovery``.

        Parameters
        ----------
        horizon : float
            CDS horizon in years.
        recovery : float
            Assumed recovery rate as a decimal (not basis points).

        Returns
        -------
        float
            Implied par spread as a decimal (e.g. ``0.012`` for 120 bps).

        Raises
        ------
        ValueError
            If ``horizon`` or ``recovery`` are invalid.

        Sources
        -------
        See ``docs/REFERENCES.md#o-kane-2008`` for CDS spread conventions.
        """
        ...

class DynamicRecoverySpec:
    """
    Recovery specification with optional notional dependence.

    Examples
    --------
    >>> from finstack_quant.valuations.models.credit import DynamicRecoverySpec
    >>> DynamicRecoverySpec.__name__
    'DynamicRecoverySpec'
    """

    @staticmethod
    def constant(recovery: float) -> DynamicRecoverySpec:
        """
        Create a constant recovery-rate specification.

        Parameters
        ----------
        recovery : float
            Recovery rate as a decimal in ``[0, 1]``.

        Returns
        -------
        DynamicRecoverySpec
            Constant recovery spec.

        Raises
        ------
        ValueError
            If ``recovery`` is out of range or non-finite.

        Examples
        --------
        >>> from finstack_quant.valuations.models.credit import DynamicRecoverySpec
        >>> callable(DynamicRecoverySpec.constant)
        True
        """
        ...

    @staticmethod
    def from_json(json: str) -> DynamicRecoverySpec:
        """
        Deserialize a recovery specification from JSON.

        Parameters
        ----------
        json : str
            Canonical JSON payload.

        Returns
        -------
        DynamicRecoverySpec
            Parsed specification.

        Raises
        ------
        ValueError
            If JSON is invalid.

        Examples
        --------
        >>> from finstack_quant.valuations.models.credit import DynamicRecoverySpec
        >>> callable(DynamicRecoverySpec.from_json)
        True
        """
        ...

    def to_json(self) -> str:
        """
        Serialize this recovery specification to canonical JSON.

        Returns
        -------
        str
            JSON string.
        """
        ...

    def recovery_at_notional(self, notional: float) -> float:
        """
        Return recovery rate for the supplied notional.

        Parameters
        ----------
        notional : float
            Outstanding notional (positive, finite).

        Returns
        -------
        float
            Recovery rate as a decimal.

        Raises
        ------
        ValueError
            If supplied inputs violate the documented type, shape, finite-value, or domain constraints.
        """
        ...

class EndogenousHazardSpec:
    """
    Hazard-rate model driven by leverage or PIK-accreted notional.

    Examples
    --------
    >>> from finstack_quant.valuations.models.credit import EndogenousHazardSpec
    >>> EndogenousHazardSpec.__name__
    'EndogenousHazardSpec'
    """

    @staticmethod
    def power_law(
        base_hazard: float,
        base_leverage: float,
        exponent: float,
    ) -> EndogenousHazardSpec:
        """
        Create a power-law hazard model around a base leverage point.

        Parameters
        ----------
        base_hazard : float
            Hazard rate at ``base_leverage`` (decimal annualized intensity).
        base_leverage : float
            Reference leverage ratio (e.g. debt / EBITDA).
        exponent : float
            Power-law sensitivity of hazard to leverage.

        Returns
        -------
        EndogenousHazardSpec
            Endogenous hazard specification.

        Raises
        ------
        ValueError
            If parameters are non-finite or violate constraints.

        Examples
        --------
        >>> from finstack_quant.valuations.models.credit import EndogenousHazardSpec
        >>> callable(EndogenousHazardSpec.power_law)
        True
        """
        ...

    @staticmethod
    def from_json(json: str) -> EndogenousHazardSpec:
        """
        Deserialize an endogenous hazard specification from JSON.

        Parameters
        ----------
        json : str
            Canonical JSON payload.

        Returns
        -------
        EndogenousHazardSpec
            Parsed specification.

        Raises
        ------
        ValueError
            If JSON is invalid.

        Examples
        --------
        >>> from finstack_quant.valuations.models.credit import EndogenousHazardSpec
        >>> callable(EndogenousHazardSpec.from_json)
        True
        """
        ...

    def to_json(self) -> str:
        """
        Serialize this hazard specification to canonical JSON.

        Returns
        -------
        str
            JSON string.
        """
        ...

    def hazard_at_leverage(self, leverage: float) -> float:
        """
        Return hazard rate at the supplied leverage.

        Parameters
        ----------
        leverage : float
            Leverage ratio (positive, finite).

        Returns
        -------
        float
            Annualized hazard rate as a decimal.

        Raises
        ------
        ValueError
            If supplied inputs violate the documented type, shape, finite-value, or domain constraints.
        """
        ...

    def hazard_after_pik_accrual(
        self,
        accreted_notional: float,
        asset_value: float,
    ) -> float:
        """
        Return hazard rate after PIK accrual changes leverage.

        Parameters
        ----------
        accreted_notional : float
            PIK-accreted notional outstanding.
        asset_value : float
            Firm asset value used in the leverage mapping.

        Returns
        -------
        float
            Updated annualized hazard rate as a decimal.

        Raises
        ------
        ValueError
            If supplied inputs violate the documented type, shape, finite-value, or domain constraints.
        """
        ...

class CreditState:
    """
    Point-in-time credit state for toggle and path-dependent credit logic.

    Examples
    --------
    >>> from finstack_quant.valuations.models.credit import CreditState
    >>> CreditState.__name__
    'CreditState'
    """

    def __init__(
        self,
        hazard_rate: float = 0.0,
        distance_to_default: float | None = None,
        leverage: float = 0.0,
        accreted_notional: float = 0.0,
        coupon_due: float = 0.0,
        asset_value: float | None = None,
    ) -> None:
        """
        Create a credit-state snapshot.

        Parameters
        ----------
        hazard_rate : float, optional
            Instantaneous hazard rate as a decimal. Default ``0.0``.
        distance_to_default : float, optional
            Structural distance-to-default if available.
        leverage : float, optional
            Leverage ratio for endogenous hazard models. Default ``0.0``.
        accreted_notional : float, optional
            PIK-accreted notional. Default ``0.0``.
        coupon_due : float, optional
            Coupon amount due at the decision date. Default ``0.0``.
        asset_value : float, optional
            Firm asset value for structural/toggle models.

        Raises
        ------
        ValueError
            If supplied inputs violate the documented type, shape, finite-value, or domain constraints.
        """
        ...

    def to_json(self) -> str:
        """
        Serialize this state to canonical JSON.

        Returns
        -------
        str
            JSON string.
        """
        ...

class ToggleExerciseModel:
    """
    Exercise model for PIK/cash toggle and similar embedded options.

    Examples
    --------
    >>> from finstack_quant.valuations.models.credit import ToggleExerciseModel
    >>> ToggleExerciseModel.__name__
    'ToggleExerciseModel'
    """

    @staticmethod
    def threshold(
        variable: str,
        threshold: float,
        direction: str,
    ) -> ToggleExerciseModel:
        """
        Create a threshold exercise rule on a credit-state variable.

        Parameters
        ----------
        variable : str
            State variable name (e.g. ``"leverage"``, ``"distance_to_default"``).
        threshold : float
            Threshold value triggering exercise.
        direction : str
            ``"above"`` or ``"below"`` — exercise when the variable is above or
            below the threshold.

        Returns
        -------
        ToggleExerciseModel
            Threshold exercise specification.

        Raises
        ------
        ValueError
            If ``variable`` or ``direction`` is not recognized.

        Examples
        --------
        >>> from finstack_quant.valuations.models.credit import ToggleExerciseModel
        >>> callable(ToggleExerciseModel.threshold)
        True
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
        """
        Create an optimal exercise model from nested-path parameters.

        Parameters
        ----------
        nested_paths : int
            Number of nested Monte Carlo paths for the inner optimization.
        equity_discount_rate : float
            Equity-holder discount rate as a decimal.
        asset_vol : float
            Asset volatility as a decimal.
        risk_free_rate : float
            Risk-free rate as a decimal.
        horizon : float
            Exercise horizon in years.

        Returns
        -------
        ToggleExerciseModel
            Optimal exercise specification.

        Raises
        ------
        ValueError
            If supplied inputs violate the documented type, shape, finite-value, or domain constraints.

        Examples
        --------
        >>> from finstack_quant.valuations.models.credit import ToggleExerciseModel
        >>> callable(ToggleExerciseModel.optimal)
        True
        """
        ...

    @staticmethod
    def from_json(json: str) -> ToggleExerciseModel:
        """
        Deserialize a toggle exercise model from JSON.

        Parameters
        ----------
        json : str
            Canonical JSON payload.

        Returns
        -------
        ToggleExerciseModel
            Parsed model.

        Raises
        ------
        ValueError
            If JSON is invalid.

        Examples
        --------
        >>> from finstack_quant.valuations.models.credit import ToggleExerciseModel
        >>> callable(ToggleExerciseModel.from_json)
        True
        """
        ...

    def to_json(self) -> str:
        """
        Serialize this exercise model to canonical JSON.

        Returns
        -------
        str
            JSON string.
        """
        ...
