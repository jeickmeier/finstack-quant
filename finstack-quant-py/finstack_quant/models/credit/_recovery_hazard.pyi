"""Stub declarations mirroring ``finstack-quant-py/src/bindings/models/credit/recovery_hazard.rs``."""

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

class DynamicRecoverySpec:
    """
    Recovery specification with optional notional dependence.

    Examples
    --------
    >>> from finstack_quant.models.credit import DynamicRecoverySpec
    >>> spec = DynamicRecoverySpec.constant(0.4)
    >>> spec.recovery_at_notional(100.0)
    0.4

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
        >>> from finstack_quant.models.credit import DynamicRecoverySpec
        >>> spec = DynamicRecoverySpec.constant(0.4)
        >>> spec.recovery_at_notional(100.0)
        0.4

        """
        ...

    @staticmethod
    def inverse_linear(base_recovery: float, base_notional: float) -> DynamicRecoverySpec:
        """
        Recovery inversely proportional to notional: ``R(N) = R_0 * N_0 / N``.

        Parameters
        ----------
        base_recovery : float
            Reference recovery ``R_0`` as a decimal in ``[0, 1]``.
        base_notional : float
            Reference notional ``N_0`` (strictly positive), in the instrument's currency.

        Returns
        -------
        DynamicRecoverySpec
            Specification with ``kind == "inverse_linear"``.

        Raises
        ------
        ValueError
            On non-finite or out-of-range inputs.

        Examples
        --------
        >>> from finstack_quant.models.credit import DynamicRecoverySpec
        >>> DynamicRecoverySpec.inverse_linear(0.4, 100.0).recovery_at_notional(200.0)
        0.2
        """
        ...

    @staticmethod
    def inverse_power(base_recovery: float, base_notional: float, exponent: float) -> DynamicRecoverySpec:
        """
        Recovery decaying as a power of notional: ``R(N) = R_0 * (N_0 / N)**exponent``.

        Parameters
        ----------
        base_recovery : float
            Reference recovery ``R_0`` as a decimal in ``[0, 1]``.
        base_notional : float
            Reference notional ``N_0`` (strictly positive), in the instrument's currency.
        exponent : float
            Non-negative decay exponent (``1.0`` reproduces ``inverse_linear``).

        Returns
        -------
        DynamicRecoverySpec
            Specification with ``kind == "inverse_power"``.

        Raises
        ------
        ValueError
            On non-finite or out-of-range inputs.

        Examples
        --------
        >>> from finstack_quant.models.credit import DynamicRecoverySpec
        >>> DynamicRecoverySpec.inverse_power(0.4, 100.0, 1.0).kind
        'inverse_power'
        """
        ...

    @staticmethod
    def floored_inverse(base_recovery: float, base_notional: float, floor: float) -> DynamicRecoverySpec:
        """
        Inverse-linear recovery floored at a minimum: ``max(R_0 * N_0 / N, floor)``.

        Parameters
        ----------
        base_recovery : float
            Reference recovery ``R_0`` as a decimal in ``[0, 1]``.
        base_notional : float
            Reference notional ``N_0`` (strictly positive), in the instrument's currency.
        floor : float
            Minimum recovery as a decimal in ``[0, base_recovery]``.

        Returns
        -------
        DynamicRecoverySpec
            Specification with ``kind == "floored_inverse"``.

        Raises
        ------
        ValueError
            On non-finite or out-of-range inputs.

        Examples
        --------
        >>> from finstack_quant.models.credit import DynamicRecoverySpec
        >>> DynamicRecoverySpec.floored_inverse(0.4, 100.0, 0.3).recovery_at_notional(400.0)
        0.3
        """
        ...

    @staticmethod
    def linear_decline(
        base_recovery: float, base_notional: float, sensitivity: float, floor: float
    ) -> DynamicRecoverySpec:
        """
        Recovery declining linearly with accretion: ``clamp(R_0 * (1 - sensitivity * (N / N_0 - 1)), floor, R_0)``.

        Parameters
        ----------
        base_recovery : float
            Reference recovery ``R_0`` as a decimal in ``[0, 1]``.
        base_notional : float
            Reference notional ``N_0`` (strictly positive), in the instrument's currency.
        sensitivity : float
            Dimensionless, non-negative relative recovery sensitivity to ``N / N_0 - 1``.
        floor : float
            Minimum recovery as a decimal in ``[0, base_recovery]``.

        Returns
        -------
        DynamicRecoverySpec
            Specification with ``kind == "linear_decline"``.

        Raises
        ------
        ValueError
            On non-finite or out-of-range inputs.

        Examples
        --------
        >>> from finstack_quant.models.credit import DynamicRecoverySpec
        >>> spec = DynamicRecoverySpec.linear_decline(0.4, 100.0, sensitivity=0.5, floor=0.1)
        >>> round(spec.recovery_at_notional(120.0), 2)
        0.36
        """
        ...

    @property
    def kind(self) -> str:
        """
        Canonical name of the active recovery model: ``"constant"``, ``"inverse_linear"``, ``"inverse_power"``, ``"floored_inverse"`` or ``"linear_decline"``.

        Returns
        -------
        str
            Canonical name of the active recovery model: ``"constant"``, ``"inverse_linear"``, ``"inverse_power"``, ``"floored_inverse"`` or ``"linear_decline"``.

        Notes
        -----
        This accessor does not raise; it returns the stored or derived value.
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
        >>> from finstack_quant.models.credit import DynamicRecoverySpec
        >>> spec = DynamicRecoverySpec.constant(0.4)
        >>> DynamicRecoverySpec.from_json(spec.to_json()).recovery_at_notional(100.0)
        0.4

        """
        ...

    def to_json(self) -> str:
        """
        Serialize this recovery specification to canonical JSON.

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

        Notes
        -----
        This method does not raise; out-of-domain or non-finite inputs yield ``NaN`` or ``inf`` rather than an exception.
        """
        ...

    @property
    def base_recovery(self) -> float:
        """
        Base (reference) recovery rate ``R_0``.

        Returns
        -------
        float
            Recovery rate as a decimal in ``[0, 1]``.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

    @property
    def base_notional(self) -> float:
        """
        Base (reference) notional ``N_0`` the recovery mapping is anchored to.

        Returns
        -------
        float
            Reference notional (positive).

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

    @property
    def model(self) -> Any:
        """
        Notional-to-recovery mapping, in canonical JSON form.

        Returns
        -------
        Any
            ``"constant"`` / ``"inverse_linear"`` for the parameterless models,
            or a single-key mapping (``inverse_power``, ``floored_inverse``,
            ``linear_decline``) carrying that model's parameters.

        Notes
        -----
        This accessor does not raise; it returns the stored or derived value.
        """
        ...

    def to_arrow(self) -> ArrowTable:
        """Export the recovery specification as a single-row :class:`~finstack_quant.core.table.ArrowTable`.

        Returns
        -------
        ArrowTable
            Single-row table with the canonical specification fields as columns.

        Raises
        ------
        ValueError
            If the result cannot be represented as an Arrow table.
        """
        ...

    def to_pandas(self) -> pandas.DataFrame:
        """Export the recovery specification as a single-row pandas ``DataFrame``.

        Returns
        -------
        pandas.DataFrame
            Single-row frame with the canonical specification fields as columns.

        Raises
        ------
        ImportError
            If ``pandas`` is not installed.
        ValueError
            If the result cannot be represented as a pandas DataFrame.
        """
        ...

    def to_polars(self) -> pl.DataFrame:
        """Export the recovery specification as a single-row Polars ``DataFrame``.

        Returns
        -------
        polars.DataFrame
            Single-row frame with the canonical specification fields as columns.

        Raises
        ------
        ImportError
            If ``polars`` is not installed.
        ValueError
            If the result cannot be represented as a Polars DataFrame.
        """
        ...

    def to_pyarrow(self) -> Any:
        """Export the recovery specification as a single-row ``pyarrow.Table``.

        Returns
        -------
        pyarrow.Table
            Single-row table with the canonical specification fields as columns.

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

class EndogenousHazardSpec:
    """
    Hazard-rate model driven by leverage or PIK-accreted notional.

    Examples
    --------
    >>> from finstack_quant.models.credit import EndogenousHazardSpec
    >>> spec = EndogenousHazardSpec.power_law(0.02, 4.0, 2.0)
    >>> (spec.hazard_at_leverage(4.0), spec.hazard_at_leverage(8.0))
    (0.02, 0.08)

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
        >>> from finstack_quant.models.credit import EndogenousHazardSpec
        >>> spec = EndogenousHazardSpec.power_law(0.02, 4.0, 2.0)
        >>> spec.hazard_at_leverage(8.0)
        0.08

        """
        ...

    @staticmethod
    def exponential(base_hazard: float, base_leverage: float, sensitivity: float) -> EndogenousHazardSpec:
        """
        Exponential feedback: ``lambda(L) = lambda_0 * exp(sensitivity * (L - L_0))``.

        Parameters
        ----------
        base_hazard : float
            Reference annualized hazard rate ``lambda_0`` (non-negative decimal).
        base_leverage : float
            Reference leverage ``L_0`` (strictly positive ratio).
        sensitivity : float
            Non-negative exponential sensitivity per unit of leverage.

        Returns
        -------
        EndogenousHazardSpec
            Specification with ``kind == "exponential"``.

        Raises
        ------
        ValueError
            On non-finite or out-of-range inputs.

        Examples
        --------
        >>> from finstack_quant.models.credit import EndogenousHazardSpec
        >>> EndogenousHazardSpec.exponential(0.02, 4.0, 0.5).hazard_at_leverage(4.0)
        0.02
        """
        ...

    @staticmethod
    def tabular(leverage_points: list[float], hazard_points: list[float]) -> EndogenousHazardSpec:
        """
        Piecewise-linear tabular feedback through ``(leverage, hazard)`` points.

        Parameters
        ----------
        leverage_points : list[float]
            One or more finite non-negative leverage ratios, strictly increasing
            when multiple points are supplied; one point gives constant hazard.
        hazard_points : list[float]
            Non-negative annualized hazard rates, one per leverage point; the
            base leverage and base hazard are taken from the first point.

        Returns
        -------
        EndogenousHazardSpec
            Specification with ``kind == "tabular"``.

        Raises
        ------
        ValueError
            If the lists differ in length, are empty, are not
            strictly increasing in leverage, or contain non-finite or negative
            values.

        Examples
        --------
        >>> from finstack_quant.models.credit import EndogenousHazardSpec
        >>> EndogenousHazardSpec.tabular([2.0, 6.0], [0.01, 0.05]).hazard_at_leverage(4.0)
        0.03
        >>> EndogenousHazardSpec.tabular([2.0], [0.01]).hazard_at_leverage(8.0)
        0.01
        """
        ...

    @property
    def kind(self) -> str:
        """
        Canonical name of the active leverage-to-hazard mapping: ``"power_law"``, ``"exponential"`` or ``"tabular"``.

        Returns
        -------
        str
            Canonical name of the active leverage-to-hazard mapping: ``"power_law"``, ``"exponential"`` or ``"tabular"``.

        Notes
        -----
        This accessor does not raise; it returns the stored or derived value.
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
        >>> from finstack_quant.models.credit import EndogenousHazardSpec
        >>> spec = EndogenousHazardSpec.power_law(0.02, 4.0, 2.0)
        >>> EndogenousHazardSpec.from_json(spec.to_json()).hazard_at_leverage(8.0)
        0.08

        """
        ...

    def to_json(self) -> str:
        """
        Serialize this hazard specification to canonical JSON.

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

        Notes
        -----
        This method does not raise; out-of-domain or non-finite inputs yield ``NaN`` or ``inf`` rather than an exception.
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

        Notes
        -----
        This method does not raise; out-of-domain or non-finite inputs yield ``NaN`` or ``inf`` rather than an exception.
        """
        ...

    @property
    def base_hazard_rate(self) -> float:
        """
        Base (reference) hazard rate ``lambda_0``.

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
    def base_leverage(self) -> float:
        """
        Base (reference) leverage level ``L_0`` the hazard mapping is anchored to.

        Returns
        -------
        float
            Reference leverage ratio (positive).

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

    @property
    def leverage_hazard_map(self) -> Any:
        """
        Leverage-to-hazard mapping, in canonical JSON form.

        Returns
        -------
        Any
            A single-key mapping (``power_law``, ``exponential``, ``tabular``)
            carrying that model's parameters.

        Notes
        -----
        This accessor does not raise; it returns the stored or derived value.
        """
        ...

    def to_arrow(self) -> ArrowTable:
        """Export the hazard specification as a single-row :class:`~finstack_quant.core.table.ArrowTable`.

        Returns
        -------
        ArrowTable
            Single-row table with the canonical specification fields as columns.

        Raises
        ------
        ValueError
            If the result cannot be represented as an Arrow table.
        """
        ...

    def to_pandas(self) -> pandas.DataFrame:
        """Export the hazard specification as a single-row pandas ``DataFrame``.

        Returns
        -------
        pandas.DataFrame
            Single-row frame with the canonical specification fields as columns.

        Raises
        ------
        ImportError
            If ``pandas`` is not installed.
        ValueError
            If the result cannot be represented as a pandas DataFrame.
        """
        ...

    def to_polars(self) -> pl.DataFrame:
        """Export the hazard specification as a single-row Polars ``DataFrame``.

        Returns
        -------
        polars.DataFrame
            Single-row frame with the canonical specification fields as columns.

        Raises
        ------
        ImportError
            If ``polars`` is not installed.
        ValueError
            If the result cannot be represented as a Polars DataFrame.
        """
        ...

    def to_pyarrow(self) -> Any:
        """Export the hazard specification as a single-row ``pyarrow.Table``.

        Returns
        -------
        pyarrow.Table
            Single-row table with the canonical specification fields as columns.

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
