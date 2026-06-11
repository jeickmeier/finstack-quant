"""Volatility surface arbitrage detection.

Static checks for butterfly arbitrage, calendar-spread arbitrage, and
Dupire local-vol density positivity on an implied-volatility grid.
"""

from __future__ import annotations

from typing import Any, Optional

__all__ = [
    "check_butterfly_grid",
    "check_calendar_spread_grid",
    "check_local_vol_density_grid",
    "check_surface_grid",
]

def check_butterfly_grid(
    strikes: list[float],
    expiries: list[float],
    vols: list[list[float]],
    forward_prices: list[float],
    tolerance: float = 1e-6,
) -> list[dict[str, Any]]:
    """Check butterfly arbitrage on an implied-vol grid."""
    ...

def check_calendar_spread_grid(
    strikes: list[float],
    expiries: list[float],
    vols: list[list[float]],
    forward_prices: list[float],
    tolerance: float = 1e-6,
) -> list[dict[str, Any]]:
    """Check calendar-spread arbitrage (total-variance monotonicity)."""
    ...

def check_local_vol_density_grid(
    strikes: list[float],
    expiries: list[float],
    vols: list[list[float]],
    forward_prices: list[float],
) -> list[dict[str, Any]]:
    """Check Dupire local-vol density positivity."""
    ...

def check_surface_grid(
    strikes: list[float],
    expiries: list[float],
    vols: list[list[float]],
    forward: Optional[float] = None,
    forward_prices: Optional[list[float]] = None,
    tolerance: float = 1e-6,
) -> dict[str, Any]:
    """Run butterfly, calendar-spread, and local-vol density checks together."""
    ...
