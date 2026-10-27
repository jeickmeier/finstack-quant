"""Stub declarations mirroring ``finstack-quant-py/src/bindings/models/factor/risk/stress.rs``."""

from __future__ import annotations

import numpy as np
import numpy.typing as npt
import pandas as pd

class StressPositionEntry:
    """
    Single position's contribution to tail stress.

    Examples
    --------
    >>> from finstack_quant.models.factor.risk import StressPositionEntry
    >>> item = StressPositionEntry.from_json(
    ...     '{"position_id":"P1","avg_tail_pnl":-5.0,"pct_of_tail_loss":1.0,"worst_scenario_pnl":-10.0}'
    ... )
    >>> (item.position_id, item.worst_scenario_pnl)
    ('P1', -10.0)
    """

    @staticmethod
    def from_json(json: str) -> StressPositionEntry:
        """
        Deserialize a stress position entry from canonical JSON.

        Parameters
        ----------
        json : str
            Canonical serialized per-position tail-stress contribution, normally
            produced by ``StressPositionEntry.to_json``.

        Returns
        -------
        StressPositionEntry
            Validated `StressPositionEntry` instance reconstructed from the canonical JSON payload.

        Raises
        ------
        ValueError
            If the payload is malformed or cannot be deserialized into the documented portfolio type.

        Examples
        --------
        >>> from finstack_quant.models.factor.risk import StressPositionEntry
        >>> item = StressPositionEntry.from_json(
        ...     '{"position_id":"P1","avg_tail_pnl":-5.0,"pct_of_tail_loss":1.0,"worst_scenario_pnl":-10.0}'
        ... )
        >>> item.pct_of_tail_loss
        1.0
        """
        ...

    def to_json(self) -> str:
        """
        Serialize this stress position entry to JSON.
        Returns
        -------
        str
            Canonical JSON representation of this `StressPositionEntry`, suitable for a matching `from_json` call.

        Raises
        ------
        ValueError
            If the value cannot be serialized to JSON.
        """
        ...

    @property
    def position_id(self) -> str:
        """
        Portfolio position identifier.

        Returns
        -------
        str
            Portfolio position identifier.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

    @property
    def avg_tail_pnl(self) -> float:
        """
        Average P&L across tail scenarios.

        Returns
        -------
        float
            Average P&L across tail scenarios.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

    @property
    def pct_of_tail_loss(self) -> float:
        """
        Share of aggregate tail loss.

        Returns
        -------
        float
            Share of aggregate tail loss.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

    @property
    def worst_scenario_pnl(self) -> float:
        """
        Worst single-scenario P&L for this position.

        Returns
        -------
        float
            Worst single-scenario P&L for this position.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

    def to_dataframe(self) -> pd.DataFrame:
        """
        Export this entry as a single-row pandas DataFrame.

        Returns
        -------
        pd.DataFrame
            One row with columns ``position_id``, ``avg_tail_pnl``, ``pct_of_tail_loss``, ``worst_scenario_pnl``.

        Raises
        ------
        ValueError
            If the value cannot be serialized.
        """
        ...

    def __repr__(self) -> str:
        """Return a concise debug representation.
        Returns
        -------
        str
        """
        ...

class TailScenarioBreakdown:
    """
    Breakdown of a single tail scenario.

    Examples
    --------
    >>> from finstack_quant.models.factor.risk import TailScenarioBreakdown
    >>> scenario = TailScenarioBreakdown.from_json('{"scenario_index":0,"portfolio_pnl":-10.0,"position_pnls":[-10.0]}')
    >>> (scenario.scenario_index, scenario.portfolio_pnl)
    (0, -10.0)
    """

    @staticmethod
    def from_json(json: str) -> TailScenarioBreakdown:
        """
        Deserialize a tail scenario breakdown from canonical JSON.

        Parameters
        ----------
        json : str
            Canonical serialized tail-scenario P&L breakdown, normally
            produced by ``TailScenarioBreakdown.to_json``.

        Returns
        -------
        TailScenarioBreakdown
            Validated `TailScenarioBreakdown` instance reconstructed from the canonical JSON payload.

        Raises
        ------
        ValueError
            If the payload is malformed or cannot be deserialized into the documented portfolio type.

        Examples
        --------
        >>> from finstack_quant.models.factor.risk import TailScenarioBreakdown
        >>> scenario = TailScenarioBreakdown.from_json(
        ...     '{"scenario_index":0,"portfolio_pnl":-10.0,"position_pnls":[-10.0]}'
        ... )
        >>> scenario.position_pnls
        [-10.0]
        """
        ...

    def to_json(self) -> str:
        """
        Serialize this tail scenario breakdown to JSON.
        Returns
        -------
        str
            Canonical JSON representation of this `TailScenarioBreakdown`, suitable for a matching `from_json` call.

        Raises
        ------
        ValueError
            If the value cannot be serialized to JSON.
        """
        ...

    @property
    def scenario_index(self) -> int:
        """
        Scenario index in the source P&L matrix.

        Returns
        -------
        int
            Scenario index in the source P&L matrix.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

    @property
    def portfolio_pnl(self) -> float:
        """
        Portfolio P&L for this tail scenario.

        Returns
        -------
        float
            Portfolio P&L for this tail scenario.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

    @property
    def position_pnls(self) -> list[float]:
        """
        Per-position P&L for this scenario, index-aligned to
        ``StressAttribution.position_ids`` (entry ``i`` is the P&L for
        ``position_ids[i]``).

        Returns
        -------
        list[float]
            Per-position P&L for this scenario, index-aligned to

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

    @property
    def position_ids(self) -> list[str] | None:
        """
        Position ordering inherited from the parent ``StressAttribution``.

        Returns
        -------
        list[str] | None
            Identifiers aligned with :attr:`position_pnls`, or ``None`` for a
            breakdown reconstructed from JSON on its own.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

    def to_dataframe(self, position_ids: list[str] | None = None) -> pd.DataFrame:
        """
        Export this scenario's per-position P&L as a pandas DataFrame.

        Columns: ``position_id``, ``pnl`` (portfolio-currency amount; a loss
        is negative).

        Parameters
        ----------
        position_ids : list[str] | None
            Position identifiers aligned with :attr:`position_pnls`. Defaults
            to the ordering inherited from the parent
            ``StressAttribution.position_ids`` when this breakdown was read
            from ``tail_scenarios``; required for a breakdown built via
            :meth:`from_json`.

        Returns
        -------
        pd.DataFrame
            One row per position.

        Raises
        ------
        ValueError
            If no identifiers are available or their count does not match
            ``len(position_pnls)``.
        """
        ...

    def __repr__(self) -> str:
        """Return a concise debug representation.
        Returns
        -------
        str
        """
        ...

class StressAttribution:
    """
    Per-position attribution of portfolio losses in tail scenarios.

    Examples
    --------
    >>> from finstack_quant.models.factor.risk import StressAttribution
    >>> doc = '{"var_threshold":-5.0,"n_tail_scenarios":1,"position_ids":["P1"],"position_contributions":[],"tail_scenarios":[]}'
    >>> result = StressAttribution.from_json(doc)
    >>> (result.var_threshold, result.n_tail_scenarios)
    (-5.0, 1)
    """

    @staticmethod
    def from_json(json: str) -> StressAttribution:
        """
        Deserialize stress attribution from canonical JSON.

        Parameters
        ----------
        json : str
            Canonical serialized tail-loss attribution, normally produced by
            ``StressAttribution.to_json``.

        Returns
        -------
        StressAttribution
            Validated `StressAttribution` instance reconstructed from the canonical JSON payload.

        Raises
        ------
        ValueError
            If the payload is malformed or cannot be deserialized into the documented portfolio type.

        Examples
        --------
        >>> from finstack_quant.models.factor.risk import StressAttribution
        >>> doc = '{"var_threshold":-5.0,"n_tail_scenarios":1,"position_ids":["P1"],"position_contributions":[],"tail_scenarios":[]}'
        >>> StressAttribution.from_json(doc).position_ids
        ['P1']
        """
        ...

    def to_json(self) -> str:
        """
        Serialize this stress attribution to JSON.
        Returns
        -------
        str
            Canonical JSON representation of this `StressAttribution`, suitable for a matching `from_json` call.

        Raises
        ------
        ValueError
            If the value cannot be serialized to JSON.
        """
        ...

    @property
    def var_threshold(self) -> float:
        """
        VaR threshold used to select tail scenarios.

        Returns
        -------
        float
            VaR threshold used to select tail scenarios.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

    @property
    def n_tail_scenarios(self) -> int:
        """
        Number of scenarios classified as tail scenarios.

        Returns
        -------
        int
            Number of scenarios classified as tail scenarios.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

    @property
    def position_ids(self) -> list[str]:
        """
        Canonical position ordering shared by every ``tail_scenarios`` entry.
        ``tail_scenarios[k].position_pnls[i]`` is the P&L for ``position_ids[i]``.

        Returns
        -------
        list[str]
            Canonical position ordering shared by every ``tail_scenarios`` entry.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

    @property
    def position_contributions(self) -> list[StressPositionEntry]:
        """
        Per-position tail-loss contributions.
        Returns
        -------
        list[StressPositionEntry]

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

    @property
    def tail_scenarios(self) -> list[TailScenarioBreakdown]:
        """
        Detailed tail scenario breakdowns.
        Returns
        -------
        list[TailScenarioBreakdown]

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

    def to_dataframe(self) -> pd.DataFrame:
        """
        Export the per-position tail-loss contributions as a DataFrame.

        One row per entry of :attr:`position_contributions`, in the same
        (largest-driver-first) order.

        Columns: ``position_id``, ``avg_tail_pnl`` (portfolio-currency average
        across tail scenarios; a loss is negative), ``pct_of_tail_loss``
        (**fraction**, not percentage, of total portfolio tail loss),
        ``worst_scenario_pnl``.

        Returns
        -------
        pd.DataFrame
            One row per contributing position.

        Raises
        ------
        ValueError
            If the result cannot be serialized into a pandas object.
        """
        ...

    def to_scenario_dataframe(self) -> pd.DataFrame:
        """
        Export the tail scenario x position P&L matrix as a DataFrame.

        Rows are tail scenarios indexed by
        ``TailScenarioBreakdown.scenario_index``; columns are the position
        identifiers from :attr:`position_ids`, in that order. Every cell is a
        portfolio-currency P&L (a loss is negative).

        Columns: one per entry of :attr:`position_ids`.

        Returns
        -------
        pd.DataFrame
            One row per tail scenario, indexed by scenario index.

        Raises
        ------
        ValueError
            If any tail scenario's ``position_pnls`` width disagrees with
            ``len(position_ids)`` (only reachable through a hand-built
            :meth:`from_json` payload).
        """
        ...

    def __repr__(self) -> str:
        """Return a concise debug representation.
        Returns
        -------
        str
        """
        ...

def build_stress_attribution(
    position_ids: list[str] | None,
    position_pnls: list[list[float]] | npt.NDArray[np.float64] | pd.DataFrame,
    confidence: float | None = None,
) -> StressAttribution:
    """
    Build tail-scenario stress attribution from position P&Ls.

    Parameters
    ----------
    position_ids : list[str] | None
        Position identifiers. May be ``None`` when ``position_pnls`` is a
        ``pandas.DataFrame`` (its column labels are used).
    position_pnls : list[list[float]] | numpy.ndarray | pd.DataFrame
        P&L matrix, losses negative. A ``pandas.DataFrame`` is read as rows =
        scenarios, columns = positions. A nested list or 2-D array is read as
        ``n_positions x n_scenarios`` (position-major, one row per position).
        C-contiguous ``float64`` arrays use the direct buffer path.
    confidence : float | None
        Tail confidence level in ``(0.5, 1)``. The Rust engine selects
        ``floor((1 - confidence) * n_scenarios)`` tail scenarios. ``None``
        uses ``0.95``.

    Returns
    -------
    StressAttribution
        VaR threshold, tail scenario count, per-position tail contributions
        and scenario-level P&L breakdowns.

    Raises
    ------
    ValueError
        If dimensions are inconsistent or the matrix does not have one row per
        position id, confidence is outside ``(0.5, 1)``, the requested tail
        has zero scenarios, or any P&L is non-finite.

    Examples
    --------
    >>> from finstack_quant.models.factor.risk import build_stress_attribution
    >>> pnl = [list(range(-10, 10)), [2 * value for value in range(-10, 10)]]
    >>> result = build_stress_attribution(["A", "B"], pnl)
    >>> (result.var_threshold, result.n_tail_scenarios)
    (-30.0, 1)
    """
    ...
