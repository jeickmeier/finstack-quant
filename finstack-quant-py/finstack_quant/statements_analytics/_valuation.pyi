"""Stub declarations mirroring ``finstack-quant-py/src/bindings/statements_analytics/valuation.rs``."""

from __future__ import annotations
from decimal import Decimal

import datetime
from collections.abc import Mapping, Sequence
from typing import Any, Literal, TypedDict
import pandas as pd
from finstack_quant.statements import CheckReport, FinancialModelSpec, MonteCarloResults, StatementResult
from finstack_quant.core.currency import Currency
from finstack_quant.core.market_data import MarketContext
from finstack_quant.core.money import Money
from finstack_quant.statements_analytics._checks import CreditMapping, ThreeStatementMapping
from finstack_quant.statements_analytics._credit_context import CreditContextMetrics
from finstack_quant.statements_analytics._typed import TornadoEntry

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
    def to_dataframe(self) -> pd.DataFrame:
        """
        Export as a single-row pandas ``DataFrame``.

        Columns: ``currency``, ``equity_value``, ``enterprise_value``,
        ``net_debt``, ``other_equity_claims``, ``valuation_discount``, ``terminal_value_pv`` (float
        amounts in ``currency``), ``equity_value_per_share``,
        ``diluted_shares`` (``None`` when absent), then the DCF working:
        ``wacc`` (decimal), ``pv_explicit``, ``terminal_value``,
        ``total_debt``, ``cash`` (float amounts in ``currency``) and
        ``terminal_discount_years``; these six are ``None`` for a result
        loaded from JSON written before they were recorded.

        Returns
        -------
        pd.DataFrame
            One row with columns ``currency``, ``equity_value``, ``enterprise_value``, ``net_debt``, ``other_equity_claims``, ``valuation_discount``, ``terminal_value_pv``, ``equity_value_per_share``, ``diluted_shares``, ``wacc``, ``pv_explicit``, ``terminal_value``, ``total_debt``, ``cash``, ``terminal_discount_years``.

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

class TerminalValueSpec:
    """Terminal value method for a DCF.

    Build with one of the constructors; the wire form is the tagged serde enum
    (``{"type": "gordon_growth", "stable_growth_rate": 0.02}``).

    Examples
    --------
    >>> from finstack_quant.statements_analytics import TerminalValueSpec
    >>> TerminalValueSpec.gordon_growth(0.02).kind
    'gordon_growth'
    >>> TerminalValueSpec.exit_multiple(9.0, 120.0).params["multiple"]
    9.0
    """
    @staticmethod
    def gordon_growth(stable_growth_rate: float) -> TerminalValueSpec:
        """
        Gordon growth: ``TV = FCF_terminal * (1 + g) / (WACC - g)``.

        Every input is stored verbatim, so this constructor does not raise.

        Parameters
        ----------
        stable_growth_rate : float
            Perpetual stable growth rate ``g`` in decimal form (``0.02`` = 2%).
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
    def exit_multiple(multiple: float, terminal_metric: float | None = None) -> TerminalValueSpec:
        """
        Exit multiple: ``TV = terminal_metric * multiple``.

        Every input is stored verbatim, so this constructor does not raise.

        Parameters
        ----------
        multiple : float
            Exit multiple in turns (``9.0`` = 9.0x).
        terminal_metric : float or None
            Terminal-year metric (EBITDA, revenue, ...) in model currency.
            Omit when
            ``DcfOptions.exit_multiple_metric_node`` supplies the metric from
            the statement model. Without that node, ``evaluate_dcf`` raises
            ``ValueError`` unless the metric is positive and finite.
        Returns
        -------
        TerminalValueSpec
            Exit multiple: ``TV = terminal_metric * multiple``.

        Examples
        --------
        >>> from finstack_quant.statements_analytics import TerminalValueSpec
        >>> TerminalValueSpec.exit_multiple(8.0, 120.0).kind
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

def dcf_sensitivity(
    model: Any,
    wacc: float,
    terminal_value: Any,
    ufcf_node: str = "ufcf",
    net_debt_override: float | None = None,
    options: Mapping[str, Any] | str | None = None,
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
        Node id containing monetary unlevered free cash flow in model currency.
        Defaults to the Rust ``DEFAULT_UFCF_NODE`` (``"ufcf"``). Gordon Growth /
        H-Model terminal cash flow requires a complete contiguous calendar year
        at the end of the forecast.
    net_debt_override : float | None
        Flat net-debt amount in model currency. Without an override, debt and
        cash must be monetary in model currency and available at a period
        ending on or before the inclusive valuation date; future balances are
        rejected.
    options : dict | str | None
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
        ``to_dataframe()`` gives the tornado table.

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
    >>> list(sens.to_dataframe().columns)
    ['parameter_id', 'downside', 'upside', 'swing']
    """

def evaluate_dcf(
    model: Any,
    inputs: Mapping[str, Any] | str,
    options: Mapping[str, Any] | str | None = None,
    market: Any | None = None,
    as_of: Any | None = None,
) -> CorporateValuationResult:
    """Evaluate DCF valuation on a financial model.

    Parameters
    ----------
    model : FinancialModelSpec | str
        A ``FinancialModelSpec`` object or a JSON string; metadata must contain
        a ``"currency"`` key.
    inputs : dict | str
        The Rust ``DcfInputs`` serde form (the document WASM ``evaluateDcf``
        takes as ``inputs``), as a dict or JSON text. Unknown keys raise
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
    options : dict | str | None
        The Rust ``DcfOptions`` serde form. Every key is optional and a
        missing one takes its Rust default: ``mid_year_convention``
        (``False``), ``max_stable_growth_rate`` (``0.05``),
        ``shares_outstanding``, ``equity_bridge`` / ``valuation_discounts``
        (serde dicts or the typed ``EquityBridge`` / ``ValuationDiscounts``),
        ``exit_multiple_metric_node`` (statement monetary flow node whose
        complete trailing-year sum replaces ``terminal_metric`` on an
        exit-multiple terminal; actual period boundaries must cover one
        complete contiguous calendar year) and the sensitivity knobs. ``None`` uses every default.
    market : MarketContext | str | None
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

def evaluate_lbo(model: Any, config: Mapping[str, Any] | str) -> LboResult:
    """Evaluate a leveraged-buyout transaction against a statement model.

    Entry enterprise value is priced at the explicitly selected entry period, the sponsor
    equity check is the sources-and-uses residual, and exit proceeds are the
    exit enterprise value less modelled net debt at ``exit_period``. IRR is out
    of scope: pair ``exit_equity_proceeds`` with the equity outflow at close and
    call ``finstack_quant.portfolio.mwr_xirr``.

    Parameters
    ----------
    model : FinancialModelSpec | str
        A ``FinancialModelSpec`` object or a JSON string; metadata must include
        a ``"currency"`` key.
    config : dict | str
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

def run_corporate_analysis(
    model: Any,
    options: Mapping[str, Any] | str | None = None,
    market: Any | None = None,
) -> CorporateAnalysis:
    """Run the full corporate analysis pipeline.

    Evaluates statements and optionally runs DCF equity valuation plus credit
    context through the Rust ``run_corporate_analysis`` (the same entry point
    and the same ``options`` document WASM ``runCorporateAnalysis`` takes).

    Parameters
    ----------
    model : FinancialModelSpec | str
        A ``FinancialModelSpec`` object or a JSON string.
    options : dict | str | None
        The Rust ``CorporateAnalysisOptions`` serde form, as a dict or JSON
        text. Every key is optional; ``None`` or an omitted key takes the
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
    market : MarketContext | str | None
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

class DcfInputs:
    """Authored operating cash-flow and terminal-value assumptions for a DCF.

    Examples
    --------
    >>> from finstack_quant.statements_analytics import DcfInputs, TerminalValueSpec
    >>> isinstance(DcfInputs(wacc=0.10, terminal_value=TerminalValueSpec.gordon_growth(0.02)), DcfInputs)
    True
    """

    def __init__(
        self,
        *,
        wacc: float,
        terminal_value: TerminalValueSpec,
        ufcf_node: str = "ufcf",
        net_debt_override: float | None = None,
    ) -> None:
        """Construct and validate assumptions that do not depend on statement data.

        Parameters
        ----------
        wacc : float
            Annual effective rate above −100%, expressed as a decimal fraction.
        terminal_value : TerminalValueSpec
            Gordon growth, H-model, or enterprise exit multiple method.
        ufcf_node : str
            Exact statement node for signed monetary cash flows. Defaults to "ufcf".
        net_debt_override : float or None
            Authored signed net debt in model currency. None reads the admitted debt and cash balances.

        Raises
        ------
        ValueError
            For an invalid WACC, empty cash-flow node, or nonfinite net debt.
        """
        ...

class DcfOptions:
    """Additional company valuation terms and timing controls.

    Examples
    --------
    >>> from finstack_quant.statements_analytics import DcfOptions
    >>> isinstance(DcfOptions(mid_year_convention=True), DcfOptions)
    True
    """

    def __init__(
        self,
        *,
        mid_year_convention: bool = False,
        equity_bridge: EquityBridge | None = None,
        shares_outstanding: float | None = None,
        valuation_discounts: ValuationDiscounts | None = None,
        exit_multiple_metric_node: str | None = None,
    ) -> None:
        """Resolve authored options to Rust valuation settings.

        Parameters
        ----------
        mid_year_convention : bool
            Use each forecast period's elapsed midpoint when True. The default uses close dates.
        equity_bridge : EquityBridge or None
            Authored claims and cash in the valuation currency. None uses the explicit inputs and model balances.
        shares_outstanding : float or None
            Positive basic share count. None omits per-share valuation.
        valuation_discounts : ValuationDiscounts or None
            Separately elected discounts applied after the enterprise-to-equity bridge.
        exit_multiple_metric_node : str or None
            Named additive terminal earnings node. None uses the explicit metric supplied with the multiple.

        Raises
        ------
        ValueError
            For an unknown field or a value that violates the Rust wire type.
        """
        ...

class StatementValuationCut:
    """Statement information cutoff and effective valuation date in one currency.

    Examples
    --------
    >>> from finstack_quant.statements_analytics import StatementValuationCut
    >>> isinstance(StatementValuationCut("2024-12-31", "2024-12-30", "USD"), StatementValuationCut)
    True
    """

    def __init__(
        self,
        as_of: datetime.date | str,
        observed_through: datetime.date | str,
        currency: Currency | str,
    ) -> None:
        """Capture the requested reporting currency and observation window.

        Parameters
        ----------
        as_of : datetime.date or str
            Effective valuation day in ISO format or as a calendar date.
        observed_through : datetime.date or str
            Last eligible publication date, on or before valuation.
        currency : Currency or str
            ISO reporting currency shared by the valuation and underlying model.

        Raises
        ------
        ValueError
            For an invalid date or currency.
        TypeError
            For an input outside the supported host types.
        """
        ...

class TerminalFcffBasis:
    """Amount basis for annual free cash flow at a growth terminal.

    Examples
    --------
    >>> from finstack_quant.statements_analytics import TerminalFcffBasis
    >>> isinstance(TerminalFcffBasis.trailing_additive(), TerminalFcffBasis)
    True
    """

    @staticmethod
    def authored(amount: Money) -> TerminalFcffBasis:
        """Elect explicitly authored annual FCFF at the terminal horizon.

        Parameters
        ----------
        amount : Money
            Signed annual cash-flow amount in the valuation currency.

        This method does not raise.

        Returns
        -------
        TerminalFcffBasis
            Literal annual amount basis.

        Examples
        --------
        >>> from finstack_quant.core.money import Money
        >>> from finstack_quant.statements_analytics import TerminalFcffBasis
        >>> isinstance(TerminalFcffBasis.authored(Money(100, "USD")), TerminalFcffBasis)
        True
        """
        ...

    @staticmethod
    def trailing_additive() -> TerminalFcffBasis:
        """Elect a complete twelve-month sum of the named additive forecast cash flows.

        This method does not raise.

        Returns
        -------
        TerminalFcffBasis
            Terminal-period basis that validates contiguous coverage during valuation.

        Examples
        --------
        >>> from finstack_quant.statements_analytics import TerminalFcffBasis
        >>> isinstance(TerminalFcffBasis.trailing_additive(), TerminalFcffBasis)
        True
        """
        ...

class DcfProjection:
    """Explicit projection horizon and annual terminal cash-flow basis.

    Examples
    --------
    >>> from finstack_quant.statements_analytics import DcfProjection, TerminalFcffBasis
    >>> projection = DcfProjection(
    ...     terminal_date="2028-12-31", normalized_annual_fcff=TerminalFcffBasis.trailing_additive()
    ... )
    >>> isinstance(projection, DcfProjection)
    True
    """

    def __init__(
        self,
        *,
        terminal_date: datetime.date | str,
        normalized_annual_fcff: TerminalFcffBasis | None = None,
        forward_remainder_node: str | None = None,
        stable_growth_ceiling: float | None = None,
    ) -> None:
        """Author the terminal date and any extra forecast evidence.

        Parameters
        ----------
        terminal_date : datetime.date or str
            The terminal valuation date, no earlier than the final forecast receipt.
        normalized_annual_fcff : TerminalFcffBasis or None
            Same-currency annual FCFF election. Required for Gordon growth or H-model.
        forward_remainder_node : str or None
            Exact node for the forward part of a forecast period crossing the valuation date.
        stable_growth_ceiling : float or None
            Optional elected upper bound on stable growth as an annual decimal rate.

        Raises
        ------
        ValueError
            For an invalid terminal date.
        """
        ...

class CorporateDcfRequest:
    """Complete corporate DCF terms on an explicitly selected valuation cut.

    Examples
    --------
    >>> from finstack_quant.statements_analytics import CorporateDcfRequest, DcfInputs, DcfProjection
    >>> from finstack_quant.statements_analytics import StatementValuationCut, TerminalValueSpec
    >>> request = CorporateDcfRequest(
    ...     cut=StatementValuationCut("2024-12-31", "2024-12-31", "USD"),
    ...     inputs=DcfInputs(wacc=0.10, terminal_value=TerminalValueSpec.exit_multiple(10.0, 8.0)),
    ...     projection=DcfProjection(terminal_date="2025-12-31"),
    ... )
    >>> isinstance(request, CorporateDcfRequest)
    True
    """

    def __init__(
        self,
        *,
        cut: StatementValuationCut,
        inputs: DcfInputs,
        projection: DcfProjection,
        options: DcfOptions | None = None,
    ) -> None:
        """Combine the explicit statement date, pricing terms and projection.

        This constructor does not raise.

        Parameters
        ----------
        cut : StatementValuationCut
            Effective date, information cutoff and reporting currency.
        inputs : DcfInputs
            Cash-flow node and terminal and discounting assumptions.
        projection : DcfProjection
            Terminal month and authorized forecast basis.
        options : DcfOptions or None
            Optional timing, equity bridge and share terms. None uses Rust defaults.
        """
        ...

def evaluate_corporate_dcf(
    model: FinancialModelSpec | str,
    request: CorporateDcfRequest,
    market: MarketContext | None = None,
) -> CorporateValuationResult:
    """Evaluate a statement model at its information cutoff and value its forecast.

    Parameters
    ----------
    model : FinancialModelSpec or str
        Model or serialized model with a source snapshot and dated publication metadata.
    request : CorporateDcfRequest
        Effective valuation date, information cutoff, currency and valuation terms.
    market : MarketContext or None
        Market inputs for capital-structure-linked statement nodes. None supplies no external curves.

    Returns
    -------
    CorporateValuationResult
        Enterprise value, cash flow and terminal timing, and equity bridge in the stated currency.

    Raises
    ------
    ValueError
        For an invalid cut, inconsistent currency, incomplete assumptions or an inadmissible valuation.
    KeyError
        If a required model observation cannot be read.

    Examples
    --------
    >>> from finstack_quant.core.money import Money
    >>> from finstack_quant.statements import ModelBuilder
    >>> from finstack_quant.statements_analytics import CorporateDcfRequest, DcfInputs, DcfProjection
    >>> from finstack_quant.statements_analytics import StatementValuationCut, TerminalFcffBasis, TerminalValueSpec
    >>> from finstack_quant.statements_analytics import evaluate_corporate_dcf
    >>> builder = ModelBuilder("qualified-dcf")
    >>> _ = builder.periods("2025..2025")
    >>> _ = builder.source_snapshot("forecast-2024-12-31")
    >>> _ = builder.currency("USD")
    >>> _ = builder.value_money("ufcf", [("2025", Money(100, "USD"))])
    >>> request = CorporateDcfRequest(
    ...     cut=StatementValuationCut("2024-12-31", "2024-12-31", "USD"),
    ...     inputs=DcfInputs(wacc=0.10, terminal_value=TerminalValueSpec.gordon_growth(0.02), net_debt_override=50.0),
    ...     projection=DcfProjection(
    ...         terminal_date="2026-01-01", normalized_annual_fcff=TerminalFcffBasis.authored(Money(100, "USD"))
    ...     ),
    ... )
    >>> round(evaluate_corporate_dcf(builder.build(), request).enterprise_value.amount, 2)
    1249.78
    """
    ...

def evaluate_dcf_from_results(
    model: FinancialModelSpec | str,
    results: StatementResult | str,
    request: CorporateDcfRequest,
) -> CorporateValuationResult:
    """Reprice a company from a verified statement snapshot already in memory.

    Parameters
    ----------
    model : FinancialModelSpec or str
        Unchanged source model used to generate results.
    results : StatementResult or str
        Evaluated observations with a matching run hash, cutoff and effective date.
    request : CorporateDcfRequest
        The authorized statement cut, forecast selection and pricing terms.

    Returns
    -------
    CorporateValuationResult
        Same-currency enterprise and common-equity values from the reused snapshot.

    Raises
    ------
    ValueError
        If the statement provenance differs from the requested model or cut, or valuation fails.
    KeyError
        If an admitted required model observation is absent.

    Examples
    --------
    >>> from finstack_quant.statements_analytics import evaluate_dcf_from_results
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
    >>> round(evaluate_dcf_from_results(model, results, request).enterprise_value.amount, 2)
    1249.78
    """
    ...

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
    >>> hasattr(DcfSensitivityGridResult, "to_dataframe")
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

    def to_dataframe(self) -> pd.DataFrame:
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
    >>> dcf_sensitivity_grid(model, results, request, axes).to_dataframe().shape
    (2, 3)
    """
    ...

class DcfDistributionResult:
    """Same-currency company values repriced from coherent statement samples.

    Examples
    --------
    >>> from finstack_quant.statements_analytics import DcfDistributionResult
    >>> hasattr(DcfDistributionResult, "quantiles_to_dataframe")
    True
    """

    def to_dataframe(self) -> pd.DataFrame:
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
        """
        ...

    def quantiles_to_dataframe(self) -> pd.DataFrame:
        """Present R-7 sample quantiles of enterprise and common-equity value.

        Returns
        -------
        pd.DataFrame
            One row per requested probability, with both monetary values
            expressed as floats in the reported ISO currency.

        Raises
        ------
        ImportError
            If pandas is unavailable.
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
    >>> len(evaluate_dcf_monte_carlo(model, results, samples, request).to_dataframe())
    3
    """
    ...

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
