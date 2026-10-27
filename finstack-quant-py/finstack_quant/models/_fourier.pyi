"""Stub declarations mirroring ``finstack-quant-py/src/bindings/models/fourier.rs``."""

# Fourier option pricing helpers

def bs_cos_price(
    spot: float,
    strike: float,
    rate: float,
    div_yield: float,
    vol: float,
    expiry: float,
    is_call: bool,
    n_terms: int | None = None,
) -> float:
    """
    Price a European option under Black-Scholes with the COS method.

    Parameters
    ----------
    spot : float
        Spot price of the underlying.
    strike : float
        Option strike.
    rate : float
        Continuously compounded risk-free rate (decimal).
    div_yield : float
        Continuous dividend yield (decimal).
    vol : float
        Annualized volatility (decimal); must be strictly positive.
    expiry : float
        Time to expiry in years.
    is_call : bool
        ``True`` for a call, ``False`` for a put.
    n_terms : int, optional
        Number of COS terms in ``1..=65536``. Uses 128 when ``None``.

    Returns
    -------
    float
        Per-unit option price.

    Raises
    ------
    ValueError
        If ``spot``, ``strike``, ``expiry`` or ``vol`` is not finite and
        positive, ``rate`` or ``div_yield`` is non-finite, or ``n_terms`` is
        outside ``1..=65536``.
    RuntimeError
        If the inputs produce an invalid COS truncation range, a non-finite
        characteristic-function value, or a non-finite option price.

    Examples
    --------
    >>> from finstack_quant.models import bs_cos_price
    >>> round(bs_cos_price(100.0, 100.0, 0.05, 0.0, 0.2, 1.0, True), 4)
    10.4506

    Sources
    -------
    - Fang-Oosterlee (2008): see docs/REFERENCES.md#fang-oosterlee-2008
    - Black-Scholes (1973): see docs/REFERENCES.md#black-scholes-1973

    """
    ...

def vg_cos_price(
    spot: float,
    strike: float,
    rate: float,
    div_yield: float,
    sigma: float,
    theta: float,
    nu: float,
    expiry: float,
    is_call: bool,
    n_terms: int | None = None,
) -> float:
    """
    Price a European option under Variance Gamma with the COS method.

    Parameters
    ----------
    spot : float
        Spot price of the underlying.
    strike : float
        Option strike.
    rate : float
        Continuously compounded risk-free rate (decimal).
    div_yield : float
        Continuous dividend yield (decimal).
    sigma : float
        Positive finite volatility of the subordinated Brownian motion.
    theta : float
        Finite annual log-return drift of the subordinated Brownian motion;
        its sign controls skew.
    nu : float
        Positive finite variance rate of the Gamma time change, in years;
        larger values increase log-return tail thickness.
    expiry : float
        Time to expiry in years.
    is_call : bool
        ``True`` for a call, ``False`` for a put.
    n_terms : int, optional
        Number of COS terms in ``1..=65536``. Uses 128 when ``None``.

    Returns
    -------
    float
        Per-unit option price.

    Raises
    ------
    ValueError
        If ``spot``, ``strike`` or ``expiry`` is not finite and positive, a
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
    >>> round(vg_cos_price(100.0, 100.0, 0.05, 0.0, 0.2, -0.1, 0.2, 1.0, True), 4)
    10.4445

    Sources
    -------
    - Fang-Oosterlee (2008): see docs/REFERENCES.md#fang-oosterlee-2008
    - Madan-Carr-Chang (1998): see docs/REFERENCES.md#madan-carr-chang-1998

    """
    ...

def merton_jump_cos_price(
    spot: float,
    strike: float,
    rate: float,
    div_yield: float,
    sigma: float,
    mu_jump: float,
    sigma_jump: float,
    lambda_: float,
    expiry: float,
    is_call: bool,
    n_terms: int | None = None,
) -> float:
    """
    Price a European option under Merton jump-diffusion with the COS method.

    Parameters
    ----------
    spot : float
        Spot price of the underlying.
    strike : float
        Option strike.
    rate : float
        Continuously compounded risk-free rate (decimal).
    div_yield : float
        Continuous dividend yield (decimal).
    sigma : float
        Diffusion volatility (decimal).
    mu_jump : float
        Mean of the jump size distribution.
    sigma_jump : float
        Standard deviation of the jump size.
    lambda_ : float
        Jump intensity (expected jumps per year).
    expiry : float
        Time to expiry in years.
    is_call : bool
        ``True`` for a call, ``False`` for a put.
    n_terms : int, optional
        Number of COS terms in ``1..=65536``. Uses 128 when ``None``.

    Returns
    -------
    float
        Per-unit option price.

    Raises
    ------
    ValueError
        If ``spot``, ``strike`` or ``expiry`` is not finite and positive, a
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
    >>> round(merton_jump_cos_price(100.0, 100.0, 0.05, 0.0, 0.2, -0.1, 0.2, 0.5, 1.0, True), 4)
    12.1642

    Sources
    -------
    - Fang-Oosterlee (2008): see docs/REFERENCES.md#fang-oosterlee-2008
    - Merton jump-diffusion (1976): see docs/REFERENCES.md#merton-1976-jump

    """
    ...
