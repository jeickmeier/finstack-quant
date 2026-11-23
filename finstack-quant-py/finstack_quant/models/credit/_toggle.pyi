"""Stub declarations mirroring ``finstack-quant-py/src/bindings/models/credit/toggle.rs``."""

from __future__ import annotations

from finstack_quant.core.types import CreditRating
import datetime
import numpy as np
from numpy.typing import NDArray
import pandas
import polars as pl
from finstack_quant.core.dates import DayCount
from finstack_quant.core.market_data import HazardCurve
from finstack_quant.core.table import ArrowTable, LabeledSeries
from typing import Any

class CreditState:
    """
    Point-in-time credit state for toggle and path-dependent credit logic.

    Examples
    --------
    >>> import json
    >>> from finstack_quant.models.credit import CreditState
    >>> state = CreditState(hazard_rate=0.02, leverage=4.0)
    >>> (json.loads(state.to_json())["hazard_rate"], json.loads(state.to_json())["leverage"])
    (0.02, 4.0)

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
            If any supplied value is non-finite (JSON cannot carry ``NaN`` or
            infinities).
        """
        ...

    def to_json(self) -> str:
        """
        Serialize this state to canonical JSON.

        Returns
        -------
        str
            JSON string.

        Raises
        ------
        ValueError
            If the value cannot be serialized to JSON.
        """
        ...

    @staticmethod
    def from_json(json: str) -> CreditState:
        """
        Deserialize a `CreditState` from JSON produced by :meth:`to_json`.

        Completes the wire round-trip, which is also what makes this type
        picklable.

        Parameters
        ----------
        json : str
            Canonical JSON produced by :meth:`to_json`.

        Returns
        -------
        CreditState
            Validated state reconstructed from the canonical JSON payload.

        Examples
        --------
        >>> from finstack_quant.models.credit import CreditState
        >>> state = CreditState(hazard_rate=0.02, leverage=4.0)
        >>> restored = CreditState.from_json(state.to_json())
        >>> (restored.hazard_rate, restored.leverage)
        (0.02, 4.0)

        Raises
        ------
        ValueError
            If ``json`` is malformed or does not match the serialized schema.
        """
        ...

    @property
    def hazard_rate(self) -> float:
        """
        Instantaneous default intensity at this observation.

        Returns
        -------
        float
            Annualized hazard rate as a decimal.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

    @property
    def distance_to_default(self) -> float | None:
        """
        Structural distance-to-default.

        Returns
        -------
        float or None
            Distance in standard deviations, or ``None`` when unavailable.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

    @property
    def leverage(self) -> float:
        """
        Leverage ratio (debt / assets).

        Returns
        -------
        float
            Leverage ratio.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

    @property
    def accreted_notional(self) -> float:
        """
        Accreted (PIK-augmented) notional outstanding.

        Returns
        -------
        float
            Outstanding notional.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

    @property
    def coupon_due(self) -> float:
        """
        Cash coupon amount due at this decision date.

        Returns
        -------
        float
            Coupon amount.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

    @property
    def asset_value(self) -> float | None:
        """
        Fair value of the firm's assets.

        Returns
        -------
        float or None
            Asset value, or ``None`` when unavailable.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

    def to_arrow(self) -> ArrowTable:
        """Export the observed credit state as a single-row :class:`~finstack_quant.core.table.ArrowTable`.

        Returns
        -------
        ArrowTable
            Single-row table with the canonical state fields as columns.

        Raises
        ------
        ValueError
            If the result cannot be represented as an Arrow table.
        """
        ...

    def to_pandas(self) -> pandas.DataFrame:
        """Export the observed credit state as a single-row pandas ``DataFrame``.

        Returns
        -------
        pandas.DataFrame
            Single-row frame with the canonical state fields as columns.

        Raises
        ------
        ImportError
            If ``pandas`` is not installed.
        ValueError
            If the result cannot be represented as a pandas DataFrame.
        """
        ...

    def to_polars(self) -> pl.DataFrame:
        """Export the observed credit state as a single-row Polars ``DataFrame``.

        Returns
        -------
        polars.DataFrame
            Single-row frame with the canonical state fields as columns.

        Raises
        ------
        ImportError
            If ``polars`` is not installed.
        ValueError
            If the result cannot be represented as a Polars DataFrame.
        """
        ...

    def to_pyarrow(self) -> Any:
        """Export the observed credit state as a single-row ``pyarrow.Table``.

        Returns
        -------
        pyarrow.Table
            Single-row table with the canonical state fields as columns.

        Raises
        ------
        ImportError
            If ``pyarrow`` is not installed.
        ValueError
            If the result cannot be represented as a PyArrow table.
        """
        ...

    def __arrow_c_stream__(self, requested_schema: object | None = None) -> object: ...
    def __arrow_c_schema__(self) -> object: ...
    def _repr_html_(self) -> str | None: ...
    def __reduce__(self) -> tuple[Any, tuple[str]]:
        """Reconstruct this value for ``pickle`` through its canonical JSON form.

        Returns
        -------
        tuple[Any, tuple[str]]
            The bound ``from_json`` reconstruction callable and its JSON payload.

        Raises
        ------
        ValueError
            If serialization fails or the JSON payload cannot reconstruct the
            stored value, including non-finite floats unsupported by its schema.
        """
        ...

class ToggleExerciseModel:
    """
    Exercise model for PIK/cash toggle and similar embedded options.

    Examples
    --------
    >>> import json
    >>> from finstack_quant.models.credit import ToggleExerciseModel
    >>> model = ToggleExerciseModel.threshold("leverage", 5.0, "above")
    >>> json.loads(model.to_json())["threshold"]["state_variable"]
    'leverage'

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
            If ``variable`` or ``direction`` is not recognized (the message
            lists the accepted labels), or ``threshold`` is non-finite.

        Examples
        --------
        >>> import json
        >>> from finstack_quant.models.credit import ToggleExerciseModel
        >>> model = ToggleExerciseModel.threshold("leverage", 5.0, "above")
        >>> json.loads(model.to_json())["threshold"]["direction"]
        'above'

        """
        ...

    @staticmethod
    def optimal(
        nested_paths: int,
        asset_vol: float,
        risk_free_rate: float,
        horizon: float,
    ) -> ToggleExerciseModel:
        """
        Create an optimal exercise model comparing common-path undiscounted payoff sums.

        Parameters
        ----------
        nested_paths : int
            Number of nested Monte Carlo paths for the inner optimization.
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
            If ``nested_paths`` is zero, a rate is non-finite, ``asset_vol`` is
            negative or non-finite, or ``horizon`` is not finite and positive.

        Examples
        --------
        >>> import json
        >>> from finstack_quant.models.credit import ToggleExerciseModel
        >>> model = ToggleExerciseModel.optimal(100, 0.3, 0.05, 1.0)
        >>> json.loads(model.to_json())["optimal_exercise"]["nested_paths"]
        100
        """
        ...

    @staticmethod
    def stochastic(variable: str, intercept: float, sensitivity: float) -> ToggleExerciseModel:
        """
        Stochastic rule: PIK with probability ``logistic(intercept + sensitivity * x)``.

        Parameters
        ----------
        variable : str
            Credit-state variable ``x`` observed: ``"hazard_rate"``,
            ``"distance_to_default"`` or ``"leverage"``.
        intercept : float
            Logit intercept.
        sensitivity : float
            Logit slope per unit of the variable.

        Returns
        -------
        ToggleExerciseModel
            Rule with ``kind == "stochastic"``.

        Raises
        ------
        ValueError
            For an unknown variable string, or a non-finite intercept or
            sensitivity.

        Examples
        --------
        >>> from finstack_quant.models.credit import ToggleExerciseModel
        >>> ToggleExerciseModel.stochastic("leverage", -2.0, 0.5).kind
        'stochastic'
        """
        ...

    def should_pik(self, state: CreditState, u: float) -> bool:
        """
        Whether the rule elects PIK for ``state`` given one uniform draw.

        Parameters
        ----------
        state : CreditState
            Observed credit state at the decision date.
        u : float
            Uniform draw in ``[0, 1)``; ignored by threshold rules, used as the
            Bernoulli draw by stochastic rules (PIK when ``u`` is below the
            logistic probability). Optimal-exercise rules run their nested
            simulation with a seed derived from ``u``, so equal draws give equal
            decisions.

        Returns
        -------
        bool
            ``True`` when the coupon is paid in kind.

        Raises
        ------
        RuntimeError
            If the nested Monte Carlo model cannot produce a finite normal draw.
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
        >>> import json
        >>> from finstack_quant.models.credit import ToggleExerciseModel
        >>> model = ToggleExerciseModel.threshold("leverage", 5.0, "above")
        >>> restored = ToggleExerciseModel.from_json(model.to_json())
        >>> json.loads(restored.to_json())["threshold"]["threshold"]
        5.0

        """
        ...

    def to_json(self) -> str:
        """
        Serialize this exercise model to canonical JSON.

        Returns
        -------
        str
            JSON string.

        Raises
        ------
        ValueError
            If the value cannot be serialized to JSON.
        """
        ...

    @property
    def kind(self) -> str:
        """
        Which exercise rule this model carries.

        Returns
        -------
        str
            One of ``"threshold"``, ``"stochastic"`` or ``"optimal_exercise"`` —
            the canonical serde tag, so it also names the single key in the
            ``to_json`` payload.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

    @property
    def params(self) -> Any:
        """
        Parameters of the active rule, in canonical JSON form.

        Returns
        -------
        Any
            Mapping whose keys depend on ``kind``: ``state_variable`` /
            ``threshold`` / ``direction`` for a threshold rule,
            ``state_variable`` / ``intercept`` / ``sensitivity`` for a
            stochastic one, and the nested-Monte-Carlo settings for optimal
            exercise.

        Notes
        -----
        This accessor does not raise; it returns the stored or derived value.
        """
        ...

    def __reduce__(self) -> tuple[Any, tuple[str]]:
        """Reconstruct this value for ``pickle`` through its canonical JSON form.

        Returns
        -------
        tuple[Any, tuple[str]]
            The bound ``from_json`` reconstruction callable and its JSON payload.

        Raises
        ------
        ValueError
            If serialization fails or the JSON payload cannot reconstruct the
            stored value, including non-finite floats unsupported by its schema.
        """
        ...
