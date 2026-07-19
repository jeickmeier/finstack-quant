"""Concurrent GIL-release coverage for CPU-heavy portfolio bindings."""

from __future__ import annotations

from collections.abc import Callable
from datetime import date, timedelta
import json
import math
import sys
import threading
import time

import pytest

from finstack_quant.core.market_data import DiscountCurve, MarketContext
from finstack_quant.portfolio import (
    Portfolio,
    aggregate_metrics,
    amihud_illiquidity,
    attribute_portfolio_pnl,
    build_portfolio_from_spec,
    carino_link,
    compute_factor_sensitivities,
    decompose_factor_risk,
    factor_stress,
    kyle_lambda,
    position_what_if,
    replay_portfolio,
    roll_effective_spread,
    twrr_linked,
    value_portfolio,
)

AS_OF = date(2025, 1, 15)


def _portfolio_spec_json(position_count: int) -> str:
    positions = []
    for index in range(position_count):
        instrument_id = f"DEP-{index}"
        positions.append({
            "position_id": f"POS-{index}",
            "entity_id": "FUND",
            "instrument_id": instrument_id,
            "instrument_spec": {
                "type": "deposit",
                "spec": {
                    "id": instrument_id,
                    "notional": {
                        "amount": 1_000_000.0 + index,
                        "currency": "USD",
                    },
                    "start_date": AS_OF.isoformat(),
                    "maturity": "2025-07-15",
                    "day_count": "Act360",
                    "quote_rate": 0.04,
                    "discount_curve_id": "USD-OIS",
                    "attributes": {},
                },
            },
            "quantity": 1.0,
            "unit": "units",
        })
    return json.dumps({
        "id": f"GIL-{position_count}",
        "as_of": AS_OF.isoformat(),
        "base_ccy": "USD",
        "entities": {"FUND": {"id": "FUND"}},
        "positions": positions,
    })


def _market(as_of: date = AS_OF, rate: float = 0.04) -> MarketContext:
    knots = [(year, math.exp(-rate * year)) for year in (0.0, 0.5, 1.0, 2.0)]
    return MarketContext().insert(DiscountCurve("USD-OIS", as_of, knots))


def _replay_snapshots_json(snapshot_count: int) -> str:
    snapshots = []
    for index in range(snapshot_count):
        snapshot_date = AS_OF + timedelta(days=index)
        snapshots.append({
            "date": snapshot_date.isoformat(),
            "market": json.loads(_market(snapshot_date, 0.04 + index * 0.0001).to_json()),
        })
    return json.dumps(snapshots)


def _factor_model_config_json() -> str:
    return json.dumps({
        "factors": [
            {
                "id": "usd_rates",
                "factor_type": "Rates",
                "market_mapping": {
                    "CurveParallel": {
                        "curve_ids": ["USD-OIS"],
                        "units": "rate_bp",
                    }
                },
                "description": "Parallel USD rates shift",
            }
        ],
        "covariance": {
            "factor_ids": ["usd_rates"],
            "n": 1,
            "data": [0.0001],
        },
        "matching": {
            "MappingTable": [
                {
                    "dependency_filter": {},
                    "attribute_filter": {},
                    "factor_id": "usd_rates",
                }
            ]
        },
        "pricing_mode": "full_repricing",
        "risk_measure": "variance",
    })


def _sensitivity_positions_json(position_count: int) -> str:
    spec = json.loads(_portfolio_spec_json(position_count))
    return json.dumps([
        {
            "id": position["position_id"],
            "instrument": position["instrument_spec"],
            "weight": position["quantity"],
        }
        for position in spec["positions"]
    ])


def _assert_releases_gil[T](call: Callable[[], T]) -> T:
    """Run one native call while a Python heartbeat requires the GIL.

    A long thread-switch interval prevents ordinary Python bytecode boundaries
    around the call from scheduling the heartbeat. The heartbeat itself yields
    explicitly, so it can make progress only while the native binding releases
    the GIL and cannot starve the calling thread when Rust finishes.
    """
    started = threading.Event()
    stop = threading.Event()
    progress = [0]

    def heartbeat() -> None:
        started.set()
        while not stop.is_set():
            progress[0] += 1
            time.sleep(0)

    worker = threading.Thread(target=heartbeat, daemon=True)
    worker.start()
    assert started.wait(timeout=1.0)

    old_interval = sys.getswitchinterval()
    sys.setswitchinterval(1.0)
    before = progress[0]
    try:
        result = call()
        after = progress[0]
    finally:
        sys.setswitchinterval(old_interval)
        stop.set()
        worker.join(timeout=1.0)

    assert after > before, "background Python thread made no progress during native work"
    return result


def test_portfolio_from_spec_releases_gil_during_json_build() -> None:
    portfolio_json = _portfolio_spec_json(10_000)
    portfolio = _assert_releases_gil(lambda: Portfolio.from_spec(portfolio_json))

    assert len(portfolio) == 10_000


def test_portfolio_from_spec_detached_parse_preserves_value_error_mapping() -> None:
    with pytest.raises(ValueError, match="EOF"):
        Portfolio.from_spec('{"positions":')


def test_compatibility_build_spec_path_releases_gil() -> None:
    portfolio_json = _portfolio_spec_json(10_000)
    round_tripped = _assert_releases_gil(lambda: build_portfolio_from_spec(portfolio_json))

    assert len(json.loads(round_tripped)["positions"]) == 10_000


def test_value_portfolio_json_inputs_release_gil_and_preserve_result() -> None:
    portfolio_json = _portfolio_spec_json(3_000)
    market_json = _market().to_json()

    result_json = _assert_releases_gil(lambda: value_portfolio(portfolio_json, market_json, metrics=[]))
    result = json.loads(result_json)

    assert len(result["position_values"]) == 3_000
    assert result["as_of"] == AS_OF.isoformat()


def test_raw_valuation_metric_aggregation_releases_gil() -> None:
    portfolio = Portfolio.from_spec(_portfolio_spec_json(3_000))
    market = _market()
    valuation_json = value_portfolio(portfolio, market, metrics=[])

    metrics_json = _assert_releases_gil(
        lambda: aggregate_metrics(
            valuation_json,
            "USD",
            market,
            AS_OF.isoformat(),
        )
    )

    by_position = json.loads(metrics_json)["by_position"]
    assert len(by_position) == 3_000
    assert all(entry["metrics"] == {} for entry in by_position.values())


def test_attribution_nested_serialization_releases_gil() -> None:
    portfolio = Portfolio.from_spec(_portfolio_spec_json(256))
    next_date = AS_OF + timedelta(days=1)
    result = attribute_portfolio_pnl(
        portfolio,
        _market(AS_OF, 0.04),
        _market(next_date, 0.041),
        AS_OF.isoformat(),
        next_date.isoformat(),
        "Parallel",
    )

    by_position_json = _assert_releases_gil(result.by_position_json)

    assert len(json.loads(by_position_json)) == 256


def test_replay_parse_compute_and_serialize_release_gil() -> None:
    portfolio = Portfolio.from_spec(_portfolio_spec_json(500))
    snapshots_json = _replay_snapshots_json(20)

    result_json = _assert_releases_gil(lambda: replay_portfolio(portfolio, snapshots_json, '{"mode":"PvOnly"}'))
    result = json.loads(result_json)

    assert isinstance(result_json, str)
    assert len(result["steps"]) == 20
    assert result["summary"]["num_steps"] == 20


def test_replay_detached_parse_preserves_value_error_mapping() -> None:
    portfolio = Portfolio.from_spec(_portfolio_spec_json(1))

    with pytest.raises(ValueError, match="invalid snapshots JSON"):
        replay_portfolio(portfolio, "{}", '{"mode":"PvOnly"}')


def test_factor_stress_releases_gil_and_returns_position_results() -> None:
    portfolio = Portfolio.from_spec(_portfolio_spec_json(256))
    market = _market()
    config_json = _factor_model_config_json()

    result = _assert_releases_gil(
        lambda: factor_stress(
            portfolio,
            market,
            config_json,
            AS_OF.isoformat(),
            [("usd_rates", 1.0)],
        )
    )

    assert len(result.position_pnl) == 256
    assert math.isfinite(result.total_pnl)
    assert result.stressed_decomposition.total_risk >= 0.0

    result_json = _assert_releases_gil(result.to_json)
    round_tripped = _assert_releases_gil(lambda: type(result).from_json(result_json))
    assert round_tripped.total_pnl == pytest.approx(result.total_pnl)


def test_factor_stress_rejects_invalid_config_after_detached_parse() -> None:
    portfolio = Portfolio.from_spec(_portfolio_spec_json(1))

    with pytest.raises(ValueError, match="EOF"):
        factor_stress(
            portfolio,
            _market(),
            '{"factors":',
            AS_OF.isoformat(),
            [("usd_rates", 1.0)],
        )


def test_position_what_if_uses_combined_baseline_analysis() -> None:
    portfolio = Portfolio.from_spec(_portfolio_spec_json(2))
    result = position_what_if(
        portfolio,
        _market(),
        _factor_model_config_json(),
        AS_OF.isoformat(),
        [{"kind": "remove", "position_id": "POS-1"}],
    )

    assert result.before.total_risk > result.after.total_risk
    assert len(result.delta) == 1


def test_large_twrr_parse_link_and_serialize_release_gil() -> None:
    returns_json = json.dumps([0.000001] * 200_000)

    result_json = _assert_releases_gil(lambda: twrr_linked(returns_json, 2.0))
    assert result_json is not None
    result = json.loads(result_json)

    assert result["num_periods"] == 200_000
    assert result["cumulative"] > 0.0


def test_large_carino_parse_compute_and_serialize_release_gil() -> None:
    period = [
        {
            "sector": "A",
            "portfolio_weight": 0.6,
            "benchmark_weight": 0.4,
            "portfolio_return": 0.000002,
            "benchmark_return": 0.000001,
        },
        {
            "sector": "B",
            "portfolio_weight": 0.4,
            "benchmark_weight": 0.6,
            "portfolio_return": -0.000001,
            "benchmark_return": 0.0,
        },
    ]
    periods_json = json.dumps([period] * 10_000)

    result_json = _assert_releases_gil(lambda: carino_link(periods_json))
    result = json.loads(result_json)

    assert len(result["periods"]) == 10_000
    assert [entry["sector"] for entry in result["linked_sectors"]] == ["A", "B"]


def test_sensitivity_result_conversion_and_decomposition_release_gil() -> None:
    factor_config = json.loads(_factor_model_config_json())
    positions_json = _sensitivity_positions_json(2_000)
    factors_json = json.dumps(factor_config["factors"])
    covariance_json = json.dumps(factor_config["covariance"])
    market = _market()
    matrix = _assert_releases_gil(
        lambda: compute_factor_sensitivities(
            positions_json,
            factors_json,
            market,
            AS_OF.isoformat(),
        )
    )
    decomposition = _assert_releases_gil(lambda: decompose_factor_risk(matrix, covariance_json))

    assert matrix.n_positions == 2_000
    assert matrix.n_factors == 1
    assert len(matrix.position_ids) == 2_000
    assert math.isfinite(decomposition.total_risk)


def test_vector_liquidity_estimators_release_gil() -> None:
    returns = [0.001, -0.001] * 500_000
    volumes = [1_000_000.0] * len(returns)

    roll = _assert_releases_gil(lambda: roll_effective_spread(returns))
    amihud = _assert_releases_gil(lambda: amihud_illiquidity(returns, volumes))
    kyle = _assert_releases_gil(lambda: kyle_lambda(volumes, returns))

    assert roll is not None
    assert roll > 0.0
    assert amihud is not None
    assert amihud > 0.0
    assert kyle is None
