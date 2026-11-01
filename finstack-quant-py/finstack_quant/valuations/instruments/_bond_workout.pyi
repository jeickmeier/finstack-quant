"""Stub declarations mirroring ``finstack-quant-py/src/bindings/valuations/bond_workout.rs``."""

from __future__ import annotations

import datetime
from typing import Any, Literal
import pandas as pd
from finstack_quant.core.market_data import MarketContext
from finstack_quant.core.money import Money
from finstack_quant.valuations.instruments._instruments import Bond

class BondWorkoutRow:
    """
    One admissible exit of a bond: a row of a :class:`BondWorkoutLadder`.

    Examples
    --------
    >>> from finstack_quant.valuations.instruments import BondWorkoutRow
    >>> row = BondWorkoutRow.from_json(
    ...     '{"date": "2027-01-15", "kind": "call", "redemption_price_pct": 103.0,'
    ...     ' "redemption_amount": {"amount": "1030000", "currency": "USD"}, "make_whole": false,'
    ...     ' "yield_rate": 0.0375, "i_spread": 0.002, "is_worst": true}'
    ... )
    >>> row.date.isoformat(), row.kind, row.redemption_price_pct, row.yield_rate, row.is_worst
    ('2027-01-15', 'call', 103.0, 0.0375, True)
    >>> row.redemption_amount.amount, row.make_whole, row.i_spread
    (1030000.0, False, 0.002)
    """

    @staticmethod
    def from_json(json: str) -> BondWorkoutRow:
        """
        Deserialize from the JSON produced by ``to_json``.

        Parameters
        ----------
        json : str
            JSON-encoded ``BondWorkoutRow`` (the shape ``to_json`` writes).

        Returns
        -------
        BondWorkoutRow
            The decoded value.

        Raises
        ------
        ValueError
            If ``json`` is malformed or carries unknown fields.

        Examples
        --------
        >>> from finstack_quant.valuations.instruments import BondWorkoutRow
        >>> try:
        ...     BondWorkoutRow.from_json("{}")
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
            The row as canonical JSON.

        Raises
        ------
        ValueError
            If the value cannot be serialized.
        """
        ...

    def to_dict(self) -> dict[str, Any]:
        """
        Return the row's wire form as a dictionary.

        Returns
        -------
        dict[str, Any]
            The parsed form of :meth:`to_json`.

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

    def __repr__(self) -> str:
        """
        Return ``repr(self)``.

        Returns
        -------
        str
            A one-line summary of the row.
        """
        ...

    @property
    def date(self) -> datetime.date:
        """
        Date the holder leaves the bond on this exit.

        A maturity row carries the business-day-adjusted final payment date when
        that is later than the contractual maturity.

        Returns
        -------
        datetime.date
            The date the holder leaves the bond.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

    @property
    def kind(self) -> Literal["call", "put", "maturity"]:
        """
        How the holder leaves the bond on :attr:`date`.

        A return floor lowered into the call schedule appears as ``"call"``.

        Returns
        -------
        Literal["call", "put", "maturity"]
            The kind of exit.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

    @property
    def redemption_price_pct(self) -> float:
        """
        Clean redemption as percent of the principal outstanding at the exit.

        A make-whole call shows its make-whole price, which can exceed the
        contractual strike.

        Returns
        -------
        float
            The price in percent points (``103.0`` = 103%).

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

    @property
    def redemption_amount(self) -> Money:
        """
        Clean cash the exit redeems; accrued interest is paid on top.

        Returns
        -------
        Money
            The amount in the bond's currency.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

    @property
    def make_whole(self) -> bool:
        """
        Whether the redemption is a make-whole price off a reference curve.

        Returns
        -------
        bool
            ``True`` for a make-whole call.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

    @property
    def yield_rate(self) -> float | None:
        """
        Street yield to this exit, compounded at the coupon frequency and timed from the quote date.

        Returns
        -------
        float | None
            The yield as a decimal, or ``None`` when the path has no finite yield:
            an exit on the quote date, or same-day cash at or above the price. A
            worst row can be ``None``, where the ``ytw`` metric is negative infinity.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

    @property
    def i_spread(self) -> float | None:
        """
        :attr:`yield_rate` less the swap par rate from the quote date.

        This is the convention of the ``i_spread`` metric. The par rate runs to
        :attr:`date` when the bond has an exit other than maturity, and to contractual
        maturity when it has none, which differs from :attr:`date` if the final
        payment rolls.

        Returns
        -------
        float | None
            The spread as a decimal, or ``None`` when :attr:`yield_rate` is ``None``.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

    @property
    def is_worst(self) -> bool:
        """
        Whether this is the worst exit, the one the ``ytw`` metric selects.

        At most one row of a ladder is marked; none is when no exit has a yield and
        each pays at least the price at once.

        Returns
        -------
        bool
            ``True`` on the worst row.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

class BondWorkoutLadder:
    """
    Yield and spread to every exit of a bond at one market price.

    Returned by :func:`bond_workout_ladder`.

    Examples
    --------
    >>> from finstack_quant.valuations.instruments import BondWorkoutLadder
    >>> ladder = BondWorkoutLadder.from_json(
    ...     '{"instrument_id": "B", "as_of": "2025-01-15", "quote_date": "2025-01-16",'
    ...     ' "clean_price_pct": 101.0, "dirty_price": {"amount": "1010000", "currency": "USD"}, "rows": []}'
    ... )
    >>> ladder.instrument_id, ladder.clean_price_pct, ladder.dirty_price.amount, ladder.rows
    ('B', 101.0, 1010000.0, [])
    >>> ladder.as_of.isoformat(), ladder.quote_date.isoformat()
    ('2025-01-15', '2025-01-16')
    """

    @staticmethod
    def from_json(json: str) -> BondWorkoutLadder:
        """
        Deserialize from the JSON produced by ``to_json``.

        Parameters
        ----------
        json : str
            JSON-encoded ``BondWorkoutLadder`` (the shape ``to_json`` writes).

        Returns
        -------
        BondWorkoutLadder
            The decoded value.

        Raises
        ------
        ValueError
            If ``json`` is malformed or carries unknown fields.

        Examples
        --------
        >>> from finstack_quant.valuations.instruments import BondWorkoutLadder
        >>> try:
        ...     BondWorkoutLadder.from_json("{}")
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
            The ladder as canonical JSON.

        Raises
        ------
        ValueError
            If the value cannot be serialized.
        """
        ...

    def to_dict(self) -> dict[str, Any]:
        """
        Return the ladder's wire form as a dictionary.

        Returns
        -------
        dict[str, Any]
            The parsed form of :meth:`to_json`.

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

    def __repr__(self) -> str:
        """
        Return ``repr(self)``.

        Returns
        -------
        str
            A one-line summary of the ladder.
        """
        ...

    def to_dataframe(self) -> pd.DataFrame:
        """
        The rows as a pandas ``DataFrame``, in date order.

        Columns: ``date`` (ISO string), ``kind``, ``redemption_price_pct``,
        ``redemption_amount``, ``currency``, ``make_whole``, ``yield_rate``,
        ``i_spread`` and ``is_worst``. A null yield or spread is ``NaN``.

        Returns
        -------
        pd.DataFrame
            One row per exit.

        Raises
        ------
        ValueError
            If the rows cannot be converted to a frame.
        """
        ...

    @property
    def instrument_id(self) -> str:
        """
        Identifier of the bond.

        Returns
        -------
        str
            The bond's id.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

    @property
    def as_of(self) -> datetime.date:
        """
        Valuation date the ladder was requested at.

        Returns
        -------
        datetime.date
            The date the ladder was requested at.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

    @property
    def quote_date(self) -> datetime.date:
        """
        Settlement date of the quote; every yield is timed from it.

        Returns
        -------
        datetime.date
            The quote date.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

    @property
    def clean_price_pct(self) -> float:
        """
        Clean price every row is solved at, as percent of the original notional.

        The quoted clean price, or for a bond with no exercise rights and no quote
        the model price at the quote date.

        Returns
        -------
        float
            The price in percent points.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

    @property
    def dirty_price(self) -> Money:
        """
        :attr:`clean_price_pct` plus accrued interest at the quote date.

        Exits are alternatives for one price, so the rows share it.

        Returns
        -------
        Money
            The price in the bond's currency.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

    @property
    def rows(self) -> list[BondWorkoutRow]:
        """
        One row per candidate exit, in date order.

        Adjacent call windows that share a boundary date each keep a row; a put and
        a call on one date each keep theirs. A bond with no exercise rights has one
        maturity row.

        Returns
        -------
        list[BondWorkoutRow]
            The rows.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

def bond_workout_ladder(bond: Bond, market: MarketContext | str, as_of: datetime.date | str) -> BondWorkoutLadder:
    """
    Yield and I-spread to every call, put and maturity exit of a bond, with the worst marked.

    Answers which call date is the worst workout and what the yield to each is.
    The exits are the ones the ``ytw`` metric walks, a return floor lowered into
    calls first, and the row marked worst holds that metric's yield and the
    ``i_spread`` metric's spread. Z-spread is not reported per exit: the path that
    metric discounts is the worst one only, and no per-exit convention exists.

    Parameters
    ----------
    bond : Bond
        Bond to walk. A bond with exercise rights must carry
        ``quoted_clean_price_pct``; one with none falls back to its model price.
    market : MarketContext or str
        Typed ``MarketContext`` or serialized market-context JSON, holding the
        bond's discount curve (the source of the swap par rates) and any floating
        index and make-whole reference curves.
    as_of : datetime.date | str
        Valuation date, either a date-like object or an ISO 8601 string. The quote
        date and coupon entitlement follow the bond's settlement convention.

    Returns
    -------
    BondWorkoutLadder
        One row per exit in date order, and the one price they are solved at.

    Raises
    ------
    KeyError
        If a bond with exercise rights has no quoted clean price, or a curve the
        bond needs is missing.
    ValueError
        If ``as_of`` is not a date or the market is malformed.
    RuntimeError
        If a yield cannot be solved.

    Examples
    --------
    >>> import datetime, json
    >>> from finstack_quant.core.market_data import DiscountCurve, MarketContext
    >>> from finstack_quant.valuations.instruments import Bond, bond_workout_ladder
    >>> spec = json.loads(Bond.example_callable().to_json())
    >>> spec["instrument"]["spec"]["instrument_pricing_overrides"] = {
    ...     "market_quotes": {"quoted_clean_price_pct": 108.0}
    ... }
    >>> bond = Bond.from_json(json.dumps(spec))
    >>> as_of = datetime.date(2025, 1, 15)
    >>> market = MarketContext().insert(DiscountCurve.flat("USD-OIS", as_of, 0.04))
    >>> ladder = bond_workout_ladder(bond, market, as_of)
    >>> [row.kind for row in ladder.rows][-1]
    'maturity'
    >>> sum(row.is_worst for row in ladder.rows)
    1
    """
    ...
