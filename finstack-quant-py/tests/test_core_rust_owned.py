"""Core primitives, dates and market data: behaviour owned by Rust.

Every value and message asserted here comes from the Rust crates, so the WASM
facade test ``finstack-quant-wasm/tests/facade/core_rust_owned.test.mjs``
asserts the same strings for the JavaScript twins.
"""

from __future__ import annotations

from collections.abc import Callable
import datetime as dt
import re
from typing import Any

import pytest

from finstack_quant.core.currency import Currency
from finstack_quant.core.dates import DayCount, DayCountContext
from finstack_quant.core.market_data import (
    DiscountCurve,
    FxDeltaVolSurface,
    FxMatrix,
    HazardCurve,
    VolCube,
)
from finstack_quant.core.math import stats
from finstack_quant.core.money import Money
from finstack_quant.valuations.instruments import FxForward


def _unknown_currency(code: str) -> str:
    return f'Invalid currency code "{code}": not a supported ISO-4217 alphabetic code'


def _raises_exactly(message: str) -> Any:
    """``pytest.raises(ValueError)`` pinning the whole Rust-owned message."""
    return pytest.raises(ValueError, match=f"^{re.escape(message)}$")


@pytest.mark.parametrize(
    ("build", "code"),
    [
        (lambda: Currency(" USD "), " USD "),
        (lambda: Currency("XXX"), "XXX"),
        (lambda: Money.from_decimal_str("1", " usd "), " usd "),
        (lambda: Money(1.0, " usd "), " usd "),
    ],
)
def test_one_rust_currency_parser_names_the_code_and_does_not_trim(build: Callable[[], object], code: str) -> None:
    with _raises_exactly(_unknown_currency(code)):
        build()


def test_money_text_amount_is_not_trimmed() -> None:
    assert Money("100.25", "USD").amount_decimal == Money.from_decimal_str("100.25", "USD").amount_decimal
    with pytest.raises(ValueError, match="Invalid Decimal value"):
        Money(" 100.25 ", "USD")


@pytest.mark.parametrize(
    ("money", "expected"),
    [
        (Money(0, "USD"), '{"amount":"-0","currency":"USD"}'),
        (Money.from_decimal_str("0.00", "USD"), '{"amount":"-0.00","currency":"USD"}'),
        (Money.from_decimal_str("1.2500", "USD"), '{"amount":"-1.2500","currency":"USD"}'),
    ],
)
def test_negation_is_the_exact_rust_checked_neg(money: Money, expected: str) -> None:
    negated = -money
    assert negated.to_json() == expected
    assert (-negated).to_json() == money.to_json()


def test_negated_zero_display_matches_wasm() -> None:
    assert str(-Money(0, "USD")) == "USD -0.00"


def test_zero_scalar_division_is_the_rust_validation_error() -> None:
    with _raises_exactly("Validation error: division by zero"):
        _ = Money(10, "USD") / 0


def test_format_with_rounding_parses_serde_names_with_rust_default() -> None:
    m = Money.from_decimal_str("2.345", "USD")
    assert m.format_with(2) == m.format_with(2, True, None, "bankers")
    with _raises_exactly(
        'Validation error: invalid value "BANKERS": unknown variant `BANKERS`, expected one of '
        "`bankers`, `away_from_zero`, `toward_zero`, `floor`, `ceil`"
    ):
        m.format_with(2, True, None, "BANKERS")


def test_year_fractions_with_and_without_context_match_wasm() -> None:
    start, end = dt.date(2024, 3, 1), dt.date(2024, 9, 1)
    assert DayCount.ACT_365L.year_fraction(start, end) == 0.5041095890410959
    assert DayCount.ACT_365L.year_fraction(start, end, frequency="6M") == 0.5027322404371585
    assert DayCount.ACT_365L.signed_year_fraction(end, start, frequency="6M") == -0.5027322404371585
    assert DayCount.ACT_365L.signed_year_fraction(end, start) == -0.5041095890410959
    assert DayCount.BUS_252.year_fraction(start, end, calendar="nyse") == 0.503968253968254
    assert DayCount.BUS_252.signed_year_fraction(end, start, calendar="nyse") == -0.503968253968254


def test_day_count_context_coupon_period_is_validated_in_rust() -> None:
    with _raises_exactly(
        "Validation error: coupon period start must be before end, got start=1970-01-11 end=1970-01-06"
    ):
        DayCountContext(coupon_period=(dt.date(1970, 1, 11), dt.date(1970, 1, 6)))


@pytest.mark.parametrize(("rr_10d", "bf_10d"), [([0.02], None), (None, [0.01])])
def test_fx_delta_vol_wing_pairing_is_the_rust_constructor_check(
    rr_10d: list[float] | None, bf_10d: list[float] | None
) -> None:
    with _raises_exactly("Validation error: rr_10d and bf_10d must both be provided or both omitted"):
        FxDeltaVolSurface("X", [1.0], [0.1], [0.01], [0.005], rr_10d, bf_10d)


def test_vol_cube_node_dicts_use_the_rust_serde_shape() -> None:
    node = {"alpha": 0.02, "beta": 0.5, "rho": -0.2, "nu": 0.3}
    cube = VolCube("C", [1.0], [2.0], [{**node, "shift": None}], [0.03])
    assert cube.interpolation_mode == "vol"
    assert VolCube("C", [1.0], [2.0], [node], [0.03], "total_variance").interpolation_mode == "total_variance"
    with pytest.raises(ValueError, match=r"params_row_major\[0\]: unknown field `shfit`"):
        VolCube("C", [1.0], [2.0], [{**node, "shfit": 0.01}], [0.03])
    with pytest.raises(ValueError, match=r"params_row_major\[0\]: missing field `nu`"):
        VolCube("C", [1.0], [2.0], [{"alpha": 0.02, "beta": 0.5, "rho": -0.2}], [0.03])


def test_hazard_curve_recovery_errors_come_from_the_rust_builder() -> None:
    with _raises_exactly("Validation error: recovery_rate must be a decimal fraction in [0, 1], got 1.5"):
        HazardCurve("HZ", "2025-01-01", [(1.0, 0.01), (5.0, 0.02)], recovery_rate=1.5)
    # Knots are validated before recovery, exactly as in WASM.
    with _raises_exactly("Values must be non-negative"):
        HazardCurve("HZ", "2025-01-01", [(1.0, -0.02)], recovery_rate=1.5)


def test_discount_curve_validation_mode_defaults_in_rust() -> None:
    knots = [(0.0, 1.0), (1.0, 0.98), (5.0, 0.88)]
    default = DiscountCurve("USD-OIS", "2025-01-01", knots)
    explicit = DiscountCurve("USD-OIS", "2025-01-01", knots, validation_mode="market_standard")
    assert default.df(2.5) == explicit.df(2.5)
    with pytest.raises(ValueError, match="forward_floor is only valid"):
        DiscountCurve("USD-OIS", "2025-01-01", knots, forward_floor=-0.01)


def test_fx_matrix_from_dict_uses_the_rust_currency_pair_parser() -> None:
    matrix = FxMatrix.from_dict({"EURUSD": 1.1, "GBP/USD": 1.25})
    assert matrix.rate("GBP", "USD", dt.date(2025, 1, 2)).rate == 1.25
    assert matrix.rate("EUR", "USD", dt.date(2025, 1, 2), "cashflow_date").rate == 1.1
    # Six bytes but not six characters: a ValueError, never a PanicException.
    for key in ["é€x", "EURUS"]:
        with _raises_exactly(f'Validation error: invalid FX pair "{key}": expected "EURUSD" or "EUR/USD"'):
            FxMatrix.from_dict({key: 1.0})
    with _raises_exactly(_unknown_currency("XYZ")):
        FxMatrix.from_dict({"EUR/XYZ": 1.0})


def test_realized_variance_defaults_are_rust_owned() -> None:
    prices = [100.0, 101.0, 100.5, 102.0]
    assert stats.realized_variance(prices) == stats.realized_variance(prices, "close_to_close", 252.0)
    o = [100.0, 101.5, 100.8, 102.0]
    h = [102.0, 103.0, 102.5, 103.5]
    lo = [99.0, 100.2, 99.9, 101.0]
    c = [101.0, 102.0, 101.5, 103.0]
    assert stats.realized_variance_ohlc(o, h, lo, c) == stats.realized_variance_ohlc(o, h, lo, c, "yang_zhang", 252.0)


def test_fx_forward_trade_date_convention_defaults_in_rust_and_accepts_short_codes() -> None:
    args = ("F", "EUR", "USD", dt.date(2024, 1, 15), "3M", 1_000_000.0, "USD-OIS", "EUR-OIS")
    default = FxForward.from_trade_date(*args)
    assert default.maturity == dt.date(2024, 4, 17)
    for name in ["MF", "modified_following"]:
        assert FxForward.from_trade_date(*args, business_day_convention=name).maturity == default.maturity
    with pytest.raises(ValueError, match="invalid business_day_convention: Unknown business day convention"):
        FxForward.from_trade_date(*args, business_day_convention="bogus")
