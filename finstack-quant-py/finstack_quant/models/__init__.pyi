"""Reusable analytical, numerical, volatility, Fourier, and stochastic models.

The root namespace contains model-family functions and classes. Correlation,
credit, Monte Carlo, and rates APIs are grouped under the matching submodules.

Examples
--------
>>> from finstack_quant.models import bs_price
>>> round(bs_price(100.0, 100.0, 0.05, 0.0, 0.2, 1.0, True), 4)
10.4506
"""

from finstack_quant.models import correlation as correlation
from finstack_quant.models import credit as credit
from finstack_quant.models import factor as factor
from finstack_quant.models import liquidity as liquidity
from finstack_quant.models import monte_carlo as monte_carlo
from finstack_quant.models import rates as rates
from finstack_quant.models import volatility as volatility
from finstack_quant.models._closed_form import (
    BsGreeks as BsGreeks,
    ForwardGreeks as ForwardGreeks,
    asian_option_price as asian_option_price,
    bachelier_greeks as bachelier_greeks,
    bachelier_price as bachelier_price,
    barrier_option_price as barrier_option_price,
    black76_greeks as black76_greeks,
    black76_implied_vol as black76_implied_vol,
    black76_price as black76_price,
    black_shifted_price as black_shifted_price,
    black_shifted_vega as black_shifted_vega,
    bs_greeks as bs_greeks,
    bs_implied_vol as bs_implied_vol,
    bs_price as bs_price,
    heston_price as heston_price,
    lookback_option_price as lookback_option_price,
    quanto_option_price as quanto_option_price,
    vanilla_expiry_payoff as vanilla_expiry_payoff,
)
from finstack_quant.models._fourier import (
    bs_cos_price as bs_cos_price,
    merton_jump_cos_price as merton_jump_cos_price,
    vg_cos_price as vg_cos_price,
)

__all__ = [
    "BsGreeks",
    "ForwardGreeks",
    "asian_option_price",
    "bachelier_greeks",
    "bachelier_price",
    "barrier_option_price",
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
