"""Stub declarations mirroring ``finstack-quant-py/src/bindings/statements_analytics/valuation/inputs.rs``."""

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
from finstack_quant.statements_analytics._valuation.corporate import evaluate_dcf, wacc
from finstack_quant.statements_analytics._valuation.sensitivity import CorporateValuationResult

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
    model: FinancialModelSpec,
    request: CorporateDcfRequest,
    market: MarketContext | None = None,
) -> CorporateValuationResult:
    """Evaluate a statement model at its information cutoff and value its forecast.

    Parameters
    ----------
    model : FinancialModelSpec
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
    model: FinancialModelSpec,
    results: StatementResult,
    request: CorporateDcfRequest,
) -> CorporateValuationResult:
    """Reprice a company from a verified statement snapshot already in memory.

    Parameters
    ----------
    model : FinancialModelSpec
        Unchanged source model used to generate results.
    results : StatementResult
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
