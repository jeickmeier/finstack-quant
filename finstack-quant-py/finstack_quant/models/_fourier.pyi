"""Stub declarations mirroring ``finstack-quant-py/src/bindings/models/fourier.rs``."""

from typing import Any

# Fourier option pricing helpers

def bs_cos_price(
    market: dict[str, Any] | str,
    vol: float,
) -> float:
    """
    Price a European option under Black-Scholes with the COS method.

    Parameters
    ----------
    market : dict or str
        Serde form of the Rust ``CosMarketParams``, as a dict or JSON text:
        ``spot`` and ``strike`` (float, the same price units); ``rate``
        (continuously compounded risk-free rate, decimal); ``div_yield``
        (continuous dividend yield, decimal); ``expiry`` (years);
        ``option_type`` (``"call"`` or ``"put"``) are required; ``n_terms``
        (int, number of COS terms in ``1..=65536``) is optional and defaults
        to 128 when omitted or ``None``.
    vol : float
        Annualized volatility (decimal); must be strictly positive.

    Returns
    -------
    float
        Per-unit option price.

    Raises
    ------
    ValueError
        If ``market`` has an unknown or missing key or a wrongly typed value, or ``spot``, ``strike``, ``expiry`` or ``vol`` is not finite and
        positive, ``rate`` or ``div_yield`` is non-finite, or ``n_terms`` is
        outside ``1..=65536``.
    RuntimeError
        If the inputs produce an invalid COS truncation range, a non-finite
        characteristic-function value, or a non-finite option price.

    Examples
    --------
    >>> from finstack_quant.models import bs_cos_price
    >>> round(
    ...     bs_cos_price(
    ...         {"spot": 100.0, "strike": 100.0, "rate": 0.05, "div_yield": 0.0, "expiry": 1.0, "option_type": "call"},
    ...         0.2,
    ...     ),
    ...     4,
    ... )
    10.4506

    Sources
    -------
    - Fang-Oosterlee (2008): see docs/REFERENCES.md#fang-oosterlee-2008
    - Black-Scholes (1973): see docs/REFERENCES.md#black-scholes-1973

    """
    ...

def vg_cos_price(
    market: dict[str, Any] | str,
    sigma: float,
    theta: float,
    nu: float,
) -> float:
    """
    Price a European option under Variance Gamma with the COS method.

    Parameters
    ----------
    market : dict or str
        Serde form of the Rust ``CosMarketParams``, as a dict or JSON text:
        ``spot`` and ``strike`` (float, the same price units); ``rate``
        (continuously compounded risk-free rate, decimal); ``div_yield``
        (continuous dividend yield, decimal); ``expiry`` (years);
        ``option_type`` (``"call"`` or ``"put"``) are required; ``n_terms``
        (int, number of COS terms in ``1..=65536``) is optional and defaults
        to 128 when omitted or ``None``.
    sigma : float
        Positive finite volatility of the subordinated Brownian motion.
    theta : float
        Finite annual log-return drift of the subordinated Brownian motion;
        its sign controls skew.
    nu : float
        Positive finite variance rate of the Gamma time change, in years;
        larger values increase log-return tail thickness.

    Returns
    -------
    float
        Per-unit option price.

    Raises
    ------
    ValueError
        If ``market`` has an unknown or missing key or a wrongly typed value, or ``spot``, ``strike`` or ``expiry`` is not finite and positive, a
        model input is non-finite, ``sigma`` or ``nu`` is not positive, the
        martingale condition ``1 - theta*nu - 0.5*sigma**2*nu > 0`` fails, or
        ``n_terms`` is outside ``1..=65536``.
    RuntimeError
        If the Variance Gamma parameters produce an invalid COS truncation
        range, a non-finite characteristic-function value, or a non-finite
        option price.

    Examples
    --------
    >>> from finstack_quant.models import vg_cos_price
    >>> round(
    ...     vg_cos_price(
    ...         {"spot": 100.0, "strike": 100.0, "rate": 0.05, "div_yield": 0.0, "expiry": 1.0, "option_type": "call"},
    ...         0.2,
    ...         -0.1,
    ...         0.2,
    ...     ),
    ...     4,
    ... )
    10.4445

    Sources
    -------
    - Fang-Oosterlee (2008): see docs/REFERENCES.md#fang-oosterlee-2008
    - Madan-Carr-Chang (1998): see docs/REFERENCES.md#madan-carr-chang-1998

    """
    ...

def merton_jump_cos_price(
    market: dict[str, Any] | str,
    sigma: float,
    mu_jump: float,
    sigma_jump: float,
    lambda_: float,
) -> float:
    """
    Price a European option under Merton jump-diffusion with the COS method.

    Parameters
    ----------
    market : dict or str
        Serde form of the Rust ``CosMarketParams``, as a dict or JSON text:
        ``spot`` and ``strike`` (float, the same price units); ``rate``
        (continuously compounded risk-free rate, decimal); ``div_yield``
        (continuous dividend yield, decimal); ``expiry`` (years);
        ``option_type`` (``"call"`` or ``"put"``) are required; ``n_terms``
        (int, number of COS terms in ``1..=65536``) is optional and defaults
        to 128 when omitted or ``None``.
    sigma : float
        Diffusion volatility (decimal).
    mu_jump : float
        Mean of the jump size distribution.
    sigma_jump : float
        Standard deviation of the jump size.
    lambda_ : float
        Jump intensity (expected jumps per year).

    Returns
    -------
    float
        Per-unit option price.

    Raises
    ------
    ValueError
        If ``market`` has an unknown or missing key or a wrongly typed value, or ``spot``, ``strike`` or ``expiry`` is not finite and positive, a
        model input is non-finite, ``sigma``, ``sigma_jump`` or ``lambda_`` is
        negative, the jump compensator is non-finite, or ``n_terms`` is
        outside ``1..=65536``.
    RuntimeError
        If the jump-diffusion parameters produce an invalid COS truncation
        range, a non-finite characteristic-function value, or a non-finite
        option price.

    Examples
    --------
    >>> from finstack_quant.models import merton_jump_cos_price
    >>> round(
    ...     merton_jump_cos_price(
    ...         {"spot": 100.0, "strike": 100.0, "rate": 0.05, "div_yield": 0.0, "expiry": 1.0, "option_type": "call"},
    ...         0.2,
    ...         -0.1,
    ...         0.2,
    ...         0.5,
    ...     ),
    ...     4,
    ... )
    12.1642

    Sources
    -------
    - Fang-Oosterlee (2008): see docs/REFERENCES.md#fang-oosterlee-2008
    - Merton jump-diffusion (1976): see docs/REFERENCES.md#merton-1976-jump

    """
    ...
