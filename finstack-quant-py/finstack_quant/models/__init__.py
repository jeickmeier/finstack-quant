"""Reusable analytical, Fourier, credit, correlation, liquidity, and stochastic models.

Bindings for the ``finstack-quant-models`` Rust crate.

Examples:
--------
>>> from finstack_quant.models import bs_price
>>> round(bs_price(100.0, 100.0, 0.05, 0.0, 0.2, 1.0, True), 4)
10.4506

"""

from finstack_quant.finstack_quant import models as _models
from finstack_quant.models import (
    correlation as correlation,
    credit as credit,
    factor as factor,
    liquidity as liquidity,
    monte_carlo as monte_carlo,
    rates as rates,
    volatility as volatility,
)

BsGreeks = _models.BsGreeks
ForwardGreeks = _models.ForwardGreeks
asian_option_price = _models.asian_option_price
bachelier_greeks = _models.bachelier_greeks
bachelier_price = _models.bachelier_price
barrier_call = _models.barrier_call
barrier_put = _models.barrier_put
black76_greeks = _models.black76_greeks
black76_implied_vol = _models.black76_implied_vol
black76_price = _models.black76_price
black_shifted_price = _models.black_shifted_price
black_shifted_vega = _models.black_shifted_vega
bs_cos_price = _models.bs_cos_price
bs_greeks = _models.bs_greeks
bs_implied_vol = _models.bs_implied_vol
bs_price = _models.bs_price
heston_price = _models.heston_price
lookback_option_price = _models.lookback_option_price
merton_jump_cos_price = _models.merton_jump_cos_price
quanto_option_price = _models.quanto_option_price
vanilla_expiry_payoff = _models.vanilla_expiry_payoff
vg_cos_price = _models.vg_cos_price

__all__: list[str] = [
    "BsGreeks",
    "ForwardGreeks",
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
