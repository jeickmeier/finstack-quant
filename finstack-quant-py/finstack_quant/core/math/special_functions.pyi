"""Special mathematical functions: normal distribution, error function, gamma.

Examples
--------
>>> from finstack_quant.core.math import special_functions
>>> round(special_functions.norm_cdf(0.0), 10)
0.5
"""

from __future__ import annotations

__all__ = [
    "erf",
    "ln_gamma",
    "log_norm_cdf",
    "norm_cdf",
    "norm_cdf_with_params",
    "norm_pdf",
    "norm_pdf_with_params",
    "standard_normal_inv_cdf",
    "student_t_cdf",
    "student_t_inv_cdf",
]

def norm_cdf_with_params(x: float, mean: float, std_dev: float) -> float:
    """
    Normal CDF with explicit mean and standard deviation.

    Parameters
    ----------
    x : float
        Evaluation point.
    mean : float
        Distribution mean.
    std_dev : float
        Distribution standard deviation; must be strictly positive.

    Returns
    -------
    float
        ``P(X <= x)`` for ``X ~ N(mean, std_dev**2)``.

    Raises
    ------
    ValueError
        If *std_dev* is not finite and strictly positive. Non-finite *x* or
        *mean* are not rejected; they follow IEEE-754 arithmetic.

    Examples
    --------
    >>> from finstack_quant.core.math import special_functions
    >>> round(special_functions.norm_cdf_with_params(1.0, 1.0, 2.0), 10)
    0.5

    """
    ...

def norm_pdf_with_params(x: float, mean: float, std_dev: float) -> float:
    """
    Normal PDF with explicit mean and standard deviation.

    Parameters
    ----------
    x : float
        Evaluation point.
    mean : float
        Distribution mean.
    std_dev : float
        Distribution standard deviation; must be strictly positive.

    Returns
    -------
    float
        Density of ``N(mean, std_dev**2)`` at *x*.

    Raises
    ------
    ValueError
        If *std_dev* is not finite and strictly positive. Non-finite *x* or
        *mean* are not rejected; they follow IEEE-754 arithmetic.

    Examples
    --------
    >>> from finstack_quant.core.math import special_functions
    >>> round(special_functions.norm_pdf_with_params(0.0, 0.0, 1.0), 10)
    0.3989422804

    """
    ...

def norm_cdf(x: float) -> float:
    r"""Standard normal cumulative distribution function :math:`\Phi(x)`.

    Scalar only: for a vector of inputs use ``[norm_cdf(v) for v in xs]``
    or ``numpy.vectorize(norm_cdf)(xs)``; there is no array overload.

    Returns :math:`P(Z \le x)` where :math:`Z \sim N(0, 1)`.

    Parameters
    ----------
    x : float
        Input value.

    Returns
    -------
    float
        Probability in ``[0, 1]``.

    Raises
    ------
    None
        This pure function returns a floating-point result for every ``float`` input.

    Examples
    --------
    >>> from finstack_quant.core.math import special_functions
    >>> round(special_functions.norm_cdf(0.0), 10)
    0.5
    """
    ...

def log_norm_cdf(x: float) -> float:
    """Natural log of the standard normal CDF, preserving negative-tail precision.

    Parameters
    ----------
    x : float
        Threshold in standard-deviation units; infinities are accepted and NaN propagates.

    Returns
    -------
    float
        Natural log probability. Returns negative infinity at negative infinity
        and zero at positive infinity.

    Raises
    ------
    None
        This scalar function returns a floating-point result for every float input.

    Examples
    --------
    >>> from finstack_quant.core.math import special_functions
    >>> round(special_functions.log_norm_cdf(-40.0), 6)
    -804.608442
    """
    ...

def norm_pdf(x: float) -> float:
    r"""Standard normal probability density function :math:`\varphi(x)`.

    Returns :math:`\frac{1}{\sqrt{2\pi}} \exp(-x^2/2)`.

    Parameters
    ----------
    x : float
        Input value.

    Returns
    -------
    float
        Density value.

    Raises
    ------
    None
        This pure function returns a floating-point result for every ``float`` input.

    Examples
    --------
    >>> from finstack_quant.core.math import special_functions
    >>> round(special_functions.norm_pdf(0.0), 10)
    0.3989422804
    """
    ...

def standard_normal_inv_cdf(p: float) -> float:
    r"""Inverse standard normal CDF :math:`\Phi^{-1}(p)`.

    Returns *x* such that :math:`\Phi(x) = p`.

    Parameters
    ----------
    p : float
        Probability in ``(0, 1)``.

    Returns
    -------
    float
        Quantile *x* such that ``Phi(x) = p``.

    Raises
    ------
    None
        Out-of-range probabilities map to signed infinity; NaN propagates to the result.

    Examples
    --------
    >>> from finstack_quant.core.math import special_functions
    >>> round(special_functions.standard_normal_inv_cdf(0.5), 10)
    0.0
    """
    ...

def erf(x: float) -> float:
    r"""Error function :math:`\mathrm{erf}(x) = \frac{2}{\sqrt{\pi}} \int_0^x e^{-t^2} dt`.

    Parameters
    ----------
    x : float
        Input value.

    Returns
    -------
    float
        Value in ``[-1, 1]``.

    Raises
    ------
    None
        This pure function delegates to the error-function approximation without validation.

    Examples
    --------
    >>> from finstack_quant.core.math import special_functions
    >>> round(special_functions.erf(1.0), 10)
    0.8427007929
    """
    ...

def ln_gamma(x: float) -> float:
    r"""Natural logarithm of the Gamma function :math:`\ln(\Gamma(x))`.

    Returns ``float('inf')`` for :math:`x \le 0`.

    Parameters
    ----------
    x : float
        Input value.

    Returns
    -------
    float
        Natural logarithm of the gamma function for ``x > 0``. Returns positive
        infinity at ``x = 0``; negative inputs are outside the supported domain
        and can return ``nan``.

    Raises
    ------
    None
        This pure function returns a floating-point result for every ``float`` input.

    Examples
    --------
    >>> from finstack_quant.core.math import special_functions
    >>> round(special_functions.ln_gamma(1.0), 10)
    -0.0
    """
    ...

def student_t_cdf(x: float, df: float) -> float:
    r"""Student-t cumulative distribution function.

    Returns :math:`P(T \le x)` where :math:`T \sim t(\nu)`.

    Parameters
    ----------
    x : float
        Input value.
    df : float
        Degrees of freedom (:math:`\nu > 0`).

    Returns
    -------
    float
        Probability in ``[0, 1]``.

    Raises
    ------
    ValueError
        If ``df`` is non-finite or not positive.

    Examples
    --------
    >>> from finstack_quant.core.math import special_functions
    >>> round(special_functions.student_t_cdf(0.0, 10.0), 10)
    0.5
    """
    ...

def student_t_inv_cdf(p: float, df: float) -> float:
    r"""Inverse Student-t CDF (quantile function).

    Returns *x* such that :math:`P(T \le x) = p` where :math:`T \sim t(\nu)`.

    Parameters
    ----------
    p : float
        Probability in ``(0, 1)``.
    df : float
        Degrees of freedom (:math:`\nu > 0`).

    Returns
    -------
    float
        Quantile *x*.

    Raises
    ------
    ValueError
        If ``df`` is non-finite or not positive. Probabilities at or beyond
        the domain edges saturate to negative or positive infinity.

    Examples
    --------
    >>> from finstack_quant.core.math import special_functions
    >>> round(special_functions.student_t_inv_cdf(0.5, 10.0), 10)
    0.0
    """
    ...
