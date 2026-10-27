"""Stub declarations mirroring ``finstack-quant-py/src/bindings/statements_analytics/reports.rs``."""

from __future__ import annotations

from typing import Any
import pandas as pd

class CreditAssessment:
    """Structured credit assessment: leverage, coverage and free cash flow at a
    period plus the ascending per-period series.

    Ratios are ``None`` when their inputs have mixed scalar/money
    representations, different currencies, or non-finite arithmetic.
    Trailing-year ratios require all annual, semiannual, quarterly or monthly
    periods, including fiscal periods, or the complete Gregorian daily year
    ending after the assessment date. Weekly and fiscal-daily ratios are
    unavailable because the result does not contain their date calendar.

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
        Ratios are ``None`` when their inputs have mixed scalar/money
        representations, different currencies, or non-finite arithmetic.
        Trailing-year ratios require all annual, semiannual, quarterly or monthly
        periods, including fiscal periods, or the complete Gregorian daily year
        ending after the assessment date. Weekly and fiscal-daily ratios are
        unavailable because the result does not contain their date calendar.

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
        Formatted credit assessment report text. Trailing-year ratios require a
        complete annual, semiannual, quarterly, monthly or Gregorian daily year;
        weekly and fiscal-daily ratios are reported as unavailable.

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
