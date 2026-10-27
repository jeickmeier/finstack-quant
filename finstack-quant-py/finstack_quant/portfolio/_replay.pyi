"""Stub declarations mirroring ``finstack-quant-py/src/bindings/portfolio/replay.rs``."""

from __future__ import annotations

import datetime
from typing import Any
import pandas as pd
from finstack_quant.core.market_data import MarketContext
from finstack_quant.portfolio._container import Portfolio

class ReplayResult:
    """
    Full output of a historical portfolio replay run.

    Returned by :func:`replay_portfolio`. Carries per-step valuations, the
    aggregate summary, and any best-effort skipped snapshots.

    Examples
    --------
    >>> import json
    >>> from finstack_quant.portfolio import ReplayResult
    >>> money = {"amount": "0", "currency": "USD"}
    >>> doc = {
    ...     "steps": [],
    ...     "summary": {
    ...         "start_date": "2025-01-15",
    ...         "end_date": "2025-01-15",
    ...         "num_steps": 0,
    ...         "start_value": money,
    ...         "end_value": money,
    ...         "total_mtm_pnl": money,
    ...         "max_mtm_drawdown": money,
    ...         "max_mtm_drawdown_pct": 0.0,
    ...         "max_mtm_drawdown_peak_date": "2025-01-15",
    ...         "max_mtm_drawdown_trough_date": "2025-01-15",
    ...     },
    ... }
    >>> ReplayResult.from_json(json.dumps(doc)).summary["num_steps"]
    0
    """

    @staticmethod
    def from_json(json: str) -> ReplayResult:
        """
        Deserialize a replay result from canonical JSON.

        Parameters
        ----------
        json : str
            JSON produced by :meth:`to_json`.

        Returns
        -------
        ReplayResult
            Reconstructed result.

        Raises
        ------
        ValueError
            If the payload is malformed.

        Examples
        --------
        >>> from finstack_quant.portfolio import ReplayResult
        >>> try:
        ...     ReplayResult.from_json("{}")
        ... except ValueError:
        ...     print("missing fields")
        missing fields
        """
        ...

    @property
    def steps(self) -> list[dict[str, object]]:
        """
        Per-step output as records, full valuations included.

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
    def summary(self) -> dict[str, object]:
        """
        Aggregate statistics across the full replay.

        Returns
        -------
        dict[str, object]
            JSON-shaped view of the canonical Rust field.

        Notes
        -----
        This accessor does not raise; it returns the stored or derived value.
        """
        ...

    @property
    def skipped_dates(self) -> list[list[object]]:
        """
        Best-effort skipped snapshots as ``(date, reason)`` pairs.

        Returns
        -------
        list[list[object]]
            ``[date, reason]`` pairs; empty for strict-mode runs.

        Notes
        -----
        This accessor does not raise; it returns the stored or derived value.
        """
        ...

    def to_dataframe(self) -> pd.DataFrame:
        """
        Per-step value and P&L ladder as a pandas DataFrame.

        Columns: ``date``, ``value``, ``daily_mtm_pnl``, ``cumulative_mtm_pnl``. The schema is pinned, so an empty result keeps the
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

def replay_portfolio(
    portfolio: Portfolio | str,
    snapshots: list[tuple[datetime.date | str, MarketContext | str]] | list[dict[str, Any]] | str,
    config: dict[str, Any] | str,
) -> ReplayResult:
    """
    Replay a portfolio through dated market snapshots.

    Parameters
    ----------
    portfolio : Portfolio or str
        Typed :class:`Portfolio` or JSON ``PortfolioSpec``.
    snapshots : list[tuple[date, MarketContext | str]] | list[dict] | str
        Dated market snapshots in ascending order: ``(date, market)`` pairs
        (dates as ``datetime.date`` or ISO strings), JSON-shaped
        ``{"date": "YYYY-MM-DD", "market": {...}}`` dicts, or the canonical
        JSON array string.
    config : dict | str
        ``ReplayConfig`` as a dict or JSON string: required ``mode``
        (``"pv_only"``, ``"pv_and_pnl"`` or ``"full_attribution"``) and
        optional ``attribution_method``, ``valuation_options`` and
        ``on_error``; unknown keys are rejected. The same input the WASM
        ``replayPortfolio`` takes.

    Returns
    -------
    ReplayResult
        Typed result with ``steps``, ``summary`` and ``skipped_dates``
        getters plus ``to_dataframe()``; serialize with ``to_json()`` for the
        wire string.

    Raises
    ------
    ValueError
        If the snapshots or config are malformed (a missing ``mode`` or an
        empty timeline is rejected).
    PortfolioError
        If a snapshot valuation fails under a strict error policy.

    Examples
    --------
    >>> from finstack_quant.portfolio import replay_portfolio
    >>> spec = '{"id":"empty","base_currency":"USD","as_of":"2025-01-01","entities":{},"positions":[]}'
    >>> try:
    ...     replay_portfolio(spec, [], {"mode": "pv_only"})
    ... except ValueError as exc:
    ...     print("must be non-empty" in str(exc))
    True
    """
    ...
