"""Stub declarations mirroring ``finstack-quant-py/src/bindings/core/dates/sifma.rs``."""

from __future__ import annotations

import datetime

class SifmaSettlementClass:
    """
    SIFMA good-delivery settlement class.

    Examples
    --------
    >>> from finstack_quant.core.dates import SifmaSettlementClass
    >>> SifmaSettlementClass.from_agency_term("FNMA", 30) == SifmaSettlementClass.A
    True

    """

    A: SifmaSettlementClass
    """Class A: 30-year conventional (FNMA/FHLMC) and UMBS pools."""
    B: SifmaSettlementClass
    """Class B: 15-year fixed-rate agency pools."""
    C: SifmaSettlementClass
    """Class C: 30-year GNMA single-family pools."""
    D: SifmaSettlementClass
    """Class D: balloons, ARMs, multifamily and other non-standard programs."""

    @classmethod
    def from_agency_term(cls, agency: str, term_years: int) -> SifmaSettlementClass:
        """
        Infer the SIFMA good-delivery class from an agency and term.

        Parameters
        ----------
        agency : str
            Agency or program label, such as ``"FNMA"``, ``"FHLMC"``, or
            ``"GNMA"``, interpreted using the library's settlement table.
        term_years : int
            Original mortgage term in whole years, normally ``15`` or ``30``.

        Returns
        -------
        SifmaSettlementClass
            Settlement class used to select the monthly SIFMA delivery date.

        Notes
        -----
        This method does not raise; it returns a fixed instance.

        Examples
        --------
        >>> from finstack_quant.core.dates import SifmaSettlementClass
        >>> SifmaSettlementClass.from_agency_term("FNMA", 30) == SifmaSettlementClass.A
        True
        """
        ...

def sifma_settlement_date(
    month: int, year: int, settlement_class: SifmaSettlementClass | None = None
) -> datetime.date | None:
    """
    Return the published SIFMA settlement date for a delivery month and class.

    Parameters
    ----------
    month : int
        Delivery month number from ``1`` through ``12``.
    year : int
        Four-digit delivery calendar year.
    settlement_class : SifmaSettlementClass, optional
        Good-delivery class inferred from the agency/program and mortgage term.
        ``None`` uses the Rust default, Class A (conventional 30-year UMBS).

    Returns
    -------
    datetime.date | None
        The exact published settlement date for the requested month, year,
        and settlement class, or ``None`` when the embedded SIFMA calendar
        has no date for that combination.

    Raises
    ------
    ValueError
        If *month* is outside ``1`` through ``12``.

    Examples
    --------
    >>> import datetime
    >>> from finstack_quant.core.dates import sifma_settlement_date, SifmaSettlementClass
    >>> sifma_settlement_date(1, 2026)
    datetime.date(2026, 1, 14)
    >>> sifma_settlement_date(1, 2026, SifmaSettlementClass.B)
    datetime.date(2026, 1, 20)

    """
    ...

def estimated_sifma_settlement_date(month: int, year: int, settlement_class: SifmaSettlementClass) -> datetime.date:
    """
    Estimate a class-specific SIFMA settlement date for projections.

    This uses the holiday-calendar business-day anchor even when a published
    settlement exists; use ``sifma_settlement_date`` for operational dates.

    Parameters
    ----------
    month : int
        Delivery month number from ``1`` through ``12``.
    year : int
        Delivery calendar year from ``1`` through ``9999``.
    settlement_class : SifmaSettlementClass
        Good-delivery class whose conventional estimated date is required.

    Returns
    -------
    datetime.date
        Deterministic estimated settlement date for the requested class.

    Raises
    ------
    ValueError
        If *month* is outside ``1`` through ``12``, the year/date is outside
        Python years ``1..9999``, or the month has too few business days.

    Examples
    --------
    >>> import datetime
    >>> from finstack_quant.core.dates import estimated_sifma_settlement_date, SifmaSettlementClass
    >>> estimated_sifma_settlement_date(1, 2030, SifmaSettlementClass.C)
    datetime.date(2030, 1, 22)

    """
    ...

def next_sifma_settlement(date: datetime.date | str) -> datetime.date | None:
    """
    Return the next published SIFMA settlement date strictly after a date.

    Parameters
    ----------
    date : datetime.date | str
        Calendar date from which to search the published settlement calendar.

    Returns
    -------
    datetime.date or None
        Earliest available settlement date not before ``date``, or ``None``.

    Raises
    ------
    TypeError
        If *date* is not a date-like object with integer ``year``, ``month``,
        and ``day`` attributes.
    ValueError
        If those attributes do not form a valid calendar date.

    Examples
    --------
    >>> import datetime
    >>> from finstack_quant.core.dates import next_sifma_settlement
    >>> next_sifma_settlement(datetime.date(2026, 1, 15))
    datetime.date(2026, 2, 12)

    """
    ...
