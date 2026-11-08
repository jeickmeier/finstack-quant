"""Actual valuation grids shared by analytics fixture declarations."""

from collections.abc import Sequence
from datetime import date, timedelta


def valuation_grid(endpoints: Sequence[date | str]) -> list[date | str]:
    """Declare a fixture opening valuation one day before its first endpoint."""
    if not endpoints:
        return []
    first = endpoints[0]
    opening = (date.fromisoformat(first) if isinstance(first, str) else first) - timedelta(days=1)
    return [opening, *endpoints]
