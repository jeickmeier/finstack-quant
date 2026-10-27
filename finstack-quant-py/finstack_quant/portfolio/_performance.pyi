"""Stub declarations mirroring ``finstack-quant-py/src/bindings/portfolio/performance.rs``."""

from __future__ import annotations

import datetime
from typing import Any
import pandas as pd

class LinkedReturn:
    """
    Result of geometrically linking TWRR sub-period returns.

    Returned by :func:`twrr_linked`. For horizons below one year
    :attr:`annualised` mirrors :attr:`cumulative` (GIPS 2020 reports such
    horizons cumulatively).

    Examples
    --------
    >>> from finstack_quant.portfolio import twrr_linked
    >>> result = twrr_linked("[0.05,0.03]", horizon_years=1.0)
    >>> (round(result.cumulative, 4), result.num_periods)
    (0.0815, 2)
    """

    @property
    def cumulative(self) -> float:
        """
        Cumulative return over the full horizon.

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
    def annualised(self) -> float:
        """
        Annualised return; mirrors ``cumulative`` below one year.

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
    def num_periods(self) -> int:
        """
        Number of sub-periods that were linked.

        Returns
        -------
        int
            The stored count from the canonical Rust result.

        Notes
        -----
        This accessor does not raise; it returns the stored or derived value.
        """
        ...

    def to_dataframe(self) -> pd.DataFrame:
        """
        Single-row pandas DataFrame view of the linked return.

        Columns: ``cumulative``, ``annualised``, ``num_periods``. The schema is pinned, so an empty result keeps the
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
    def from_json(json: str) -> LinkedReturn:
        """
        Deserialize a ``LinkedReturn`` from canonical JSON.

        Parameters
        ----------
        json : str
            Canonical payload produced by :meth:`to_json` or the paired
            ``*_json`` wire function.

        Returns
        -------
        LinkedReturn
            Validated instance reconstructed from the canonical JSON payload.

        Raises
        ------
        ValueError
            If the payload is malformed or does not match the serialized
            ``LinkedReturn`` schema.

        Examples
        --------
        >>> from finstack_quant.portfolio import LinkedReturn
        >>> doc = '{"cumulative": 0.0815, "annualised": 0.0815, "num_periods": 2}'
        >>> LinkedReturn.from_json(doc).num_periods
        2
        """
        ...

def twrr_modified_dietz(period: dict[str, Any] | str) -> float:
    """
    Compute a Modified-Dietz TWRR sub-period return.

    Parameters
    ----------
    period : dict | str
        ``TwrrPeriod`` as a dict or JSON string: ``beginning_market_value``,
        ``ending_market_value`` and optional ``cashflows: [{amount,
        fraction_of_period_remaining}]`` (omitted means no flows). A positive
        ``amount`` is a contribution into the portfolio; the fraction, in
        ``[0, 1]``, is the share of the period remaining after the flow.
        Unknown keys are rejected. The same input the WASM
        ``twrrModifiedDietz`` takes.

    Returns
    -------
    float
        Sub-period time-weighted return as a decimal.

    Raises
    ------
    ValueError
        If the period is malformed or has unknown keys, a cashflow weight lies
        outside ``[0, 1]``, or the Dietz denominator is non-positive.

    Examples
    --------
    >>> from finstack_quant.portfolio import twrr_modified_dietz
    >>> twrr_modified_dietz({"beginning_market_value": 100.0, "ending_market_value": 110.0})
    0.1
    >>> twrr_modified_dietz({
    ...     "beginning_market_value": 100.0,
    ...     "ending_market_value": 110.0,
    ...     "cashflows": [{"amount": 5.0, "fraction_of_period_remaining": 0.5}],
    ... }) == 5.0 / 102.5
    True
    """
    ...

def twrr_linked(returns_json: str | dict[str, Any] | list[Any] | pd.DataFrame, horizon_years: float) -> LinkedReturn:
    """
    Geometrically link TWRR sub-period returns over a horizon.

    Parameters
    ----------
    returns_json : str | dict | list | pandas.DataFrame
        JSON array of sub-period decimal returns (e.g. from Modified Dietz).
    horizon_years : float
        Reporting horizon in years used to annualize the linked return.

    Returns
    -------
    LinkedReturn
        Typed result with ``cumulative``, ``annualised`` and
        ``num_periods``; use serialize with ``to_json()`` for the wire string.

    Raises
    ------
    ValueError
        If ``returns_json`` is malformed, any sub-period return is non-finite
        or at most -1, or the compounded growth factor is non-positive.

    Examples
    --------
    >>> import json
    >>> from finstack_quant.portfolio import twrr_linked
    >>> result = twrr_linked("[0.05,0.03]", horizon_years=1.0)
    >>> round(result.cumulative, 4)
    0.0815
    """
    ...

def mwr_xirr(
    cashflows: list[tuple[datetime.date | str, float]] | list[dict[str, Any]] | pd.DataFrame | str,
) -> float:
    """
    Compute the money-weighted return (XIRR, Act/365F) from dated cashflows.

    Binds Rust ``mwr_xirr``.

    Parameters
    ----------
    cashflows : list[tuple[date, float]] | list[dict] | pd.DataFrame | str
        Dated flows from the investor's cash account: contributions negative,
        terminal value / distributions positive. Accepts ``(date, amount)``
        pairs (dates as ``datetime.date`` or ISO strings), dicts with
        ``date`` and ``amount`` keys, a DataFrame with those columns, or the
        canonical JSON array string. Dates are sorted and equal-date flows
        netted; remaining nonzero flows must change sign exactly once.

    Returns
    -------
    float
        Unique internal rate of return as a finite decimal annualized rate
        greater than -1, using Act/365F year fractions.

    Raises
    ------
    ValueError
        If the flows are malformed, have fewer than two nonzero net dates,
        do not have exactly one net sign change, or no sufficiently accurate
        finite return greater than -1 can be found. Nonconventional cashflows
        are rejected even if a solver could find one of their possible roots.
    RuntimeError
        If the numerical solver fails to converge within the valid return bracket.

    Examples
    --------
    >>> import datetime as dt
    >>> from finstack_quant.portfolio import mwr_xirr
    >>> round(mwr_xirr([(dt.date(2025, 1, 1), -100.0), (dt.date(2026, 1, 1), 110.0)]), 6)
    0.1
    >>> round(mwr_xirr('[{"date":"2025-01-01","amount":-100.0},{"date":"2026-01-01","amount":110.0}]'), 6)
    0.1
    """
    ...
