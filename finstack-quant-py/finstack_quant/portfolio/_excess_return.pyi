"""Stub declarations mirroring ``finstack-quant-py/src/bindings/portfolio/excess_return.rs``."""

from __future__ import annotations

from typing import Any
import pandas as pd
import polars as pl
from finstack_quant.core.market_data import DiscountCurve
from finstack_quant.core.table import ArrowTable

class DurationCellTable:
    """
    Duration-cell base-return table.

    Returned by :func:`cell_returns_from_reference` and
    :func:`cell_returns_from_curves`; pass ``to_json()`` to
    :func:`excess_returns` as the base table.

    Examples
    --------
    >>> from finstack_quant.portfolio import cell_returns_from_reference
    >>> reference = [{"duration": 1.0, "total_return": 0.02}]
    >>> table = cell_returns_from_reference(reference, "UST", {"width": 2.0})
    >>> table.cells[0]["base_return"]
    0.02
    """

    @property
    def base_label(self) -> str:
        """
        Label identifying the reference universe (e.g. ``"UST"``).

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
    def cells(self) -> list[dict[str, object]]:
        """
        Cells as records, in ascending duration order.

        Returns
        -------
        list[dict[str, object]]
            JSON-shaped view of the canonical Rust field.

        Notes
        -----
        This accessor does not raise; it returns the stored or derived value.
        """
        ...

    def to_arrow(self) -> ArrowTable:
        """
        Cells as an :class:`~finstack_quant.core.table.ArrowTable`.

        Columns: ``label``, ``lower``, ``upper``, ``base_return``, ``observed``. The schema is pinned, so an empty result keeps the
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
        Cells as a pandas ``DataFrame``.

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
        Cells as a Polars ``DataFrame``.

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
        Cells as a ``pyarrow.Table``.

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
            Canonical JSON representation, suitable for a matching
            :meth:`from_json` call.

        Raises
        ------
        ValueError
            If the value cannot be serialized to JSON.
        """
        ...

    @staticmethod
    def from_json(json: str) -> DurationCellTable:
        """
        Deserialize a ``DurationCellTable`` from canonical JSON.

        Parameters
        ----------
        json : str
            Canonical payload produced by :meth:`to_json` or the paired
            ``*_json`` wire function.

        Returns
        -------
        DurationCellTable
            Validated instance reconstructed from the canonical JSON payload.

        Raises
        ------
        ValueError
            If the payload is malformed or does not match the serialized
            ``DurationCellTable`` schema.

        Examples
        --------
        >>> from finstack_quant.portfolio import DurationCellTable
        >>> doc = '{"base_label": "UST", "cells": []}'
        >>> DurationCellTable.from_json(doc).base_label
        'UST'
        """
        ...

class ExcessReturnResult:
    """
    Per-position and portfolio-level duration-matched credit excess returns.

    Returned by :func:`excess_returns`.

    Examples
    --------
    >>> from finstack_quant.portfolio import cell_returns_from_reference, excess_returns
    >>> reference = [{"duration": 1.0, "total_return": 0.02}]
    >>> table = cell_returns_from_reference(reference, "UST", {"width": 2.0})
    >>> positions = [{"id": "B1", "weight": 1.0, "duration": 1.0, "total_return": 0.03}]
    >>> result = excess_returns(positions, table)
    >>> round(result.portfolio_excess_return, 4)
    0.01
    """

    @property
    def base_label(self) -> str:
        """
        Label of the base curve the excess returns were measured against.

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
    def positions(self) -> list[dict[str, object]]:
        """
        Per-position results as records, in input order.

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
    def portfolio_total_return(self) -> float:
        """
        Weight-weighted portfolio total return.

        Returns
        -------
        float
            The stored value from the canonical Rust result.

        Notes
        -----
        This accessor does not raise; it returns the stored or derived value.
        """
        ...

    @property
    def portfolio_base_return(self) -> float:
        """
        Weight-weighted portfolio base return.

        Returns
        -------
        float
            The stored value from the canonical Rust result.

        Notes
        -----
        This accessor does not raise; it returns the stored or derived value.
        """
        ...

    @property
    def portfolio_excess_return(self) -> float:
        """
        Weight-weighted portfolio excess return.

        Returns
        -------
        float
            The stored value from the canonical Rust result.

        Notes
        -----
        This accessor does not raise; it returns the stored or derived value.
        """
        ...

    def to_arrow(self) -> ArrowTable:
        """
        Per-position excess returns as an :class:`~finstack_quant.core.table.ArrowTable`.

        Columns: ``id``, ``cell``, ``base_return``, ``excess_return``. The schema is pinned, so an empty result keeps the
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
        Per-position excess returns as a pandas ``DataFrame``.

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
        Per-position excess returns as a Polars ``DataFrame``.

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
        Per-position excess returns as a ``pyarrow.Table``.

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
            Canonical JSON representation, suitable for a matching
            :meth:`from_json` call.

        Raises
        ------
        ValueError
            If the value cannot be serialized to JSON.
        """
        ...

    @staticmethod
    def from_json(json: str) -> ExcessReturnResult:
        """
        Deserialize a ``ExcessReturnResult`` from canonical JSON.

        Parameters
        ----------
        json : str
            Canonical payload produced by :meth:`to_json` or the paired
            ``*_json`` wire function.

        Returns
        -------
        ExcessReturnResult
            Validated instance reconstructed from the canonical JSON payload.

        Raises
        ------
        ValueError
            If the payload is malformed or does not match the serialized
            ``ExcessReturnResult`` schema.

        Examples
        --------
        >>> from finstack_quant.portfolio import ExcessReturnResult
        >>> doc = '{"base_label": "UST", "positions": [], "portfolio_total_return": 0.0, "portfolio_base_return": 0.0, "portfolio_excess_return": 0.0}'
        >>> ExcessReturnResult.from_json(doc).base_label
        'UST'
        """
        ...

def cell_returns_from_reference(
    reference: dict[str, Any] | list[Any] | pd.DataFrame,
    base_label: str,
    config: dict[str, Any] | list[Any] | pd.DataFrame,
) -> DurationCellTable:
    """
    Build a duration-cell base-return table from a reference universe.

    Binds Rust ``finstack_quant_portfolio::cell_returns_from_reference``
    (Dynkin, Hyman & Vankudre 1998, Appendix B): buckets ``reference``
    into fixed-width duration cells and averages each cell's member total
    returns, interpolating interior gaps and flat-extrapolating
    leading/trailing gaps.

    Parameters
    ----------
    reference : dict | list | pandas.DataFrame
        List of ``ReferenceReturn`` objects (``duration``,
        ``total_return``, both decimals with duration in years); must be
        non-empty. Unknown fields are rejected.
    base_label : str
        Label identifying the resulting curve (e.g. ``"UST"``), carried
        through to the output's ``base_label`` for policy visibility.
    config : dict | list | pandas.DataFrame
        ``CellConfig`` dict; ``width`` is its only field (cell width in
        years, finite and positive) and is required — there is no default.

    Returns
    -------
    DurationCellTable
        Typed cell table with ``to_arrow()`` / ``to_pandas()`` / ``to_polars()``
        / ``to_pyarrow()`` / ``to_json()`` exits;
        serialize with ``to_json()`` for the wire string.

    Raises
    ------
    PortfolioError
        If ``reference`` is empty, ``config.width`` is not finite and
        positive, any reference entry has a non-finite ``total_return`` or a
        non-finite/negative ``duration``, the width produces two numerically
        distinct cells that collide on their one-decimal label, or the
        largest reference ``duration`` divided by ``config.width`` would
        require more than an internal sanity bound of cells (100,000; a
        units mistake, e.g. days instead of years, rather than a legitimate
        duration grid).
    ValueError
        If ``reference`` or ``config`` is malformed or carries unknown fields.

    Sources
    -------
    See ``docs/REFERENCES.md#dynkin-hyman-vankudre-1998``.

    Examples
    --------
    >>> from finstack_quant.portfolio import cell_returns_from_reference
    >>> reference = [{"duration": 1.0, "total_return": 0.02}]
    >>> table = cell_returns_from_reference(reference, "UST", {"width": 2.0})
    >>> table.cells[0]["base_return"]
    0.02
    """
    ...

def cell_returns_from_curves(
    start: DiscountCurve,
    end: DiscountCurve,
    horizon_years: float,
    max_duration: float,
    base_label: str,
    config: dict[str, Any] | list[Any] | pd.DataFrame,
) -> DurationCellTable:
    """
    Build a duration-cell base-return table from start/end discount curves.

    Binds Rust ``finstack_quant_portfolio::cell_returns_from_curves``: each
    cell's base return is the holding-period return of a hypothetical
    zero-coupon position bought at the cell midpoint off ``start`` and
    revalued off ``end`` after ``horizon_years`` have elapsed. Every
    resulting cell is observed (a curve has a discount factor at every
    queried point), unlike the reference-universe path in
    :func:`cell_returns_from_reference`.

    Parameters
    ----------
    start : DiscountCurve
        Discount curve observed at the start of the holding period.
    end : DiscountCurve
        Discount curve observed ``horizon_years`` later, at period end.
    horizon_years : float
        Length of the holding period, in years; must be finite and positive.
    max_duration : float
        Upper bound of the duration grid, in years; must be finite and
        strictly greater than ``horizon_years``.
    base_label : str
        Label identifying the base curve (e.g. ``"UST"``, ``"USD-SOFR"``),
        stamped into the result purely for policy visibility.
    config : dict | list | pandas.DataFrame
        ``CellConfig`` dict; ``width`` is its only field and is required.

    Returns
    -------
    DurationCellTable
        Typed cell table with ``to_arrow()`` / ``to_pandas()`` / ``to_polars()``
        / ``to_pyarrow()`` / ``to_json()`` exits;
        serialize with ``to_json()`` for the wire string.

    Raises
    ------
    PortfolioError
        If ``config.width`` or ``horizon_years`` is not finite and positive,
        ``max_duration`` does not strictly exceed ``horizon_years``,
        ``max_duration`` divided by ``config.width`` would require more than
        an internal sanity bound of cells (100,000; a units mistake rather
        than a legitimate duration grid), a cell's midpoint does not exceed
        ``horizon_years`` (unavoidable whenever ``config.width`` is not
        strictly greater than ``2 * horizon_years``, since the first cell's
        midpoint is ``config.width / 2``), either curve produces a
        non-positive/non-finite discount factor at a queried time, or the
        width produces colliding one-decimal cell labels.
    ValueError
        If ``config`` is malformed.

    Sources
    -------
    See ``docs/REFERENCES.md#dynkin-hyman-vankudre-1998``.

    Examples
    --------
    >>> from datetime import date
    >>> from finstack_quant.core.market_data import DiscountCurve
    >>> from finstack_quant.portfolio import cell_returns_from_curves
    >>> start = DiscountCurve.flat("start", date(2025, 1, 1), 0.02, "act_365f")
    >>> end = DiscountCurve.flat("end", date(2025, 4, 1), 0.03, "act_365f")
    >>> table = cell_returns_from_curves(start, end, 0.25, 2.0, "UST", {"width": 1.0})
    >>> len(table.cells)
    2
    """
    ...

def excess_returns(
    positions: dict[str, Any] | list[Any] | pd.DataFrame | pl.DataFrame | Any,
    table: DurationCellTable | dict[str, Any] | list[Any] | pd.DataFrame | pl.DataFrame | Any,
) -> ExcessReturnResult:
    """
    Compute duration-matched credit excess returns against a base-return table.

    Binds Rust ``finstack_quant_portfolio::excess_returns`` (Dynkin, Hyman &
    Vankudre 1998, Appendix B): each position's ``duration`` is matched to
    its duration cell in ``table`` and the position's excess return is
    ``total_return - cell.base_return``, the credit-specific component of
    performance isolated from the general level/shape move of the base curve.

    Parameters
    ----------
    positions : dict | list | pandas.DataFrame | polars.DataFrame | Any
        Record list, pandas/Polars DataFrame, or Arrow table of
        ``ExcessReturnPosition`` objects (``id``, ``weight``, ``duration``,
        ``total_return``); weights must sum to ``1.0`` within ``1e-6``.
    table : DurationCellTable | dict | list | pandas.DataFrame | polars.DataFrame | Any
        Typed ``DurationCellTable`` as returned by
        :func:`cell_returns_from_reference` or
        :func:`cell_returns_from_curves`, its dict form (for example
        ``json.loads(table.to_json())``), or a tabular record input.

    Returns
    -------
    ExcessReturnResult
        Typed result with per-position and portfolio-level
        total/base/excess returns; use :func:`excess_returns` for the
        raw wire string.

    Raises
    ------
    PortfolioError
        If ``table`` has no cells; a cell has an empty or duplicate
        label, non-finite bounds or base return, a negative lower bound, a
        non-positive width, non-ascending lower bounds, or overlaps its
        predecessor; a position has a non-finite
        weight/duration/total_return; a position's duration falls outside
        every cell, including a valid gap (the error names the position); or
        the position weights do not sum to ``1.0`` within tolerance.
    ValueError
        If ``positions`` or ``table`` is malformed.

    Sources
    -------
    See ``docs/REFERENCES.md#dynkin-hyman-vankudre-1998``.

    Examples
    --------
    >>> from finstack_quant.portfolio import cell_returns_from_reference, excess_returns
    >>> reference = [{"duration": 1.0, "total_return": 0.02}]
    >>> table = cell_returns_from_reference(reference, "UST", {"width": 2.0})
    >>> positions = [{"id": "B1", "weight": 1.0, "duration": 1.0, "total_return": 0.03}]
    >>> result = excess_returns(positions, table)
    >>> round(result.portfolio_excess_return, 4)
    0.01
    """
    ...
