"""Stub declarations mirroring ``finstack-quant-py/src/bindings/statements/monte_carlo.rs``."""

from __future__ import annotations

from typing import Any
import pandas as pd

class MonteCarloConfig:
    """
    Set reproducible Monte Carlo sampling and retained-output options.

    Parameters
    ----------
    n_paths : int
        Number of stochastic paths to simulate; larger values improve sampling
        precision at greater runtime and memory cost.
    seed : int
        Deterministic random-number seed used to reproduce the simulation.
    percentiles : list[float] or None
        Requested percentile levels as decimal probabilities, such as ``0.95``;
        ``None`` uses the engine defaults.
    include_path_data : bool
        Whether to retain individual path data in addition to summary outputs.

    Examples
    --------
    >>> from finstack_quant.statements import MonteCarloConfig
    >>> config = MonteCarloConfig(4, 7, [0.5])
    >>> (config.n_paths, config.seed, config.percentiles)
    (4, 7, [0.5])

    """
    def __init__(
        self,
        n_paths: int,
        seed: int,
        percentiles: list[float] | None = ...,
        include_path_data: bool = ...,
    ) -> None:
        """
        Configure a Monte Carlo run over a statement model's forecast periods.

        Parameters
        ----------
        n_paths : int
            Number of simulated paths. Percentile standard error scales as
            ``1 / sqrt(n_paths)``, so tails need proportionally more paths:
            roughly 1 000–2 000 for means/medians, 5 000–10 000 for the 5th
            and 95th percentiles, and 10 000+ for 1st/99th percentiles, CVaR,
            or breach probabilities. No minimum is imposed.
        seed : int
            Base seed from which per-path and per-node seeds are derived. The
            same seed reproduces the run exactly, in serial and in parallel.
        percentiles : list[float] or None, default None
            Percentiles to compute, each a **decimal fraction in [0, 1]** —
            ``0.05`` is the 5th percentile, not ``5``. Stored as given; the
            engine sorts, deduplicates and range-checks them when
            ``Evaluator.evaluate_monte_carlo`` starts, which raises
            ``ValueError`` for a value that is not finite or lies outside
            [0, 1]. ``None`` or an empty list uses the engine default
            ``[0.05, 0.5, 0.95]``.
        include_path_data : bool, default False
            Whether to retain the full per-path long table. Off by default
            because it grows as ``n_paths * metrics * forecast periods``.

        Notes
        -----
        Construction does not raise; arguments are stored as supplied.
        """
        ...

    @staticmethod
    def from_json(json: str) -> MonteCarloConfig:
        """
        Deserialize Monte Carlo sampling settings from canonical JSON.

        Parameters
        ----------
        json : str
            JSON payload describing paths, seed, percentile levels, and output
            retention.

        Returns
        -------
        MonteCarloConfig
            Validated configuration reconstructed from canonical JSON.

        Raises
        ------
        ValueError
            If ``json`` is malformed or violates the serialized schema.

        Examples
        --------
        >>> from finstack_quant.statements import MonteCarloConfig
        >>> config = MonteCarloConfig(4, 7, [0.5])
        >>> MonteCarloConfig.from_json(config.to_json()).n_paths
        4

        """
        ...

    def to_json(self) -> str:
        """
        Serialize this configuration to canonical JSON.

        Returns
        -------
        str
            Canonical JSON accepted by :meth:`from_json` and
            :meth:`Evaluator.evaluate_monte_carlo`.

        Raises
        ------
        ValueError
            If the value cannot be serialized to JSON.
        """
        ...

    @property
    def n_paths(self) -> int:
        """
        Number of paths the simulation will draw.

        Returns
        -------
        int
            Configured stochastic path count.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

    @property
    def seed(self) -> int:
        """
        Base seed from which per-path and per-node seeds are derived.

        Reusing a seed reproduces the run bit-for-bit; serial and parallel
        execution agree.

        Returns
        -------
        int
            Seed used to reproduce path sampling.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

    @property
    def percentiles(self) -> list[float]:
        """
        Percentiles to compute, as decimal fractions in [0, 1].

        Returns
        -------
        list[float]
            Quantile levels exactly as supplied (not yet normalized) —
            ``0.05`` is the 5th percentile, not ``5``. See
            ``MonteCarloResults.percentiles`` for the sorted, deduplicated
            levels a run computed.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

    @property
    def include_path_data(self) -> bool:
        """
        Whether the run retains the full per-path table alongside percentiles.

        Returns
        -------
        bool
            ``True`` when the result includes path-level data, reachable via
            :meth:`MonteCarloResults.to_paths_dataframe`.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

class MonteCarloResults:
    """
    Hold percentile summaries from statement-model Monte Carlo evaluation.

    Examples
    --------
    >>> from finstack_quant.statements import MonteCarloResults
    >>> payload = '{"percentile_results":{},"n_paths":2,"percentiles":[0.5],"forecast_periods":[],"path_data":null}'
    >>> MonteCarloResults.from_json(payload).n_paths
    2

    """

    @staticmethod
    def from_json(json: str) -> MonteCarloResults:
        """
        Deserialize Monte Carlo output from canonical JSON.

        Parameters
        ----------
        json : str
            JSON payload returned by :meth:`Evaluator.evaluate_monte_carlo` or a matching
            serialized result.

        Returns
        -------
        MonteCarloResults
            Validated result reconstructed from canonical JSON.

        Raises
        ------
        ValueError
            If ``json`` is malformed or violates the serialized schema.

        Examples
        --------
        >>> from finstack_quant.statements import MonteCarloResults
        >>> payload = '{"percentile_results":{},"n_paths":2,"percentiles":[0.5],"forecast_periods":[],"path_data":null}'
        >>> MonteCarloResults.from_json(payload).forecast_periods
        []

        """
        ...

    def to_json(self) -> str:
        """
        Serialize these results to canonical JSON.

        Returns
        -------
        str
            Canonical JSON accepted by :meth:`from_json`.

        Raises
        ------
        ValueError
            If the value cannot be serialized to JSON.
        """
        ...

    @property
    def n_paths(self) -> int:
        """
        Number of paths actually simulated.

        The aggregator fails the run unless this equals the configured
        ``n_paths``, so a partial simulation never reaches a result.

        Returns
        -------
        int
            Path count represented by the summaries.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

    @property
    def percentiles(self) -> list[float]:
        """
        Percentiles computed for every metric and period.

        Returns
        -------
        list[float]
            Sorted, deduplicated quantile levels as decimal fractions in
            [0, 1] — ``0.5`` is the median.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

    @property
    def forecast_periods(self) -> list[str]:
        """
        Forecast periods covered by the simulation, in evaluation order.

        Only non-actual periods are simulated, so historical periods are absent
        from this list and from every percentile series.

        Returns
        -------
        list[str]
            Period identifier strings (e.g. ``"2025Q1"``).

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

    @property
    def metrics(self) -> list[str]:
        """
        Metric (node) identifiers that were simulated, in model order.

        Returns
        -------
        list[str]
            Node identifiers with percentile summaries.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

    @property
    def warnings(self) -> list[dict[str, Any]]:
        """
        Warnings raised while evaluating paths, in their serde form.

        One dict per ``EvalWarning`` with a single snake_case variant key
        (``"division_by_zero"``, ``"non_finite_value"``, ...) whose value holds
        the variant fields (periods as ``"2025Q1"``-style ids, non-finite
        numbers as ``"nan"`` / ``"inf"`` / ``"-inf"``), identical to the WASM
        ``Evaluator.evaluateMonteCarlo`` result.

        Returns
        -------
        list[dict[str, Any]]
            One dict per recorded warning; empty when every path evaluated
            cleanly.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

    def breach_probability(self, metric: str, threshold: float) -> float | None:
        """
        Probability that ``metric`` exceeds ``threshold`` in any forecast period.

        Counts upside breaches only (``value > threshold``) across the
        simulated paths; for a downside test simulate a negated metric or a
        derived node that flips the sign.

        Parameters
        ----------
        metric : str
            Simulated node identifier.
        threshold : float
            Breach level in the metric's own units.

        Returns
        -------
        float | None
            Fraction of paths in ``[0, 1]`` with at least one breach, or
            ``None`` when the metric was not simulated, there are no forecast
            periods, or the retained path data is incomplete. Results restored
            from JSON / pickle support this calculation when they include
            complete path data (``include_path_data=True``).

        Notes
        -----
        This method reads already-computed state and does not raise.
        """
        ...

    def percentile_by_period(self, metric: str, percentile: float) -> dict[str, float] | None:
        """
        Look up one percentile of one metric as a period-keyed dict.

        Parameters
        ----------
        metric : str
            Node identifier whose simulated distribution to read.
        percentile : float
            Quantile as a decimal fraction in [0, 1] (``0.95`` for the 95th
            percentile). It must match a configured percentile to within
            1e-12; nothing is interpolated between configured levels.

        Returns
        -------
        dict[str, float] or None
            Period identifier → percentile value, in forecast-period order.
            ``None`` when the metric is unknown or the percentile was not
            configured for this run.

        Notes
        -----
        This method does not raise; unknown metrics or unconfigured percentiles
        return ``None``.
        """
        ...

    def to_dataframe(self, metric: str) -> pd.DataFrame:
        """
        Export one metric's percentile fan as a pandas ``DataFrame``.

        Rows are the metric's forecast periods (index name ``period``, in
        evaluation order); each column holds one configured percentile of the
        simulated distribution, in the metric's own units (currency amounts for
        monetary nodes, unitless otherwise).

        Columns: one per configured percentile, named ``p<q>`` where ``q`` is
        the quantile as a decimal fraction — ``p0.05``, ``p0.5``, ``p0.95`` for
        the default levels. A cell is ``NaN`` when a period is missing that
        percentile.

        Parameters
        ----------
        metric : str
            Node identifier whose fan chart to build.

        Returns
        -------
        pd.DataFrame
            Percentile-by-period frame indexed by period identifier.

        Raises
        ------
        KeyError
            If ``metric`` was not simulated.

        """
        ...

    def to_paths_dataframe(self) -> pd.DataFrame:
        """
        Export the retained per-path values as a long-format ``DataFrame``.

        Populated only when the run was configured with
        ``include_path_data=True``; otherwise the frame is empty but still
        carries the documented columns. Rows are ordered canonically by
        ``(path_id, metric, period)`` so serial and parallel runs match.

        Columns: ``path_id`` (0-based path index), ``period`` (period
        identifier string), ``metric`` (node identifier), ``value`` (float64,
        in the metric's own units).

        Returns
        -------
        pd.DataFrame
            Long-format per-path frame.

        Notes
        -----
        This accessor does not raise; it returns the stored or derived value.
        """
        ...
