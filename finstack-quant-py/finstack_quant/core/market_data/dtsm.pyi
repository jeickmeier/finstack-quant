"""
Dynamic term-structure model bindings: Diebold-Li and yield-curve PCA.

Function-based API for:

- Diebold-Li (2006) dynamic Nelson-Siegel factor extraction and VAR(1) forecast.
- PCA decomposition of yield-curve changes.
- PCA-based scenario generation (N-sigma shocks along principal components).

Rates in ``yields_matrix`` are continuously compounded zero yields in decimal
form (e.g. ``0.045`` for 4.5%). Tenors are in years unless you also rescale
``lambda_decay`` to a months convention.

Examples
--------
>>> import finstack_quant.core.market_data.dtsm as dtsm
>>> dtsm.__name__
'finstack_quant.core.market_data.dtsm'
"""

from __future__ import annotations

from typing import Any

__all__ = [
    "diebold_li_fit_factors",
    "diebold_li_forecast",
    "nelson_siegel_yields",
    "yield_pca_fit",
    "yield_pca_scenario",
]

def diebold_li_fit_factors(
    tenors: list[float],
    yields_matrix: list[list[float]],
    lambda_decay: float = 0.7308,
    /,
) -> dict[str, Any]:
    """
    Extract Nelson-Siegel factors from a yield panel via Diebold-Li (2006).

    Parameters
    ----------
    tenors : list[float]
        Tenor grid in years, length ``N``, strictly ascending and all positive.
    yields_matrix : list[list[float]]
        Yield panel ``yields_matrix[date_idx][tenor_idx]`` with ``T`` rows of
        ``N`` continuously compounded zero rates each.
    lambda_decay : float, optional
        Diebold-Li decay parameter for tenors **in years** (default ``0.7308``,
        the years-equivalent of Diebold-Li's canonical ``0.0609`` months value).
        The runtime binding exposes this positionally as ``lambda``; the stub
        uses ``lambda_decay`` because ``lambda`` is a Python keyword.

    Returns
    -------
    dict[str, Any]
        Dict with keys:

        - ``beta1`` : list[float] — level factor per date (length ``T``).
        - ``beta2`` : list[float] — slope factor per date (length ``T``).
        - ``beta3`` : list[float] — curvature factor per date (length ``T``).
        - ``r_squared`` : list[float] — cross-sectional R² per tenor (length ``N``).
        - ``r_squared_avg`` : float — average R² across tenors.

    Raises
    ------
    ValueError
        If tenors or the yield panel are malformed, non-finite, or fail extraction.

    Sources
    -------
    See ``docs/REFERENCES.md#diebold-li-2006``.

    Examples
    --------
    >>> from finstack_quant.core.market_data.dtsm import diebold_li_fit_factors
    >>> factors = diebold_li_fit_factors(tenors, yields_matrix)  # doctest: +SKIP
    """
    ...

def diebold_li_forecast(
    tenors: list[float],
    yields_matrix: list[list[float]],
    horizon: int,
    lambda_decay: float = 0.7308,
    /,
) -> dict[str, Any]:
    """
    VAR(1) forecast of Diebold-Li factors and yields out to ``horizon`` periods.

    Fits factors on the panel, estimates VAR(1) dynamics, and forecasts
    ``horizon`` observation steps ahead.

    Parameters
    ----------
    tenors : list[float]
        Tenor grid in years, length ``N``.
    yields_matrix : list[list[float]]
        Yield panel ``yields_matrix[date_idx][tenor_idx]`` (``T`` rows, ``N`` columns).
    horizon : int
        Forecast horizon in observation periods (must be ``>= 1``).
    lambda_decay : float, optional
        Diebold-Li decay for tenors in years (default ``0.7308``). See
        :func:`diebold_li_fit_factors` for the years-vs-months convention.

    Returns
    -------
    dict[str, Any]
        Dict with keys:

        - ``horizon`` : int — forecast horizon.
        - ``tenors`` : list[float] — tenor grid (length ``N``).
        - ``forecast_factors`` : list[float] — ``[beta1, beta2, beta3]`` forecast.
        - ``forecast_yields`` : list[float] — point forecast yields (length ``N``).
        - ``confidence_bands`` : dict with ``lower_95`` and ``upper_95``, each a
          list[float] of length ``N`` (95% Gaussian bands from the h-step VAR(1)
          forecast error covariance).

    Raises
    ------
    ValueError
        If inputs are invalid or VAR fitting/forecasting fails.

    Sources
    -------
    See ``docs/REFERENCES.md#diebold-li-2006``.

    Examples
    --------
    >>> from finstack_quant.core.market_data.dtsm import diebold_li_forecast
    >>> fc = diebold_li_forecast(tenors, yields_matrix, horizon=6)  # doctest: +SKIP
    """
    ...

def nelson_siegel_yields(
    lambda_decay: float,
    factors: tuple[float, float, float],
    tenors: list[float],
    /,
) -> list[float]:
    """
    Evaluate the static Nelson-Siegel (1987) curve for one factor triple.

    This is the Diebold-Li cross-sectional equation for a single date::

        y(tau) = beta1 + beta2 * s(tau) + beta3 * (s(tau) - exp(-lambda * tau))
        s(tau) = (1 - exp(-lambda * tau)) / (lambda * tau)

    Use it to reconstruct a fitted or forecast curve from the factors returned
    by :func:`diebold_li_fit_factors` or :func:`diebold_li_forecast`.

    Parameters
    ----------
    lambda_decay : float
        Exponential decay parameter for tenors **in years**; must be finite and
        strictly positive. ``0.7308`` is the years-equivalent of Diebold-Li's
        canonical ``0.0609`` months value and places the curvature peak at
        ≈2.45 years. The runtime binding exposes this positionally as
        ``lambda``; the stub uses ``lambda_decay`` because ``lambda`` is a
        Python keyword.
    factors : tuple[float, float, float]
        The triple ``(beta1, beta2, beta3)`` = ``(level, slope, curvature)`` in
        decimal yield units (``0.045`` for 4.5%). All three must be finite.
    tenors : list[float]
        Maturities in years, each finite and non-negative. Order is preserved in
        the output; no sorting or de-duplication is applied.

    Returns
    -------
    list[float]
        Fitted yields in decimal units, one per input tenor and in the same
        order as ``tenors``.

    Raises
    ------
    ValueError
        If ``lambda_decay`` is non-positive or non-finite, a factor is
        non-finite, or a tenor is negative or non-finite.

    Sources
    -------
    See ``docs/REFERENCES.md#diebold-li-2006``.

    Examples
    --------
    >>> from finstack_quant.core.market_data.dtsm import nelson_siegel_yields
    >>> ys = nelson_siegel_yields(0.7308, (0.06, -0.02, 0.01), [1.0, 10.0])  # doctest: +SKIP
    """
    ...

def yield_pca_fit(
    yield_changes: list[list[float]],
    n_components: int = 3,
) -> dict[str, Any]:
    """
    PCA decomposition of a yield-change panel.

    Parameters
    ----------
    yield_changes : list[list[float]]
        Panel of yield changes ``yield_changes[date_idx][tenor_idx]`` in decimal
        units (e.g. ``0.001`` for a 10 bp move).
    n_components : int, optional
        Number of principal components to retain. Default ``3``.

    Returns
    -------
    dict[str, Any]
        Dict with eigenvalues, explained-variance ratios, and component loadings
        per tenor (exact keys match the Rust serde shape returned at runtime).

    Raises
    ------
    ValueError
        If the panel is empty, ragged, non-finite, or ``n_components`` is invalid.

    Examples
    --------
    >>> from finstack_quant.core.market_data.dtsm import yield_pca_fit
    >>> pca = yield_pca_fit(changes, n_components=3)  # doctest: +SKIP
    """
    ...

def yield_pca_scenario(
    yield_changes: list[list[float]],
    component_index: int,
    sigma_shock: float,
    n_components: int = 3,
) -> list[float]:
    """
    Apply a single-component N-sigma PCA shock to the mean yield curve.

    Parameters
    ----------
    yield_changes : list[list[float]]
        Historical yield-change panel used to fit PCA (same shape as
        :func:`yield_pca_fit`).
    component_index : int
        Zero-based principal component index to shock.
    sigma_shock : float
        Shock size in standard deviations (e.g. ``2.0`` for a +2σ move).
    n_components : int, optional
        Number of components used in the PCA fit. Default ``3``.

    Returns
    -------
    list[float]
        Scenario yield shift per tenor (decimal units), length equal to the
        number of columns in ``yield_changes``.

    Raises
    ------
    ValueError
        If PCA fitting fails or ``component_index`` is out of range.

    Examples
    --------
    >>> from finstack_quant.core.market_data.dtsm import yield_pca_scenario
    >>> shift = yield_pca_scenario(changes, component_index=0, sigma_shock=-2.0)  # doctest: +SKIP
    """
    ...
