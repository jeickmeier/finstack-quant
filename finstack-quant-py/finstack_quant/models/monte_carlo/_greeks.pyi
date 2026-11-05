"""Stub declarations mirroring ``finstack-quant-py/src/bindings/models/monte_carlo/greeks.rs``."""

from __future__ import annotations
from typing import Any
from finstack_quant.models.monte_carlo._results import Estimate

def finite_diff_delta(
    spec: dict[str, Any] | str,
) -> Estimate:
    """
    Finite-difference delta for a vanilla European option under GBM.

    Central difference in spot with common random numbers: the up and down
    valuations share each path's random draws, the estimate is the mean of
    the per-path differences, and ``stderr`` is the paired
    (common-random-number) standard error of those differences.

    Parameters
    ----------
    spec : dict or str
        Serde form of the Rust ``GbmEuropeanFdSpec``, as a dict or JSON text.
        Required keys: ``spot`` (finite positive spot price; the down-bumped
        state must remain at least ``1e-12``); ``strike`` (same units as
        ``spot``); ``rate`` and ``dividend_yield`` (continuously compounded
        decimals); ``volatility`` (annualized volatility, decimal, strictly
        positive); ``time_to_expiry`` (years); ``option_type`` (``"call"`` or
        ``"put"``). Optional keys, each defaulting to the registry value when
        omitted or ``None``: ``num_paths`` (paths per evaluation, default
        ``10_000``); ``seed`` (RNG seed, default ``42``); ``num_steps``
        (time-grid steps, default ``50``); ``bump_size`` (relative Monte Carlo
        spot shock, default ``0.01`` = 1% of spot, not a closed-form local
        Greek step: the absolute bump is ``max(abs(spot) * bump_size, 1e-8)``
        and must leave a symmetric central stencil above the spot floor);
        ``currency`` (ISO currency code, default USD).

    Returns
    -------
    Estimate
        ``mean`` is the delta, ``stderr`` its paired (common-random-number)
        standard error, ``ci_lower`` / ``ci_upper`` the symmetric 95% band.

    Raises
    ------
    ValueError
        If ``spec`` has an unknown or missing key or a wrongly typed value, or ``volatility`` is not strictly positive, ``spot`` or ``bump_size`` is
        non-finite or non-positive, the symmetric down-bump falls below
        ``1e-12``, or another pricing input is invalid.

    Examples
    --------
    >>> from finstack_quant.models.monte_carlo import finite_diff_delta
    >>> est = finite_diff_delta({
    ...     "spot": 100,
    ...     "strike": 100,
    ...     "rate": 0.05,
    ...     "dividend_yield": 0.0,
    ...     "volatility": 0.2,
    ...     "time_to_expiry": 1.0,
    ...     "option_type": "call",
    ...     "num_paths": 200,
    ...     "seed": 7,
    ...     "num_steps": 10,
    ... })
    >>> 0 < est.mean < 1 and est.stderr >= 0
    True
    """
    ...

def finite_diff_gamma(
    spec: dict[str, Any] | str,
) -> Estimate:
    """
    Finite-difference gamma for a vanilla European option under GBM.

    Second central difference in spot with common random numbers; the
    estimate is the mean of the per-path second differences and ``stderr``
    is their paired (common-random-number) standard error.

    Parameters
    ----------
    spec : dict or str
        Serde form of the Rust ``GbmEuropeanFdSpec``, as a dict or JSON text.
        Required keys: ``spot`` (finite positive spot price; the down-bumped
        state must remain at least ``1e-12``); ``strike`` (same units as
        ``spot``); ``rate`` and ``dividend_yield`` (continuously compounded
        decimals); ``volatility`` (annualized volatility, decimal, strictly
        positive); ``time_to_expiry`` (years); ``option_type`` (``"call"`` or
        ``"put"``). Optional keys, each defaulting to the registry value when
        omitted or ``None``: ``num_paths`` (paths per evaluation, default
        ``10_000``); ``seed`` (RNG seed, default ``42``); ``num_steps``
        (time-grid steps, default ``50``); ``bump_size`` (relative Monte Carlo
        spot shock, default ``0.01`` = 1% of spot, not a closed-form local
        Greek step: the absolute bump is ``max(abs(spot) * bump_size, 1e-8)``
        and must leave a symmetric central stencil above the spot floor);
        ``currency`` (ISO currency code, default USD).

    Returns
    -------
    Estimate
        ``mean`` is the gamma, ``stderr`` its paired (common-random-number)
        standard error.

    Raises
    ------
    ValueError
        If ``spec`` has an unknown or missing key or a wrongly typed value, or ``volatility`` is not strictly positive, ``spot`` or ``bump_size`` is
        non-finite or non-positive, the symmetric down-bump falls below
        ``1e-12``, or another pricing input is invalid.

    Examples
    --------
    >>> from finstack_quant.models.monte_carlo import finite_diff_gamma
    >>> est = finite_diff_gamma({
    ...     "spot": 100,
    ...     "strike": 100,
    ...     "rate": 0.05,
    ...     "dividend_yield": 0.0,
    ...     "volatility": 0.2,
    ...     "time_to_expiry": 1.0,
    ...     "option_type": "call",
    ...     "num_paths": 200,
    ...     "seed": 7,
    ...     "num_steps": 10,
    ... })
    >>> est.mean > 0 and est.stderr >= 0
    True
    """
    ...
