"""Stub declarations mirroring ``finstack-quant-py/src/bindings/portfolio/allocation.rs``."""

from __future__ import annotations

from typing import Any
import pandas as pd

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

    def to_dataframe(self) -> pd.DataFrame:
        """
        Per-strategy allocations as a pandas DataFrame.

        Columns: ``id``, ``weight``, ``capital``, ``volatility``, ``risk_contribution``. The schema is pinned, so an empty result keeps the
        same dtypes as a populated one.

        Returns
        -------
        pd.DataFrame
            One row per entry, with the pinned column schema above.

        Raises
        ------
        ValueError
            If the result cannot be serialized into a pandas object.
        """
        ...

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

def allocate_weights(spec_json: str | dict[str, Any] | list[Any] | pd.DataFrame) -> WeightAllocationResult:
    """
    Allocate strategy weights from a JSON specification.

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
    spec_json : str | dict | list | pandas.DataFrame
        JSON-serialized ``WeightAllocationSpec`` with a scheme, finite signed
        total capital, strategy rows, and any required covariance data.

    Returns
    -------
    WeightAllocationResult
        Typed result with ``scheme``, ``allocations`` and ``diagnostics``
        getters plus ``to_dataframe()``; serialize with ``to_json()`` for the
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
