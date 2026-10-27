"""Stub declarations mirroring ``finstack-quant-py/src/bindings/analytics/regression.rs``."""

from __future__ import annotations

def constrained_least_squares(
    exposures: list[float],
    n_factors: int,
    returns: list[float],
    weights: list[float],
) -> list[float]:
    """
    Fit factor returns satisfying the equality constraint ``w'Xf = w'r``.

    Binds Rust
    ``finstack_quant_analytics::regression::constrained_least_squares``: adds
    the minimal Lagrangian correction to an unconstrained OLS fit so the
    corrected factor returns exactly reproduce the weighted realized return
    ``w'r``. Typically used to fit the benchmark factor returns consumed by
    :func:`finstack_quant.portfolio.factor_brinson_attribution`, which
    requires factor returns satisfying that same completeness condition.

    Parameters
    ----------
    exposures : list[float]
        Row-major factor exposure matrix, ``n_assets x n_factors``: asset
        ``i``'s exposure to factor ``j`` is ``exposures[i * n_factors + j]``.
    n_factors : int
        Number of factor columns in ``exposures``; must be a positive integer
        representable as the platform's unsigned pointer-sized integer.
    returns : list[float]
        Realized asset returns, length ``n_assets`` (defines ``n_assets``).
    weights : list[float]
        Holding weights whose weighted return ``w'r`` must be fully
        reproduced by ``w'Xf`` (e.g. benchmark weights for a
        benchmark-return attribution); length ``n_assets``.

    Returns
    -------
    list[float]
        Constrained factor returns ``f``, one per factor, satisfying
        ``w'Xf = w'r`` to numerical precision.

    Raises
    ------
    AnalyticsError
        If ``n_factors`` is zero, ``returns`` is empty, ``exposures`` or
        ``weights`` has the wrong length, ``n_assets * n_factors`` overflows,
        any input value is non-finite, the design matrix is rank-deficient,
        coefficient rescaling or the constraint correction produces a
        non-finite value, or ``w`` and the constraint direction are
        numerically orthogonal while OLS does not already satisfy the
        constraint, so no scalar correction can restore it.
    TypeError
        If ``n_factors`` is not an integer.
    OverflowError
        If ``n_factors`` is negative or exceeds the platform's unsigned
        pointer-sized integer range.

    Sources
    -------
    See ``docs/REFERENCES.md#jeet-partani-2023``.

    Examples
    --------
    >>> from finstack_quant.analytics import constrained_least_squares
    >>> exposures = [0.0, 1.0, 1.0, 0.0, 0.0, 1.0]
    >>> returns = [0.05, 0.02, 0.01]
    >>> weights = [0.6, 0.3, 0.1]
    >>> f = constrained_least_squares(exposures, 2, returns, weights)
    >>> len(f)
    2
    """
    ...
