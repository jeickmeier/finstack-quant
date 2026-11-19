"""Stub declarations mirroring ``finstack-quant-py/src/bindings/statements/monte_carlo.rs``."""

from __future__ import annotations

from typing import Any
import pandas as pd
from finstack_quant.core.money import Money

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
    metrics : list[str]
        Node identifiers to report; only these are sampled and stored.
    percentiles : list[float] or None
        Requested percentile levels as decimal probabilities, such as ``0.95``;
        ``None`` uses the engine defaults.
    include_path_data : bool
        Whether to retain individual path data in addition to summary outputs.

    Examples
    --------
    >>> from finstack_quant.statements import MonteCarloConfig
    >>> config = MonteCarloConfig(4, 7, ["revenue"], [0.5])
    >>> (config.n_paths, config.seed, config.metrics, config.percentiles)
    (4, 7, ['revenue'], [0.5])

    """
    def __init__(
        self,
        n_paths: int,
        seed: int,
        metrics: list[str],
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
        metrics : list[str]
            Node identifiers to report: at least one, each a node of the
            model, none repeated. Only these are sampled and stored, and the
            run holds ``n_paths * len(metrics) * forecast periods`` samples.
        percentiles : list[float] or None, default None
            Percentiles to compute, each a **decimal fraction in [0, 1]** —
            ``0.05`` is the 5th percentile, not ``5``. Stored as given; the
            engine sorts and checks them when
            ``Evaluator.evaluate_monte_carlo`` starts, which raises
            ``ValueError`` for a value that is not finite, lies outside
            [0, 1] or is repeated. ``None`` or an empty list uses the engine
            default ``[0.05, 0.5, 0.95]``.
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
        >>> config = MonteCarloConfig(4, 7, ["revenue"], [0.5])
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

    @property
    def metrics(self) -> list[str]:
        """
        Node identifiers the run reports, as supplied.

        Returns
        -------
        list[str]
            The requested metrics, in order. The engine requires at least
            one, each a node of the model and none repeated, when
            ``Evaluator.evaluate_monte_carlo`` starts.

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
    >>> payload = '{"percentile_results":{},"n_paths":2,"seed":"7","stream_version":2,"metrics":[],"percentiles":[0.5],"forecast_periods":[]}'
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
        >>> payload = '{"percentile_results":{},"n_paths":2,"seed":"7","stream_version":2,"metrics":[],"percentiles":[0.5],"forecast_periods":[]}'
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
        Metric (node) identifiers that were requested, in the requested order.

        Returns
        -------
        list[str]
            The run's metrics. Each has a percentile summary when the run is
            complete.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

    @property
    def incomplete(self) -> dict[str, Any] | None:
        """
        Why the run published no statistics, or ``None`` for a complete run.

        A run is complete only when every path has a value for every
        requested metric in every forecast period.

        Returns
        -------
        dict[str, Any] | None
            ``{"metric", "period", "path", "reason", "missing_samples"}``: the
            first sample without a value in metric, period and path order, why
            its cell had none (``"missing"``, ``"masked"`` or an unavailable
            reason such as ``"missing_peer_shock"``), and how many samples
            had none across the run.

        Raises
        ------
        ValueError
            If the stored record cannot be serialized.
        """
        ...

    @property
    def seed(self) -> int:
        """
        The run's exact base seed.

        Returns
        -------
        int
            The seed the run was configured with. With ``stream_version`` it
            replays any path from the result alone.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

    @property
    def stream_version(self) -> int:
        """
        Version of the mapping from seed, path and node to a random stream.

        Returns
        -------
        int
            The stream version the run used.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

    @property
    def warnings(self) -> list[dict[str, Any]]:
        """
        Distinct warnings raised in the simulated forecast periods, in their serde form.

        Each warning is reported once however many paths raised it, in a
        canonical sorted order. Warnings in actual periods are omitted: those
        periods are not simulated and every path repeats the deterministic
        evaluation's.

        One dict per ``EvalWarning`` with a single snake_case variant key
        (``"division_by_zero"``, ``"non_finite_value"``, ...) whose value holds
        the variant fields (periods as ``"2025Q1"``-style ids, non-finite
        numbers as ``"nan"`` / ``"inf"`` / ``"-inf"``), identical to the WASM
        ``Evaluator.evaluateMonteCarlo`` result.

        Returns
        -------
        list[dict[str, Any]]
            One dict per distinct forecast-period warning; empty when every
            path evaluated its forecast periods cleanly.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

    def breach_probability(self, metric: str, threshold: float | Money) -> float | None:
        """
        Probability that ``metric`` exceeds ``threshold`` in any forecast period.

        A path counts once when any forecast-period value is strictly greater
        than the threshold. For a floor, simulate a derived node that flips
        the sign.

        Parameters
        ----------
        metric : str
            A requested metric.
        threshold : float | Money
            Finite upper threshold in the metric's own units: a ``float`` for
            a scalar metric, ``Money`` in the metric's currency for a monetary
            one.

        Returns
        -------
        float | None
            Fraction of paths in ``[0, 1]`` with at least one breach, or
            ``None`` when no path samples are available: the run is
            incomplete, the metric was not requested, or the result was
            restored without path data (``include_path_data=True``).

        Raises
        ------
        ValueError
            If the threshold is not finite or not in the metric's units, or a
            restored path table does not hold exactly one finite sample for
            every path and forecast period.
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
            percentile). It must equal a configured percentile exactly; nothing
            is interpolated between configured levels.

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
        the quantile in percent — ``p5``, ``p50``, ``p95`` for the default
        levels. A cell is ``NaN`` when a period is missing that
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
