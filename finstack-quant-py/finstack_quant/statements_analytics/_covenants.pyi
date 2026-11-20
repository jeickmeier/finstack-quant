"""Stub declarations mirroring ``finstack-quant-py/src/bindings/statements_analytics/covenants.rs``."""

from __future__ import annotations

from collections.abc import Mapping, Sequence
from datetime import date
from typing import Any, Literal, TypedDict

import pandas as pd
from finstack_quant.covenants import CovenantEngine, CovenantForecastConfig, FutureBreach
from finstack_quant.statements import FinancialModelSpec, StatementResult

class _RatioUnit(TypedDict):
    kind: Literal["ratio"]

class _MoneyUnit(TypedDict):
    kind: Literal["money"]
    currency: str

class _CovenantObservationBinding(TypedDict):
    node: str
    unit: _RatioUnit | _MoneyUnit

# Covenant evaluation and forecasting over statement results

def evaluate_covenants(
    covenants: CovenantEngine,
    results: StatementResult | str,
    node_mapping: dict[str, str] | None = None,
) -> pd.DataFrame:
    """
    Evaluate covenant compliance at the end of every statement result period.

    Each period of ``results.periods`` is tested on its last calendar day
    with the covenants applicable on that date: a covenant window covering the date replaces the engine's
    top-level specifications, and waivers and threshold schedules apply as in
    ``CovenantEngine.evaluate``. A node is read only when an applicable
    covenant needs it, so a node that is missing or non-finite in a period
    where no applicable covenant reads it (for example a trailing ratio before
    four quarters of history, outside the window that tests it) does not
    raise. The engine's breach history is not updated.

    Parameters
    ----------
    results : StatementResult or str
        Evaluated statement results, typed or as JSON. Node ids are the
        covenant metric ids (``metric_id``, or each covenant type's
        conventional name); ratios are in turns (``3.5`` means 3.5x) and
        amounts in the model currency. Its ``periods`` give the test dates.
    covenants : CovenantEngine
        Covenant engine whose specifications, windows, waivers and threshold
        schedules are evaluated.
    node_mapping : dict of str to str or None
        Covenant metric IDs mapped to exact result node IDs. Unlisted
        metrics use their own identifiers; use None to map all by identity.

    Returns
    -------
    pd.DataFrame
        Long frame with one row per (test date, applicable covenant), dates in
        timeline order and covenants in engine order, with the columns
        of ``CovenantEngine.evaluate_series``: ``as_of`` (ISO string),
        ``covenant`` (instance key), ``covenant_type``, ``passed``,
        ``actual_value``, ``threshold``, ``headroom``, ``headroom_raw``, ``details``. WASM
        returns the same reports as ``DatedCovenantReports`` rows.

    Raises
    ------
    KeyError
        If a metric an applicable covenant reads has no value in a period.
    ValueError
        If such a metric is non-finite, the engine is invalid, or ``results``
        JSON is malformed.

    Examples
    --------
    >>> from finstack_quant.covenants import CovenantEngine, CovenantTemplate
    >>> from finstack_quant.statements import Evaluator, ModelBuilder
    >>> from finstack_quant.statements_analytics import evaluate_covenants
    >>> builder = ModelBuilder("borrower")
    >>> _ = builder.periods("2025Q1..Q2", None)
    >>> _ = builder.value("total_leverage", [("2025Q1", 6.0), ("2025Q2", 7.5)])
    >>> _ = builder.value("senior_leverage", [("2025Q1", 3.0), ("2025Q2", 3.5)])
    >>> model = builder.build()
    >>> engine = CovenantEngine.from_template(CovenantTemplate.cov_lite(7.0, 4.5))
    >>> frame = evaluate_covenants(engine, Evaluator().evaluate(model))
    >>> frame.loc[frame["covenant"] == "max_total_leverage", ["as_of", "passed"]].values.tolist()
    [['2025-03-31', True], ['2025-06-30', False]]

    """
    ...

def forecast_breaches(
    covenants: CovenantEngine,
    results: StatementResult | str,
    config: CovenantForecastConfig | None = None,
    node_mapping: dict[str, str] | None = None,
) -> list[FutureBreach]:
    """
    Forecast covenant breaches over every statement result period.

    Every period of ``results.periods`` is a test date (its last calendar
    day). Effective windows, waivers and threshold schedules are
    honored and non-numeric covenants are skipped. Deterministic breaches are
    always listed; in stochastic mode dates whose breach probability reaches
    ``config.breach_probability_threshold`` are listed too. Without
    ``config.reference_date`` the horizon starts the day before the first
    result period begins.

    Parameters
    ----------
    results : StatementResult or str
        Evaluated statement results, typed or as JSON, holding every metric
        the active numeric covenants read on each test date (ratios in turns).
    covenants : CovenantEngine
        Covenant engine whose effective specifications are forecast.
    config : CovenantForecastConfig or None, optional
        Forecast policy; ``None`` means the deterministic, maintenance-scope
        default.
    node_mapping : dict of str to str or None
        Covenant metric IDs mapped to exact result node IDs. Unlisted
        metrics use their own identifiers; use None to map all by identity.

    Returns
    -------
    list[FutureBreach]
        Breaches sorted by breach date, then covenant id.

    Raises
    ------
    KeyError
        If a metric an active covenant reads has no value in a period.
    ValueError
        If such a metric is non-finite, the engine or ``config`` is invalid
        (for example stochastic mode without ``volatility``), or ``results``
        JSON is malformed.

    Examples
    --------
    >>> from finstack_quant.covenants import CovenantEngine, CovenantForecastConfig, CovenantTemplate, covenant_template
    >>> from finstack_quant.statements import Evaluator, ModelBuilder
    >>> from finstack_quant.statements_analytics import forecast_breaches
    >>> builder = ModelBuilder("borrower")
    >>> _ = builder.periods("2025Q1..Q2", None)
    >>> _ = builder.value("total_leverage", [("2025Q1", 6.0), ("2025Q2", 7.5)])
    >>> _ = builder.value("senior_leverage", [("2025Q1", 3.0), ("2025Q2", 3.5)])
    >>> model = builder.build()
    >>> engine = CovenantEngine.from_template(CovenantTemplate.cov_lite(7.0, 4.5))
    >>> config = CovenantForecastConfig().with_scope("incurrence")  # cov-lite tests are incurrence tests
    >>> breaches = forecast_breaches(engine, Evaluator().evaluate(model), config)
    >>> [(b.covenant_id, b.breach_date.isoformat()) for b in breaches]
    [('max_total_leverage', '2025-06-30')]

    """
    ...

class StatementCovenantSpec:
    """Select effective covenant terms and qualified statement observations.

    Examples
    --------
    >>> from finstack_quant.statements_analytics import StatementCovenantSpec
    >>> spec = StatementCovenantSpec(
    ...     scope="maintenance",
    ...     covenant_ids=["max_leverage"],
    ...     periods=["2025Q1"],
    ...     observed_through="2025-04-30",
    ...     bindings={"leverage": {"node": "net_leverage", "unit": {"kind": "ratio"}}},
    ... )
    """

    def __init__(
        self,
        *,
        scope: Literal["maintenance", "incurrence"],
        covenant_ids: Sequence[str],
        periods: Sequence[str],
        observed_through: date | str,
        bindings: Mapping[str, _CovenantObservationBinding],
    ) -> None:
        """Name the exact tests, observation periods and dated evidence.

        Parameters
        ----------
        scope : str
            Contract test scope, either maintenance or incurrence.
        covenant_ids : Sequence[str]
            Unique covenant instance IDs in display order.
        periods : Sequence[str]
            Unique requested period IDs in model timeline order.
        observed_through : date or str
            Inclusive information cutoff, as a date or ISO YYYY-MM-DD.
        bindings : Mapping[str, _CovenantObservationBinding]
            Metric IDs and exact statement nodes with ratio or ISO currency units.

        Raises
        ------
        ValueError
            If a date, period, scope or binding has an invalid representation.
        """
        ...

class StatementCovenantAssessment:
    """Covenant terms and available evidence for each elected period.

    Examples
    --------
    >>> from datetime import date
    >>> from finstack_quant.core.market_data import MarketContext
    >>> from finstack_quant.covenants import CovenantEngine, CovenantForecastConfig, CovenantTemplate
    >>> from finstack_quant.statements import Evaluator, ModelBuilder
    >>> from finstack_quant.statements_analytics import StatementCovenantSpec
    >>> from finstack_quant.statements_analytics import assess_statement_covenants, forecast_statement_covenants
    >>> builder = ModelBuilder("borrower")
    >>> _ = builder.periods("2025Q1..Q2", None)
    >>> _ = builder.source_snapshot("approved-forecast")
    >>> _ = builder.value("ratio", [("2025Q1", 3.0), ("2025Q2", 5.0)])
    >>> model = builder.build()
    >>> cutoff = date(2024, 12, 31)
    >>> results = Evaluator().evaluate_with_market(model, MarketContext(), cutoff, cutoff)
    >>> engine = CovenantEngine.from_template(CovenantTemplate.cov_lite(4.0, 2.5))
    >>> spec = StatementCovenantSpec(
    ...     scope="incurrence",
    ...     covenant_ids=["max_total_leverage"],
    ...     periods=["2025Q1", "2025Q2"],
    ...     observed_through=cutoff,
    ...     bindings={"total_leverage": {"node": "ratio", "unit": {"kind": "ratio"}}},
    ... )
    >>> assessment = assess_statement_covenants(engine, model, results, spec)
    >>> assessment.to_dataframe()["passed"].tolist()
    [True, False]
    """

    @property
    def provenance(self) -> dict[str, Any]:
        """Return source identity and dates for the admitted statement evaluation.

        Returns
        -------
        dict[str, Any]
            Run identity, model and information cutoff used for the report.

        Raises
        ------
        ValueError
            If run metadata cannot be converted to Python.
        """
        ...

    def to_dataframe(self) -> pd.DataFrame:
        """Return a row per period and covenant, with nullable pass and raw headroom.

        Returns
        -------
        pd.DataFrame
            One row per period and selected covenant, including availability and raw headroom.

        Raises
        ------
        ImportError
            If pandas is unavailable.
        """
        ...

    def to_json(self) -> str:
        """Return evidence and assessment states as JSON.

        Returns
        -------
        str
            Serialized run identity and dated observations, effective terms and reports.

        Raises
        ------
        ValueError
            If the result cannot be represented as JSON.
        """
        ...

    @staticmethod
    def from_json(json: str) -> StatementCovenantAssessment:
        """Rebuild the native StatementCovenantAssessment from its JSON record.

        Parameters
        ----------
        json : str
            JSON object with the dated evidence and fields of this record.

        Returns
        -------
        StatementCovenantAssessment
            Decoded native value.

        Raises
        ------
        ValueError
            If the JSON syntax or field values are invalid.

        Examples
        --------
        >>> from finstack_quant.statements_analytics import StatementCovenantAssessment
        >>> try:
        ...     StatementCovenantAssessment.from_json("null")
        ... except ValueError:
        ...     print("A record is required")
        A record is required
        """
        ...

    def __reduce__(self) -> tuple[object, tuple[str]]:
        """Return the reconstruction callable and native JSON arguments for pickle."""
        ...

class StatementCovenantForecast:
    """Prospective breaches and their assessment coverage.

    Examples
    --------
    >>> from datetime import date
    >>> from finstack_quant.core.market_data import MarketContext
    >>> from finstack_quant.covenants import CovenantEngine, CovenantForecastConfig, CovenantTemplate
    >>> from finstack_quant.statements import Evaluator, ModelBuilder
    >>> from finstack_quant.statements_analytics import StatementCovenantSpec
    >>> from finstack_quant.statements_analytics import assess_statement_covenants, forecast_statement_covenants
    >>> builder = ModelBuilder("borrower")
    >>> _ = builder.periods("2025Q1..Q2", None)
    >>> _ = builder.source_snapshot("approved-forecast")
    >>> _ = builder.value("ratio", [("2025Q1", 3.0), ("2025Q2", 5.0)])
    >>> model = builder.build()
    >>> cutoff = date(2024, 12, 31)
    >>> results = Evaluator().evaluate_with_market(model, MarketContext(), cutoff, cutoff)
    >>> engine = CovenantEngine.from_template(CovenantTemplate.cov_lite(4.0, 2.5))
    >>> spec = StatementCovenantSpec(
    ...     scope="incurrence",
    ...     covenant_ids=["max_total_leverage"],
    ...     periods=["2025Q1", "2025Q2"],
    ...     observed_through=cutoff,
    ...     bindings={"total_leverage": {"node": "ratio", "unit": {"kind": "ratio"}}},
    ... )
    >>> config = CovenantForecastConfig(reference_date=cutoff).with_scope("incurrence")
    >>> forecast = forecast_statement_covenants(engine, model, results, spec, config)
    >>> [breach.breach_date.isoformat() for breach in forecast.breaches]
    ['2025-06-30']
    """

    @property
    def assessment(self) -> StatementCovenantAssessment:
        """Return all elected forecast observations, including missing evidence.

        This property does not raise.

        Returns
        -------
        StatementCovenantAssessment
            Dated coverage and evidence for every selected forecast period.
        """
        ...

    @property
    def breaches(self) -> list[FutureBreach]:
        """Return breach evidence sorted by observation date and then by covenant ID.

        This property does not raise.

        Returns
        -------
        list[FutureBreach]
            Projected breached tests sorted by reporting date and covenant ID.
        """
        ...

    @property
    def reference_date(self) -> date:
        """Return the elected forecast as-of date.

        Returns
        -------
        date
            Elected base date for projecting future covenant tests.

        Raises
        ------
        OverflowError
            If the date falls outside the Python calendar range.
        """
        ...

def assess_statement_covenants(
    engine: CovenantEngine,
    model: FinancialModelSpec | str,
    results: StatementResult | str,
    spec: StatementCovenantSpec,
) -> StatementCovenantAssessment:
    """Assess the selected historical and projected reporting dates.

    Parameters
    ----------
    engine : CovenantEngine
        Contract terms, windows, waiver dates and threshold schedule.
    model : FinancialModelSpec or str
        Statement model with its source and publication dates.
    results : StatementResult or str
        Statement run for the exact model and information cutoff.
    spec : StatementCovenantSpec
        Contract scope, test IDs, period IDs and typed named observations.

    Returns
    -------
    StatementCovenantAssessment
        Per-date statuses, available evidence and original-unit signed headroom.

    Raises
    ------
    ValueError
        If the model, result, scope, requested dates or typed bindings are invalid.
    KeyError
        If an explicitly requested covenant or period is unknown.

    Examples
    --------
    >>> from datetime import date
    >>> from finstack_quant.core.market_data import MarketContext
    >>> from finstack_quant.covenants import CovenantEngine, CovenantForecastConfig, CovenantTemplate
    >>> from finstack_quant.statements import Evaluator, ModelBuilder
    >>> from finstack_quant.statements_analytics import StatementCovenantSpec
    >>> from finstack_quant.statements_analytics import assess_statement_covenants, forecast_statement_covenants
    >>> builder = ModelBuilder("borrower")
    >>> _ = builder.periods("2025Q1..Q2", None)
    >>> _ = builder.source_snapshot("approved-forecast")
    >>> _ = builder.value("ratio", [("2025Q1", 3.0), ("2025Q2", 5.0)])
    >>> model = builder.build()
    >>> cutoff = date(2024, 12, 31)
    >>> results = Evaluator().evaluate_with_market(model, MarketContext(), cutoff, cutoff)
    >>> engine = CovenantEngine.from_template(CovenantTemplate.cov_lite(4.0, 2.5))
    >>> spec = StatementCovenantSpec(
    ...     scope="incurrence",
    ...     covenant_ids=["max_total_leverage"],
    ...     periods=["2025Q1", "2025Q2"],
    ...     observed_through=cutoff,
    ...     bindings={"total_leverage": {"node": "ratio", "unit": {"kind": "ratio"}}},
    ... )
    >>> assessment = assess_statement_covenants(engine, model, results, spec)
    >>> assessment.to_dataframe()["passed"].tolist()
    [True, False]
    """
    ...

def forecast_statement_covenants(
    engine: CovenantEngine,
    model: FinancialModelSpec | str,
    results: StatementResult | str,
    spec: StatementCovenantSpec,
    config: CovenantForecastConfig,
) -> StatementCovenantForecast:
    """Forecast breaches with explicit reporting coverage.

    Parameters
    ----------
    engine : CovenantEngine
        Contract terms, windows, waiver dates and threshold schedule.
    model : FinancialModelSpec or str
        Statement model with its source and publication dates.
    results : StatementResult or str
        Statement run for the exact model and information cutoff.
    spec : StatementCovenantSpec
        Contract scope, test IDs, forecast period IDs and typed named observations.
    config : CovenantForecastConfig
        Forecast policy with an explicit reference date and matching scope.

    Returns
    -------
    StatementCovenantForecast
        Dated breach evidence and coverage across all selected forecast periods.

    Raises
    ------
    ValueError
        If the evidence, typed bindings, forecast parameters or dates are invalid.
    KeyError
        If an explicitly requested covenant or period is unknown.

    Examples
    --------
    >>> from datetime import date
    >>> from finstack_quant.core.market_data import MarketContext
    >>> from finstack_quant.covenants import CovenantEngine, CovenantForecastConfig, CovenantTemplate
    >>> from finstack_quant.statements import Evaluator, ModelBuilder
    >>> from finstack_quant.statements_analytics import StatementCovenantSpec
    >>> from finstack_quant.statements_analytics import assess_statement_covenants, forecast_statement_covenants
    >>> builder = ModelBuilder("borrower")
    >>> _ = builder.periods("2025Q1..Q2", None)
    >>> _ = builder.source_snapshot("approved-forecast")
    >>> _ = builder.value("ratio", [("2025Q1", 3.0), ("2025Q2", 5.0)])
    >>> model = builder.build()
    >>> cutoff = date(2024, 12, 31)
    >>> results = Evaluator().evaluate_with_market(model, MarketContext(), cutoff, cutoff)
    >>> engine = CovenantEngine.from_template(CovenantTemplate.cov_lite(4.0, 2.5))
    >>> spec = StatementCovenantSpec(
    ...     scope="incurrence",
    ...     covenant_ids=["max_total_leverage"],
    ...     periods=["2025Q1", "2025Q2"],
    ...     observed_through=cutoff,
    ...     bindings={"total_leverage": {"node": "ratio", "unit": {"kind": "ratio"}}},
    ... )
    >>> config = CovenantForecastConfig(reference_date=cutoff).with_scope("incurrence")
    >>> forecast = forecast_statement_covenants(engine, model, results, spec, config)
    >>> [breach.breach_date.isoformat() for breach in forecast.breaches]
    ['2025-06-30']
    """
    ...
