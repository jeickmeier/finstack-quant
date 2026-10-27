"""Stub declarations mirroring ``finstack-quant-py/src/bindings/models/monte_carlo/greeks.rs``."""

from __future__ import annotations
from finstack_quant.models.monte_carlo._results import Estimate

def finite_diff_delta(
    spot: float,
    strike: float,
    rate: float,
    div_yield: float,
    vol: float,
    expiry: float,
    is_call: bool,
    num_paths: int | None = None,
    seed: int | None = None,
    num_steps: int | None = None,
    bump_size: float | None = None,
    currency: str | None = None,
) -> Estimate:
    """
    Finite-difference delta for a vanilla European option under GBM.

    Central difference in spot with common random numbers: the up and down
    valuations share each path's random draws, the estimate is the mean of
    the per-path differences, and ``stderr`` is the paired
    (common-random-number) standard error of those differences.

    Parameters
    ----------
    spot : float
        Finite positive spot price. The down-bumped state must remain at least
        ``1e-12``.
    strike : float
        Strike price.
    rate : float
        Risk-free rate (continuously compounded decimal).
    div_yield : float
        Dividend yield (continuously compounded decimal).
    vol : float
        Volatility (decimal); must be strictly positive.
    expiry : float
        Maturity in years.
    is_call : bool
        ``True`` for a call, ``False`` for a put.
    num_paths : int, optional
        Paths per evaluation (default ``10_000``).
    seed : int, optional
        RNG seed (default ``42``).
    num_steps : int, optional
        Time-grid steps (default ``50``).
    bump_size : float, optional
        Relative Monte Carlo spot shock (default ``0.01`` = 1% of spot), not
        a closed-form local Greek step. The absolute bump is
        ``max(abs(spot) * bump_size, 1e-8)`` and must leave a symmetric
        central stencil above the spot floor.
    currency : str, optional
        ISO currency code. Defaults to USD.

    Returns
    -------
    Estimate
        ``mean`` is the delta, ``stderr`` its paired (common-random-number)
        standard error, ``ci_lower`` / ``ci_upper`` the symmetric 95% band.

    Raises
    ------
    ValueError
        If ``vol`` is not strictly positive, ``spot`` or ``bump_size`` is
        non-finite or non-positive, the symmetric down-bump falls below
        ``1e-12``, or another pricing input is invalid.

    Examples
    --------
    >>> from finstack_quant.models.monte_carlo import finite_diff_delta
    >>> est = finite_diff_delta(100, 100, 0.05, 0.0, 0.2, 1.0, True, num_paths=200, seed=7, num_steps=10)
    >>> 0 < est.mean < 1 and est.stderr >= 0
    True
    """
    ...

def finite_diff_gamma(
    spot: float,
    strike: float,
    rate: float,
    div_yield: float,
    vol: float,
    expiry: float,
    is_call: bool,
    num_paths: int | None = None,
    seed: int | None = None,
    num_steps: int | None = None,
    bump_size: float | None = None,
    currency: str | None = None,
) -> Estimate:
    """
    Finite-difference gamma for a vanilla European option under GBM.

    Second central difference in spot with common random numbers; the
    estimate is the mean of the per-path second differences and ``stderr``
    is their paired (common-random-number) standard error.

    Parameters
    ----------
    spot : float
        Finite positive spot price. The down-bumped state must remain at least
        ``1e-12``.
    strike : float
        Strike price.
    rate : float
        Risk-free rate (continuously compounded decimal).
    div_yield : float
        Dividend yield (continuously compounded decimal).
    vol : float
        Volatility (decimal); must be strictly positive.
    expiry : float
        Maturity in years.
    is_call : bool
        ``True`` for a call, ``False`` for a put.
    num_paths : int, optional
        Paths per evaluation (default ``10_000``).
    seed : int, optional
        RNG seed (default ``42``).
    num_steps : int, optional
        Time-grid steps (default ``50``).
    bump_size : float, optional
        Relative Monte Carlo spot shock (default ``0.01`` = 1% of spot), not
        a closed-form local Greek step. The absolute bump is
        ``max(abs(spot) * bump_size, 1e-8)`` and must leave a symmetric
        central stencil above the spot floor.
    currency : str, optional
        ISO currency code. Defaults to USD.

    Returns
    -------
    Estimate
        ``mean`` is the gamma, ``stderr`` its paired (common-random-number)
        standard error.

    Raises
    ------
    ValueError
        If ``vol`` is not strictly positive, ``spot`` or ``bump_size`` is
        non-finite or non-positive, the symmetric down-bump falls below
        ``1e-12``, or another pricing input is invalid.

    Examples
    --------
    >>> from finstack_quant.models.monte_carlo import finite_diff_gamma
    >>> est = finite_diff_gamma(100, 100, 0.05, 0.0, 0.2, 1.0, True, num_paths=200, seed=7, num_steps=10)
    >>> est.mean > 0 and est.stderr >= 0
    True
    """
    ...
