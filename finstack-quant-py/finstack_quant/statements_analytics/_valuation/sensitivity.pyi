"""Stub declarations mirroring ``finstack-quant-py/src/bindings/statements_analytics/valuation/sensitivity.rs``."""

from __future__ import annotations

from decimal import Decimal
import datetime
from collections.abc import Mapping, Sequence
from typing import Any, Literal, TypedDict
import pandas as pd
import polars as pl
from finstack_quant.statements import CheckReport, FinancialModelSpec, MonteCarloResults, StatementResult
from finstack_quant.core.currency import Currency
from finstack_quant.core.market_data import MarketContext
from finstack_quant.core.money import Money
from finstack_quant.core.table import ArrowTable
from finstack_quant.statements_analytics._checks import CreditMapping, ThreeStatementMapping
from finstack_quant.statements_analytics._credit_context import CreditContextMetrics
from finstack_quant.statements_analytics._typed.results import TornadoEntry
from finstack_quant.statements_analytics._valuation.corporate import wacc
from finstack_quant.statements_analytics._valuation.inputs import (
    CorporateDcfRequest,
    DcfInputs,
    DcfProjection,
    EquityBridge,
    StatementValuationCut,
    TerminalFcffBasis,
    TerminalValueSpec,
)

class DcfSensitivityGridSpec:
    """Two explicit ordered parameter axes for an enterprise-value grid.

    Examples
    --------
    >>> from finstack_quant.statements_analytics import DcfSensitivityGridSpec
    >>> isinstance(DcfSensitivityGridSpec([0.09, 0.11], [0.01, 0.02, 0.03]), DcfSensitivityGridSpec)
    True
    """

    def __init__(self, wacc_values: Sequence[float], terminal_values: Sequence[float]) -> None:
        """Validate two finite, nonempty axes with distinct levels.

        Parameters
        ----------
        wacc_values : Sequence of float
            Annual effective WACC decimal rates in row order.
        terminal_values : Sequence of float
            Stable growth decimal rates or enterprise exit multiple turns in column order.

        Raises
        ------
        ValueError
            For an empty axis, a nonfinite level or duplicate represented levels.
        """
        ...

class DcfSensitivityGridResult:
    """Rectangular native DCF results with marked unavailable endpoints.

    Examples
    --------
    >>> from finstack_quant.statements_analytics import DcfSensitivityGridResult
    >>> hasattr(DcfSensitivityGridResult, "to_pandas")
    True
    """

    @property
    def baseline_enterprise_value(self) -> Money:
        """Get the enterprise value at the original statement DCF assumptions.

        This method does not raise.

        Returns
        -------
        Money
            Baseline enterprise value in model currency.
        """
        ...

    def to_arrow(self) -> ArrowTable:
        """Present enterprise value as WACC rows and terminal-level columns as an ``ArrowTable``.

        Returns
        -------
        ArrowTable
            Requested row and column order with ``wacc`` followed by one float
            column per terminal level. Values are decimal currency amounts with
            null cells for levels outside the admitted DCF domain.

        Raises
        ------
        ValueError
            If the Arrow table cannot be constructed.
        """
        ...

    def to_pandas(self) -> pd.DataFrame:
        """Present enterprise value as WACC rows and terminal-level columns.

        Returns
        -------
        pd.DataFrame
            Requested row and column order. Values are decimal currency amounts
            with missing cells for levels outside the admitted DCF domain.

        Raises
        ------
        ImportError
            If pandas is unavailable.
        """
        ...

    def to_polars(self) -> pl.DataFrame:
        """Present enterprise value as WACC rows and terminal-level columns as a Polars ``DataFrame``.

        Returns
        -------
        polars.DataFrame
            Requested row and column order with ``wacc`` followed by one float
            column per terminal level.

        Raises
        ------
        ImportError
            If polars is unavailable.
        ValueError
            If the frame cannot be constructed.
        """
        ...

    def to_pyarrow(self) -> Any:
        """Present enterprise value as WACC rows and terminal-level columns as a ``pyarrow.Table``.

        Returns
        -------
        pyarrow.Table
            Requested row and column order with ``wacc`` followed by one float
            column per terminal level.

        Raises
        ------
        ImportError
            If pyarrow is unavailable.
        ValueError
            If the table cannot be constructed.
        """
        ...

    def __arrow_c_stream__(self, requested_schema: object | None = None) -> object: ...
    def __arrow_c_schema__(self) -> object: ...
    def _repr_html_(self) -> str | None: ...

def dcf_sensitivity_on_results(
    model: FinancialModelSpec,
    results: StatementResult,
    request: CorporateDcfRequest,
    shocks: _SensitivityShocks,
) -> _DcfShockReport:
    """Reprice independent changes in discount rate and terminal assumptions.

    Parameters
    ----------
    model : FinancialModelSpec
        Original model whose identity appears in the dated run.
    results : StatementResult
        Dated statement run with forecast cash and opening balances.
    request : CorporateDcfRequest
        Baseline assumptions, reporting currency and valuation cut.
    shocks : mapping
        Positive independent low and high distances. WACC and growth use
        decimal rates. Exit multiple shocks use turns for absolute shocks
        or decimal fractions of a positive baseline for relative shocks.

    Returns
    -------
    dict
        Baseline enterprise value and ranked pairs of requested endpoints.
        Each endpoint reports a signed change in the baseline currency or
        the reason the requested parameter level is outside its domain.

    Raises
    ------
    ValueError
        If the baseline is invalid or the shocks disagree with the terminal method.
    RuntimeError
        If an admissible repricing cannot be evaluated.

    Examples
    --------
    >>> from finstack_quant.statements_analytics import dcf_sensitivity_on_results
    >>> from datetime import date
    >>> from finstack_quant.core.market_data import MarketContext
    >>> from finstack_quant.core.money import Money
    >>> from finstack_quant.statements import Evaluator, ModelBuilder
    >>> from finstack_quant.statements_analytics import CorporateDcfRequest, DcfInputs, DcfProjection
    >>> from finstack_quant.statements_analytics import StatementValuationCut, TerminalFcffBasis, TerminalValueSpec
    >>> as_of = date(2024, 12, 31)
    >>> market = MarketContext()
    >>> b = ModelBuilder("qualified-dcf").periods("2025..2025").currency("USD")
    >>> _ = b.source_snapshot("forecast-at-2024-close")
    >>> _ = b.value_money("ufcf", [("2025", Money(100, "USD"))])
    >>> model = b.build()
    >>> evaluator = Evaluator()
    >>> results = evaluator.evaluate_with_market(model, market, as_of, as_of)
    >>> request = CorporateDcfRequest(
    ...     cut=StatementValuationCut(as_of, as_of, "USD"),
    ...     inputs=DcfInputs(wacc=0.10, terminal_value=TerminalValueSpec.gordon_growth(0.02), net_debt_override=50),
    ...     projection=DcfProjection(
    ...         terminal_date="2026-01-01", normalized_annual_fcff=TerminalFcffBasis.authored(Money(100, "USD"))
    ...     ),
    ... )
    >>> shocks = {
    ...     "wacc": {"low": "0.01", "high": "0.02"},
    ...     "terminal": {"parameter": "stable_growth", "shocks": {"low": "0.005", "high": "0.01"}},
    ... }
    >>> len(dcf_sensitivity_on_results(model, results, request, shocks)["pairs"])
    2
    """
    ...

def dcf_sensitivity_grid(
    model: FinancialModelSpec | str,
    results: StatementResult | str,
    request: CorporateDcfRequest,
    axes: DcfSensitivityGridSpec,
) -> DcfSensitivityGridResult:
    """Reprice a rectangular grid using one verified statement snapshot.

    Parameters
    ----------
    model : FinancialModelSpec or str
        Unchanged source model used to generate results.
    results : StatementResult or str
        The same statement run at the requested observation and valuation dates.
    request : CorporateDcfRequest
        Baseline assumptions and valuation horizon.
    axes : DcfSensitivityGridSpec
        WACC decimal rates as rows and stable growth rates or exit multiples as columns.

    Returns
    -------
    DcfSensitivityGridResult
        Enterprise values and signed changes against the baseline with unavailable cells.

    Raises
    ------
    ValueError
        If provenance, baseline or axes are invalid, or pricing an admitted endpoint fails.
    KeyError
        If an applicable baseline observation is absent.

    Examples
    --------
    >>> from finstack_quant.statements_analytics import dcf_sensitivity_grid
    >>> from datetime import date
    >>> from finstack_quant.core.market_data import MarketContext
    >>> from finstack_quant.core.money import Money
    >>> from finstack_quant.statements import Evaluator, ModelBuilder
    >>> from finstack_quant.statements_analytics import CorporateDcfRequest, DcfInputs, DcfProjection
    >>> from finstack_quant.statements_analytics import StatementValuationCut, TerminalFcffBasis, TerminalValueSpec
    >>> as_of = date(2024, 12, 31)
    >>> market = MarketContext()
    >>> b = ModelBuilder("qualified-dcf").periods("2025..2025").currency("USD")
    >>> _ = b.source_snapshot("forecast-at-2024-close")
    >>> _ = b.value_money("ufcf", [("2025", Money(100, "USD"))])
    >>> model = b.build()
    >>> evaluator = Evaluator()
    >>> results = evaluator.evaluate_with_market(model, market, as_of, as_of)
    >>> request = CorporateDcfRequest(
    ...     cut=StatementValuationCut(as_of, as_of, "USD"),
    ...     inputs=DcfInputs(wacc=0.10, terminal_value=TerminalValueSpec.gordon_growth(0.02), net_debt_override=50),
    ...     projection=DcfProjection(
    ...         terminal_date="2026-01-01", normalized_annual_fcff=TerminalFcffBasis.authored(Money(100, "USD"))
    ...     ),
    ... )
    >>> from finstack_quant.statements_analytics import DcfSensitivityGridSpec
    >>> axes = DcfSensitivityGridSpec([0.09, 0.10], [0.015, 0.02, 0.025])
    >>> dcf_sensitivity_grid(model, results, request, axes).to_pandas().shape
    (2, 3)
    """
    ...

class DcfDistributionResult:
    """Same-currency company values repriced from coherent statement samples.

    Examples
    --------
    >>> from finstack_quant.statements_analytics import DcfDistributionResult
    >>> hasattr(DcfDistributionResult, "quantiles_to_table")
    True
    """

    def to_arrow(self) -> ArrowTable:
        """Present one enterprise and equity valuation per retained path as an ``ArrowTable``.

        Returns
        -------
        ArrowTable
            Zero-based path ID (uint32), float presentation values in model
            currency, and the common ISO currency code in original path order.

        Raises
        ------
        ValueError
            If the table cannot be constructed.
        """
        ...

    def to_pandas(self) -> pd.DataFrame:
        """Present one enterprise and equity valuation per retained path.

        Returns
        -------
        pd.DataFrame
            Zero-based path ID, float presentation values in model currency,
            and the common ISO currency code in original path order.

        Raises
        ------
        ImportError
            If pandas is unavailable.
        ValueError
            If the frame cannot be constructed.
        """
        ...

    def to_polars(self) -> pl.DataFrame:
        """Present one enterprise and equity valuation per retained path as a Polars ``DataFrame``.

        Returns
        -------
        polars.DataFrame
            Zero-based path ID, float presentation values in model currency,
            and the common ISO currency code in original path order.

        Raises
        ------
        ImportError
            If polars is unavailable.
        ValueError
            If the frame cannot be constructed.
        """
        ...

    def to_pyarrow(self) -> Any:
        """Present one enterprise and equity valuation per retained path as a ``pyarrow.Table``.

        Returns
        -------
        pyarrow.Table
            Zero-based path ID, float presentation values in model currency,
            and the common ISO currency code in original path order.

        Raises
        ------
        ImportError
            If pyarrow is unavailable.
        ValueError
            If the table cannot be constructed.
        """
        ...

    def __arrow_c_stream__(self, requested_schema: object | None = None) -> object: ...
    def __arrow_c_schema__(self) -> object: ...
    def _repr_html_(self) -> str | None: ...
    def quantiles_to_table(self) -> ArrowTable:
        """Present R-7 sample quantiles of enterprise and common-equity value as an ``ArrowTable``.

        Returns
        -------
        ArrowTable
            One row per requested probability, with both monetary values
            expressed as floats in the reported ISO currency.

        Raises
        ------
        ValueError
            If the table cannot be constructed.
        """
        ...

def evaluate_dcf_monte_carlo(
    model: FinancialModelSpec | str,
    results: StatementResult | str,
    paths: MonteCarloResults,
    request: CorporateDcfRequest,
) -> DcfDistributionResult:
    """Reprice retained monetary forecasts using one complete sample at a time.

    Parameters
    ----------
    model : FinancialModelSpec or str
        The original model with the same identity as the baseline and simulation.
    results : StatementResult or str
        Verified baseline with the matching information cutoff and opening balances.
    paths : MonteCarloResults
        Simulation that retained full samples for each selected forecast cash-flow node.
    request : CorporateDcfRequest
        Common valuation cutoff, reporting currency, WACC and terminal basis.

    Returns
    -------
    DcfDistributionResult
        Same-currency per-path returns and quantiles in the simulation's requested order.

    Raises
    ------
    ValueError
        If the statement run or cut differs, required paths are missing, or a path cannot price.

    Examples
    --------
    >>> from finstack_quant.statements_analytics import evaluate_dcf_monte_carlo
    >>> from datetime import date
    >>> from finstack_quant.core.market_data import MarketContext
    >>> from finstack_quant.core.money import Money
    >>> from finstack_quant.statements import Evaluator, ModelBuilder
    >>> from finstack_quant.statements_analytics import CorporateDcfRequest, DcfInputs, DcfProjection
    >>> from finstack_quant.statements_analytics import StatementValuationCut, TerminalFcffBasis, TerminalValueSpec
    >>> as_of = date(2024, 12, 31)
    >>> market = MarketContext()
    >>> b = ModelBuilder("qualified-dcf").periods("2025..2025").currency("USD")
    >>> _ = b.source_snapshot("forecast-at-2024-close")
    >>> _ = b.value_money("ufcf", [("2025", Money(100, "USD"))])
    >>> model = b.build()
    >>> evaluator = Evaluator()
    >>> results = evaluator.evaluate_with_market(model, market, as_of, as_of)
    >>> request = CorporateDcfRequest(
    ...     cut=StatementValuationCut(as_of, as_of, "USD"),
    ...     inputs=DcfInputs(wacc=0.10, terminal_value=TerminalValueSpec.gordon_growth(0.02), net_debt_override=50),
    ...     projection=DcfProjection(
    ...         terminal_date="2026-01-01", normalized_annual_fcff=TerminalFcffBasis.authored(Money(100, "USD"))
    ...     ),
    ... )
    >>> from finstack_quant.statements import MonteCarloConfig
    >>> config = MonteCarloConfig(3, 7, ["ufcf"], [0.5], include_path_data=True)
    >>> samples = evaluator.evaluate_monte_carlo(model, config, market, as_of, as_of)
    >>> len(evaluate_dcf_monte_carlo(model, results, samples, request).to_pandas())
    3
    """
    ...

class CorporateValuationResult:
    """DCF outputs in the model currency.

    Examples
    --------
    >>> from finstack_quant.statements_analytics import CorporateValuationResult
    >>> r = CorporateValuationResult.from_json(
    ...     '{"equity_value":{"amount":"90","currency":"USD"},"enterprise_value":{"amount":"100","currency":"USD"},'
    ...     '"net_debt":{"amount":"10","currency":"USD"},"other_equity_claims":{"amount":"0","currency":"USD"},"valuation_discount":{"amount":"0","currency":"USD"},'
    ...     '"terminal_value_pv":{"amount":"60","currency":"USD"},'
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
        Interest-bearing debt less cash in model currency.

        This property does not raise.

        Returns
        -------
        Money
            Interest-bearing debt less cash in model currency.
        """
    @property
    def other_equity_claims(self) -> Money:
        """Return net non-debt claims deducted from enterprise value.

        This property does not raise.

        Returns
        -------
        Money
            Net preferred, minority and other claims after non-operating
            assets and named adjustments in model currency. A negative
            amount increases common equity.
        """
    @property
    def valuation_discount(self) -> Money:
        """
        Total valuation discount (DLOM, DLOC, other) deducted after the
        EV-to-equity bridge; zero without ``valuation_discounts``.
        ``enterprise_value - net_debt - other_equity_claims - valuation_discount`` equals
        ``equity_value``.

        This property does not raise.

        Returns
        -------
        Money
            Discount amount in the model currency.
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
    @property
    def wacc(self) -> float | None:
        """
        Discount rate (WACC) applied to every explicit flow and to the terminal
        value, as an annually compounded decimal (``0.10`` = 10%).

        This property does not raise.

        Returns
        -------
        float | None
            The WACC, or ``None`` only for a result loaded from JSON written before the DCF working was recorded.
        """
    @property
    def periods(self) -> list[dict[str, Any]]:
        """
        Explicit forecast periods that were discounted, in date order.

        Each row is a dict with ``period_id`` (str), ``date`` (ISO date string,
        the last day of the period), ``free_cash_flow``, ``discount_years``
        (ACT/365.25 from the valuation date, less the mid-year shift when the
        mid-year convention is on), ``discount_factor``
        (``(1 + wacc) ** -discount_years``) and ``present_value``
        (``free_cash_flow / (1 + wacc) ** discount_years``). Amounts are
        unrounded floats in the model currency. The ``present_value`` column
        sums to ``pv_explicit``, and that sum plus ``terminal_value_pv`` is
        ``enterprise_value`` (to within one minor currency unit, because the
        ``Money`` totals are rounded).

        This property does not raise.

        Returns
        -------
        list[dict[str, Any]]
            One dict per explicit period; empty only for a result loaded from JSON written before the DCF working was recorded.
        """
    @property
    def pv_explicit(self) -> Money | None:
        """
        Present value of the explicit forecast flows: the sum of the
        ``present_value`` of ``periods``.

        This property does not raise.

        Returns
        -------
        Money | None
            Explicit-period present value in the model currency, or ``None`` only for a result loaded from JSON written before the DCF working was recorded.
        """
    @property
    def terminal_value(self) -> Money | None:
        """
        Terminal value at the horizon date, before discounting.

        ``terminal_value / (1 + wacc) ** terminal_discount_years`` equals
        ``terminal_value_pv``.

        This property does not raise.

        Returns
        -------
        Money | None
            Undiscounted terminal value in the model currency, or ``None`` only for a result loaded from JSON written before the DCF working was recorded.
        """
    @property
    def terminal_discount_years(self) -> float | None:
        """
        Discounting tenor of the terminal value in years (ACT/365.25 from the
        valuation date to the last explicit flow date).

        An exit-multiple terminal value uses the full tenor; a
        growth-perpetuity terminal value uses the mid-year-adjusted tenor when
        the mid-year convention is on.

        This property does not raise.

        Returns
        -------
        float | None
            Tenor in years, or ``None`` only for a result loaded from JSON written before the DCF working was recorded.
        """
    @property
    def equity_bridge(self) -> EquityBridge | None:
        """
        EV-to-equity bridge whose net adjustment is ``net_debt``: gross debt
        (``total_debt``) and ``cash`` separately, plus preferred equity,
        minority interest, non-operating assets and other adjustments, in the
        model currency.

        ``total_debt - cash`` equals ``net_debt`` unless the ``equity_bridge``
        option supplied further components. A flat ``net_debt_override`` is
        reported as ``total_debt`` with ``cash`` zero.

        This property does not raise.

        Returns
        -------
        EquityBridge | None
            The bridge, or ``None`` only for a result loaded from JSON written before the DCF working was recorded.
        """
    def to_arrow(self) -> ArrowTable:
        """
        Export as a single-row ``ArrowTable``.

        Columns: ``currency``, ``equity_value``, ``enterprise_value``,
        ``net_debt``, ``other_equity_claims``, ``valuation_discount``, ``terminal_value_pv`` (float
        amounts in ``currency``), ``equity_value_per_share``,
        ``diluted_shares`` (null when absent), then the DCF working:
        ``wacc`` (decimal), ``pv_explicit``, ``terminal_value``,
        ``total_debt``, ``cash`` (float amounts in ``currency``) and
        ``terminal_discount_years``; these six are null for a result
        loaded from JSON written before they were recorded.

        Returns
        -------
        ArrowTable
            One row summarizing corporate valuation metrics.

        Raises
        ------
        ValueError
            If the result cannot be converted to an Arrow table.
        """
    def to_pandas(self) -> pd.DataFrame:
        """
        Export as a single-row pandas ``DataFrame``.

        Columns match :meth:`to_arrow`.

        Returns
        -------
        pd.DataFrame
            One row summarizing corporate valuation metrics.

        Raises
        ------
        ImportError
            If pandas is not installed in the running interpreter.
        ValueError
            If the result cannot be converted to a pandas DataFrame.
        """
    def to_polars(self) -> pl.DataFrame:
        """
        Export as a single-row Polars ``DataFrame``.

        Returns
        -------
        polars.DataFrame
            One row summarizing corporate valuation metrics.

        Raises
        ------
        ImportError
            If polars is not installed in the running interpreter.
        ValueError
            If the result cannot be converted to a Polars DataFrame.
        """
    def to_pyarrow(self) -> Any:
        """
        Export as a single-row ``pyarrow.Table``.

        Returns
        -------
        pyarrow.Table
            One row summarizing corporate valuation metrics.

        Raises
        ------
        ImportError
            If pyarrow is not installed in the running interpreter.
        ValueError
            If the result cannot be converted to a PyArrow table.
        """
    def __arrow_c_stream__(self, requested_schema: object | None = None) -> object: ...
    def __arrow_c_schema__(self) -> object: ...
    def _repr_html_(self) -> str | None: ...
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
    >>> list(r.to_pandas()["swing"])
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
    def to_arrow(self) -> ArrowTable:
        """
        Export the tornado table as an ``ArrowTable``.

        Columns: ``parameter_id``, ``downside``, ``upside``, ``swing`` (EV
        deltas in the baseline currency). One row per entry, in ranked order.

        Returns
        -------
        ArrowTable
            Ranked tornado sensitivity table.

        Raises
        ------
        ValueError
            If the result cannot be converted to an Arrow table.
        """
    def to_pandas(self) -> pd.DataFrame:
        """
        Export the tornado table as a pandas ``DataFrame``.

        Columns match :meth:`to_arrow`. One row per entry, in ranked order.

        Returns
        -------
        pd.DataFrame
            Ranked tornado sensitivity DataFrame.

        Raises
        ------
        ImportError
            If pandas is not installed in the running interpreter.
        ValueError
            If the result cannot be converted to a pandas DataFrame.
        """
    def to_polars(self) -> pl.DataFrame:
        """
        Export the tornado table as a Polars ``DataFrame``.

        Returns
        -------
        polars.DataFrame
            Ranked tornado sensitivity Polars frame.

        Raises
        ------
        ImportError
            If polars is not installed in the running interpreter.
        ValueError
            If the result cannot be converted to a Polars DataFrame.
        """
    def to_pyarrow(self) -> Any:
        """
        Export the tornado table as a ``pyarrow.Table``.

        Returns
        -------
        pyarrow.Table
            Ranked tornado sensitivity PyArrow table.

        Raises
        ------
        ImportError
            If pyarrow is not installed in the running interpreter.
        ValueError
            If the result cannot be converted to a PyArrow table.
        """
    def __arrow_c_stream__(self, requested_schema: object | None = None) -> object: ...
    def __arrow_c_schema__(self) -> object: ...
    def _repr_html_(self) -> str | None: ...
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

class _RateShocks(TypedDict):
    low: str | Decimal
    high: str | Decimal

class _MultipleShocks(_RateShocks):
    kind: Literal["absolute", "relative"]

class _StableGrowthShock(TypedDict):
    parameter: Literal["stable_growth"]
    shocks: _RateShocks

class _ExitMultipleShock(TypedDict):
    parameter: Literal["exit_multiple"]
    shocks: _MultipleShocks

class _SensitivityShocks(TypedDict):
    wacc: _RateShocks
    terminal: _StableGrowthShock | _ExitMultipleShock

class _MoneyWire(TypedDict):
    amount: str
    currency: str

class _PricedEndpoint(TypedDict):
    status: Literal["priced"]
    level: float
    enterprise_value: _MoneyWire
    change: _MoneyWire

class _UnavailableEndpoint(TypedDict):
    status: Literal["unavailable"]
    level: float
    reason: str

class _ShockPair(TypedDict):
    parameter_id: str
    low: _PricedEndpoint | _UnavailableEndpoint
    high: _PricedEndpoint | _UnavailableEndpoint

class _DcfShockReport(TypedDict):
    baseline_enterprise_value: _MoneyWire
    pairs: list[_ShockPair]
