"""Shared listed-future position and settlement terms."""

from datetime import date
from typing import Any, Literal, NotRequired, TypedDict, Unpack

from finstack_quant.core.currency import Currency

class CashSettlement(TypedDict):
    """Cash settlement of final exchange variation margin.

    Examples
    --------
    >>> from finstack_quant.valuations.instruments import ListedFutureTerms
    >>> terms = ListedFutureTerms(2, 1000, "USD", 112.25, "2025-09-19", "2025-09-30", "long")
    >>> _ = terms.with_settlement({"type": "cash"})
    """

    type: Literal["cash"]

class PhysicalSettlement(TypedDict):
    """Delivery of a specified asset and quantity per exchange contract.

    Examples
    --------
    >>> from finstack_quant.valuations.instruments import ListedFutureTerms
    >>> terms = ListedFutureTerms(2, 1000, "USD", 112.25, "2025-09-19", "2025-09-30", "long")
    >>> _ = terms.with_settlement({"type": "physical", "asset": "UST-BASKET", "quantity_per_contract": 100_000})
    """

    type: Literal["physical"]
    asset: str
    quantity_per_contract: float

class ListedFutureTerms:
    """Standard position, price units and lifecycle terms for a listed future.

    Examples
    --------
    >>> from finstack_quant.valuations.instruments import ListedFutureTerms
    >>> terms = ListedFutureTerms(2, 1000, "USD", 112.25, "2025-09-19", "2025-09-30", "long")
    >>> isinstance(terms.with_marks(113.0, None), ListedFutureTerms)
    True
    """

    def __init__(
        self,
        contracts: float,
        multiplier: float,
        currency: Currency | str,
        entry_price: float,
        last_trading_date: date | str,
        settlement_date: date | str,
        position: Literal["long", "short"],
    ) -> None:
        """Create a cash-settled listed future position without supplied marks.

        Parameters
        ----------
        contracts : float
            Positive finite number of contracts. Fractional values are permitted.
        multiplier : float
            Positive value in settlement currency of one contract price point.
        currency : Currency or str
            Currency of variation margin and P&L.
        entry_price : float
            Finite trade fill in contract price points.
        last_trading_date : date or str
            Final exchange trading date as a date or ISO string.
        settlement_date : date or str
            Final settlement date, on or after the last trading date.
        position : str
            Direction, either "long" or "short".

        Raises
        ------
        ValueError
            If numbers, currency, direction, or the date ordering are invalid.
        """
        ...

    def with_contracts(self, contracts: float) -> ListedFutureTerms:
        """Change the number of exchange contracts on an independent copy.

        Parameters
        ----------
        contracts : float
            Number of contracts. Must be positive and finite.

        Returns
        -------
        ListedFutureTerms
            Independent position with the same pricing and lifecycle basis.

        Raises
        ------
        ValueError
            If the requested contract count is invalid.
        """
        ...

    def with_marks(self, quoted_price: float | None, settlement_price: float | None) -> ListedFutureTerms:
        """Replace the observed marks on an independent copy.

        Parameters
        ----------
        quoted_price : float or None
            Optional live exchange mark in contract price points.
        settlement_price : float or None
            Optional official final settlement mark in contract price points.

        Returns
        -------
        ListedFutureTerms
            Independent terms with both marks set or cleared.

        Raises
        ------
        ValueError
            If a supplied mark is nonfinite.
        """
        ...

    def with_settlement(self, settlement: CashSettlement | PhysicalSettlement) -> ListedFutureTerms:
        """Replace the contract's final settlement policy on a copy.

        Parameters
        ----------
        settlement : CashSettlement or PhysicalSettlement
            Cash settlement, or physical delivery of a named asset with a
            positive finite quantity per exchange contract.

        Returns
        -------
        ListedFutureTerms
            Independent terms with the selected policy.

        Raises
        ------
        ValueError
            If the settlement shape or physical asset and quantity are invalid.
        """
        ...

    def __reduce__(self) -> tuple[Any, tuple[str]]:
        """Serialize the exact position terms for pickle reconstruction.

        Returns
        -------
        tuple[Any, tuple[str]]
            Reconstruction callable with the native position JSON argument.

        Raises
        ------
        ValueError
            If the terms cannot be serialized.
        """
        ...

    def to_json(self) -> str:
        """Serialize the position and lifecycle terms to canonical JSON.

        Returns
        -------
        str
            JSON document suitable for a listed instrument or wire interchange.

        Raises
        ------
        ValueError
            If serialization of the stored terms fails.
        """
        ...

    @staticmethod
    def from_json(json: str) -> ListedFutureTerms:
        """Load and validate listed future terms from canonical JSON.

        Parameters
        ----------
        json : str
            Serialized native terms with contract units and ISO settlement dates.

        Returns
        -------
        ListedFutureTerms
            Validated native terms.

        Raises
        ------
        ValueError
            If the document is malformed or the position or lifecycle terms are invalid.

        Examples
        --------
        >>> from finstack_quant.valuations.instruments import ListedFutureTerms
        >>> terms = ListedFutureTerms(2, 1000, "USD", 112.25, "2025-09-19", "2025-09-30", "long")
        >>> ListedFutureTerms.from_json(terms.to_json()).to_json() == terms.to_json()
        True
        """
        ...

class FutureOptionFields(TypedDict):
    """Wire fields for the shared contractual option-on-future terms.

    Examples
    --------
    >>> from finstack_quant.valuations.instruments import FutureOptionTerms
    >>> FutureOptionTerms.example().to_dict()["contracts"] > 0
    True
    """

    underlying: str
    futures_price: float
    strike: float
    contracts: float
    multiplier: float
    currency: Currency | str
    option_type: Literal["call", "put"]
    exercise_style: Literal["european", "american"]
    expiry: date | str
    settlement: dict[str, object]
    model: Literal["black76", "normal"]
    premium_style: Literal["premium_paid", "futures_style"]
    day_count: str
    discount_curve_id: str
    option_reference_price: NotRequired[float | None]
    underlying_price_change_per_bp: NotRequired[float | None]
    position: NotRequired[Literal["long", "short"]]
    exercise: NotRequired[dict[str, object] | None]

class FutureOptionTerms:
    """Contract and mark conventions shared by options on listed futures.

    Examples
    --------
    >>> from finstack_quant.valuations.instruments import FutureOptionTerms
    >>> FutureOptionTerms.example().to_dict()["contracts"] > 0
    True
    """

    def __init__(self, **fields: Unpack[FutureOptionFields]) -> None:
        """Validate and store the native futures-option terms.

        Parameters
        ----------
        fields : FutureOptionFields
            Underlying identifier, futures price and strike in contract points,
            positive contract count and settlement-currency multiplier, ISO
            currency, payoff and exercise style, expiry and settlement dates,
            quotation model, premium convention, day count and discount curve.
            Futures-style margin requires a reference option price in option
            points. Optional rate mapping is price points per upward basis point.
            Omitted position is long. Exercise records the official futures
            mark and date when the option has expired or already exercised.

        Raises
        ------
        ValueError
            If any required contract term is missing or an option lifecycle,
            model, quantity, price or settlement field is invalid.
        """
        ...

    @staticmethod
    def example() -> FutureOptionTerms:
        """Construct the native example of an option contract on a future.

        Returns
        -------
        FutureOptionTerms
            Example with explicit model, premium and settlement conventions.

        Raises
        ------
        ValueError
            If the bundled native example cannot be validated.

        Examples
        --------
        >>> from finstack_quant.valuations.instruments import FutureOptionTerms
        >>> FutureOptionTerms.example().to_dict()["strike"] > 0
        True
        """
        ...

    def to_dict(self) -> FutureOptionFields:
        """Return independent terms in the canonical field form.

        Returns
        -------
        FutureOptionFields
            Terms suitable for inspection or explicit reconstruction.

        Raises
        ------
        ValueError
            If an input field cannot be represented in the wire form.
        """
        ...
