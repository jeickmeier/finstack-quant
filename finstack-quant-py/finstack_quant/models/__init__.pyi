"""Reusable analytical, numerical, volatility, Fourier, and stochastic models.

The root namespace contains model-family functions and classes. Correlation,
credit, Monte Carlo, and rates APIs are grouped under the matching submodules.

Examples
--------
>>> from finstack_quant.models import bs_price
>>> round(bs_price(100.0, 100.0, 0.05, 0.0, 0.2, 1.0, True), 4)
10.4506
"""

from typing import Any

import pandas as pd

from finstack_quant.models import correlation as correlation
from finstack_quant.models import credit as credit
from finstack_quant.models import factor as factor
from finstack_quant.models import liquidity as liquidity
from finstack_quant.models import monte_carlo as monte_carlo
from finstack_quant.models import rates as rates
from finstack_quant.models import volatility as volatility

__all__ = [
    "BsGreeks",
    "asian_option_price",
    "bachelier_greeks",
    "bachelier_price",
    "barrier_call",
    "barrier_put",
    "black76_greeks",
    "black76_implied_vol",
    "black76_price",
    "black_shifted_price",
    "black_shifted_vega",
    "bs_cos_price",
    "bs_greeks",
    "bs_implied_vol",
    "bs_price",
    "correlation",
    "credit",
    "factor",
    "heston_price",
    "liquidity",
    "lookback_option_price",
    "merton_jump_cos_price",
    "monte_carlo",
    "quanto_option_price",
    "rates",
    "vanilla_expiry_payoff",
    "vg_cos_price",
    "volatility",
]

class BsGreeks:
    """
    Black-Scholes / Garman-Kohlhagen Greeks for one European option (per unit).

    ``vega``, ``rho_r`` and ``rho_q`` are per 1% (0.01) move; ``theta`` is per
    day under the ``theta_days`` basis passed to :func:`bs_greeks`; ``delta``
    and ``gamma`` are per unit of spot. Immutable, compares by value, picklable.

    Examples
    --------
    >>> from finstack_quant.models import bs_greeks
    >>> g = bs_greeks(100.0, 100.0, 0.05, 0.0, 0.2, 1.0, True)
    >>> round(g.delta, 4)
    0.6368
    >>> g.to_series().index.tolist()
    ['delta', 'gamma', 'vega', 'theta', 'rho_r', 'rho_q']
    """

    @property
    def delta(self) -> float:
        """
        Spot delta per unit of underlying.

        Returns
        -------
        float
            Sensitivity of the option value to a one-unit change in spot, in the same currency units as the option price (dimensionless for a unit-notional option).

        Notes
        -----
        This accessor does not raise.
        """
        ...

    @property
    def gamma(self) -> float:
        """
        Gamma per unit of underlying.

        Returns
        -------
        float
            Second derivative of the option value with respect to spot: the change in *delta* per one-unit move in the underlying.

        Notes
        -----
        This accessor does not raise.
        """
        ...

    @property
    def vega(self) -> float:
        """
        Vega per 1% (0.01) move in volatility.

        Returns
        -------
        float
            Change in option value for a 0.01 (one percentage point) increase in the lognormal Black-Scholes volatility.

        Notes
        -----
        This accessor does not raise.
        """
        ...

    @property
    def theta(self) -> float:
        """
        Theta per day under the ``theta_days`` basis (negative = decay).

        Returns
        -------
        float
            Change in option value per calendar or business day, using the ``theta_days`` year basis supplied to :func:`bs_greeks`; negative values indicate time decay.

        Notes
        -----
        This accessor does not raise.
        """
        ...

    @property
    def rho_r(self) -> float:
        """
        Rho to the domestic / risk-free rate per 1% (0.01) move.

        Returns
        -------
        float
            Change in option value for a 0.01 (100 bp) parallel increase in the continuously compounded domestic risk-free rate.

        Notes
        -----
        This accessor does not raise.
        """
        ...

    @property
    def rho_q(self) -> float:
        """
        Rho to the dividend yield / foreign rate per 1% (0.01) move.

        Returns
        -------
        float
            Change in option value for a 0.01 (100 bp) increase in the continuous dividend yield (or foreign rate for an FX option).

        Notes
        -----
        This accessor does not raise.
        """
        ...

    def is_valid(self) -> bool:
        """
        Return ``True`` when every Greek is finite and ``gamma`` / ``vega`` are non-negative.

        Returns
        -------
        bool
            Validity flag; delta is deliberately not bounded. Does not raise.
        """
        ...

    def to_series(self) -> pd.Series:
        """
        Return the Greeks as a float ``pandas.Series`` named ``bs_greeks``.

        Returns
        -------
        pandas.Series
            Index ``delta, gamma, vega, theta, rho_r, rho_q``. Does not raise.
        """
        ...

    def to_dataframe(self) -> pd.DataFrame:
        """
        Return the Greeks as a single-row ``pandas.DataFrame``.

        Returns
        -------
        pandas.DataFrame
            Columns ``delta, gamma, vega, theta, rho_r, rho_q``. Does not raise.
        """
        ...

    def to_json(self) -> str:
        """
        Serialize to compact JSON with the canonical field names.

        Returns
        -------
        str
            JSON object with the six Greek fields. Does not raise.
        """
        ...

    @staticmethod
    def from_json(json: str) -> BsGreeks:
        """
        Deserialize from the JSON produced by :meth:`to_json`.

        Parameters
        ----------
        json : str
            JSON object with ``delta, gamma, vega, theta, rho_r, rho_q``.

        Returns
        -------
        BsGreeks
            Reconstructed Greeks.

        Raises
        ------
        ValueError
            If a field is missing or unknown.

        Examples
        --------
        >>> from finstack_quant.models import BsGreeks, bs_greeks
        >>> g = bs_greeks(100.0, 100.0, 0.05, 0.0, 0.2, 1.0, True)
        >>> BsGreeks.from_json(g.to_json()) == g
        True
        """
        ...

    def __eq__(self, other: object) -> bool: ...
    def __reduce__(self) -> tuple[Any, tuple[str]]: ...

def bs_price(
    spot: float,
    strike: float,
    rate: float,
    div_yield: float,
    vol: float,
    expiry: float,
    is_call: bool,
) -> float:
    """
    Per-unit Black-Scholes / Garman-Kohlhagen price of a European option.

    All rates are continuously compounded decimals; ``vol`` is annualized
    vol; ``expiry`` is years to expiry. Pass ``is_call=False`` for puts.

    Parameters
    ----------
    spot : float
        Spot price of the underlying.
    strike : float
        Option strike.
    rate : float
        Continuously compounded risk-free rate (decimal).
    div_yield : float
        Continuous dividend/borrow yield (decimal).
    vol : float
        Annualized volatility (decimal); must be non-negative.
    expiry : float
        Time to expiry in years.
    is_call : bool
        ``True`` for a call, ``False`` for a put.

    Returns
    -------
    float
        Per-unit option price.

    Raises
    ------
    ValueError
        If spot or strike is non-positive, volatility or expiry is negative,
        any numerical input is non-finite, or discounted legs or price overflow.

    Examples
    --------
    >>> from finstack_quant.models import bs_price
    >>> round(bs_price(100.0, 100.0, 0.05, 0.0, 0.2, 1.0, True), 4)
    10.4506

    Sources
    -------
    - Black-Scholes (1973): see docs/REFERENCES.md#black-scholes-1973
    - Merton (1973): see docs/REFERENCES.md#merton-1973
    - Garman-Kohlhagen (1983): see docs/REFERENCES.md#garman-kohlhagen-1983

    """
    ...

def vanilla_expiry_payoff(spot: float, strike: float, is_call: bool) -> float:
    """
    Vanilla option payoff at expiry: ``max(±(spot - strike), 0)``.

    Parameters
    ----------
    spot : float
        Underlying level at expiry, in the same price units as ``strike``.
        Must be finite and non-negative; zero spot is allowed.
    strike : float
        Exercise price; must be finite and strictly positive.
    is_call : bool
        ``True`` for a call (``max(spot - strike, 0)``), ``False`` for a put
        (``max(strike - spot, 0)``).

    Returns
    -------
    float
        Undiscounted expiry payoff in the same units as ``spot`` and ``strike``.

    Raises
    ------
    ValueError
        If ``spot`` is non-finite or negative, or ``strike`` is non-finite or
        not strictly positive.

    Examples
    --------
    >>> from finstack_quant.models import vanilla_expiry_payoff
    >>> vanilla_expiry_payoff(110.0, 100.0, True)
    10.0
    """
    ...

def bs_greeks(
    spot: float,
    strike: float,
    rate: float,
    div_yield: float,
    vol: float,
    expiry: float,
    is_call: bool,
    theta_days: float = 365.0,
) -> BsGreeks:
    """
    Black-Scholes / Garman-Kohlhagen Greeks as a typed :class:`BsGreeks`.

    ``vega``, ``rho_r`` and ``rho_q`` are per 1% move; ``theta`` is per-day
    using the ``theta_days`` day-count denominator (ACT/365 by default).

    Parameters
    ----------
    spot : float
        Spot price of the underlying.
    strike : float
        Option strike.
    rate : float
        Continuously compounded risk-free rate (decimal).
    div_yield : float
        Continuous dividend/borrow yield (decimal).
    vol : float
        Annualized volatility (decimal); must be positive.
    expiry : float
        Time to expiry in years; must be positive.
    is_call : bool
        ``True`` for a call, ``False`` for a put.
    theta_days : float, default 365.0
        Day-count denominator for theta scaling.

    Returns
    -------
    BsGreeks
        Typed Greeks with ``to_series()`` / ``to_dataframe()`` exits.

    Raises
    ------
    ValueError
        If any numeric input is non-finite; ``spot`` or ``strike`` is
        non-positive; ``vol``, ``expiry``, or ``theta_days`` is non-positive;
        or a computed Greek is non-finite.

    Examples
    --------
    >>> from finstack_quant.models import bs_greeks
    >>> greeks = bs_greeks(100.0, 100.0, 0.05, 0.0, 0.2, 1.0, True)
    >>> (round(greeks.delta, 4), round(greeks.rho_r, 4))
    (0.6368, 0.5323)

    Sources
    -------
    - Black-Scholes (1973): see docs/REFERENCES.md#black-scholes-1973
    - Merton (1973): see docs/REFERENCES.md#merton-1973
    - Garman-Kohlhagen (1983): see docs/REFERENCES.md#garman-kohlhagen-1983

    """
    ...

def bs_implied_vol(
    spot: float,
    strike: float,
    rate: float,
    div_yield: float,
    expiry: float,
    price: float,
    is_call: bool,
) -> float:
    """
    Solve for Black-Scholes implied volatility given a target price.

    Parameters
    ----------
    spot : float
        Spot price of the underlying.
    strike : float
        Option strike.
    rate : float
        Continuously compounded risk-free rate (decimal).
    div_yield : float
        Continuous dividend/borrow yield (decimal).
    expiry : float
        Time to expiry in years; must be strictly positive (an expired option
        has no implied volatility).
    price : float
        Observed option price in the same units as spot.
    is_call : bool
        ``True`` for a call, ``False`` for a put.

    Returns
    -------
    float
        Implied volatility as a decimal.

    Raises
    ------
    ValueError
        If an input is non-finite; ``expiry``, ``spot``, ``strike`` or
        ``price`` is non-positive; ``price`` is at or below intrinsic value or
        cannot be bracketed; or the solver does not converge.

    Examples
    --------
    >>> from finstack_quant.models import bs_implied_vol, bs_price
    >>> price = bs_price(100.0, 100.0, 0.05, 0.0, 0.2, 1.0, True)
    >>> round(bs_implied_vol(100.0, 100.0, 0.05, 0.0, 1.0, price, True), 6)
    0.2

    Sources
    -------
    - Black-Scholes (1973): see docs/REFERENCES.md#black-scholes-1973
    - Merton (1973): see docs/REFERENCES.md#merton-1973

    """
    ...

def black76_implied_vol(
    forward: float,
    strike: float,
    df: float,
    expiry: float,
    price: float,
    is_call: bool,
) -> float:
    """
    Solve for Black-76 (forward-based) implied volatility given a target price.

    Parameters
    ----------
    forward : float
        Forward price at expiry.
    strike : float
        Option strike.
    df : float
        Discount factor from valuation date to expiry.
    expiry : float
        Time to expiry in years; must be strictly positive.
    price : float
        Observed (discounted) option price, same units as forward.
    is_call : bool
        ``True`` for a call, ``False`` for a put.

    Returns
    -------
    float
        Implied volatility as a decimal.

    Raises
    ------
    ValueError
        If an input is non-finite; ``expiry``, ``forward``, ``strike``, ``df``
        or ``price`` is non-positive; ``price`` is not above intrinsic or
        cannot be bracketed; or the solver does not converge.

    Examples
    --------
    >>> from finstack_quant.models import black76_implied_vol
    >>> round(black76_implied_vol(100.0, 100.0, 0.95, 1.0, 7.5673, True), 6)
    0.2

    Sources
    -------
    - Black (1976): see docs/REFERENCES.md#black-1976

    """
    ...

def black76_price(
    forward: float,
    strike: float,
    df: float,
    expiry: float,
    vol: float,
    is_call: bool,
) -> float:
    """
    Black-76 per-unit price of a European option on a forward.

    ``df * Black(forward, strike, vol, expiry)``: the undiscounted Black
    premium scaled by the supplied discount factor.

    Parameters
    ----------
    forward : float
        Forward price or rate at expiry.
    strike : float
        Strike in the same units as ``forward``.
    df : float
        Discount factor from valuation date to expiry (positive decimal).
    expiry : float
        Time to expiry in years.
    vol : float
        Annualized lognormal (Black) volatility, decimal.
    is_call : bool
        ``True`` for a call, ``False`` for a put.

    Returns
    -------
    float
        Discounted per-unit option price in the units of ``forward``.

    Raises
    ------
    ValueError
        If the inputs produce a non-finite price.

    Examples
    --------
    >>> from finstack_quant.models import black76_price
    >>> round(black76_price(100.0, 100.0, 0.95, 1.0, 0.2, True), 4)
    7.5673

    Sources
    -------
    - Black (1976): see docs/REFERENCES.md#black-1976
    """
    ...

def black76_greeks(
    forward: float,
    strike: float,
    expiry: float,
    vol: float,
    is_call: bool,
) -> dict[str, float]:
    """
    Black-76 undiscounted forward Greeks ``{"delta", "gamma", "vega"}``.

    ``delta`` / ``gamma`` are with respect to the forward; ``vega`` is per
    unit (1.0) change in ``vol``. Multiply by the discount factor for
    present-value sensitivities.

    Parameters
    ----------
    forward : float
        Forward price or rate at expiry.
    strike : float
        Strike in the same units as ``forward``.
    expiry : float
        Time to expiry in years.
    vol : float
        Annualized lognormal (Black) volatility, decimal.
    is_call : bool
        ``True`` for a call, ``False`` for a put (only ``delta`` differs).

    Returns
    -------
    dict[str, float]
        ``{"delta": ..., "gamma": ..., "vega": ...}``.

    Raises
    ------
    ValueError
        If any Greek is non-finite for the supplied inputs.

    Examples
    --------
    >>> from finstack_quant.models import black76_greeks
    >>> round(black76_greeks(100.0, 100.0, 1.0, 0.2, True)["delta"], 4)
    0.5398

    Sources
    -------
    - Black (1976): see docs/REFERENCES.md#black-1976
    """
    ...

def bachelier_price(
    forward: float,
    strike: float,
    normal_vol: float,
    expiry: float,
    is_call: bool,
) -> float:
    """
    Bachelier (normal-model) undiscounted per-unit price of a European option.

    Parameters
    ----------
    forward : float
        Forward price or rate at expiry (may be negative).
    strike : float
        Strike in the same units as ``forward``.
    normal_vol : float
        Annualized **absolute** (normal) volatility in the units of
        ``forward`` (``0.0075`` = 75 bp on decimal rates).
    expiry : float
        Time to expiry in years.
    is_call : bool
        ``True`` for a call (payer), ``False`` for a put (receiver).

    Returns
    -------
    float
        Undiscounted per-unit option value in the units of ``forward``.

    Raises
    ------
    ValueError
        If the inputs produce a non-finite price.

    Examples
    --------
    >>> from finstack_quant.models import bachelier_price
    >>> round(bachelier_price(0.03, 0.03, 0.0075, 1.0, True), 6)
    0.002992

    Sources
    -------
    - Bachelier (1900): see docs/REFERENCES.md#bachelier-1900
    """
    ...

def bachelier_greeks(
    forward: float,
    strike: float,
    normal_vol: float,
    expiry: float,
    is_call: bool,
) -> dict[str, float]:
    """
    Bachelier (normal-model) undiscounted forward Greeks ``{"delta", "gamma", "vega"}``.

    ``vega`` is per unit (1.0) change in ``normal_vol`` (absolute units).

    Parameters
    ----------
    forward : float
        Forward price or rate at expiry (may be negative).
    strike : float
        Strike in the same units as ``forward``.
    normal_vol : float
        Annualized absolute (normal) volatility in the units of ``forward``.
    expiry : float
        Time to expiry in years.
    is_call : bool
        ``True`` for a call, ``False`` for a put (only ``delta`` differs).

    Returns
    -------
    dict[str, float]
        ``{"delta": ..., "gamma": ..., "vega": ...}``.

    Raises
    ------
    ValueError
        If any Greek is non-finite for the supplied inputs.

    Examples
    --------
    >>> from finstack_quant.models import bachelier_greeks
    >>> round(bachelier_greeks(0.03, 0.03, 0.0075, 1.0, True)["delta"], 2)
    0.5

    Sources
    -------
    - Bachelier (1900): see docs/REFERENCES.md#bachelier-1900
    """
    ...

def black_shifted_price(
    forward: float,
    strike: float,
    vol: float,
    expiry: float,
    shift: float,
    is_call: bool,
) -> float:
    """
    Shifted (displaced) Black undiscounted per-unit price for negative-rate markets.

    Prices ``Black(forward + shift, strike + shift, vol, expiry)``.

    Parameters
    ----------
    forward : float
        Forward rate at expiry (decimal; may be negative).
    strike : float
        Strike (decimal, same units as ``forward``).
    vol : float
        Annualized shifted-lognormal volatility, decimal.
    expiry : float
        Time to expiry in years.
    shift : float
        Displacement added to forward and strike, in rate units (``0.03`` =
        3% shift); both shifted values must be positive.
    is_call : bool
        ``True`` for a call, ``False`` for a put.

    Returns
    -------
    float
        Undiscounted per-unit option value in the units of ``forward``.

    Raises
    ------
    ValueError
        If the inputs produce a non-finite price.

    Examples
    --------
    >>> from finstack_quant.models import black_shifted_price
    >>> round(black_shifted_price(-0.005, -0.005, 0.25, 1.0, 0.03, True), 6)
    0.002487
    """
    ...

def black_shifted_vega(
    forward: float,
    strike: float,
    vol: float,
    expiry: float,
    shift: float,
) -> float:
    """
    Shifted (displaced) Black vega per unit (1.0) change in ``vol``, undiscounted.

    Parameters
    ----------
    forward : float
        Forward rate at expiry (decimal; may be negative).
    strike : float
        Strike (decimal, same units as ``forward``).
    vol : float
        Annualized shifted-lognormal volatility, decimal.
    expiry : float
        Time to expiry in years.
    shift : float
        Displacement added to forward and strike, in rate units.

    Returns
    -------
    float
        Undiscounted vega in the units of ``forward`` per unit vol.

    Raises
    ------
    ValueError
        If the inputs produce a non-finite vega.

    Examples
    --------
    >>> from finstack_quant.models import black_shifted_vega
    >>> black_shifted_vega(-0.005, -0.005, 0.25, 1.0, 0.03) > 0
    True
    """
    ...

def heston_price(
    spot: float,
    strike: float,
    expiry: float,
    rate: float,
    div_yield: float,
    kappa: float,
    theta: float,
    sigma_v: float,
    rho: float,
    v0: float,
    is_call: bool = True,
) -> float:
    """
    Closed-form (Fourier) Heston price of a European option.

    Semi-analytical Heston (1993) price via the stable characteristic-function
    branch with adaptive quadrature; puts use put-call parity.

    Parameters
    ----------
    spot : float
        Spot price of the underlying.
    strike : float
        Option strike.
    expiry : float
        Time to expiry in years; ``expiry <= 0`` returns intrinsic value.
    rate : float
        Continuously compounded risk-free rate (decimal).
    div_yield : float
        Continuous dividend / foreign yield (decimal).
    kappa : float
        Mean-reversion speed of the variance process (per year, positive).
    theta : float
        Long-run variance level (variance units, positive).
    sigma_v : float
        Volatility of variance (vol-of-vol, positive).
    rho : float
        Spot/variance correlation in ``(-1, 1)``.
    v0 : float
        Initial instantaneous variance (variance, not volatility; positive).
    is_call : bool, default True
        ``True`` for a call, ``False`` for a put.

    Returns
    -------
    float
        Present-value per-unit option price.

    Raises
    ------
    ValueError
        If a Heston parameter or rate is non-finite or outside its domain.
    RuntimeError
        If the Fourier integration fails to produce a finite price.

    Examples
    --------
    >>> from finstack_quant.models import heston_price
    >>> p = heston_price(100.0, 100.0, 1.0, 0.05, 0.02, 2.0, 0.04, 0.3, -0.7, 0.04)
    >>> 5.0 < p < 15.0
    True

    Sources
    -------
    - Heston (1993): see docs/REFERENCES.md#heston-1993
    - Albrecher et al. (2007): see docs/REFERENCES.md#albrecher-2007-little-heston-trap
    """
    ...

# Closed-form exotics

def barrier_call(
    spot: float,
    strike: float,
    barrier: float,
    rate: float,
    div_yield: float,
    vol: float,
    expiry: float,
    direction: str,
    knock: str,
) -> float:
    """
    Reiner-Rubinstein continuous-monitoring barrier call price.

    ``direction`` is ``"up"`` or ``"down"``; ``knock`` is ``"in"`` or ``"out"``.

    Parameters
    ----------
    spot : float
        Spot price of the underlying.
    strike : float
        Option strike.
    barrier : float
        Barrier level.
    rate : float
        Continuously compounded risk-free rate (decimal).
    div_yield : float
        Continuous dividend/borrow yield (decimal).
    vol : float
        Annualized volatility (decimal).
    expiry : float
        Time to expiry in years.
    direction : str
        ``"up"`` or ``"down"``.
    knock : str
        ``"in"`` or ``"out"``.

    Returns
    -------
    float
        Per-unit barrier call price.

    Raises
    ------
    ValueError
        If ``direction`` is not ``"up"`` or ``"down"``, ``knock`` is not
        ``"in"`` or ``"out"``, or the formula produces a non-finite price.

    Examples
    --------
    >>> from finstack_quant.models import barrier_call
    >>> round(barrier_call(100.0, 100.0, 120.0, 0.05, 0.0, 0.2, 1.0, "up", "out"), 4)
    1.1761

    Sources
    -------
    - Reiner-Rubinstein (1991): see docs/REFERENCES.md#reiner-rubinstein-1991

    """
    ...

def barrier_put(
    spot: float,
    strike: float,
    barrier: float,
    rate: float,
    div_yield: float,
    vol: float,
    expiry: float,
    direction: str,
    knock: str,
) -> float:
    """
    Reiner-Rubinstein continuous-monitoring barrier put price.

    ``direction`` is ``"up"`` or ``"down"``; ``knock`` is ``"in"`` or ``"out"``.

    Parameters
    ----------
    spot : float
        Spot price of the underlying.
    strike : float
        Option strike.
    barrier : float
        Barrier level.
    rate : float
        Continuously compounded risk-free rate (decimal).
    div_yield : float
        Continuous dividend/borrow yield (decimal).
    vol : float
        Annualized volatility (decimal).
    expiry : float
        Time to expiry in years.
    direction : str
        ``"up"`` or ``"down"``.
    knock : str
        ``"in"`` or ``"out"``.

    Returns
    -------
    float
        Per-unit barrier put price.

    Raises
    ------
    ValueError
        If ``direction`` is not ``"up"`` or ``"down"``, ``knock`` is not
        ``"in"`` or ``"out"``, or the formula produces a non-finite price.

    Examples
    --------
    >>> from finstack_quant.models import barrier_put
    >>> barrier_put(100.0, 100.0, 80.0, 0.05, 0.0, 0.2, 1.0, "down", "out") > 0
    True

    Sources
    -------
    - Reiner-Rubinstein (1991): see docs/REFERENCES.md#reiner-rubinstein-1991

    """
    ...

def asian_option_price(
    spot: float,
    strike: float,
    rate: float,
    div_yield: float,
    vol: float,
    expiry: float,
    num_fixings: int,
    averaging: str = "arithmetic",
    is_call: bool = True,
) -> float:
    """
    Arithmetic (Turnbull-Wakeman) or geometric (Kemna-Vorst) Asian option price.

    Parameters
    ----------
    spot : float
        Spot price of the underlying.
    strike : float
        Option strike.
    rate : float
        Continuously compounded risk-free rate (decimal).
    div_yield : float
        Continuous dividend/borrow yield (decimal).
    vol : float
        Annualized volatility (decimal).
    expiry : float
        Time to expiry in years.
    num_fixings : int
        Number of averaging fixings.
    averaging : str, default "arithmetic"
        ``"arithmetic"`` (Turnbull-Wakeman) or ``"geometric"`` (Kemna-Vorst).
    is_call : bool, default True
        ``True`` for a call, ``False`` for a put.

    Returns
    -------
    float
        Per-unit Asian option price.

    Raises
    ------
    ValueError
        If ``averaging`` is not ``"arithmetic"`` or ``"geometric"``, or the
        formula produces a non-finite price.

    Examples
    --------
    >>> from finstack_quant.models import asian_option_price
    >>> round(asian_option_price(100.0, 100.0, 0.05, 0.0, 0.2, 1.0, 12), 4)
    6.1742

    Sources
    -------
    - Kemna-Vorst (1990): see docs/REFERENCES.md#kemna-vorst-1990
    - Turnbull-Wakeman (1991): see docs/REFERENCES.md#turnbull-wakeman-1991

    """
    ...

def lookback_option_price(
    spot: float,
    strike: float,
    rate: float,
    div_yield: float,
    vol: float,
    expiry: float,
    extremum: float,
    strike_type: str = "fixed",
    is_call: bool = True,
) -> float:
    """
    Conze-Viswanathan lookback option price.

    For ``strike_type="floating"``, ``strike`` is ignored and ``extremum``
    is the observed min (call) / max (put) to date.

    Parameters
    ----------
    spot : float
        Spot price of the underlying.
    strike : float
        Option strike (ignored for floating strike).
    rate : float
        Continuously compounded risk-free rate (decimal).
    div_yield : float
        Continuous dividend/borrow yield (decimal).
    vol : float
        Annualized volatility (decimal).
    expiry : float
        Time to expiry in years.
    extremum : float
        Observed extremum (min for call, max for put) to date.
    strike_type : str, default "fixed"
        ``"fixed"`` or ``"floating"``.
    is_call : bool, default True
        ``True`` for a call, ``False`` for a put.

    Returns
    -------
    float
        Per-unit lookback option price.

    Raises
    ------
    ValueError
        If ``strike_type`` is not ``"fixed"`` or ``"floating"``, or the
        formula produces a non-finite price.

    Examples
    --------
    >>> from finstack_quant.models import lookback_option_price
    >>> round(lookback_option_price(100.0, 100.0, 0.05, 0.0, 0.2, 1.0, 100.0), 4)
    19.1676

    Sources
    -------
    - Conze-Viswanathan (1991): see docs/REFERENCES.md#conze-viswanathan-1991

    """
    ...

def quanto_option_price(
    spot: float,
    strike: float,
    expiry: float,
    rate_domestic: float,
    rate_foreign: float,
    div_yield: float,
    vol_asset: float,
    vol_fx: float,
    correlation: float,
    is_call: bool = True,
) -> float:
    """
    Quanto option (FX-adjusted cross-currency) price in domestic currency.

    Parameters
    ----------
    spot : float
        Spot price of the underlying in foreign currency.
    strike : float
        Option strike in foreign currency.
    expiry : float
        Time to expiry in years.
    rate_domestic : float
        Domestic risk-free rate (decimal, continuously compounded).
    rate_foreign : float
        Foreign risk-free rate (decimal, continuously compounded).
    div_yield : float
        Dividend yield on the underlying (decimal).
    vol_asset : float
        Volatility of the underlying asset (decimal).
    vol_fx : float
        Volatility of the FX rate (decimal).
    correlation : float
        Correlation between asset and FX returns.
    is_call : bool, default True
        ``True`` for a call, ``False`` for a put.

    Returns
    -------
    float
        Per-unit quanto option price in domestic currency.

    Raises
    ------
    ValueError
        If the supplied inputs produce a non-finite quanto price.

    Examples
    --------
    >>> from finstack_quant.models import quanto_option_price
    >>> round(quanto_option_price(100.0, 100.0, 1.0, 0.05, 0.02, 0.01, 0.2, 0.1, 0.3), 4)
    7.7844

    Sources
    -------
    - Garman-Kohlhagen (1983): see docs/REFERENCES.md#garman-kohlhagen-1983

    """
    ...

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
        Positive number of COS terms. Uses a default when ``None``.

    Returns
    -------
    float
        Per-unit option price.

    Raises
    ------
    ValueError
        If ``vol`` is not strictly positive, the inputs produce an invalid COS
        truncation range, a non-finite characteristic-function value, or a
        non-finite option price.

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
        VG diffusion parameter (volatility).
    theta : float
        VG drift parameter.
    nu : float
        VG variance rate parameter.
    expiry : float
        Time to expiry in years.
    is_call : bool
        ``True`` for a call, ``False`` for a put.
    n_terms : int, optional
        Positive number of COS terms. Uses a default when ``None``.

    Returns
    -------
    float
        Per-unit option price.

    Raises
    ------
    ValueError
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
        Positive number of COS terms. Uses a default when ``None``.

    Returns
    -------
    float
        Per-unit option price.

    Raises
    ------
    ValueError
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
