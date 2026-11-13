"""Stub declarations mirroring ``finstack-quant-py/src/bindings/core/dates/imm.rs``."""

from __future__ import annotations

import datetime

# IMM, CDS rolls and third-Friday lattice dates

def third_wednesday(month: int, year: int) -> datetime.date:
    """
    Return the third Wednesday of a month — the IMM date convention.

    Parameters
    ----------
    month : int
        Month number from ``1`` through ``12``.
    year : int
        Calendar year from ``1`` through ``9999``.

    Returns
    -------
    datetime.date
        The third Wednesday of the given month.

    Raises
    ------
    ValueError
        If *month* is outside ``1..12`` or the year/date is outside
        Python years ``1..9999``.

    Examples
    --------
    >>> from finstack_quant.core.dates import third_wednesday
    >>> third_wednesday(3, 2025)
    datetime.date(2025, 3, 19)

    """
    ...

def third_friday(month: int, year: int) -> datetime.date:
    """
    Return the third Friday of a month — the listed-equity-option expiry.

    Parameters
    ----------
    month : int
        Month number from ``1`` through ``12``.
    year : int
        Calendar year from ``1`` through ``9999``.

    Returns
    -------
    datetime.date
        The third Friday of the given month.

    Raises
    ------
    ValueError
        If *month* is outside ``1..12`` or the year/date is outside
        Python years ``1..9999``.

    Examples
    --------
    >>> from finstack_quant.core.dates import third_friday
    >>> third_friday(3, 2025)
    datetime.date(2025, 3, 21)

    """
    ...

def next_imm(date: datetime.date | str) -> datetime.date:
    """
    Return the next quarterly IMM date strictly after *date*.

    Parameters
    ----------
    date : datetime.date | str
        Reference date.

    Returns
    -------
    datetime.date
        Next March/June/September/December IMM date after *date*.

    Raises
    ------
    TypeError
        If *date* is not a date-like object with integer ``year``, ``month``,
        and ``day`` attributes.
    ValueError
        If those attributes do not form a valid calendar date or the selected
        roll/expiry cannot be represented in Python years ``1..9999``.

    Examples
    --------
    >>> import datetime
    >>> from finstack_quant.core.dates import next_imm
    >>> next_imm(datetime.date(2025, 5, 1))
    datetime.date(2025, 6, 18)

    """
    ...

def is_imm_date(date: datetime.date | str) -> bool:
    """
    Return whether *date* is a quarterly IMM date.

    Parameters
    ----------
    date : datetime.date | str
        Candidate date.

    Returns
    -------
    bool
        ``True`` when *date* is the third Wednesday of a quarterly month.

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
    >>> from finstack_quant.core.dates import is_imm_date
    >>> is_imm_date(datetime.date(2025, 3, 19))
    True

    """
    ...

def is_cds_date(date: datetime.date | str) -> bool:
    """
    Return whether *date* is a standard CDS roll date.

    Parameters
    ----------
    date : datetime.date | str
        Candidate date.

    Returns
    -------
    bool
        ``True`` when *date* is a standard CDS roll date.

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
    >>> from finstack_quant.core.dates import is_cds_date
    >>> is_cds_date(datetime.date(2025, 6, 20))
    True

    """
    ...

def next_cds_date(date: datetime.date | str) -> datetime.date:
    """
    Return the next standard CDS roll date strictly after *date*.

    Parameters
    ----------
    date : datetime.date | str
        Reference date.

    Returns
    -------
    datetime.date
        Next standard CDS roll date.

    Raises
    ------
    TypeError
        If *date* is not a date-like object with integer ``year``, ``month``,
        and ``day`` attributes.
    ValueError
        If those attributes do not form a valid calendar date or the selected
        roll/expiry cannot be represented in Python years ``1..9999``.

    Examples
    --------
    >>> import datetime
    >>> from finstack_quant.core.dates import next_cds_date
    >>> next_cds_date(datetime.date(2025, 5, 1))
    datetime.date(2025, 6, 20)

    """
    ...

def prev_cds_date(date: datetime.date | str) -> datetime.date:
    """
    Return the most recent standard CDS roll date strictly before *date*.

    Parameters
    ----------
    date : datetime.date | str
        Reference date.

    Returns
    -------
    datetime.date
        Most recent standard CDS roll date.

    Raises
    ------
    TypeError
        If *date* is not a date-like object with integer ``year``, ``month``,
        and ``day`` attributes.
    ValueError
        If those attributes do not form a valid calendar date or the selected
        roll/expiry cannot be represented in Python years ``1..9999``.

    Examples
    --------
    >>> import datetime
    >>> from finstack_quant.core.dates import prev_cds_date
    >>> prev_cds_date(datetime.date(2025, 5, 1))
    datetime.date(2025, 3, 20)

    """
    ...

def prev_cds_semiannual_roll(date: datetime.date | str) -> datetime.date:
    """
    Return the latest 20 March or 20 September on or before *date*.

    Inclusive: a date on that lattice is returned unchanged. This selects one
    explicitly chosen unadjusted lattice; it does not determine a premium
    accrual, protection or settlement date, and composing it with a tenor is
    a product regime's rule.

    Parameters
    ----------
    date : datetime.date | str
        Reference date.

    Returns
    -------
    datetime.date
        Most recent March or September CDS roll date.

    Raises
    ------
    TypeError
        If *date* is not a date-like object with integer ``year``, ``month``,
        and ``day`` attributes.
    ValueError
        If those attributes do not form a valid calendar date or the selected
        roll/expiry cannot be represented in Python years ``1..9999``.

    Examples
    --------
    >>> import datetime
    >>> from finstack_quant.core.dates import prev_cds_semiannual_roll
    >>> prev_cds_semiannual_roll(datetime.date(2025, 5, 1))
    datetime.date(2025, 3, 20)

    """
    ...

def next_semiannual_cds_maturity(date: datetime.date | str) -> datetime.date:
    """
    Return the earliest 20 June or 20 December on or after *date*.

    Inclusive: a date on that lattice is returned unchanged. This selects one
    explicitly chosen unadjusted lattice, not every standard CDS maturity
    (3M and 9M quotes mature on 20 March or 20 September); a product regime
    composes trade dates and tenors into maturities.

    Parameters
    ----------
    date : datetime.date | str
        Unadjusted date from which the next 20 June or 20 December is selected.

    Returns
    -------
    datetime.date
        The selected 20 June or 20 December.

    Raises
    ------
    TypeError
        If *date* is not a date-like object with integer ``year``, ``month``,
        and ``day`` attributes.
    ValueError
        If those attributes do not form a valid calendar date or the selected
        roll/expiry cannot be represented in Python years ``1..9999``.

    Examples
    --------
    >>> import datetime
    >>> from finstack_quant.core.dates import next_semiannual_cds_maturity
    >>> next_semiannual_cds_maturity(datetime.date(2025, 5, 1))
    datetime.date(2025, 6, 20)

    """
    ...

def next_third_friday(date: datetime.date | str) -> datetime.date:
    """
    Return the next unadjusted monthly third Friday after *date*.

    Exchange holidays are not applied. This primitive does not determine a
    listed option's expiration or last trading date.

    Parameters
    ----------
    date : datetime.date | str
        Reference date.

    Returns
    -------
    datetime.date
        Next third Friday strictly after *date*, even when it is a holiday.

    Raises
    ------
    TypeError
        If *date* is not a date-like object with integer ``year``, ``month``,
        and ``day`` attributes.
    ValueError
        If those attributes do not form a valid calendar date or the selected
        roll/expiry cannot be represented in Python years ``1..9999``.

    Examples
    --------
    >>> import datetime
    >>> from finstack_quant.core.dates import next_third_friday
    >>> next_third_friday(datetime.date(2025, 5, 1))
    datetime.date(2025, 5, 16)

    """
    ...
