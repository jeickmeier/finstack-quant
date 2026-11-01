"""Stub declarations mirroring ``finstack-quant-py/src/bindings/valuations/bond_quotes.rs``."""

from __future__ import annotations

import datetime
from typing import Any
import pandas as pd
from finstack_quant.core.market_data import MarketContext
from finstack_quant.valuations.instruments._instruments import Bond

class BondQuoteSet:
    """
    Every price, yield and spread quote of a bond, solved from any one of them.

    The value :func:`compute_quotes` returns. Yields and spreads are decimals
    (``0.01`` = 100bp), each the value of the metric of the same name on the bond at
    this clean price. An optional field is ``None`` when it does not apply to the bond
    or its metric has no finite value, never infinity or NaN.

    Examples
    --------
    >>> from finstack_quant.valuations.instruments import BondQuoteSet
    >>> quotes = BondQuoteSet.from_json(
    ...     '{"clean_price_currency": 985000.0, "clean_price_pct": 98.5, "dirty_price_currency": 995000.0,'
    ...     ' "ytm": 0.055, "ytw": 0.055, "z_spread": 0.012, "discount_margin": null, "oas": 0.011,'
    ...     ' "asw_par": 0.01, "asw_market": 0.0105, "i_spread": 0.0118,'
    ...     ' "japanese_simple_yield": 0.054, "moosmuller_ytm": 0.0548}'
    ... )
    >>> quotes.clean_price_pct, quotes.ytm, quotes.discount_margin
    (98.5, 0.055, None)
    >>> quotes.clean_price_currency, quotes.dirty_price_currency, quotes.ytw, quotes.z_spread
    (985000.0, 995000.0, 0.055, 0.012)
    >>> quotes.oas, quotes.asw_par, quotes.asw_market, quotes.i_spread
    (0.011, 0.01, 0.0105, 0.0118)
    >>> quotes.japanese_simple_yield, quotes.moosmuller_ytm
    (0.054, 0.0548)
    """

    @staticmethod
    def from_json(json: str) -> BondQuoteSet:
        """
        Deserialize from the JSON produced by ``to_json``.

        Parameters
        ----------
        json : str
            JSON-encoded ``BondQuoteSet`` (the shape ``to_json`` writes).

        Returns
        -------
        BondQuoteSet
            The decoded value.

        Raises
        ------
        ValueError
            If ``json`` is malformed or carries unknown fields.

        Examples
        --------
        >>> from finstack_quant.valuations.instruments import BondQuoteSet
        >>> try:
        ...     BondQuoteSet.from_json("{}")
        ... except ValueError:
        ...     print("rejected")
        rejected
        """
        ...

    def to_json(self) -> str:
        """
        Serialize to the JSON shape ``from_json`` accepts.

        Returns
        -------
        str
            The set as canonical JSON, with ``null`` for a field that is ``None``.

        Raises
        ------
        ValueError
            If the value cannot be serialized.
        """
        ...

    def to_dict(self) -> dict[str, Any]:
        """
        Return every field as a plain ``dict`` (canonical serde shape).

        Returns
        -------
        dict[str, Any]
            Field name to value, with ``None`` for a field that is ``None``.

        Raises
        ------
        ValueError
            If the value cannot be serialized.
        """
        ...

    def __reduce__(self) -> tuple[Any, tuple[str]]:
        """
        Support ``pickle`` through the ``to_json`` / ``from_json`` round-trip.

        Returns
        -------
        tuple[Any, tuple[str]]
            :meth:`from_json` and the ``to_json`` text.

        Raises
        ------
        ValueError
            If serialization fails.
        """
        ...

    def to_dataframe(self) -> pd.DataFrame:
        """
        Return the set as a one-row table, one column per field.

        Returns
        -------
        pandas.DataFrame
            Thirteen ``float64`` columns in field order; a field that is ``None`` is
            ``NaN``.

        Raises
        ------
        ValueError
            If the value cannot be serialized.
        """
        ...

    @property
    def clean_price_currency(self) -> float:
        """
        Clean price in the bond's currency, for its whole notional.

        Returns
        -------
        float
            The price.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

    @property
    def clean_price_pct(self) -> float:
        """
        Clean price as percentage of par (``99.5`` = 99.5%, the quote convention).

        Returns
        -------
        float
            The price.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

    @property
    def dirty_price_currency(self) -> float:
        """
        Dirty price at the quote date in the bond's currency, for its whole notional: clean price plus accrued.

        Returns
        -------
        float
            The price.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

    @property
    def ytm(self) -> float | None:
        """
        Street yield to maturity as a decimal, compounded at the coupon frequency.

        Returns
        -------
        float or None
            The value, or ``None`` when it does not apply or has no finite value.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

    @property
    def ytw(self) -> float | None:
        """
        Yield to worst across every call, put and maturity exit, as a decimal.

        Returns
        -------
        float or None
            The value, or ``None`` when it does not apply or has no finite value.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

    @property
    def z_spread(self) -> float | None:
        """
        Z-spread over the discount curve as a decimal; on a bond with exercise rights it discounts the worst exit's flows.

        Returns
        -------
        float or None
            The value, or ``None`` when it does not apply or has no finite value.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

    @property
    def discount_margin(self) -> float | None:
        """
        Discount margin as a decimal; ``None`` unless the bond has floating coupons.

        Returns
        -------
        float or None
            The value, or ``None`` when it does not apply or has no finite value.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

    @property
    def oas(self) -> float | None:
        """
        Option-adjusted spread as a decimal, from the bond's tree model.

        Returns
        -------
        float or None
            The value, or ``None`` when it does not apply or has no finite value.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

    @property
    def asw_par(self) -> float | None:
        """
        Asset swap par spread as a decimal; ``None`` unless the bond has a fixed coupon and no custom cashflows, or custom cashflows with a floating specification.

        Returns
        -------
        float or None
            The value, or ``None`` when it does not apply or has no finite value.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

    @property
    def asw_market(self) -> float | None:
        """
        Asset swap market spread as a decimal; ``None`` under the same rule as ``asw_par``, including on a plain floating-rate note.

        Returns
        -------
        float or None
            The value, or ``None`` when it does not apply or has no finite value.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

    @property
    def i_spread(self) -> float | None:
        """
        I-spread as a decimal: the yield less the swap par rate, to the worst exit on a bond with exercise rights.

        Returns
        -------
        float or None
            The value, or ``None`` when it does not apply or has no finite value.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

    @property
    def japanese_simple_yield(self) -> float | None:
        """
        Japanese simple yield as a decimal; ``None`` unless the coupon is fixed.

        Returns
        -------
        float or None
            The value, or ``None`` when it does not apply or has no finite value.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

    @property
    def moosmuller_ytm(self) -> float | None:
        """
        Moosmüller yield to maturity as a decimal: simple interest to the next coupon, then periodic compounding.

        Returns
        -------
        float or None
            The value, or ``None`` when it does not apply or has no finite value.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

    def __repr__(self) -> str:
        """
        Return ``repr(self)``.

        Returns
        -------
        str
            A one-line summary of the set.
        """
        ...

def compute_quotes(
    bond: Bond,
    market: MarketContext | str,
    as_of: datetime.date | str,
    quote_input: dict[str, float] | str,
) -> BondQuoteSet:
    """
    Solve every price, yield and spread quote of a bond from any one of them.

    The quote is normalized to a clean price and every other field is the metric of
    the same name at that price, in one call and with no repricing loop. The bond's
    default pricing model runs (tree when it has exercise rights) on the bond family's
    registries; the bond passed in is unchanged and any quote it carries is replaced.

    Parameters
    ----------
    bond : Bond
        Bond to convert. A bond with exercise rights needs HW1F parameters on the bond
        or in the market, because its ``oas`` is part of the set.
    market : MarketContext or str
        Typed ``MarketContext`` or serialized market-context JSON, holding the bond's
        discount curve, the source of the swap par rates, and any floating index curve.
    as_of : datetime.date | str
        Valuation date, either a date-like object or an ISO 8601 string. The quote
        date and accrued interest follow the bond's settlement convention.
    quote_input : dict[str, float] | str
        One ``BondQuoteInput``, a dict or JSON string with exactly one key:
        ``clean_price_pct`` (percent of par), ``dirty_price_currency`` (currency, the
        whole notional), ``ytm``, ``ytw``, ``z_spread``, ``discount_margin``, ``oas``,
        ``asw_market``, ``i_spread`` or ``japanese_simple_yield`` (decimals, ``0.01`` =
        100bp).

    Returns
    -------
    BondQuoteSet
        The whole set. A field is ``None`` when it does not apply to the bond or has no
        finite value.

    Raises
    ------
    ValueError
        If ``quote_input`` has an unknown or second key or a non-finite value, a dirty
        price is not positive, ``as_of`` is not a date, or the market is malformed. A
        quote kind the bond does not have raises it too: ``discount_margin`` on a bond
        with no floating coupons, ``asw_market`` or ``japanese_simple_yield`` on a
        floating coupon, and ``z_spread`` on a bond with exercise rights. So does a
        metric of the set that cannot be solved, which fails the whole set:
        ``z_spread`` on a bond with a call or put on the quote date below the price,
        or ``oas`` on a bond with exercise rights and no HW1F parameters.
    KeyError
        If a curve the bond needs to normalize the quote is missing.

    Notes
    -----
    ``asw_market`` prices the swap to maturity while the metric runs it to the worst
    exit, so on a bond whose worst exit is early the set's ``asw_market`` is not the
    input, and feeding it back does not recover the price.

    Examples
    --------
    >>> import datetime
    >>> from finstack_quant.core.market_data import DiscountCurve, MarketContext
    >>> from finstack_quant.valuations.instruments import Bond, compute_quotes
    >>> bond = Bond.example()
    >>> as_of = datetime.date(2025, 1, 15)
    >>> market = MarketContext().insert(DiscountCurve.flat("USD-TREASURY", as_of, 0.04))
    >>> quotes = compute_quotes(bond, market, as_of, {"clean_price_pct": 98.5})
    >>> quotes.clean_price_pct, quotes.discount_margin
    (98.5, None)
    >>> back = compute_quotes(bond, market, as_of, {"ytm": quotes.ytm})
    >>> round(back.clean_price_pct, 6)
    98.5
    """
    ...
