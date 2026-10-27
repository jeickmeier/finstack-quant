"""Stub declarations mirroring ``finstack-quant-py/src/bindings/statements_analytics/scorecards.rs``."""

from __future__ import annotations

from collections.abc import Mapping
from typing import Any
import pandas as pd
from finstack_quant.statements import FinancialModelSpec, StatementResult

class ScorecardMetric:
    """
    Define one weighted metric in a credit-rating scorecard.

    Parameters
    ----------
    name : str
        Stable metric label used in scorecard reports and validation errors.
    formula : str
        Statement-model formula or node expression used to calculate the metric.
    weight : float
        Non-negative contribution weight for the composite rating; defaults to
        ``1.0`` before normalization across usable metrics.
    thresholds : Mapping[str, tuple[float, float]] or str or None
        Mapping (or JSON object string) that defines rating thresholds for the
        calculated metric; ``None`` means no thresholds.
    description : str or None
        Optional reader-facing explanation of the metric and its credit meaning.

    Examples
    --------
    >>> from finstack_quant.statements_analytics import ScorecardMetric
    >>> metric = ScorecardMetric("leverage", "total_debt / ebitda", weight=0.5)
    >>> (metric.name, metric.weight)
    ('leverage', 0.5)

    """

    def __init__(
        self,
        name: str,
        formula: str,
        weight: float = 1.0,
        thresholds: Mapping[str, tuple[float, float]] | str | None = None,
        description: str | None = None,
    ) -> None:
        """
        Define one weighted formula and its rating thresholds.

        Parameters
        ----------
        name : str
            Stable metric label used in reports and validation diagnostics.
        formula : str
            Statement formula or node expression evaluated for the metric.
        weight : float, default 1.0
            Non-negative contribution weight before normalization across usable metrics.
        thresholds : Mapping[str, tuple[float, float]] or str or None, default None
            Mapping (or JSON object string) of rating label to its lower and upper
            threshold pair. ``None`` means no thresholds.
        description : str or None, default None
            Optional reader-facing explanation of the metric's credit meaning.

        Raises
        ------
        ValueError
            If ``thresholds`` is malformed or does not map ratings to numeric ranges.

        """
        ...

    @property
    def name(self) -> str:
        """
        Metric name, used as the key in the report's metric scores.

        Returns
        -------
        str
            Metric name.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...
    @property
    def formula(self) -> str:
        """
        DSL formula evaluated to produce the metric value.

        Returns
        -------
        str
            DSL formula for the metric.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...
    @property
    def weight(self) -> float:
        """
        Weight of this metric in the overall score.

        Weights are relative and need not sum to 1; the report divides the included
        weight by the configured weight to report ``weight_coverage``.

        Returns
        -------
        float
            Relative weight of the metric in the overall score.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...
    @property
    def description(self) -> str | None:
        """
        Optional human-readable description of the metric.

        Returns
        -------
        str or None
            Metric description, or ``None`` when unset.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...
    @property
    def thresholds(self) -> dict[str, tuple[float, float]]:
        """
        Return a copy of the rating thresholds ordered by rating label.

        This property does not raise. Mutating the returned dictionary does
        not change the metric's stored thresholds.

        Returns
        -------
        dict[str, tuple[float, float]]
            Rating labels mapped to lower and upper bounds in the units of
            the metric formula; an empty dictionary when no bands are configured.
        """
        ...
    def thresholds_json(self) -> str:
        """
        Serialize rating-label thresholds to JSON.

        Returns
        -------
        str
            JSON object mapping rating labels to two-element lower and upper
            threshold arrays.

        Raises
        ------
        ValueError
            If the thresholds cannot be serialized to JSON.
        """
        ...
    def to_json(self) -> str:
        """
        Serialize this object to canonical JSON.

        Returns
        -------
        str
            Canonical JSON representation of this `ScorecardMetric`, suitable for a matching `from_json` call.

        Raises
        ------
        ValueError
            If the value cannot be serialized to JSON.
        """
        ...
    @staticmethod
    def from_json(json: str) -> ScorecardMetric:
        """
        Deserialize one scorecard metric from canonical JSON.

        Parameters
        ----------
        json : str
            JSON payload containing metric formula, weight, and thresholds.

        Returns
        -------
        ScorecardMetric
            Validated `ScorecardMetric` instance reconstructed from the canonical JSON payload.

        Raises
        ------
        ValueError
            If ``json`` is malformed or violates this type's serialized schema.

        Examples
        --------
        >>> from finstack_quant.statements_analytics import ScorecardMetric
        >>> metric = ScorecardMetric("leverage", "total_debt / ebitda", weight=0.5)
        >>> ScorecardMetric.from_json(metric.to_json()).formula
        'total_debt / ebitda'

        """
        ...

class ScorecardConfig:
    """
    Configuration for credit scorecard analysis.

    ``period`` optionally pins the rated period (e.g. ``"2025Q4"``); when
    ``None`` the scorecard rates the last actual period in the model if any
    exists, otherwise the last model period.

    Parameters
    ----------
    rating_scale : str
        Rating-scale identifier used to interpret metric thresholds; defaults
        to the canonical ``"sp"`` scale.
    metrics : list[ScorecardMetric]
        Weighted metric definitions used to calculate the composite rating.
    min_rating : str or None
        Optional minimum acceptable rating used by downstream validation.
    period : str or None
        Optional model period to rate; ``None`` chooses the latest available
        actual or model period.

    Examples
    --------
    >>> from finstack_quant.statements_analytics import ScorecardConfig
    >>> config = ScorecardConfig(period="2025Q1")
    >>> (config.rating_scale, config.period, config.metrics)
    ('sp', '2025Q1', [])

    """

    def __init__(
        self,
        rating_scale: str | None = None,
        metrics: list[ScorecardMetric] = ...,
        min_rating: str | None = None,
        period: str | None = None,
    ) -> None:
        """
        Configure rating scale, weighted metrics, and rated period selection.

        Parameters
        ----------
        rating_scale : str | None, default None
            Canonical id (``"sp"`` or ``"moodys"``) used to interpret thresholds.
            None uses the Rust default, ``"sp"``.
        metrics : list[ScorecardMetric]
            Weighted metric definitions included in the composite rating.
        min_rating : str or None, default None
            Optional minimum acceptable rating for downstream validation.
        period : str or None, default None
            Model period to rate; ``None`` selects the latest actual period,
            falling back to the last model period.

        Notes
        -----
        Construction does not raise; arguments are stored as supplied.
        """
        ...

    @property
    def rating_scale(self) -> str:
        """
        Configured rating scale identifier (for example ``"sp"`` or ``"moodys"``).

        Returns
        -------
        str
            Rating scale identifier.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...
    @property
    def min_rating(self) -> str | None:
        """
        Minimum acceptable rating on ``rating_scale``, or ``None`` when the scorecard
        imposes no floor.

        Returns
        -------
        str or None
            Minimum acceptable rating, or ``None``.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...
    @property
    def period(self) -> str | None:
        """
        Period to rate, as a period-id string (e.g. ``"2025Q4"``).

        ``None`` means the last actual period in the model if any exists, otherwise the
        last model period.

        Returns
        -------
        str or None
            Period-id string to rate, or ``None`` for the default period.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...
    @property
    def metrics(self) -> list[ScorecardMetric]:
        """
        Metric definitions evaluated by the scorecard, in configured order.

        Returns
        -------
        list[ScorecardMetric]
            Metric definitions in configured order.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...
    def validate(self) -> None:
        """
        Validate this object's invariants without executing a report.

        Raises
        ------
        ValueError
            If required fields are missing, out of range, or internally inconsistent.
        """
        ...
    def to_json(self) -> str:
        """
        Serialize this object to canonical JSON.

        Returns
        -------
        str
            Canonical JSON representation of this `ScorecardConfig`, suitable for a matching `from_json` call.

        Raises
        ------
        ValueError
            If the value cannot be serialized to JSON.
        """
        ...
    @staticmethod
    def from_json(json: str) -> ScorecardConfig:
        """
        Deserialize a credit-scorecard configuration from canonical JSON.

        Parameters
        ----------
        json : str
            JSON payload containing scale, metrics, period selection, and
            optional minimum-rating policy.

        Returns
        -------
        ScorecardConfig
            Validated `ScorecardConfig` instance reconstructed from the canonical JSON payload.

        Raises
        ------
        ValueError
            If ``json`` is malformed or violates this type's serialized schema.

        Examples
        --------
        >>> from finstack_quant.statements_analytics import ScorecardConfig
        >>> config = ScorecardConfig(period="2025Q1")
        >>> ScorecardConfig.from_json(config.to_json()).period
        '2025Q1'

        """
        ...

class ScorecardReport:
    """
    Report produced by ``CreditScorecardExtension.execute``.

    ``data_json()`` includes the rated ``period``, the ``partial`` flag, and
    ``weight_coverage`` alongside the per-metric scores and rating.

    With no usable positive-weight evidence, status is ``"failed"`` and the
    ``rating`` and ``total_score`` data entries are ``None``; ``partial`` is true.

    Examples
    --------
    >>> from finstack_quant.statements_analytics import ScorecardReport
    >>> report = ScorecardReport.from_json('{"status":"success","message":"Complete"}')
    >>> (report.status, report.message, report.errors)
    ('success', 'Complete', [])

    """

    @property
    def status(self) -> str:
        """
        Overall scorecard status after metric evaluation.

        Returns
        -------
        str
            Overall scorecard status after metric evaluation.

        Raises
        ------
        RuntimeError
            If the stored enum cannot be rendered as its canonical Rust wire label.
        """
        ...
    @property
    def message(self) -> str:
        """
        Human-readable summary of the run.

        Returns
        -------
        str
            Summary message for the run.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...
    @property
    def warnings(self) -> list[str]:
        """
        Non-fatal warnings raised while scoring (e.g. an excluded metric).

        Returns
        -------
        list[str]
            Non-fatal warnings raised while scoring.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...
    @property
    def errors(self) -> list[str]:
        """
        Per-metric failures. A non-empty list means ``status`` is ``"failed"``.

        Returns
        -------
        list[str]
            Per-metric failure messages.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...
    @property
    def data(self) -> dict[str, Any]:
        """
        Return the structured report payload as a Python dictionary.

        The dictionary is reconstructed from canonical JSON, so mutations to
        it do not modify the report. An absent payload is an empty dictionary.

        Returns
        -------
        dict[str, Any]
            A copy of the scorecard payload with the rated ``period``, metric
            scores, rating, ``partial`` flag and ``weight_coverage``. Missing
            payload fields remain absent; JSON null values become ``None``.

        Raises
        ------
        ValueError
            If the payload cannot be converted through canonical JSON.
        """
        ...
    def data_json(self) -> str:
        """
        Serialize report payload fields to JSON.

        Returns
        -------
        str
            JSON serialization of the structured scorecard payload, including
            the rated period, metric scores, rating, partial flag, and weight coverage.

        Raises
        ------
        ValueError
            If the payload cannot be serialized to JSON.
        """
        ...
    def to_json(self) -> str:
        """
        Serialize this object to canonical JSON.

        Returns
        -------
        str
            Canonical JSON representation of this `ScorecardReport`, suitable for a matching `from_json` call.

        Raises
        ------
        ValueError
            If the value cannot be serialized to JSON.
        """
        ...
    @staticmethod
    def from_json(json: str) -> ScorecardReport:
        """
        Deserialize a credit-scorecard report from canonical JSON.

        Parameters
        ----------
        json : str
            JSON payload returned by a scorecard extension execution.

        Returns
        -------
        ScorecardReport
            Validated `ScorecardReport` instance reconstructed from the canonical JSON payload.

        Raises
        ------
        ValueError
            If ``json`` is malformed or violates this type's serialized schema.

        Examples
        --------
        >>> from finstack_quant.statements_analytics import ScorecardReport
        >>> ScorecardReport.from_json('{"status":"success","message":"Complete"}').status
        'success'

        """
        ...

    def to_dataframe(self) -> pd.DataFrame:
        """
        Export the report header as a single-row pandas ``DataFrame``.

        Columns: ``status``, ``message``, ``rating``, ``rating_scale``,
        ``period``, ``total_score``, ``partial``, ``weight_coverage``,
        ``warning_count``, ``error_count``.

        ``period`` is the rated period-id string. ``weight_coverage`` is a
        decimal fraction in ``[0, 1]``: the included metric weight over the
        configured metric weight, so ``0.8`` means a fifth of the configured
        weight was excluded. ``partial`` is ``True`` when any metric was
        excluded or errored. Fields absent from the report payload are
        ``None``. Per-metric detail lives in ``to_metric_scores_dataframe``.

        Returns
        -------
        pd.DataFrame
            One row describing the scorecard run.

        Raises
        ------
        ValueError
            If the result cannot be serialized into a pandas object.
        RuntimeError
            If the report status cannot be rendered as its canonical Rust wire label.
        """
        ...

    def to_metric_scores_dataframe(self) -> pd.DataFrame:
        """
        Export the per-metric scores as a pandas ``DataFrame``.

        Columns: ``metric``, ``value``, ``score``, ``weight``,
        ``weighted_score``. One row per scored metric, in configured order; a
        report with no scored metrics still carries the full column schema.

        ``value`` is the metric's evaluated value in its own units, ``score``
        its mapped rating score, ``weight`` the configured weight, and
        ``weighted_score`` is ``score * weight``. Metrics that errored or were
        excluded do not appear here - see ``errors`` and ``weight_coverage``.

        Returns
        -------
        pd.DataFrame
            One row per scored metric.

        Notes
        -----
        This accessor does not raise; it returns the stored or derived value.
        """
        ...

class CreditScorecardExtension:
    """
    Credit scorecard extension for rating assignment and stress testing.

    Examples
    --------
    >>> from finstack_quant.statements_analytics import ScorecardConfig, CreditScorecardExtension
    >>> CreditScorecardExtension(ScorecardConfig()).config().rating_scale
    'sp'

    """

    def __init__(self, config: ScorecardConfig) -> None:
        """
        Create an extension driven by ``config``.

        Parameters
        ----------
        config : ScorecardConfig
            Rating scale, weighted metrics, and period-selection policy to use.

        Returns
        -------
        None

        Notes
        -----
        Construction does not raise; arguments are stored as supplied.
        """
        ...
    def config(self) -> ScorecardConfig:
        """
        Return the configuration this extension runs with.

        Returns
        -------
        ScorecardConfig
            The configuration supplied at construction.

        Notes
        -----
        This method does not raise.
        """
        ...
    def execute(self, model: FinancialModelSpec | str, results: StatementResult | str) -> ScorecardReport:
        """
        Calculate a credit scorecard against evaluated statement results.

        Parameters
        ----------
        model : FinancialModelSpec or str
            Model specification object or equivalent JSON used to resolve nodes.
        results : StatementResult or str
            Evaluated statement result object or equivalent JSON to rate.

        Returns
        -------
        ScorecardReport
            Scorecard status, diagnostics, rating, and per-metric results for
            the supplied model evaluation.

        Raises
        ------
        ValueError
            If the scorecard configuration, model, or statement results are invalid.

        """
        ...

    ...
