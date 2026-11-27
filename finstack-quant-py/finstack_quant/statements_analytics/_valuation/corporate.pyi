"""Stub declarations mirroring ``finstack-quant-py/src/bindings/statements_analytics/valuation/corporate.rs``."""

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
from finstack_quant.statements_analytics._valuation.inputs import (
    DcfInputs,
    DcfOptions,
    EquityBridge,
    TerminalValueSpec,
    ValuationDiscounts,
)
from finstack_quant.statements_analytics._valuation.sensitivity import CorporateValuationResult, DcfSensitivityResult

class LboCheckMappings:
    """
    Node mappings that switch on the LBO model check suite.

    The constructor stores the supplied values and does not raise.

    Parameters
    ----------
    three_statement : ThreeStatementMapping | dict
        Balance-sheet / income / cash-flow node mapping.
    credit : CreditMapping | dict
        Leverage and coverage node mapping.

    Examples
    --------
    >>> from finstack_quant.statements_analytics import CreditMapping, LboCheckMappings, ThreeStatementMapping
    >>> three = ThreeStatementMapping(
    ...     cash_node="cash",
    ...     retained_earnings_node="re",
    ...     net_income_node="ni",
    ...     assets_nodes=["assets"],
    ...     liabilities_nodes=["liabilities"],
    ...     equity_nodes=["equity"],
    ... )
    >>> credit = CreditMapping(debt_node="debt", ebitda_node="ebitda", interest_expense_node="interest")
    >>> m = LboCheckMappings(three, credit)
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
        Entry metric read from the explicitly selected entry period.

        This property does not raise.

        Returns
        -------
        float
            Entry metric read from the explicitly selected entry period.
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
    def to_arrow(self) -> ArrowTable:
        """
        Export as a single-row ``ArrowTable``.

        Columns: ``currency``, ``entry_enterprise_value``, ``entry_metric``,
        ``debt_total``, ``equity_check``, ``sources_total``, ``uses_total``,
        ``sources_uses_balanced``, ``exit_enterprise_value``, ``exit_metric``,
        ``exit_net_debt``, ``exit_equity_proceeds``, ``moic``. Money columns are
        float amounts in ``currency``.

        Returns
        -------
        ArrowTable
            One row summarizing LBO returns and balance checks.

        Raises
        ------
        ValueError
            If the result cannot be converted to an Arrow table.
        """
    def to_pandas(self) -> pd.DataFrame:
        """
        Export as a single-row pandas ``DataFrame``.

        Columns match :meth:`to_arrow`. Money columns are float amounts in
        ``currency``.

        Returns
        -------
        pd.DataFrame
            One row summarizing LBO returns and balance checks.

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
            One row summarizing LBO returns and balance checks.

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
            One row summarizing LBO returns and balance checks.

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
    >>> analysis.equity is None
    True
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
    def credit(self) -> dict[str, CreditContextMetrics]:
        """Get the per-instrument credit coverage and leverage observations.

        This property does not raise.

        Returns
        -------
        dict[str, CreditContextMetrics]
            Instrument IDs mapped to their typed period series and minima.
        """
        ...
    def to_arrow(self) -> ArrowTable:
        """
        Export the per-instrument credit metrics as a long ``ArrowTable``.

        Columns: ``instrument_id``, ``period`` (period-id string), ``dscr``,
        ``dscr_total``, ``dscr_incl_fees``, ``interest_coverage`` (turns;
        null where a metric is not available for that period). One row per
        (instrument, period) present on any metric series.

        Returns
        -------
        ArrowTable
            Long-format per-instrument credit metrics table.

        Raises
        ------
        ValueError
            If the result cannot be converted to an Arrow table.
        """
    def to_pandas(self) -> pd.DataFrame:
        """
        Export the per-instrument credit metrics as a long pandas ``DataFrame``.

        Columns match :meth:`to_arrow`.

        Returns
        -------
        pd.DataFrame
            Long-format per-instrument credit metrics DataFrame.

        Raises
        ------
        ImportError
            If pandas is not installed in the running interpreter.
        ValueError
            If the result cannot be converted to a pandas DataFrame.
        """
    def to_polars(self) -> pl.DataFrame:
        """
        Export the per-instrument credit metrics as a long Polars ``DataFrame``.

        Returns
        -------
        polars.DataFrame
            Long-format per-instrument credit metrics Polars frame.

        Raises
        ------
        ImportError
            If polars is not installed in the running interpreter.
        ValueError
            If the result cannot be converted to a Polars DataFrame.
        """
    def to_pyarrow(self) -> Any:
        """
        Export the per-instrument credit metrics as a long ``pyarrow.Table``.

        Returns
        -------
        pyarrow.Table
            Long-format per-instrument credit metrics PyArrow table.

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

def evaluate_dcf(
    model: FinancialModelSpec,
    inputs: Mapping[str, Any],
    options: Mapping[str, Any] | None = None,
    market: MarketContext | None = None,
    as_of: Any | None = None,
) -> CorporateValuationResult:
    """Evaluate DCF valuation on a financial model.

    Parameters
    ----------
    model : FinancialModelSpec
        A ``FinancialModelSpec``; metadata must contain
        a ``"currency"`` key.
    inputs : dict
        The Rust ``DcfInputs`` serde form (the document WASM ``evaluateDcf``
        takes as ``inputs``), as a dict. Unknown keys raise
        ``ValueError``:

        * ``wacc`` (float, required): weighted average cost of capital in
          decimal form (``0.10`` = 10%).
        * ``terminal_value`` (TerminalValueSpec | dict, required): terminal
          value method, typed or as its tagged serde form such as
          ``{"type": "gordon_growth", "stable_growth_rate": 0.02}``.
        * ``ufcf_node`` (str, optional): node id containing monetary unlevered
          free cash flow in model currency; defaults to the Rust
          ``DEFAULT_UFCF_NODE`` (``"ufcf"``). Gordon Growth / H-Model terminal
          cash flow requires a complete contiguous calendar year at the end of
          the forecast.
        * ``net_debt_override`` (float | None, optional): flat net-debt amount
          in model currency. Without an override (the default), debt and cash
          must be monetary in model currency and available at a period ending
          on or before the inclusive valuation date; future balances are
          rejected.
    options : dict | None
        The Rust ``DcfOptions`` serde form. Every key is optional and a
        missing one takes its Rust default: ``mid_year_convention``
        (``False``), ``max_stable_growth_rate`` (``0.05``),
        ``shares_outstanding``, ``equity_bridge`` / ``valuation_discounts``
        (serde dicts or the typed ``EquityBridge`` / ``ValuationDiscounts``),
        ``exit_multiple_metric_node`` (statement monetary flow node whose
        complete trailing-year sum replaces ``terminal_metric`` on an
        exit-multiple terminal; actual period boundaries must cover one
        complete contiguous calendar year) and the sensitivity knobs. ``None`` uses every default.
    market : MarketContext | None
        Market context used for statement evaluation (capital-structure curve
        lookups); DCF discounting stays WACC-only. Requires ``as_of``.
    as_of : datetime.date | str | None
        DCF valuation date and, with ``market``, the statement visibility and
        market-data date. Defaults to the first forecast boundary. As a
        visibility cutoff it hides each actual whose availability date (by
        default the period's exclusive end, the first day of the next period) is
        after it: a quarter-end ``"2025-03-31"`` hides 2025Q1's actuals, so pass
        ``"2025-04-01"`` or set the nodes' ``availability_dates``. Only forecast
        periods ending after ``as_of`` are discounted.

    Returns
    -------
    CorporateValuationResult
        ``equity_value``, ``enterprise_value``, ``net_debt`` and
        ``terminal_value_pv`` as ``Money``; per-share values as floats; and
        the DCF working (``wacc``, ``periods``, ``pv_explicit``,
        ``terminal_value``, ``terminal_discount_years``, ``equity_bridge``).

    Raises
    ------
    ValueError
        If ``market`` is set without ``as_of``, ``inputs`` misses ``wacc`` or
        ``terminal_value``, ``inputs`` or ``options`` is malformed (an unknown
        key included), ``as_of`` hides an actual that a value node needs, no
        forecast cash flow ends after ``as_of``, or the model, cash-flow node,
        exit-multiple node or DCF inputs are invalid.
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
    >>> _ = b.with_meta("currency", "USD")
    >>> inputs = {
    ...     "wacc": 0.10,
    ...     "terminal_value": TerminalValueSpec.gordon_growth(0.02),
    ...     "net_debt_override": 0.0,
    ... }
    >>> result = evaluate_dcf(b.build(), inputs)
    >>> str(result.enterprise_value.currency)
    'USD'
    """

def dcf_sensitivity(
    model: FinancialModelSpec,
    wacc: float,
    terminal_value: Any,
    ufcf_node: str = "ufcf",
    net_debt_override: float | None = None,
    options: Mapping[str, Any] | None = None,
) -> DcfSensitivityResult:
    """Rank the headline DCF assumptions by enterprise-value impact.

    The statement model is evaluated once; each shocked point re-runs only the
    DCF. Entries are EV deltas versus the baseline, sorted by descending
    absolute swing.

    Parameters
    ----------
    model : FinancialModelSpec
        A ``FinancialModelSpec``; metadata must include
        a ``"currency"`` key.
    wacc : float
        Baseline WACC in decimal form.
    terminal_value : TerminalValueSpec | dict
        Terminal value method; selects whether the growth rate or the exit
        multiple is shocked.
    ufcf_node : str
        Node id containing monetary unlevered free cash flow in model currency.
        Defaults to the Rust ``DEFAULT_UFCF_NODE`` (``"ufcf"``). Gordon Growth /
        H-Model terminal cash flow requires a complete contiguous calendar year
        at the end of the forecast.
    net_debt_override : float | None
        Flat net-debt amount in model currency. Without an override, debt and
        cash must be monetary in model currency and available at a period
        ending on or before the inclusive valuation date; future balances are
        rejected.
    options : dict | None
        The Rust ``DcfOptions`` serde form (the document WASM
        ``dcfSensitivity`` takes as ``optionsJson``). Every key is optional
        and a missing one takes its Rust default: ``wacc_sensitivity_bump``
        (``0.01`` = +/-100bp on WACC and terminal growth),
        ``wacc_denominator_epsilon`` (``0.005``), ``max_stable_growth_rate``
        (``0.05``), ``exit_multiple_bump`` (``{"absolute": 1.0}`` turns or
        ``{"relative": 0.10}``), ``mid_year_convention`` (``False``),
        ``exit_multiple_metric_node`` (statement monetary flow node supplying
        the complete trailing-year terminal metric), ``equity_bridge``,
        ``shares_outstanding`` and ``valuation_discounts``. Typed
        ``EquityBridge`` / ``ValuationDiscounts`` values may sit in place of
        their dicts. ``None`` uses every default.

    Returns
    -------
    DcfSensitivityResult
        Baseline EV, ranked ``entries`` and the clamped-shock flags;
        ``to_pandas()`` or ``to_arrow()`` gives the tornado table.

    Raises
    ------
    ValueError
        If a payload or ``options`` is malformed (an unknown ``options`` key
        included), or the model or DCF inputs are invalid (including an
        exit-multiple node with insufficient or noncontiguous history).
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
    >>> _ = b.with_meta("currency", "USD")
    >>> sens = dcf_sensitivity(
    ...     b.build(),
    ...     0.10,
    ...     TerminalValueSpec.gordon_growth(0.02),
    ...     net_debt_override=0.0,
    ...     options={"wacc_sensitivity_bump": 0.02},
    ... )
    >>> list(sens.to_pandas().columns)
    ['parameter_id', 'downside', 'upside', 'swing']
    """

def evaluate_lbo(model: FinancialModelSpec, config: Mapping[str, Any]) -> LboResult:
    """Evaluate a leveraged-buyout transaction against a statement model.

    Entry enterprise value is priced at the explicitly selected entry period, the sponsor
    equity check is the sources-and-uses residual, and exit proceeds are the
    exit enterprise value less modelled net debt at ``exit_period``. IRR is out
    of scope: pair ``exit_equity_proceeds`` with the equity outflow at close and
    call ``finstack_quant.portfolio.mwr_xirr``.

    Parameters
    ----------
    model : FinancialModelSpec
        A ``FinancialModelSpec``; metadata must include
        a ``"currency"`` key.
    config : dict
        The Rust ``LboConfig`` serde form (the document WASM ``evaluateLbo``
        takes as ``configJson``): ``entry_multiple`` (turns, ``8.5`` = 8.5x),
        ``entry_metric_node`` (monetary node read at the selected entry period),
        ``transaction_fees`` (model currency), ``sources``
        (``[{"name": ..., "amount": ...}]`` funded at close, model currency),
        ``entry_period`` (the period used to value the entry metric),
        ``exit_multiple``, ``exit_metric_node``, ``exit_net_debt_node``,
        ``exit_period`` (``"2029"`` or ``"2029Q4"``) and the optional
        ``check_mappings`` (a serde dict or a typed ``LboCheckMappings``),
        which runs the LBO model check suite against the same evaluation and
        populates ``LboResult.checks``. Every key except ``check_mappings`` is
        required.

    Returns
    -------
    LboResult
        Money outputs in the model currency, ``moic`` as a scalar, and
        ``checks`` (``CheckReport`` or ``None``).

    Raises
    ------
    ValueError
        If ``config`` is malformed (a missing or unknown key included), a
        tranche amount is invalid, ``exit_period`` does not parse, sources and
        uses cannot balance, or the model fails to evaluate.
    KeyError
        If a metric or net-debt node is missing at the required period.

    Examples
    --------
    >>> from finstack_quant.core.money import Money
    >>> from finstack_quant.statements import ModelBuilder
    >>> from finstack_quant.statements_analytics import evaluate_lbo
    >>> b = ModelBuilder("m")
    >>> _ = b.periods("2025Q1..2026Q1", None)
    >>> _ = b.with_meta("currency", "USD")
    >>> quarters = ["2025Q1", "2025Q2", "2025Q3", "2025Q4", "2026Q1"]
    >>> _ = b.value_money("ebitda", [(q, Money(v, "USD")) for q, v in zip(quarters, [100.0] * 4 + [120.0])])
    >>> _ = b.value_money("net_debt", [(q, Money(v, "USD")) for q, v in zip(quarters, [300.0] * 4 + [200.0])])
    >>> config = {
    ...     "entry_multiple": 8.0,
    ...     "entry_metric_node": "ebitda",
    ...     "entry_period": "2025Q1",
    ...     "transaction_fees": 0.0,
    ...     "sources": [{"name": "debt", "amount": 500.0}],
    ...     "exit_multiple": 9.0,
    ...     "exit_metric_node": "ebitda",
    ...     "exit_net_debt_node": "net_debt",
    ...     "exit_period": "2026Q1",
    ... }
    >>> round(evaluate_lbo(b.build(), config).moic, 4)
    2.9333
    """

class WaccInputs:
    """Capital-structure inputs with exact capital weights and annual effective costs.

    Examples
    --------
    >>> from finstack_quant.statements_analytics import WaccInputs
    >>> inputs = WaccInputs(
    ...     equity_weight="0.6", cost_of_equity=0.10, debt_weight="0.4", cost_of_debt=0.05, tax_rate=0.25
    ... )
    >>> WaccInputs.from_json(inputs.to_json()) == inputs
    True
    """

    def __init__(
        self,
        *,
        equity_weight: Decimal | str | int,
        cost_of_equity: float,
        debt_weight: Decimal | str | int,
        cost_of_debt: float,
        tax_rate: float,
    ) -> None:
        """Build the inputs by keyword.

        Parameters
        ----------
        equity_weight : Decimal or str or int
            Equity share of total capital as a decimal fraction; non-negative.
        cost_of_equity : float
            Required return on equity in decimal form (``0.115`` = 11.5%).
        debt_weight : Decimal or str or int
            Debt share of total capital as a decimal fraction; non-negative and
            summing with ``equity_weight`` to ``1.0``.
        cost_of_debt : float
            Pre-tax marginal borrowing yield in decimal form.
        tax_rate : float
            Marginal corporate tax rate as a decimal in ``[0, 1]``.

        Raises
        ------
        TypeError
            If either capital weight is not supplied as an exact decimal.
        ValueError
            If either weight cannot be represented exactly.
        """

    @property
    def equity_weight(self) -> Decimal:
        """Equity share of total capital as a decimal fraction.

        This property does not raise.

        Returns
        -------
        Decimal
            The exact equity weight.
        """

    @property
    def cost_of_equity(self) -> float:
        """Required return on equity as a decimal.

        This property does not raise.

        Returns
        -------
        float
            The cost of equity.
        """

    @property
    def debt_weight(self) -> Decimal:
        """Debt share of total capital as a decimal fraction.

        This property does not raise.

        Returns
        -------
        Decimal
            The exact debt weight.
        """

    @property
    def cost_of_debt(self) -> float:
        """Pre-tax cost of debt as a decimal.

        This property does not raise.

        Returns
        -------
        float
            The pre-tax cost of debt.
        """

    @property
    def tax_rate(self) -> float:
        """Marginal corporate tax rate as a decimal.

        This property does not raise.

        Returns
        -------
        float
            The tax rate.
        """

    @staticmethod
    def from_json(json: str) -> WaccInputs:
        """Deserialize from canonical JSON.

        Parameters
        ----------
        json : str
            ``WaccInputs`` JSON (strict: unknown or missing fields are rejected).

        Returns
        -------
        WaccInputs
            Reconstructed inputs.

        Raises
        ------
        ValueError
            If the payload is not valid ``WaccInputs`` JSON.

        Examples
        --------
        >>> from finstack_quant.statements_analytics import WaccInputs
        >>> text = '{"equity_weight": "1", "cost_of_equity": 0.1, "debt_weight": "0", "cost_of_debt": 0.05, "tax_rate": 0.25}'
        >>> str(WaccInputs.from_json(text).equity_weight)
        '1'
        """

    def to_json(self) -> str:
        """Serialize to compact JSON.

        Returns
        -------
        str
            JSON accepted by :meth:`from_json`.

        Raises
        ------
        ValueError
            If serialization fails.
        """

def wacc(inputs: WaccInputs) -> float:
    """Weighted-average cost of capital (WACC).

    ``WACC = w_E * r_E + w_D * r_D * (1 - T)``.

    Parameters
    ----------
    inputs : WaccInputs
        Exact nonnegative market-value weights summing to one, annual effective
        equity and pre-tax debt costs above -100%, and a tax rate in [0, 1].

    Returns
    -------
    float
        Blended discount rate as a decimal fraction.

    Raises
    ------
    ValueError
        If weights are negative or fail to sum exactly to one, an annual
        effective cost or the result is nonfinite or at or below -100%,
        or the tax rate falls outside ``[0, 1]``.

    Examples
    --------
    >>> from finstack_quant.statements_analytics import WaccInputs, wacc
    >>> inputs = WaccInputs(
    ...     equity_weight="0.6", cost_of_equity=0.10, debt_weight="0.4", cost_of_debt=0.05, tax_rate=0.25
    ... )
    >>> round(wacc(inputs), 4)
    0.075
    """

def run_corporate_analysis(
    model: FinancialModelSpec,
    options: Mapping[str, Any] | None = None,
    market: MarketContext | None = None,
) -> CorporateAnalysis:
    """Run the full corporate analysis pipeline.

    Evaluates statements and optionally runs DCF equity valuation plus credit
    context through the Rust ``run_corporate_analysis`` (the same entry point
    and the same ``options`` document WASM ``runCorporateAnalysis`` takes).

    Parameters
    ----------
    model : FinancialModelSpec
        A ``FinancialModelSpec``.
    options : dict | None
        The Rust ``CorporateAnalysisOptions`` serde form, as a dict. Every
        key is optional; ``None`` or an omitted key takes the
        default below, and an unknown key raises ``ValueError``:

        * ``wacc`` (float): enables DCF valuation at this decimal discount
          rate (``0.10`` = 10%); default no DCF.
        * ``terminal_value`` (TerminalValueSpec | dict): terminal value
          method; required when ``wacc`` is set.
        * ``net_debt_override`` (float): flat net debt in model currency for
          the equity bridge; default derived from the model.
        * ``cfads_node`` (str): CFADS numerator required when the model has
          capital-structure credit analytics; no EBITDA fallback is applied.
        * ``interest_coverage_node`` (str): earnings numerator used for
          interest coverage; default ``"ebitda"``.
        * ``check_suite`` (CheckSuiteSpec | dict): check suite required for
          DCF or credit analysis; must include ``NonFiniteCheck``.
        * ``as_of`` (datetime.date | str): valuation date as a date or ISO
          ``YYYY-MM-DD``; required when ``market`` is set.
        * ``ltv_value_node`` (str): monetary statement node supplying a
          per-period LTV denominator in reporting currency; scalar,
          foreign-currency or non-finite values are rejected. Missing or
          non-positive values or an omitted node leave LTV unavailable.
    market : MarketContext | None
        Market context for statement evaluation and instrument pricing (not
        WACC discounting); requires ``options["as_of"]``.

    Returns
    -------
    CorporateAnalysis
        ``statement`` (``StatementResult``), ``equity``
        (``CorporateValuationResult`` or ``None``), ``credit`` (per-instrument
        metrics).

    Raises
    ------
    ValueError
        If ``terminal_value`` is missing while ``wacc`` is set, ``market`` is
        set without ``as_of``, ``options`` or a payload is malformed (an
        unknown key included), or the pipeline fails.
    KeyError
        If a referenced node is missing from the model.

    Examples
    --------
    >>> from finstack_quant.statements import ModelBuilder
    >>> from finstack_quant.statements_analytics import run_corporate_analysis
    >>> b = ModelBuilder("m")
    >>> _ = b.periods("2024Q1..Q2", None)
    >>> _ = b.value("revenue", [("2024Q1", 1.0), ("2024Q2", 2.0)])
    >>> run_corporate_analysis(b.build(), {"interest_coverage_node": "revenue"}).statement.node_count
    1
    """
