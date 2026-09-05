"""Financial validation and lifecycle contracts at the Python boundary."""

from datetime import date
import json
import math

import pytest

from finstack_quant.core.dates import Schedule
from finstack_quant.core.market_data import HazardCurve, InflationCurve, MarketContext, PriceCurve


def cpi_curve() -> InflationCurve:
    return InflationCurve("CPI", "2025-01-01", 300.0, [(0.0, 300.0), (1.0, 306.0), (2.0, 312.0)])


@pytest.mark.parametrize(("t1", "t2"), [(1.0, 1.0), (1.0, 0.0), (math.nan, 1.0), (0.0, math.inf)])
def test_invalid_inflation_interval_raises_value_error(t1: float, t2: float) -> None:
    with pytest.raises(ValueError, match="finite t1 < t2"):
        cpi_curve().inflation_rate(t1, t2)


@pytest.mark.parametrize("base_cpi", [math.nan, math.inf, 0.0, -300.0, 301.0])
def test_invalid_base_cpi_is_rejected(base_cpi: float) -> None:
    with pytest.raises(ValueError, match="base_cpi"):
        InflationCurve("CPI", "2025-01-01", base_cpi, [(0.0, 300.0), (1.0, 306.0)])


def test_curve_deserialization_preserves_validation() -> None:
    state = json.loads(cpi_curve().to_json())
    state["base_cpi"] = -300.0
    with pytest.raises(ValueError, match="base_cpi"):
        InflationCurve.from_json(json.dumps(state))


def test_hazard_interpolation_rejects_inconsistent_survival() -> None:
    with pytest.raises(ValueError, match="log-linear"):
        HazardCurve("HZ", "2025-01-01", [(1.0, 0.02), (2.0, 1.0)], recovery_rate=0.4, interp="linear")


@pytest.mark.parametrize("rule", ["weekly", "cds", "imm"])
def test_eom_rejects_incompatible_schedules(rule: str) -> None:
    builder = Schedule.builder("2025-01-01", "2025-12-20").end_of_month(True)
    if rule == "weekly":
        builder = builder.frequency("1W")
    elif rule == "cds":
        builder = builder.cds_imm()
    else:
        builder = builder.imm()
    with pytest.raises(ValueError, match="end-of-month"):
        builder.build()


def test_market_roll_preserves_price_and_cpi_near_origin() -> None:
    cpi = cpi_curve()
    price = PriceCurve("PRICE", "2025-01-01", [(0.0, 100.0), (1.0, 110.0), (2.0, 120.0)])
    market = MarketContext()
    market.insert(cpi)
    market.insert(price)
    rolled = market.roll_forward(183)
    dt = (date(2025, 7, 3) - date(2025, 1, 1)).days / 365.0
    assert rolled.get_inflation_curve("CPI").cpi(1e-8) == pytest.approx(cpi.cpi(dt + 1e-8))
    assert rolled.get_price_curve("PRICE").price(1e-8) == pytest.approx(price.price(dt + 1e-8))
