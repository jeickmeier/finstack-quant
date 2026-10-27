"""Stub declarations mirroring ``finstack-quant-py/src/bindings/portfolio/excess_return.rs``."""

from __future__ import annotations

from typing import Any
import pandas as pd
from finstack_quant.core.market_data import DiscountCurve

class DurationCellTable:
    """
    Duration-cell base-return table.

    Returned by :func:`cell_returns_from_reference` and
    :func:`cell_returns_from_curves`; pass ``to_json()`` to
    :func:`excess_returns` as the base table.

    Examples
    --------
    >>> import json
    >>> from finstack_quant.portfolio import cell_returns_from_reference
    >>> reference = [{"duration": 1.0, "total_return": 0.02}]
    >>> table = cell_returns_from_reference(json.dumps(reference), "UST", '{"width":2.0}')
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

    def to_dataframe(self) -> pd.DataFrame:
        """
        Cells as a pandas DataFrame.

        Columns: ``label``, ``lower``, ``upper``, ``base_return``, ``observed``. The schema is pinned, so an empty result keeps the
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
    >>> import json
    >>> from finstack_quant.portfolio import cell_returns_from_reference, excess_returns
    >>> reference = [{"duration": 1.0, "total_return": 0.02}]
    >>> table = cell_returns_from_reference(json.dumps(reference), "UST", '{"width":2.0}')
    >>> positions = [{"id": "B1", "weight": 1.0, "duration": 1.0, "total_return": 0.03}]
    >>> result = excess_returns(json.dumps(positions), table.to_json())
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

    def to_dataframe(self) -> pd.DataFrame:
        """
        Per-position excess returns as a pandas DataFrame.

        Columns: ``id``, ``cell``, ``base_return``, ``excess_return``. The schema is pinned, so an empty result keeps the
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
    reference_json: str | dict[str, Any] | list[Any] | pd.DataFrame,
    base_label: str,
    config_json: str | dict[str, Any] | list[Any] | pd.DataFrame,
) -> DurationCellTable:
    """
    Build a duration-cell base-return table from a reference universe.

    Binds Rust ``finstack_quant_portfolio::cell_returns_from_reference``
    (Dynkin, Hyman & Vankudre 1998, Appendix B): buckets ``reference_json``
    into fixed-width duration cells and averages each cell's member total
    returns, interpolating interior gaps and flat-extrapolating
    leading/trailing gaps.

    Parameters
    ----------
    reference_json : str | dict | list | pandas.DataFrame
        JSON array of ``ReferenceReturn`` objects (``duration``,
        ``total_return``, both decimals with duration in years); must be
        non-empty. Unknown fields are rejected.
    base_label : str
        Label identifying the resulting curve (e.g. ``"UST"``), carried
        through to the output's ``base_label`` for policy visibility.
    config_json : str | dict | list | pandas.DataFrame
        JSON ``CellConfig``; ``width`` is its only field (cell width in
        years, finite and positive) and is required — there is no default.

    Returns
    -------
    DurationCellTable
        Typed cell table with ``to_dataframe()`` / ``to_json()`` exits; use
        serialize with ``to_json()`` for the wire string.

    Raises
    ------
    PortfolioError
        If ``reference_json`` is empty, ``config.width`` is not finite and
        positive, any reference entry has a non-finite ``total_return`` or a
        non-finite/negative ``duration``, the width produces two numerically
        distinct cells that collide on their one-decimal label, or the
        largest reference ``duration`` divided by ``config.width`` would
        require more than an internal sanity bound of cells (100,000; a
        units mistake, e.g. days instead of years, rather than a legitimate
        duration grid).
    ValueError
        If any JSON argument is malformed or carries unknown fields.

    Sources
    -------
    See ``docs/REFERENCES.md#dynkin-hyman-vankudre-1998``.

    Examples
    --------
    >>> import json
    >>> from finstack_quant.portfolio import cell_returns_from_reference
    >>> reference = [{"duration": 1.0, "total_return": 0.02}]
    >>> table = cell_returns_from_reference(json.dumps(reference), "UST", '{"width":2.0}')
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
    config_json: str | dict[str, Any] | list[Any] | pd.DataFrame,
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
    config_json : str | dict | list | pandas.DataFrame
        JSON ``CellConfig``; ``width`` is its only field and is required.

    Returns
    -------
    DurationCellTable
        Typed cell table with ``to_dataframe()`` / ``to_json()`` exits; use
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
        If ``config_json`` is malformed.

    Sources
    -------
    See ``docs/REFERENCES.md#dynkin-hyman-vankudre-1998``.

    Examples
    --------
    >>> from datetime import date
    >>> import json
    >>> from finstack_quant.core.market_data import DiscountCurve
    >>> from finstack_quant.portfolio import cell_returns_from_curves
    >>> start = DiscountCurve.flat("start", date(2025, 1, 1), 0.02)
    >>> end = DiscountCurve.flat("end", date(2025, 4, 1), 0.03)
    >>> table = cell_returns_from_curves(start, end, 0.25, 2.0, "UST", '{"width":1.0}')
    >>> len(table.cells)
    2
    """
    ...

def excess_returns(
    positions_json: str | dict[str, Any] | list[Any] | pd.DataFrame,
    table_json: DurationCellTable | str | dict[str, Any] | list[Any] | pd.DataFrame,
) -> ExcessReturnResult:
    """
    Compute duration-matched credit excess returns against a base-return table.

    Binds Rust ``finstack_quant_portfolio::excess_returns`` (Dynkin, Hyman &
    Vankudre 1998, Appendix B): each position's ``duration`` is matched to
    its duration cell in ``table_json`` and the position's excess return is
    ``total_return - cell.base_return``, the credit-specific component of
    performance isolated from the general level/shape move of the base curve.

    Parameters
    ----------
    positions_json : str | dict | list | pandas.DataFrame
        JSON array of ``ExcessReturnPosition`` objects (``id``, ``weight``,
        ``duration``, ``total_return``); weights must sum to ``1.0`` within
        ``1e-6``.
    table_json : DurationCellTable | str | dict | list | pandas.DataFrame
        Typed ``DurationCellTable`` or its JSON form, as returned by
        :func:`cell_returns_from_reference`,
        :func:`cell_returns_from_curves`, or
        ``DurationCellTable.to_json()``.

    Returns
    -------
    ExcessReturnResult
        Typed result with per-position and portfolio-level
        total/base/excess returns; use :func:`excess_returns` for the
        raw wire string.

    Raises
    ------
    PortfolioError
        If ``table_json`` has no cells; a cell has an empty or duplicate
        label, non-finite bounds or base return, a negative lower bound, a
        non-positive width, non-ascending lower bounds, or overlaps its
        predecessor; a position has a non-finite
        weight/duration/total_return; a position's duration falls outside
        every cell, including a valid gap (the error names the position); or
        the position weights do not sum to ``1.0`` within tolerance.
    ValueError
        If any JSON argument is malformed.

    Sources
    -------
    See ``docs/REFERENCES.md#dynkin-hyman-vankudre-1998``.

    Examples
    --------
    >>> import json
    >>> from finstack_quant.portfolio import cell_returns_from_reference, excess_returns
    >>> reference = [{"duration": 1.0, "total_return": 0.02}]
    >>> table = cell_returns_from_reference(json.dumps(reference), "UST", '{"width":2.0}')
    >>> positions = [{"id": "B1", "weight": 1.0, "duration": 1.0, "total_return": 0.03}]
    >>> result = excess_returns(json.dumps(positions), table.to_json())
    >>> round(result.portfolio_excess_return, 4)
    0.01
    """
    ...
