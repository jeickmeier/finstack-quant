"""Stub declarations mirroring ``finstack-quant-py/src/bindings/core/market_data/curves/price.rs``."""

from __future__ import annotations

import datetime
from typing import Any, NotRequired, Optional, Sequence, TypedDict, Union
import pandas as pd
from finstack_quant.core.dates import DayCount
from finstack_quant.core.market_data import DateLike, TimeOrDate

class _PriceCurveOptions(TypedDict):
    id: str
    base_date: DateLike
    knots: Sequence[tuple[float, float]]
    kind: str
    day_count: Union[DayCount, str]
    calendar_id: NotRequired[Optional[str]]
    interp: str
    extrapolation: str

class PriceCurve:
    """
    Forward price curve for commodities, other price-based assets and
    volatility indices.

    Stores ``(t, forward_price)`` knots in years from ``base_date`` in
    absolute price units, or index points (vol points) when
    ``kind="vol_index"``. ``price`` accepts a year fraction or a date.

    Examples
    --------
    >>> from finstack_quant.core.market_data import PriceCurve
    >>> curve = PriceCurve({
    ...     "id": "WTI",
    ...     "base_date": "2025-01-01",
    ...     "knots": [(0.0, 70.0), (1.0, 72.0)],
    ...     "kind": "price",
    ...     "interp": "linear",
    ...     "extrapolation": "flat",
    ...     "day_count": "act_365f",
    ... })
    >>> curve.price(0.5)
    71.0
    >>> vix = PriceCurve({
    ...     "id": "VIX",
    ...     "base_date": "2025-01-01",
    ...     "knots": [(0.0, 18.0), (1.0, 21.0)],
    ...     "kind": "vol_index",
    ...     "interp": "linear",
    ...     "extrapolation": "flat",
    ...     "day_count": "act_365f",
    ... })
    >>> (vix.kind, vix.price(0.0))
    ('vol_index', 18.0)

    """

    def __init__(self, options: _PriceCurveOptions) -> None:
        """
        Construct a price curve from native level pillars and explicit source conventions.

        Parameters
        ----------
        options : dict
            Required fields are ``id``, ISO or Python ``base_date``, ``knots`` (at least two
            strictly increasing nonnegative times in years paired with finite native levels),
            ``kind`` ("price" for signed prices or "vol_index" for nonnegative index points),
            ``day_count`` (DayCount or label), ``interp`` ("linear", "cubic_hermite" or
            positive-only "log_linear"), and ``extrapolation`` ("flat", "extend" or "error").
            Supply a registered ``calendar_id`` for Bus/252. A zero-time source pillar is optional.

        Raises
        ------
        ValueError
            If the source clock, interpolation, tail policy or source pillars are invalid.
        KeyError
            If the supplied calendar identifier is unknown.
        TypeError
            If a required field is absent or an option is unrecognized or mistyped.

        Examples
        --------
        >>> from finstack_quant.core.market_data import PriceCurve
        >>> curve = PriceCurve({
        ...     "id": "WTI",
        ...     "base_date": "2025-01-01",
        ...     "knots": [(0.5, 70.0), (1.0, 72.0)],
        ...     "kind": "price",
        ...     "interp": "linear",
        ...     "extrapolation": "error",
        ...     "day_count": "act_365f",
        ... })
        >>> curve.price(0.75)
        71.0

        """
        ...

    @staticmethod
    def from_json(json: str) -> PriceCurve:
        """
        Deserialize a curve from its canonical JSON wire form.

        Parameters
        ----------
        json : str
            JSON produced by :meth:`to_json`.

        Returns
        -------
        PriceCurve
            The constructed price curve, ready for forward-price queries.

        Raises
        ------
        ValueError
            If the JSON is malformed, has unknown fields, or fails validation.

        Examples
        --------
        >>> from finstack_quant.core.market_data import PriceCurve
        >>> curve = PriceCurve({
        ...     "id": "WTI",
        ...     "base_date": "2025-01-01",
        ...     "knots": [(0.0, 70.0), (1.0, 72.0)],
        ...     "kind": "price",
        ...     "interp": "linear",
        ...     "extrapolation": "flat",
        ...     "day_count": "act_365f",
        ... })
        >>> PriceCurve.from_json(curve.to_json()) == curve
        True

        """
        ...

    def to_json(self) -> str:
        """
        Serialize to the canonical JSON wire form.

        Returns
        -------
        str
            Compact JSON accepted by :meth:`from_json`.

        Raises
        ------
        ValueError
            If the curve cannot be serialized.
        """
        ...

    def price(self, t: TimeOrDate) -> float:
        """
        Forward price (or vol-index level) at a year fraction or date.

        Parameters
        ----------
        t : float, datetime.date or str
            Year fraction from ``base_date``, or a date converted with the
            curve day count.

        Returns
        -------
        float
            Price in the curve's units.

        Raises
        ------
        ValueError
            If the time or price is non-finite, extrapolation is forbidden, a time or date
            precedes the source base, or the evaluated volatility-index level is negative.
        """
        ...

    def bump_parallel_pct(self, pct: float) -> PriceCurve:
        """Return a price curve whose levels have a uniform relative shift.

        Parameters
        ----------
        pct : float
            Signed percentage change applied to every knot. A value of
            2.5 increases each level by 2.5 percent.

        Returns
        -------
        PriceCurve
            Shocked copy with the original knot dates and curve conventions.

        Raises
        ------
        ValueError
            If the shift is non-finite, produces unrepresentable prices, or
            reverses the sign of a volatility-index curve.
        """
        ...

    def to_dataframe(self) -> pd.DataFrame:
        """
        Export knots as a pandas ``DataFrame``.

        Returns
        -------
        pd.DataFrame
            Columns ``t`` (years) and ``price``; one row per knot.

        Raises
        ------
        ValueError
            If the frame cannot be built.
        """
        ...

    @property
    def id(self) -> str:
        """
        Identifier this curve is registered and looked up under.

        Returns
        -------
        str
            The curve id supplied at construction (for example ``"USD-OIS"``); market-data containers key on it exactly.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

    @property
    def base_date(self) -> datetime.date:
        """
        Valuation date the curve's time axis is measured from.

        Returns
        -------
        datetime.date
            The base date; a year fraction ``t`` on this curve means ``t`` years after this date under :attr:`day_count`.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

    @property
    def kind(self) -> str:
        """
        What the stored forward values represent.

        Returns
        -------
        str
            Either ``"price"`` (forward prices in currency units) or ``"vol_index"`` (index levels in volatility points).

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

    @property
    def calendar_id(self) -> Optional[str]:
        """
        Registered calendar used to convert dates on the source clock.

        Returns
        -------
        str or None
            Canonical calendar identifier, or ``None`` for a calendar-independent clock.

        Notes
        -----
        This accessor does not raise.
        """
        ...

    @property
    def knots(self) -> list[float]:
        """
        Pillar times the curve is defined on.

        Returns
        -------
        list[float]
            Strictly ascending year fractions measured from ``base_date``; interpolation happens between neighbouring entries.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

    @property
    def prices(self) -> list[float]:
        """
        Forward levels at the curve pillars.

        Returns
        -------
        list[float]
            One forward per knot, in absolute price units or volatility points depending on :attr:`kind`, in :attr:`knots` order.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

    @property
    def day_count(self) -> str:
        """
        Day-count convention converting calendar dates to curve time.

        Returns
        -------
        str
            Lower-case ISDA day-count label such as ``"act_365f"`` or ``"act_360"``.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

    @property
    def interp_style(self) -> str:
        """
        Interpolation scheme used between pillars.

        Returns
        -------
        str
            Style label such as ``"linear"`` or ``"log_linear"``, fixing how values between knots are produced.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

    @property
    def extrapolation(self) -> str:
        """
        Policy applied beyond the first and last pillar.

        Returns
        -------
        str
            Policy label such as ``"flat"`` or ``"extend"``, governing queries outside the knot range.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

    def __eq__(self, other: object) -> bool: ...
    def __reduce__(self) -> tuple[Any, tuple[str]]:
        """Reconstruct the curve through its validated canonical JSON when unpickling."""
        ...
    def __repr__(self) -> str: ...
    def _repr_html_(self) -> Optional[str]: ...
