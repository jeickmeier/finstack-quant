"""Volatility surface arbitrage detection.

Model-free static checks for butterfly arbitrage (Durrleman density),
calendar-spread arbitrage (total-variance monotonicity), and Dupire local-vol
density positivity on an implied-volatility grid. Inputs are flat arrays so
callers can pass numpy-friendly grids without first constructing a
``VolSurface`` wrapper.
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
    """Check butterfly arbitrage via Durrleman's ``g(k)`` density condition.

    Parameters
    ----------
    strikes : list[float]
        Monotonically increasing strike grid.
    expiries : list[float]
        Monotonically increasing expiry grid in years.
    vols : list[list[float]]
        Implied volatilities shaped ``[n_expiries][n_strikes]`` (decimal, e.g.
        ``0.20`` for 20%).
    forward_prices : list[float]
        Forward prices per expiry, or a single value broadcast across expiries.
    tolerance : float, optional
        Tolerance in total-variance units. Default ``1e-6``.

    Returns
    -------
    list[dict[str, Any]]
        One dict per violation with keys ``type``, ``severity``, ``strike``,
        ``expiry``, ``adjacent_expiry``, ``magnitude``, ``value``, ``message``,
        and ``description``.

    Raises
    ------
    ValueError
        If grid dimensions are inconsistent or inputs are non-finite.

    Sources
    -------
    See ``docs/REFERENCES.md#dupire-1994`` for local-volatility density context.

    Examples
    --------
    >>> from finstack_quant.core.market_data.arbitrage import check_butterfly_grid
    >>> violations = check_butterfly_grid(strikes, expiries, vols, [100.0])  # doctest: +SKIP
    """
    ...

def check_calendar_spread_grid(
    strikes: list[float],
    expiries: list[float],
    vols: list[list[float]],
    forward_prices: list[float],
    tolerance: float = 1e-6,
) -> list[dict[str, Any]]:
    """Check calendar-spread arbitrage (total-variance monotonicity in log-moneyness).

    Parameters
    ----------
    strikes : list[float]
        Monotonically increasing strike grid.
    expiries : list[float]
        Monotonically increasing expiry grid in years.
    vols : list[list[float]]
        Implied vols shaped ``[n_expiries][n_strikes]``.
    forward_prices : list[float]
        Forward prices per expiry or one broadcast value.
    tolerance : float, optional
        Tolerance in total-variance units. Default ``1e-6``.

    Returns
    -------
    list[dict[str, Any]]
        Violation dicts with the same schema as :func:`check_butterfly_grid`.

    Raises
    ------
    ValueError
        If grid dimensions are inconsistent or inputs are non-finite.

    Examples
    --------
    >>> from finstack_quant.core.market_data.arbitrage import check_calendar_spread_grid
    >>> violations = check_calendar_spread_grid(strikes, expiries, vols, [100.0])  # doctest: +SKIP
    """
    ...

def check_local_vol_density_grid(
    strikes: list[float],
    expiries: list[float],
    vols: list[list[float]],
    forward_prices: list[float],
) -> list[dict[str, Any]]:
    """Check Dupire local-volatility density positivity on the implied-vol grid.

    Parameters
    ----------
    strikes : list[float]
        Monotonically increasing strike grid.
    expiries : list[float]
        Monotonically increasing expiry grid in years.
    vols : list[list[float]]
        Implied vols shaped ``[n_expiries][n_strikes]``.
    forward_prices : list[float]
        Forward prices per expiry or one broadcast value.

    Returns
    -------
    list[dict[str, Any]]
        Violation dicts with the same schema as :func:`check_butterfly_grid`.

    Raises
    ------
    ValueError
        If grid dimensions are inconsistent or inputs are non-finite.

    Sources
    -------
    See ``docs/REFERENCES.md#dupire-1994``.

    Examples
    --------
    >>> from finstack_quant.core.market_data.arbitrage import check_local_vol_density_grid
    >>> violations = check_local_vol_density_grid(strikes, expiries, vols, [100.0])  # doctest: +SKIP
    """
    ...

def check_surface_grid(
    strikes: list[float],
    expiries: list[float],
    vols: list[list[float]],
    forward: Optional[float] = None,
    forward_prices: Optional[list[float]] = None,
    tolerance: float = 1e-6,
) -> dict[str, Any]:
    """Run butterfly, calendar-spread, and local-vol density checks together.

    Parameters
    ----------
    strikes : list[float]
        Monotonically increasing strike grid.
    expiries : list[float]
        Monotonically increasing expiry grid in years.
    vols : list[list[float]]
        Implied vols shaped ``[n_expiries][n_strikes]``.
    forward : float, optional
        Single forward price broadcast to all expiries. Mutually exclusive with
        ``forward_prices`` when both would supply different values.
    forward_prices : list[float], optional
        Per-expiry forward prices. When omitted, ``forward`` must be supplied.
    tolerance : float, optional
        Tolerance in total-variance units. Default ``1e-6``.

    Returns
    -------
    dict[str, Any]
        Summary dict with keys ``butterfly``, ``calendar_spread``, and
        ``local_vol_density``, each a list of violation dicts, plus
        ``is_arbitrage_free`` (``True`` when all three lists are empty).

    Raises
    ------
    ValueError
        If neither ``forward`` nor ``forward_prices`` is supplied, or grid
        inputs are invalid.

    Examples
    --------
    >>> from finstack_quant.core.market_data.arbitrage import check_surface_grid
    >>> report = check_surface_grid(strikes, expiries, vols, forward=100.0)  # doctest: +SKIP
    >>> report["is_arbitrage_free"]  # doctest: +SKIP
    True
    """
    ...
