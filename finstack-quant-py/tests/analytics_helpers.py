"""Actual valuation date helpers shared by analytics fixture declarations."""

from collections.abc import Sequence
from datetime import date, timedelta


def opening_date(endpoints: Sequence[date | str]) -> date:
    """Declare a fixture opening valuation one day before its first endpoint."""
    if not endpoints:
        msg = "endpoints must be non-empty"
        raise ValueError(msg)
    first = endpoints[0]
    return (date.fromisoformat(first) if isinstance(first, str) else first) - timedelta(days=1)
