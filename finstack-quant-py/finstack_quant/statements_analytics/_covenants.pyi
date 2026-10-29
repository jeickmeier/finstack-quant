"""Stub declarations mirroring ``finstack-quant-py/src/bindings/statements_analytics/covenants.rs``."""

from __future__ import annotations

import pandas as pd
from finstack_quant.covenants import CovenantEngine, CovenantForecastConfig, FutureBreach
from finstack_quant.statements import FinancialModelSpec, StatementResult

# Covenant evaluation and forecasting over statement results

def evaluate_covenants(
    results: StatementResult | str,
    covenants: CovenantEngine,
    model: FinancialModelSpec | str,
) -> pd.DataFrame:
    """
    Evaluate covenant compliance at the end of every statement result period.

    Each period that carries a value in ``results`` is tested on its last
    calendar day (from the model calendar) with the covenants applicable on
    that date: a covenant window covering the date replaces the engine's
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
        amounts in the model currency.
    covenants : CovenantEngine
        Covenant engine whose specifications, windows, waivers and threshold
        schedules are evaluated.
    model : FinancialModelSpec or str
        Model, typed or as JSON, whose period calendar gives each result
        period's test date.

    Returns
    -------
    pd.DataFrame
        Long frame with one row per (test date, applicable covenant), dates in
        ascending period order and covenants in engine order, with the columns
        of ``CovenantEngine.evaluate_series``: ``as_of`` (ISO string),
        ``covenant`` (instance key), ``covenant_type``, ``passed``,
        ``actual_value``, ``threshold``, ``headroom``, ``details``. WASM
        returns the same reports as ``DatedCovenantReports`` rows.

    Raises
    ------
    KeyError
        If a metric an applicable covenant reads has no value in a period.
    ValueError
        If such a metric is non-finite, a result period is missing from the
        model calendar, the engine is invalid, or ``results`` / ``model`` JSON
        is malformed.

    Examples
    --------
    >>> from finstack_quant.covenants import CovenantEngine, cov_lite
    >>> from finstack_quant.statements import Evaluator, ModelBuilder
    >>> from finstack_quant.statements_analytics import evaluate_covenants
    >>> builder = ModelBuilder("borrower")
    >>> _ = builder.periods("2025Q1..Q2", None)
    >>> _ = builder.value("total_leverage", [("2025Q1", 6.0), ("2025Q2", 7.5)])
    >>> _ = builder.value("senior_leverage", [("2025Q1", 3.0), ("2025Q2", 3.5)])
    >>> model = builder.build()
    >>> engine = CovenantEngine.from_specs(cov_lite(7.0, 4.5))
    >>> frame = evaluate_covenants(Evaluator().evaluate(model), engine, model)
    >>> frame.loc[frame["covenant"] == "max_total_leverage", ["as_of", "passed"]].values.tolist()
    [['2025-03-31', True], ['2025-06-30', False]]

    """
    ...

def forecast_breaches(
    results: StatementResult | str,
    covenants: CovenantEngine,
    model: FinancialModelSpec | str,
    config: CovenantForecastConfig | None = None,
) -> list[FutureBreach]:
    """
    Forecast covenant breaches over every statement result period.

    Every period that carries a value in ``results`` is a test date (its last
    calendar day). Effective windows, waivers and threshold schedules are
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
    model : FinancialModelSpec or str
        Model, typed or as JSON, whose period calendar gives the test dates
        and the default horizon start.
    config : CovenantForecastConfig or None, optional
        Forecast policy; ``None`` means the deterministic, maintenance-scope
        default.

    Returns
    -------
    list[FutureBreach]
        Breaches sorted by breach date, then covenant id.

    Raises
    ------
    KeyError
        If a metric an active covenant reads has no value in a period.
    ValueError
        If such a metric is non-finite, a result period is missing from the
        model calendar, the engine or ``config`` is invalid (for example
        stochastic mode without ``volatility``), or ``results`` / ``model``
        JSON is malformed.

    Examples
    --------
    >>> from finstack_quant.covenants import CovenantEngine, CovenantForecastConfig, cov_lite
    >>> from finstack_quant.statements import Evaluator, ModelBuilder
    >>> from finstack_quant.statements_analytics import forecast_breaches
    >>> builder = ModelBuilder("borrower")
    >>> _ = builder.periods("2025Q1..Q2", None)
    >>> _ = builder.value("total_leverage", [("2025Q1", 6.0), ("2025Q2", 7.5)])
    >>> _ = builder.value("senior_leverage", [("2025Q1", 3.0), ("2025Q2", 3.5)])
    >>> model = builder.build()
    >>> engine = CovenantEngine.from_specs(cov_lite(7.0, 4.5))
    >>> config = CovenantForecastConfig().with_scope("incurrence")  # cov-lite tests are incurrence tests
    >>> breaches = forecast_breaches(Evaluator().evaluate(model), engine, model, config)
    >>> [(b.covenant_id, b.breach_date.isoformat()) for b in breaches]
    [('max_total_leverage', '2025-06-30')]

    """
    ...
