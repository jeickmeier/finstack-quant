"""Shared listed-future position and settlement terms."""

from datetime import date
from typing import Any, Literal, TypedDict

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
