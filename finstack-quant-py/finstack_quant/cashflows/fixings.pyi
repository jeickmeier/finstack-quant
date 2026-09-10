"""Raw coupon observations and deterministic fixing materialization.

Examples
--------
>>> from finstack_quant.cashflows.fixings import ProjectedFixing
>>> ProjectedFixing("FIXING:USD-SOFR", "2025-01-03", 0.03).value
0.03
"""

from __future__ import annotations

import datetime

from finstack_quant.cashflows.builder import CashFlowSchedule
from finstack_quant.core.market_data import MarketContext

__all__ = ["ProjectedFixing", "materialize_fixings"]

class ProjectedFixing:
    """Raw observation used by coupon projection before contractual adjustments.

    Examples
    --------
    >>> from finstack_quant.cashflows.fixings import ProjectedFixing
    >>> observation = ProjectedFixing("FIXING:USD-SOFR", "2025-01-03", 0.03)
    >>> observation.series_id, str(observation.date), observation.value
    ('FIXING:USD-SOFR', '2025-01-03', 0.03)
    """

    def __init__(self, series_id: str, date: datetime.date | str, value: float | None = None) -> None:
        """Construct an observation retained in schedule metadata.

        Parameters
        ----------
        series_id : str
            Canonical ``FIXING:`` identifier, including the tenor of a CMS index
            or the quote orientation of an FX reset series.
        date : datetime.date or str
            Contractual observation date after fixing-calendar adjustments.
        value : float, optional
            Raw annualized decimal index rate before spread, gearing, caps and
            floors, or quote currency per base currency for FX observations.
            ``None`` records an unavailable projection dependency.

        Raises
        ------
        ValueError
            If the observation date cannot be parsed. Identifier/value validation
            occurs when a roll crosses the observation and needs to use it.
        """
        ...

    @property
    def series_id(self) -> str:
        """Return the canonical fixing-series identifier and quote orientation.

        Returns
        -------
        str
            Stored identifier; no market lookup is performed.

        Notes
        -----
        This accessor does not raise.
        """
        ...

    @property
    def date(self) -> datetime.date:
        """Return the contractual observation date.

        Returns
        -------
        datetime.date
            Calendar-adjusted date at which the raw index observation is fixed.

        Notes
        -----
        This accessor does not raise.
        """
        ...

    @property
    def value(self) -> float | None:
        """Return the raw rate or FX quote before coupon transformations.

        Returns
        -------
        float, optional
            Annualized decimal index rate or oriented FX quote; ``None`` denotes
            an unavailable projection that must be supplied before crossing.

        Notes
        -----
        This accessor does not raise.
        """
        ...

def materialize_fixings(
    market: MarketContext,
    schedules: list[CashFlowSchedule],
    old_date: datetime.date | str,
    new_date: datetime.date | str,
) -> MarketContext:
    """Return a new market containing observations crossed in ``(old_date, new_date]``.

    Parameters
    ----------
    market : MarketContext
        Immutable pre-roll projection market. Existing exact-date observations
        take precedence and retain their values, currencies and interpolation.
    schedules : list[CashFlowSchedule]
        Canonical schedules projected on the pre-roll market. Raw observations
        come from their ``meta.projected_fixings``; coupon spreads are excluded.
    old_date : datetime.date or str
        Exclusive fixing-window origin. Earlier missing fixings are not invented.
    new_date : datetime.date or str
        Inclusive fixing horizon on or after the origin. Curves are not rolled
        by this helper; scenario time rolls call it before changing curve dates.

    Returns
    -------
    MarketContext
        Independent market with new exact-date fixings and unchanged source curves.
        Repeating the same window preserves existing observations deterministically.

    Raises
    ------
    ValueError
        If dates cannot be parsed, the horizon moves backward, a crossed series
        identifier is malformed, or a required projection is missing, non-finite
        or conflicts with another projection for the same index/date.

    Examples
    --------
    >>> from finstack_quant.cashflows.fixings import materialize_fixings
    >>> from finstack_quant.core.market_data import MarketContext
    >>> rolled = materialize_fixings(MarketContext(), [], "2025-01-02", "2025-01-03")
    >>> isinstance(rolled, MarketContext)
    True
    """
    ...
