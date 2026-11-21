"""Stub declarations mirroring ``finstack-quant-py/src/bindings/portfolio/allocation.rs``."""

from __future__ import annotations

from collections.abc import Sequence
from typing import Any, Literal
import numpy as np
import numpy.typing as npt
import pandas as pd
import polars as pl
from finstack_quant.core.table import ArrowTable

class StrategyAllocationInput:
    """One strategy's inputs to the selected allocation scheme.

    Examples
    --------
    >>> from finstack_quant.portfolio import StrategyAllocationInput
    >>> strategy = StrategyAllocationInput("credit", fixed_weight=0.5)
    """

    def __init__(
        self,
        id: str,
        *,
        fixed_weight: float | None = None,
        returns: list[float] | None = None,
        risk_budget: float | None = None,
    ) -> None:
        """Store one strategy's allocation inputs.

        Parameters
        ----------
        id : str
            Stable strategy identifier used in the allocation rows.
        fixed_weight : float, optional
            Nonnegative capital fraction for the fixed scheme.
        returns : list[float], optional
            Historical decimal strategy returns for inverse volatility.
        risk_budget : float, optional
            Target contribution fraction for the risk budget scheme.

        Raises
        ------
        TypeError
            If an argument has an incompatible Python type. Scheme validation
            occurs when allocate_weights consumes these inputs.
        """
        ...

class WeightAllocationSpec:
    """Request strategy weights and rounded capital allocations.

    Examples
    --------
    >>> from finstack_quant.portfolio import StrategyAllocationInput, WeightAllocationSpec, allocate_weights
    >>> request = WeightAllocationSpec("equal", 100.0, [StrategyAllocationInput("credit")])
    >>> allocate_weights(request).allocations[0]["capital"]
    100.0
    """

    def __init__(
        self,
        scheme: Literal["equal", "fixed", "inverse_volatility", "risk_budget"],
        total_capital: float,
        strategies: list[StrategyAllocationInput],
        *,
        covariance: Sequence[Sequence[float]]
        | npt.NDArray[np.float64]
        | pd.DataFrame
        | pl.DataFrame
        | Any
        | None = None,
        money_decimal_places: int | None = None,
    ) -> None:
        """Store inputs to the native strategy allocator.

        Parameters
        ----------
        scheme : str
            Equal, fixed, inverse_volatility or risk_budget, spelled in lowercase.
        total_capital : float
            Finite signed capital in the reporting currency.
        strategies : list[StrategyAllocationInput]
            Rows in strategy order. Ties in rounding follow this order.
        covariance : sequence[sequence[float]] | numpy.ndarray | pandas.DataFrame | polars.DataFrame | Any, optional
            Square strategy-return covariance in strategy order for risk
            budgeting (nested sequences, 2-D numeric array, pandas/Polars
            DataFrame, or Arrow table).
        money_decimal_places : int, optional
            Scale from zero through 12 for capital rounding. None uses 10.

        Raises
        ------
        TypeError
            If an argument has an incompatible Python type.
        ValueError
            If the scheme is unknown.
        OverflowError
            If the scale is outside the unsigned 32-bit integer representation.
            The allocator validates the financial inputs and supported scale.
        """
        ...

class WeightAllocationResult:
    """
    Strategy weight-allocation result.

    Returned by :func:`allocate_weights`.

    Examples
    --------
    >>> from finstack_quant.portfolio import allocate_weights
    >>> spec = '{"scheme":"equal","total_capital":1000.0,"strategies":[{"id":"a"},{"id":"b"}]}'
    >>> [row["weight"] for row in allocate_weights(spec).allocations]
    [0.5, 0.5]
    """

    @property
    def scheme(self) -> str:
        """
        Allocation scheme applied (e.g. ``"inverse_volatility"``).

        Returns
        -------
        str
            The stored label from the canonical Rust result.

        Notes
        -----
        This accessor does not raise; it returns the stored or derived value.
        """
        ...

    @property
    def allocations(self) -> list[dict[str, object]]:
        """
        Per-strategy allocation rows as records.

        Returns
        -------
        list[dict[str, object]]
            JSON-shaped view of the canonical Rust field.

        Notes
        -----
        This accessor does not raise; it returns the stored or derived value.
        """
        ...

    @property
    def diagnostics(self) -> dict[str, object]:
        """
        Portfolio-level diagnostics (``weights_sum``, ``leverage``).

        Returns
        -------
        dict[str, object]
            JSON-shaped view of the canonical Rust field.

        Notes
        -----
        This accessor does not raise; it returns the stored or derived value.
        """
        ...

    def to_arrow(self) -> ArrowTable:
        """
        Per-strategy allocations as an :class:`~finstack_quant.core.table.ArrowTable`.

        Columns: ``id``, ``weight``, ``capital``, ``volatility``, ``risk_contribution``. The schema is pinned, so an empty result keeps the
        same dtypes as a populated one.

        Returns
        -------
        ArrowTable
            One row per entry, with the pinned column schema above.

        Raises
        ------
        ValueError
            If the result cannot be serialized into an Arrow table.
        """
        ...

    def to_pandas(self) -> pd.DataFrame:
        """
        Per-strategy allocations as a pandas ``DataFrame``.

        Returns
        -------
        pd.DataFrame
            One row per entry, with the pinned column schema above.

        Raises
        ------
        ImportError
            If ``pandas`` is not installed.
        """
        ...

    def to_polars(self) -> pl.DataFrame:
        """
        Per-strategy allocations as a Polars ``DataFrame``.

        Returns
        -------
        pl.DataFrame
            One row per entry, with the pinned column schema above.

        Raises
        ------
        ImportError
            If ``polars`` is not installed.
        """
        ...

    def to_pyarrow(self) -> Any:
        """
        Per-strategy allocations as a ``pyarrow.Table``.

        Returns
        -------
        pyarrow.Table
            One row per entry, with the pinned column schema above.

        Raises
        ------
        ImportError
            If ``pyarrow`` is not installed.
        """
        ...

    def __arrow_c_stream__(self, requested_schema: object | None = None) -> object: ...
    def __arrow_c_schema__(self) -> object: ...
    def _repr_html_(self) -> str | None: ...
    def to_json(self) -> str:
        """
        Serialize this result to canonical JSON.

        Returns
        -------
        str
            Canonical JSON wire document, byte-identical to the paired
            ``*_json`` function's output for the same inputs.

        Raises
        ------
        ValueError
            If the value cannot be serialized to JSON.
        """
        ...

    @staticmethod
    def from_json(json: str) -> WeightAllocationResult:
        """
        Deserialize a ``WeightAllocationResult`` from JSON produced by :meth:`to_json`.

        Completes the wire round-trip, which is also what makes this type
        picklable.

        Parameters
        ----------
        json : str
            Canonical JSON produced by :meth:`to_json`.

        Returns
        -------
        WeightAllocationResult
            Validated instance reconstructed from the canonical JSON payload.

        Raises
        ------
        ValueError
            If ``json`` is malformed or does not match the serialized schema.

        Examples
        --------
        >>> from finstack_quant.portfolio import WeightAllocationResult, allocate_weights
        >>> spec = '{"scheme":"equal","total_capital":1000.0,"strategies":[{"id":"a"},{"id":"b"}]}'
        >>> result = allocate_weights(spec)
        >>> WeightAllocationResult.from_json(result.to_json()).to_json() == result.to_json()
        True
        """
        ...

def allocate_weights(spec: WeightAllocationSpec | str | dict[str, Any]) -> WeightAllocationResult:
    """
    Allocate strategy weights from a typed specification.

    The specification contains a ``scheme`` (for example
    ``"inverse_volatility"``), ``total_capital``, and a list of strategy
    objects with ``id`` and return/history fields required by the selected
    scheme. The Rust allocator computes normalized weights and money amounts;
    Python only passes the JSON through.

    ``total_capital`` must be finite and may be positive, negative, or zero.
    Nonnegative strategy weights allocate capital with the same sign, summing
    to the total rounded at ``money_decimal_places`` (default 10, maximum 12).
    Minor units go to the largest fractional remainders, with ties awarded in
    strategy input order. Zero-weight strategies receive zero capital.

    Parameters
    ----------
    spec : WeightAllocationSpec | str | dict
        JSON-serialized ``WeightAllocationSpec`` with a scheme, finite signed
        total capital, strategy rows, and any required covariance data.

    Returns
    -------
    WeightAllocationResult
        Typed result with ``scheme``, ``allocations`` and ``diagnostics``
        getters plus ``to_arrow()`` / ``to_pandas()`` / ``to_polars()`` /
        ``to_pyarrow()``; serialize with ``to_json()`` for the
        wire string. A spec that allocates without error is valid.

    Raises
    ------
    ValueError
        If the JSON is malformed, required fields are missing, the
        scheme is unsupported, or the selected scheme cannot be evaluated.
    PortfolioError
        If scheme invariants fail or capital cannot be represented at the
        requested rounding precision.

    Examples
    --------
    >>> import json
    >>> from finstack_quant.portfolio import allocate_weights
    >>> spec = '{"scheme":"equal","total_capital":1000.0,"strategies":[{"id":"a"},{"id":"b"}]}'
    >>> [entry["weight"] for entry in allocate_weights(spec).allocations]
    [0.5, 0.5]
    """
    ...
