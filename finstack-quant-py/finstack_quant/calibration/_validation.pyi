"""Stub declarations mirroring ``finstack-quant-py/src/bindings/calibration/validation.rs``."""

from finstack_quant.core.market_data import VolSurface
from finstack_quant.calibration._config import ValidationConfig

def validate_surface(surface: VolSurface, config: ValidationConfig, forwards: list[float]) -> None:
    """Run the forward-aware calendar and call-convexity checks plus the volatility bounds.

    Parameters
    ----------
    surface : VolSurface
        Implied-volatility surface (expiries in years, absolute strikes).
    config : ValidationConfig
        Thresholds; ``check_arbitrage=False`` skips arbitrage checks but
        retains forward-input validation and volatility bounds.
    forwards : list[float]
        Forward price for each surface expiry, in expiry order (same units as
        the strikes); one finite positive entry per expiry.

    Raises
    ------
    ValueError
        If ``forwards`` does not have one finite positive entry per expiry, or
        any check fails.

    Examples:
    --------
    >>> from finstack_quant.calibration import ValidationConfig, validate_surface
    >>> from finstack_quant.core.market_data import VolSurface
    >>> surface = VolSurface("EQ-VOL", [1.0, 2.0], [90.0, 100.0, 110.0], [[0.205, 0.20, 0.205], [0.215, 0.21, 0.215]])
    >>> validate_surface(surface, ValidationConfig(), [100.0, 100.0])

    """

def validate_calendar_spread(surface: VolSurface, config: ValidationConfig, forwards: list[float]) -> None:
    """Check that total variance does not decrease with expiry at fixed forward moneyness.

    Parameters
    ----------
    surface : VolSurface
        Implied-volatility surface (expiries in years, absolute strikes).
    config : ValidationConfig
        Thresholds; ``check_arbitrage=False`` skips the check and
        forward-input validation remains active.
    forwards : list[float]
        Forward price for each surface expiry, in expiry order (same units as
        the strikes); one finite positive entry per expiry.

    Raises
    ------
    ValueError
        If ``forwards`` does not match the expiries, or total variance
        decreases between two expiries at fixed forward moneyness.

    Examples:
    --------
    >>> from finstack_quant.calibration import ValidationConfig, validate_calendar_spread
    >>> from finstack_quant.core.market_data import VolSurface
    >>> surface = VolSurface("EQ-VOL", [1.0, 2.0], [90.0, 100.0, 110.0], [[0.205, 0.20, 0.205], [0.215, 0.21, 0.215]])
    >>> validate_calendar_spread(surface, ValidationConfig(), [100.0, 100.0])

    """

def validate_butterfly_spread(surface: VolSurface, config: ValidationConfig, forwards: list[float]) -> None:
    """Check that undiscounted Black call prices are convex in strike at each expiry.

    Every adjacent vertical call spread must also cost between zero and its
    strike width; the price tolerance is ``max(config.tolerance, 1e-12)`` times the forward.

    Parameters
    ----------
    surface : VolSurface
        Implied-volatility surface (expiries in years, absolute strikes).
    config : ValidationConfig
        Thresholds; ``check_arbitrage=False`` skips the check and
        forward-input validation remains active.
    forwards : list[float]
        Forward price for each surface expiry, in expiry order (same units as
        the strikes); one finite positive entry per expiry.

    Raises
    ------
    ValueError
        If ``forwards`` does not match the expiries, or call prices are not
        convex in strike.

    Examples:
    --------
    >>> from finstack_quant.calibration import ValidationConfig, validate_butterfly_spread
    >>> from finstack_quant.core.market_data import VolSurface
    >>> surface = VolSurface("EQ-VOL", [1.0, 2.0], [90.0, 100.0, 110.0], [[0.205, 0.20, 0.205], [0.215, 0.21, 0.215]])
    >>> validate_butterfly_spread(surface, ValidationConfig(), [100.0, 100.0])

    """

def validate_vol_bounds(surface: VolSurface, config: ValidationConfig) -> None:
    """Check that every surface volatility is positive and at most ``config.max_volatility``.

    Parameters
    ----------
    surface : VolSurface
        Implied-volatility surface (expiries in years, absolute strikes).
    config : ValidationConfig
        Thresholds; ``max_volatility`` is the upper bound (annualized decimal).

    Raises
    ------
    ValueError
        If a volatility is not positive or exceeds ``max_volatility``.

    Examples:
    --------
    >>> from finstack_quant.calibration import ValidationConfig, validate_vol_bounds
    >>> from finstack_quant.core.market_data import VolSurface
    >>> surface = VolSurface("EQ-VOL", [1.0, 2.0], [90.0, 100.0, 110.0], [[0.205, 0.20, 0.205], [0.215, 0.21, 0.215]])
    >>> validate_vol_bounds(surface, ValidationConfig())

    """
