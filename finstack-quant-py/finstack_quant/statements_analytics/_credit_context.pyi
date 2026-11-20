"""Dated borrower credit measurements and typed corporate credit metrics."""

from collections.abc import Sequence
import datetime
from typing import Literal, TypedDict

import pandas as pd
from finstack_quant.core.money import Money
from finstack_quant.statements import StatementResult

_Measure = Literal[
    "cash_dscr",
    "cash_and_pik_dscr",
    "cash_dscr_with_fees",
    "interest_coverage",
    "loan_to_value",
    "debt_to_enterprise_value",
]

class _ReferenceValue(TypedDict):
    measure: Literal["loan_to_value", "debt_to_enterprise_value"]
    date: datetime.date | str
    amount: Money
    source_id: str

class CreditContextSpec:
    """Select one borrowing, one statement run, and explicitly named credit ratios.

    Examples
    --------
    >>> from finstack_quant.statements_analytics import CreditContextSpec
    >>> request = CreditContextSpec(
    ...     borrowing_id="term",
    ...     run_id="approved-run",
    ...     currency="USD",
    ...     periods=["2025Q1"],
    ...     measures=["cash_dscr"],
    ...     cfads_node="cfads",
    ...     reference_values=[],
    ... )
    >>> isinstance(request, CreditContextSpec)
    True
    """

    def __init__(
        self,
        *,
        borrowing_id: str,
        run_id: str,
        currency: str,
        periods: Sequence[str],
        measures: Sequence[_Measure],
        reference_values: Sequence[_ReferenceValue],
        cfads_node: str | None = None,
        earnings_node: str | None = None,
        recognized_interest_node: str | None = None,
        cash_fees_node: str | None = None,
    ) -> None:
        """Author the reporting scope and monetary sources the selected ratios need.

        Parameters
        ----------
        borrowing_id : str
            Contractual identifier of the borrowing in the evaluated capital structure.
        run_id : str
            Content identity of the point-in-time statement and capital cashflows.
        currency : str
            ISO denomination of all selected ratio operands.
        periods : sequence of str
            Explicit unique period labels belonging to the identified statement run.
        measures : sequence of labels
            Distinct credit measures in reporting order.
        reference_values : sequence of mappings
            Sourced borrowing-scoped positive collateral or enterprise values in the
            reporting currency, each matched to its particular closing date.
        cfads_node : str or None
            Same-currency cash flow available for debt service, required for all DSCR variants.
        earnings_node : str or None
            Same-currency earnings for interest coverage.
        recognized_interest_node : str or None
            Same-basis recognized interest, including PIK, for interest coverage.
        cash_fees_node : str or None
            Elected cash fees for the fee-inclusive DSCR denominator.

        Raises
        ------
        ValueError
            If a supplied field does not match the native wire type.
        """
        ...

class CreditContextReport:
    """Dated credit measurements and coverage for an identified borrowing.

    Examples
    --------
    >>> from finstack_quant.statements_analytics import CreditContextReport
    >>> hasattr(CreditContextReport, "rows")
    True
    """

    @property
    def borrowing_id(self) -> str:
        """Get the admitted borrowing's contractual identity.

        This property does not raise.

        Returns
        -------
        str
            Borrowing ID from the evaluated capital structure.
        """
        ...

    @property
    def rows(self) -> list[CreditMeasureRow]:
        """Get every selected period and measure with its availability.

        This property does not raise.

        Returns
        -------
        list[CreditMeasureRow]
            All elected pairs in report order, including pairs with missing evidence.
        """
        ...

    def to_dataframe(self) -> pd.DataFrame:
        """Present dated ratios and their coverage status in a long frame.

        Returns
        -------
        pd.DataFrame
            Period, measure, status, ratio, reference source ID and explanation.
            An unknown ratio is missing in the frame.

        Raises
        ------
        ImportError
            If pandas is unavailable.
        ValueError
            If result data cannot be shaped into a frame.
        """
        ...

    def coverage_to_dataframe(self) -> pd.DataFrame:
        """Get the observation coverage and qualified minima by measure.

        Returns
        -------
        pd.DataFrame
            Rows show measured, inapplicable and unavailable counts, the observed minimum over measured rows, and the full-path minimum when every applicable period is known.

        Raises
        ------
        ImportError
            If pandas is unavailable.
        ValueError
            If result data cannot be shaped into the requested frame.
        """
        ...

class CreditMeasureRow:
    """One ratio observation, preserving the exact monetary operands and dates.

    Examples
    --------
    >>> from finstack_quant.statements_analytics import CreditMeasureRow
    >>> hasattr(CreditMeasureRow, "numerator")
    True
    """

    @property
    def period(self) -> str:
        """Get the observation's period.

        This property does not raise.

        Returns
        -------
        str
            Exact reporting period label of the selected observation.
        """
        ...

    @property
    def measure(self) -> _Measure:
        """Get the observation's measure.

        Returns
        -------
        _Measure
            Canonical name of the elected financial measure.

        Raises
        ------
        ValueError
            If the measure cannot be encoded for Python.
        """
        ...

    @property
    def date(self) -> datetime.date:
        """Get the observation's date.

        Returns
        -------
        date
            Last calendar day included in the reporting period.

        Raises
        ------
        OverflowError
            If the date cannot be represented by Python.
        """
        ...

    @property
    def ratio(self) -> float | None:
        """Get the observation's ratio.

        This property does not raise.

        Returns
        -------
        float | None
            Unitless ratio, or None if unavailable or inapplicable.
        """
        ...

    @property
    def numerator(self) -> Money | None:
        """Get the observation's numerator.

        This property does not raise.

        Returns
        -------
        Money | None
            Exact signed monetary numerator, or None if unmeasured.
        """
        ...

    @property
    def denominator(self) -> Money | None:
        """Get the observation's denominator.

        This property does not raise.

        Returns
        -------
        Money | None
            Exact positive monetary denominator, or None if unmeasured.
        """
        ...

    @property
    def reason(self) -> str | None:
        """Get the observation's reason.

        This property does not raise.

        Returns
        -------
        str | None
            Reason the measure is unavailable, or None otherwise.
        """
        ...

    @property
    def reference_source_id(self) -> str | None:
        """Get the observation's reference source id.

        This property does not raise.

        Returns
        -------
        str | None
            Identity of the matched collateral or enterprise valuation, when applicable.
        """
        ...

class CreditContextMetrics:
    """Per-instrument coverage and leverage series in a corporate analysis result.

    Examples
    --------
    >>> from finstack_quant.statements_analytics import CreditContextMetrics
    >>> hasattr(CreditContextMetrics, "dscr")
    True
    """

    @property
    def dscr(self) -> list[tuple[str, float]]:
        """Get the observation's dscr.

        This property does not raise.

        Returns
        -------
        list[tuple[str, float]]
            Ordered period labels and cash debt service coverage observations as decimal ratios.
        """
        ...

    @property
    def dscr_min(self) -> float | None:
        """Get the observation's dscr min.

        This property does not raise.

        Returns
        -------
        float | None
            Observed minimum cash debt service coverage as a decimal ratio, or None if empty.
        """
        ...

    @property
    def dscr_total(self) -> list[tuple[str, float]]:
        """Get the observation's dscr total.

        This property does not raise.

        Returns
        -------
        list[tuple[str, float]]
            Ordered period labels and cash and PIK debt service coverage observations as decimal ratios.
        """
        ...

    @property
    def dscr_total_min(self) -> float | None:
        """Get the observation's dscr total min.

        This property does not raise.

        Returns
        -------
        float | None
            Observed minimum cash and PIK debt service coverage as a decimal ratio, or None if empty.
        """
        ...

    @property
    def dscr_incl_fees(self) -> list[tuple[str, float]]:
        """Get the observation's dscr incl fees.

        This property does not raise.

        Returns
        -------
        list[tuple[str, float]]
            Ordered period labels and cash and fee-inclusive debt service coverage observations as decimal ratios.
        """
        ...

    @property
    def dscr_incl_fees_min(self) -> float | None:
        """Get the observation's dscr incl fees min.

        This property does not raise.

        Returns
        -------
        float | None
            Observed minimum cash and fee-inclusive debt service coverage as a decimal ratio, or None if empty.
        """
        ...

    @property
    def interest_coverage(self) -> list[tuple[str, float]]:
        """Get the observation's interest coverage.

        This property does not raise.

        Returns
        -------
        list[tuple[str, float]]
            Ordered period labels and recognized interest coverage observations as decimal ratios.
        """
        ...

    @property
    def interest_coverage_min(self) -> float | None:
        """Get the observation's interest coverage min.

        This property does not raise.

        Returns
        -------
        float | None
            Observed minimum recognized interest coverage as a decimal ratio, or None if empty.
        """
        ...

    @property
    def ltv(self) -> list[tuple[str, float]]:
        """Get the observation's ltv.

        This property does not raise.

        Returns
        -------
        list[tuple[str, float]]
            Ordered period labels and relative closing leverage observations as decimal ratios.
        """
        ...

    @property
    def skipped_periods(self) -> list[str]:
        """Get the observation's skipped periods.

        This property does not raise.

        Returns
        -------
        list[str]
            Selected periods that lacked one or more denominator or numerator observations.
        """
        ...

    def to_dataframe(self) -> pd.DataFrame:
        """Present the available ratios for this borrowing by period.

        Returns
        -------
        pd.DataFrame
            Ordered reporting periods with one column per ratio, including LTV as a decimal fraction. Missing ratio observations appear as missing values.

        Raises
        ------
        ImportError
            If pandas is unavailable.
        ValueError
            If result data cannot be shaped into the requested frame.
        """
        ...

def evaluate_credit_context(results: StatementResult, spec: CreditContextSpec) -> CreditContextReport:
    """Calculate one borrowing's selected credit ratios on one statement run.

    Parameters
    ----------
    results : StatementResult
        Point-in-time evaluation holding the borrowing's capital cashflow evidence.
    spec : CreditContextSpec
        Matching source identity, dates, currency, borrowing and named ratio inputs.

    Returns
    -------
    CreditContextReport
        Measured, unavailable, and inapplicable ratios for the elected periods.

    Raises
    ------
    ValueError
        If source identity, scope, dates, selected nodes or monetary units are invalid.

    Examples
    --------
    >>> from finstack_quant.statements import Evaluator, ModelBuilder
    >>> from finstack_quant.statements_analytics import CreditContextSpec, evaluate_credit_context
    >>> b = ModelBuilder("borrower").periods("2025..2025").currency("USD")
    >>> _ = b.value_scalar("cfads", {"2025": 100})
    >>> request = CreditContextSpec(
    ...     borrowing_id="loan",
    ...     run_id="missing-cut",
    ...     currency="USD",
    ...     periods=["2025"],
    ...     measures=["cash_dscr"],
    ...     cfads_node="cfads",
    ...     reference_values=[],
    ... )
    >>> evaluate_credit_context(Evaluator().evaluate(b.build()), request)
    Traceback (most recent call last):
    ValueError: ...
    """
    ...
