# finstack-quant-py/finstack_quant/reporting/format.py
"""Deterministic value formatters for rendered reports.

All helpers return display strings; non-finite / missing inputs render as the
em-interpunct placeholder ``"·"`` so tables never show ``nan``.
"""

from __future__ import annotations

import math
from typing import Any

_PLACEHOLDER = "·"


def _missing(x: Any) -> bool:
    if x is None:
        return True
    try:
        return isinstance(x, float) and math.isnan(x)
    except TypeError:
        return False


def pct(x: Any, dp: int = 1, signed: bool = False) -> str:
    """Format a value already expressed in percentage points.

    Parameters
    ----------
    x : Any
        Percentage-point value, such as ``13.2`` for ``13.2%``; missing values
        render as the report placeholder.
    dp : int
        Number of digits after the decimal point; defaults to one.
    signed : bool
        Whether non-negative values receive an explicit ``+`` sign.
    """
    if _missing(x):
        return _PLACEHOLDER
    sign = "+" if (signed and x >= 0) else ""
    return f"{sign}{x:.{dp}f}%"


def ratio(x: Any, dp: int = 2) -> str:
    """Format a unitless ratio such as a Sharpe ratio.

    Parameters
    ----------
    x : Any
        Unitless numeric ratio; missing values render as the report placeholder.
    dp : int
        Number of digits after the decimal point; defaults to two.
    """
    if _missing(x):
        return _PLACEHOLDER
    return f"{x:.{dp}f}"


def money(amount: Any, currency: str | None = None, dp: int = 2) -> str:
    """Format a monetary amount with grouping and an optional currency code.

    Parameters
    ----------
    amount : Any
        Monetary amount in the display currency; missing values use the placeholder.
    currency : str or None
        Optional ISO currency code appended after the formatted amount.
    dp : int
        Number of digits after the decimal point; defaults to two.
    """
    if _missing(amount):
        return _PLACEHOLDER
    body = f"{amount:,.{dp}f}"
    return f"{body} {currency}" if currency else body


def sign_class(x: Any) -> str:
    """Return a sign CSS class for a numeric value.

    Parameters
    ----------
    x : Any
        Numeric value whose sign selects ``"pos"``, ``"neg"``, or no class.
    """
    if _missing(x) or x == 0:
        return ""
    return "pos" if x > 0 else "neg"


def fmt_date(d: Any) -> str:
    """Format a date-like object as ``"19 Jun 2026"``.

    Parameters
    ----------
    d : Any
        Date-like object with ``strftime`` or a value convertible to string.
    """
    if _missing(d):
        return _PLACEHOLDER
    if hasattr(d, "strftime"):
        return d.strftime("%d %b %Y").lstrip("0").replace(" 0", " ")
    return str(d)
