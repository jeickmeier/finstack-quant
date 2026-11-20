"""Core handle members bound in both hosts: the Python side of the S20 goldens.

The WASM facade test ``finstack-quant-wasm/tests/facade/core_members.test.mjs``
asserts the same values for the JavaScript twins. Every number is computed by
the Rust crates, so both hosts must agree exactly.
"""

from __future__ import annotations

import datetime as dt
import math

import pytest

from finstack_quant.core.dates import DayCount, DayCountContext, Tenor
from finstack_quant.core.market_data import HazardCurve, VolCube
from finstack_quant.core.math.stats import log_returns, realized_variance, realized_variance_ohlc
from finstack_quant.core.types import Bps, Rate

PRICES = [100.0, 101.0, 99.5, 100.2, 102.0]
OPEN = [100.0, 101.0, 99.5, 100.2]
HIGH = [101.5, 102.0, 100.8, 101.0]
LOW = [99.0, 99.2, 98.9, 99.6]
CLOSE = [101.0, 99.5, 100.2, 100.6]


def test_realized_variance_defaults_are_rust_owned() -> None:
    assert realized_variance(PRICES) == pytest.approx(0.04341008921510013, rel=1e-13)
    assert realized_variance(PRICES, 52.0) == pytest.approx(0.008957637457084153, rel=1e-13)
    assert realized_variance_ohlc(OPEN[1:], HIGH[1:], LOW[1:], CLOSE[1:], previous_close=CLOSE[0]) == pytest.approx(
        0.04489594471070698, rel=1e-13
    )
    assert realized_variance_ohlc(OPEN, HIGH, LOW, CLOSE, "parkinson") == pytest.approx(0.04439216444474536, rel=1e-13)


def test_realized_variance_availability_and_log_domain() -> None:
    assert realized_variance([100.0]) is None
    assert realized_variance([100.0, 100.0]) == 0.0
    assert log_returns([1e-300, 1e300]) == pytest.approx([600 * math.log(10)])
    with pytest.raises(ValueError, match=r"positive"):
        log_returns([0.0])


def test_rate_family_values_match_wasm() -> None:
    assert Rate("12.5bp").as_decimal == 0.00125
    assert Rate("17.5bp").as_bp == 17.5
    assert Bps(-1995).as_percent == -19.95
    assert Rate(0.0525).to_json() == "0.0525"
    assert Bps(25).to_json() == "25"


def test_day_count_and_tenor_values_match_wasm() -> None:
    assert str(DayCount.from_name(" Act_Act_Isma ")) == "act_act_isma"
    assert str(DayCount.NL_365) == "nl_365"
    tenor = Tenor.parse("3M")
    assert str(tenor.unit) == "M"
    assert tenor.to_days_approx() == 91
    assert Tenor.parse("1M").add_to_date(dt.date(2025, 1, 31), business_day_convention="unadjusted") == dt.date(
        2025, 2, 28
    )
    # No convention: the Rust default (modified following) applies with a calendar.
    assert Tenor.parse("1M").add_to_date(
        dt.date(2025, 5, 31), "nyse", business_day_convention="modified_following"
    ) == dt.date(2025, 6, 30)
    assert (
        Tenor.parse("6M").to_years_with_context(
            dt.date(2025, 1, 15), day_count=DayCount.ACT_360, business_day_convention="unadjusted"
        )
        == 0.5027777777777778
    )


def test_day_count_context_round_trips_like_wasm() -> None:
    ctx = DayCountContext("nyse", "3M", 252, (dt.date(1970, 1, 11), dt.date(1970, 1, 21)), True)
    assert DayCountContext.from_json(ctx.to_json()) == ctx


def test_vol_cube_grid_index_is_checked_by_rust() -> None:
    cube = VolCube.from_json(
        '{"id":"CUBE","expiries":[1.0,2.0],"tenors":[5.0],'
        '"params":[{"alpha":0.02,"beta":0.5,"rho":-0.2,"nu":0.4},'
        '{"alpha":0.03,"beta":0.5,"rho":-0.1,"nu":0.3,"shift":0.01}],'
        '"forwards":[0.03,0.035],"interpolation_mode":"vol"}'
    )
    assert cube.forward_at(1, 0) == 0.035
    with pytest.raises(ValueError, match=r"grid index \(2, 0\) outside shape \(2, 1\)"):
        cube.forward_at(2, 0)
    with pytest.raises(ValueError, match=r"grid index \(0, 1\) outside shape \(2, 1\)"):
        cube.params_at(0, 1)


def test_hazard_curve_json_round_trip() -> None:
    curve = HazardCurve(
        "HZ",
        "2025-01-01",
        [(1.0, 0.02), (5.0, 0.03)],
        recovery_rate=0.4,
        day_count="act_365f",
        issuer="ACME",
        seniority="senior",
        currency="USD",
    )
    assert HazardCurve.from_json(curve.to_json()).to_json() == curve.to_json()
    assert curve.knots == [1.0, 5.0]
    assert curve.hazard_rates == [0.02, 0.03]
    assert curve.day_count == "act_365f"
    assert curve.calendar_id is None


def test_realized_variance_includes_the_opening_gap() -> None:
    bars = [110.0] * 3
    for method in ["yang_zhang", "close_to_close"]:
        actual = realized_variance_ohlc(bars, bars, bars, bars, method, 252.0, 100.0)
        assert actual == pytest.approx(252 * math.log(1.1) ** 2 / 3)
        with pytest.raises(ValueError, match="previous_close"):
            realized_variance_ohlc(bars, bars, bars, bars, method, 252.0)
    assert realized_variance_ohlc([110.0], [110.0], [110.0], [110.0], "yang_zhang", 252.0, 100.0) is None
