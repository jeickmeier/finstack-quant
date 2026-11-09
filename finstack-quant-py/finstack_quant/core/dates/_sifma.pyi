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

def sifma_settlement_date(month: int, year: int, settlement_class: SifmaSettlementClass) -> datetime.date:
    """
    Return the published SIFMA settlement date for a delivery month and class.

    Parameters
    ----------
    month : int
        Delivery month number from ``1`` through ``12``.
    year : int
        Four-digit delivery calendar year.
    settlement_class : SifmaSettlementClass
        Good-delivery class whose published date applies. Required: there is
        no default class.

    Returns
    -------
    datetime.date
        The maintained published Settlement date for the requested month,
        year and class, unchanged.

    Raises
    ------
    KeyError
        If the maintained SIFMA data has no published date for that month,
        year and class.
    ValueError
        If *month* is outside ``1`` through ``12``, or the stored date is
        invalid.

    Examples
    --------
    >>> import datetime
    >>> from finstack_quant.core.dates import sifma_settlement_date, SifmaSettlementClass
    >>> sifma_settlement_date(1, 2026, SifmaSettlementClass.A)
    datetime.date(2026, 1, 14)
    >>> sifma_settlement_date(1, 2026, SifmaSettlementClass.B)
    datetime.date(2026, 1, 20)

    """
    ...

def next_sifma_settlement(date: datetime.date | str, settlement_class: SifmaSettlementClass) -> datetime.date:
    """
    Return the next published SIFMA settlement date for a class strictly after a date.

    Parameters
    ----------
    date : datetime.date | str
        Reference date. A settlement on this date is not returned.
    settlement_class : SifmaSettlementClass
        Good-delivery class whose published dates are searched. Required.

    Returns
    -------
    datetime.date
        The current month's published date when it is later than ``date``,
        otherwise the following month's published date.

    Raises
    ------
    KeyError
        If a required current- or following-month date is not in the
        maintained data; gaps are never skipped.
    TypeError
        If *date* is not a date-like object with integer ``year``, ``month``,
        and ``day`` attributes.
    ValueError
        If those attributes do not form a valid calendar date, or ``date``
        has no later month.

    Examples
    --------
    >>> import datetime
    >>> from finstack_quant.core.dates import next_sifma_settlement, SifmaSettlementClass
    >>> next_sifma_settlement(datetime.date(2026, 1, 15), SifmaSettlementClass.A)
    datetime.date(2026, 2, 12)

    """
    ...
