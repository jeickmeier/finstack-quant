"""Stub declarations mirroring ``finstack-quant-py/src/bindings/core/market_data/curves/price.rs``."""

from __future__ import annotations

import datetime
from typing import Any, Optional, Sequence, Union
import pandas as pd
from finstack_quant.core.dates import DayCount
from finstack_quant.core.market_data import DateLike, TimeOrDate

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
    >>> curve = PriceCurve("WTI", "2025-01-01", [(0.0, 70.0), (1.0, 72.0)])
    >>> curve.price(0.5)
    71.0
    >>> vix = PriceCurve("VIX", "2025-01-01", [(0.0, 18.0), (1.0, 21.0)], kind="vol_index")
    >>> (vix.kind, vix.spot_price)
    ('vol_index', 18.0)

    """

    def __init__(
        self,
        id: str,
        base_date: DateLike,
        knots: Sequence[tuple[float, float]],
        *,
        kind: Optional[str] = None,
        spot_price: Optional[float] = None,
        extrapolation: Optional[str] = None,
        interp: Optional[str] = None,
        day_count: Optional[Union[DayCount, str]] = None,
    ) -> None:
        """
        Construct a price curve from ``(time_years, forward_price)`` knots.

        Parameters
        ----------
        id : str
            Unique curve identifier (e.g. ``"WTI-FORWARD"`` or ``"VIX"``).
        base_date : datetime.date or str
            Valuation date anchoring ``t = 0``.
        knots : Sequence[tuple[float, float]]
            ``(time_years, forward_price)`` pairs in absolute price units. At
            least two knots; the first must be at ``t = 0`` unless
            ``spot_price`` is given.
        kind : str, optional
            ``"price"`` (default; signed prices allowed) or ``"vol_index"``
            (non-negative volatility-index levels in vol points, e.g. ``18.0``).
            A ``"vol_index"`` curve is stored and retrieved through
            :meth:`MarketContext.get_vol_index_curve`.
        spot_price : float, optional
            Spot level at ``t = 0``; inferred from a ``t = 0`` knot when omitted.
            An explicit spot is inserted at zero if that knot is absent and
            must match any supplied zero-time knot.
        extrapolation : str, optional
            Extrapolation policy; default ``"flat_zero"``.
        interp : str, optional
            Interpolation style; default ``"linear"``.
        day_count : DayCount | str, optional
            Day-count label; default ``"act_365f"``.

        Raises
        ------
        ValueError
            If fewer than two knots are given, a knot is non-finite or
            duplicated, spot cannot be inferred or conflicts with a zero-time
            knot, a vol-index level is
            negative, or a label is unknown.

        Examples
        --------
        >>> from finstack_quant.core.market_data import PriceCurve
        >>> PriceCurve("WTI", "2025-01-01", [(0.5, 70.0), (1.0, 72.0)], spot_price=69.5).spot_price
        69.5

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
        >>> curve = PriceCurve("WTI", "2025-01-01", [(0.0, 70.0), (1.0, 72.0)])
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
            If a date precedes ``base_date``.
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
    def spot_price(self) -> float:
        """
        Level of the underlying on the base date.

        Returns
        -------
        float
            Spot price in the curve's currency, or the index level in volatility points when :attr:`kind` is ``"vol_index"``.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
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
            Policy label such as ``"flat_zero"`` or ``"flat_forward"``, governing queries outside the knot range.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

    def __eq__(self, other: object) -> bool: ...
    def __reduce__(self) -> tuple[Any, tuple[str]]: ...
    def __repr__(self) -> str: ...
    def _repr_html_(self) -> Optional[str]: ...
