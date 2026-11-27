"""Stub declarations mirroring ``finstack-quant-py/src/bindings/portfolio/replay.rs``."""

from __future__ import annotations

import datetime
from typing import Any, Literal
import pandas as pd
import polars as pl
from finstack_quant.core.market_data import MarketContext
from finstack_quant.core.money import Money
from finstack_quant.core.table import ArrowTable
from finstack_quant.portfolio._attribution import PortfolioAttribution
from finstack_quant.portfolio._container import Portfolio
from finstack_quant.portfolio._valuation import PortfolioValuation

class ReplayConfig:
    """Configure replay mode, per-snapshot valuation, and failure policy.

    Examples
    --------
    >>> from finstack_quant.portfolio import ReplayConfig
    >>> ReplayConfig("pv_and_pnl", on_error="best_effort").on_error
    'best_effort'
    """

    def __init__(
        self,
        mode: Literal["pv_only", "pv_and_pnl", "full_attribution"],
        *,
        attribution_method: str = "parallel",
        valuation_options: dict[str, Any] | str | None = None,
        on_error: Literal["strict", "best_effort"] = "strict",
    ) -> None:
        """Build replay settings for one timeline.

        Parameters
        ----------
        mode : str
            pv_only for marks, pv_and_pnl for mark changes, or full_attribution
            for mark changes with the configured attribution method.
        attribution_method : str
            Canonical attribution method label. Used in full_attribution only.
        valuation_options : dict or str or None
            Valuation option mapping or canonical JSON for each replay date.
            Omit to use the standard valuation profile.
        on_error : str
            strict fails on a snapshot error. best_effort records skipped dates.

        Raises
        ------
        ValueError
            If a label is unknown or valuation options are malformed.
        """
        ...

    @property
    def mode(self) -> str:
        """Requested replay calculation mode.

        Returns
        -------
        str
            Requested replay calculation mode.

        Raises
        ------
        ValueError
            If the stored field cannot be converted to its Python representation.
        """
        ...

    @property
    def attribution_method(self) -> str:
        """Canonical label of the attribution method.

        Returns
        -------
        str
            Canonical label of the attribution method.

        Raises
        ------
        ValueError
            If the stored field cannot be converted to its Python representation.
        """
        ...

    @property
    def valuation_options(self) -> dict[str, Any]:
        """Valuation option fields used for each snapshot.

        Returns
        -------
        dict[str, Any]
            Valuation option fields used for each snapshot.

        Raises
        ------
        ValueError
            If the stored field cannot be converted to its Python representation.
        """
        ...

    @property
    def on_error(self) -> str:
        """Canonical policy for failures during replay.

        Returns
        -------
        str
            Canonical policy for failures during replay.

        Raises
        ------
        ValueError
            If the stored field cannot be converted to its Python representation.
        """
        ...

class ReplayStep:
    """Single dated valuation with its associated mark-to-market change.

    Examples
    --------
    >>> from finstack_quant.core.market_data import MarketContext
    >>> from finstack_quant.portfolio import Portfolio, ReplayConfig, replay_portfolio
    >>> portfolio = Portfolio.builder("replay", "USD", "2025-01-15").build()
    >>> run = replay_portfolio(portfolio, [("2025-01-15", MarketContext())], ReplayConfig("pv_only"))
    >>> run.steps[0].date.isoformat()
    '2025-01-15'
    """

    @property
    def date(self) -> datetime.date:
        """Replay valuation date.

        Returns
        -------
        datetime.date
            Replay valuation date.

        Raises
        ------
        ValueError
            If the recorded date is outside the Python datetime range.
        """
        ...

    @property
    def valuation(self) -> PortfolioValuation:
        """Full portfolio valuation on this date.

        Returns
        -------
        PortfolioValuation
            Full portfolio valuation on this date.

        Notes
        -----
        This accessor does not raise.
        """
        ...

    @property
    def daily_mtm_pnl(self) -> Money | None:
        """Change in marked value since the previous snapshot, excluding paid cashflows. None at the first snapshot or in PV-only mode.

        Returns
        -------
        Money | None
            Change in marked value since the previous snapshot, excluding paid cashflows. None at the first snapshot or in PV-only mode.

        Notes
        -----
        This accessor does not raise.
        """
        ...

    @property
    def cumulative_mtm_pnl(self) -> Money | None:
        """Change in marked value since the first snapshot, excluding paid cashflows. None at the first snapshot or in PV-only mode.

        Returns
        -------
        Money | None
            Change in marked value since the first snapshot, excluding paid cashflows. None at the first snapshot or in PV-only mode.

        Notes
        -----
        This accessor does not raise.
        """
        ...

    @property
    def attribution(self) -> PortfolioAttribution | None:
        """Factor attribution from the prior snapshot, available only in full-attribution mode after the first step.

        Returns
        -------
        PortfolioAttribution | None
            Factor attribution from the prior snapshot, available only in full-attribution mode after the first step.

        Notes
        -----
        This accessor does not raise.
        """
        ...

class ReplaySummary:
    """Aggregate mark-to-market and drawdown diagnostics across the replay.

    Examples
    --------
    >>> from finstack_quant.core.market_data import MarketContext
    >>> from finstack_quant.portfolio import Portfolio, ReplayConfig, replay_portfolio
    >>> portfolio = Portfolio.builder("replay", "USD", "2025-01-15").build()
    >>> run = replay_portfolio(portfolio, [("2025-01-15", MarketContext())], ReplayConfig("pv_only"))
    >>> run.summary.num_steps
    1
    """

    @property
    def start_date(self) -> datetime.date:
        """First valued snapshot date.

        Returns
        -------
        datetime.date
            First valued snapshot date.

        Raises
        ------
        ValueError
            If the recorded date is outside the Python datetime range.
        """
        ...

    @property
    def end_date(self) -> datetime.date:
        """Last valued snapshot date.

        Returns
        -------
        datetime.date
            Last valued snapshot date.

        Raises
        ------
        ValueError
            If the recorded date is outside the Python datetime range.
        """
        ...

    @property
    def num_steps(self) -> int:
        """Number of successfully valued snapshots, including the first.

        Returns
        -------
        int
            Number of successfully valued snapshots, including the first.

        Notes
        -----
        This accessor does not raise.
        """
        ...

    @property
    def start_value(self) -> Money:
        """Portfolio value at the first snapshot in the reporting currency.

        Returns
        -------
        Money
            Portfolio value at the first snapshot in the reporting currency.

        Notes
        -----
        This accessor does not raise.
        """
        ...

    @property
    def end_value(self) -> Money:
        """Portfolio value at the last snapshot in the reporting currency.

        Returns
        -------
        Money
            Portfolio value at the last snapshot in the reporting currency.

        Notes
        -----
        This accessor does not raise.
        """
        ...

    @property
    def total_mtm_pnl(self) -> Money:
        """End value minus start value, excluding paid cashflows, in reporting currency.

        Returns
        -------
        Money
            End value minus start value, excluding paid cashflows, in reporting currency.

        Notes
        -----
        This accessor does not raise.
        """
        ...

    @property
    def max_mtm_drawdown(self) -> Money:
        """Largest marked-value decline from a prior peak measured in the reporting currency.

        Returns
        -------
        Money
            Largest marked-value decline from a prior peak measured in the reporting currency.

        Notes
        -----
        This accessor does not raise.
        """
        ...

    @property
    def max_mtm_drawdown_pct(self) -> float:
        """Largest marked-value decline as a fraction of a positive peak. Zero if there was no positive peak.

        Returns
        -------
        float
            Largest marked-value decline as a fraction of a positive peak. Zero if there was no positive peak.

        Notes
        -----
        This accessor does not raise.
        """
        ...

    @property
    def max_mtm_drawdown_peak_date(self) -> datetime.date:
        """Peak date for the largest currency drawdown.

        Returns
        -------
        datetime.date
            Peak date for the largest currency drawdown.

        Raises
        ------
        ValueError
            If the recorded date is outside the Python datetime range.
        """
        ...

    @property
    def max_mtm_drawdown_trough_date(self) -> datetime.date:
        """Trough date for the largest currency drawdown.

        Returns
        -------
        datetime.date
            Trough date for the largest currency drawdown.

        Raises
        ------
        ValueError
            If the recorded date is outside the Python datetime range.
        """
        ...

    @property
    def max_mtm_drawdown_pct_peak_date(self) -> datetime.date | None:
        """Peak date for the largest fractional drawdown, or None if no positive peak produced a decline.

        Returns
        -------
        datetime.date | None
            Peak date for the largest fractional drawdown, or None if no positive peak produced a decline.

        Raises
        ------
        ValueError
            If the recorded date is outside the Python datetime range.
        """
        ...

    @property
    def max_mtm_drawdown_pct_trough_date(self) -> datetime.date | None:
        """Trough date for the largest fractional drawdown, or None if no positive peak produced a decline.

        Returns
        -------
        datetime.date | None
            Trough date for the largest fractional drawdown, or None if no positive peak produced a decline.

        Raises
        ------
        ValueError
            If the recorded date is outside the Python datetime range.
        """
        ...

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
    >>> ReplayResult.from_json(json.dumps(doc)).summary.num_steps
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
    def steps(self) -> list[ReplayStep]:
        """Valuations and optional P&L by snapshot date, in timeline order.

        Returns
        -------
        list[ReplayStep]
            Valuations and optional P&L by snapshot date, in timeline order.

        Notes
        -----
        This accessor does not raise.
        """
        ...

    @property
    def summary(self) -> ReplaySummary:
        """Aggregate valuation and marked drawdown for the run.

        Returns
        -------
        ReplaySummary
            Aggregate valuation and marked drawdown for the run.

        Notes
        -----
        This accessor does not raise.
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

    def to_arrow(self) -> ArrowTable:
        """
        Per-step value and P&L ladder as an :class:`~finstack_quant.core.table.ArrowTable`.

        Columns: ``date``, ``value``, ``daily_mtm_pnl``, ``cumulative_mtm_pnl``. The schema is pinned, so an empty result keeps the
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
        Per-step value and P&L ladder as a pandas ``DataFrame``.

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
        Per-step value and P&L ladder as a Polars ``DataFrame``.

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
        Per-step value and P&L ladder as a ``pyarrow.Table``.

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

def replay_portfolio(
    portfolio: Portfolio,
    snapshots: list[tuple[datetime.date | str, MarketContext]] | list[dict[str, Any]] | str,
    config: ReplayConfig | dict[str, Any] | str,
) -> ReplayResult:
    """
    Replay a portfolio through dated market snapshots.

    Parameters
    ----------
    portfolio : Portfolio
        Built :class:`Portfolio` to replay.
    snapshots : list[tuple[date, MarketContext]] | list[dict] | str
        Dated market snapshots in ascending order: ``(date, market)`` pairs
        (dates as ``datetime.date`` or ISO strings), JSON-shaped
        ``{"date": "YYYY-MM-DD", "market": {...}}`` dicts, or the canonical
        JSON array string.
    config : ReplayConfig | dict | str
        ``ReplayConfig`` as a dict or JSON string: required ``mode``
        (``"pv_only"``, ``"pv_and_pnl"`` or ``"full_attribution"``) and
        optional ``attribution_method``, ``valuation_options`` and
        ``on_error``; unknown keys are rejected. The same input the WASM
        ``replayPortfolio`` takes.

    Returns
    -------
    ReplayResult
        Typed result with ``steps``, ``summary`` and ``skipped_dates``
        getters plus ``to_arrow()`` / ``to_pandas()`` / ``to_polars()`` /
        ``to_pyarrow()``; serialize with ``to_json()`` for the
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
    >>> from finstack_quant.portfolio import Portfolio, replay_portfolio
    >>> spec = '{"id":"empty","base_currency":"USD","as_of":"2025-01-01","entities":{},"positions":[]}'
    >>> try:
    ...     replay_portfolio(Portfolio.from_spec(spec), [], {"mode": "pv_only"})
    ... except ValueError as exc:
    ...     print("must be non-empty" in str(exc))
    True
    """
    ...
