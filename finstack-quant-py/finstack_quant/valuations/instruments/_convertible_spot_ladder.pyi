"""Stub declarations mirroring ``finstack-quant-py/src/bindings/valuations/convertible_spot_ladder.rs``."""

from __future__ import annotations

import datetime
from typing import Any, Literal
import pandas as pd
from finstack_quant.core.market_data import MarketContext
from finstack_quant.valuations.instruments._typed_convertible import ConvertibleBond

class ConvertibleSpotRow:
    """
    A convertible at one share price: a row of a :class:`ConvertibleSpotLadder`.

    Examples
    --------
    >>> from finstack_quant.valuations.instruments import ConvertibleSpotRow
    >>> row = ConvertibleSpotRow.from_json(
    ...     '{"spot": 100.0, "is_market_spot": true, "price": 1158348.78, "delta": 7368.0,'
    ...     ' "hedge_ratio": 0.7368, "regime": "balanced", "conversion_value": 1000000.0, "parity": 1.0,'
    ...     ' "conversion_premium": 0.158, "bond_floor": 818730.75}'
    ... )
    >>> row.spot, row.is_market_spot, row.price, row.delta
    (100.0, True, 1158348.78, 7368.0)
    >>> row.hedge_ratio, row.regime, row.conversion_value, row.parity
    (0.7368, 'balanced', 1000000.0, 1.0)
    >>> row.conversion_premium, row.bond_floor
    (0.158, 818730.75)
    """

    @staticmethod
    def from_json(json: str) -> ConvertibleSpotRow:
        """
        Deserialize from the JSON produced by ``to_json``.

        Parameters
        ----------
        json : str
            JSON-encoded ``ConvertibleSpotRow`` (the shape ``to_json`` writes).

        Returns
        -------
        ConvertibleSpotRow
            The decoded value.

        Raises
        ------
        ValueError
            If ``json`` is malformed or carries unknown fields.

        Examples
        --------
        >>> from finstack_quant.valuations.instruments import ConvertibleSpotRow
        >>> try:
        ...     ConvertibleSpotRow.from_json("{}")
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
    def spot(self) -> float:
        """
        Share price of this row, in the bond's currency.

        Returns
        -------
        float
            The share price.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

    @property
    def is_market_spot(self) -> bool:
        """
        Whether :attr:`spot` is the share price the market holds.

        Returns
        -------
        bool
            ``True`` on the market row.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

    @property
    def price(self) -> float:
        """
        Tree value of the whole position on this share price, the bond's PV.

        Returns
        -------
        float
            The value in the ladder's currency, not percent of par.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

    @property
    def delta(self) -> float | None:
        """
        Change in :attr:`price` per unit of share price.

        A central difference with the bond's spot bump, the ``delta`` metric's definition.

        Returns
        -------
        float | None
            The delta in shares, or ``None`` if not finite.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

    @property
    def hedge_ratio(self) -> float | None:
        """
        :attr:`delta` as a share of the effective conversion ratio, the decimal the regime bands read.

        Returns
        -------
        float | None
            The ratio as a decimal, or ``None`` on a mandatory-variable bond, whose share
            count moves with the stock, if the conversion ratio is not positive, or if
            :attr:`delta` is ``None``.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

    @property
    def regime(self) -> Literal["busted", "balanced", "equity_sensitive"] | None:
        """
        Which of bond or stock drives value, from :attr:`hedge_ratio`.

        ``busted`` below 0.40, ``balanced`` from 0.40 to 0.80 inclusive and
        ``equity_sensitive`` above. These are the bands Putnam and Lord Abbett publish;
        managers differ, and MSCI uses thirds.

        Returns
        -------
        Literal["busted", "balanced", "equity_sensitive"] | None
            The regime, or ``None`` where :attr:`hedge_ratio` is.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

    @property
    def conversion_value(self) -> float:
        """
        Immediate conversion value of the position, the ``conversion_value`` metric.

        Returns
        -------
        float
            The value in the ladder's currency.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

    @property
    def parity(self) -> float:
        """
        Conversion value over notional, the ``parity`` metric.

        Returns
        -------
        float
            The ratio; ``1.0`` is par, not one dollar.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

    @property
    def conversion_premium(self) -> float | None:
        """
        :attr:`price` over :attr:`conversion_value`, less one; the ``conversion_premium`` metric.

        Returns
        -------
        float | None
            The premium as a decimal, or ``None`` when conversion value is not positive or
            the ratio is not finite.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

    @property
    def bond_floor(self) -> float:
        """
        Value of the coupons and principal with no conversion or exercise rights, the ``bond_floor`` metric.

        Returns
        -------
        float
            The value in the ladder's currency; it does not depend on the share price, so
            every row holds the same number.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

class ConvertibleSpotLadder:
    """
    A convertible at a ladder of share prices.

    Returned by :func:`convertible_spot_ladder`.

    Examples
    --------
    >>> from finstack_quant.valuations.instruments import ConvertibleSpotLadder
    >>> ladder = ConvertibleSpotLadder.from_json(
    ...     '{"instrument_id": "CB", "as_of": "2025-01-15", "currency": "USD", "market_spot": 100.0, "rows": []}'
    ... )
    >>> ladder.instrument_id, ladder.currency, ladder.market_spot, ladder.rows
    ('CB', 'USD', 100.0, [])
    >>> ladder.as_of.isoformat()
    '2025-01-15'
    """

    @staticmethod
    def from_json(json: str) -> ConvertibleSpotLadder:
        """
        Deserialize from the JSON produced by ``to_json``.

        Parameters
        ----------
        json : str
            JSON-encoded ``ConvertibleSpotLadder`` (the shape ``to_json`` writes).

        Returns
        -------
        ConvertibleSpotLadder
            The decoded value.

        Raises
        ------
        ValueError
            If ``json`` is malformed or carries unknown fields.

        Examples
        --------
        >>> from finstack_quant.valuations.instruments import ConvertibleSpotLadder
        >>> try:
        ...     ConvertibleSpotLadder.from_json("{}")
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
        The rows as a pandas ``DataFrame``, in ascending share price.

        Columns: ``spot``, ``is_market_spot``, ``price``, ``delta``, ``hedge_ratio``,
        ``regime``, ``conversion_value``, ``parity``, ``conversion_premium`` and
        ``bond_floor``. A null number is ``NaN``.

        Returns
        -------
        pd.DataFrame
            One row per share price.

        Raises
        ------
        ImportError
            If ``pyarrow`` is not installed.
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
        Valuation date of the ladder.

        Returns
        -------
        datetime.date
            The valuation date.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

    @property
    def currency(self) -> str:
        """
        Currency of the bond, which every money field is in.

        Returns
        -------
        str
            The ISO-4217 code.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

    @property
    def market_spot(self) -> float:
        """
        Share price the market holds.

        Returns
        -------
        float
            The share price.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

    @property
    def rows(self) -> list[ConvertibleSpotRow]:
        """
        One row per share price, in ascending order.

        Returns
        -------
        list[ConvertibleSpotRow]
            The rows.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

def convertible_spot_ladder(
    bond: ConvertibleBond,
    market: MarketContext | str,
    as_of: datetime.date | str,
    spots: list[float] | None = None,
) -> ConvertibleSpotLadder:
    """
    Price, delta, parity, conversion premium, regime and bond floor of a convertible at a ladder of share prices.

    Each row reprices on the market with only the share price moved, on the
    convertible tree, the bond's only model, so volatility, rates, dividends and
    credit stay as supplied. Price, delta, parity, conversion value, premium and floor
    are the definitions of the metrics of those names. Each row runs the tree three times.

    Parameters
    ----------
    bond : ConvertibleBond
        Convertible to value. Its ``metric_pricing_overrides.bump_config`` sets the
        spot bump behind delta (default 1% of each row's share price).
    market : MarketContext or str
        Typed ``MarketContext`` or serialized market-context JSON holding the bond's
        discount curve, share price, volatility and any dividend yield, hazard curve
        and floating index.
    as_of : datetime.date | str
        Valuation date and origin of the tree's ACT/365F clock, either a date-like
        object or an ISO 8601 string.
    spots : list[float] or None, optional
        Share prices in the bond's currency, each finite and positive, sorted ascending
        with duplicates dropped. ``None`` uses 0.25, 0.5, 0.75, 1, 1.25, 1.5, 2 and 3
        times the market's share price.

    Returns
    -------
    ConvertibleSpotLadder
        A row per share price and the market's share price.

    Raises
    ------
    KeyError
        If market data the bond needs is missing.
    ValueError
        If ``spots`` is empty or holds a value that is not finite and positive, the
        bond fails validation, or ``as_of`` or the market is malformed.
    RuntimeError
        If the tree pricer fails.

    Examples
    --------
    >>> import datetime, json
    >>> from finstack_quant.core.market_data import DiscountCurve, HazardCurve, MarketContext
    >>> from finstack_quant.valuations.instruments import ConvertibleBond, convertible_spot_ladder
    >>> spec = json.loads(ConvertibleBond.example().to_json())
    >>> spec["instrument"]["spec"]["conversion"]["ratio"] = 10_000.0
    >>> bond = ConvertibleBond.from_json(json.dumps(spec))
    >>> as_of = datetime.date(2025, 1, 15)
    >>> market = (
    ...     MarketContext()
    ...     .insert(DiscountCurve.flat("USD-IG", as_of, 0.04))
    ...     .insert(HazardCurve("USD-CREDIT-BBB", as_of, [(1.0, 0.025)], recovery_rate=0.0))
    ...     .insert_price("TECH", 100.0, "USD")
    ...     .insert_price("TECH-VOL", 0.30)
    ... )
    >>> ladder = convertible_spot_ladder(bond, market, as_of, [50.0, 100.0, 300.0])
    >>> [row.parity for row in ladder.rows]
    [0.5, 1.0, 3.0]
    >>> [row.regime for row in ladder.rows]
    ['busted', 'balanced', 'equity_sensitive']
    """
    ...
