"""Stub declarations mirroring ``finstack-quant-py/src/bindings/attribution/return_contribution.rs``."""

from __future__ import annotations

from typing import Any
import pandas as pd
import polars as pl
from finstack_quant.core.table import ArrowTable, LabeledSeries

# Return Contribution

class ReturnContributionResult:
    """
    Return-contribution attribution result.

    Returned by :func:`attribute_return_contribution`. Decomposes a portfolio
    return into per-instrument, per-group, and per-factor contributions, with
    an optional Brinson-Fachler benchmark-relative block.

    Examples
    --------
    >>> from finstack_quant.attribution import ReturnContributionResult
    >>> try:
    ...     ReturnContributionResult.from_json("{}")
    ... except ValueError as exc:
    ...     "missing field" in str(exc)
    True
    """

    @property
    def portfolio_return(self) -> float:
        """
        Total portfolio return, equal to the summed instrument contributions.

        Returns
        -------
        float
            Period return as a decimal fraction (``0.01`` is 1%). Equal to the
            sum of the ``contribution`` field across
            :attr:`instrument_contribution`.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

    @property
    def instrument_contribution(self) -> list[dict[str, Any]]:
        """
        Per-instrument contribution rows.

        Returns
        -------
        list[dict[str, Any]]
            One record per instrument with ``id``, ``weight``, ``return``,
            ``contribution``, and ``active_contribution`` keys.

        Notes
        -----
        This accessor does not raise; it returns the stored or derived value.
        """
        ...

    @property
    def group_contribution(self) -> dict[str, Any]:
        """
        Contributions keyed by group dimension.

        Returns
        -------
        dict[str, Any]
            Maps each group dimension name (for example ``"sector"``) to its
            list of bucket records, each with a ``key`` bucket label and the
            summed ``contribution`` of the instruments in that bucket.

        Notes
        -----
        This accessor does not raise; it returns the stored or derived value.
        """
        ...

    @property
    def factor_contribution(self) -> list[dict[str, Any]]:
        """
        Factor contribution rows.

        Returns
        -------
        list[dict[str, Any]]

        Notes
        -----
        This accessor does not raise; it returns the stored or derived value.
        """
        ...

    @property
    def specific_return(self) -> float | None:
        """
        Idiosyncratic residual when factor rows were supplied.

        Returns
        -------
        float or None
            ``portfolio_return - sum(factor contributions)``; ``None`` when
            the spec carried no factors.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

    @property
    def benchmark_relative(self) -> dict[str, Any] | None:
        """
        Brinson-Fachler benchmark-relative block.

        Returns
        -------
        dict[str, Any] or None
            ``None`` unless benchmark inputs were supplied on the spec.

        Notes
        -----
        This accessor does not raise; it returns the stored or derived value.
        """
        ...

    @property
    def warnings(self) -> list[str]:
        """
        Non-fatal diagnostic warnings from the contribution run.

        Returns
        -------
        list[str]
            For example leveraged weights from a near-flat net-market-value
            book.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

    def to_arrow(self) -> ArrowTable:
        """
        Per-instrument contributions as an :class:`~finstack_quant.core.table.ArrowTable`.

        Returns
        -------
        ArrowTable
            One row per instrument with ``id``, ``weight``, ``return``,
            ``contribution``, and ``active_contribution`` columns; schema columns
            are present even for an empty result.

        Raises
        ------
        ValueError
            If the underlying Arrow record batch cannot be built.
        """
        ...

    def to_pandas(self) -> pd.DataFrame:
        """
        Per-instrument contributions as a pandas DataFrame.

        Returns
        -------
        pd.DataFrame
            One row per instrument with ``id``, ``weight``, ``return``,
            ``contribution``, and ``active_contribution`` (``NaN`` when no
            benchmark was supplied) columns; the schema columns are present
            even for an empty result.

        Raises
        ------
        ImportError
            If ``pandas`` is not installed.
        """
        ...

    def to_polars(self) -> pl.DataFrame:
        """
        Per-instrument contributions as a ``polars.DataFrame``.

        Returns
        -------
        pl.DataFrame
            Per-instrument contributions constructed via the Arrow PyCapsule interface.

        Raises
        ------
        ImportError
            If ``polars`` is not installed.
        """
        ...

    def to_pyarrow(self) -> Any:
        """
        Per-instrument contributions as a ``pyarrow.Table``.

        Returns
        -------
        pyarrow.Table
            Zero-copy table built through the Arrow PyCapsule interface.

        Raises
        ------
        ImportError
            If ``pyarrow`` is not installed.
        """
        ...

    def __arrow_c_stream__(self, requested_schema: Any | None = None) -> Any: ...
    def __arrow_c_schema__(self) -> Any: ...
    def to_group_table(self) -> ArrowTable:
        """
        Group-bucket contributions as a long :class:`~finstack_quant.core.table.ArrowTable`.

        Returns
        -------
        ArrowTable
            Columns ``dimension`` (the ``group:<dimension>`` label name),
            ``key`` (bucket) and ``contribution``; empty with schema columns
            when the spec carried no group labels.

        Raises
        ------
        ValueError
            If the result cannot be serialized into an Arrow table.
        """
        ...

    def to_factor_table(self) -> ArrowTable:
        """
        Factor contributions as an :class:`~finstack_quant.core.table.ArrowTable`.

        Returns
        -------
        ArrowTable
            Columns ``factor``, ``exposure``, ``factor_return``,
            ``contribution``; empty with schema columns when no factor rows
            were supplied.

        Raises
        ------
        ValueError
            If the result cannot be serialized into an Arrow table.
        """
        ...

    def _repr_html_(self) -> str | None: ...
    def to_series(self) -> LabeledSeries:
        """
        Per-instrument contributions as a :class:`~finstack_quant.core.table.LabeledSeries` indexed by instrument id.

        Returns
        -------
        LabeledSeries
            Named ``contribution`` with index ``id``.

        Raises
        ------
        ValueError
            If the series cannot be constructed.
        """
        ...

    def to_json(self) -> str:
        """
        Serialize to a compact JSON string.

        Returns
        -------
        str
            Canonical JSON representation, suitable for a matching
            :meth:`from_json` call.

        Raises
        ------
        ValueError
            If the value cannot be serialized to JSON.
        """
        ...

    @staticmethod
    def from_json(json: str) -> ReturnContributionResult:
        """
        Deserialize from a JSON string.

        Parameters
        ----------
        json : str
            JSON produced by :meth:`to_json`.

        Returns
        -------
        ReturnContributionResult
            Validated instance reconstructed from the canonical JSON payload.

        Raises
        ------
        ValueError
            If ``json`` is malformed or does not match the result schema.

        Examples
        --------
        >>> from finstack_quant.attribution import ReturnContributionResult
        >>> try:
        ...     ReturnContributionResult.from_json("{}")
        ... except ValueError as exc:
        ...     "missing field" in str(exc)
        True
        """
        ...
