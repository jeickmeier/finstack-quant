"""Inputs whose defaults and validation belong to Rust, not to the Python binding.

Each case was a binding that invented a default, skipped a Rust check, or
accepted on one path what Rust rejects on another.
"""

from __future__ import annotations

import json
import math
import pickle

import pytest

from finstack_quant import statements
from finstack_quant.calibration import VolQuote
from finstack_quant.models import liquidity, volatility
from finstack_quant.statements_analytics import GoalSeekResult, TerminalValueSpec, evaluate_dcf


def _dcf_model() -> statements.FinancialModelSpec:
    from finstack_quant.core.money import Money

    builder = statements.ModelBuilder("dcf")
    builder.periods("2025..2026")
    builder.value_money("ufcf", [("2025", Money(100.0, "USD")), ("2026", Money(110.0, "USD"))])
    builder.with_meta("currency", '"USD"')
    return builder.build()


def test_exit_multiple_requires_terminal_metric_and_rejects_a_zero_placeholder() -> None:
    with pytest.raises(TypeError):
        TerminalValueSpec.exit_multiple(9.0)  # type: ignore[call-arg]
    priced = evaluate_dcf(_dcf_model(), 0.10, TerminalValueSpec.exit_multiple(9.0, 120.0), net_debt_override=0.0)
    assert priced.terminal_value_pv.amount > 0.0
    # Without exit_multiple_metric_node a zero metric would price a zero terminal value.
    with pytest.raises(ValueError, match="terminal_metric"):
        evaluate_dcf(_dcf_model(), 0.10, TerminalValueSpec.exit_multiple(9.0, 0.0), net_debt_override=0.0)


@pytest.mark.parametrize(
    "build",
    [
        lambda **kw: VolQuote.swaption_vol("S", "2027-05-08", "2032-05-08", 0.04, 0.20, **kw),
        lambda **kw: VolQuote.cap_floor_vol("C", "2027-05-08", 0.04, 0.20, **kw),
    ],
)
def test_vol_quote_type_is_required(build) -> None:  # noqa: ANN001
    # The quote type decides whether 0.20 is a 20% Black vol or a 2000bp normal vol.
    with pytest.raises(TypeError):
        build()
    black = json.loads(build(quote_type="black_lognormal").to_json())
    assert next(iter(black.values()))["quote_type"] == "black_lognormal"


def test_vol_quote_optional_fields_take_the_rust_authoring_defaults() -> None:
    swaption = json.loads(VolQuote.swaption_vol("S", "2027-05-08", "2032-05-08", 0.04, 0.0072, "normal").to_json())
    assert swaption["swaption_vol"]["convention"] == "USD"
    cap = json.loads(VolQuote.cap_floor_vol("C", "2027-05-08", 0.04, 0.0072, quote_type="normal").to_json())
    assert cap["cap_floor_vol"]["is_cap"] is True


def test_liquidity_profile_validates_observation_days_in_rust() -> None:
    args = ("X", 100.0, 99.9, 100.1, 1e6, 1000.0, 0.1)
    with pytest.raises(ValueError, match="observation_days"):
        liquidity.LiquidityProfile(*args, observation_days=0)
    default = liquidity.LiquidityProfile(*args)
    assert default.observation_days == 20
    assert default.spread_volatility_kind == "relative"
    explicit = liquidity.LiquidityProfile(*args, spread_volatility_kind="absolute", observation_days=60)
    assert pickle.loads(pickle.dumps(explicit)) == explicit  # noqa: S301


def test_arbitrage_grid_checks_default_to_the_rust_tolerance() -> None:
    strikes = [80.0, 90.0, 100.0, 110.0, 120.0]
    expiries = [0.5, 1.0]
    # Total variance at K=100 falls by 5e-8 between the two expiries: a minor
    # calendar-spread arbitrage that a 1e-6 tolerance would hide.
    rows = [[0.30, 0.25, 0.20, 0.25, 0.30], [0.29, 0.245, math.sqrt(0.02 - 5e-8), 0.245, 0.29]]
    forwards = [100.0, 100.0]
    default = volatility.check_calendar_spread_grid(strikes, expiries, rows, forwards)
    explicit = volatility.check_calendar_spread_grid(strikes, expiries, rows, forwards, tolerance=1e-10)
    assert len(default) == len(explicit) == 1


def test_goal_seek_result_from_json_validates_the_embedded_model() -> None:
    empty_model = {"id": "m", "periods": [], "nodes": {}, "schema_version": 1}
    with pytest.raises(ValueError, match="at least one period"):
        GoalSeekResult.from_json(json.dumps({"solved_value": 1.0, "model": empty_model}))
    assert GoalSeekResult.from_json('{"solved_value": 2.5, "model": null}').solved_value == 2.5


def test_per_position_metric_json_rejects_unknown_metric_ids() -> None:
    from finstack_quant.portfolio import PerPositionMetric

    assert json.loads(PerPositionMetric.from_json('{"metric":"dv01"}').to_json()) == {"metric": "dv01"}
    # An unknown id used to deserialize as a custom metric and solve to a zero objective.
    for bad in ("bogus", "bucketed_dv01::USD-OIS::10y"):
        with pytest.raises(ValueError, match="Unknown metric"):
            PerPositionMetric.from_json(json.dumps({"metric": bad}))
        with pytest.raises(ValueError, match="Unknown metric"):
            PerPositionMetric.metric(bad)
