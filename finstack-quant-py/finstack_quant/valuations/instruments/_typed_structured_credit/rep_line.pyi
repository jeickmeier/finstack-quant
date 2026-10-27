"""Stub declarations mirroring ``finstack-quant-py/src/bindings/valuations/typed_structured_credit/rep_line.rs``."""

from __future__ import annotations

import datetime
from typing import Any
import pandas as pd
from finstack_quant.core.dates import DayCount
from finstack_quant.core.money import Money
from finstack_quant.core.types import Bps, Rate

class RepLine:
    """
    Aggregated representative line for pool modeling.

    Examples
    --------
    >>> import datetime
    >>> from finstack_quant.core.currency import Currency
    >>> from finstack_quant.core.dates import DayCount
    >>> from finstack_quant.core.money import Money
    >>> from finstack_quant.valuations.instruments import RepLine
    >>> line = RepLine(
    ...     "LINE-1",
    ...     Money(80_000_000.0, Currency("USD")),
    ...     0.07,
    ...     datetime.date(2031, 1, 15),
    ...     12,
    ...     DayCount.ACT_360,
    ...     asset_type={"type": "first_lien_loan"},
    ...     cpr=0.10,
    ...     cdr=0.02,
    ...     recovery_rate=0.45,
    ... )
    >>> "LINE-1" in repr(line)
    True
    """

    def __init__(
        self,
        id: str,
        balance: Money,
        rate: float | Rate,
        maturity: datetime.date | datetime.datetime | pd.Timestamp | str,
        seasoning_months: int,
        day_count: DayCount,
        *,
        asset_type: dict[str, Any],
        spread_bp: float | Bps | None = None,
        forward_curve_id: str | None = None,
        index_floor_bp: float | None = None,
        cpr: float | None = None,
        cdr: float | None = None,
        recovery_rate: float | None = None,
        contractual_payment: Money | None = None,
        amortization_term_months: int | None = None,
        io_months: int | None = None,
    ) -> None:
        """
        Aggregated representative line for pool modeling.

        Parameters
        ----------
        id : str
            Unique identifier for the rep line.
        balance : Money
            Aggregated balance.
        rate : float | Rate
            Weighted average coupon as an annual decimal rate (e.g. ``0.07``
            = 7%).
        maturity : datetime.date | datetime.datetime | pandas.Timestamp | str
            Weighted average maturity date (date-like or ISO-8601 string).
        seasoning_months : int
            Weighted average seasoning in months.
        day_count : DayCount
            Day count convention.
        asset_type : dict[str, Any]
            Canonical AssetType JSON object; specifies bullet or level-pay amortization.
        spread_bp : float | Bps, optional
            Weighted average spread over the reference index, in basis
            points (e.g. ``150.0`` = 150bp), for floating-rate lines.
        forward_curve_id : str, optional
            Reference index identifier, if floating.
        index_floor_bp : float, optional
            Floor on the floating index in basis points (``100`` = 1%), applied before
            ``spread_bp``; ignored on fixed-rate lines.
        cpr : float, optional
            Constant prepayment rate override, as an annual decimal (e.g.
            ``0.10`` = 10% CPR).
        cdr : float, optional
            Constant default rate override, as an annual decimal (e.g.
            ``0.02`` = 2% CDR).
        recovery_rate : float, optional
            Recovery rate override, as a decimal fraction (e.g. ``0.45`` =
            45%).
        contractual_payment : Money, optional
            Contractual periodic payment for level-pay lines; inferred from the
            balance and remaining schedule when ``None``.
        amortization_term_months : int, optional
            Schedule length in months from origination; with
            ``seasoning_months`` the line amortizes over ``term − seasoning``.
        io_months : int, optional
            Interest-only window in months from origination.

        Returns
        -------
        RepLine
            The rep line.

        Raises
        ------
        TypeError
            If ``maturity`` is neither date-like nor a string, or ``rate`` /
            ``spread_bp`` are not numbers / ``Rate`` / ``Bps``.
        ValueError
            If a string ``maturity`` is not valid ISO-8601.

        Examples
        --------
        >>> import datetime
        >>> from finstack_quant.core.currency import Currency
        >>> from finstack_quant.core.dates import DayCount
        >>> from finstack_quant.core.money import Money
        >>> from finstack_quant.valuations.instruments import RepLine
        >>> line = RepLine(
        ...     "LINE-1",
        ...     Money(80_000_000.0, Currency("USD")),
        ...     0.07,
        ...     datetime.date(2031, 1, 15),
        ...     12,
        ...     DayCount.ACT_360,
        ...     asset_type={"type": "first_lien_loan"},
        ...     cpr=0.10,
        ...     cdr=0.02,
        ...     recovery_rate=0.45,
        ... )
        >>> "LINE-1" in repr(line)
        True
        """
        ...

    @staticmethod
    def from_json(json: str) -> RepLine:
        """
        Deserialize from the JSON produced by :meth:`to_json`.

        Parameters
        ----------
        json : str
            Strict JSON object with exactly the fields ``to_json`` writes.

        Returns
        -------
        RepLine
            The reconstructed value.

        Raises
        ------
        ValueError
            If the JSON is malformed or has the wrong shape.

        Examples
        --------
        >>> import datetime
        >>> from finstack_quant.core.currency import Currency
        >>> from finstack_quant.core.dates import DayCount
        >>> from finstack_quant.core.money import Money
        >>> from finstack_quant.valuations.instruments import RepLine
        >>> restored = RepLine.from_json(
        ...     RepLine(
        ...         "LINE-1",
        ...         Money(1.0, Currency("USD")),
        ...         0.07,
        ...         datetime.date(2031, 1, 15),
        ...         12,
        ...         DayCount.ACT_360,
        ...         asset_type={"type": "first_lien_loan"},
        ...     ).to_json()
        ... )
        >>> restored.to_json() == RepLine(
        ...     "LINE-1",
        ...     Money(1.0, Currency("USD")),
        ...     0.07,
        ...     datetime.date(2031, 1, 15),
        ...     12,
        ...     DayCount.ACT_360,
        ...     asset_type={"type": "first_lien_loan"},
        ... ).to_json()
        True
        """
        ...

    def to_json(self) -> str:
        """
        Serialize to the canonical JSON wire form (also used by ``pickle``).

        Returns
        -------
        str
            Strict JSON accepted by :meth:`from_json`.

        Raises
        ------
        ValueError
            If canonical serialization fails.
        """
        ...

    def to_dict(self) -> dict[str, Any]:
        """
        Return every field as a plain ``dict`` (canonical serde shape).

        Returns
        -------
        dict[str, Any]
            Same content as ``json.loads(self.to_json())``.

        Raises
        ------
        ValueError
            If canonical serialization fails.
        """
        ...

    def __reduce__(self) -> tuple[Any, tuple[str]]:
        """
        Support ``pickle`` via the ``to_json`` / ``from_json`` round-trip.

        Returns
        -------
        tuple[Any, tuple[str]]
            ``(from_json, (payload,))``.

        Raises
        ------
        ValueError
            If the value cannot be reconstructed from its JSON form.
        """
        ...

    @property
    def asset_type(self) -> dict[str, Any]:
        """Return the canonical asset classification and amortization behavior.

        Returns
        -------
        dict[str, Any]
            Rust AssetType wire object, such as ``{"type": "first_lien_loan"}``.

        Raises
        ------
        RuntimeError
            If conversion to a Python dictionary fails.
        """
        ...

    @property
    def id(self) -> str:
        """
        Rep line identifier.

        Returns
        -------
        str
            Unique identifier.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

    @property
    def balance(self) -> Money:
        """
        Aggregated balance.

        Returns
        -------
        Money
            Currency-tagged balance.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

    @property
    def rate(self) -> float:
        """
        Weighted average coupon as an annual decimal rate.

        Returns
        -------
        float
            ``0.07`` for 7%.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

    @property
    def spread_bp(self) -> float | None:
        """
        Weighted average spread in basis points, or ``None`` for fixed lines.

        Returns
        -------
        float | None
            Spread in bp, or ``None``.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

    @property
    def forward_curve_id(self) -> str | None:
        """
        Reference index identifier, or ``None``.

        Returns
        -------
        str | None
            Index id, or ``None``.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

    @property
    def index_floor_bp(self) -> float | None:
        """
        Floor on the floating index in basis points, applied before the spread.

        Returns
        -------
        float | None
            ``None`` when the row is unfloored or fixed-rate.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

    @property
    def contractual_payment(self) -> Money | None:
        """
        Contractual periodic payment for level-pay lines.

        Returns
        -------
        Money | None
            ``None`` when the payment is inferred.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

    @property
    def amortization_term_months(self) -> int | None:
        """
        Amortization schedule length in months from origination.

        Returns
        -------
        int | None
            ``None`` when unset.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

    @property
    def io_months(self) -> int | None:
        """
        Interest-only window in months from origination.

        Returns
        -------
        int | None
            ``None`` when unset.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

    @property
    def maturity(self) -> datetime.date:
        """
        Weighted average maturity date.

        Returns
        -------
        datetime.date
            The maturity date.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

    @property
    def seasoning_months(self) -> int:
        """
        Weighted average seasoning in months.

        Returns
        -------
        int
            Months since origination.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

    @property
    def day_count(self) -> DayCount:
        """
        Day count convention.

        Returns
        -------
        DayCount
            e.g. ``DayCount.ACT_360``.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

    @property
    def cpr(self) -> float | None:
        """
        Constant prepayment rate override (annual decimal), or ``None``.

        Returns
        -------
        float | None
            CPR, or ``None``.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

    @property
    def cdr(self) -> float | None:
        """
        Constant default rate override (annual decimal), or ``None``.

        Returns
        -------
        float | None
            CDR, or ``None``.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

    @property
    def recovery_rate(self) -> float | None:
        """
        Recovery rate override (decimal fraction), or ``None``.

        Returns
        -------
        float | None
            Recovery, or ``None``.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...
