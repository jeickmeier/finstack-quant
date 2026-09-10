"""
Statement analysis: sensitivity, variance, scenarios, backtesting, goal seek, DCF, corporate, reports, introspection.

Examples
--------
>>> from finstack_quant.statements_analytics import backtest_forecast
>>> backtest_forecast([1.0, 2.0], [1.0, 2.5]).n
2

"""

from __future__ import annotations

import datetime
from collections.abc import Mapping
from typing import Any, ClassVar

import pandas as pd

from finstack_quant.statements import CheckReport, FinancialModelSpec, StatementResult
from finstack_quant.core.market_data import MarketContext
from finstack_quant.core.money import Money
from finstack_quant.core.table import ArrowTable

__all__ = [
    "AccountType",
    "BridgeChart",
    "BridgeStep",
    "CompanyMetrics",
    "CorkscrewAccount",
    "CorkscrewConfig",
    "CorkscrewExtension",
    "CorkscrewReport",
    "CorporateAnalysis",
    "CorporateValuationResult",
    "CreditAssessment",
    "CreditAssessmentPoint",
    "CreditMapping",
    "CreditScorecardExtension",
    "DcfSensitivityResult",
    "DependencyTracer",
    "DimensionScore",
    "EclBucket",
    "EclResult",
    "EquityBridge",
    "Explanation",
    "ExplanationStep",
    "Exposure",
    "ForecastMetrics",
    "FreeRentWindowSpec",
    "GoalSeekResult",
    "LboCheckMappings",
    "LboResult",
    "LeaseGrowthConvention",
    "LeaseSpec",
    "ManagementFeeBase",
    "ManagementFeeSpec",
    "PLSummaryReport",
    "ParameterSpec",
    "PeerFilter",
    "PeerSet",
    "PeerStats",
    "PropertyTemplateNodes",
    "QualitativeFlags",
    "RegressionResult",
    "RelativeValueResult",
    "RenewalSpec",
    "RentRollOutputNodes",
    "RentStepSpec",
    "ScenarioDiff",
    "ScenarioResults",
    "ScenarioSet",
    "ScorecardConfig",
    "ScorecardMetric",
    "ScorecardReport",
    "ScoringDimension",
    "SensitivityConfig",
    "SensitivityResult",
    "Stage",
    "StageResult",
    "StagingConfig",
    "TerminalValueSpec",
    "ThreeStatementMapping",
    "TornadoEntry",
    "ValuationDiscounts",
    "VarianceConfig",
    "VarianceReport",
    "VarianceRow",
    "WeightedEclResult",
    "add_ncf_buildup",
    "add_noi_buildup",
    "add_property_operating_statement",
    "add_rent_roll",
    "add_roll_forward",
    "add_roll_forward_with_opening",
    "add_vintage_buildup",
    "backtest_forecast",
    "classify_stage",
    "compute_ecl",
    "compute_ecl_weighted",
    "compute_multiple",
    "credit_assessment",
    "credit_assessment_report_text",
    "dcf_sensitivity",
    "evaluate_dcf",
    "evaluate_lbo",
    "evaluate_scenario_set",
    "explain_formula",
    "explain_formula_text",
    "generate_tornado_entries",
    "goal_seek",
    "peer_stats",
    "percentile_rank",
    "pl_summary_report",
    "pl_summary_report_text",
    "regression_fair_value",
    "render_check_report_html",
    "render_check_report_text",
    "run_checks",
    "run_corporate_analysis",
    "run_credit_underwriting_checks",
    "run_sensitivity",
    "run_three_statement_checks",
    "run_variance",
    "scenario_diff",
    "score_relative_value",
    "validate_scorecard_config",
    "variance_bridge",
    "wacc",
    "z_score",
]

class DependencyTracer:
    """
    Reusable dependency tracer for a financial model.

    Parameters
    ----------
    model : FinancialModelSpec or str
        ``FinancialModelSpec`` object or JSON string.

    Examples
    --------
    >>> from finstack_quant.statements import ModelBuilder
    >>> from finstack_quant.statements_analytics import DependencyTracer
    >>> builder = ModelBuilder("demo")
    >>> _ = builder.periods("2025Q1..Q1")
    >>> _ = builder.value("revenue", [("2025Q1", 100.0)])
    >>> _ = builder.value("cost", [("2025Q1", 60.0)])
    >>> _ = builder.compute("profit", "revenue - cost")
    >>> sorted(DependencyTracer(builder.build()).direct_dependencies("profit"))
    ['cost', 'revenue']

    """

    def __init__(self, model: FinancialModelSpec | str) -> None:
        """
        Create a dependency tracer for the given model.

        Parameters
        ----------
        model : FinancialModelSpec or str
            ``FinancialModelSpec`` object or JSON string.

        Raises
        ------
        ValueError
            If model JSON is malformed or its dependency graph is invalid.

        """
        ...
    def dependency_tree(self, node_id: str) -> str:
        """
        Return an ASCII dependency tree for ``node_id``.

        Parameters
        ----------
        node_id : str
            Root node to trace.

        Returns
        -------
        str
            Multi-line ASCII tree rooted at ``node_id`` and containing its
            complete upstream dependency hierarchy.

        Raises
        ------
        ValueError
            If node_id is unknown or its dependency graph is invalid.

        """
        ...

    def dependency_tree_detailed(self, results: StatementResult | str, node_id: str, period: str) -> str:
        """
        Return an ASCII dependency tree annotated with values for one period.

        Parameters
        ----------
        results : StatementResult or str
            Statement results to annotate with.
        node_id : str
            Root node to trace.
        period : str
            Period to annotate.

        Returns
        -------
        str
            Multi-line ASCII dependency tree whose nodes are annotated with
            values from ``results`` for ``period``.

        Raises
        ------
        ValueError
            If results JSON or period is invalid, or node_id cannot be traced.

        """
        ...

    def direct_dependencies(self, node_id: str) -> list[str]:
        """
        List immediate dependencies of ``node_id``.

        Parameters
        ----------
        node_id : str
            Statement node ID whose directly referenced inputs are requested.

        Returns
        -------
        list[str]
            Statement node IDs referenced directly by ``node_id``.

        Raises
        ------
        ValueError
            If node_id is unknown or its dependency graph is invalid.

        """
        ...
    def all_dependencies(self, node_id: str) -> list[str]:
        """
        List all transitive dependencies of ``node_id``.

        Parameters
        ----------
        node_id : str
            Statement node ID whose complete upstream dependency set is requested.

        Returns
        -------
        list[str]
            Direct and transitive upstream node IDs in dependency order, with
            dependencies preceding their dependents.

        Raises
        ------
        ValueError
            If node_id is unknown or the dependency traversal fails.

        """
        ...
    def dependents(self, node_id: str) -> list[str]:
        """
        List nodes that depend on ``node_id``.

        Parameters
        ----------
        node_id : str
            Statement node ID whose downstream dependents are requested.

        Returns
        -------
        list[str]
            Statement node IDs that directly depend on ``node_id``.

        Raises
        ------
        ValueError
            If node_id is unknown or its dependency graph is invalid.

        """
        ...

# Comparable-company analysis

# Credit scorecard extension

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
        to the ``"S&P"`` scale.
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
    ('S&P', '2025Q1', [])

    """

    def __init__(
        self,
        rating_scale: str = "S&P",
        metrics: list[ScorecardMetric] = ...,
        min_rating: str | None = None,
        period: str | None = None,
    ) -> None:
        """
        Configure rating scale, weighted metrics, and rated period selection.

        Parameters
        ----------
        rating_scale : str, default "S&P"
            Registered rating-scale identifier used to interpret thresholds.
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
        Rating scale identifier (e.g. ``"S&P"``, ``"Moody's"``, ``"Fitch"``).

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

        Notes
        -----
        This accessor does not raise; it returns the stored value.
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
    'S&P'

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

def validate_scorecard_config(config: ScorecardConfig) -> None:
    """
    Validate a scorecard configuration without executing it.

    Parameters
    ----------
    config : ScorecardConfig
        Rating scale, metrics, thresholds, and period policy to validate.

    Raises
    ------
    ValueError
        If the scorecard configuration is internally inconsistent.

    Examples
    --------
    >>> from finstack_quant.statements_analytics import ScorecardConfig, validate_scorecard_config
    >>> validate_scorecard_config(ScorecardConfig())

    """
    ...

# Corkscrew (balance-sheet roll-forward) extension

class AccountType:
    """
    Balance-sheet account classifier: asset / liability / equity.

    Examples
    --------
    >>> from finstack_quant.statements_analytics import AccountType
    >>> AccountType.Asset.value()
    'asset'

    """

    Asset: AccountType
    Liability: AccountType
    Equity: AccountType

    @staticmethod
    def from_str(value: str) -> AccountType:
        """
        Parse a balance-sheet account classification.

        Parameters
        ----------
        value : str
            Case-insensitive ``"asset"``, ``"liability"``, or ``"equity"`` value.

        Returns
        -------
        AccountType
            Enum member corresponding to the exact snake-case account type.

        Raises
        ------
        ValueError
            If value is not asset, liability, or equity.

        Examples
        --------
        >>> from finstack_quant.statements_analytics import AccountType
        >>> AccountType.from_str("liability").value()
        'liability'

        """
        ...
    def value(self) -> str:
        """
        Return the canonical snake-case identifier for this variant.

        Returns
        -------
        str
            Exact JSON identifier: ``"asset"``, ``"liability"``, or ``"equity"``.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

class CorkscrewAccount:
    """
    Map one balance-sheet account to its corkscrew input nodes.

    Identity: ``expected = prev + Σ changes − Σ decreases`` (or
    ``beginning + Σ changes − Σ decreases`` when
    ``beginning_balance_node`` is set). Pair a roll-forward template with
    ``changes`` as increases and ``decreases`` as positive disposals.

    Parameters
    ----------
    node_id : str
        Statement node ID receiving the period-end account balance.
    account_type : AccountType
        Asset, liability, or equity classification controlling change signs.
    changes : list[str]
        Statement node IDs added to the opening balance (increases or signed
        net changes).
    decreases : list[str]
        Statement node IDs subtracted from the opening balance (positive
        repayments, outflows, disposals). Default empty.
    beginning_balance_node : str or None
        Optional node ID supplying the opening balance instead of an inferred
        first-period balance.

    Examples
    --------
    >>> from finstack_quant.statements_analytics import AccountType, CorkscrewAccount
    >>> account = CorkscrewAccount("inventory_end", AccountType.Asset, ["purchases"], ["disposals"])
    >>> (account.node_id, account.changes, account.decreases)
    ('inventory_end', ['purchases'], ['disposals'])

    """

    def __init__(
        self,
        node_id: str,
        account_type: AccountType,
        changes: list[str] = ...,
        decreases: list[str] = ...,
        beginning_balance_node: str | None = None,
    ) -> None:
        """
        Map a balance-sheet account to its roll-forward input nodes.

        Parameters
        ----------
        node_id : str
            Statement node receiving the period-end account balance.
        account_type : AccountType
            Asset, liability, or equity classification controlling change signs.
        changes : list[str]
            Statement node IDs added to the opening balance.
        decreases : list[str]
            Statement node IDs subtracted from the opening balance. Default empty.
        beginning_balance_node : str or None, default None
            Optional explicit opening-balance node; ``None`` uses the account's
            prior-period balance.

        Notes
        -----
        Construction does not raise; arguments are stored as supplied.
        """
        ...

    @property
    def node_id(self) -> str:
        """
        Node id of the balance account being rolled forward.

        Returns
        -------
        str
            Node id of the balance account.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...
    @property
    def account_type(self) -> AccountType:
        """
        Balance-sheet classifier: asset, liability, or equity.

        Returns
        -------
        AccountType
            Balance-sheet classifier for the account.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...
    @property
    def changes(self) -> list[str]:
        """
        Node ids of the period increases (or signed net changes) added to the
        balance.

        Identity: ``expected = prev + Σ changes − Σ decreases``. Prefer
        :attr:`decreases` for positive outflows so roll-forward decrease
        nodes do not need to be negated.

        Returns
        -------
        list[str]
            Node ids of the change series added to the balance.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...
    @property
    def decreases(self) -> list[str]:
        """
        Node ids of positive decreases (repayments, outflows, disposals)
        subtracted from the balance.

        Returns
        -------
        list[str]
            Node ids of the decrease series subtracted from the balance.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...
    @property
    def beginning_balance_node(self) -> str | None:
        """
        Node id overriding the beginning balance, or ``None`` to use the account's own
        prior-period closing balance.

        Returns
        -------
        str or None
            Beginning-balance override node id, or ``None``.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...
    def to_json(self) -> str:
        """
        Serialize this object to canonical JSON.

        Returns
        -------
        str
            Canonical JSON representation of this `CorkscrewAccount`, suitable for a matching `from_json` call.

        Raises
        ------
        ValueError
            If the value cannot be serialized to JSON.
        """
        ...
    @staticmethod
    def from_json(json: str) -> CorkscrewAccount:
        """
        Deserialize an account mapping from canonical JSON.

        Parameters
        ----------
        json : str
            JSON payload identifying the balance node, type, and change nodes.

        Returns
        -------
        CorkscrewAccount
            Validated `CorkscrewAccount` instance reconstructed from the canonical JSON payload.

        Raises
        ------
        ValueError
            If ``json`` is malformed or violates this type's serialized schema.

        Examples
        --------
        >>> from finstack_quant.statements_analytics import AccountType, CorkscrewAccount
        >>> account = CorkscrewAccount("cash", AccountType.Asset, ["cash_change"])
        >>> CorkscrewAccount.from_json(account.to_json()).account_type.value()
        'asset'

        """
        ...

class CorkscrewConfig:
    """
    Configure corkscrew roll-forward validation across balance accounts.

    Parameters
    ----------
    accounts : list[CorkscrewAccount]
        Account mappings to reconcile; defaults to an empty validation set.
    tolerance : float
        Absolute currency-unit tolerance allowed for reconciliation differences;
        defaults to ``0.01``.
    fail_on_error : bool
        Whether reconciliation errors abort execution instead of being reported.

    Examples
    --------
    >>> from finstack_quant.statements_analytics import CorkscrewConfig
    >>> config = CorkscrewConfig(tolerance=0.05)
    >>> (config.accounts, config.tolerance, config.fail_on_error)
    ([], 0.05, False)

    """

    def __init__(
        self,
        accounts: list[CorkscrewAccount] = ...,
        tolerance: float = 0.01,
        fail_on_error: bool = False,
    ) -> None:
        """
        Configure account roll-forwards and reconciliation failure policy.

        Parameters
        ----------
        accounts : list[CorkscrewAccount]
            Balance-sheet accounts and their change-node mappings.
        tolerance : float, default 0.01
            Maximum absolute reconciliation difference in model currency units.
        fail_on_error : bool, default False
            Whether any failed account reconciliation makes execution fail.

        Notes
        -----
        Construction does not raise; arguments are stored as supplied.
        """
        ...

    @property
    def accounts(self) -> list[CorkscrewAccount]:
        """
        Balance accounts validated by this configuration, in configured order.

        Returns
        -------
        list[CorkscrewAccount]
            Validated balance accounts in configured order.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...
    @property
    def tolerance(self) -> float:
        """
        Absolute roll-forward tolerance, in the balance node's own units.

        A period is flagged when
        ``abs(closing - (opening + sum(changes) - sum(decreases))) >
        tolerance``.

        Returns
        -------
        float
            Absolute roll-forward tolerance in the balance node's units.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...
    @property
    def fail_on_error(self) -> bool:
        """
        When ``True``, any roll-forward violation is fatal (reported as an error) rather
        than a warning.

        Returns
        -------
        bool
            Whether roll-forward violations are treated as fatal.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...
    def to_json(self) -> str:
        """
        Serialize this object to canonical JSON.

        Returns
        -------
        str
            Canonical JSON representation of this `CorkscrewConfig`, suitable for a matching `from_json` call.

        Raises
        ------
        ValueError
            If the value cannot be serialized to JSON.
        """
        ...
    @staticmethod
    def from_json(json: str) -> CorkscrewConfig:
        """
        Deserialize corkscrew validation settings from canonical JSON.

        Parameters
        ----------
        json : str
            JSON payload containing account mappings and reconciliation policy.

        Returns
        -------
        CorkscrewConfig
            Validated `CorkscrewConfig` instance reconstructed from the canonical JSON payload.

        Raises
        ------
        ValueError
            If ``json`` is malformed or violates this type's serialized schema.

        Examples
        --------
        >>> from finstack_quant.statements_analytics import CorkscrewConfig
        >>> config = CorkscrewConfig(tolerance=0.05)
        >>> CorkscrewConfig.from_json(config.to_json()).tolerance
        0.05

        """
        ...

class CorkscrewReport:
    """
    Report produced by ``CorkscrewExtension.execute``.

    Examples
    --------
    >>> from finstack_quant.statements_analytics import CorkscrewReport
    >>> report = CorkscrewReport.from_json('{"status":"success","message":"Balanced"}')
    >>> (report.status, report.message, report.warnings)
    ('success', 'Balanced', [])

    """

    @property
    def status(self) -> str:
        """
        Corkscrew run status: ``success`` when the walk completed, else ``failed``.

        Returns
        -------
        str
            Overall execution status.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...
    @property
    def message(self) -> str:
        """
        Human-readable summary of the validation run.

        Returns
        -------
        str
            Summary message for the validation run.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...
    @property
    def warnings(self) -> list[str]:
        """
        Non-fatal warnings, including roll-forward breaks reported when
        ``fail_on_error`` is ``False``.

        Returns
        -------
        list[str]
            Non-fatal warnings raised during validation.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...
    @property
    def errors(self) -> list[str]:
        """
        Roll-forward violations treated as fatal (``fail_on_error=True``) plus any
        structural failure. A non-empty list means ``status`` is ``"failed"``.

        Returns
        -------
        list[str]
            Fatal validation failures.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...
    def data_json(self) -> str:
        """
        Serialize report payload fields to JSON.

        Returns
        -------
        str
            JSON serialization of the structured reconciliation payload,
            including account-level balance checks and differences.

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
            Canonical JSON representation of this `CorkscrewReport`, suitable for a matching `from_json` call.

        Raises
        ------
        ValueError
            If the value cannot be serialized to JSON.
        """
        ...
    @staticmethod
    def from_json(json: str) -> CorkscrewReport:
        """
        Deserialize a corkscrew validation report from canonical JSON.

        Parameters
        ----------
        json : str
            JSON payload returned by a corkscrew extension execution.

        Returns
        -------
        CorkscrewReport
            Validated `CorkscrewReport` instance reconstructed from the canonical JSON payload.

        Raises
        ------
        ValueError
            If ``json`` is malformed or violates this type's serialized schema.

        Examples
        --------
        >>> from finstack_quant.statements_analytics import CorkscrewReport
        >>> CorkscrewReport.from_json('{"status":"success","message":"Balanced"}').status
        'success'

        """
        ...

    def to_dataframe(self) -> pd.DataFrame:
        """
        Export the report header as a single-row pandas ``DataFrame``.

        Columns: ``status``, ``message``, ``account_count``,
        ``warning_count``, ``error_count``.

        ``account_count`` is the number of validated accounts. Per-account
        detail lives in ``to_validations_dataframe``.

        Returns
        -------
        pd.DataFrame
            One row describing the validation run.

        Raises
        ------
        ValueError
            If the result cannot be serialized into a pandas object.
        """
        ...

    def to_validations_dataframe(self) -> pd.DataFrame:
        """
        Export the per-account roll-forward validations as a pandas
        ``DataFrame``.

        Columns: ``account``, ``type``, ``periods_validated``, ``max_error``,
        ``is_valid``. One row per validated account, in configured order; a
        report with no validations still carries the full column schema.

        ``type`` is the account classifier (``"asset"``, ``"liability"``,
        ``"equity"``), ``periods_validated`` is a count of model periods, and
        ``max_error`` is the largest absolute roll-forward break across those
        periods, in the balance node's own units. ``is_valid`` is ``False``
        when ``max_error`` exceeded the configured tolerance.

        Returns
        -------
        pd.DataFrame
            One row per validated account.

        Notes
        -----
        This accessor does not raise; it returns the stored or derived value.
        """
        ...

class CorkscrewExtension:
    """
    Corkscrew extension for balance-sheet roll-forward validation.

    Examples
    --------
    >>> from finstack_quant.statements_analytics import CorkscrewConfig, CorkscrewExtension
    >>> CorkscrewExtension(CorkscrewConfig()).config().tolerance
    0.01

    """

    def __init__(self, config: CorkscrewConfig) -> None:
        """
        Create an extension driven by ``config``.

        Parameters
        ----------
        config : CorkscrewConfig
            Accounts, tolerance, and error policy used during reconciliation.

        Returns
        -------
        None

        Notes
        -----
        Construction does not raise; arguments are stored as supplied.
        """
        ...
    def config(self) -> CorkscrewConfig:
        """
        Return the configuration this extension runs with.

        Returns
        -------
        CorkscrewConfig
            The configuration supplied at construction.

        Notes
        -----
        This method does not raise.
        """
        ...
    def execute(self, model: FinancialModelSpec | str, results: StatementResult | str) -> CorkscrewReport:
        """
        Validate account roll-forwards against evaluated statement results.

        Parameters
        ----------
        model : FinancialModelSpec or str
            Model specification object or JSON used to resolve configured nodes.
        results : StatementResult or str
            Evaluated statement results object or JSON to reconcile.

        Returns
        -------
        CorkscrewReport
            Reconciliation status, diagnostics, and account-level roll-forward
            results for the supplied model evaluation.

        Raises
        ------
        ValueError
            If the corkscrew configuration, model, or statement results are invalid.

        """
        ...

# Vintage template

def add_vintage_buildup(
    model: FinancialModelSpec | str,
    name: str,
    new_volume_node: str,
    decay_curve: list[float],
) -> FinancialModelSpec:
    """
    Apply the vintage (cohort) buildup template to a model spec.

    Returns a typed ``FinancialModelSpec`` with the convolution
    node added.

    Parameters
    ----------
    model : FinancialModelSpec or str
        Model specification object or JSON to augment with the cohort schedule.
    name : str
        Prefix used to name the generated vintage buildup nodes.
    new_volume_node : str
        Existing node ID that supplies new volume for each cohort period.
    decay_curve : list[float]
        Ordered cohort-retention factors by elapsed period, expressed as decimal
        multipliers of original volume.

    Returns
    -------
    FinancialModelSpec
        Typed model specification containing the generated vintage-convolution node.

    Raises
    ------
    ValueError
        If model JSON, a node identifier, or decay_curve is invalid.

    Examples
    --------
    >>> from finstack_quant.statements_analytics import add_vintage_buildup
    >>> from finstack_quant.statements import FinancialModelSpec, ModelBuilder
    >>> builder = ModelBuilder("template")
    >>> _ = builder.periods("2025Q1..Q2")
    >>> _ = builder.value("new_volume", [("2025Q1", 10.0), ("2025Q2", 12.0)])
    >>> model = builder.build()
    >>> updated = add_vintage_buildup(model, "customers", "new_volume", [1.0, 0.8])
    >>> updated.node_count > model.node_count
    True

    """
    ...

# Roll-forward template

def add_roll_forward(
    model: FinancialModelSpec | str,
    name: str,
    increases: list[str],
    decreases: list[str],
) -> FinancialModelSpec:
    """
    Apply the roll-forward template (Beginning + Increases - Decreases = Ending) to a model spec.

    Returns a typed ``FinancialModelSpec`` with ``{name}_beg`` and
    ``{name}_end`` nodes added. The first period opens at zero; use
    ``add_roll_forward_with_opening`` for an explicit opening balance.

    Parameters
    ----------
    model : FinancialModelSpec or str
        Model specification object or JSON to augment with roll-forward nodes.
    name : str
        Prefix used to name the generated beginning and ending balance nodes.
    increases : list[str]
        Existing node IDs whose period values increase the ending balance.
    decreases : list[str]
        Existing node IDs whose period values decrease the ending balance.

    Returns
    -------
    FinancialModelSpec
        Typed model specification containing zero-opening beginning- and ending-balance nodes.

    Raises
    ------
    ValueError
        If model JSON or a roll-forward node identifier is invalid.

    Examples
    --------
    >>> from finstack_quant.statements_analytics import add_roll_forward
    >>> from finstack_quant.statements import FinancialModelSpec, ModelBuilder
    >>> builder = ModelBuilder("template")
    >>> _ = builder.periods("2025Q1..Q2")
    >>> _ = builder.value("adds", [("2025Q1", 10.0), ("2025Q2", 12.0)])
    >>> model = builder.build()
    >>> updated = add_roll_forward(model, "balance", ["adds"], [])
    >>> updated.has_node("balance_end")
    True

    """
    ...

def add_roll_forward_with_opening(
    model: FinancialModelSpec | str,
    name: str,
    increases: list[str],
    decreases: list[str],
    opening: float,
) -> FinancialModelSpec:
    """
    Apply the roll-forward template with an explicit first-period opening balance.

    Same as ``add_roll_forward`` except the first period's beginning balance
    is ``opening`` instead of zero. Returns a typed ``FinancialModelSpec``.

    Parameters
    ----------
    model : FinancialModelSpec or str
        Model specification object or JSON to augment with roll-forward nodes.
    name : str
        Prefix used to name the generated beginning and ending balance nodes.
    increases : list[str]
        Existing node IDs whose period values increase the ending balance.
    decreases : list[str]
        Existing node IDs whose period values decrease the ending balance.
    opening : float
        Beginning balance assigned to the first modeled period in model units.

    Returns
    -------
    FinancialModelSpec
        Typed model specification containing the seeded beginning- and ending-balance nodes.

    Raises
    ------
    ValueError
        If model JSON, opening, or a roll-forward node identifier is invalid.

    Examples
    --------
    >>> from finstack_quant.statements_analytics import add_roll_forward_with_opening
    >>> from finstack_quant.statements import FinancialModelSpec, ModelBuilder
    >>> builder = ModelBuilder("template")
    >>> _ = builder.periods("2025Q1..Q2")
    >>> _ = builder.value("adds", [("2025Q1", 10.0), ("2025Q2", 12.0)])
    >>> model = builder.build()
    >>> updated = add_roll_forward_with_opening(model, "balance", ["adds"], [], 100.0)
    >>> updated.has_node("balance_beg")
    True

    """
    ...

# Real-estate template

class RentStepSpec:
    """
    Reset a lease's base rent from one model period onward.

    Parameters
    ----------
    start : str
        First model period label at which the stepped rent applies.
    rent : float
        Replacement periodic rent in the model's currency units.

    Examples
    --------
    >>> from finstack_quant.statements_analytics import RentStepSpec
    >>> step = RentStepSpec("2026Q1", 125.0)
    >>> (step.start, step.rent)
    ('2026Q1', 125.0)

    """

    def __init__(self, start: str, rent: float) -> None:
        """
        Reset a lease's periodic rent from a specified model period onward.

        Parameters
        ----------
        start : str
            First model-period label at which the replacement rent applies.
        rent : float
            Replacement periodic rent in the model's currency units.

        Raises
        ------
        ValueError
            If start is not a valid model period.

        """
        ...

    @property
    def start(self) -> str:
        """
        Period (inclusive) this rent level takes effect, as a period-id string.

        Returns
        -------
        str
            Effective period as a period-id string.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...
    @property
    def rent(self) -> float:
        """
        Rent for one model period from ``start``, in model currency units.

        This replaces the prevailing rent level rather than adding to it, and restarts
        growth compounding from ``start``.

        Returns
        -------
        float
            Rent per model period from ``start``.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...
    def to_json(self) -> str:
        """
        Serialize this object to canonical JSON.

        Returns
        -------
        str
            Canonical JSON representation of this `RentStepSpec`, suitable for a matching `from_json` call.

        Raises
        ------
        ValueError
            If the value cannot be serialized to JSON.
        """
        ...
    @staticmethod
    def from_json(json: str) -> RentStepSpec:
        """
        Deserialize a rent-step specification from canonical JSON.

        Parameters
        ----------
        json : str
            JSON payload containing the step start period and replacement rent.

        Returns
        -------
        RentStepSpec
            Validated `RentStepSpec` instance reconstructed from the canonical JSON payload.

        Raises
        ------
        ValueError
            If ``json`` is malformed or violates this type's serialized schema.

        Examples
        --------
        >>> from finstack_quant.statements_analytics import RentStepSpec
        >>> step = RentStepSpec("2026Q1", 125.0)
        >>> RentStepSpec.from_json(step.to_json()).rent
        125.0

        """
        ...

class FreeRentWindowSpec:
    """
    Define a finite concession window that sets lease rent to zero.

    Parameters
    ----------
    start : str
        First model period label affected by the free-rent concession.
    periods : int
        Number of consecutive modeled periods with rent set to zero.

    Examples
    --------
    >>> from finstack_quant.statements_analytics import FreeRentWindowSpec
    >>> window = FreeRentWindowSpec("2025Q1", 2)
    >>> (window.start, window.periods)
    ('2025Q1', 2)

    """

    def __init__(self, start: str, periods: int) -> None:
        """
        Define a dated window of consecutive rent-free model periods.

        Parameters
        ----------
        start : str
            First model-period label affected by the concession.
        periods : int
            Number of consecutive modeled periods with rent set to zero.

        Raises
        ------
        ValueError
            If start is not a valid model period.

        """
        ...

    @property
    def start(self) -> str:
        """
        Period (inclusive) free rent starts, as a period-id string.

        Returns
        -------
        str
            First free-rent period as a period-id string.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...
    @property
    def periods(self) -> int:
        """
        Length of the concession as a count of model periods.

        Returns
        -------
        int
            Concession length in model periods.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...
    def to_json(self) -> str:
        """
        Serialize this object to canonical JSON.

        Returns
        -------
        str
            Canonical JSON representation of this `FreeRentWindowSpec`, suitable for a matching `from_json` call.

        Raises
        ------
        ValueError
            If the value cannot be serialized to JSON.
        """
        ...
    @staticmethod
    def from_json(json: str) -> FreeRentWindowSpec:
        """
        Deserialize a free-rent concession window from canonical JSON.

        Parameters
        ----------
        json : str
            JSON payload containing the concession start period and duration.

        Returns
        -------
        FreeRentWindowSpec
            Validated `FreeRentWindowSpec` instance reconstructed from the canonical JSON payload.

        Raises
        ------
        ValueError
            If ``json`` is malformed or violates this type's serialized schema.

        Examples
        --------
        >>> from finstack_quant.statements_analytics import FreeRentWindowSpec
        >>> window = FreeRentWindowSpec("2025Q1", 2)
        >>> FreeRentWindowSpec.from_json(window.to_json()).periods
        2

        """
        ...

class RenewalSpec:
    """
    Model a lease renewal in expected-value terms after the base term.

    Parameters
    ----------
    term_periods : int
        Number of modeled periods in the renewal term when the tenant renews.
    probability : float
        Decimal probability of renewal from zero through one.
    downtime_periods : int
        Vacancy periods between the original lease and renewal; defaults to zero.
    rent_factor : float
        Multiplier applied to the ending scheduled rent for the renewal term;
        defaults to ``1.0``.
    free_rent_periods : int
        Initial renewal periods with rent set to zero; defaults to no concession.

    Examples
    --------
    >>> from finstack_quant.statements_analytics import RenewalSpec
    >>> renewal = RenewalSpec(4, 0.75, downtime_periods=1)
    >>> (renewal.term_periods, renewal.probability, renewal.downtime_periods)
    (4, 0.75, 1)

    """

    def __init__(
        self,
        term_periods: int,
        probability: float,
        downtime_periods: int = 0,
        rent_factor: float = 1.0,
        free_rent_periods: int = 0,
    ) -> None:
        """
        Define expected-value assumptions for a lease renewal term.

        Parameters
        ----------
        term_periods : int
            Number of modeled periods in the renewal term when renewal occurs.
        probability : float
            Decimal renewal probability from zero through one.
        downtime_periods : int, default 0
            Vacancy periods between the original lease and renewal.
        rent_factor : float, default 1.0
            Multiplier applied to ending scheduled rent for the renewal term.
        free_rent_periods : int, default 0
            Initial renewal periods with rent set to zero.

        Notes
        -----
        Construction does not raise; arguments are stored as supplied.
        """
        ...

    @property
    def term_periods(self) -> int:
        """
        Renewal term length as a count of model periods.

        Returns
        -------
        int
            Renewal term length in model periods.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...
    @property
    def probability(self) -> float:
        """
        Probability of renewal as a decimal fraction in ``[0, 1]``.

        Renewal is modelled in expected-value terms, so this weights the renewal rent
        rather than selecting a branch.

        Returns
        -------
        float
            Renewal probability in ``[0, 1]``.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...
    @property
    def downtime_periods(self) -> int:
        """
        Rent-free downtime after the initial term ends, as a count of model periods.

        Returns
        -------
        int
            Downtime length in model periods.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...
    @property
    def rent_factor(self) -> float:
        """
        Multiplier applied to the last contractual rent of the initial term (``1.05``
        means renewal starts 5% above the prior rent level).

        Returns
        -------
        float
            Multiplier on the prior rent level at renewal.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...
    @property
    def free_rent_periods(self) -> int:
        """
        Number of model periods of free rent at renewal start.

        Returns
        -------
        int
            Count of free-rent model periods at renewal start.

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
            Canonical JSON representation of this `RenewalSpec`, suitable for a matching `from_json` call.

        Raises
        ------
        ValueError
            If the value cannot be serialized to JSON.
        """
        ...
    @staticmethod
    def from_json(json: str) -> RenewalSpec:
        """
        Deserialize renewal assumptions from canonical JSON.

        Parameters
        ----------
        json : str
            JSON payload containing renewal term, probability, downtime, and
            rent assumptions.

        Returns
        -------
        RenewalSpec
            Validated `RenewalSpec` instance reconstructed from the canonical JSON payload.

        Raises
        ------
        ValueError
            If ``json`` is malformed or violates this type's serialized schema.

        Examples
        --------
        >>> from finstack_quant.statements_analytics import RenewalSpec
        >>> renewal = RenewalSpec(4, 0.75)
        >>> RenewalSpec.from_json(renewal.to_json()).probability
        0.75

        """
        ...

    def to_dataframe(self) -> pd.DataFrame:
        """
        Export the renewal spec as a single-row pandas ``DataFrame``.

        Columns: ``downtime_periods``, ``term_periods``, ``probability``,
        ``rent_factor``, ``free_rent_periods``.

        The three ``*_periods`` columns are counts of model periods;
        ``probability`` is a decimal fraction in ``[0, 1]`` and
        ``rent_factor`` is a multiplier on the prior rent level.

        Returns
        -------
        pd.DataFrame
            One row describing the renewal terms.

        Raises
        ------
        ValueError
            If the result cannot be serialized into a pandas object.
        """
        ...

class LeaseGrowthConvention:
    """
    Compounding convention for lease rent growth.

    Examples
    --------
    >>> from finstack_quant.statements_analytics import LeaseGrowthConvention
    >>> LeaseGrowthConvention.PerPeriod.value()
    'per_period'

    """

    PerPeriod: LeaseGrowthConvention
    AnnualEscalator: LeaseGrowthConvention

    @staticmethod
    def from_str(value: str) -> LeaseGrowthConvention:
        """
        Parse a lease rent-growth compounding convention.

        Parameters
        ----------
        value : str
            Case-insensitive ``"per_period"`` or ``"annual_escalator"`` value.

        Returns
        -------
        LeaseGrowthConvention
            Enum member corresponding to the exact snake-case growth convention.

        Raises
        ------
        ValueError
            If value is not per_period or annual_escalator.

        Examples
        --------
        >>> from finstack_quant.statements_analytics import LeaseGrowthConvention
        >>> LeaseGrowthConvention.from_str("annual_escalator").value()
        'annual_escalator'

        """
        ...
    def value(self) -> str:
        """
        Return the canonical snake-case identifier for this variant.

        Returns
        -------
        str
            Exact JSON identifier, either ``"per_period"`` or
            ``"annual_escalator"``.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

class LeaseSpec:
    """
    Describe a rich lease schedule for rent-roll generation.

    Parameters
    ----------
    node_id : str
        Statement node ID receiving the lease's rental-revenue series.
    start : str
        First included model period label for the lease term.
    base_rent : float
        Contractual periodic rent before escalators, concessions, and occupancy
        scaling, in the model's currency units.
    end : str or None
        Optional final included model period; ``None`` extends through horizon.
    growth_rate : float
        Decimal rent-growth rate interpreted by ``growth_convention``.
    growth_convention : LeaseGrowthConvention
        Whether rent growth compounds every model period or as an annual
        step. Default ``AnnualEscalator`` (Argus/NCREIF anniversary bump).
        ``PerPeriod`` must be set explicitly.
    rent_steps : list[RentStepSpec]
        Explicit rent resets applied from each step's start period onward.
    free_rent_periods : int
        Number of initial included periods with rent set to zero.
    free_rent_windows : list[FreeRentWindowSpec]
        Additional dated rent-free concession windows within the lease term.
    occupancy : float
        Decimal occupancy multiplier applied to scheduled rent.
    renewal : RenewalSpec or None
        Optional expected-value renewal assumptions applied after the base term.

    Examples
    --------
    >>> from finstack_quant.statements_analytics import LeaseSpec
    >>> lease = LeaseSpec("lease_a", "2025Q1", 100.0, end="2025Q4")
    >>> (lease.node_id, lease.base_rent, lease.growth_convention.value())
    ('lease_a', 100.0, 'annual_escalator')

    """

    def __init__(
        self,
        node_id: str,
        start: str,
        base_rent: float,
        end: str | None = None,
        growth_rate: float = 0.0,
        growth_convention: LeaseGrowthConvention = ...,
        rent_steps: list[RentStepSpec] = ...,
        free_rent_periods: int = 0,
        free_rent_windows: list[FreeRentWindowSpec] = ...,
        occupancy: float = 1.0,
        renewal: RenewalSpec | None = None,
    ) -> None:
        """
        Define a lease schedule with escalators, concessions, and renewal terms.

        Parameters
        ----------
        node_id : str
            Statement node receiving the lease's rental-revenue series.
        start : str
            First included model-period label for the lease term.
        base_rent : float
            Contractual periodic rent before escalators, concessions, and
            occupancy scaling, in the model's currency units.
        end : str or None, default None
            Optional final included model period; ``None`` extends through horizon.
        growth_rate : float, default 0.0
            Decimal rent-growth rate interpreted by ``growth_convention``.
        growth_convention : LeaseGrowthConvention
            Whether growth compounds every model period or as an annual
            step. Default ``AnnualEscalator``; set ``PerPeriod`` explicitly
            for per-period compounding.
        rent_steps : list[RentStepSpec]
            Explicit rent resets applied from each step's start period onward.
        free_rent_periods : int, default 0
            Number of initial included periods with rent set to zero.
        free_rent_windows : list[FreeRentWindowSpec]
            Additional dated rent-free windows within the base lease term.
        occupancy : float, default 1.0
            Decimal occupancy multiplier applied to scheduled rent.
        renewal : RenewalSpec or None, default None
            Optional expected-value renewal assumptions applied after the base term.

        Raises
        ------
        ValueError
            If start or end is not a valid model period.

        """
        ...

    @property
    def node_id(self) -> str:
        """
        Base node id; per-lease detail nodes are derived from it (``{node_id}.pgi``,
        ``.free_rent``, ``.vacancy_loss``, ``.effective_rent``).

        Returns
        -------
        str
            Base node id for the lease's derived detail nodes.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...
    @property
    def start(self) -> str:
        """
        First period (inclusive) the lease is active, as a period-id string (e.g.
        ``"2025Q1"``).

        Returns
        -------
        str
            First active period as a period-id string.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...
    @property
    def end(self) -> str | None:
        """
        Last period (inclusive) of the initial term, or ``None`` to run through the
        model end (which also disables renewal modelling).

        Returns
        -------
        str or None
            Last period of the initial term, or ``None``.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...
    @property
    def base_rent(self) -> float:
        """
        Base rent for one model period at ``start``, in model currency units.

        A quarterly model means rent per quarter, not per year.

        Returns
        -------
        float
            Base rent per model period at ``start``.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...
    @property
    def growth_rate(self) -> float:
        """
        Growth rate applied within a rent segment as a decimal fraction (``0.03`` =
        +3%), compounded per ``growth_convention``.

        Returns
        -------
        float
            Segment growth rate as a decimal fraction.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...
    @property
    def growth_convention(self) -> LeaseGrowthConvention:
        """
        Compounding convention for ``growth_rate``: every model period (``per_period``)
        or once per lease-start anniversary (``annual_escalator``).

        Returns
        -------
        LeaseGrowthConvention
            Compounding convention for ``growth_rate``.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...
    @property
    def free_rent_periods(self) -> int:
        """
        Number of model periods of free rent counted from ``start``, before any
        additional ``free_rent_windows``.

        Returns
        -------
        int
            Count of free-rent model periods from ``start``.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...
    @property
    def occupancy(self) -> float:
        """
        Occupancy factor in ``[0, 1]`` applied to non-free contractual rent.

        Returns
        -------
        float
            Occupancy factor in ``[0, 1]``.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...
    @property
    def renewal(self) -> RenewalSpec | None:
        """
        Renewal modelling applied after ``end``, or ``None`` for no renewal.

        Returns
        -------
        RenewalSpec or None
            Renewal specification, or ``None``.

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
            Canonical JSON representation of this `LeaseSpec`, suitable for a matching `from_json` call.

        Raises
        ------
        ValueError
            If the value cannot be serialized to JSON.
        """
        ...
    @staticmethod
    def from_json(json: str) -> LeaseSpec:
        """
        Deserialize a rich lease schedule from canonical JSON.

        Parameters
        ----------
        json : str
            JSON payload containing term, rent, escalation, concession, and
            renewal assumptions.

        Returns
        -------
        LeaseSpec
            Validated `LeaseSpec` instance reconstructed from the canonical JSON payload.

        Raises
        ------
        ValueError
            If ``json`` is malformed or violates this type's serialized schema.

        Examples
        --------
        >>> from finstack_quant.statements_analytics import LeaseSpec
        >>> lease = LeaseSpec("lease_a", "2025Q1", 100.0)
        >>> LeaseSpec.from_json(lease.to_json()).start
        '2025Q1'

        """
        ...

    def to_dataframe(self) -> pd.DataFrame:
        """
        Export the lease spec as a single-row pandas ``DataFrame``.

        Columns: ``node_id``, ``start``, ``end``, ``base_rent``,
        ``growth_rate``, ``growth_convention``, ``free_rent_periods``,
        ``occupancy``, ``rent_step_count``, ``free_rent_window_count``,
        ``has_renewal``.

        ``start`` and ``end`` are period-id strings, ``base_rent`` is per
        model period, ``growth_rate`` and ``occupancy`` are decimal fractions,
        and ``free_rent_periods`` is a count of model periods. The nested
        collections are summarised as counts here - read ``renewal`` (and its
        own ``to_dataframe``) for the renewal terms.

        Returns
        -------
        pd.DataFrame
            One row describing the lease.

        Raises
        ------
        ValueError
            If the result cannot be serialized into a pandas object.
        """
        ...

class RentRollOutputNodes:
    """
    Name the aggregate model nodes produced by a rent-roll template.

    Parameters
    ----------
    rent_pgi_node : str
        Node ID for potential gross rent before concessions and vacancy.
    free_rent_node : str
        Node ID for rent waived through free-rent concessions.
    vacancy_loss_node : str
        Node ID for the revenue reduction caused by vacancy or occupancy.
    rent_effective_node : str
        Node ID for effective rent after concessions and vacancy adjustments.

    Examples
    --------
    >>> from finstack_quant.statements_analytics import RentRollOutputNodes
    >>> nodes = RentRollOutputNodes()
    >>> (nodes.rent_pgi_node, nodes.rent_effective_node)
    ('rent_pgi', 'rent_effective')

    """

    def __init__(
        self,
        rent_pgi_node: str = "rent_pgi",
        free_rent_node: str = "free_rent",
        vacancy_loss_node: str = "vacancy_loss",
        rent_effective_node: str = "rent_effective",
    ) -> None:
        """
        Name the four aggregate statement nodes produced by a rent roll.

        Parameters
        ----------
        rent_pgi_node : str, default "rent_pgi"
            Node ID for potential gross rent before concessions and vacancy.
        free_rent_node : str, default "free_rent"
            Node ID for rent waived through free-rent concessions.
        vacancy_loss_node : str, default "vacancy_loss"
            Node ID for the revenue reduction caused by vacancy or occupancy.
        rent_effective_node : str, default "rent_effective"
            Node ID for effective rent after concessions and vacancy adjustments.

        Notes
        -----
        Construction does not raise; arguments are stored as supplied.
        """
        ...

    @property
    def rent_pgi_node(self) -> str:
        """
        Node id holding total contractual rent (potential gross income) across all
        leases.

        Returns
        -------
        str
            Node id for total potential gross income.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...
    @property
    def free_rent_node(self) -> str:
        """
        Node id holding total free-rent concessions.

        Returns
        -------
        str
            Node id for total free-rent concessions.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...
    @property
    def vacancy_loss_node(self) -> str:
        """
        Node id holding total vacancy loss, including occupancy and renewal-probability
        effects.

        Returns
        -------
        str
            Node id for total vacancy loss.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...
    @property
    def rent_effective_node(self) -> str:
        """
        Node id holding total effective rent, the EGI rent component ``rent_pgi -
        free_rent - vacancy_loss``.

        Returns
        -------
        str
            Node id for total effective rent.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...
    def to_json(self) -> str:
        """
        Serialize this object to canonical JSON.

        Returns
        -------
        str
            Canonical JSON representation of this `RentRollOutputNodes`, suitable for a matching `from_json` call.

        Raises
        ------
        ValueError
            If the value cannot be serialized to JSON.
        """
        ...
    @staticmethod
    def from_json(json: str) -> RentRollOutputNodes:
        """
        Deserialize rent-roll output-node names from canonical JSON.

        Parameters
        ----------
        json : str
            JSON payload identifying potential, concession, vacancy, and
            effective-rent output nodes.

        Returns
        -------
        RentRollOutputNodes
            Validated `RentRollOutputNodes` instance reconstructed from the canonical JSON payload.

        Raises
        ------
        ValueError
            If ``json`` is malformed or violates this type's serialized schema.

        Examples
        --------
        >>> from finstack_quant.statements_analytics import RentRollOutputNodes
        >>> nodes = RentRollOutputNodes()
        >>> RentRollOutputNodes.from_json(nodes.to_json()).vacancy_loss_node
        'vacancy_loss'

        """
        ...

    def to_dataframe(self) -> pd.DataFrame:
        """
        Export the node-id mapping as a single-row pandas ``DataFrame``.

        Columns: ``rent_pgi_node``, ``free_rent_node``, ``vacancy_loss_node``,
        ``rent_effective_node``. Every value is a statement node id, not a
        numeric amount.

        Returns
        -------
        pd.DataFrame
            One row of rent-roll output node ids.

        Raises
        ------
        ValueError
            If the result cannot be serialized into a pandas object.
        """
        ...

class ManagementFeeBase:
    """
    Basis for management fee calculation.

    Examples
    --------
    >>> from finstack_quant.statements_analytics import ManagementFeeBase
    >>> ManagementFeeBase.Egi.value()
    'egi'

    """

    Egi: ManagementFeeBase
    EffectiveRent: ManagementFeeBase

    @staticmethod
    def from_str(value: str) -> ManagementFeeBase:
        """
        Parse a management-fee calculation basis.

        Parameters
        ----------
        value : str
            Case-insensitive ``"egi"`` or ``"effective_rent"`` basis value.

        Returns
        -------
        ManagementFeeBase
            Enum member corresponding to the exact snake-case fee basis.

        Raises
        ------
        ValueError
            If value is not egi or effective_rent.

        Examples
        --------
        >>> from finstack_quant.statements_analytics import ManagementFeeBase
        >>> ManagementFeeBase.from_str("effective_rent").value()
        'effective_rent'

        """
        ...
    def value(self) -> str:
        """
        Return the canonical snake-case identifier for this variant.

        Returns
        -------
        str
            Exact JSON identifier, either ``"egi"`` or ``"effective_rent"``.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

class ManagementFeeSpec:
    """
    Set a percentage management fee and the revenue base it applies to.

    Parameters
    ----------
    rate : float
        Decimal fee rate, such as ``0.03`` for a 3% management fee.
    base : ManagementFeeBase
        Effective-rent or EGI base used to calculate the fee; defaults to the
        binding's standard basis.

    Examples
    --------
    >>> from finstack_quant.statements_analytics import ManagementFeeBase, ManagementFeeSpec
    >>> fee = ManagementFeeSpec(0.03, ManagementFeeBase.Egi)
    >>> (fee.rate, fee.base.value())
    (0.03, 'egi')

    """

    def __init__(self, rate: float, base: ManagementFeeBase = ...) -> None:
        """
        Define a percentage management fee and its revenue base.

        Parameters
        ----------
        rate : float
            Decimal fee rate, such as ``0.03`` for a 3% management fee.
        base : ManagementFeeBase
            Effective-rent or EGI base used to calculate the fee.

        Notes
        -----
        Construction does not raise; arguments are stored as supplied.
        """
        ...

    @property
    def rate(self) -> float:
        """
        Management fee rate as a decimal fraction (``0.03`` = 3%).

        Returns
        -------
        float
            Management fee rate as a decimal fraction.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...
    @property
    def base(self) -> ManagementFeeBase:
        """
        Basis the fee applies to: ``egi`` (effective gross income) or ``effective_rent``
        (rent only, excluding other income).

        Returns
        -------
        ManagementFeeBase
            Basis the management fee applies to.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...
    def to_json(self) -> str:
        """
        Serialize this object to canonical JSON.

        Returns
        -------
        str
            Canonical JSON representation of this `ManagementFeeSpec`, suitable for a matching `from_json` call.

        Raises
        ------
        ValueError
            If the value cannot be serialized to JSON.
        """
        ...
    @staticmethod
    def from_json(json: str) -> ManagementFeeSpec:
        """
        Deserialize management-fee assumptions from canonical JSON.

        Parameters
        ----------
        json : str
            JSON payload containing the decimal rate and revenue basis.

        Returns
        -------
        ManagementFeeSpec
            Validated `ManagementFeeSpec` instance reconstructed from the canonical JSON payload.

        Raises
        ------
        ValueError
            If ``json`` is malformed or violates this type's serialized schema.

        Examples
        --------
        >>> from finstack_quant.statements_analytics import ManagementFeeSpec
        >>> fee = ManagementFeeSpec(0.03)
        >>> ManagementFeeSpec.from_json(fee.to_json()).rate
        0.03

        """
        ...

class PropertyTemplateNodes:
    """
    Name generated node IDs for a property operating-statement template.

    Parameters
    ----------
    rent_roll : RentRollOutputNodes or None
        Optional rent-roll output names; ``None`` uses the template defaults.
    other_income_total_node : str
        Node ID aggregating other-income components.
    egi_node : str
        Node ID for effective gross income after rent and other income.
    management_fee_node : str
        Node ID for the management-fee expense series.
    opex_total_node : str
        Node ID aggregating operating-expense components.
    noi_node : str
        Node ID for net operating income before capital expenditures.
    capex_total_node : str
        Node ID aggregating capital-expenditure components.
    ncf_node : str
        Node ID for net cash flow after operating items and capital expenditure.

    Examples
    --------
    >>> from finstack_quant.statements_analytics import PropertyTemplateNodes
    >>> nodes = PropertyTemplateNodes()
    >>> (nodes.noi_node, nodes.ncf_node, nodes.rent_roll.rent_effective_node)
    ('noi', 'ncf', 'rent_effective')

    """

    def __init__(
        self,
        rent_roll: RentRollOutputNodes | None = None,
        other_income_total_node: str = "other_income_total",
        egi_node: str = "egi",
        management_fee_node: str = "management_fee",
        opex_total_node: str = "opex_total",
        noi_node: str = "noi",
        capex_total_node: str = "capex_total",
        ncf_node: str = "ncf",
    ) -> None:
        """
        Name the generated statement nodes for a property operating template.

        Parameters
        ----------
        rent_roll : RentRollOutputNodes or None, default None
            Optional rent-roll output names; ``None`` uses the template defaults.
        other_income_total_node : str, default "other_income_total"
            Node ID aggregating other-income components.
        egi_node : str, default "egi"
            Node ID for effective gross income after rent and other income.
        management_fee_node : str, default "management_fee"
            Node ID for the management-fee expense series.
        opex_total_node : str, default "opex_total"
            Node ID aggregating operating-expense components.
        noi_node : str, default "noi"
            Node ID for net operating income before capital expenditures.
        capex_total_node : str, default "capex_total"
            Node ID aggregating capital-expenditure components.
        ncf_node : str, default "ncf"
            Node ID for net cash flow after operating items and capital expenditure.

        Notes
        -----
        Construction does not raise; arguments are stored as supplied.
        """
        ...

    @property
    def rent_roll(self) -> RentRollOutputNodes:
        """
        Rent-roll output node ids (PGI, free rent, vacancy loss, effective rent).

        Returns
        -------
        RentRollOutputNodes
            Rent-roll output node ids.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...
    @property
    def other_income_total_node(self) -> str:
        """
        Node id holding total other (non-rent) income.

        Returns
        -------
        str
            Node id for total other income.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...
    @property
    def egi_node(self) -> str:
        """
        Node id holding effective gross income (EGI).

        Returns
        -------
        str
            Node id for effective gross income.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...
    @property
    def management_fee_node(self) -> str:
        """
        Node id holding the management fee, when one is configured.

        Returns
        -------
        str
            Node id for the management fee.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...
    @property
    def opex_total_node(self) -> str:
        """
        Node id holding total operating expenses, inclusive of the management fee when
        one is configured.

        Returns
        -------
        str
            Node id for total operating expenses.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...
    @property
    def noi_node(self) -> str:
        """
        Node id holding net operating income (NOI).

        Returns
        -------
        str
            Node id for net operating income.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...
    @property
    def capex_total_node(self) -> str:
        """
        Node id holding total capital expenditure.

        Returns
        -------
        str
            Node id for total capital expenditure.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...
    @property
    def ncf_node(self) -> str:
        """
        Node id holding net cash flow, ``noi - capex_total``.

        Returns
        -------
        str
            Node id for net cash flow.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...
    def to_json(self) -> str:
        """
        Serialize this object to canonical JSON.

        Returns
        -------
        str
            Canonical JSON representation of this `PropertyTemplateNodes`, suitable for a matching `from_json` call.

        Raises
        ------
        ValueError
            If the value cannot be serialized to JSON.
        """
        ...
    @staticmethod
    def from_json(json: str) -> PropertyTemplateNodes:
        """
        Deserialize property-template node names from canonical JSON.

        Parameters
        ----------
        json : str
            JSON payload identifying rent, income, expense, NOI, capex, and NCF
            nodes.

        Returns
        -------
        PropertyTemplateNodes
            Validated `PropertyTemplateNodes` instance reconstructed from the canonical JSON payload.

        Raises
        ------
        ValueError
            If ``json`` is malformed or violates this type's serialized schema.

        Examples
        --------
        >>> from finstack_quant.statements_analytics import PropertyTemplateNodes
        >>> nodes = PropertyTemplateNodes()
        >>> PropertyTemplateNodes.from_json(nodes.to_json()).egi_node
        'egi'

        """
        ...

    def to_dataframe(self) -> pd.DataFrame:
        """
        Export the node-id mapping as a single-row pandas ``DataFrame``.

        Columns: ``rent_pgi_node``, ``free_rent_node``, ``vacancy_loss_node``,
        ``rent_effective_node``, ``other_income_total_node``, ``egi_node``,
        ``management_fee_node``, ``opex_total_node``, ``noi_node``,
        ``capex_total_node``, ``ncf_node``.

        The four rent-roll node ids are flattened in rather than nested, so
        every value is a plain statement node id, not a numeric amount.

        Returns
        -------
        pd.DataFrame
            One row of property template node ids.

        Raises
        ------
        ValueError
            If the result cannot be serialized into a pandas object.
        """
        ...

def add_noi_buildup(
    model: FinancialModelSpec | str,
    total_revenue_node: str,
    revenue_nodes: list[str],
    total_expenses_node: str,
    expense_nodes: list[str],
    noi_node: str,
) -> FinancialModelSpec:
    """
    Apply the NOI buildup template and return a typed ``FinancialModelSpec``.

    Parameters
    ----------
    model : FinancialModelSpec or str
        Model specification object or JSON to augment with NOI calculations.
    total_revenue_node : str
        Output node ID that sums the selected revenue nodes.
    revenue_nodes : list[str]
        Existing node IDs included as revenue in the NOI calculation.
    total_expenses_node : str
        Output node ID that sums the selected operating-expense nodes.
    expense_nodes : list[str]
        Existing node IDs included as operating expenses in the NOI calculation.
    noi_node : str
        Output node ID for revenue less operating expenses.

    Returns
    -------
    FinancialModelSpec
        Typed model specification containing revenue, expense, and NOI aggregation nodes.

    Raises
    ------
    ValueError
        If model JSON or a revenue, expense, or NOI node identifier is invalid.

    Examples
    --------
    >>> from finstack_quant.statements_analytics import add_noi_buildup
    >>> from finstack_quant.statements import FinancialModelSpec, ModelBuilder
    >>> builder = ModelBuilder("template")
    >>> _ = builder.periods("2025Q1..Q2")
    >>> _ = builder.value("rent", [("2025Q1", 100.0)])
    >>> _ = builder.value("opex", [("2025Q1", 30.0)])
    >>> model = builder.build()
    >>> updated = add_noi_buildup(model, "revenue", ["rent"], "expenses", ["opex"], "noi")
    >>> updated.has_node("noi")
    True

    """
    ...

def add_ncf_buildup(
    model: FinancialModelSpec | str,
    noi_node: str,
    capex_nodes: list[str],
    ncf_node: str,
) -> FinancialModelSpec:
    """
    Apply the NCF buildup template and return a typed ``FinancialModelSpec``.

    Parameters
    ----------
    model : FinancialModelSpec or str
        Model specification object or JSON to augment with NCF calculations.
    noi_node : str
        Existing node ID supplying net operating income before capital spending.
    capex_nodes : list[str]
        Existing node IDs whose values are deducted as capital expenditures.
    ncf_node : str
        Output node ID for net operating income less capital expenditures.

    Returns
    -------
    FinancialModelSpec
        Typed model specification containing the NCF node after deducting the selected capex nodes.

    Raises
    ------
    ValueError
        If model JSON or an NOI, capital-expenditure, or NCF node identifier is invalid.

    Examples
    --------
    >>> from finstack_quant.statements_analytics import add_ncf_buildup
    >>> from finstack_quant.statements import FinancialModelSpec, ModelBuilder
    >>> builder = ModelBuilder("template")
    >>> _ = builder.periods("2025Q1..Q2")
    >>> _ = builder.value("noi", [("2025Q1", 70.0)])
    >>> _ = builder.value("capex", [("2025Q1", 10.0)])
    >>> model = builder.build()
    >>> add_ncf_buildup(model, "noi", ["capex"], "ncf").has_node("ncf")
    True

    """
    ...

def add_rent_roll(
    model: FinancialModelSpec | str,
    leases: list[LeaseSpec],
    nodes: RentRollOutputNodes | None = None,
) -> FinancialModelSpec:
    """
    Apply the rich rent-roll template and return a typed ``FinancialModelSpec``.

    Parameters
    ----------
    model : FinancialModelSpec or str
        Model specification object or JSON to augment with rental-revenue nodes.
    leases : list[LeaseSpec]
        Rich lease schedules to calculate and aggregate into the rent roll.
    nodes : RentRollOutputNodes or None
        Optional aggregate output-node names; ``None`` uses template defaults.

    Returns
    -------
    FinancialModelSpec
        Typed model specification containing per-lease schedules and aggregate rent-roll nodes.

    Raises
    ------
    ValueError
        If model JSON, a lease specification, or a rent-roll output node is invalid.

    Examples
    --------
    >>> from finstack_quant.statements_analytics import LeaseSpec, add_rent_roll
    >>> from finstack_quant.statements import FinancialModelSpec, ModelBuilder
    >>> builder = ModelBuilder("template")
    >>> _ = builder.periods("2025Q1..Q2")
    >>> model = builder.build()
    >>> lease = LeaseSpec("lease_a", "2025Q1", 100.0)
    >>> add_rent_roll(model, [lease]).has_node("rent_effective")
    True

    """
    ...

def add_property_operating_statement(
    model: FinancialModelSpec | str,
    leases: list[LeaseSpec],
    other_income_nodes: list[str] = ...,
    opex_nodes: list[str] = ...,
    capex_nodes: list[str] = ...,
    management_fee: ManagementFeeSpec | None = None,
    nodes: PropertyTemplateNodes | None = None,
) -> FinancialModelSpec:
    """
    Apply the full property operating-statement template and return a typed model.

    Parameters
    ----------
    model : FinancialModelSpec or str
        Model specification object or JSON to augment with property statements.
    leases : list[LeaseSpec]
        Rich lease schedules used to build rental-revenue and rent-roll outputs.
    other_income_nodes : list[str]
        Existing node IDs aggregated as other income; defaults to an empty list.
    opex_nodes : list[str]
        Existing node IDs aggregated as operating expenses; defaults to empty.
    capex_nodes : list[str]
        Existing node IDs aggregated as capital expenditures; defaults to empty.
    management_fee : ManagementFeeSpec or None
        Optional fee assumptions; ``None`` omits management-fee calculation.
    nodes : PropertyTemplateNodes or None
        Optional generated-node names; ``None`` uses the template defaults.

    Returns
    -------
    FinancialModelSpec
        Typed model specification containing the rent roll, EGI, NOI, capex, and NCF buildup.

    Raises
    ------
    ValueError
        If model JSON, a lease or fee specification, or an operating-statement node is invalid.

    Examples
    --------
    >>> from finstack_quant.statements_analytics import LeaseSpec, add_property_operating_statement
    >>> from finstack_quant.statements import FinancialModelSpec, ModelBuilder
    >>> builder = ModelBuilder("template")
    >>> _ = builder.periods("2025Q1..Q2")
    >>> model = builder.build()
    >>> lease = LeaseSpec("lease_a", "2025Q1", 100.0)
    >>> updated = add_property_operating_statement(model, [lease])
    >>> updated.has_node("ncf")
    True

    """
    ...

class BridgeChart:
    """
    Bridge decomposition of a metric's variance across named drivers.

    Examples
    --------
    >>> from finstack_quant.statements_analytics import BridgeChart
    >>> BridgeChart.from_json("{")
    Traceback (most recent call last):
    ValueError: ...
    """
    @staticmethod
    def from_json(json: str) -> BridgeChart:
        """
        Rebuild a ``BridgeChart`` from its canonical JSON form.

        Parameters
        ----------
        json : str
            JSON document produced by :meth:`to_json`, in this type's
            canonical wire schema.

        Returns
        -------
        BridgeChart
            The value decoded from the supplied JSON document.

        Raises
        ------
        ValueError
            If ``json`` is malformed or is not a valid ``BridgeChart`` document.

        Examples
        --------
        >>> from finstack_quant.statements_analytics import BridgeChart
        >>> BridgeChart.from_json("{")
        Traceback (most recent call last):
        ValueError: ...
        """
    def to_json(self) -> str:
        """
        Serialize to canonical JSON.

        Returns
        -------
        str
            Canonical JSON encoding of this ``BridgeChart``, accepted by
            :meth:`from_json`.

        Raises
        ------
        ValueError
            If the value cannot be serialized to JSON.
        """
    @property
    def target_metric(self) -> str:
        """
                Node identifier of the metric this bridge decomposes (e.g.
        ``"ebitda"``).

                This property does not raise.

                Returns
                -------
                str
                    Node identifier of the metric this bridge decomposes (e.g. ``"ebitda"``).
        """
    @property
    def period(self) -> str:
        """
        Period the bridge covers, as a period-id string (e.g. ``"2025Q1"``).

        This property does not raise.

        Returns
        -------
        str
            Period the bridge covers, as a period-id string (e.g. ``"2025Q1"``).
        """
    @property
    def baseline_label(self) -> str:
        """
        Label for the baseline scenario (e.g. ``"management_case"``).

        This property does not raise.

        Returns
        -------
        str
            Label for the baseline scenario (e.g. ``"management_case"``).
        """
    @property
    def comparison_label(self) -> str:
        """
        Label for the comparison scenario (e.g. ``"bank_case"``).

        This property does not raise.

        Returns
        -------
        str
            Label for the comparison scenario (e.g. ``"bank_case"``).
        """
    @property
    def baseline_value(self) -> float:
        """
        Target-metric value in the baseline scenario, in the metric's units.

        This property does not raise.

        Returns
        -------
        float
            Target-metric value in the baseline scenario, in the metric's units.
        """
    @property
    def comparison_value(self) -> float:
        """
        Target-metric value in the comparison scenario, in the metric's units.

        This property does not raise.

        Returns
        -------
        float
            Target-metric value in the comparison scenario, in the metric's units.
        """
    @property
    def steps(self) -> list[BridgeStep]:
        """
        Ordered driver contributions making up the bridge.

        This property does not raise.

        Returns
        -------
        list[BridgeStep]
            Ordered driver contributions making up the bridge.
        """
    def to_dataframe(self) -> pd.DataFrame:
        """
        Export the driver steps as a pandas ``DataFrame``.

        Columns: ``driver``, ``contribution``. One row per bridge step, in
        decomposition order; an empty bridge still carries both columns.
        Contributions are raw deltas in each driver's own units.

        The scalar header fields (``target_metric``, ``period``,
        ``baseline_label``, ``comparison_label``, ``baseline_value``,
        ``comparison_value``, ``unexplained``) are chart metadata and are not
        repeated on every row.

        Returns
        -------
        pd.DataFrame
            Export the driver steps as a pandas ``DataFrame``. Columns: ``driver``, ``contribution``. One row per bridge step, in decomposition order; an empty bridge still carries both columns. Contributions are raw deltas in each driver's own units. The scalar header fields (``target_metric``, ``period``, ``baseline_label``, ``comparison_label``, ``baseline_value``, ``comparison_value``, ``unexplained``) are chart metadata and are not repeated on every row.

        Raises
        ------
        ImportError
            If pandas is not installed in the running interpreter.
        ValueError
            If the result cannot be converted to its wire form.
        """
    @property
    def unexplained(self) -> float:
        """
                Residual variance not explained by the driver deltas.

        Driver contributions are raw deltas in driver units rather than
        sensitivities of the target metric, so they generally do not sum to
        the target variance. This term makes that gap explicit.

                This property does not raise.

                Returns
                -------
                float
                    Residual variance not explained by the driver deltas. Driver contributions are raw deltas in driver units rather than sensitivities of the target metric, so they generally do not sum to the target variance. This term makes that gap explicit.
        """

class BridgeStep:
    """
    One driver step in a bridge decomposition.

    Examples
    --------
    >>> from finstack_quant.statements_analytics import BridgeStep
    >>> [field for field in ("driver", "contribution") if hasattr(BridgeStep, field)]
    ['driver', 'contribution']
    """
    @property
    def driver(self) -> str:
        """
        Driver node identifier (e.g. ``"revenue"``).

        This property does not raise.

        Returns
        -------
        str
            Driver node identifier (e.g. ``"revenue"``).
        """
    @property
    def contribution(self) -> float:
        """
                This driver's raw delta between the two scenarios, in the *driver's*
        own units.

        Contributions are not sensitivities of the target metric, so they
        generally do not sum to the target variance — see
        ``BridgeChart.unexplained``.

                This property does not raise.

                Returns
                -------
                float
                    This driver's raw delta between the two scenarios, in the *driver's* own units. Contributions are not sensitivities of the target metric, so they generally do not sum to the target variance — see ``BridgeChart.unexplained``.
        """

class CompanyMetrics:
    """
    Metrics for one company in a peer set.

    Monetary values must already be in one currency; ratios are plain scalars
    (``6.5`` = 6.5x) and growth/margin inputs are decimals (``0.05`` = 5%).
    Canonical metric names populate dedicated fields; any other name is kept
    in ``custom``.

    The constructor stores the supplied values and does not raise.

    Parameters
    ----------
    id : str
        Company identifier.
    metrics : dict[str, float | None]
        Flat ``{metric_name: value}`` map. Known names (``enterprise_value``,
        ``market_cap``, ``share_price``, ``oas_bp``, ``yield_pct``, ``ebitda``,
        ``revenue``, ``ebit``, ``ufcf``, ``lfcf``, ``net_income``,
        ``book_value``, ``tangible_book_value``, ``dividends_per_share``,
        ``leverage``, ``interest_coverage``, ``revenue_growth``,
        ``ebitda_margin``) fill their fields; other names go to ``custom``.
        ``None`` values are treated as missing.
    tags : list[str] | None
        Attribute tags used by ``PeerFilter.required_tags`` / ``excluded_tags``.
    meta : dict[str, str] | None
        Attribute metadata (``gics_sector``, ``gics_industry``, ``country``,
        ``rating``) used by ``PeerFilter``.

    Examples
    --------
    >>> from finstack_quant.statements_analytics import CompanyMetrics
    >>> CompanyMetrics("ACME", {"leverage": 3.0, "oas_bp": 250.0}).get("leverage")
    3.0
    """
    def __init__(
        self, id: str, metrics: Any | None = None, tags: list[str] | None = None, meta: dict[str, str] | None = None
    ) -> None: ...
    @property
    def id(self) -> str:
        """
        Company identifier.

        This property does not raise.

        Returns
        -------
        str
            Company identifier.
        """
    @property
    def tags(self) -> list[str]:
        """
        Free-form attribute tags carried alongside the metrics.

        This property does not raise.

        Returns
        -------
        list[str]
            Tags in insertion order; empty when the company carries none.
        """
    @property
    def meta(self) -> dict[str, str]:
        """
        Attribute metadata.

        This property does not raise.

        Returns
        -------
        dict[str, str]
            Attribute metadata.
        """
    @property
    def custom(self) -> list[tuple[str, float]]:
        """
        Custom (non-canonical) metrics.

        This property does not raise.

        Returns
        -------
        list[tuple[str, float]]
            Custom (non-canonical) metrics.
        """
    def get(self, name: str) -> float | None:
        """
        Read one metric by name (canonical field or ``custom`` key).

        This method reads already-computed state and does not raise.

        Parameters
        ----------
        name : str
            Canonical metric field name (for example ``"ebitda"``) or a key
            of the ``custom`` mapping. Absent metrics return ``None``.

        Returns
        -------
        float | None
            Read one metric by name (canonical field or ``custom`` key). Returns ``None`` when the metric is absent.
        """
    def to_json(self) -> str:
        """
        Serialize to canonical JSON.

        Returns
        -------
        str
            Canonical JSON encoding of this value, accepted by
            :meth:`from_json`.

        Raises
        ------
        ValueError
            If the value cannot be serialized to JSON.
        """
    @staticmethod
    def from_json(json: str) -> CompanyMetrics:
        """
        Deserialize from canonical JSON.

        Parameters
        ----------
        json : str
            JSON document produced by :meth:`to_json`, in this type's
            canonical wire schema.

        Returns
        -------
        CompanyMetrics
            The value decoded from the supplied JSON document.

        Raises
        ------
        ValueError
            If ``json`` is not a valid ``CompanyMetrics`` document.

        Examples
        --------
        >>> from finstack_quant.statements_analytics import CompanyMetrics
        >>> CompanyMetrics.from_json("{")
        Traceback (most recent call last):
        ValueError: ...
        """

class CorporateAnalysis:
    """Orchestrated statement + equity + credit analysis envelope.

    Examples
    --------
    >>> from finstack_quant.statements import ModelBuilder
    >>> from finstack_quant.statements_analytics import run_corporate_analysis
    >>> b = ModelBuilder("m")
    >>> _ = b.periods("2024Q1..Q2", None)
    >>> _ = b.value("revenue", [("2024Q1", 1.0), ("2024Q2", 2.0)])
    >>> analysis = run_corporate_analysis(b.build())
    >>> analysis.equity is None, analysis.ev_suppressed_non_positive
    (True, False)
    """
    @property
    def statement(self) -> StatementResult:
        """
        Full statement evaluation.

        This property does not raise.

        Returns
        -------
        StatementResult
            Full statement evaluation.
        """
    @property
    def equity(self) -> CorporateValuationResult | None:
        """
        DCF valuation, or ``None`` when no ``wacc`` was configured.

        This property does not raise.

        Returns
        -------
        CorporateValuationResult | None
            DCF valuation, or ``None`` when no ``wacc`` was configured.
        """
    @property
    def credit(self) -> Any:
        """
                Per-instrument credit metrics as ``{instrument_id: CreditContextMetrics dict}``
        (serde form, including ``dscr_incl_fees`` / ``dscr_incl_fees_min``).

                This property does not raise.

                Returns
                -------
                Any
                    Per-instrument credit metrics as ``{instrument_id: CreditContextMetrics dict}`` (serde form, including ``dscr_incl_fees`` / ``dscr_incl_fees_min``).
        """
    @property
    def ev_suppressed_non_positive(self) -> bool:
        """
        Whether a non-positive DCF enterprise value was excluded from LTV.

        This property does not raise.

        Returns
        -------
        bool
            Whether a non-positive DCF enterprise value was excluded from LTV.
        """
    def to_dataframe(self) -> pd.DataFrame:
        """
        Export the per-instrument credit metrics as a long pandas ``DataFrame``.

        Columns: ``instrument_id``, ``period`` (period-id string), ``dscr``,
        ``dscr_total``, ``dscr_incl_fees``, ``interest_coverage`` (turns;
        ``NaN`` where a metric is not available for that period). One row per
        (instrument, period) present on any metric series.

        Returns
        -------
        pd.DataFrame
            Export the per-instrument credit metrics as a long pandas ``DataFrame``. Columns: ``instrument_id``, ``period`` (period-id string), ``dscr``, ``dscr_total``, ``dscr_incl_fees``, ``interest_coverage`` (turns; ``NaN`` where a metric is not available for that period). One row per (instrument, period) present on any metric series.

        Raises
        ------
        ImportError
            If pandas is not installed in the running interpreter.
        ValueError
            If the result cannot be converted to its wire form.
        """
    def to_json(self) -> str:
        """
        Serialize to canonical JSON.

        Returns
        -------
        str
            Canonical JSON encoding of this value, accepted by
            :meth:`from_json`.

        Raises
        ------
        ValueError
            If the value cannot be serialized to JSON.
        """
    @staticmethod
    def from_json(json: str) -> CorporateAnalysis:
        """
        Deserialize from canonical JSON.

        Parameters
        ----------
        json : str
            JSON document produced by :meth:`to_json`, in this type's
            canonical wire schema.

        Returns
        -------
        CorporateAnalysis
            The value decoded from the supplied JSON document.

        Raises
        ------
        ValueError
            If ``json`` is not a valid ``CorporateAnalysis`` document.

        Examples
        --------
        >>> from finstack_quant.statements_analytics import CorporateAnalysis
        >>> CorporateAnalysis.from_json("{")
        Traceback (most recent call last):
        ValueError: ...
        """

class CorporateValuationResult:
    """DCF outputs in the model currency.

    Examples
    --------
    >>> from finstack_quant.statements_analytics import CorporateValuationResult
    >>> r = CorporateValuationResult.from_json(
    ...     '{"equity_value":{"amount":"90","currency":"USD"},"enterprise_value":{"amount":"100","currency":"USD"},'
    ...     '"net_debt":{"amount":"10","currency":"USD"},"terminal_value_pv":{"amount":"60","currency":"USD"},'
    ...     '"equity_value_per_share":null,"diluted_shares":null}'
    ... )
    >>> r.equity_value.amount
    90.0
    """
    @property
    def equity_value(self) -> Money:
        """
        Equity value (EV less net debt, after discounts).

        This property does not raise.

        Returns
        -------
        Money
            Equity value (EV less net debt, after discounts).
        """
    @property
    def enterprise_value(self) -> Money:
        """
        Enterprise value (PV of forecast cash flows plus terminal value).

        This property does not raise.

        Returns
        -------
        Money
            Enterprise value (PV of forecast cash flows plus terminal value).
        """
    @property
    def net_debt(self) -> Money:
        """
        Net debt (or effective bridge amount) subtracted from EV.

        This property does not raise.

        Returns
        -------
        Money
            Net debt (or effective bridge amount) subtracted from EV.
        """
    @property
    def terminal_value_pv(self) -> Money:
        """
        Present value of the terminal value.

        This property does not raise.

        Returns
        -------
        Money
            Present value of the terminal value.
        """
    @property
    def equity_value_per_share(self) -> float | None:
        """
        Equity value per diluted share, or ``None`` without ``shares_outstanding``.

        This property does not raise.

        Returns
        -------
        float | None
            Equity value per diluted share, or ``None`` without ``shares_outstanding``.
        """
    @property
    def diluted_shares(self) -> float | None:
        """
        Diluted share count, or ``None`` without ``shares_outstanding``.

        This property does not raise.

        Returns
        -------
        float | None
            Diluted share count, or ``None`` without ``shares_outstanding``.
        """
    def to_dataframe(self) -> pd.DataFrame:
        """
        Export as a single-row pandas ``DataFrame``.

        Columns: ``currency``, ``equity_value``, ``enterprise_value``,
        ``net_debt``, ``terminal_value_pv`` (float amounts in ``currency``),
        ``equity_value_per_share``, ``diluted_shares`` (``None`` when absent).

        Returns
        -------
        pd.DataFrame
            Export as a single-row pandas ``DataFrame``. Columns: ``currency``, ``equity_value``, ``enterprise_value``, ``net_debt``, ``terminal_value_pv`` (float amounts in ``currency``), ``equity_value_per_share``, ``diluted_shares`` (``None`` when absent).

        Raises
        ------
        ImportError
            If pandas is not installed in the running interpreter.
        ValueError
            If the result cannot be converted to its wire form.
        """
    def to_json(self) -> str:
        """
        Serialize to canonical JSON (``Money`` fields as ``{"amount", "currency"}``).

        Returns
        -------
        str
            Serialize to canonical JSON (``Money`` fields as ``{"amount", "currency"}``).

        Raises
        ------
        ValueError
            If the value cannot be serialized to JSON.
        """
    @staticmethod
    def from_json(json: str) -> CorporateValuationResult:
        """
        Deserialize from canonical JSON.

        Parameters
        ----------
        json : str
            JSON document produced by :meth:`to_json`, in this type's
            canonical wire schema.

        Returns
        -------
        CorporateValuationResult
            The value decoded from the supplied JSON document.

        Raises
        ------
        ValueError
            If ``json`` is not a valid ``CorporateValuationResult`` document.

        Examples
        --------
        >>> from finstack_quant.statements_analytics import CorporateValuationResult
        >>> CorporateValuationResult.from_json("{")
        Traceback (most recent call last):
        ValueError: ...
        """

class CreditAssessment:
    """Structured credit assessment: leverage, coverage and free cash flow at a
    period plus the ascending per-period series.

    Examples
    --------
    >>> from finstack_quant.statements_analytics import CreditAssessment
    >>> a = CreditAssessment.from_json(
    ...     '{"period":"2025Q4","leverage_ratio":3.0,"interest_coverage":null,"free_cash_flow":null,"series":[]}'
    ... )
    >>> a.period, a.leverage_ratio
    ('2025Q4', 3.0)
    """
    @property
    def period(self) -> str:
        """
        Assessment period-id string (e.g. ``"2025Q4"``).

        This property does not raise.

        Returns
        -------
        str
            Assessment period-id string (e.g. ``"2025Q4"``).
        """
    @property
    def leverage_ratio(self) -> float | None:
        """
        Leverage ratio at ``period`` in turns, or ``None``.

        This property does not raise.

        Returns
        -------
        float | None
            Leverage ratio at ``period`` in turns, or ``None``.
        """
    @property
    def interest_coverage(self) -> float | None:
        """
        Interest coverage at ``period`` in turns, or ``None``.

        This property does not raise.

        Returns
        -------
        float | None
            Interest coverage at ``period`` in turns, or ``None``.
        """
    @property
    def free_cash_flow(self) -> float | None:
        """
        Free cash flow at ``period``, or ``None``.

        This property does not raise.

        Returns
        -------
        float | None
            Free cash flow at ``period``, or ``None``.
        """
    @property
    def series(self) -> list[CreditAssessmentPoint]:
        """
        Ascending per-period points up to and including ``period``.

        This property does not raise.

        Returns
        -------
        list[CreditAssessmentPoint]
            Ascending per-period points up to and including ``period``.
        """
    def to_dataframe(self) -> pd.DataFrame:
        """
        Export the per-period series as a pandas ``DataFrame``.

        Columns: ``period`` (period-id string), ``leverage_ratio``,
        ``interest_coverage`` (turns), ``free_cash_flow`` (model units);
        ``NaN`` where a metric is unavailable. One row per period, ascending.

        Returns
        -------
        pd.DataFrame
            Export the per-period series as a pandas ``DataFrame``. Columns: ``period`` (period-id string), ``leverage_ratio``, ``interest_coverage`` (turns), ``free_cash_flow`` (model units); ``NaN`` where a metric is unavailable. One row per period, ascending.

        Raises
        ------
        ImportError
            If pandas is not installed in the running interpreter.
        ValueError
            If the result cannot be converted to its wire form.
        """
    def to_json(self) -> str:
        """
        Serialize to canonical JSON.

        Returns
        -------
        str
            Canonical JSON encoding of this value, accepted by
            :meth:`from_json`.

        Raises
        ------
        ValueError
            If the value cannot be serialized to JSON.
        """
    @staticmethod
    def from_json(json: str) -> CreditAssessment:
        """
        Deserialize from canonical JSON.

        Parameters
        ----------
        json : str
            JSON document produced by :meth:`to_json`, in this type's
            canonical wire schema.

        Returns
        -------
        CreditAssessment
            The value decoded from the supplied JSON document.

        Raises
        ------
        ValueError
            If ``json`` is not a valid ``CreditAssessment`` document.

        Examples
        --------
        >>> from finstack_quant.statements_analytics import CreditAssessment
        >>> CreditAssessment.from_json("{")
        Traceback (most recent call last):
        ValueError: ...
        """

class CreditAssessmentPoint:
    """
    One period's structured credit metrics.

    Examples
    --------
    >>> from finstack_quant.statements_analytics import CreditAssessmentPoint
    >>> [field for field in ("period", "leverage_ratio") if hasattr(CreditAssessmentPoint, field)]
    ['period', 'leverage_ratio']
    """
    @property
    def period(self) -> str:
        """
        Period-id string (e.g. ``"2025Q4"``).

        This property does not raise.

        Returns
        -------
        str
            Period-id string (e.g. ``"2025Q4"``).
        """
    @property
    def leverage_ratio(self) -> float | None:
        """
        Total debt / TTM EBITDA in turns, or ``None`` without a full window.

        This property does not raise.

        Returns
        -------
        float | None
            Total debt / TTM EBITDA in turns, or ``None`` without a full window.
        """
    @property
    def interest_coverage(self) -> float | None:
        """
        TTM EBITDA / TTM interest expense in turns, or ``None``.

        This property does not raise.

        Returns
        -------
        float | None
            TTM EBITDA / TTM interest expense in turns, or ``None``.
        """
    @property
    def free_cash_flow(self) -> float | None:
        """
        Free cash flow at the period in model units, or ``None``.

        This property does not raise.

        Returns
        -------
        float | None
            Free cash flow at the period in model units, or ``None``.
        """

class CreditMapping:
    """
    Node-id mapping for the credit-underwriting check suite.

    The constructor stores the supplied values and does not raise.

    Parameters
    ----------
    debt_node : str
        Total-debt node.
    ebitda_node : str
        EBITDA node.
    interest_expense_node : str
        Interest-expense node.
    fcf_node : str | None
        Free-cash-flow node (enables the FCF sign check).
    cash_node : str | None
        Cash balance node (enables the liquidity check).
    cash_burn_node : str | None
        Cash-burn node (liquidity runway).
    leverage_warn : tuple[float, float] | None
        ``(warn, error)`` debt/EBITDA thresholds in turns.
    coverage_min_warn : float | None
        Minimum EBITDA/interest coverage in turns before a warning.

    Examples
    --------
    >>> from finstack_quant.statements_analytics import CreditMapping
    >>> CreditMapping("total_debt", "ebitda", "interest_expense", leverage_warn=(4.0, 6.0)).leverage_warn
    (4.0, 6.0)
    """
    def __init__(
        self,
        debt_node: str,
        ebitda_node: str,
        interest_expense_node: str,
        fcf_node: str | None = None,
        cash_node: str | None = None,
        cash_burn_node: str | None = None,
        leverage_warn: tuple[float, float] | None = None,
        coverage_min_warn: float | None = None,
    ) -> None: ...
    @property
    def debt_node(self) -> str:
        """
        Total-debt node.

        This property does not raise.

        Returns
        -------
        str
            Total-debt node.
        """
    @property
    def ebitda_node(self) -> str:
        """
        Statement node id read as EBITDA by the credit metrics.

        This property does not raise.

        Returns
        -------
        str
            Node id resolved against the evaluated statement model.
        """
    @property
    def interest_expense_node(self) -> str:
        """
        Interest-expense node.

        This property does not raise.

        Returns
        -------
        str
            Interest-expense node.
        """
    @property
    def fcf_node(self) -> str | None:
        """
        Free-cash-flow node, or ``None``.

        This property does not raise.

        Returns
        -------
        str | None
            Free-cash-flow node, or ``None``.
        """
    @property
    def cash_node(self) -> str | None:
        """
        Cash balance node, or ``None``.

        This property does not raise.

        Returns
        -------
        str | None
            Cash balance node, or ``None``.
        """
    @property
    def cash_burn_node(self) -> str | None:
        """
        Cash-burn node, or ``None``.

        This property does not raise.

        Returns
        -------
        str | None
            Cash-burn node, or ``None``.
        """
    @property
    def leverage_warn(self) -> tuple[float, float] | None:
        """
        ``(warn, error)`` leverage thresholds in turns, or ``None``.

        This property does not raise.

        Returns
        -------
        tuple[float, float] | None
            ``(warn, error)`` leverage thresholds in turns, or ``None``.
        """
    @property
    def coverage_min_warn(self) -> float | None:
        """
        Minimum coverage in turns before a warning, or ``None``.

        This property does not raise.

        Returns
        -------
        float | None
            Minimum coverage in turns before a warning, or ``None``.
        """
    def to_json(self) -> str:
        """
        Serialize to canonical JSON.

        Returns
        -------
        str
            Canonical JSON encoding of this value, accepted by
            :meth:`from_json`.

        Raises
        ------
        ValueError
            If the value cannot be serialized to JSON.
        """
    @staticmethod
    def from_json(json: str) -> CreditMapping:
        """
        Deserialize from canonical JSON.

        Parameters
        ----------
        json : str
            JSON document produced by :meth:`to_json`, in this type's
            canonical wire schema.

        Returns
        -------
        CreditMapping
            The value decoded from the supplied JSON document.

        Raises
        ------
        ValueError
            If ``json`` is not a valid ``CreditMapping`` document.

        Examples
        --------
        >>> from finstack_quant.statements_analytics import CreditMapping
        >>> CreditMapping.from_json("{")
        Traceback (most recent call last):
        ValueError: ...
        """

class DcfSensitivityResult:
    """Tornado ranking of the headline DCF assumptions by enterprise-value impact.

    Examples
    --------
    >>> from finstack_quant.statements_analytics import DcfSensitivityResult
    >>> r = DcfSensitivityResult.from_json(
    ...     '{"baseline_enterprise_value":{"amount":"100","currency":"USD"},'
    ...     '"entries":[{"parameter_id":"wacc","downside":-5.0,"upside":6.0}],'
    ...     '"wacc_down":0.09,"wacc_down_clamped":false,"terminal_growth_up":0.03,"terminal_growth_up_clamped":false}'
    ... )
    >>> list(r.to_dataframe()["swing"])
    [11.0]
    """
    @property
    def baseline_enterprise_value(self) -> Money:
        """
        Unshocked enterprise value.

        This property does not raise.

        Returns
        -------
        Money
            Unshocked enterprise value.
        """
    @property
    def entries(self) -> list[TornadoEntry]:
        """
                Tornado entries sorted by descending absolute swing (EV deltas versus
        the baseline).

                This property does not raise.

                Returns
                -------
                list[TornadoEntry]
                    Tornado entries sorted by descending absolute swing (EV deltas versus the baseline).
        """
    @property
    def wacc_down(self) -> float:
        """
        WACC used for the downside shock, in decimal form.

        This property does not raise.

        Returns
        -------
        float
            WACC used for the downside shock, in decimal form.
        """
    @property
    def wacc_down_clamped(self) -> bool:
        """
        Whether the WACC downside was clamped to keep ``wacc - g`` positive.

        This property does not raise.

        Returns
        -------
        bool
            Whether the WACC downside was clamped to keep ``wacc - g`` positive.
        """
    @property
    def terminal_growth_up(self) -> float | None:
        """
                Terminal growth used for the upside shock (decimal), or ``None`` for
        an exit-multiple terminal.

                This property does not raise.

                Returns
                -------
                float | None
                    Terminal growth used for the upside shock (decimal), or ``None`` for an exit-multiple terminal.
        """
    @property
    def terminal_growth_up_clamped(self) -> bool:
        """
        Whether the terminal-growth upside was clamped.

        This property does not raise.

        Returns
        -------
        bool
            Whether the terminal-growth upside was clamped.
        """
    def to_dataframe(self) -> pd.DataFrame:
        """
        Export the tornado table as a pandas ``DataFrame``.

        Columns: ``parameter_id``, ``downside``, ``upside``, ``swing`` (EV
        deltas in the baseline currency). One row per entry, in ranked order.

        Returns
        -------
        pd.DataFrame
            Export the tornado table as a pandas ``DataFrame``. Columns: ``parameter_id``, ``downside``, ``upside``, ``swing`` (EV deltas in the baseline currency). One row per entry, in ranked order.

        Raises
        ------
        ImportError
            If pandas is not installed in the running interpreter.
        ValueError
            If the result cannot be converted to its wire form.
        """
    def to_json(self) -> str:
        """
        Serialize to canonical JSON.

        Returns
        -------
        str
            Canonical JSON encoding of this value, accepted by
            :meth:`from_json`.

        Raises
        ------
        ValueError
            If the value cannot be serialized to JSON.
        """
    @staticmethod
    def from_json(json: str) -> DcfSensitivityResult:
        """
        Deserialize from canonical JSON.

        Parameters
        ----------
        json : str
            JSON document produced by :meth:`to_json`, in this type's
            canonical wire schema.

        Returns
        -------
        DcfSensitivityResult
            The value decoded from the supplied JSON document.

        Raises
        ------
        ValueError
            If ``json`` is not a valid ``DcfSensitivityResult`` document.

        Examples
        --------
        >>> from finstack_quant.statements_analytics import DcfSensitivityResult
        >>> DcfSensitivityResult.from_json("{")
        Traceback (most recent call last):
        ValueError: ...
        """

class DimensionScore:
    """
    Decomposed score of one dimension in a `RelativeValueResult`.

    Examples
    --------
    >>> from finstack_quant.statements_analytics import DimensionScore
    >>> [field for field in ("label", "percentile") if hasattr(DimensionScore, field)]
    ['label', 'percentile']
    """
    @property
    def label(self) -> str:
        """
        Dimension label.

        This property does not raise.

        Returns
        -------
        str
            Dimension label.
        """
    @property
    def percentile(self) -> float:
        """
        Percentile rank of the subject within peers (0-1).

        This property does not raise.

        Returns
        -------
        float
            Percentile rank of the subject within peers (0-1).
        """
    @property
    def z_score(self) -> float:
        """
        Raw z-score of the subject versus the peer distribution.

        This property does not raise.

        Returns
        -------
        float
            Raw z-score of the subject versus the peer distribution.
        """
    @property
    def regression_residual(self) -> float | None:
        """
        Raw regression residual in Y units, or ``None`` without explanatory X.

        This property does not raise.

        Returns
        -------
        float | None
            Raw regression residual in Y units, or ``None`` without explanatory X.
        """
    @property
    def r_squared(self) -> float | None:
        """
        Regression R-squared, or ``None`` without explanatory X.

        This property does not raise.

        Returns
        -------
        float | None
            Regression R-squared, or ``None`` without explanatory X.
        """
    @property
    def weight(self) -> float:
        """
        Dimension weight in the composite.

        This property does not raise.

        Returns
        -------
        float
            Dimension weight in the composite.
        """

class EclBucket:
    """
    One integration bucket of an ECL calculation.

    Examples
    --------
    >>> from finstack_quant.statements_analytics import EclBucket
    >>> [field for field in ("t_start", "t_end") if hasattr(EclBucket, field)]
    ['t_start', 't_end']
    """
    @property
    def t_start(self) -> float:
        """
        Bucket start in years.

        This property does not raise.

        Returns
        -------
        float
            Bucket start in years.
        """
    @property
    def t_end(self) -> float:
        """
        Bucket end in years.

        This property does not raise.

        Returns
        -------
        float
            Bucket end in years.
        """
    @property
    def marginal_pd(self) -> float:
        """
        Marginal default probability within the bucket (decimal).

        This property does not raise.

        Returns
        -------
        float
            Marginal default probability within the bucket (decimal).
        """
    @property
    def lgd(self) -> float:
        """
        Loss given default applied in the bucket (decimal).

        This property does not raise.

        Returns
        -------
        float
            Loss given default applied in the bucket (decimal).
        """
    @property
    def ead(self) -> float:
        """
        Exposure at default in the bucket, in base currency.

        This property does not raise.

        Returns
        -------
        float
            Exposure at default in the bucket, in base currency.
        """
    @property
    def discount_factor(self) -> float:
        """
        Discount factor at the bucket midpoint, or at recovery for Stage 3.

        This property does not raise.

        Returns
        -------
        float
            Discount factor at the bucket midpoint, or at recovery for Stage 3.
        """
    @property
    def ecl(self) -> float:
        """
        Bucket ECL contribution in base currency.

        This property does not raise.

        Returns
        -------
        float
            Bucket ECL contribution in base currency.
        """

class EclResult:
    """
    ECL for one exposure under one PD scenario, with bucket detail.

    Examples
    --------
    >>> from finstack_quant.statements_analytics import EclResult
    >>> EclResult.from_json("{")
    Traceback (most recent call last):
    ValueError: ...
    """
    @property
    def exposure_id(self) -> str:
        """
        Exposure identifier.

        This property does not raise.

        Returns
        -------
        str
            Exposure identifier.
        """
    @property
    def stage(self) -> Stage:
        """
        Stage used for the measurement horizon.

        This property does not raise.

        Returns
        -------
        Stage
            Stage used for the measurement horizon.
        """
    @property
    def ecl(self) -> float:
        """
        Total ECL in the exposure's base currency.

        This property does not raise.

        Returns
        -------
        float
            Total ECL in the exposure's base currency.
        """
    @property
    def horizon(self) -> float:
        """
        Measurement horizon in years.

        This property does not raise.

        Returns
        -------
        float
            Measurement horizon in years.
        """
    @property
    def buckets(self) -> list[EclBucket]:
        """
        Bucket-level contributions in time order.

        This property does not raise.

        Returns
        -------
        list[EclBucket]
            Bucket-level contributions in time order.
        """
    @property
    def meta(self) -> Any:
        """
        Result metadata (numeric mode, rounding context) as a dict.

        This property does not raise.

        Returns
        -------
        Any
            Result metadata (numeric mode, rounding context) as a dict.
        """
    def to_dataframe(self) -> pd.DataFrame:
        """
        Export the buckets as a pandas ``DataFrame``.

        Columns: ``t_start``, ``t_end`` (years), ``marginal_pd``, ``lgd``
        (decimals), ``ead``, ``ecl`` (base currency), ``discount_factor``.
        One row per bucket in time order.

        Returns
        -------
        pd.DataFrame
            Export the buckets as a pandas ``DataFrame``. Columns: ``t_start``, ``t_end`` (years), ``marginal_pd``, ``lgd`` (decimals), ``ead``, ``ecl`` (base currency), ``discount_factor``. One row per bucket in time order.

        Raises
        ------
        ImportError
            If pandas is not installed in the running interpreter.
        ValueError
            If the result cannot be converted to its wire form.
        """
    def to_json(self) -> str:
        """
        Serialize to canonical JSON.

        Returns
        -------
        str
            Canonical JSON encoding of this value, accepted by
            :meth:`from_json`.

        Raises
        ------
        ValueError
            If the value cannot be serialized to JSON.
        """
    @staticmethod
    def from_json(json: str) -> EclResult:
        """
        Deserialize from canonical JSON.

        Parameters
        ----------
        json : str
            JSON document produced by :meth:`to_json`, in this type's
            canonical wire schema.

        Returns
        -------
        EclResult
            The value decoded from the supplied JSON document.

        Raises
        ------
        ValueError
            If ``json`` is not a valid ``EclResult`` document.

        Examples
        --------
        >>> from finstack_quant.statements_analytics import EclResult
        >>> EclResult.from_json("{")
        Traceback (most recent call last):
        ValueError: ...
        """

class EquityBridge:
    """
    Structured enterprise-to-equity bridge.

    ``net_adjustment = total_debt - cash + preferred_equity + minority_interest
    - non_operating_assets + sum(other_adjustments)``; all amounts in the model
    currency.

    The constructor stores the supplied values and does not raise.

    Parameters
    ----------
    total_debt : float
        Interest-bearing debt. Default ``0.0``.
    cash : float
        Cash and equivalents. Default ``0.0``.
    preferred_equity : float
        Preferred equity claims. Default ``0.0``.
    minority_interest : float
        Non-controlling interests. Default ``0.0``.
    non_operating_assets : float
        Non-operating assets added back. Default ``0.0``.
    other_adjustments : list[tuple[str, float]]
        Labelled additional claims (positive reduces equity). Default ``[]``.

    Examples
    --------
    >>> from finstack_quant.statements_analytics import EquityBridge
    >>> EquityBridge(total_debt=500.0, cash=100.0).net_adjustment
    400.0
    """
    def __init__(
        self,
        total_debt: float = 0.0,
        cash: float = 0.0,
        preferred_equity: float = 0.0,
        minority_interest: float = 0.0,
        non_operating_assets: float = 0.0,
        other_adjustments: list[tuple[str, float]] = ...,
    ) -> None: ...
    @property
    def total_debt(self) -> float:
        """
        Interest-bearing debt.

        This property does not raise.

        Returns
        -------
        float
            Interest-bearing debt.
        """
    @property
    def cash(self) -> float:
        """
        Cash and equivalents.

        This property does not raise.

        Returns
        -------
        float
            Cash and equivalents.
        """
    @property
    def preferred_equity(self) -> float:
        """
        Preferred equity claims.

        This property does not raise.

        Returns
        -------
        float
            Preferred equity claims.
        """
    @property
    def minority_interest(self) -> float:
        """
        Non-controlling interests.

        This property does not raise.

        Returns
        -------
        float
            Non-controlling interests.
        """
    @property
    def non_operating_assets(self) -> float:
        """
        Non-operating assets.

        This property does not raise.

        Returns
        -------
        float
            Non-operating assets.
        """
    @property
    def other_adjustments(self) -> list[tuple[str, float]]:
        """
        Labelled additional claims.

        This property does not raise.

        Returns
        -------
        list[tuple[str, float]]
            Labelled additional claims.
        """
    @property
    def net_adjustment(self) -> float:
        """
        Net amount subtracted from enterprise value.

        This property does not raise.

        Returns
        -------
        float
            Net amount subtracted from enterprise value.
        """
    def to_json(self) -> str:
        """
        Serialize to canonical JSON.

        Returns
        -------
        str
            Canonical JSON encoding of this value, accepted by
            :meth:`from_json`.

        Raises
        ------
        ValueError
            If the value cannot be serialized to JSON.
        """
    @staticmethod
    def from_json(json: str) -> EquityBridge:
        """
        Deserialize from canonical JSON (unknown fields are rejected).

        Parameters
        ----------
        json : str
            JSON document produced by :meth:`to_json`, in this type's
            canonical wire schema.

        Returns
        -------
        EquityBridge
            Deserialize from canonical JSON (unknown fields are rejected).

        Raises
        ------
        ValueError
            If ``json`` is not a valid ``EquityBridge`` document.

        Examples
        --------
        >>> from finstack_quant.statements_analytics import EquityBridge
        >>> EquityBridge.from_json("{")
        Traceback (most recent call last):
        ValueError: ...
        """

class Explanation:
    """Explanation of how a node's value was derived at one period.

    Examples
    --------
    >>> from finstack_quant.statements_analytics import Explanation
    >>> e = Explanation.from_json(
    ...     '{"node_id":"profit","period_id":"2025Q1","final_value":50.0,'
    ...     '"node_type":"calculated","formula_text":"revenue * 0.5","breakdown":[]}'
    ... )
    >>> e.node_id, e.final_value
    ('profit', 50.0)
    """
    @property
    def node_id(self) -> str:
        """
        Explained node id.

        This property does not raise.

        Returns
        -------
        str
            Explained node id.
        """
    @property
    def period_id(self) -> str:
        """
        Period-id string of the explanation.

        This property does not raise.

        Returns
        -------
        str
            Period-id string of the explanation.
        """
    @property
    def final_value(self) -> float:
        """
        Node value at the period.

        This property does not raise.

        Returns
        -------
        float
            Node value at the period.
        """
    @property
    def node_type(self) -> str:
        """
        Node type serde name (e.g. ``"calculated"``, ``"input"``).

        This property does not raise.

        Returns
        -------
        str
            Node type serde name (e.g. ``"calculated"``, ``"input"``).
        """
    @property
    def formula_text(self) -> str | None:
        """
        Formula text, or ``None`` for non-formula nodes.

        This property does not raise.

        Returns
        -------
        str | None
            Formula text, or ``None`` for non-formula nodes.
        """
    @property
    def breakdown(self) -> list[ExplanationStep]:
        """
        Component breakdown in evaluation order.

        This property does not raise.

        Returns
        -------
        list[ExplanationStep]
            Component breakdown in evaluation order.
        """
    def to_text(self) -> str:
        """
        Human-readable multi-line explanation.

        This method reads already-computed state and does not raise.

        Returns
        -------
        str
            Human-readable multi-line explanation.
        """
    def to_dataframe(self) -> pd.DataFrame:
        """
        Export the breakdown as a pandas ``DataFrame``.

        Columns: ``component``, ``value``, ``operation`` (``None`` when absent).
        One row per breakdown step in evaluation order.

        Returns
        -------
        pd.DataFrame
            Export the breakdown as a pandas ``DataFrame``. Columns: ``component``, ``value``, ``operation`` (``None`` when absent). One row per breakdown step in evaluation order.

        Raises
        ------
        ImportError
            If pandas is not installed in the running interpreter.
        ValueError
            If the result cannot be converted to its wire form.
        """
    def to_json(self) -> str:
        """
        Serialize to canonical JSON (identical to the WASM ``explainFormula`` output).

        Returns
        -------
        str
            Serialize to canonical JSON (identical to the WASM ``explainFormula`` output).

        Raises
        ------
        ValueError
            If the value cannot be serialized to JSON.
        """
    @staticmethod
    def from_json(json: str) -> Explanation:
        """
        Deserialize from canonical JSON.

        Parameters
        ----------
        json : str
            JSON document produced by :meth:`to_json`, in this type's
            canonical wire schema.

        Returns
        -------
        Explanation
            The value decoded from the supplied JSON document.

        Raises
        ------
        ValueError
            If ``json`` is not a valid ``Explanation`` document.

        Examples
        --------
        >>> from finstack_quant.statements_analytics import Explanation
        >>> Explanation.from_json("{")
        Traceback (most recent call last):
        ValueError: ...
        """

class ExplanationStep:
    """
    One component of a formula explanation.

    Examples
    --------
    >>> from finstack_quant.statements_analytics import ExplanationStep
    >>> [field for field in ("component", "value") if hasattr(ExplanationStep, field)]
    ['component', 'value']
    """
    @property
    def component(self) -> str:
        """
        Component node id or literal text.

        This property does not raise.

        Returns
        -------
        str
            Component node id or literal text.
        """
    @property
    def value(self) -> float:
        """
        Component value at the explained period.

        This property does not raise.

        Returns
        -------
        float
            Component value at the explained period.
        """
    @property
    def operation(self) -> str | None:
        """
        Operation applied to the component (``"+"``, ``"*"``, ...), or ``None``.

        This property does not raise.

        Returns
        -------
        str | None
            Operation applied to the component (``"+"``, ``"*"``, ...), or ``None``.
        """

class Exposure:
    """
    A single credit exposure at a reporting date.

    Wraps the Rust ``Exposure`` and carries the two lifetime PDs the
    simplified SICR test compares. ``classify_stage`` reads days past due,
    qualitative flags, rating labels, previous stage and performing periods
    (``ead``, ``lgd`` and ``eir`` do not affect staging); ``compute_ecl`` prices
    ``ead + undrawn * ccf`` with ``lgd``, ``eir``, ``remaining_maturity`` and
    any ``ead_schedule``.

    The constructor stores the supplied values and does not raise.

    Parameters
    ----------
    id : str
        Unique identifier for the exposure.
    ead : float
        Drawn outstanding balance at the reporting date, in base currency.
    lgd : float
        Loss given default as a decimal fraction in ``[0, 1]``. For Stage 3,
        this is the undiscounted loss fraction; allowance is current EAD less
        discounted recovery ``(1 - lgd) * EAD``.
    eir : float
        Effective interest rate as a decimal annual rate, used as the IFRS 9
        discount rate.
    remaining_maturity : float
        Remaining maturity in years.
    current_pd : float
        Current lifetime probability of default as a decimal in ``[0, 1]``.
    origination_pd : float
        Lifetime probability of default at initial recognition, decimal.
    dpd : int
        Days past due. Default ``0``.
    undrawn : float
        Undrawn commitment in the same currency as ``ead``. Default ``0.0``.
    ccf : float
        Credit-conversion factor applied to ``undrawn``, decimal in ``[0, 1]``.
        Default ``0.75`` (Basel IRB revolver).
    current_rating : str | None
        Current rating label, used with ``origination_rating`` for the
        rating-downgrade trigger. Default ``None``.
    origination_rating : str | None
        Rating label at initial recognition. Default ``None``.
    qualitative_flags : QualitativeFlags | None
        SICR and default-evidence flags. Default: no flags.
    previous_stage : Stage | str | None
        Stage assigned at the previous reporting date, enabling the curing
        rules. Default ``None``.
    consecutive_performing_periods : int
        Performing periods since the last trigger, for curing. Default ``0``.
    ead_schedule : list[tuple[float, float]] | None
        Optional EAD amortisation profile as ``(time_years, ead)`` knots.
    segments : list[str] | None
        Portfolio segment keys. Default ``[]``.

    Examples
    --------
    >>> from finstack_quant.statements_analytics import Exposure
    >>> Exposure("loan", 1_000_000.0, 0.45, 0.06, 3.0, 0.02, 0.015).dpd
    0
    """
    def __init__(
        self,
        id: str,
        ead: float,
        lgd: float,
        eir: float,
        remaining_maturity: float,
        current_pd: float,
        origination_pd: float,
        dpd: int = 0,
        undrawn: float = 0.0,
        ccf: float = 0.75,
        current_rating: str | None = None,
        origination_rating: str | None = None,
        qualitative_flags: QualitativeFlags | None = None,
        previous_stage: Any | None = None,
        consecutive_performing_periods: int = 0,
        ead_schedule: list[tuple[float, float]] | None = None,
        segments: list[str] | None = None,
    ) -> None: ...
    @property
    def id(self) -> str:
        """
        Unique identifier for the exposure.

        This property does not raise.

        Returns
        -------
        str
            Unique identifier for the exposure.
        """
    @property
    def ead(self) -> float:
        """
        Drawn outstanding balance in base currency.

        This property does not raise.

        Returns
        -------
        float
            Drawn outstanding balance in base currency.
        """
    @property
    def undrawn(self) -> float:
        """
        Undrawn commitment in the same currency as ``ead``.

        This property does not raise.

        Returns
        -------
        float
            Undrawn commitment in the same currency as ``ead``.
        """
    @property
    def ccf(self) -> float:
        """
        Credit-conversion factor applied to ``undrawn`` (decimal in ``[0, 1]``).

        This property does not raise.

        Returns
        -------
        float
            Credit-conversion factor applied to ``undrawn`` (decimal in ``[0, 1]``).
        """
    @property
    def lgd(self) -> float:
        """
        Loss given default as a decimal fraction.

        This property does not raise.

        Returns
        -------
        float
            Loss given default as a decimal fraction.
        """
    @property
    def eir(self) -> float:
        """
        Effective interest rate as a decimal annual rate.

        This property does not raise.

        Returns
        -------
        float
            Effective interest rate as a decimal annual rate.
        """
    @property
    def remaining_maturity(self) -> float:
        """
        Remaining maturity in years.

        This property does not raise.

        Returns
        -------
        float
            Remaining maturity in years.
        """
    @property
    def dpd(self) -> int:
        """
        Days the exposure is past due at the reporting date.

        This property does not raise.

        Returns
        -------
        int
            Whole days past due; ``0`` for a performing exposure.
        """
    @property
    def current_rating(self) -> str | None:
        """
        Current rating label, or ``None``.

        This property does not raise.

        Returns
        -------
        str | None
            Current rating label, or ``None``.
        """
    @property
    def origination_rating(self) -> str | None:
        """
        Rating label at initial recognition, or ``None``.

        This property does not raise.

        Returns
        -------
        str | None
            Rating label at initial recognition, or ``None``.
        """
    @property
    def qualitative_flags(self) -> QualitativeFlags:
        """
        SICR and default-evidence flags.

        This property does not raise.

        Returns
        -------
        QualitativeFlags
            SICR and default-evidence flags.
        """
    @property
    def previous_stage(self) -> Stage | None:
        """
        Stage at the previous reporting date, or ``None``.

        This property does not raise.

        Returns
        -------
        Stage | None
            Stage at the previous reporting date, or ``None``.
        """
    @property
    def consecutive_performing_periods(self) -> int:
        """
        Performing periods since the last trigger.

        This property does not raise.

        Returns
        -------
        int
            Performing periods since the last trigger.
        """
    @property
    def ead_schedule(self) -> list[tuple[float, float]] | None:
        """
        EAD amortisation profile as ``(time_years, ead)`` knots, or ``None``.

        This property does not raise.

        Returns
        -------
        list[tuple[float, float]] | None
            EAD amortisation profile as ``(time_years, ead)`` knots, or ``None``.
        """
    @property
    def segments(self) -> list[str]:
        """
        Portfolio segment keys.

        This property does not raise.

        Returns
        -------
        list[str]
            Portfolio segment keys.
        """
    def to_dataframe(self) -> pd.DataFrame:
        """
        Export the exposure as a single-row pandas ``DataFrame``.

        Columns: ``id``, ``ead``, ``undrawn``, ``ccf``, ``lgd``, ``eir``,
        ``remaining_maturity``, ``current_pd``, ``origination_pd``, ``dpd``,
        ``current_rating``, ``origination_rating``. Amounts are in the
        exposure's base currency; ``ccf``, ``lgd`` and the PDs are decimal
        fractions; ``eir`` is a decimal annual rate; ``remaining_maturity`` is
        in years; ``dpd`` is whole days.

        Returns
        -------
        pd.DataFrame
            Export the exposure as a single-row pandas ``DataFrame``. Columns: ``id``, ``ead``, ``undrawn``, ``ccf``, ``lgd``, ``eir``, ``remaining_maturity``, ``current_pd``, ``origination_pd``, ``dpd``, ``current_rating``, ``origination_rating``. Amounts are in the exposure's base currency; ``ccf``, ``lgd`` and the PDs are decimal fractions; ``eir`` is a decimal annual rate; ``remaining_maturity`` is in years; ``dpd`` is whole days.

        Raises
        ------
        ImportError
            If pandas is not installed in the running interpreter.
        ValueError
            If the result cannot be converted to its wire form.
        """

class ForecastMetrics:
    """Forecast accuracy metrics (MAE, MAPE, sMAPE, RMSE).

    Examples
    --------
    >>> from finstack_quant.statements_analytics import backtest_forecast
    >>> metrics = backtest_forecast([100.0, 110.0], [98.0, 112.0])
    >>> metrics.n, metrics.mae
    (2, 2.0)
    """
    @property
    def mae(self) -> float:
        """
        Mean absolute error in data units.

        This property does not raise.

        Returns
        -------
        float
            Mean absolute error in data units.
        """
    @property
    def mape(self) -> float:
        """
                Mean absolute percentage error in percent (``5.0`` = 5%); ``NaN``
        when every actual is zero.

                This property does not raise.

                Returns
                -------
                float
                    Mean absolute percentage error in percent (``5.0`` = 5%); ``NaN`` when every actual is zero.
        """
    @property
    def mape_effective_n(self) -> int:
        """
        Number of observations with a non-zero actual used by ``mape``.

        This property does not raise.

        Returns
        -------
        int
            Number of observations with a non-zero actual used by ``mape``.
        """
    @property
    def smape(self) -> float:
        """
        Symmetric MAPE in percent.

        This property does not raise.

        Returns
        -------
        float
            Symmetric MAPE in percent.
        """
    @property
    def rmse(self) -> float:
        """
        Root mean squared error in data units.

        This property does not raise.

        Returns
        -------
        float
            Root mean squared error in data units.
        """
    @property
    def n(self) -> int:
        """
        Number of observations.

        This property does not raise.

        Returns
        -------
        int
            Number of observations.
        """
    def summary(self) -> str:
        """
        One-line human-readable summary (Rust ``ForecastMetrics::summary``).

        This method reads already-computed state and does not raise.

        Returns
        -------
        str
            One-line human-readable summary (Rust ``ForecastMetrics::summary``).
        """
    def to_series(self) -> pd.DataFrame:
        """
        Export as a pandas ``Series`` indexed by metric name.

        Index: ``mae``, ``mape``, ``mape_effective_n``, ``smape``, ``rmse``,
        ``n``; counts are cast to float.

        Returns
        -------
        pd.DataFrame
            Export as a pandas ``Series`` indexed by metric name. Index: ``mae``, ``mape``, ``mape_effective_n``, ``smape``, ``rmse``, ``n``; counts are cast to float.

        Raises
        ------
        ImportError
            If pandas is not installed in the running interpreter.
        ValueError
            If the result cannot be converted to its wire form.
        """
    def to_json(self) -> str:
        """
        Serialize to canonical JSON.

        Returns
        -------
        str
            Canonical JSON encoding of this value, accepted by
            :meth:`from_json`.

        Raises
        ------
        ValueError
            If the value cannot be serialized to JSON.
        """
    @staticmethod
    def from_json(json: str) -> ForecastMetrics:
        """
        Deserialize from canonical JSON.

        Parameters
        ----------
        json : str
            JSON document produced by :meth:`to_json`, in this type's
            canonical wire schema.

        Returns
        -------
        ForecastMetrics
            The value decoded from the supplied JSON document.

        Raises
        ------
        ValueError
            If ``json`` is not a valid ``ForecastMetrics`` document.

        Examples
        --------
        >>> from finstack_quant.statements_analytics import ForecastMetrics
        >>> ForecastMetrics.from_json("{")
        Traceback (most recent call last):
        ValueError: ...
        """

class GoalSeekResult:
    """
    Result of a goal-seek solve.

    Examples
    --------
    >>> from finstack_quant.statements_analytics import GoalSeekResult
    >>> [field for field in ("solved_value", "model") if hasattr(GoalSeekResult, field)]
    ['solved_value', 'model']
    """
    @property
    def solved_value(self) -> float:
        """
        Driver value that reaches the target.

        This property does not raise.

        Returns
        -------
        float
            Driver value that reaches the target.
        """
    @property
    def model(self) -> FinancialModelSpec | None:
        """
                Model with the solved driver written in, or ``None`` when
        ``update_model=False``.

                This property does not raise.

                Returns
                -------
                FinancialModelSpec | None
                    Model with the solved driver written in, or ``None`` when ``update_model=False``.
        """

class LboCheckMappings:
    """
    Node mappings that switch on the LBO model check suite.

    The constructor stores the supplied values and does not raise.

    Parameters
    ----------
    three_statement : ThreeStatementMapping | dict | str
        Balance-sheet / income / cash-flow node mapping.
    credit : CreditMapping | dict | str
        Leverage and coverage node mapping.

    Examples
    --------
    >>> from finstack_quant.statements_analytics import CreditMapping, LboCheckMappings, ThreeStatementMapping
    >>> m = LboCheckMappings(ThreeStatementMapping("cash", "re", "ni"), CreditMapping("debt", "ebitda", "interest"))
    >>> m.credit.debt_node
    'debt'
    """
    def __init__(self, three_statement: Any, credit: Any) -> None: ...
    @property
    def three_statement(self) -> ThreeStatementMapping:
        """
        Three-statement node mapping.

        This property does not raise.

        Returns
        -------
        ThreeStatementMapping
            Three-statement node mapping.
        """
    @property
    def credit(self) -> CreditMapping:
        """
        Credit node mapping.

        This property does not raise.

        Returns
        -------
        CreditMapping
            Credit node mapping.
        """
    def to_json(self) -> str:
        """
        Serialize to canonical JSON.

        Returns
        -------
        str
            Canonical JSON encoding of this value, accepted by
            :meth:`from_json`.

        Raises
        ------
        ValueError
            If the value cannot be serialized to JSON.
        """
    @staticmethod
    def from_json(json: str) -> LboCheckMappings:
        """
        Deserialize from canonical JSON (unknown fields are rejected).

        Parameters
        ----------
        json : str
            JSON document produced by :meth:`to_json`, in this type's
            canonical wire schema.

        Returns
        -------
        LboCheckMappings
            Deserialize from canonical JSON (unknown fields are rejected).

        Raises
        ------
        ValueError
            If ``json`` is not a valid ``LboCheckMappings`` document.

        Examples
        --------
        >>> from finstack_quant.statements_analytics import LboCheckMappings
        >>> LboCheckMappings.from_json("{")
        Traceback (most recent call last):
        ValueError: ...
        """

class LboResult:
    """Outputs of an LBO evaluation in the model currency.

    Examples
    --------
    >>> from finstack_quant.statements_analytics import LboResult
    >>> m = '{"amount":"100","currency":"USD"}'
    >>> r = LboResult.from_json(
    ...     '{"entry_enterprise_value":'
    ...     + m
    ...     + ',"entry_metric":10.0,"debt_total":'
    ...     + m
    ...     + ',"equity_check":'
    ...     + m
    ...     + ',"sources_total":'
    ...     + m
    ...     + ',"uses_total":'
    ...     + m
    ...     + ',"sources_uses_balanced":true,'
    ...     + '"exit_enterprise_value":'
    ...     + m
    ...     + ',"exit_metric":12.0,"exit_net_debt":'
    ...     + m
    ...     + ',"exit_equity_proceeds":'
    ...     + m
    ...     + ',"moic":2.0,"checks":null}'
    ... )
    >>> r.moic
    2.0
    """
    @property
    def entry_enterprise_value(self) -> Money:
        """
        Entry enterprise value (``entry_multiple * entry_metric``).

        This property does not raise.

        Returns
        -------
        Money
            Entry enterprise value (``entry_multiple * entry_metric``).
        """
    @property
    def entry_metric(self) -> float:
        """
        Entry metric read from the model's first period.

        This property does not raise.

        Returns
        -------
        float
            Entry metric read from the model's first period.
        """
    @property
    def debt_total(self) -> Money:
        """
        Total funded debt at close.

        This property does not raise.

        Returns
        -------
        Money
            Total funded debt at close.
        """
    @property
    def equity_check(self) -> Money:
        """
        Sponsor equity check (sources-and-uses residual).

        This property does not raise.

        Returns
        -------
        Money
            Sponsor equity check (sources-and-uses residual).
        """
    @property
    def sources_total(self) -> Money:
        """
        Total sources at close.

        This property does not raise.

        Returns
        -------
        Money
            Total sources at close.
        """
    @property
    def uses_total(self) -> Money:
        """
        Total uses at close (entry EV plus fees).

        This property does not raise.

        Returns
        -------
        Money
            Total uses at close (entry EV plus fees).
        """
    @property
    def sources_uses_balanced(self) -> bool:
        """
        Whether sources equal uses within tolerance.

        This property does not raise.

        Returns
        -------
        bool
            Whether sources equal uses within tolerance.
        """
    @property
    def exit_enterprise_value(self) -> Money:
        """
        Exit enterprise value.

        This property does not raise.

        Returns
        -------
        Money
            Exit enterprise value.
        """
    @property
    def exit_metric(self) -> float:
        """
        Exit metric read at ``exit_period``.

        This property does not raise.

        Returns
        -------
        float
            Exit metric read at ``exit_period``.
        """
    @property
    def exit_net_debt(self) -> Money:
        """
        Net debt at exit.

        This property does not raise.

        Returns
        -------
        Money
            Net debt at exit.
        """
    @property
    def exit_equity_proceeds(self) -> Money:
        """
        Equity proceeds at exit.

        This property does not raise.

        Returns
        -------
        Money
            Equity proceeds at exit.
        """
    @property
    def moic(self) -> float:
        """
        Multiple of invested capital (``2.4`` = 2.4x).

        This property does not raise.

        Returns
        -------
        float
            Multiple of invested capital (``2.4`` = 2.4x).
        """
    @property
    def checks(self) -> CheckReport | None:
        """
                LBO model check report, or ``None`` when no ``check_mappings`` were
        supplied.

                This property does not raise.

                Returns
                -------
                CheckReport | None
                    LBO model check report, or ``None`` when no ``check_mappings`` were supplied.
        """
    def to_dataframe(self) -> pd.DataFrame:
        """
        Export as a single-row pandas ``DataFrame``.

        Columns: ``currency``, ``entry_enterprise_value``, ``entry_metric``,
        ``debt_total``, ``equity_check``, ``sources_total``, ``uses_total``,
        ``sources_uses_balanced``, ``exit_enterprise_value``, ``exit_metric``,
        ``exit_net_debt``, ``exit_equity_proceeds``, ``moic``. Money columns are
        float amounts in ``currency``.

        Returns
        -------
        pd.DataFrame
            Export as a single-row pandas ``DataFrame``. Columns: ``currency``, ``entry_enterprise_value``, ``entry_metric``, ``debt_total``, ``equity_check``, ``sources_total``, ``uses_total``, ``sources_uses_balanced``, ``exit_enterprise_value``, ``exit_metric``, ``exit_net_debt``, ``exit_equity_proceeds``, ``moic``. Money columns are float amounts in ``currency``.

        Raises
        ------
        ImportError
            If pandas is not installed in the running interpreter.
        ValueError
            If the result cannot be converted to its wire form.
        """
    def to_json(self) -> str:
        """
        Serialize to canonical JSON.

        Returns
        -------
        str
            Canonical JSON encoding of this value, accepted by
            :meth:`from_json`.

        Raises
        ------
        ValueError
            If the value cannot be serialized to JSON.
        """
    @staticmethod
    def from_json(json: str) -> LboResult:
        """
        Deserialize from canonical JSON.

        Parameters
        ----------
        json : str
            JSON document produced by :meth:`to_json`, in this type's
            canonical wire schema.

        Returns
        -------
        LboResult
            The value decoded from the supplied JSON document.

        Raises
        ------
        ValueError
            If ``json`` is not a valid ``LboResult`` document.

        Examples
        --------
        >>> from finstack_quant.statements_analytics import LboResult
        >>> LboResult.from_json("{")
        Traceback (most recent call last):
        ValueError: ...
        """

class PLSummaryReport:
    """P&L summary of selected line items across periods.

    Built by ``pl_summary_report``; renders to text, an ``ArrowTable`` or a
    pandas frame from the same Rust implementation.

    Examples
    --------
    >>> from finstack_quant.statements import ModelBuilder, Evaluator
    >>> from finstack_quant.statements_analytics import pl_summary_report
    >>> b = ModelBuilder("m")
    >>> _ = b.periods("2025Q1..Q2", None)
    >>> _ = b.value("revenue", [("2025Q1", 1.0), ("2025Q2", 2.0)])
    >>> report = pl_summary_report(Evaluator().evaluate(b.build()), ["revenue"], ["2025Q1", "2025Q2"])
    >>> list(report.to_dataframe()["value"])
    [1.0, 2.0]
    """
    @property
    def line_items(self) -> list[str]:
        """
        Node ids shown as rows.

        This property does not raise.

        Returns
        -------
        list[str]
            Node ids shown as rows.
        """
    @property
    def periods(self) -> list[str]:
        """
        Period-id strings shown as columns.

        This property does not raise.

        Returns
        -------
        list[str]
            Period-id strings shown as columns.
        """
    def to_text(self) -> str:
        """
        Render the box-drawn text table.

        This method reads already-computed state and does not raise.

        Returns
        -------
        str
            Render the box-drawn text table.
        """
    def to_table(self) -> Any:
        """
        Export the report as a long ``ArrowTable``.

        Columns: ``line_item``, ``period``, ``value`` (nullable; missing line
        items are null rather than ``0.0``).

        Returns
        -------
        Any
            Export the report as a long ``ArrowTable``. Columns: ``line_item``, ``period``, ``value`` (nullable; missing line items are null rather than ``0.0``).

        Raises
        ------
        ImportError
            If pandas is not installed in the running interpreter.
        ValueError
            If the result cannot be converted to its wire form.
        """
    def to_dataframe(self) -> pd.DataFrame:
        """
        Export the report as a long pandas ``DataFrame``.

        Columns: ``line_item``, ``period`` (period-id string), ``value`` (the
        node's value in its own units; ``NaN`` where the line item is missing).
        One row per (line item, period). Pivot with
        ``df.pivot(index="line_item", columns="period", values="value")`` for
        the line-items-by-periods layout of the text report.

        Returns
        -------
        pd.DataFrame
            Export the report as a long pandas ``DataFrame``. Columns: ``line_item``, ``period`` (period-id string), ``value`` (the node's value in its own units; ``NaN`` where the line item is missing). One row per (line item, period). Pivot with ``df.pivot(index="line_item", columns="period", values="value")`` for the line-items-by-periods layout of the text report.

        Raises
        ------
        ImportError
            If pandas is not installed in the running interpreter.
        ValueError
            If the result cannot be converted to its wire form.
        """

class ParameterSpec:
    """
    One parameter to vary in a sensitivity run.

    The constructor stores the supplied values and does not raise.

    Parameters
    ----------
    node_id : str
        Node identifier to perturb.
    period : str
        Period-id string of the perturbed value (e.g. ``"2025Q2"``).
    base_value : float
        Unperturbed value, recorded for reference.
    perturbations : list[float]
        Absolute replacement values applied one at a time.

    Examples
    --------
    >>> from finstack_quant.statements_analytics import ParameterSpec
    >>> ParameterSpec.with_percentages("revenue", "2025Q2", 100.0, [-10.0, 10.0]).perturbations
    [90.0, 110.00000000000001]
    """
    def __init__(self, node_id: str, period: str, base_value: float, perturbations: list[float]) -> None: ...
    @staticmethod
    def with_percentages(node_id: str, period: str, base_value: float, pct_range: list[float]) -> ParameterSpec:
        """
        Build a spec whose perturbations are ``base_value * (1 + pct / 100)``.

        Parameters
        ----------
        node_id : str
            Node identifier to perturb.
        period : str
            Period-id string of the perturbed value.
        base_value : float
            Unperturbed value the percentages are applied to.
        pct_range : list[float]
            Percentage bumps (``[-10.0, 0.0, 10.0]`` = -10%, 0%, +10%).

        Returns
        -------
        ParameterSpec
            Build a spec whose perturbations are ``base_value * (1 + pct / 100)``.

        Raises
        ------
        ValueError
            If ``period`` does not parse.

        Examples
        --------
        >>> from finstack_quant.statements_analytics import ParameterSpec
        >>> ParameterSpec.with_percentages("revenue", "2025Q1", 100.0, [-0.1, 0.1]).perturbations
        [99.9, 100.1]
        """
    @property
    def node_id(self) -> str:
        """
        Node identifier to perturb.

        This property does not raise.

        Returns
        -------
        str
            Node identifier to perturb.
        """
    @property
    def period(self) -> str:
        """
        Period-id string of the perturbed value.

        This property does not raise.

        Returns
        -------
        str
            Period-id string of the perturbed value.
        """
    @property
    def base_value(self) -> float:
        """
        Unperturbed value.

        This property does not raise.

        Returns
        -------
        float
            Unperturbed value.
        """
    @property
    def perturbations(self) -> list[float]:
        """
        Absolute replacement values applied one at a time.

        This property does not raise.

        Returns
        -------
        list[float]
            Absolute replacement values applied one at a time.
        """
    def to_json(self) -> str:
        """
        Serialize to canonical JSON.

        Returns
        -------
        str
            Canonical JSON encoding of this value, accepted by
            :meth:`from_json`.

        Raises
        ------
        ValueError
            If the value cannot be serialized to JSON.
        """
    @staticmethod
    def from_json(json: str) -> ParameterSpec:
        """
        Deserialize from canonical JSON.

        Parameters
        ----------
        json : str
            JSON document produced by :meth:`to_json`, in this type's
            canonical wire schema.

        Returns
        -------
        ParameterSpec
            The value decoded from the supplied JSON document.

        Raises
        ------
        ValueError
            If ``json`` is not a valid ``ParameterSpec`` document.

        Examples
        --------
        >>> from finstack_quant.statements_analytics import ParameterSpec
        >>> ParameterSpec.from_json("{")
        Traceback (most recent call last):
        ValueError: ...
        """

class PeerFilter:
    """
    Screening criteria for building a peer set from a universe.

    All non-empty criteria are AND-ed; list criteria are OR-ed within.

    The constructor stores the supplied values and does not raise.

    Parameters
    ----------
    gics_sectors : list[str]
        GICS sector codes to include (``meta["gics_sector"]``).
    gics_industries : list[str]
        GICS industry codes to include (``meta["gics_industry"]``).
    countries : list[str]
        ISO country codes to include (``meta["country"]``).
    market_cap_min : float | None
        Inclusive market-cap floor.
    market_cap_max : float | None
        Inclusive market-cap ceiling.
    ratings : list[str]
        Rating bands to include (``meta["rating"]``).
    required_tags : list[str]
        Tags every peer must carry.
    excluded_tags : list[str]
        Tags no peer may carry.
    selectors : list[str]
        Attribute selector strings (``Attributes.matches_selector``).

    Examples
    --------
    >>> from finstack_quant.statements_analytics import PeerFilter
    >>> PeerFilter(ratings=["BB", "B"]).ratings
    ['BB', 'B']
    """
    def __init__(
        self,
        gics_sectors: list[str] = ...,
        gics_industries: list[str] = ...,
        countries: list[str] = ...,
        market_cap_min: float | None = None,
        market_cap_max: float | None = None,
        ratings: list[str] = ...,
        required_tags: list[str] = ...,
        excluded_tags: list[str] = ...,
        selectors: list[str] = ...,
    ) -> None: ...
    @property
    def gics_sectors(self) -> list[str]:
        """
        GICS sector codes to include.

        This property does not raise.

        Returns
        -------
        list[str]
            GICS sector codes to include.
        """
    @property
    def gics_industries(self) -> list[str]:
        """
        GICS industry codes to include.

        This property does not raise.

        Returns
        -------
        list[str]
            GICS industry codes to include.
        """
    @property
    def countries(self) -> list[str]:
        """
        ISO country codes to include.

        This property does not raise.

        Returns
        -------
        list[str]
            ISO country codes to include.
        """
    @property
    def market_cap_min(self) -> float | None:
        """
        Inclusive market-cap floor, or ``None``.

        This property does not raise.

        Returns
        -------
        float | None
            Inclusive market-cap floor, or ``None``.
        """
    @property
    def market_cap_max(self) -> float | None:
        """
        Inclusive market-cap ceiling, or ``None``.

        This property does not raise.

        Returns
        -------
        float | None
            Inclusive market-cap ceiling, or ``None``.
        """
    @property
    def ratings(self) -> list[str]:
        """
        Rating bands to include.

        This property does not raise.

        Returns
        -------
        list[str]
            Rating bands to include.
        """
    @property
    def required_tags(self) -> list[str]:
        """
        Tags every peer must carry.

        This property does not raise.

        Returns
        -------
        list[str]
            Tags every peer must carry.
        """
    @property
    def excluded_tags(self) -> list[str]:
        """
        Tags no peer may carry.

        This property does not raise.

        Returns
        -------
        list[str]
            Tags no peer may carry.
        """
    @property
    def selectors(self) -> list[str]:
        """
        Attribute selector strings.

        This property does not raise.

        Returns
        -------
        list[str]
            Attribute selector strings.
        """
    def accepts(self, company: CompanyMetrics) -> bool:
        """
        Whether ``company`` satisfies every criterion.

        This method reads already-computed state and does not raise.

        Parameters
        ----------
        company : CompanyMetrics
            Candidate peer tested against the filter's size, sector and
            metric criteria.

        Returns
        -------
        bool
            Whether ``company`` satisfies every criterion.
        """
    def to_json(self) -> str:
        """
        Serialize to canonical JSON.

        Returns
        -------
        str
            Canonical JSON encoding of this value, accepted by
            :meth:`from_json`.

        Raises
        ------
        ValueError
            If the value cannot be serialized to JSON.
        """
    @staticmethod
    def from_json(json: str) -> PeerFilter:
        """
        Deserialize from canonical JSON.

        Parameters
        ----------
        json : str
            JSON document produced by :meth:`to_json`, in this type's
            canonical wire schema.

        Returns
        -------
        PeerFilter
            The value decoded from the supplied JSON document.

        Raises
        ------
        ValueError
            If ``json`` is not a valid ``PeerFilter`` document.

        Examples
        --------
        >>> from finstack_quant.statements_analytics import PeerFilter
        >>> PeerFilter.from_json("{")
        Traceback (most recent call last):
        ValueError: ...
        """

class PeerSet:
    """
    A subject company alongside its comparison peers.

    The constructor stores the supplied values and does not raise.

    Parameters
    ----------
    subject : CompanyMetrics | dict | str
        The company being evaluated (typed, serde dict, or JSON).
    peers : list[CompanyMetrics | dict | str]
        Peer companies.
    period_basis : str
        ``"ltm"``, ``"ntm"`` or a custom label such as ``"FY2025E"``.
        Default ``"ltm"``.

    Examples
    --------
    >>> from finstack_quant.statements_analytics import CompanyMetrics, PeerSet
    >>> subject = CompanyMetrics("SUBJ", {"leverage": 2.0})
    >>> peers = [CompanyMetrics("P1", {"leverage": 1.0}), CompanyMetrics("P2", {"leverage": 3.0})]
    >>> PeerSet(subject, peers).peer_count
    2
    """
    def __init__(self, subject: Any, peers: list[Any], period_basis: str = "ltm") -> None: ...
    @staticmethod
    def from_universe(subject: Any, universe: list[Any], filter: PeerFilter, period_basis: str = "ltm") -> PeerSet:
        """
        Build a peer set from a universe by applying a ``PeerFilter``.

        The subject is never included in the peers even when it passes.

        Raises
        ------
        ValueError
            If ``subject`` or a universe entry is not valid company metrics.

        Parameters
        ----------
        subject : CompanyMetrics | dict | str
            The company being evaluated.
        universe : list[CompanyMetrics | dict | str]
            Candidate companies.
        filter : PeerFilter
            Screening criteria.
        period_basis : str
            ``"ltm"``, ``"ntm"`` or a custom label. Default ``"ltm"``.
        Returns
        -------
        PeerSet
            Build a peer set from a universe by applying a ``PeerFilter``. The subject is never included in the peers even when it passes.

        Examples
        --------
        >>> from finstack_quant.statements_analytics import PeerFilter, PeerSet
        >>> PeerSet.from_universe("{}", [], PeerFilter())
        Traceback (most recent call last):
        ValueError: ...
        """
    @staticmethod
    def from_dataframe(
        df: Any, subject_id: str, period_basis: str = "ltm", id_column: str | None = None
    ) -> pd.DataFrame:
        """
        Build a peer set from a pandas ``DataFrame`` (rows = companies).

        Parameters
        ----------
        df : pandas.DataFrame
            One row per company. The index (or ``id_column``) supplies company
            ids; numeric columns become metrics (canonical names fill their
            fields, others go to ``custom``); string columns become attribute
            ``meta`` entries; ``NaN``/``None`` cells are treated as missing.
        subject_id : str
            Id of the subject row; every other row becomes a peer.
        period_basis : str
            ``"ltm"``, ``"ntm"`` or a custom label. Default ``"ltm"``.
        id_column : str | None
            Column holding company ids; ``None`` uses the index.

        Returns
        -------
        pd.DataFrame
            Build a peer set from a pandas ``DataFrame`` (rows = companies).

        Raises
        ------
        KeyError
            If ``subject_id`` is not present.
        ValueError
            If a cell is neither numeric, string, nor missing.

        Examples
        --------
        >>> from finstack_quant.statements_analytics import PeerSet
        >>> PeerSet.from_dataframe("{}", "subject")
        Traceback (most recent call last):
        AttributeError: ...
        """
    @property
    def subject(self) -> CompanyMetrics:
        """
        The subject company.

        This property does not raise.

        Returns
        -------
        CompanyMetrics
            The subject company.
        """
    @property
    def peers(self) -> list[CompanyMetrics]:
        """
        Companies retained as comparables for the subject.

        This property does not raise.

        Returns
        -------
        list[CompanyMetrics]
            Peers in selection order, always excluding the subject.
        """
    @property
    def period_basis(self) -> str:
        """
        Period basis label (``"ltm"``, ``"ntm"`` or the custom label).

        This property does not raise.

        Returns
        -------
        str
            Period basis label (``"ltm"``, ``"ntm"`` or the custom label).
        """
    @property
    def peer_count(self) -> int:
        """
        Number of peers (excluding the subject).

        This property does not raise.

        Returns
        -------
        int
            Number of peers (excluding the subject).
        """
    def to_json(self) -> str:
        """
        Serialize to canonical JSON.

        Returns
        -------
        str
            Canonical JSON encoding of this value, accepted by
            :meth:`from_json`.

        Raises
        ------
        ValueError
            If the value cannot be serialized to JSON.
        """
    @staticmethod
    def from_json(json: str) -> PeerSet:
        """
        Deserialize from canonical JSON.

        Parameters
        ----------
        json : str
            JSON document produced by :meth:`to_json`, in this type's
            canonical wire schema.

        Returns
        -------
        PeerSet
            The value decoded from the supplied JSON document.

        Raises
        ------
        ValueError
            If ``json`` is not a valid ``PeerSet`` document.

        Examples
        --------
        >>> from finstack_quant.statements_analytics import PeerSet
        >>> PeerSet.from_json("{")
        Traceback (most recent call last):
        ValueError: ...
        """

class PeerStats:
    """Descriptive statistics of a peer distribution.

    Examples
    --------
    >>> from finstack_quant.statements_analytics import peer_stats
    >>> peer_stats([1.0, 2.0, 3.0, 4.0, 5.0]).median
    3.0
    """
    @property
    def count(self) -> int:
        """
        Number of observations.

        This property does not raise.

        Returns
        -------
        int
            Number of observations.
        """
    @property
    def mean(self) -> float:
        """
        Arithmetic mean.

        This property does not raise.

        Returns
        -------
        float
            Arithmetic mean.
        """
    @property
    def median(self) -> float:
        """
        Median of the peer metric across the peer set.

        This property does not raise.

        Returns
        -------
        float
            Median value in the metric's own units.
        """
    @property
    def std_dev(self) -> float:
        """
        Sample standard deviation.

        This property does not raise.

        Returns
        -------
        float
            Sample standard deviation.
        """
    @property
    def min(self) -> float:
        """
        Smallest peer value of the metric.

        This property does not raise.

        Returns
        -------
        float
            Minimum value in the metric's own units.
        """
    @property
    def max(self) -> float:
        """
        Largest peer value of the metric.

        This property does not raise.

        Returns
        -------
        float
            Maximum value in the metric's own units.
        """
    @property
    def q1(self) -> float:
        """
        First quartile (25th percentile) of the peer metric.

        This property does not raise.

        Returns
        -------
        float
            25th-percentile value in the metric's own units.
        """
    @property
    def q3(self) -> float:
        """
        Third quartile (75th percentile) of the peer metric.

        This property does not raise.

        Returns
        -------
        float
            75th-percentile value in the metric's own units.
        """
    @property
    def iqr(self) -> float:
        """
        Interquartile range ``q3 - q1``.

        This property does not raise.

        Returns
        -------
        float
            Interquartile range ``q3 - q1``.
        """
    def to_dataframe(self) -> pd.DataFrame:
        """
        Export as a single-row pandas ``DataFrame`` with one column per statistic.

        Returns
        -------
        pd.DataFrame
            Export as a single-row pandas ``DataFrame`` with one column per statistic.

        Raises
        ------
        ImportError
            If pandas is not installed in the running interpreter.
        ValueError
            If the result cannot be converted to its wire form.
        """
    def to_json(self) -> str:
        """
        Serialize to canonical JSON.

        Returns
        -------
        str
            Canonical JSON encoding of this value, accepted by
            :meth:`from_json`.

        Raises
        ------
        ValueError
            If the value cannot be serialized to JSON.
        """
    @staticmethod
    def from_json(json: str) -> PeerStats:
        """
        Deserialize from canonical JSON.

        Parameters
        ----------
        json : str
            JSON document produced by :meth:`to_json`, in this type's
            canonical wire schema.

        Returns
        -------
        PeerStats
            The value decoded from the supplied JSON document.

        Raises
        ------
        ValueError
            If ``json`` is not a valid ``PeerStats`` document.

        Examples
        --------
        >>> from finstack_quant.statements_analytics import PeerStats
        >>> PeerStats.from_json("{")
        Traceback (most recent call last):
        ValueError: ...
        """

class QualitativeFlags:
    """
    Qualitative SICR and default-evidence flags for staging (IFRS 9 B5.5.17 / B5.5.37).

    The constructor stores the supplied values and does not raise.

    Parameters
    ----------
    watchlist : bool
        Exposure is on an internal watchlist (SICR indicator). Default ``False``.
    forbearance : bool
        Forbearance measures were granted (SICR indicator). Default ``False``.
    adverse_conditions : bool
        Adverse business, financial or economic conditions (SICR indicator).
        Default ``False``.
    custom : list[str]
        Additional caller-defined SICR flags; any non-empty entry counts as an
        active flag. Default ``[]``.
    bankruptcy : bool
        Objective evidence of default: bankruptcy or similar proceedings.
        Default ``False``.
    distressed_modification : bool
        Objective evidence of default: distressed restructuring. Default ``False``.
    cross_default : bool
        Objective evidence of default: cross-default triggered. Default ``False``.
    other_default_evidence : list[str]
        Additional caller-defined default-evidence flags. Default ``[]``.

    Examples
    --------
    >>> from finstack_quant.statements_analytics import QualitativeFlags
    >>> QualitativeFlags(watchlist=True).active_flags
    ['watchlist']
    """
    def __init__(
        self,
        watchlist: bool = False,
        forbearance: bool = False,
        adverse_conditions: bool = False,
        custom: list[str] = ...,
        bankruptcy: bool = False,
        distressed_modification: bool = False,
        cross_default: bool = False,
        other_default_evidence: list[str] = ...,
    ) -> None: ...
    @property
    def watchlist(self) -> bool:
        """
        Internal watchlist flag (SICR indicator).

        This property does not raise.

        Returns
        -------
        bool
            Internal watchlist flag (SICR indicator).
        """
    @property
    def forbearance(self) -> bool:
        """
        Forbearance flag (SICR indicator).

        This property does not raise.

        Returns
        -------
        bool
            Forbearance flag (SICR indicator).
        """
    @property
    def adverse_conditions(self) -> bool:
        """
        Adverse-conditions flag (SICR indicator).

        This property does not raise.

        Returns
        -------
        bool
            Adverse-conditions flag (SICR indicator).
        """
    @property
    def custom(self) -> list[str]:
        """
        Caller-defined SICR flags.

        This property does not raise.

        Returns
        -------
        list[str]
            Caller-defined SICR flags.
        """
    @property
    def bankruptcy(self) -> bool:
        """
        Bankruptcy flag (objective evidence of default).

        This property does not raise.

        Returns
        -------
        bool
            Bankruptcy flag (objective evidence of default).
        """
    @property
    def distressed_modification(self) -> bool:
        """
        Distressed-modification flag (objective evidence of default).

        This property does not raise.

        Returns
        -------
        bool
            Distressed-modification flag (objective evidence of default).
        """
    @property
    def cross_default(self) -> bool:
        """
        Cross-default flag (objective evidence of default).

        This property does not raise.

        Returns
        -------
        bool
            Cross-default flag (objective evidence of default).
        """
    @property
    def other_default_evidence(self) -> list[str]:
        """
        Caller-defined default-evidence flags.

        This property does not raise.

        Returns
        -------
        list[str]
            Caller-defined default-evidence flags.
        """
    @property
    def active_flags(self) -> list[str]:
        """
        Names of the active SICR flags, in waterfall order.

        This property does not raise.

        Returns
        -------
        list[str]
            Names of the active SICR flags, in waterfall order.
        """
    @property
    def active_default_evidence(self) -> list[str]:
        """
        Names of the active default-evidence flags, in waterfall order.

        This property does not raise.

        Returns
        -------
        list[str]
            Names of the active default-evidence flags, in waterfall order.
        """
    def to_json(self) -> str:
        """
        Serialize to canonical JSON.

        Returns
        -------
        str
            Canonical JSON encoding of this value, accepted by
            :meth:`from_json`.

        Raises
        ------
        ValueError
            If the value cannot be serialized to JSON.
        """
    @staticmethod
    def from_json(json: str) -> QualitativeFlags:
        """
        Deserialize from canonical JSON.

        Parameters
        ----------
        json : str
            JSON document produced by :meth:`to_json`, in this type's
            canonical wire schema.

        Returns
        -------
        QualitativeFlags
            The value decoded from the supplied JSON document.

        Raises
        ------
        ValueError
            If ``json`` is not a valid ``QualitativeFlags`` document.

        Examples
        --------
        >>> from finstack_quant.statements_analytics import QualitativeFlags
        >>> QualitativeFlags.from_json("{")
        Traceback (most recent call last):
        ValueError: ...
        """

class RegressionResult:
    """Single-factor OLS fit evaluated at the subject.

    Examples
    --------
    >>> from finstack_quant.statements_analytics import regression_fair_value
    >>> regression_fair_value([1.0, 2.0, 3.0, 4.0], [3.0, 5.0, 7.0, 9.0], 3.0, 10.0).fitted_value
    7.0
    """
    @property
    def intercept(self) -> float:
        """
        Intercept (alpha).

        This property does not raise.

        Returns
        -------
        float
            Intercept (alpha).
        """
    @property
    def slope(self) -> float:
        """
        Fitted regression slope on the explanatory variable.

        This property does not raise.

        Returns
        -------
        float
            Slope coefficient in units of y per unit of x.
        """
    @property
    def r_squared(self) -> float:
        """
        Coefficient of determination.

        This property does not raise.

        Returns
        -------
        float
            Coefficient of determination.
        """
    @property
    def fitted_value(self) -> float:
        """
        ``intercept + slope * subject_x``.

        This property does not raise.

        Returns
        -------
        float
            ``intercept + slope * subject_x``.
        """
    @property
    def residual(self) -> float:
        """
        ``subject_y - fitted_value``.

        This property does not raise.

        Returns
        -------
        float
            ``subject_y - fitted_value``.
        """
    @property
    def n(self) -> int:
        """
        Number of observations used.

        This property does not raise.

        Returns
        -------
        int
            Number of observations used.
        """
    def to_dataframe(self) -> pd.DataFrame:
        """
        Export as a single-row pandas ``DataFrame``.

        Returns
        -------
        pd.DataFrame
            Export as a single-row pandas ``DataFrame``.

        Raises
        ------
        ImportError
            If pandas is not installed in the running interpreter.
        ValueError
            If the result cannot be converted to its wire form.
        """
    def to_json(self) -> str:
        """
        Serialize to canonical JSON.

        Returns
        -------
        str
            Canonical JSON encoding of this value, accepted by
            :meth:`from_json`.

        Raises
        ------
        ValueError
            If the value cannot be serialized to JSON.
        """
    @staticmethod
    def from_json(json: str) -> RegressionResult:
        """
        Deserialize from canonical JSON.

        Parameters
        ----------
        json : str
            JSON document produced by :meth:`to_json`, in this type's
            canonical wire schema.

        Returns
        -------
        RegressionResult
            The value decoded from the supplied JSON document.

        Raises
        ------
        ValueError
            If ``json`` is not a valid ``RegressionResult`` document.

        Examples
        --------
        >>> from finstack_quant.statements_analytics import RegressionResult
        >>> RegressionResult.from_json("{")
        Traceback (most recent call last):
        ValueError: ...
        """

class RelativeValueResult:
    """Composite rich/cheap score of a subject against its peers.

    Examples
    --------
    >>> from finstack_quant.statements_analytics import (
    ...     CompanyMetrics,
    ...     PeerSet,
    ...     ScoringDimension,
    ...     score_relative_value,
    ... )
    >>> peers = [CompanyMetrics(f"P{i}", {"leverage": float(i), "oas_bp": 100.0 * i}) for i in (1, 2, 3)]
    >>> peer_set = PeerSet(CompanyMetrics("SUBJ", {"leverage": 2.0, "oas_bp": 250.0}), peers)
    >>> result = score_relative_value(peer_set, [ScoringDimension("Spread vs Leverage", "oas_bp", "leverage")])
    >>> result.company_id, result.peer_count
    ('SUBJ', 3)
    """
    @property
    def company_id(self) -> str:
        """
        Subject company id.

        This property does not raise.

        Returns
        -------
        str
            Subject company id.
        """
    @property
    def composite_score(self) -> float:
        """
        Weighted composite score: positive = cheap, negative = rich.

        This property does not raise.

        Returns
        -------
        float
            Weighted composite score: positive = cheap, negative = rich.
        """
    @property
    def dimensions(self) -> list[DimensionScore]:
        """
        Per-dimension decomposition.

        This property does not raise.

        Returns
        -------
        list[DimensionScore]
            Per-dimension decomposition.
        """
    @property
    def confidence(self) -> float:
        """
        Confidence in ``[0, 1]`` from peer count and regression fit.

        This property does not raise.

        Returns
        -------
        float
            Confidence in ``[0, 1]`` from peer count and regression fit.
        """
    @property
    def peer_count(self) -> int:
        """
        Number of peers scored against.

        This property does not raise.

        Returns
        -------
        int
            Number of peers scored against.
        """
    def to_dataframe(self) -> pd.DataFrame:
        """
        Export the per-dimension scores as a pandas ``DataFrame``.

        Columns: ``label``, ``percentile`` (0-1), ``z_score``,
        ``regression_residual`` (Y units, ``NaN`` without X), ``r_squared``
        (``NaN`` without X), ``weight``. One row per dimension. The composite
        score, confidence and peer count are result metadata.

        Returns
        -------
        pd.DataFrame
            Export the per-dimension scores as a pandas ``DataFrame``. Columns: ``label``, ``percentile`` (0-1), ``z_score``, ``regression_residual`` (Y units, ``NaN`` without X), ``r_squared`` (``NaN`` without X), ``weight``. One row per dimension. The composite score, confidence and peer count are result metadata.

        Raises
        ------
        ImportError
            If pandas is not installed in the running interpreter.
        ValueError
            If the result cannot be converted to its wire form.
        """
    def to_json(self) -> str:
        """
        Serialize to canonical JSON.

        Returns
        -------
        str
            Canonical JSON encoding of this value, accepted by
            :meth:`from_json`.

        Raises
        ------
        ValueError
            If the value cannot be serialized to JSON.
        """
    @staticmethod
    def from_json(json: str) -> RelativeValueResult:
        """
        Deserialize from canonical JSON.

        Parameters
        ----------
        json : str
            JSON document produced by :meth:`to_json`, in this type's
            canonical wire schema.

        Returns
        -------
        RelativeValueResult
            The value decoded from the supplied JSON document.

        Raises
        ------
        ValueError
            If ``json`` is not a valid ``RelativeValueResult`` document.

        Examples
        --------
        >>> from finstack_quant.statements_analytics import RelativeValueResult
        >>> RelativeValueResult.from_json("{")
        Traceback (most recent call last):
        ValueError: ...
        """

class ScenarioDiff:
    """
    Variance between two named scenarios in an evaluated scenario set.

    Examples
    --------
    >>> from finstack_quant.statements_analytics import ScenarioDiff
    >>> ScenarioDiff.from_json("{")
    Traceback (most recent call last):
    ValueError: ...
    """
    @staticmethod
    def from_json(json: str) -> ScenarioDiff:
        """
        Deserialize a scenario diff from its canonical JSON form.

        Every input is stored verbatim, so this constructor does not raise.

        Parameters
        ----------
        json : str
            JSON document produced by :meth:`to_json`, in this type's
            canonical wire schema.

        Returns
        -------
        ScenarioDiff
            Deserialize a scenario diff from its canonical JSON form.

        Examples
        --------
        >>> from finstack_quant.statements_analytics import ScenarioDiff
        >>> ScenarioDiff.from_json("{")
        Traceback (most recent call last):
        ValueError: ...
        """
    def to_json(self) -> str:
        """
        Serialize to canonical JSON (``baseline``, ``comparison``, ``variance``).

        Returns
        -------
        str
            Serialize to canonical JSON (``baseline``, ``comparison``, ``variance``).

        Raises
        ------
        ValueError
            If the value cannot be serialized to JSON.
        """
    @property
    def baseline(self) -> str:
        """
        Name of the scenario used as the baseline of the diff.

        This property does not raise.

        Returns
        -------
        str
            Name of the scenario used as the baseline of the diff.
        """
    @property
    def comparison(self) -> str:
        """
        Name of the scenario compared against the baseline.

        This property does not raise.

        Returns
        -------
        str
            Name of the scenario compared against the baseline.
        """
    @property
    def variance(self) -> VarianceReport:
        """
        Underlying variance report between the two named scenarios.

        This property does not raise.

        Returns
        -------
        VarianceReport
            Underlying variance report between the two named scenarios.
        """
    def to_dataframe(self) -> pd.DataFrame:
        """
        Export the underlying variance rows as a pandas ``DataFrame``.

        Columns: ``period``, ``metric``, ``baseline``, ``comparison``,
        ``abs_var``, ``pct_var``. One row per (metric, period) pair, in report
        order; an empty diff still carries the full column schema.

        This is the same table as ``variance.to_dataframe()`` — both call one
        implementation, so the two cannot drift apart. The two scenario *names*
        are diff metadata (the ``baseline`` / ``comparison`` getters) and are
        not repeated per row; the ``baseline`` and ``comparison`` columns hold
        the metric *values* in each scenario.

        Returns
        -------
        pd.DataFrame
            Export the underlying variance rows as a pandas ``DataFrame``. Columns: ``period``, ``metric``, ``baseline``, ``comparison``, ``abs_var``, ``pct_var``. One row per (metric, period) pair, in report order; an empty diff still carries the full column schema. This is the same table as ``variance.to_dataframe()`` — both call one implementation, so the two cannot drift apart. The two scenario *names* are diff metadata (the ``baseline`` / ``comparison`` getters) and are not repeated per row; the ``baseline`` and ``comparison`` columns hold the metric *values* in each scenario.

        Raises
        ------
        ImportError
            If pandas is not installed in the running interpreter.
        ValueError
            If the result cannot be converted to its wire form.
        """

class ScenarioResults:
    """
    Typed evaluated results for a set of named scenarios.

    Named after the canonical Rust type
    (`finstack_quant_statements_analytics::analysis::ScenarioResults`).

    Examples
    --------
    >>> from finstack_quant.statements_analytics import ScenarioResults
    >>> ScenarioResults.from_json("{")
    Traceback (most recent call last):
    ValueError: ...
    """
    @staticmethod
    def from_json(json: str) -> ScenarioResults:
        """
        Rebuild a ``ScenarioResults`` from its canonical JSON form.

        Parameters
        ----------
        json : str
            JSON document produced by :meth:`to_json`, in this type's
            canonical wire schema.

        Returns
        -------
        ScenarioResults
            The value decoded from the supplied JSON document.

        Raises
        ------
        ValueError
            If ``json`` is malformed or is not a valid ``ScenarioResults`` document.

        Examples
        --------
        >>> from finstack_quant.statements_analytics import ScenarioResults
        >>> ScenarioResults.from_json("{")
        Traceback (most recent call last):
        ValueError: ...
        """
    def to_json(self) -> str:
        """
        Serialize to canonical JSON.

        Returns
        -------
        str
            Canonical JSON encoding of this ``ScenarioResults``, accepted by
            :meth:`from_json`.

        Raises
        ------
        ValueError
            If the value cannot be serialized to JSON.
        """
    @property
    def names(self) -> list[str]:
        """
        Evaluated scenario names, in the order the scenario set defined them.

        This property does not raise.

        Returns
        -------
        list[str]
            Evaluated scenario names, in the order the scenario set defined them.
        """
    def get(self, name: str) -> StatementResult | None:
        """
        Look up one scenario result by scenario name.

        Parameters
        ----------
        name : str
            Scenario name as declared in the evaluated ``ScenarioSet``.

        Returns
        -------
        StatementResult | None
            Evaluated statements for that scenario, or ``None`` when no
            scenario carries the name.

        This lookup returns ``None`` for an unknown name and does not raise.
        """
    def to_comparison_table(self, metrics: list[str]) -> Any:
        """Build a side-by-side comparison table across every evaluated scenario.

        Parameters
        ----------
        metrics : list[str]
            Node identifiers to include as rows.

        Returns
        -------
        ArrowTable
            One column per scenario, one row per (metric, period).

        Raises
        ------
        ValueError
            If the result set or `metrics` is empty.
        """
    def to_dataframe(self, metrics: list[str]) -> pd.DataFrame:
        """
        Export the scenario comparison as a pandas ``DataFrame``.

        Columns: ``period``, ``metric``, one column per scenario name holding
        that scenario's metric value, and one ``{scenario}_vs_{baseline}_frac``
        column per non-baseline scenario holding the relative change as a
        decimal fraction (``0.1`` = +10%, ``NaN`` on a near-zero baseline). The
        ``_frac`` suffix states the unit: multiply by 100 for percent.
        One row per (metric, period) pair.

        This is the same table as ``to_comparison_table`` — both call one Rust
        implementation, so the two exports cannot drift apart. The baseline is
        the scenario named ``"base"`` when present, otherwise the first
        scenario.

        Parameters
        ----------
        metrics : list[str]
            Node identifiers to include as rows.

        Returns
        -------
        pd.DataFrame
            Export the scenario comparison as a pandas ``DataFrame``. Columns: ``period``, ``metric``, one column per scenario name holding that scenario's metric value, and one ``{scenario}_vs_{baseline}_frac`` column per non-baseline scenario holding the relative change as a decimal fraction (``0.1`` = +10%, ``NaN`` on a near-zero baseline). The ``_frac`` suffix states the unit: multiply by 100 for percent. One row per (metric, period) pair. This is the same table as ``to_comparison_table`` — both call one Rust implementation, so the two exports cannot drift apart. The baseline is the scenario named ``"base"`` when present, otherwise the first scenario.

        Raises
        ------
        ValueError
            If the result set or ``metrics`` is empty.
        """

class ScenarioSet:
    """
    Named scenario definitions for statement-model evaluation.

    Parameters
    ----------
    scenarios : dict[str, dict[str, float | Money | dict[str, float | Money]]]
        Scenario name to overrides. Each override value is either a model-wide
        value applied to every forecast period (``{"revenue": 90.0}``) or a
        ``{period: value}`` dict applied to the named forecast periods only
        (``{"growth": {"2025Q3": 0.02}}``); per-period values win over a
        model-wide value for that period.
    parents : dict[str, str] | None
        Optional scenario name to parent scenario name for inheritance.

    Raises
    ------
    ValueError
        If a scenario override cannot be read or a parent name is not a string.

    Examples
    --------
    >>> from finstack_quant.statements_analytics import ScenarioSet
    >>> ScenarioSet({"base": {}, "down": {"revenue": {"2025Q2": 90.0}}}, parents={"down": "base"}).names
    ['base', 'down']
    """
    def __init__(self, scenarios: Any, parents: Any | None = None) -> None: ...
    @staticmethod
    def from_json(json: str) -> ScenarioSet:
        """
        Rebuild a ``ScenarioSet`` from its canonical JSON form.

        Parameters
        ----------
        json : str
            JSON document produced by :meth:`to_json`, in this type's
            canonical wire schema.

        Returns
        -------
        ScenarioSet
            The value decoded from the supplied JSON document.

        Raises
        ------
        ValueError
            If ``json`` is malformed or is not a valid ``ScenarioSet`` document.

        Examples
        --------
        >>> from finstack_quant.statements_analytics import ScenarioSet
        >>> ScenarioSet.from_json("{")
        Traceback (most recent call last):
        ValueError: ...
        """
    def to_json(self) -> str:
        """
        Serialize to canonical JSON.

        Returns
        -------
        str
            Canonical JSON encoding of this ``ScenarioSet``, accepted by
            :meth:`from_json`.

        Raises
        ------
        ValueError
            If the value cannot be serialized to JSON.
        """
    @property
    def names(self) -> list[str]:
        """
        Scenario names in definition (insertion) order.

        This property does not raise.

        Returns
        -------
        list[str]
            Scenario names in definition (insertion) order.
        """
    def trace(self, scenario: str) -> list[str]:
        """Resolve a scenario's inheritance lineage, root-first.

        Parameters
        ----------
        scenario : str
            Name of the scenario to trace.

        Returns
        -------
        list[str]
            Scenario names from the root ancestor through to `scenario`.

        Raises
        ------
        ValueError
            If the scenario is unknown or its parent chain contains a cycle.
        """

class ScoringDimension:
    """
    One weighted rich/cheap scoring dimension.

    Parameters
    ----------
    label : str
        Human-readable label (e.g. ``"Spread vs Leverage"``).
    y : str
        Dependent metric: a canonical name (``"oas_bp"``), a custom key, or
        ``"multiple:<name>"`` for a valuation multiple (``"multiple:ev_ebitda"``).
    x : str | None
        Optional explanatory metric in the same notation. ``None`` (default)
        scores the dependent metric against its peer distribution.
    weight : float
        Weight in the composite score. Default ``1.0``.
    direction : str
        ``"higher_is_cheap"`` (spread-like, default) or ``"higher_is_rich"``
        (multiple-like).

    Raises
    ------
    ValueError
        If ``direction`` or an extractor name is not recognized.

    Examples
    --------
    >>> from finstack_quant.statements_analytics import ScoringDimension
    >>> ScoringDimension("Spread vs Leverage", "oas_bp", "leverage").direction
    'higher_is_cheap'
    """
    def __init__(
        self, label: str, y: str, x: str | None = None, weight: float = 1.0, direction: str = "higher_is_cheap"
    ) -> None: ...
    @property
    def label(self) -> str:
        """
        Dimension label.

        This property does not raise.

        Returns
        -------
        str
            Dimension label.
        """
    @property
    def y(self) -> str:
        """
        Dependent metric in ``name`` / ``multiple:<name>`` notation.

        This property does not raise.

        Returns
        -------
        str
            Dependent metric in ``name`` / ``multiple:<name>`` notation.
        """
    @property
    def x(self) -> str | None:
        """
        Optional explanatory metric in ``name`` / ``multiple:<name>`` notation; None for distribution scoring.

        This property does not raise.

        Returns
        -------
        str | None
            Optional explanatory metric in ``name`` / ``multiple:<name>`` notation; None for distribution scoring.
        """
    @property
    def weight(self) -> float:
        """
        Weight in the composite score.

        This property does not raise.

        Returns
        -------
        float
            Weight in the composite score.
        """
    @property
    def direction(self) -> str:
        """
        ``"higher_is_cheap"`` or ``"higher_is_rich"``.

        This property does not raise.

        Returns
        -------
        str
            ``"higher_is_cheap"`` or ``"higher_is_rich"``.
        """
    def to_json(self) -> str:
        """
        Serialize to canonical JSON.

        Returns
        -------
        str
            Canonical JSON encoding of this value, accepted by
            :meth:`from_json`.

        Raises
        ------
        ValueError
            If the value cannot be serialized to JSON.
        """
    @staticmethod
    def from_json(json: str) -> ScoringDimension:
        """
        Deserialize from canonical JSON.

        Parameters
        ----------
        json : str
            JSON document produced by :meth:`to_json`, in this type's
            canonical wire schema.

        Returns
        -------
        ScoringDimension
            The value decoded from the supplied JSON document.

        Raises
        ------
        ValueError
            If ``json`` is not a valid ``ScoringDimension`` document.

        Examples
        --------
        >>> from finstack_quant.statements_analytics import ScoringDimension
        >>> ScoringDimension.from_json("{")
        Traceback (most recent call last):
        ValueError: ...
        """

class SensitivityConfig:
    """
    Configuration for statement sensitivity analysis.

    The constructor stores the supplied values and does not raise.

    Parameters
    ----------
    mode : str
        ``"diagonal"`` (one-at-a-time), ``"full_grid"`` or ``"tornado"``.
    parameters : list[ParameterSpec | tuple[str, str, float, list[float]]]
        Parameters to vary; tuples are ``(node_id, period, base_value,
        perturbations)`` with absolute replacement values.
    target_metrics : list[str]
        Node identifiers tracked across scenarios.

    Examples
    --------
    >>> from finstack_quant.statements_analytics import ParameterSpec, SensitivityConfig
    >>> cfg = SensitivityConfig("diagonal", target_metrics=["profit"])
    >>> cfg.add_parameter("revenue", "2025Q2", 100.0, pct=[-10.0, 10.0])
    >>> cfg.parameters[0].perturbations
    [90.0, 110.00000000000001]
    """
    def __init__(self, mode: str, parameters: list[Any] = ..., target_metrics: list[str] = ...) -> None: ...
    def add_parameter(
        self,
        node_id: str,
        period: str,
        base_value: float,
        perturbations: list[float] | None = None,
        pct: list[float] | None = None,
    ) -> None:
        """Append a parameter to vary.

        Parameters
        ----------
        node_id : str
            Node identifier to perturb.
        period : str
            Period-id string of the perturbed value.
        base_value : float
            Unperturbed value.
        perturbations : list[float] | None
            Absolute replacement values.
        pct : list[float] | None
            Percentage bumps applied to ``base_value`` (``[-10.0, 10.0]`` =
            -10% / +10%). Exactly one of ``perturbations`` and ``pct`` must be
            given.

        Raises
        ------
        ValueError
            If ``period`` does not parse or neither/both of ``perturbations``
            and ``pct`` are supplied.
        """
    @property
    def parameters(self) -> list[ParameterSpec]:
        """
        Configured parameters in insertion order.

        This property does not raise.

        Returns
        -------
        list[ParameterSpec]
            Configured parameters in insertion order.
        """
    @staticmethod
    def from_json(json: str) -> SensitivityConfig:
        """
        Rebuild a ``SensitivityConfig`` from its canonical JSON form.

        Parameters
        ----------
        json : str
            JSON document produced by :meth:`to_json`, in this type's
            canonical wire schema.

        Returns
        -------
        SensitivityConfig
            The value decoded from the supplied JSON document.

        Raises
        ------
        ValueError
            If ``json`` is malformed or is not a valid ``SensitivityConfig`` document.

        Examples
        --------
        >>> from finstack_quant.statements_analytics import SensitivityConfig
        >>> SensitivityConfig.from_json("{")
        Traceback (most recent call last):
        ValueError: ...
        """
    def to_json(self) -> str:
        """
        Serialize to canonical JSON.

        Returns
        -------
        str
            Canonical JSON encoding of this ``SensitivityConfig``, accepted by
            :meth:`from_json`.

        Raises
        ------
        ValueError
            If the value cannot be serialized to JSON.
        """
    @property
    def mode(self) -> str:
        """
                Analysis mode: ``"diagonal"``, ``"full_grid"``, or ``"tornado"`` (the
        serde name, identical to ``to_json``).

                This property does not raise.

                Returns
                -------
                str
                    Analysis mode: ``"diagonal"``, ``"full_grid"``, or ``"tornado"`` (the serde name, identical to ``to_json``).
        """
    @property
    def target_metrics(self) -> list[str]:
        """
        Node identifiers of the statement metrics tracked across scenarios.

        This property does not raise.

        Returns
        -------
        list[str]
            Node identifiers of the statement metrics tracked across scenarios.
        """
    @property
    def parameter_count(self) -> int:
        """
        Number of configured parameters (one `ParameterSpec` per entry).

        This property does not raise.

        Returns
        -------
        int
            Number of configured parameters (one `ParameterSpec` per entry).
        """

class SensitivityResult:
    """
    Typed root result for statement sensitivity analysis.

    Examples
    --------
    >>> from finstack_quant.statements_analytics import SensitivityResult
    >>> SensitivityResult.from_json("{")
    Traceback (most recent call last):
    ValueError: ...
    """
    @staticmethod
    def from_json(json: str) -> SensitivityResult:
        """
        Rebuild a ``SensitivityResult`` from its canonical JSON form.

        Parameters
        ----------
        json : str
            JSON document produced by :meth:`to_json`, in this type's
            canonical wire schema.

        Returns
        -------
        SensitivityResult
            The value decoded from the supplied JSON document.

        Raises
        ------
        ValueError
            If ``json`` is malformed or is not a valid ``SensitivityResult`` document.

        Examples
        --------
        >>> from finstack_quant.statements_analytics import SensitivityResult
        >>> SensitivityResult.from_json("{")
        Traceback (most recent call last):
        ValueError: ...
        """
    def to_json(self) -> str:
        """
        Serialize to canonical JSON.

        Returns
        -------
        str
            Canonical JSON encoding of this ``SensitivityResult``, accepted by
            :meth:`from_json`.

        Raises
        ------
        ValueError
            If the value cannot be serialized to JSON.
        """
    @property
    def target_metrics(self) -> list[str]:
        """
        Node identifiers of the metrics tracked by the originating config.

        This property does not raise.

        Returns
        -------
        list[str]
            Node identifiers of the metrics tracked by the originating config.
        """
    @property
    def config(self) -> SensitivityConfig:
        """
        Configuration the run was generated from.

        This property does not raise.

        Returns
        -------
        SensitivityConfig
            Configuration the run was generated from.
        """
    @property
    def baseline(self) -> StatementResult | None:
        """
        Unperturbed baseline evaluation (populated by tornado runs), or ``None``.

        This property does not raise.

        Returns
        -------
        StatementResult | None
            Unperturbed baseline evaluation (populated by tornado runs), or ``None``.
        """
    @property
    def scenarios(self) -> list[tuple[Any, StatementResult]]:
        """
                Per-scenario ``(parameter_values, results)`` pairs in generation order,
        where ``parameter_values`` is a ``{"node_id@period": value}`` dict.

                This property does not raise.

                Returns
                -------
                list[tuple[Any, StatementResult]]
                    Per-scenario ``(parameter_values, results)`` pairs in generation order, where ``parameter_values`` is a ``{"node_id@period": value}`` dict.
        """
    def to_dataframe(self, metrics: list[str] | None = None) -> pd.DataFrame:
        """
        Export the run as a long pandas ``DataFrame``.

        Columns: ``scenario`` (0-based index), one column per perturbed
        parameter named ``node_id@period`` (``NaN`` where a scenario does not
        perturb it), then ``node_id``, ``period`` and ``value`` holding each
        tracked metric's value in that scenario. One row per (scenario, metric,
        period); when no metric is tracked, one row per scenario with the
        metric columns null so the parameter grid stays visible.

        Parameters
        ----------
        metrics : list[str] | None
            Node identifiers to emit; ``None`` uses the config's
            ``target_metrics``.
        Returns
        -------
        pd.DataFrame
            Export the run as a long pandas ``DataFrame``. Columns: ``scenario`` (0-based index), one column per perturbed parameter named ``node_id@period`` (``NaN`` where a scenario does not perturb it), then ``node_id``, ``period`` and ``value`` holding each tracked metric's value in that scenario. One row per (scenario, metric, period); when no metric is tracked, one row per scenario with the metric columns null so the parameter grid stays visible.

        Raises
        ------
        ImportError
            If pandas is not installed in the running interpreter.
        ValueError
            If the result cannot be converted to its wire form.
        """
    def get_parameter_value(self, scenario_index: int, parameter: str) -> float | None:
        """
        Read the perturbed parameter value used by one sensitivity scenario.

        Parameters
        ----------
        scenario_index : int
            Zero-based position of the scenario in ``scenarios``.
        parameter : str
            Parameter name as declared on the ``ParameterSpec``.

        Returns
        -------
        float | None
            Value applied to that parameter, or ``None`` when the scenario
            did not perturb it.

        Raises
        ------
        IndexError
            If ``scenario_index`` is outside the evaluated scenario range.
        """
    def get_value(self, scenario_index: int, node_id: str, period: str) -> float | None:
        """
        Read one evaluated node value from a sensitivity scenario.

        Parameters
        ----------
        scenario_index : int
            Zero-based position of the scenario in ``scenarios``.
        node_id : str
            Statement node whose value is read.
        period : str
            Period id such as ``"2025Q4"``.

        Returns
        -------
        float | None
            Node value in that scenario and period, or ``None`` when the
            node was not evaluated there.

        Raises
        ------
        IndexError
            If ``scenario_index`` is outside the evaluated scenario range.
        ValueError
            If ``period`` is not a parsable period id.
        """

class Stage:
    """IFRS 9 impairment stage.

    ``Stage.Stage1`` measures a 12-month ECL; ``Stage.Stage2`` and
    ``Stage.Stage3`` measure lifetime ECL (Stage 3 is credit-impaired).
    ``value`` is the serde name (``"stage1"``, ``"stage2"``, ``"stage3"``)
    accepted wherever a stage string is taken.

    Examples
    --------
    >>> from finstack_quant.statements_analytics import Stage
    >>> Stage.from_str("stage2").value
    'stage2'
    >>> Stage.Stage2 == Stage.from_str("stage2")
    True
    """

    Stage1: ClassVar[Stage]
    """Performing: 12-month ECL.
    """
    Stage2: ClassVar[Stage]
    """Significant increase in credit risk: lifetime ECL.
    """
    Stage3: ClassVar[Stage]
    """Credit-impaired: lifetime ECL with PD = 1.
    """
    @staticmethod
    def from_str(value: str) -> Stage:
        """
        Parse the serde name (``"stage1"``, ``"stage2"`` or ``"stage3"``).

        Parameters
        ----------
        value : str
            Serde stage name; one of ``"stage1"``, ``"stage2"`` or
            ``"stage3"``, matching the wire representation exactly.

        Returns
        -------
        Stage
            The impairment stage the name denotes.

        Raises
        ------
        ValueError
            If ``value`` is not one of the three serde names.

        Examples
        --------
        >>> from finstack_quant.statements_analytics import Stage
        >>> Stage.from_str("stage2").value
        'stage2'
        """
    @property
    def value(self) -> str:
        """
        Serde name used in JSON and accepted by ``compute_ecl(stage=...)``.

        This property does not raise.

        Returns
        -------
        str
            Serde name used in JSON and accepted by ``compute_ecl(stage=...)``.
        """

class StageResult:
    """Outcome of IFRS 9 stage classification with its trigger audit trail.

    Examples
    --------
    >>> from finstack_quant.statements_analytics import StageResult
    >>> StageResult.from_json('{"stage":"stage1","triggers":["no_trigger"],"cured":false}').stage.value
    'stage1'
    """
    @property
    def stage(self) -> Stage:
        """
        IFRS 9 impairment stage assigned to the exposure.

        This property does not raise.

        Returns
        -------
        Stage
            Stage 1, 2 or 3 as decided by the staging rules.
        """
    @property
    def triggers(self) -> list[str]:
        """
                Ordered trigger audit trail rendered by the canonical Rust display
        (``["no_trigger"]`` for a clean Stage 1).

                This property does not raise.

                Returns
                -------
                list[str]
                    Ordered trigger audit trail rendered by the canonical Rust display (``["no_trigger"]`` for a clean Stage 1).
        """
    @property
    def cured(self) -> bool:
        """
        Whether the exposure was cured down from a higher previous stage.

        This property does not raise.

        Returns
        -------
        bool
            Whether the exposure was cured down from a higher previous stage.
        """
    def to_json(self) -> str:
        """
        Serialize to canonical JSON.

        Returns
        -------
        str
            Canonical JSON encoding of this value, accepted by
            :meth:`from_json`.

        Raises
        ------
        ValueError
            If the value cannot be serialized to JSON.
        """
    @staticmethod
    def from_json(json: str) -> StageResult:
        """
        Deserialize from canonical JSON.

        Parameters
        ----------
        json : str
            JSON document produced by :meth:`to_json`, in this type's
            canonical wire schema.

        Returns
        -------
        StageResult
            The value decoded from the supplied JSON document.

        Raises
        ------
        ValueError
            If ``json`` is not a valid ``StageResult`` document.

        Examples
        --------
        >>> from finstack_quant.statements_analytics import StageResult
        >>> StageResult.from_json("{")
        Traceback (most recent call last):
        ValueError: ...
        """

class StagingConfig:
    """
    IFRS 9 staging policy: SICR thresholds, days-past-due backstops, qualitative
    switches and curing windows.

    Every parameter defaults to the canonical Rust ``StagingConfig::default()``
    value, so ``StagingConfig()`` is the standard policy.

    The constructor stores the supplied values and does not raise.

    Parameters
    ----------
    pd_delta_absolute : float | None
        Absolute lifetime-PD increase (decimal, ``0.01`` = 1pp) that fires the
        Stage 2 SICR trigger.
    pd_delta_relative : float | None
        Relative lifetime-PD multiple (``2.0`` = PD doubled) that fires the
        Stage 2 SICR trigger; ``inf`` disables it.
    rating_downgrade_notches : int | None
        Downgrade notches from origination that fire Stage 2; ``0`` disables
        the trigger.
    rating_scale_labels : list[str] | None
        Ordered best-to-worst rating labels used to count notches; ``None``
        uses the 10-state S&P/Fitch scale.
    dpd_stage2_threshold : int | None
        Days past due at or above which the Stage 2 backstop fires (default 30).
    dpd_stage3_threshold : int | None
        Days past due at or above which Stage 3 is forced (default 90).
    qualitative_triggers_enabled : bool | None
        Whether any active SICR flag fires Stage 2.
    stage3_qualitative_triggers_enabled : bool | None
        Whether default-evidence flags force Stage 3.
    cure_periods_stage2_to_1 : int | None
        Consecutive performing periods required to cure Stage 2 to Stage 1.
    cure_periods_stage3_to_2 : int | None
        Consecutive performing periods required to cure Stage 3 to Stage 2.

    Examples
    --------
    >>> from finstack_quant.statements_analytics import StagingConfig
    >>> StagingConfig(pd_delta_absolute=0.02).pd_delta_absolute
    0.02
    """
    def __init__(
        self,
        pd_delta_absolute: float | None = None,
        pd_delta_relative: float | None = None,
        rating_downgrade_notches: int | None = None,
        rating_scale_labels: list[str] | None = None,
        dpd_stage2_threshold: int | None = None,
        dpd_stage3_threshold: int | None = None,
        qualitative_triggers_enabled: bool | None = None,
        stage3_qualitative_triggers_enabled: bool | None = None,
        cure_periods_stage2_to_1: int | None = None,
        cure_periods_stage3_to_2: int | None = None,
    ) -> None: ...
    @property
    def pd_delta_absolute(self) -> float:
        """
        Absolute lifetime-PD increase (decimal) that fires Stage 2.

        This property does not raise.

        Returns
        -------
        float
            Absolute lifetime-PD increase (decimal) that fires Stage 2.
        """
    @property
    def pd_delta_relative(self) -> float:
        """
        Relative lifetime-PD multiple that fires Stage 2 (``inf`` = disabled).

        This property does not raise.

        Returns
        -------
        float
            Relative lifetime-PD multiple that fires Stage 2 (``inf`` = disabled).
        """
    @property
    def rating_downgrade_notches(self) -> int:
        """
        Downgrade notches that fire Stage 2 (``0`` = disabled).

        This property does not raise.

        Returns
        -------
        int
            Downgrade notches that fire Stage 2 (``0`` = disabled).
        """
    @property
    def rating_scale_labels(self) -> list[str] | None:
        """
        Ordered best-to-worst rating labels, or ``None`` for the default scale.

        This property does not raise.

        Returns
        -------
        list[str] | None
            Ordered best-to-worst rating labels, or ``None`` for the default scale.
        """
    @property
    def dpd_stage2_threshold(self) -> int:
        """
        Days past due at or above which the Stage 2 backstop fires.

        This property does not raise.

        Returns
        -------
        int
            Days past due at or above which the Stage 2 backstop fires.
        """
    @property
    def dpd_stage3_threshold(self) -> int:
        """
        Days past due at or above which Stage 3 is forced.

        This property does not raise.

        Returns
        -------
        int
            Days past due at or above which Stage 3 is forced.
        """
    @property
    def qualitative_triggers_enabled(self) -> bool:
        """
        Whether active SICR flags fire Stage 2.

        This property does not raise.

        Returns
        -------
        bool
            Whether active SICR flags fire Stage 2.
        """
    @property
    def stage3_qualitative_triggers_enabled(self) -> bool:
        """
        Whether default-evidence flags force Stage 3.

        This property does not raise.

        Returns
        -------
        bool
            Whether default-evidence flags force Stage 3.
        """
    @property
    def cure_periods_stage2_to_1(self) -> int:
        """
        Performing periods required to cure Stage 2 to Stage 1.

        This property does not raise.

        Returns
        -------
        int
            Performing periods required to cure Stage 2 to Stage 1.
        """
    @property
    def cure_periods_stage3_to_2(self) -> int:
        """
        Performing periods required to cure Stage 3 to Stage 2.

        This property does not raise.

        Returns
        -------
        int
            Performing periods required to cure Stage 3 to Stage 2.
        """
    def to_json(self) -> str:
        """
        Serialize to canonical JSON.

        Returns
        -------
        str
            Canonical JSON encoding of this value, accepted by
            :meth:`from_json`.

        Raises
        ------
        ValueError
            If the value cannot be serialized to JSON.
        """
    @staticmethod
    def from_json(json: str) -> StagingConfig:
        """
        Deserialize from canonical JSON (unknown fields are rejected).

        Parameters
        ----------
        json : str
            JSON document produced by :meth:`to_json`, in this type's
            canonical wire schema.

        Returns
        -------
        StagingConfig
            Deserialize from canonical JSON (unknown fields are rejected).

        Raises
        ------
        ValueError
            If ``json`` is not a valid ``StagingConfig`` document.

        Examples
        --------
        >>> from finstack_quant.statements_analytics import StagingConfig
        >>> StagingConfig.from_json("{")
        Traceback (most recent call last):
        ValueError: ...
        """

class TerminalValueSpec:
    """Terminal value method for a DCF.

    Build with one of the constructors; the wire form is the tagged serde enum
    (``{"type": "gordon_growth", "growth_rate": 0.02}``).

    Examples
    --------
    >>> from finstack_quant.statements_analytics import TerminalValueSpec
    >>> TerminalValueSpec.gordon_growth(0.02).kind
    'gordon_growth'
    >>> TerminalValueSpec.exit_multiple(9.0).params["multiple"]
    9.0
    """
    @staticmethod
    def gordon_growth(growth_rate: float) -> TerminalValueSpec:
        """
        Gordon growth: ``TV = FCF_terminal * (1 + g) / (WACC - g)``.

        Every input is stored verbatim, so this constructor does not raise.

        Parameters
        ----------
        growth_rate : float
            Perpetual growth rate ``g`` in decimal form (``0.02`` = 2%).
        Returns
        -------
        TerminalValueSpec
            Gordon growth: ``TV = FCF_terminal * (1 + g) / (WACC - g)``.

        Examples
        --------
        >>> from finstack_quant.statements_analytics import TerminalValueSpec
        >>> TerminalValueSpec.gordon_growth(0.02).kind
        'gordon_growth'
        """
    @staticmethod
    def exit_multiple(multiple: float, terminal_metric: float = 0.0) -> TerminalValueSpec:
        """
        Exit multiple: ``TV = terminal_metric * multiple``.

        Every input is stored verbatim, so this constructor does not raise.

        Parameters
        ----------
        multiple : float
            Exit multiple in turns (``9.0`` = 9.0x).
        terminal_metric : float
            Terminal-year metric (EBITDA, revenue, ...) in model currency.
            Default ``0.0``; pass ``exit_multiple_metric_node`` to
            ``evaluate_dcf`` to read it from the statement model instead.
        Returns
        -------
        TerminalValueSpec
            Exit multiple: ``TV = terminal_metric * multiple``.

        Examples
        --------
        >>> from finstack_quant.statements_analytics import TerminalValueSpec
        >>> TerminalValueSpec.exit_multiple(8.0).kind
        'exit_multiple'
        """
    @staticmethod
    def h_model(high_growth_rate: float, stable_growth_rate: float, half_life_years: float) -> TerminalValueSpec:
        """
        H-model: growth decays linearly from ``high_growth_rate`` to
        ``stable_growth_rate`` over ``2 * half_life_years``.

        Every input is stored verbatim, so this constructor does not raise.

        Parameters
        ----------
        high_growth_rate : float
            Initial growth rate in decimal form.
        stable_growth_rate : float
            Long-run growth rate in decimal form.
        half_life_years : float
            Half-life of the decay in years.
        Returns
        -------
        TerminalValueSpec
            H-model: growth decays linearly from ``high_growth_rate`` to ``stable_growth_rate`` over ``2 * half_life_years``.

        Examples
        --------
        >>> from finstack_quant.statements_analytics import TerminalValueSpec
        >>> TerminalValueSpec.h_model(0.08, 0.02, 5.0).kind
        'h_model'
        """
    @property
    def kind(self) -> str:
        """
        Serde tag: ``"gordon_growth"``, ``"exit_multiple"`` or ``"h_model"``.

        This property does not raise.

        Returns
        -------
        str
            Serde tag: ``"gordon_growth"``, ``"exit_multiple"`` or ``"h_model"``.
        """
    @property
    def params(self) -> Any:
        """
        Method parameters as a dict (the wire form without the ``type`` tag).

        This property does not raise.

        Returns
        -------
        Any
            Method parameters as a dict (the wire form without the ``type`` tag).
        """
    def to_json(self) -> str:
        """
        Serialize to canonical JSON.

        Returns
        -------
        str
            Canonical JSON encoding of this value, accepted by
            :meth:`from_json`.

        Raises
        ------
        ValueError
            If the value cannot be serialized to JSON.
        """
    @staticmethod
    def from_json(json: str) -> TerminalValueSpec:
        """
        Deserialize from canonical JSON.

        Parameters
        ----------
        json : str
            JSON document produced by :meth:`to_json`, in this type's
            canonical wire schema.

        Returns
        -------
        TerminalValueSpec
            The value decoded from the supplied JSON document.

        Raises
        ------
        ValueError
            If ``json`` is not a valid tagged ``TerminalValueSpec`` document.

        Examples
        --------
        >>> from finstack_quant.statements_analytics import TerminalValueSpec
        >>> TerminalValueSpec.from_json("{")
        Traceback (most recent call last):
        ValueError: ...
        """

class ThreeStatementMapping:
    """
    Node-id mapping for the three-statement check suite.

    Required ids name the balance-sheet articulation nodes; optional ids switch
    on the reconciliation checks that need them (depreciation, interest, tax,
    cash-flow subtotals, capex, dividends, working capital, debt roll-forwards).

    The constructor stores the supplied values and does not raise.

    Parameters
    ----------
    cash_node : str
        Cash balance node.
    retained_earnings_node : str
        Retained-earnings balance node.
    net_income_node : str
        Net-income node.
    assets_nodes : list[str]
        Nodes summed to total assets.
    liabilities_nodes : list[str]
        Nodes summed to total liabilities.
    equity_nodes : list[str]
        Nodes summed to total equity.
    ppe_node : str | None
        Net PP&E balance node.
    depreciation_node : str | None
        Depreciation-expense node.
    interest_expense_node : str | None
        Interest-expense node.
    tax_expense_node : str | None
        Tax-expense node.
    pretax_income_node : str | None
        Pre-tax income node.
    cfo_node : str | None
        Cash from operations node.
    cfi_node : str | None
        Cash from investing node.
    cff_node : str | None
        Cash from financing node.
    total_cf_node : str | None
        Net change in cash node.
    capex_node : str | None
        Capital-expenditure node.
    dividends_node : str | None
        Dividends-paid node.
    ppe_additions_node : str | None
        PP&E additions node (capex reconciliation).
    intangible_additions_node : str | None
        Intangible additions node (capex reconciliation).
    dividends_equity_node : str | None
        Dividends recorded in the equity roll-forward.
    debt_balance_nodes : list[tuple[str, str | None]]
        ``(balance_node, rate_node)`` pairs for interest reconciliation.
    cs_interest_node : str | None
        Capital-structure interest node.
    wc_change_cf_node : str | None
        Working-capital change node on the cash-flow statement.
    current_assets_nodes : list[str]
        Current-asset nodes for the working-capital check.
    current_liabilities_nodes : list[str]
        Current-liability nodes for the working-capital check.

    Examples
    --------
    >>> from finstack_quant.statements_analytics import ThreeStatementMapping
    >>> m = ThreeStatementMapping("cash", "retained_earnings", "net_income", ["cash"], [], ["retained_earnings"])
    >>> m.cash_node
    'cash'
    """
    def __init__(
        self,
        cash_node: str,
        retained_earnings_node: str,
        net_income_node: str,
        assets_nodes: list[str] = ...,
        liabilities_nodes: list[str] = ...,
        equity_nodes: list[str] = ...,
        ppe_node: str | None = None,
        depreciation_node: str | None = None,
        interest_expense_node: str | None = None,
        tax_expense_node: str | None = None,
        pretax_income_node: str | None = None,
        cfo_node: str | None = None,
        cfi_node: str | None = None,
        cff_node: str | None = None,
        total_cf_node: str | None = None,
        capex_node: str | None = None,
        dividends_node: str | None = None,
        ppe_additions_node: str | None = None,
        intangible_additions_node: str | None = None,
        dividends_equity_node: str | None = None,
        debt_balance_nodes: list[Any] = ...,
        cs_interest_node: str | None = None,
        wc_change_cf_node: str | None = None,
        current_assets_nodes: list[str] = ...,
        current_liabilities_nodes: list[str] = ...,
    ) -> None: ...
    @property
    def cash_node(self) -> str:
        """
        Cash balance node.

        This property does not raise.

        Returns
        -------
        str
            Cash balance node.
        """
    @property
    def retained_earnings_node(self) -> str:
        """
        Retained-earnings balance node.

        This property does not raise.

        Returns
        -------
        str
            Retained-earnings balance node.
        """
    @property
    def net_income_node(self) -> str:
        """
        Net-income node.

        This property does not raise.

        Returns
        -------
        str
            Net-income node.
        """
    @property
    def assets_nodes(self) -> list[str]:
        """
        Nodes summed to total assets.

        This property does not raise.

        Returns
        -------
        list[str]
            Nodes summed to total assets.
        """
    @property
    def liabilities_nodes(self) -> list[str]:
        """
        Nodes summed to total liabilities.

        This property does not raise.

        Returns
        -------
        list[str]
            Nodes summed to total liabilities.
        """
    @property
    def equity_nodes(self) -> list[str]:
        """
        Nodes summed to total equity.

        This property does not raise.

        Returns
        -------
        list[str]
            Nodes summed to total equity.
        """
    @property
    def ppe_node(self) -> str | None:
        """
        Net PP&E node, or ``None``.

        This property does not raise.

        Returns
        -------
        str | None
            Net PP&E node, or ``None``.
        """
    @property
    def depreciation_node(self) -> str | None:
        """
        Depreciation-expense node, or ``None``.

        This property does not raise.

        Returns
        -------
        str | None
            Depreciation-expense node, or ``None``.
        """
    @property
    def interest_expense_node(self) -> str | None:
        """
        Interest-expense node, or ``None``.

        This property does not raise.

        Returns
        -------
        str | None
            Interest-expense node, or ``None``.
        """
    @property
    def tax_expense_node(self) -> str | None:
        """
        Tax-expense node, or ``None``.

        This property does not raise.

        Returns
        -------
        str | None
            Tax-expense node, or ``None``.
        """
    @property
    def pretax_income_node(self) -> str | None:
        """
        Pre-tax income node, or ``None``.

        This property does not raise.

        Returns
        -------
        str | None
            Pre-tax income node, or ``None``.
        """
    @property
    def cfo_node(self) -> str | None:
        """
        Cash from operations node, or ``None``.

        This property does not raise.

        Returns
        -------
        str | None
            Cash from operations node, or ``None``.
        """
    @property
    def cfi_node(self) -> str | None:
        """
        Cash from investing node, or ``None``.

        This property does not raise.

        Returns
        -------
        str | None
            Cash from investing node, or ``None``.
        """
    @property
    def cff_node(self) -> str | None:
        """
        Cash from financing node, or ``None``.

        This property does not raise.

        Returns
        -------
        str | None
            Cash from financing node, or ``None``.
        """
    @property
    def total_cf_node(self) -> str | None:
        """
        Net change in cash node, or ``None``.

        This property does not raise.

        Returns
        -------
        str | None
            Net change in cash node, or ``None``.
        """
    @property
    def capex_node(self) -> str | None:
        """
        Capital-expenditure node, or ``None``.

        This property does not raise.

        Returns
        -------
        str | None
            Capital-expenditure node, or ``None``.
        """
    @property
    def dividends_node(self) -> str | None:
        """
        Dividends-paid node, or ``None``.

        This property does not raise.

        Returns
        -------
        str | None
            Dividends-paid node, or ``None``.
        """
    @property
    def ppe_additions_node(self) -> str | None:
        """
        PP&E additions node, or ``None``.

        This property does not raise.

        Returns
        -------
        str | None
            PP&E additions node, or ``None``.
        """
    @property
    def intangible_additions_node(self) -> str | None:
        """
        Intangible additions node, or ``None``.

        This property does not raise.

        Returns
        -------
        str | None
            Intangible additions node, or ``None``.
        """
    @property
    def dividends_equity_node(self) -> str | None:
        """
        Dividends node in the equity roll-forward, or ``None``.

        This property does not raise.

        Returns
        -------
        str | None
            Dividends node in the equity roll-forward, or ``None``.
        """
    @property
    def debt_balance_nodes(self) -> list[Any]:
        """
        ``(balance_node, rate_node)`` pairs for interest reconciliation.

        This property does not raise.

        Returns
        -------
        list[Any]
            ``(balance_node, rate_node)`` pairs for interest reconciliation.
        """
    @property
    def cs_interest_node(self) -> str | None:
        """
        Capital-structure interest node, or ``None``.

        This property does not raise.

        Returns
        -------
        str | None
            Capital-structure interest node, or ``None``.
        """
    @property
    def wc_change_cf_node(self) -> str | None:
        """
        Working-capital change node on the cash-flow statement, or ``None``.

        This property does not raise.

        Returns
        -------
        str | None
            Working-capital change node on the cash-flow statement, or ``None``.
        """
    @property
    def current_assets_nodes(self) -> list[str]:
        """
        Current-asset nodes.

        This property does not raise.

        Returns
        -------
        list[str]
            Current-asset nodes.
        """
    @property
    def current_liabilities_nodes(self) -> list[str]:
        """
        Current-liability nodes.

        This property does not raise.

        Returns
        -------
        list[str]
            Current-liability nodes.
        """
    @property
    def all_nodes(self) -> list[str]:
        """
        Every node id referenced by the mapping.

        This property does not raise.

        Returns
        -------
        list[str]
            Every node id referenced by the mapping.
        """
    def to_json(self) -> str:
        """
        Serialize to canonical JSON.

        Returns
        -------
        str
            Canonical JSON encoding of this value, accepted by
            :meth:`from_json`.

        Raises
        ------
        ValueError
            If the value cannot be serialized to JSON.
        """
    @staticmethod
    def from_json(json: str) -> ThreeStatementMapping:
        """
        Deserialize from canonical JSON.

        Parameters
        ----------
        json : str
            JSON document produced by :meth:`to_json`, in this type's
            canonical wire schema.

        Returns
        -------
        ThreeStatementMapping
            The value decoded from the supplied JSON document.

        Raises
        ------
        ValueError
            If ``json`` is not a valid ``ThreeStatementMapping`` document.

        Examples
        --------
        >>> from finstack_quant.statements_analytics import ThreeStatementMapping
        >>> ThreeStatementMapping.from_json("{")
        Traceback (most recent call last):
        ValueError: ...
        """

class TornadoEntry:
    """One parameter's downside and upside impact in a tornado chart.

    Examples
    --------
    >>> from finstack_quant.statements_analytics import TornadoEntry
    >>> entry = TornadoEntry.from_json('{"parameter_id":"revenue","downside":-5.0,"upside":7.0}')
    >>> entry.swing
    12.0
    """
    @staticmethod
    def from_json(json: str) -> TornadoEntry:
        """
        Deserialize one tornado entry from canonical JSON.

        Every input is stored verbatim, so this constructor does not raise.

        Parameters
        ----------
        json : str
            JSON document produced by :meth:`to_json`, in this type's
            canonical wire schema.

        Returns
        -------
        TornadoEntry
            Deserialize one tornado entry from canonical JSON.

        Examples
        --------
        >>> from finstack_quant.statements_analytics import TornadoEntry
        >>> TornadoEntry.from_json("{")
        Traceback (most recent call last):
        ValueError: ...
        """
    def to_json(self) -> str:
        """
        Serialize this entry to canonical JSON.

        Returns
        -------
        str
            Serialize this entry to canonical JSON.

        Raises
        ------
        ValueError
            If the value cannot be serialized to JSON.
        """
    @property
    def parameter_id(self) -> str:
        """
        Parameter node identifier represented by this entry.

        This property does not raise.

        Returns
        -------
        str
            Parameter node identifier represented by this entry.
        """
    @property
    def downside(self) -> float:
        """
        Metric change at the parameter's minimum perturbation.

        This property does not raise.

        Returns
        -------
        float
            Metric change at the parameter's minimum perturbation.
        """
    @property
    def upside(self) -> float:
        """
        Metric change at the parameter's maximum perturbation.

        This property does not raise.

        Returns
        -------
        float
            Metric change at the parameter's maximum perturbation.
        """
    @property
    def swing(self) -> float:
        """
        Total swing magnitude, calculated as `upside - downside`.

        This property does not raise.

        Returns
        -------
        float
            Total swing magnitude, calculated as `upside - downside`.
        """

class ValuationDiscounts:
    """
    Equity-level valuation discounts (DLOM, DLOC, other).

    Each discount is a decimal fraction in ``[0, 1]`` applied multiplicatively
    to the pre-discount equity value.

    Parameters
    ----------
    dlom : float | None
        Discount for lack of marketability.
    dloc : float | None
        Discount for lack of control.
    other_discount : float | None
        Any additional discount.

    Raises
    ------
    ValueError
        If a discount is outside its admissible range.

    Examples
    --------
    >>> from finstack_quant.statements_analytics import ValuationDiscounts
    >>> ValuationDiscounts(dlom=0.25).dlom
    0.25
    """
    def __init__(
        self, dlom: float | None = None, dloc: float | None = None, other_discount: float | None = None
    ) -> None: ...
    @property
    def dlom(self) -> float | None:
        """
        Discount for lack of marketability, or ``None``.

        This property does not raise.

        Returns
        -------
        float | None
            Discount for lack of marketability, or ``None``.
        """
    @property
    def dloc(self) -> float | None:
        """
        Discount for lack of control, or ``None``.

        This property does not raise.

        Returns
        -------
        float | None
            Discount for lack of control, or ``None``.
        """
    @property
    def other_discount(self) -> float | None:
        """
        Additional discount, or ``None``.

        This property does not raise.

        Returns
        -------
        float | None
            Additional discount, or ``None``.
        """
    def to_json(self) -> str:
        """
        Serialize to canonical JSON.

        Returns
        -------
        str
            Canonical JSON encoding of this value, accepted by
            :meth:`from_json`.

        Raises
        ------
        ValueError
            If the value cannot be serialized to JSON.
        """
    @staticmethod
    def from_json(json: str) -> ValuationDiscounts:
        """
        Deserialize from canonical JSON (unknown fields are rejected).

        Parameters
        ----------
        json : str
            JSON document produced by :meth:`to_json`, in this type's
            canonical wire schema.

        Returns
        -------
        ValuationDiscounts
            Deserialize from canonical JSON (unknown fields are rejected).

        Raises
        ------
        ValueError
            If ``json`` is not a valid ``ValuationDiscounts`` document.

        Examples
        --------
        >>> from finstack_quant.statements_analytics import ValuationDiscounts
        >>> ValuationDiscounts.from_json("{")
        Traceback (most recent call last):
        ValueError: ...
        """

class VarianceConfig:
    """
    Configuration for comparing two statement results.

    The constructor stores the supplied values and does not raise.

    Parameters
    ----------
    baseline_label : str
        Label reported for the baseline side of the comparison.
    comparison_label : str
        Label reported for the comparison side.
    metrics : list[str]
        Statement node ids compared, in report order.
    periods : list[str]
        Period ids (for example ``"2025Q1"``) the comparison covers.

    Examples
    --------
    >>> from finstack_quant.statements_analytics import VarianceConfig
    >>> VarianceConfig.from_json("{")
    Traceback (most recent call last):
    ValueError: ...
    """
    def __init__(self, baseline_label: str, comparison_label: str, metrics: list[str], periods: list[str]) -> None: ...
    @staticmethod
    def from_json(json: str) -> VarianceConfig:
        """
        Rebuild a ``VarianceConfig`` from its canonical JSON form.

        Parameters
        ----------
        json : str
            JSON document produced by :meth:`to_json`, in this type's
            canonical wire schema.

        Returns
        -------
        VarianceConfig
            The value decoded from the supplied JSON document.

        Raises
        ------
        ValueError
            If ``json`` is malformed or is not a valid ``VarianceConfig`` document.

        Examples
        --------
        >>> from finstack_quant.statements_analytics import VarianceConfig
        >>> VarianceConfig.from_json("{")
        Traceback (most recent call last):
        ValueError: ...
        """
    def to_json(self) -> str:
        """
        Serialize to canonical JSON.

        Returns
        -------
        str
            Canonical JSON encoding of this ``VarianceConfig``, accepted by
            :meth:`from_json`.

        Raises
        ------
        ValueError
            If the value cannot be serialized to JSON.
        """
    @property
    def baseline_label(self) -> str:
        """
        Label for the baseline scenario (e.g. ``"management_case"``).

        This property does not raise.

        Returns
        -------
        str
            Label for the baseline scenario (e.g. ``"management_case"``).
        """
    @property
    def comparison_label(self) -> str:
        """
        Label for the comparison scenario (e.g. ``"bank_case"``).

        This property does not raise.

        Returns
        -------
        str
            Label for the comparison scenario (e.g. ``"bank_case"``).
        """
    @property
    def metrics(self) -> list[str]:
        """
        Node identifiers of the metrics compared between the two scenarios.

        This property does not raise.

        Returns
        -------
        list[str]
            Node identifiers of the metrics compared between the two scenarios.
        """
    @property
    def periods(self) -> list[str]:
        """
        Periods to compare, as period-id strings (e.g. ``"2025Q1"``).

        This property does not raise.

        Returns
        -------
        list[str]
            Periods to compare, as period-id strings (e.g. ``"2025Q1"``).
        """

class VarianceReport:
    """
    Typed root variance report.

    Examples
    --------
    >>> from finstack_quant.statements_analytics import VarianceReport
    >>> VarianceReport.from_json("{")
    Traceback (most recent call last):
    ValueError: ...
    """
    @staticmethod
    def from_json(json: str) -> VarianceReport:
        """
        Rebuild a ``VarianceReport`` from its canonical JSON form.

        Parameters
        ----------
        json : str
            JSON document produced by :meth:`to_json`, in this type's
            canonical wire schema.

        Returns
        -------
        VarianceReport
            The value decoded from the supplied JSON document.

        Raises
        ------
        ValueError
            If ``json`` is malformed or is not a valid ``VarianceReport`` document.

        Examples
        --------
        >>> from finstack_quant.statements_analytics import VarianceReport
        >>> VarianceReport.from_json("{")
        Traceback (most recent call last):
        ValueError: ...
        """
    def to_json(self) -> str:
        """
        Serialize to canonical JSON.

        Returns
        -------
        str
            Canonical JSON encoding of this ``VarianceReport``, accepted by
            :meth:`from_json`.

        Raises
        ------
        ValueError
            If the value cannot be serialized to JSON.
        """
    @property
    def baseline_label(self) -> str:
        """
        Label for the baseline scenario (e.g. ``"management_case"``).

        This property does not raise.

        Returns
        -------
        str
            Label for the baseline scenario (e.g. ``"management_case"``).
        """
    @property
    def comparison_label(self) -> str:
        """
        Label for the comparison scenario (e.g. ``"bank_case"``).

        This property does not raise.

        Returns
        -------
        str
            Label for the comparison scenario (e.g. ``"bank_case"``).
        """
    @property
    def rows(self) -> list[VarianceRow]:
        """
        Per-metric, per-period variance rows, in report order.

        This property does not raise.

        Returns
        -------
        list[VarianceRow]
            Per-metric, per-period variance rows, in report order.
        """
    def to_dataframe(self) -> pd.DataFrame:
        """
        Export the variance rows as a pandas ``DataFrame``.

        Columns: ``period``, ``metric``, ``baseline``, ``comparison``,
        ``abs_var``, ``pct_var``, ``driver_contribution``. One row per
        (metric, period) pair, in report order; an empty report still carries
        the full column schema.

        ``baseline``, ``comparison`` and ``abs_var`` are in the metric's own
        units; ``pct_var`` is a decimal fraction (``0.1`` = +10%) and is
        ``NaN`` where the baseline is effectively zero;
        ``driver_contribution`` is the ``{driver: contribution}`` dict of
        ``VarianceRow.driver_contribution`` (an object column, empty dict when
        no drivers were declared). The scenario labels are report metadata
        (``baseline_label`` / ``comparison_label``) and are not repeated per row.

        Returns
        -------
        pd.DataFrame
            Export the variance rows as a pandas ``DataFrame``. Columns: ``period``, ``metric``, ``baseline``, ``comparison``, ``abs_var``, ``pct_var``, ``driver_contribution``. One row per (metric, period) pair, in report order; an empty report still carries the full column schema. ``baseline``, ``comparison`` and ``abs_var`` are in the metric's own units; ``pct_var`` is a decimal fraction (``0.1`` = +10%) and is ``NaN`` where the baseline is effectively zero; ``driver_contribution`` is the ``{driver: contribution}`` dict of ``VarianceRow.driver_contribution`` (an object column, empty dict when no drivers were declared). The scenario labels are report metadata (``baseline_label`` / ``comparison_label``) and are not repeated per row.

        Raises
        ------
        ImportError
            If pandas is not installed in the running interpreter.
        ValueError
            If the result cannot be converted to its wire form.
        """

class VarianceRow:
    """
    One typed variance-report row.

    Examples
    --------
    >>> from finstack_quant.statements_analytics import VarianceRow
    >>> [field for field in ("period", "metric") if hasattr(VarianceRow, field)]
    ['period', 'metric']
    """
    @property
    def period(self) -> str:
        """
        Period this row covers, as a period-id string (e.g. ``"2025Q1"``).

        This property does not raise.

        Returns
        -------
        str
            Period this row covers, as a period-id string (e.g. ``"2025Q1"``).
        """
    @property
    def metric(self) -> str:
        """
        Node identifier of the compared metric.

        This property does not raise.

        Returns
        -------
        str
            Node identifier of the compared metric.
        """
    @property
    def baseline(self) -> float:
        """
        Metric value in the baseline scenario, in the metric's own units.

        This property does not raise.

        Returns
        -------
        float
            Metric value in the baseline scenario, in the metric's own units.
        """
    @property
    def comparison(self) -> float:
        """
        Metric value in the comparison scenario, in the metric's own units.

        This property does not raise.

        Returns
        -------
        float
            Metric value in the comparison scenario, in the metric's own units.
        """
    @property
    def abs_var(self) -> float:
        """
        Absolute variance ``comparison - baseline``, in the metric's units.

        This property does not raise.

        Returns
        -------
        float
            Absolute variance ``comparison - baseline``, in the metric's units.
        """
    @property
    def pct_var(self) -> float | None:
        """
                Percentage variance ``abs_var / baseline`` as a decimal fraction
        (``0.1`` = +10%).

        ``None`` when the baseline is effectively zero, where a ratio would be
        undefined rather than zero; fall back to ``abs_var`` in that case.

                This property does not raise.

                Returns
                -------
                float | None
                    Percentage variance ``abs_var / baseline`` as a decimal fraction (``0.1`` = +10%). ``None`` when the baseline is effectively zero, where a ratio would be undefined rather than zero; fall back to ``abs_var`` in that case.
        """
    @property
    def driver_contribution(self) -> Any:
        """
                Driver attribution ``{driver_node: contribution}`` computed by the
        variance analyzer, in the driver's own units; empty when the config
        declared no drivers.

                This property does not raise.

                Returns
                -------
                Any
                    Driver attribution ``{driver_node: contribution}`` computed by the variance analyzer, in the driver's own units; empty when the config declared no drivers.
        """

class WeightedEclResult:
    """Probability-weighted ECL across macro scenarios with the per-scenario
    audit trail.

    Examples
    --------
    >>> from finstack_quant.statements_analytics import Exposure, compute_ecl
    >>> exp = Exposure("loan", 1_000_000.0, 0.45, 0.06, 3.0, 0.02, 0.015)
    >>> result = compute_ecl(exp, [(1.0, 0.02), (3.0, 0.06)], stage="stage2")
    >>> result.stage.value
    'stage2'
    >>> list(result.to_dataframe().columns)[:2]
    ['scenario', 'weight']
    """
    @property
    def exposure_id(self) -> str:
        """
        Exposure identifier.

        This property does not raise.

        Returns
        -------
        str
            Exposure identifier.
        """
    @property
    def stage(self) -> Stage:
        """
        Stage used for the measurement horizon.

        This property does not raise.

        Returns
        -------
        Stage
            Stage used for the measurement horizon.
        """
    @property
    def ecl(self) -> float:
        """
        Probability-weighted ECL in the exposure's base currency.

        This property does not raise.

        Returns
        -------
        float
            Probability-weighted ECL in the exposure's base currency.
        """
    @property
    def scenario_breakdown(self) -> list[tuple[str, float, EclResult]]:
        """
        Per-scenario ``(scenario_id, weight, EclResult)`` triples.

        This property does not raise.

        Returns
        -------
        list[tuple[str, float, EclResult]]
            Per-scenario ``(scenario_id, weight, EclResult)`` triples.
        """
    @property
    def meta(self) -> Any:
        """
        Result metadata (numeric mode, rounding context) as a dict.

        This property does not raise.

        Returns
        -------
        Any
            Result metadata (numeric mode, rounding context) as a dict.
        """
    def to_dataframe(self) -> pd.DataFrame:
        """
        Export the scenario x bucket audit trail as a pandas ``DataFrame``.

        Columns: ``scenario`` (scenario id), ``weight`` (decimal probability),
        ``t_start``, ``t_end`` (years), ``marginal_pd``, ``lgd`` (decimals),
        ``ead``, ``ecl`` (base currency, unweighted per scenario),
        ``discount_factor``. One row per (scenario, bucket).

        Returns
        -------
        pd.DataFrame
            Export the scenario x bucket audit trail as a pandas ``DataFrame``. Columns: ``scenario`` (scenario id), ``weight`` (decimal probability), ``t_start``, ``t_end`` (years), ``marginal_pd``, ``lgd`` (decimals), ``ead``, ``ecl`` (base currency, unweighted per scenario), ``discount_factor``. One row per (scenario, bucket).

        Raises
        ------
        ImportError
            If pandas is not installed in the running interpreter.
        ValueError
            If the result cannot be converted to its wire form.
        """
    def to_json(self) -> str:
        """
        Serialize to canonical JSON.

        Returns
        -------
        str
            Canonical JSON encoding of this value, accepted by
            :meth:`from_json`.

        Raises
        ------
        ValueError
            If the value cannot be serialized to JSON.
        """
    @staticmethod
    def from_json(json: str) -> WeightedEclResult:
        """
        Deserialize from canonical JSON.

        Parameters
        ----------
        json : str
            JSON document produced by :meth:`to_json`, in this type's
            canonical wire schema.

        Returns
        -------
        WeightedEclResult
            The value decoded from the supplied JSON document.

        Raises
        ------
        ValueError
            If ``json`` is not a valid ``WeightedEclResult`` document.

        Examples
        --------
        >>> from finstack_quant.statements_analytics import WeightedEclResult
        >>> WeightedEclResult.from_json("{")
        Traceback (most recent call last):
        ValueError: ...
        """

def backtest_forecast(actual: list[float], forecast: list[float]) -> ForecastMetrics:
    """Compute forecast accuracy metrics (MAE, MAPE, sMAPE, RMSE).

    Parameters
    ----------
    actual : list[float]
        Observed values.
    forecast : list[float]
        Forecast values; same length as ``actual``.

    Returns
    -------
    ForecastMetrics
        Typed metrics with ``summary()`` and ``to_series()``.

    Raises
    ------
    ValueError
        If the sequences are empty or of different lengths.

    Examples
    --------
    >>> from finstack_quant.statements_analytics import backtest_forecast
    >>> backtest_forecast([1.0, 2.0], [1.0, 2.5]).n
    2
    """

def classify_stage(exposure: Exposure, config: StagingConfig | None = None) -> StageResult:
    """Classify an exposure into an IFRS 9 stage.

    Runs the full Rust staging waterfall: the Stage 3 days-past-due backstop,
    default-evidence flags, the absolute and relative PD-delta SICR tests
    (``current_pd`` versus ``origination_pd``), the rating-downgrade notch test,
    qualitative SICR flags, the Stage 2 days-past-due backstop and curing.

    Parameters
    ----------
    exposure : Exposure
        The credit exposure; ``ead``, ``lgd`` and ``eir`` do not affect staging.
    config : StagingConfig | None
        Staging policy. ``None`` uses the canonical Rust defaults
        (1pp absolute PD delta, 30/90 DPD backstops, qualitative triggers on).

    Returns
    -------
    StageResult
        Assigned ``stage`` plus the ordered ``triggers`` audit trail and the
        ``cured`` flag.

    Raises
    ------
    ValueError
        If PDs are outside [0, 1], or maturity or staging thresholds are invalid.

    Examples
    --------
    >>> from finstack_quant.statements_analytics import Exposure, classify_stage
    >>> exp = Exposure("loan", 1_000_000.0, 0.45, 0.06, 3.0, 0.02, 0.015, dpd=35)
    >>> classify_stage(exp).stage.value
    'stage2'
    """

def compute_ecl(
    exposure: Exposure,
    pd_schedule: list[tuple[float, float]],
    stage: Any | None = None,
    bucket_width_years: float | None = None,
    stage3_time_to_recovery_years: float | None = None,
) -> WeightedEclResult:
    """Compute single-scenario ECL for one exposure.

    The priced exposure at default is ``ead + undrawn * ccf``; ``lgd``, ``eir``,
    ``remaining_maturity`` and any ``ead_schedule`` are read from the exposure.

    Parameters
    ----------
    exposure : Exposure
        The credit exposure to measure.
    pd_schedule : list[tuple[float, float]]
        Cumulative PD curve as ``[(time_years, cumulative_pd), ...]``, ascending
        in time and non-decreasing in PD. A ``(0.0, 0.0)`` knot is inserted
        when absent.
    stage : Stage | str | None
        Measurement stage (``Stage`` or serde name ``"stage1"``/``"stage2"``/
        ``"stage3"``). ``None`` classifies the exposure first with the default
        ``StagingConfig``.
    bucket_width_years : float | None
        Finite integration width of at least 0.0001 years (``0.25`` = quarterly); ``None`` uses
        the canonical policy default.
    stage3_time_to_recovery_years : float | None
        Stage 3 discounting horizon to expected recovery, in years; ``None``
        uses the canonical policy default.

    Returns
    -------
    WeightedEclResult
        ``ecl`` in base currency, the ``stage`` used, and a one-scenario
        ``scenario_breakdown`` with bucket detail (``to_dataframe()``).

    Raises
    ------
    ValueError
        If ``stage`` is unknown, the PD or EAD schedule is invalid, or an
        exposure input is outside its accepted range.

    Examples
    --------
    >>> from finstack_quant.statements_analytics import Exposure, compute_ecl
    >>> exp = Exposure("loan", 1_000_000.0, 0.45, 0.06, 3.0, 0.02, 0.015)
    >>> compute_ecl(exp, [(1.0, 0.02), (3.0, 0.06)], stage="stage1").ecl > 0
    True
    """

def compute_ecl_weighted(
    exposure: Exposure,
    scenarios: list[tuple[float, list[tuple[float, float]]]],
    stage: Any | None = None,
    bucket_width_years: float | None = None,
    stage3_time_to_recovery_years: float | None = None,
) -> WeightedEclResult:
    """Compute probability-weighted ECL across macro scenarios.

    Parameters
    ----------
    exposure : Exposure
        The credit exposure to measure (EAD is ``ead + undrawn * ccf``).
    scenarios : list[tuple[float, list[tuple[float, float]]]]
        ``(weight, pd_schedule)`` pairs; weights must sum to ``1.0`` and each
        schedule follows the ``compute_ecl`` conventions.
    stage : Stage | str | None
        Measurement stage; ``None`` classifies the exposure first with the
        default ``StagingConfig``.
    bucket_width_years : float | None
        Finite integration width of at least 0.0001 years; ``None`` uses the canonical default.
    stage3_time_to_recovery_years : float | None
        Stage 3 discounting horizon in years; ``None`` uses the canonical default.

    Returns
    -------
    WeightedEclResult
        Probability-weighted ``ecl`` with one ``scenario_breakdown`` entry per
        scenario.

    Raises
    ------
    ValueError
        If ``scenarios`` is empty, weights do not sum to ``1.0``, ``stage`` is
        unknown, a schedule is invalid, or an exposure input is out of range.

    Examples
    --------
    >>> from finstack_quant.statements_analytics import Exposure, compute_ecl_weighted
    >>> exp = Exposure("loan", 1_000_000.0, 0.45, 0.06, 1.0, 0.02, 0.015)
    >>> scenarios = [(0.7, [(1.0, 0.02)]), (0.3, [(1.0, 0.05)])]
    >>> len(compute_ecl_weighted(exp, scenarios, stage="stage1").scenario_breakdown)
    2
    """

def compute_multiple(company_metrics: Any, multiple: str) -> float | None:
    """Compute a canonical valuation multiple for one company.

    Parameters
    ----------
    company_metrics : CompanyMetrics | dict[str, float]
        Typed metrics or a flat ``{metric_name: value}`` dict; only the fields
        the multiple needs must be populated.
    multiple : str
        Serde name of the multiple: ``ev_ebitda``, ``ev_revenue``, ``ev_ebit``,
        ``ev_fcf``, ``pe``, ``pb``, ``ptbv``, ``p_fcf``, ``dividend_yield``,
        ``spread_per_turn`` or ``yield_per_coverage``.

    Returns
    -------
    float | None
        Multiple value, or ``None`` when a required input is missing or the
        denominator is not positive.

    Raises
    ------
    ValueError
        If ``multiple`` is not a known name or a metric value is not numeric.

    Examples
    --------
    >>> from finstack_quant.statements_analytics import compute_multiple
    >>> compute_multiple({"enterprise_value": 8_500.0, "ebitda": 1_000.0}, "ev_ebitda")
    8.5
    """

def credit_assessment(results: Any, period: str) -> CreditAssessment:
    """Compute a structured credit assessment (leverage, interest coverage, FCF).

    Parameters
    ----------
    results : StatementResult | str
        A ``StatementResult`` object or a JSON string with ``ebitda``,
        ``total_debt``, ``interest_expense`` and ``free_cash_flow`` nodes.
    period : str
        Period identifier for the assessment (``"2025Q4"``, ``"2025M03"``,
        ``"FY2025"``). This is a ``PeriodId``, not a date.

    Returns
    -------
    CreditAssessment
        Point-in-time ratios at ``period`` plus the ascending ``series``.

    Raises
    ------
    ValueError
        If ``period`` is not a valid period identifier or ``results`` is
        malformed JSON.

    Examples
    --------
    >>> from finstack_quant.statements import ModelBuilder, Evaluator
    >>> from finstack_quant.statements_analytics import credit_assessment
    >>> b = ModelBuilder("m")
    >>> _ = b.periods("2025Q1..Q4", None)
    >>> _ = b.value("ebitda", [(f"2025Q{q}", 25.0) for q in range(1, 5)])
    >>> _ = b.value("total_debt", [(f"2025Q{q}", 300.0) for q in range(1, 5)])
    >>> credit_assessment(Evaluator().evaluate(b.build()), "2025Q4").leverage_ratio
    3.0
    """

def credit_assessment_report_text(results: Any, period: str) -> str:
    """Generate a credit assessment report as formatted text.

    Parameters
    ----------
    results : StatementResult | str
        A ``StatementResult`` object or a JSON string.
    period : str
        Period identifier for the assessment (a ``PeriodId``, not a date).

    Returns
    -------
    str
        Formatted credit assessment report text.

    Raises
    ------
    ValueError
        If ``period`` is not a valid period identifier or ``results`` is
        malformed JSON.

    Examples
    --------
    >>> from finstack_quant.statements import ModelBuilder, Evaluator
    >>> from finstack_quant.statements_analytics import credit_assessment_report_text
    >>> b = ModelBuilder("m")
    >>> _ = b.periods("2025Q1..Q2", None)
    >>> _ = b.value("revenue", [("2025Q1", 1.0), ("2025Q2", 2.0)])
    >>> credit_assessment_report_text(Evaluator().evaluate(b.build()), "2025Q2").startswith("Credit Assessment")
    True
    """

def dcf_sensitivity(
    model: Any,
    wacc: float,
    terminal_value: Any,
    ufcf_node: str = "ufcf",
    net_debt_override: float | None = None,
    wacc_sensitivity_bump: float | None = None,
    wacc_denominator_epsilon: float | None = None,
    max_stable_growth_rate: float | None = None,
    exit_multiple_bump: float | None = None,
    mid_year_convention: bool = False,
    market: Any | None = None,
    exit_multiple_metric_node: str | None = None,
) -> DcfSensitivityResult:
    """Rank the headline DCF assumptions by enterprise-value impact.

    The statement model is evaluated once; each shocked point re-runs only the
    DCF. Entries are EV deltas versus the baseline, sorted by descending
    absolute swing.

    Parameters
    ----------
    model : FinancialModelSpec | str
        A ``FinancialModelSpec`` object or a JSON string; metadata must include
        a ``"currency"`` key.
    wacc : float
        Baseline WACC in decimal form.
    terminal_value : TerminalValueSpec | dict | str
        Terminal value method; selects whether the growth rate or the exit
        multiple is shocked.
    ufcf_node : str
        Node id containing unlevered free cash flow. Default ``"ufcf"``.
    net_debt_override : float | None
        Flat net-debt amount in model currency. Without an override, debt and
        cash must be monetary in model currency and available at a period
        ending on or before the inclusive valuation date; future balances are
        rejected.
    wacc_sensitivity_bump : float | None
        Absolute shock to WACC and terminal growth in decimal (``0.01`` =
        +/-100bp); ``None`` uses the Rust ``DcfOptions`` default.
    wacc_denominator_epsilon : float | None
        Minimum ``wacc - g`` spread preserved, in decimal; ``None`` uses the
        Rust default.
    max_stable_growth_rate : float | None
        Maximum perpetual growth (decimal); ``None`` uses the 5% default.
    exit_multiple_bump : float | None
        Absolute exit-multiple shock in turns; ``None`` uses the Rust default.
    mid_year_convention : bool
        Mid-year discounting for every re-run. Default ``False``.
    market : MarketContext | str | None
        Market context for statement evaluation (not WACC discounting).
    exit_multiple_metric_node : str | None
        Statement monetary flow node supplying the complete trailing-year terminal metric; insufficient or noncontiguous history raises ValueError.

    Returns
    -------
    DcfSensitivityResult
        Baseline EV, ranked ``entries`` and the clamped-shock flags;
        ``to_dataframe()`` gives the tornado table.

    Raises
    ------
    ValueError
        If a payload is malformed or the model or DCF inputs are invalid.
    KeyError
        If ``ufcf_node`` or ``exit_multiple_metric_node`` is missing.

    Examples
    --------
    >>> from finstack_quant.core.money import Money
    >>> from finstack_quant.statements import ModelBuilder
    >>> from finstack_quant.statements_analytics import TerminalValueSpec, dcf_sensitivity
    >>> b = ModelBuilder("dcf")
    >>> _ = b.periods("2025..2026")
    >>> _ = b.value_money("ufcf", [("2025", Money(100.0, "USD")), ("2026", Money(110.0, "USD"))])
    >>> _ = b.with_meta("currency", '"USD"')
    >>> sens = dcf_sensitivity(b.build(), 0.10, TerminalValueSpec.gordon_growth(0.02), net_debt_override=0.0)
    >>> list(sens.to_dataframe().columns)
    ['parameter_id', 'downside', 'upside', 'swing']
    """

def evaluate_dcf(
    model: Any,
    wacc: float,
    terminal_value: Any,
    ufcf_node: str = "ufcf",
    net_debt_override: float | None = None,
    mid_year_convention: bool = False,
    max_stable_growth_rate: float | None = None,
    shares_outstanding: float | None = None,
    equity_bridge: Any | None = None,
    valuation_discounts: Any | None = None,
    market: Any | None = None,
    as_of: Any | None = None,
    exit_multiple_metric_node: str | None = None,
) -> CorporateValuationResult:
    """Evaluate DCF valuation on a financial model.

    Parameters
    ----------
    model : FinancialModelSpec | str
        A ``FinancialModelSpec`` object or a JSON string; metadata must contain
        a ``"currency"`` key.
    wacc : float
        Weighted average cost of capital in decimal form (``0.10`` = 10%).
    terminal_value : TerminalValueSpec | dict | str
        Terminal value method (typed, serde dict, or tagged JSON such as
        ``{"type": "gordon_growth", "growth_rate": 0.02}``).
    ufcf_node : str
        Node id containing unlevered free cash flow. Default ``"ufcf"``.
    net_debt_override : float | None
        Flat net-debt amount in model currency. Without an override, debt and
        cash must be monetary in model currency and available at a period
        ending on or before the inclusive valuation date; future balances are
        rejected.
    mid_year_convention : bool
        Mid-year discounting. Default ``False`` (year-end).
    max_stable_growth_rate : float | None
        Maximum perpetual growth accepted for Gordon Growth / H-Model
        (decimal); ``None`` uses the canonical 5% default.
    shares_outstanding : float | None
        Basic shares outstanding for per-share equity value.
    equity_bridge : EquityBridge | dict | str | None
        Structured EV-to-equity bridge.
    valuation_discounts : ValuationDiscounts | dict | str | None
        DLOM / DLOC / other discounts.
    market : MarketContext | str | None
        Market context used for statement evaluation (capital-structure curve
        lookups); DCF discounting stays WACC-only. Requires ``as_of``.
    as_of : datetime.date | str | None
        DCF valuation date and, with ``market``, the statement visibility and
        market-data date. Defaults to the first forecast boundary.
    exit_multiple_metric_node : str | None
        Statement monetary flow node whose complete trailing-year sum replaces
        ``terminal_metric`` on an exit-multiple terminal. Actual period boundaries must cover one complete contiguous calendar year; otherwise omit this node and supply an explicit annual metric.

    Returns
    -------
    CorporateValuationResult
        ``equity_value``, ``enterprise_value``, ``net_debt`` and
        ``terminal_value_pv`` as ``Money``; per-share values as floats.

    Raises
    ------
    ValueError
        If ``market`` is set without ``as_of``, a payload is malformed, or the
        model, cash-flow node, exit-multiple node or DCF inputs are invalid.
    KeyError
        If ``ufcf_node`` or ``exit_multiple_metric_node`` is missing from the model.

    Examples
    --------
    >>> from finstack_quant.core.money import Money
    >>> from finstack_quant.statements import ModelBuilder
    >>> from finstack_quant.statements_analytics import TerminalValueSpec, evaluate_dcf
    >>> b = ModelBuilder("dcf")
    >>> _ = b.periods("2025..2026")
    >>> _ = b.value_money("ufcf", [("2025", Money(100.0, "USD")), ("2026", Money(110.0, "USD"))])
    >>> _ = b.with_meta("currency", '"USD"')
    >>> result = evaluate_dcf(b.build(), 0.10, TerminalValueSpec.gordon_growth(0.02), net_debt_override=0.0)
    >>> result.enterprise_value.currency.code
    'USD'
    """

def evaluate_lbo(
    model: Any,
    entry_multiple: float,
    entry_metric_node: str,
    exit_multiple: float,
    exit_metric_node: str,
    exit_net_debt_node: str,
    exit_period: str,
    sources: list[tuple[str, float]],
    transaction_fees: float = 0.0,
    check_mappings: Any | None = None,
) -> LboResult:
    """Evaluate a leveraged-buyout transaction against a statement model.

    Entry enterprise value is priced at the model's first period, the sponsor
    equity check is the sources-and-uses residual, and exit proceeds are the
    exit enterprise value less modelled net debt at ``exit_period``. IRR is out
    of scope: pair ``exit_equity_proceeds`` with the equity outflow at close and
    call ``finstack_quant.portfolio.mwr_xirr``.

    Parameters
    ----------
    model : FinancialModelSpec | str
        A ``FinancialModelSpec`` object or a JSON string; metadata must include
        a ``"currency"`` key.
    entry_multiple : float
        Entry multiple in turns (``8.5`` = 8.5x).
    entry_metric_node : str
        Monetary node in model currency supplying the entry metric at the
        first period (e.g. ``"ebitda"``).
    exit_multiple : float
        Exit multiple in turns.
    exit_metric_node : str
        Monetary node in model currency supplying the exit metric at ``exit_period``.
    exit_net_debt_node : str
        Monetary node in model currency supplying net debt at ``exit_period``.
    exit_period : str
        Exit period label (``"2029"`` or ``"2029Q4"``).
    sources : list[tuple[str, float]]
        Funded debt tranches at close as ``(name, amount)`` in model currency.
    transaction_fees : float
        Fees funded at close, in model currency. Default ``0.0``.
    check_mappings : LboCheckMappings | dict | str | None
        When supplied, runs the LBO model check suite against the same
        evaluation and populates ``LboResult.checks``.

    Returns
    -------
    LboResult
        Money outputs in the model currency, ``moic`` as a scalar, and
        ``checks`` (``CheckReport`` or ``None``).

    Raises
    ------
    ValueError
        If a tranche amount is invalid, ``exit_period`` does not parse, sources
        and uses cannot balance, or the model fails to evaluate.
    KeyError
        If a metric or net-debt node is missing at the required period.

    Examples
    --------
    >>> from finstack_quant.statements_analytics import evaluate_lbo
    >>> evaluate_lbo("{}", 8.0, "ebitda", 9.0, "ebitda", "net_debt", "2029Q4", [])
    Traceback (most recent call last):
    ValueError: ...
    """

def evaluate_scenario_set(model: Any, scenario_set: Any) -> ScenarioResults:
    """Evaluate all scenarios in a scenario set.

    Parameters
    ----------
    model : FinancialModelSpec | str
        A ``FinancialModelSpec`` object or a JSON string.
    scenario_set : ScenarioSet | str
        A typed scenario set or its JSON serialization.

    Returns
    -------
    ScenarioResults
        Typed mapping of scenario names to statement results.

    Raises
    ------
    ValueError
        If the set is empty, a parent chain cycles, an override is
        incompatible with its node, or evaluation fails.
    KeyError
        If an override names a node missing from the model.

    Examples
    --------
    >>> from finstack_quant.statements import ModelBuilder
    >>> from finstack_quant.statements_analytics import ScenarioSet, evaluate_scenario_set
    >>> b = ModelBuilder("m")
    >>> _ = b.periods("2025Q1..Q2", None)
    >>> _ = b.value("revenue", [("2025Q1", 100.0), ("2025Q2", 110.0)])
    >>> results = evaluate_scenario_set(b.build(), ScenarioSet({"base": {}, "down": {"revenue": 90.0}}))
    >>> results.names
    ['base', 'down']
    """

def explain_formula(model: Any, results: Any, node_id: str, period: str) -> Explanation:
    """Explain a formula for a specific node and period.

    Parameters
    ----------
    model : FinancialModelSpec | str
        A ``FinancialModelSpec`` object or a JSON string.
    results : StatementResult | str
        A ``StatementResult`` object or a JSON string.
    node_id : str
        Node whose formula to explain.
    period : str
        Period string.

    Returns
    -------
    Explanation
        Typed explanation with ``breakdown`` steps, ``to_text()`` and
        ``to_dataframe()``; ``to_json()`` matches the WASM ``explainFormula``.

    Raises
    ------
    KeyError
        If ``node_id`` is not in the model or has no value at ``period``.
    ValueError
        If ``period`` does not parse or a payload is malformed JSON.

    Examples
    --------
    >>> from finstack_quant.statements import ModelBuilder, Evaluator
    >>> from finstack_quant.statements_analytics import explain_formula
    >>> b = ModelBuilder("m")
    >>> _ = b.periods("2025Q1..Q1", None)
    >>> _ = b.value("revenue", [("2025Q1", 100.0)])
    >>> _ = b.compute("profit", "revenue * 0.5")
    >>> model = b.build()
    >>> explain_formula(model, Evaluator().evaluate(model), "profit", "2025Q1").final_value
    50.0
    """

def explain_formula_text(model: Any, results: Any, node_id: str, period: str) -> str:
    """Get a detailed text explanation for a formula.

    Parameters
    ----------
    model : FinancialModelSpec | str
        A ``FinancialModelSpec`` object or a JSON string.
    results : StatementResult | str
        A ``StatementResult`` object or a JSON string.
    node_id : str
        Node whose formula to explain.
    period : str
        Period string.

    Returns
    -------
    str
        Human-readable multi-line explanation (``explain_formula(...).to_text()``).

    Raises
    ------
    KeyError
        If ``node_id`` is not in the model or has no value at ``period``.
    ValueError
        If ``period`` does not parse or a payload is malformed JSON.

    Examples
    --------
    >>> from finstack_quant.statements_analytics import explain_formula_text
    >>> explain_formula_text("{}", "{}", "ebitda", "2025Q1")
    Traceback (most recent call last):
    ValueError: ...
    """

def generate_tornado_entries(result: Any, metric_node: str, period: str | None = None) -> list[TornadoEntry]:
    """Generate tornado chart entries for a sensitivity result.

    Parameters
    ----------
    result : SensitivityResult | str
        A typed sensitivity result or its JSON serialization.
    metric_node : str
        Node to extract tornado entries for.
    period : str | None
        Optional period string to pin the tornado to.

    Returns
    -------
    list[TornadoEntry]
        Typed entries sorted by descending absolute swing.

    Raises
    ------
    ValueError
        If ``period`` does not parse or ``result`` is malformed JSON.

    Examples
    --------
    >>> from finstack_quant.statements_analytics import generate_tornado_entries
    >>> generate_tornado_entries("{}", "ebitda")
    Traceback (most recent call last):
    ValueError: ...
    """

def goal_seek(
    model: Any,
    target_node: str,
    target_period: str,
    target_value: float,
    driver_node: str,
    driver_period: str,
    update_model: bool = True,
    bounds: tuple[float, float] | None = None,
) -> GoalSeekResult:
    """Find the driver value that makes a target node reach a target value.

    Parameters
    ----------
    model : FinancialModelSpec | str
        A ``FinancialModelSpec`` object or a JSON string.
    target_node : str
        Node to drive towards ``target_value``.
    target_period : str
        Period string for the target (e.g. ``"2025Q4"``).
    target_value : float
        Desired value for the target node.
    driver_node : str
        Node whose value is adjusted to reach the target.
    driver_period : str
        Period string for the driver.
    update_model : bool
        If ``True``, the solved value is written back into the returned model.
        Default ``True``.
    bounds : tuple[float, float] | None
        Optional search bounds ``(lo, hi)``; bisection is used when set.

    Returns
    -------
    GoalSeekResult
        ``solved_value`` plus ``model`` (the updated ``FinancialModelSpec`` or
        ``None``). ``float(result)`` yields the solved value.

    Raises
    ------
    ValueError
        If a period does not parse, the solver fails to converge, or the
        bracket does not contain a root, or the final objective residual exceeds 1e-9 times max(1, abs(target_value)). Failure leaves the model unchanged.
    KeyError
        If ``target_node`` or ``driver_node`` is missing from the model.

    Examples
    --------
    >>> from finstack_quant.statements import ModelBuilder
    >>> from finstack_quant.statements_analytics import goal_seek
    >>> b = ModelBuilder("m")
    >>> _ = b.periods("2025Q1..Q1", None)
    >>> _ = b.value("revenue", [("2025Q1", 100.0)])
    >>> _ = b.compute("profit", "revenue * 0.5")
    >>> round(goal_seek(b.build(), "profit", "2025Q1", 60.0, "revenue", "2025Q1").solved_value, 6)
    120.0
    """

def peer_stats(peer_values: list[float]) -> PeerStats | None:
    """Descriptive statistics for a peer distribution.

    Degenerate input yields ``None``, so this function does not raise.

    Parameters
    ----------
    peer_values : list[float]
        Peer distribution (need not be sorted).

    Returns
    -------
    PeerStats | None
        Typed statistics, or ``None`` when no statistics can be computed
        (matching the WASM twin's ``undefined``).

    Examples
    --------
    >>> from finstack_quant.statements_analytics import peer_stats
    >>> peer_stats([1.0, 2.0, 3.0, 4.0, 5.0]).count
    5
    """

def percentile_rank(peer_values: list[float], value: float) -> float | None:
    """Percentile rank of ``value`` within ``peer_values`` (0-1 scale).

    Uses the "fraction of values less than or equal" convention (Rust
    ``percentile_rank(values, value)`` argument order).

    Degenerate input yields ``None``, so this function does not raise.

    Parameters
    ----------
    peer_values : list[float]
        Peer distribution (need not be sorted).
    value : float
        The subject value to rank.

    Returns
    -------
    float | None
        Percentile rank in ``[0, 1]``, or ``None`` when ``peer_values`` is empty.

    Examples
    --------
    >>> from finstack_quant.statements_analytics import percentile_rank
    >>> percentile_rank([100.0, 200.0, 300.0, 400.0, 500.0], 250.0)
    0.4
    """

def pl_summary_report(results: Any, line_items: list[str], periods: list[str]) -> PLSummaryReport:
    """Build a P&L summary report for selected line items and periods.

    Parameters
    ----------
    results : StatementResult | str
        A ``StatementResult`` object or a JSON string.
    line_items : list[str]
        Node ids to include as rows.
    periods : list[str]
        Period-id strings for columns (e.g. ``["2025Q1", "2025Q2"]``).

    Returns
    -------
    PLSummaryReport
        Report with ``to_text()``, ``to_table()`` and ``to_dataframe()``.

    Raises
    ------
    ValueError
        If a period does not parse or ``results`` is malformed JSON.

    Examples
    --------
    >>> from finstack_quant.statements import ModelBuilder, Evaluator
    >>> from finstack_quant.statements_analytics import pl_summary_report
    >>> b = ModelBuilder("m")
    >>> _ = b.periods("2025Q1..Q2", None)
    >>> _ = b.value("revenue", [("2025Q1", 1.0), ("2025Q2", 2.0)])
    >>> pl_summary_report(Evaluator().evaluate(b.build()), ["revenue"], ["2025Q1"]).periods
    ['2025Q1']
    """

def pl_summary_report_text(results: Any, line_items: list[str], periods: list[str]) -> str:
    """Generate a P&L summary report as formatted text.

    Parameters
    ----------
    results : StatementResult | str
        A ``StatementResult`` object or a JSON string.
    line_items : list[str]
        Node ids to include as rows.
    periods : list[str]
        Period-id strings for columns.

    Returns
    -------
    str
        Formatted P&L summary report text (same as
        ``pl_summary_report(...).to_text()``).

    Raises
    ------
    ValueError
        If a period does not parse or ``results`` is malformed JSON.

    Examples
    --------
    >>> from finstack_quant.statements import ModelBuilder, Evaluator
    >>> from finstack_quant.statements_analytics import pl_summary_report_text
    >>> b = ModelBuilder("m")
    >>> _ = b.periods("2025Q1..Q2", None)
    >>> _ = b.value("revenue", [("2025Q1", 1.0), ("2025Q2", 2.0)])
    >>> pl_summary_report_text(Evaluator().evaluate(b.build()), ["revenue"], ["2025Q1"]).startswith("P&L Summary")
    True
    """

def regression_fair_value(
    x_values: list[float], y_values: list[float], subject_x: float, subject_y: float
) -> RegressionResult | None:
    """Single-factor OLS fit and evaluation at the subject's X.

    Conventions: ``fitted_value = intercept + slope * subject_x`` and
    ``residual = subject_y - fitted_value``.

    Degenerate input yields ``None``, so this function does not raise.

    Parameters
    ----------
    x_values : list[float]
        Peer X observations (independent variable).
    y_values : list[float]
        Peer Y observations; same length as ``x_values``.
    subject_x : float
        Subject's X value at which to evaluate the fit.
    subject_y : float
        Subject's observed Y value for the residual.

    Returns
    -------
    RegressionResult | None
        Typed fit, or ``None`` if fewer than three observations are available
        or X has zero variance, lengths differ, or any input/output is non-finite.

    Examples
    --------
    >>> from finstack_quant.statements_analytics import regression_fair_value
    >>> regression_fair_value([1.0, 2.0, 3.0, 4.0], [3.0, 5.0, 7.0, 9.0], 3.0, 10.0).residual
    3.0
    """

def render_check_report_html(report: Any) -> str:
    """Render a check report as HTML with inline styles.

    Parameters
    ----------
    report : CheckReport | dict | str
        Typed report, its serde dict, or JSON string.

    Returns
    -------
    str
        HTML-formatted report suitable for Jupyter notebooks.

    Raises
    ------
    ValueError
        If ``report`` is not a valid ``CheckReport`` payload.

    Examples
    --------
    >>> from finstack_quant.statements_analytics import render_check_report_html
    >>> render_check_report_html("{}")
    Traceback (most recent call last):
    ValueError: ...
    """

def render_check_report_text(report: Any) -> str:
    """Render a check report as plain text.

    Parameters
    ----------
    report : CheckReport | dict | str
        Typed report, its serde dict, or JSON string.

    Returns
    -------
    str
        Human-readable plain-text report.

    Raises
    ------
    ValueError
        If ``report`` is not a valid ``CheckReport`` payload.

    Examples
    --------
    >>> from finstack_quant.statements_analytics import render_check_report_text
    >>> render_check_report_text("{}")
    Traceback (most recent call last):
    ValueError: ...
    """

def run_checks(model: Any, spec: Any, results: Any | None = None) -> CheckReport:
    """Run checks from a suite spec against a model.

    Resolves both built-in and formula checks from the spec, evaluates the
    model (unless ``results`` is supplied), and returns a full check report.

    Parameters
    ----------
    model : FinancialModelSpec | str
        A ``FinancialModelSpec`` object or a JSON string.
    spec : CheckSuiteSpec | dict | str
        Typed ``finstack_quant.statements.CheckSuiteSpec``, its serde dict, or
        JSON string.
    results : StatementResult | str | None
        Pre-computed evaluation results; when provided the model is not
        re-evaluated.

    Returns
    -------
    CheckReport
        Typed report with summary, findings, JSON, and DataFrame accessors.

    Raises
    ------
    ValueError
        If the spec is malformed, a formula check does not parse, or the
        evaluation fails.
    KeyError
        If a check references a node missing from the model.

    Examples
    --------
    >>> from finstack_quant.statements import ModelBuilder
    >>> from finstack_quant.statements_analytics import run_checks
    >>> b = ModelBuilder("m")
    >>> _ = b.periods("2024Q1..Q2", None)
    >>> _ = b.value("revenue", [("2024Q1", 1.0), ("2024Q2", 2.0)])
    >>> spec = {
    ...     "name": "s",
    ...     "builtin_checks": [],
    ...     "formula_checks": [
    ...         {
    ...             "id": "pos",
    ...             "name": "positive",
    ...             "category": "internal_consistency",
    ...             "severity": "error",
    ...             "formula": "revenue > 0",
    ...             "message_template": "bad {period}",
    ...         }
    ...     ],
    ... }
    >>> run_checks(b.build(), spec).passed
    True
    """

def run_corporate_analysis(
    model: Any,
    wacc: float | None = None,
    terminal_value: Any | None = None,
    net_debt_override: float | None = None,
    cfads_node: str | None = None,
    interest_coverage_node: str = "ebitda",
    check_suite: Any | None = None,
    market: Any | None = None,
    as_of: Any | None = None,
    ltv_value_node: str | None = None,
) -> CorporateAnalysis:
    """Run the full corporate analysis pipeline.

    Evaluates statements and optionally runs DCF equity valuation plus credit
    context through the Rust ``CorporateAnalysisBuilder``.

    Parameters
    ----------
    model : FinancialModelSpec | str
        A ``FinancialModelSpec`` object or a JSON string.
    wacc : float | None
        Enables DCF valuation at this decimal discount rate when set.
    terminal_value : TerminalValueSpec | dict | str | None
        Terminal value method; required when ``wacc`` is set.
    net_debt_override : float | None
        Flat net debt for the equity bridge.
    cfads_node : str | None
        CFADS numerator required when the model has capital-structure credit
        analytics; no EBITDA fallback is applied.
    interest_coverage_node : str
        Earnings numerator used for interest coverage. Default ``"ebitda"``.
    check_suite : CheckSuiteSpec | dict | str | None
        Check suite required for DCF or credit analysis; must include
        ``NonFiniteCheck``.
    market : MarketContext | str | None
        Market context for statement evaluation (not WACC discounting).
    as_of : datetime.date | str | None
        Valuation date; required when ``market`` is set.
    ltv_value_node : str | None
        Statement node supplying a per-period LTV denominator. When omitted, a
        positive DCF enterprise value is broadcast as a constant denominator.

    Returns
    -------
    CorporateAnalysis
        ``statement`` (``StatementResult``), ``equity``
        (``CorporateValuationResult`` or ``None``), ``credit`` (per-instrument
        metrics) and ``ev_suppressed_non_positive``.

    Raises
    ------
    ValueError
        If ``terminal_value`` is missing while ``wacc`` is set, ``market`` is
        set without ``as_of``, a payload is malformed, or the pipeline fails.
    KeyError
        If a referenced node is missing from the model.

    Examples
    --------
    >>> from finstack_quant.statements import ModelBuilder
    >>> from finstack_quant.statements_analytics import run_corporate_analysis
    >>> b = ModelBuilder("m")
    >>> _ = b.periods("2024Q1..Q2", None)
    >>> _ = b.value("revenue", [("2024Q1", 1.0), ("2024Q2", 2.0)])
    >>> run_corporate_analysis(b.build()).statement.node_count
    1
    """

def run_credit_underwriting_checks(model: Any, mapping: Any, results: Any | None = None) -> CheckReport:
    """Run the built-in credit-underwriting check suite.

    Parameters
    ----------
    model : FinancialModelSpec | str
        A ``FinancialModelSpec`` object or a JSON string.
    mapping : CreditMapping | dict | str
        Typed node mapping, its serde dict, or JSON string.
    results : StatementResult | str | None
        Pre-computed evaluation results; skips re-evaluation when provided.

    Returns
    -------
    CheckReport
        Typed report with summary, findings, JSON, and DataFrame accessors.

    Raises
    ------
    ValueError
        If the mapping is malformed or the evaluation fails.
    KeyError
        If a mapped node is missing from the model.

    Examples
    --------
    >>> from finstack_quant.statements_analytics import CreditMapping, run_credit_underwriting_checks
    >>> mapping = CreditMapping("total_debt", "ebitda", "interest_expense")
    >>> callable(run_credit_underwriting_checks)
    True
    """

def run_sensitivity(model: Any, config: Any) -> SensitivityResult:
    """Run sensitivity analysis on a financial model.

    Parameters
    ----------
    model : FinancialModelSpec | str
        A ``FinancialModelSpec`` object or a JSON string.
    config : SensitivityConfig | str
        A typed configuration or its JSON serialization.

    Returns
    -------
    SensitivityResult
        Typed sensitivity result with per-scenario outputs, ``baseline`` and
        DataFrame exits.

    Raises
    ------
    ValueError
        If the configuration is malformed or a scenario fails to evaluate.
    KeyError
        If a perturbed parameter or target metric is missing from the model.

    Examples
    --------
    >>> from finstack_quant.statements import ModelBuilder
    >>> from finstack_quant.statements_analytics import ParameterSpec, SensitivityConfig, run_sensitivity
    >>> b = ModelBuilder("m")
    >>> _ = b.periods("2025Q1..Q2", None)
    >>> _ = b.value("revenue", [("2025Q1", 100.0), ("2025Q2", 110.0)])
    >>> _ = b.compute("profit", "revenue * 0.5")
    >>> cfg = SensitivityConfig(
    ...     "diagonal", [ParameterSpec.with_percentages("revenue", "2025Q2", 110.0, [-10.0, 10.0])], ["profit"]
    ... )
    >>> len(run_sensitivity(b.build(), cfg))
    2
    """

def run_three_statement_checks(model: Any, mapping: Any, results: Any | None = None) -> CheckReport:
    """Run the built-in three-statement check suite.

    Parameters
    ----------
    model : FinancialModelSpec | str
        A ``FinancialModelSpec`` object or a JSON string.
    mapping : ThreeStatementMapping | dict | str
        Typed node mapping, its serde dict, or JSON string.
    results : StatementResult | str | None
        Pre-computed evaluation results; skips re-evaluation when provided.

    Returns
    -------
    CheckReport
        Typed report with summary, findings, JSON, and DataFrame accessors.

    Raises
    ------
    ValueError
        If the mapping is malformed or the evaluation fails.
    KeyError
        If a mapped node is missing from the model.

    Examples
    --------
    >>> from finstack_quant.statements_analytics import ThreeStatementMapping, run_three_statement_checks
    >>> mapping = ThreeStatementMapping("cash", "retained_earnings", "net_income")
    >>> callable(run_three_statement_checks)
    True
    """

def run_variance(base: Any, comparison: Any, config: Any) -> VarianceReport:
    """Run variance analysis comparing two statement results.

    Parameters
    ----------
    base : StatementResult | str
        A ``StatementResult`` object or a JSON string.
    comparison : StatementResult | str
        A ``StatementResult`` object or a JSON string.
    config : VarianceConfig | str
        A typed configuration or its JSON serialization.

    Returns
    -------
    VarianceReport
        Per-metric, per-period rows including ``driver_contribution``.

    Raises
    ------
    ValueError
        If the configuration is malformed.
    KeyError
        If a configured metric is missing at a configured period.

    Examples
    --------
    >>> from finstack_quant.statements_analytics import run_variance
    >>> run_variance("{}", "{}", "{}")
    Traceback (most recent call last):
    ValueError: ...
    """

def scenario_diff(
    scenario_set: Any, results: ScenarioResults, baseline: str, comparison: str, metrics: list[str], periods: list[str]
) -> ScenarioDiff:
    """Compare two evaluated scenarios metric-by-metric.

    Parameters
    ----------
    scenario_set : ScenarioSet | str
        A typed scenario set or its JSON serialization.
    results : ScenarioResults
        Output of ``evaluate_scenario_set`` for the same scenario set.
    baseline : str
        Name of the scenario to treat as the baseline.
    comparison : str
        Name of the scenario to compare against the baseline.
    metrics : list[str]
        Node identifiers to compare. Must be non-empty.
    periods : list[str]
        Period identifiers (e.g. ``"2025Q1"``). Must be non-empty.

    Returns
    -------
    ScenarioDiff
        Baseline and comparison names alongside the variance report.

    Raises
    ------
    ValueError
        If ``metrics`` or ``periods`` is empty, a scenario name is unknown, or
        a period fails to parse.
    KeyError
        If a metric is missing at a period in either scenario.

    Examples
    --------
    >>> from finstack_quant.statements_analytics import scenario_diff
    >>> scenario_diff("{}", "{}", "base", "up", ["ebitda"], ["2025Q1"])
    Traceback (most recent call last):
    TypeError: ...
    """

def score_relative_value(peer_set: Any, dimensions: Any) -> RelativeValueResult:
    """Score a subject against its peers across weighted dimensions.

    The composite is the weighted average of the direction-adjusted dimension
    scores: positive = cheap, negative = rich.

    Parameters
    ----------
    peer_set : PeerSet | dict | str
        Typed peer set, or the canonical serde ``PeerSet`` payload as a dict or
        JSON string (``{"subject": ..., "peers": [...], "period_basis": "ltm"}``).
    dimensions : list[ScoringDimension | dict] | str
        Typed dimensions, canonical ``ScoringDimension`` dicts, or a JSON list.

    Returns
    -------
    RelativeValueResult
        Composite score, per-dimension breakdown, confidence and peer count.

    Raises
    ------
    ValueError
        If a payload is malformed, a direction or extractor is unknown, or the
        peer set cannot be scored (no peers with the required metrics).

    Examples
    --------
    >>> from finstack_quant.statements_analytics import (
    ...     CompanyMetrics,
    ...     PeerSet,
    ...     ScoringDimension,
    ...     score_relative_value,
    ... )
    >>> peers = [CompanyMetrics(f"P{i}", {"pe": float(10 * i)}) for i in (1, 2, 3)]
    >>> peer_set = PeerSet(CompanyMetrics("SUBJ", {"pe": 30.0}), peers)
    >>> score_relative_value(peer_set, [ScoringDimension("pe", "pe", direction="higher_is_rich")]).composite_score < 0
    True
    """

def variance_bridge(
    base: Any,
    comparison: Any,
    target_metric: str,
    period: str,
    drivers: list[str],
    baseline_label: str,
    comparison_label: str,
) -> BridgeChart:
    """Decompose a metric's scenario variance across named drivers.

    Driver contributions are raw deltas in *driver* units rather than
    sensitivities of the target metric, so they generally do not sum to the
    target variance. The gap is reported in ``BridgeChart.unexplained``.

    Parameters
    ----------
    base : StatementResult | str
        Baseline evaluated statement result, or its JSON serialization.
    comparison : StatementResult | str
        Comparison evaluated statement result, or its JSON serialization.
    target_metric : str
        Node identifier whose variance is being explained.
    period : str
        Period identifier (e.g. ``"2025Q4"``).
    drivers : list[str]
        Node identifiers treated as explanatory drivers.
    baseline_label : str
        Display label for the baseline column.
    comparison_label : str
        Display label for the comparison column.

    Returns
    -------
    BridgeChart
        Ordered driver contributions plus the unexplained residual.

    Raises
    ------
    ValueError
        If the period fails to parse.
    KeyError
        If the target or any driver is missing from either result at ``period``.

    Examples
    --------
    >>> from finstack_quant.statements_analytics import variance_bridge
    >>> variance_bridge("{}", "{}", "ebitda", "2025Q1", ["revenue"], "base", "actual")
    Traceback (most recent call last):
    ValueError: ...
    """

def wacc(
    equity_weight: float, cost_of_equity: float, debt_weight: float, cost_of_debt: float, tax_rate: float
) -> float:
    """Weighted-average cost of capital (WACC).

    ``WACC = w_E * r_E + w_D * r_D * (1 - T)``.

    Parameters
    ----------
    equity_weight : float
        Equity share of total capital as a decimal fraction; non-negative.
    cost_of_equity : float
        Required return on equity in decimal form (``0.115`` = 11.5%).
    debt_weight : float
        Debt share of total capital as a decimal fraction; non-negative and
        summing with ``equity_weight`` to ``1.0``.
    cost_of_debt : float
        Pre-tax marginal borrowing yield in decimal form.
    tax_rate : float
        Marginal corporate tax rate as a decimal in ``[0, 1]``.

    Returns
    -------
    float
        Blended discount rate as a decimal fraction.

    Raises
    ------
    ValueError
        If a weight is negative, the weights do not sum to ``1.0``, or the tax
        rate is outside ``[0, 1]``.

    Examples
    --------
    >>> from finstack_quant.statements_analytics import wacc
    >>> round(wacc(0.6, 0.10, 0.4, 0.05, 0.25), 4)
    0.075
    """

def z_score(peer_values: list[float], value: float) -> float | None:
    """Standard (z-) score of ``value`` in the peer distribution.

    Degenerate input yields ``None``, so this function does not raise.

    Parameters
    ----------
    peer_values : list[float]
        Peer distribution.
    value : float
        The subject value.

    Returns
    -------
    float | None
        ``(value - mean(peers)) / stddev(peers)``, or ``None`` when fewer than
        two peers are provided or the peer variance is zero.

    Examples
    --------
    >>> from finstack_quant.statements_analytics import z_score
    >>> z_score([1.0, 2.0, 3.0, 4.0, 5.0], 3.0)
    0.0
    """
