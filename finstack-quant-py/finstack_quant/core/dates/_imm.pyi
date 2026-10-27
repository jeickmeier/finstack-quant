"""Stub declarations mirroring ``finstack-quant-py/src/bindings/core/dates/imm.rs``."""

from __future__ import annotations

import datetime

# IMM, CDS rolls, and listed-option expiries

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
    Return the most recent semi-annual CDS roll on or before *date*.

    Semi-annual rolls are the March and September dates only.

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
    Return the next semi-annual CDS maturity date on or after *date*.

    Parameters
    ----------
    date : datetime.date | str
        Reference date.

    Returns
    -------
    datetime.date
        Next semi-annual CDS maturity date.

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

def imm_option_expiry(month: int, year: int) -> datetime.date:
    """
    Return the expiry of the option on the IMM future for a month.

    Parameters
    ----------
    month : int
        Month number from ``1`` through ``12``.
    year : int
        Calendar year from ``1`` through ``9999``.

    Returns
    -------
    datetime.date
        Option expiry date for that IMM contract month.

    Raises
    ------
    ValueError
        If *month* is outside ``1..12`` or the year/date is outside
        Python years ``1..9999``.

    Examples
    --------
    >>> from finstack_quant.core.dates import imm_option_expiry
    >>> imm_option_expiry(3, 2025)
    datetime.date(2025, 3, 14)

    """
    ...

def next_imm_option_expiry(date: datetime.date | str) -> datetime.date:
    """
    Return the next quarterly IMM option expiry strictly after *date*.

    Parameters
    ----------
    date : datetime.date | str
        Reference date.

    Returns
    -------
    datetime.date
        Next quarterly IMM option expiry.

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
    >>> from finstack_quant.core.dates import next_imm_option_expiry
    >>> next_imm_option_expiry(datetime.date(2025, 5, 1))
    datetime.date(2025, 6, 13)

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
