"""European option COS pricing on canonical option inputs."""

from typing import Any

from finstack_quant.models._closed_form import OptionContractParams, OptionMarketParams

def bs_cos_price(market: OptionMarketParams | dict[str, Any], n_terms: int | None = None) -> float:
    """Price a European call or put under a constant lognormal volatility.

    Parameters
    ----------
    market : OptionMarketParams or dict
        Spot and strike in equal units, continuous annual discount and carry
        rates as decimals, positive expiry in years and strictly positive
        decimal annual volatility. A dictionary uses the same fields.
    n_terms : int or None
        Expansion terms from 1 through 65,536. None uses 128.

    Returns
    -------
    float
        Present value per underlying unit.

    Raises
    ------
    ValueError
        For invalid values, malformed inputs, or an invalid term count.
    RuntimeError
        If the model cannot represent its cumulants or finite option price.
    OverflowError
        If n_terms cannot be represented as a native nonnegative integer.

    Examples
    --------
    >>> from finstack_quant.models import OptionMarketParams, bs_cos_price
    >>> inputs = OptionMarketParams(
    ...     spot=100.0,
    ...     strike=100.0,
    ...     rate=0.05,
    ...     dividend_yield=0.0,
    ...     volatility=0.2,
    ...     time_to_expiry=1.0,
    ...     option_type="call",
    ... )
    >>> round(bs_cos_price(inputs), 2)
    10.45
    """
    ...

def vg_cos_price(
    contract: OptionContractParams | dict[str, Any],
    sigma: float,
    theta: float,
    nu: float,
    n_terms: int | None = None,
) -> float:
    """Price a European call or put under the Variance Gamma process.

    Parameters
    ----------
    contract : OptionContractParams or dict
        Matching spot and strike units, continuous annual decimal discount and
        carry rates, positive expiry in years and call or put direction.
    sigma : float
        Positive annual decimal volatility of the subordinated Brownian motion.
    theta : float
        Annual drift of the subordinated Brownian motion with a finite forward moment.
    nu : float
        Positive variance rate of the Gamma subordinator.
    n_terms : int or None
        Expansion terms from 1 through 65,536. None uses 128.

    Returns
    -------
    float
        Present value per underlying unit.

    Raises
    ------
    ValueError
        If the contract, process domain or term count is invalid.
    RuntimeError
        If the model cannot represent its cumulants or finite option price.
    OverflowError
        If n_terms cannot be represented as a native nonnegative integer.

    Examples
    --------
    >>> from finstack_quant.models import OptionContractParams, vg_cos_price
    >>> contract = OptionContractParams(
    ...     spot=100.0,
    ...     strike=100.0,
    ...     rate=0.05,
    ...     dividend_yield=0.0,
    ...     time_to_expiry=1.0,
    ...     option_type="call",
    ... )
    >>> vg_cos_price(contract, 0.2, -0.14, 0.2) > 0
    True
    """
    ...

def merton_jump_cos_price(
    contract: OptionContractParams | dict[str, Any],
    sigma: float,
    mu_jump: float,
    sigma_jump: float,
    lambda_: float,
    n_terms: int | None = None,
) -> float:
    """Price a European call or put under lognormal jump diffusion.

    Parameters
    ----------
    contract : OptionContractParams or dict
        Matching spot and strike units, continuous annual decimal discount and
        carry rates, positive expiry in years and call or put direction.
    sigma : float
        Finite nonnegative annual diffusion volatility as a decimal.
    mu_jump : float
        Mean log jump size with a finite risk-neutral compensator.
    sigma_jump : float
        Finite nonnegative standard deviation of the log jump size.
    lambda_ : float
        Expected jump count per year, finite and nonnegative.
    n_terms : int or None
        Expansion terms from 1 through 65,536. None uses 128.

    Returns
    -------
    float
        Present value per underlying unit.

    Raises
    ------
    ValueError
        If the contract, process domain or term count is invalid.
    RuntimeError
        If the model cannot represent its cumulants or finite option price.
    OverflowError
        If n_terms cannot be represented as a native nonnegative integer.

    Examples
    --------
    >>> from finstack_quant.models import OptionContractParams, merton_jump_cos_price
    >>> contract = OptionContractParams(
    ...     spot=100.0,
    ...     strike=100.0,
    ...     rate=0.05,
    ...     dividend_yield=0.0,
    ...     time_to_expiry=1.0,
    ...     option_type="call",
    ... )
    >>> merton_jump_cos_price(contract, 0.2, -0.05, 0.15, 0.7) > 0
    True
    """
    ...
