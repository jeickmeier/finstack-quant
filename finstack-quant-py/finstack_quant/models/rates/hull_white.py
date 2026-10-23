# ruff: noqa: D100
# Module help is compiled from the matching stub and delegated below.

from finstack_quant.finstack_quant import models as _models

_hull_white = _models.rates.hull_white

HullWhiteParams = _hull_white.HullWhiteParams
hw1f_cap_floor_price = _hull_white.hw1f_cap_floor_price
hw1f_caplet_forward_rate_normal_vol = _hull_white.hw1f_caplet_forward_rate_normal_vol
hw1f_convexity_adjustment = _hull_white.hw1f_convexity_adjustment
hw1f_zcb_option_price = _hull_white.hw1f_zcb_option_price
hw_bond_vol = _hull_white.hw_bond_vol

__all__: list[str] = [
    "HullWhiteParams",
    "hw1f_cap_floor_price",
    "hw1f_caplet_forward_rate_normal_vol",
    "hw1f_convexity_adjustment",
    "hw1f_zcb_option_price",
    "hw_bond_vol",
]

__doc__ = _models.rates.hull_white.__doc__
