"""Dated leveraged buyout authoring and reporting."""

from collections.abc import Sequence
from datetime import date
from decimal import Decimal
from typing import Any, Literal, TypedDict

from finstack_quant.core.market_data import MarketContext
from finstack_quant.core.money import Money
from finstack_quant.statements import CheckReport, CheckSuiteSpec, FinancialModelSpec
from finstack_quant.statements_analytics._valuation.corporate import LboCheckMappings
from finstack_quant.statements_analytics._valuation.inputs import StatementValuationCut

class _CashFree(TypedDict):
    kind: Literal["cash_free_debt_free"]

class _SellerAndRefinancing(TypedDict):
    kind: Literal["seller_and_refinancing"]
    seller_consideration: Money
    refinancing: Money
    closing_reserves: Money

_Acquisition = _CashFree | _SellerAndRefinancing

class _SpecSuite(TypedDict):
    kind: Literal["spec"]
    spec: CheckSuiteSpec | dict[str, Any]

class _LboSuite(TypedDict):
    kind: Literal["lbo"]
    mappings: LboCheckMappings | dict[str, Any]

class _Qualification(TypedDict):
    suite: _SpecSuite | _LboSuite
    required: bool

class LboEarnings:
    """Named, period-matched earnings and the corresponding valuation multiple.

    Examples
    --------
    >>> from finstack_quant.statements_analytics import LboEarnings
    >>> entry = LboEarnings(node="ebitda", period="2025", multiple="8", basis="as_observed")
    >>> isinstance(entry, LboEarnings)
    True
    """
    def __init__(
        self, *, node: str, period: str, multiple: Decimal | str, basis: Literal["as_observed", "trailing_additive"]
    ) -> None:
        """Named, period-matched earnings and the corresponding valuation multiple.

        Parameters
        ----------
        node : str
            Exact same-currency monetary earnings node.
        period : str
            Period label of the elected earnings observation.
        multiple : Decimal | str
            Positive exact multiple in turns.
        basis : Literal["as_observed", "trailing_additive"]
            Whether to read one matching observation or sum twelve contiguous months.

        Raises
        ------
        ValueError
            If a supplied field fails the native wire type or an unknown field is provided.
        TypeError
            If a field cannot be encoded as its declared input type.
        """
        ...
    def to_json(self) -> str:
        """Encode these assumptions using the canonical wire format.

        Returns
        -------
        str
            JSON for interchange with Rust and WASM.

        Raises
        ------
        ValueError
            If the assumptions cannot be encoded.
        """
        ...

    @staticmethod
    def from_json(json: str) -> LboEarnings:
        """Rebuild the native LboEarnings from its JSON record.

        Parameters
        ----------
        json : str
            JSON object with the dated evidence and fields of this record.

        Returns
        -------
        LboEarnings
            Decoded native value.

        Raises
        ------
        ValueError
            If the JSON syntax or field values are invalid.

        Examples
        --------
        >>> from finstack_quant.statements_analytics import LboEarnings
        >>> try:
        ...     LboEarnings.from_json("null")
        ... except ValueError:
        ...     print("A record is required")
        A record is required
        """
        ...

    def __reduce__(self) -> tuple[object, tuple[str]]:
        """Return the reconstruction callable and native JSON arguments for pickle."""
        ...

class LboDebtFunding:
    """One closing debt source with separate proceeds and face.

    Examples
    --------
    >>> from finstack_quant.statements_analytics import LboDebtFunding
    >>> from finstack_quant.core.money import Money
    >>> debt = LboDebtFunding(name="term", cash_proceeds=Money(500, "USD"), opening_face=Money(520, "USD"))
    >>> isinstance(debt, LboDebtFunding)
    True
    """
    def __init__(self, *, name: str, cash_proceeds: Money, opening_face: Money) -> None:
        """One closing debt source with separate proceeds and face.

        Parameters
        ----------
        name : str
            Unique name identifying the borrowing.
        cash_proceeds : Money
            Cash actually received at closing in transaction currency.
        opening_face : Money
            Contractual face immediately after closing in transaction currency.

        Raises
        ------
        ValueError
            If a supplied field fails the native wire type or an unknown field is provided.
        TypeError
            If a field cannot be encoded as its declared input type.
        """
        ...
    def to_json(self) -> str:
        """Encode these assumptions using the canonical wire format.

        Returns
        -------
        str
            JSON for interchange with Rust and WASM.

        Raises
        ------
        ValueError
            If the assumptions cannot be encoded.
        """
        ...

    @staticmethod
    def from_json(json: str) -> LboDebtFunding:
        """Rebuild the native LboDebtFunding from its JSON record.

        Parameters
        ----------
        json : str
            JSON object with the dated evidence and fields of this record.

        Returns
        -------
        LboDebtFunding
            Decoded native value.

        Raises
        ------
        ValueError
            If the JSON syntax or field values are invalid.

        Examples
        --------
        >>> from finstack_quant.statements_analytics import LboDebtFunding
        >>> try:
        ...     LboDebtFunding.from_json("null")
        ... except ValueError:
        ...     print("A record is required")
        A record is required
        """
        ...

    def __reduce__(self) -> tuple[object, tuple[str]]:
        """Return the reconstruction callable and native JSON arguments for pickle."""
        ...

class LboFunding:
    """Recognized cash sources and uses on the closing date.

    Examples
    --------
    >>> from finstack_quant.statements_analytics import LboFunding
    >>> from finstack_quant.core.money import Money
    >>> funding = LboFunding(
    ...     date="2025-12-31",
    ...     acquisition={"kind": "cash_free_debt_free"},
    ...     transaction_fees=Money(20, "USD"),
    ...     closing_credits=Money(0, "USD"),
    ...     debt=[],
    ... )
    >>> isinstance(funding, LboFunding)
    True
    """
    def __init__(
        self,
        *,
        date: str,
        acquisition: _Acquisition,
        transaction_fees: Money,
        closing_credits: Money,
        debt: Sequence[LboDebtFunding],
        sponsor_commitment: Money | None = None,
    ) -> None:
        """Recognized cash sources and uses on the closing date.

        Parameters
        ----------
        date : str
            Effective entry date in ISO YYYY-MM-DD format.
        acquisition : _Acquisition
            Cash-free/debt-free purchase or explicit cash seller consideration, refinancing and reserves.
        transaction_fees : Money
            Nonnegative closing fees in transaction currency.
        closing_credits : Money
            Nonnegative offsets to the transaction uses in transaction currency.
        debt : Sequence[LboDebtFunding]
            Identified debt sources with cash proceeds and separate opening face.
        sponsor_commitment : Money | None
            Sponsor cash actually committed at entry, or None for a sizing case.

        Raises
        ------
        ValueError
            If a supplied field fails the native wire type or an unknown field is provided.
        TypeError
            If a field cannot be encoded as its declared input type.
        """
        ...
    def to_json(self) -> str:
        """Encode these assumptions using the canonical wire format.

        Returns
        -------
        str
            JSON for interchange with Rust and WASM.

        Raises
        ------
        ValueError
            If the assumptions cannot be encoded.
        """
        ...

    @staticmethod
    def from_json(json: str) -> LboFunding:
        """Rebuild the native LboFunding from its JSON record.

        Parameters
        ----------
        json : str
            JSON object with the dated evidence and fields of this record.

        Returns
        -------
        LboFunding
            Decoded native value.

        Raises
        ------
        ValueError
            If the JSON syntax or field values are invalid.

        Examples
        --------
        >>> from finstack_quant.statements_analytics import LboFunding
        >>> try:
        ...     LboFunding.from_json("null")
        ... except ValueError:
        ...     print("A record is required")
        A record is required
        """
        ...

    def __reduce__(self) -> tuple[object, tuple[str]]:
        """Return the reconstruction callable and native JSON arguments for pickle."""
        ...

class LboRequest:
    """Point-in-time assumptions for an acquisition and projected exit.

    Examples
    --------
    >>> from finstack_quant.statements_analytics import LboRequest
    >>> from finstack_quant.core.money import Money
    >>> from finstack_quant.statements_analytics import LboRequest, LboDebtFunding, LboEarnings
    >>> from finstack_quant.statements_analytics import LboFunding, StatementValuationCut
    >>> close, exit_valuation_date = "2025-12-31", "2026-12-31"
    >>> def usd(n):
    ...     return Money(n, "USD")
    >>> funding = LboFunding(
    ...     date=close,
    ...     acquisition={"kind": "cash_free_debt_free"},
    ...     transaction_fees=usd(20),
    ...     closing_credits=usd(0),
    ...     debt=[LboDebtFunding(name="term", cash_proceeds=usd(500), opening_face=usd(520))],
    ...     sponsor_commitment=usd(320),
    ... )
    >>> request = LboRequest(
    ...     cut=StatementValuationCut(close, close, "USD"),
    ...     exit_valuation_date=exit_valuation_date,
    ...     entry=LboEarnings(node="ebitda", period="2025", multiple="8", basis="as_observed"),
    ...     funding=funding,
    ...     exit=LboEarnings(node="ebitda", period="2026", multiple="9", basis="as_observed"),
    ...     exit_debt_node="debt",
    ...     exit_cash_node="cash",
    ... )
    >>> isinstance(request, LboRequest)
    True
    """
    def __init__(
        self,
        *,
        cut: StatementValuationCut,
        exit_valuation_date: str,
        entry: LboEarnings,
        funding: LboFunding,
        exit: LboEarnings,
        exit_debt_node: str,
        exit_cash_node: str,
        exit_claims_node: str | None = None,
        checks: _Qualification | None = None,
    ) -> None:
        """Point-in-time assumptions for an acquisition and projected exit.

        Parameters
        ----------
        cut : StatementValuationCut
            Dated observation cutoff, acquisition date and transaction currency.
        exit_valuation_date : str
            Projected exit date in ISO YYYY-MM-DD format.
        entry : LboEarnings
            Entry earnings and purchase multiple basis.
        funding : LboFunding
            Recognized entry cash sources and uses.
        exit : LboEarnings
            Projected earnings and exit multiple basis.
        exit_debt_node : str
            Projected contractual closing debt face node.
        exit_cash_node : str
            Projected eligible closing cash node.
        exit_claims_node : str | None
            Optional node for other claims ranking ahead of common equity.
        checks : _Qualification | None
            Optional check suite and requirement policy:
            ``{"suite": ..., "required": bool}``. ``suite`` is a tagged choice:
            ``{"kind": "spec", "spec": CheckSuiteSpec}`` runs an explicit suite
            of built-in and formula checks, and
            ``{"kind": "lbo", "mappings": LboCheckMappings}`` runs the LBO model
            suite (three-statement and credit checks) built from node mappings.
            With ``required`` true, a suite that does not pass raises instead of
            returning a result.

        Raises
        ------
        ValueError
            If a supplied field fails the native wire type or an unknown field is provided.
        TypeError
            If a field cannot be encoded as its declared input type.
        """
        ...
    def to_json(self) -> str:
        """Encode these assumptions using the canonical wire format.

        Returns
        -------
        str
            JSON for interchange with Rust and WASM.

        Raises
        ------
        ValueError
            If the assumptions cannot be encoded.
        """
        ...

    @staticmethod
    def from_json(json: str) -> LboRequest:
        """Rebuild the native LboRequest from its JSON record.

        Parameters
        ----------
        json : str
            JSON object with the dated evidence and fields of this record.

        Returns
        -------
        LboRequest
            Decoded native value.

        Raises
        ------
        ValueError
            If the JSON syntax or field values are invalid.

        Examples
        --------
        >>> from finstack_quant.statements_analytics import LboRequest
        >>> try:
        ...     LboRequest.from_json("null")
        ... except ValueError:
        ...     print("A record is required")
        A record is required
        """
        ...

    def __reduce__(self) -> tuple[object, tuple[str]]:
        """Return the reconstruction callable and native JSON arguments for pickle."""
        ...

class LboFundingResult:
    """Reconciled closing cash sources and uses.

    Examples
    --------
    >>> from finstack_quant.statements_analytics import LboFundingResult
    >>> hasattr(LboFundingResult, "funding_gap")
    True
    """

    @property
    def cash_uses(self) -> Money:
        """Get the reported cash uses.

        This property does not raise.

        Returns
        -------
        Money
            Transaction cash uses after elected credits.
        """
        ...

    @property
    def debt_cash_proceeds(self) -> Money:
        """Get the reported debt cash proceeds.

        This property does not raise.

        Returns
        -------
        Money
            Total actual debt cash proceeds.
        """
        ...

    @property
    def opening_debt_face(self) -> Money:
        """Get the reported opening debt face.

        This property does not raise.

        Returns
        -------
        Money
            Total contractual debt face at entry.
        """
        ...

    @property
    def required_sponsor_cash(self) -> Money:
        """Get the reported required sponsor cash.

        This property does not raise.

        Returns
        -------
        Money
            Uses less recognized debt proceeds.
        """
        ...

    @property
    def committed_cash_sources(self) -> Money | None:
        """Get the reported committed cash sources.

        This property does not raise.

        Returns
        -------
        Money | None
            Actual cash sources if all commitments were authored.
        """
        ...

    @property
    def funding_gap(self) -> Money | None:
        """Get the reported funding gap.

        This property does not raise.

        Returns
        -------
        Money | None
            Uses less committed cash sources, positive for a shortage, or None for sizing.
        """
        ...

class LboExitBridge:
    """Projected common equity bridge and available sponsor multiples.

    Examples
    --------
    >>> from finstack_quant.statements_analytics import LboExitBridge
    >>> hasattr(LboExitBridge, "funded_moic")
    True
    """

    @property
    def enterprise_value(self) -> Money:
        """Get the reported enterprise value.

        This property does not raise.

        Returns
        -------
        Money
            Projected exit enterprise value.
        """
        ...

    @property
    def net_debt(self) -> Money:
        """Get the reported net debt.

        This property does not raise.

        Returns
        -------
        Money
            Closing face less eligible cash.
        """
        ...

    @property
    def other_claims(self) -> Money:
        """Get the reported other claims.

        This property does not raise.

        Returns
        -------
        Money
            Projected claims ranking ahead of equity.
        """
        ...

    @property
    def common_equity_residual(self) -> Money:
        """Get the reported common equity residual.

        This property does not raise.

        Returns
        -------
        Money
            Signed exit residual before limited liability.
        """
        ...

    @property
    def sponsor_proceeds(self) -> Money:
        """Get the reported sponsor proceeds.

        This property does not raise.

        Returns
        -------
        Money
            Projected proceeds floored at zero by limited liability.
        """
        ...

    @property
    def sized_moic(self) -> float | None:
        """Get the reported sized moic.

        This property does not raise.

        Returns
        -------
        float | None
            Sponsor multiple on the modeled required cash if meaningful.
        """
        ...

    @property
    def funded_moic(self) -> float | None:
        """Get the reported funded moic.

        This property does not raise.

        Returns
        -------
        float | None
            Sponsor multiple on committed cash when transaction funding reconciles.
        """
        ...

class LboResult:
    """Dated LBO investment and exit evidence.

    Examples
    --------
    >>> from finstack_quant.statements_analytics import LboResult
    >>> hasattr(LboResult, "funding")
    True
    """

    @property
    def entry_date(self) -> date:
        """Get the acquisition date.

        Returns
        -------
        date
            Actual acquisition date used for the funding bridge.

        Raises
        ------
        OverflowError
            If the calendar year is outside the Python date domain.
        """
        ...

    @property
    def exit_valuation_date(self) -> date:
        """Get the projected exit date.

        Returns
        -------
        date
            Projected exit observation date.

        Raises
        ------
        OverflowError
            If the calendar year is outside the Python date domain.
        """
        ...

    @property
    def run(self) -> dict[str, object]:
        """Get the identity of the input statement run.

        Returns
        -------
        dict[str, object]
            Model, publication and valuation identities of the admitted statement run.

        Raises
        ------
        ValueError
            If the stored run identity cannot be serialized.
        """
        ...

    @property
    def checks(self) -> CheckReport | None:
        """Get the elected statement checks and their verdict.

        This property does not raise.

        Returns
        -------
        CheckReport | None
            Check outcomes and evidence, or None if no check suite was requested.
        """
        ...

    @property
    def entry_earnings(self) -> Money:
        """Get the reported entry earnings.

        This property does not raise.

        Returns
        -------
        Money
            Matched entry earnings in transaction currency.
        """
        ...

    @property
    def entry_enterprise_value(self) -> Money:
        """Get the reported entry enterprise value.

        This property does not raise.

        Returns
        -------
        Money
            Entry enterprise value at the chosen purchase multiple.
        """
        ...

    @property
    def funding(self) -> LboFundingResult:
        """Get the reported funding.

        This property does not raise.

        Returns
        -------
        LboFundingResult
            Reconciled entry cash flows and signed funding gap.
        """
        ...

    @property
    def exit_earnings(self) -> Money:
        """Get the reported exit earnings.

        This property does not raise.

        Returns
        -------
        Money
            Projected earnings at the exit date.
        """
        ...

    @property
    def exit(self) -> LboExitBridge:
        """Get the reported exit.

        This property does not raise.

        Returns
        -------
        LboExitBridge
            Projected signed residual and qualified sponsor returns.
        """
        ...

    def to_json(self) -> str:
        """Encode this result using the canonical wire format.

        Returns
        -------
        str
            JSON holding the run's provenance, both dates, the entry and exit
            earnings and values, the funding reconciliation, the exit bridge
            and the check report. It is the object WASM ``evaluateLbo``
            returns.

        Raises
        ------
        ValueError
            If the result cannot be encoded.
        """
        ...

    @staticmethod
    def from_json(json: str) -> LboResult:
        """Rebuild a dated LBO result from its JSON record.

        Parameters
        ----------
        json : str
            JSON written by :meth:`to_json`. Unknown fields are rejected.

        Returns
        -------
        LboResult
            Decoded result. Nothing is recomputed.

        Raises
        ------
        ValueError
            If the JSON syntax or field values are invalid.

        Examples
        --------
        >>> from finstack_quant.statements_analytics import LboResult
        >>> try:
        ...     LboResult.from_json("null")
        ... except ValueError:
        ...     print("A record is required")
        A record is required
        """
        ...

    def __reduce__(self) -> tuple[object, tuple[str]]:
        """Return the reconstruction callable and native JSON arguments for pickle."""
        ...

def evaluate_lbo(
    model: FinancialModelSpec,
    request: LboRequest,
    market: MarketContext | None = None,
) -> LboResult:
    """Reconcile dated acquisition funding and calculate projected sponsor returns.

    Parameters
    ----------
    model : FinancialModelSpec
        Identified statement model containing approved actual and projected periods.
    request : LboRequest
        Closing terms and exit assumptions in one transaction currency.
    market : MarketContext or None
        Market data required by linked capital observations. None uses no market data.

    Returns
    -------
    LboResult
        Reconciled funding and only the sponsor multiples supported by the transaction.

    Raises
    ------
    ValueError
        If admitted periods, dates, currencies, source evidence or required checks are invalid.

    Examples
    --------
    >>> from finstack_quant.statements_analytics import evaluate_lbo
    >>> from finstack_quant.core.money import Money
    >>> from finstack_quant.statements_analytics import LboRequest, LboDebtFunding, LboEarnings
    >>> from finstack_quant.statements_analytics import LboFunding, StatementValuationCut
    >>> close, exit_valuation_date = "2025-12-31", "2026-12-31"
    >>> def usd(n):
    ...     return Money(n, "USD")
    >>> funding = LboFunding(
    ...     date=close,
    ...     acquisition={"kind": "cash_free_debt_free"},
    ...     transaction_fees=usd(20),
    ...     closing_credits=usd(0),
    ...     debt=[LboDebtFunding(name="term", cash_proceeds=usd(500), opening_face=usd(520))],
    ...     sponsor_commitment=usd(320),
    ... )
    >>> request = LboRequest(
    ...     cut=StatementValuationCut(close, close, "USD"),
    ...     exit_valuation_date=exit_valuation_date,
    ...     entry=LboEarnings(node="ebitda", period="2025", multiple="8", basis="as_observed"),
    ...     funding=funding,
    ...     exit=LboEarnings(node="ebitda", period="2026", multiple="9", basis="as_observed"),
    ...     exit_debt_node="debt",
    ...     exit_cash_node="cash",
    ... )
    >>> from finstack_quant.statements import ModelBuilder
    >>> builder = ModelBuilder("buyout").periods("2025..2026", "2025").currency("USD")
    >>> _ = builder.source_snapshot("2025-close")
    >>> _ = builder.value_money("ebitda", [("2025", usd(100)), ("2026", usd(120))])
    >>> _ = builder.availability_dates("ebitda", [("2025", close)])
    >>> _ = builder.value_money("debt", [("2026", usd(440))])
    >>> _ = builder.value_money("cash", [("2026", usd(40))])
    >>> evaluate_lbo(builder.build(), request).exit.funded_moic
    2.125
    """
    ...
