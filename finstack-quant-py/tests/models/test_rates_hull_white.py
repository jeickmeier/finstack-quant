"""Hull-White one-factor scalar bindings."""

import datetime
import json
import math
import pickle

import pytest

from finstack_quant.core.market_data import DiscountCurve
from finstack_quant.models.rates.hull_white import (
    HullWhiteParams,
    hw1f_cap_floor_price,
    hw1f_caplet_forward_rate_normal_vol,
    hw1f_convexity_adjustment,
    hw1f_zcb_option_price,
    hw_bond_vol,
)


def test_params_constant_and_piecewise_agree_with_scalars() -> None:
    params = HullWhiteParams(0.05, 0.01)
    assert params.get_times() == [0.0]
    assert params.get_values() == [0.01]
    assert json.loads(params.to_json())["volatility"] == {"times": [0.0], "values": [0.01]}
    assert params.get_sigma(2.0) == 0.01
    assert params.bond_vol(0.0, 1.0, 2.0) == pytest.approx(hw_bond_vol(0.05, 0.01, 0.0, 1.0, 2.0))
    expected_var = 0.01**2 * (1.0 - math.exp(-0.1)) / 0.1
    assert params.state_variance(1.0) == pytest.approx(expected_var)
    assert HullWhiteParams.from_json(params.to_json()) == params
    assert pickle.loads(pickle.dumps(params)) == params  # noqa: S301

    stepped = HullWhiteParams.piecewise(0.05, [0.0, 1.0], [0.01, 0.02])
    assert stepped.get_sigma(1.0) == 0.02
    assert stepped.get_sigma(1.5) == 0.02
    with pytest.raises(ValueError, match=r"kappa must be positive and finite"):
        HullWhiteParams(0.0, 0.01)
    with pytest.raises(ValueError, match=r"first knot must be 0.0"):
        HullWhiteParams.piecewise(0.05, [0.5, 1.0], [0.01, 0.02])


def test_scalar_kernels_satisfy_known_identities() -> None:
    call = hw1f_zcb_option_price(0.98, 0.94, 0.96, 0.03, True)
    put = hw1f_zcb_option_price(0.98, 0.94, 0.96, 0.03, False)
    assert call - put == pytest.approx(0.94 - 0.96 * 0.98)
    ho_lee = hw1f_convexity_adjustment(1e-12, 0.01, 1.0, 1.25)
    assert ho_lee == pytest.approx(0.5 * 0.01**2 * 1.0 * (2.0 * 1.25 - 1.0))
    assert hw1f_caplet_forward_rate_normal_vol(0.05, 0.01, 1.0, 0.25) > 0.0
    assert hw1f_caplet_forward_rate_normal_vol(0.05, 0.01, 0.0, 0.25) == 0.0


@pytest.mark.parametrize("invalid", [math.nan, math.inf, -math.inf, -0.01])
def test_convexity_invalid_parameters_remain_visible_at_zero_expiry(invalid: float) -> None:
    assert math.isnan(hw1f_convexity_adjustment(invalid, 0.01, 0.0, 1.0))
    assert math.isnan(hw1f_convexity_adjustment(0.05, invalid, 0.0, 1.0))


def test_cap_floor_parity_with_discount_curve() -> None:
    curve = DiscountCurve.flat("USD-OIS", datetime.date(2025, 1, 1), 0.03)
    periods = [(1.0, 2.0, 1.0)]
    cap = hw1f_cap_floor_price(0.05, 0.01, periods, 0.025, True, curve, curve)
    floor = hw1f_cap_floor_price(0.05, 0.01, periods, 0.025, False, curve, curve)
    pf_fix, pf_pay = curve.df(1.0), curve.df(2.0)
    forward = (pf_fix / pf_pay - 1.0) / 1.0
    assert cap - floor == pytest.approx(pf_pay * 1.0 * (forward - 0.025))
    projection = DiscountCurve.flat("USD-FWD", datetime.date(2025, 1, 1), 0.04)
    projected_forward = projection.df(1.0) / projection.df(2.0) - 1.0
    projected_cap = hw1f_cap_floor_price(0.05, 0.01, periods, 0.025, True, curve, projection)
    projected_floor = hw1f_cap_floor_price(0.05, 0.01, periods, 0.025, False, curve, projection)
    assert projected_cap - projected_floor == pytest.approx(pf_pay * (projected_forward - 0.025))
    assert projected_cap > cap
    other_discount = DiscountCurve.flat("USD-OIS-OTHER", datetime.date(2025, 1, 1), 0.06)
    other_cap = hw1f_cap_floor_price(0.05, 0.01, periods, 0.025, True, other_discount, projection)
    assert other_cap / projected_cap == pytest.approx(other_discount.df(2.0) / pf_pay)
    with pytest.raises(TypeError):
        hw1f_cap_floor_price(0.05, 0.01, periods, 0.025, True, curve)
    with pytest.raises(TypeError):
        hw1f_cap_floor_price(0.05, 0.01, periods, 0.025, True, curve, None)
    with pytest.raises(ValueError, match=r"must be positive, got -0.05"):
        hw1f_cap_floor_price(-0.05, 0.01, periods, 0.025, True, curve, curve)
