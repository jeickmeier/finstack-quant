"""Typed covenants surface: Python results against the cross-host goldens.

``finstack-quant-wasm/tests/facade/covenants_parity.test.mjs`` computes every
case below through the WASM facade and pins the values in
``finstack-quant-wasm/tests/facade/golden/covenants_parity.json``. The same
inputs go through the Python bindings here and must give the same JSON.
Floats compare to 1e-12 relative, the native-vs-wasm32 allowance of
INVARIANTS.md §2.1.
"""

from __future__ import annotations

from collections.abc import Callable
import json
import math
from pathlib import Path
import re
from typing import Any

import pandas as pd
import pytest

from finstack_quant import covenants
from finstack_quant.core.dates import Tenor
from finstack_quant.covenants import (
    Covenant,
    CovenantConsequence,
    CovenantEngine,
    CovenantForecastConfig,
    CovenantSpec,
    CovenantType,
    CovenantWaiver,
    SpringingCondition,
    ThresholdSchedule,
)

GOLDEN: dict[str, Any] = json.loads(
    (Path(__file__).parents[2] / "finstack-quant-wasm/tests/facade/golden/covenants_parity.json").read_text()
)

SERIES = pd.DataFrame(
    {"debt_to_ebitda": [4.8, 4.0, 4.4], "ebitda": [100.0, 110.0, 105.0]},
    index=["2026-06-30", "2026-03-31", "2026-09-30"],
)


def wire(value: Any) -> Any:
    """The JSON a WASM caller sees for a Python binding value."""
    if hasattr(value, "to_json"):
        return json.loads(value.to_json())
    if isinstance(value, dict):
        return {key: wire(item) for key, item in value.items()}
    if isinstance(value, (list, tuple)):
        return [wire(item) for item in value]
    return value


DECIMAL = re.compile(r"-?\d+(\.\d+)?")


def assert_same(actual: Any, expected: Any, path: str = "$") -> None:
    """Structural equality with a 1e-12 relative allowance on floats."""
    if isinstance(expected, dict):
        assert isinstance(actual, dict), path
        assert sorted(actual) == sorted(expected), path
        for key in expected:
            assert_same(actual[key], expected[key], f"{path}.{key}")
    elif isinstance(expected, list):
        assert isinstance(actual, list), path
        assert len(actual) == len(expected), path
        for index, (left, right) in enumerate(zip(actual, expected, strict=True)):
            assert_same(left, right, f"{path}[{index}]")
    elif isinstance(expected, str) and isinstance(actual, str) and DECIMAL.fullmatch(expected):
        # Exact-decimal text of an f64 result (``Money.amount``): same allowance as a float.
        assert DECIMAL.fullmatch(actual), path
        assert math.isclose(float(actual), float(expected), rel_tol=1e-12, abs_tol=1e-12), (
            f"{path}: {actual} != {expected}"
        )
    elif isinstance(expected, bool) or expected is None or isinstance(expected, str):
        assert actual == expected, path
    else:
        assert isinstance(actual, (int, float)), path
        assert not isinstance(actual, bool), path
        assert math.isclose(actual, expected, rel_tol=1e-12, abs_tol=1e-12), f"{path}: {actual} != {expected}"


def leverage() -> Covenant:
    return Covenant(CovenantType.max_debt_to_ebitda(4.5), Tenor.parse("3M"), "max_leverage")


def leverage_spec() -> CovenantSpec:
    return CovenantSpec(leverage(), "debt_to_ebitda")


def _covenant_types() -> Any:
    return wire([
        CovenantType.max_debt_to_ebitda(4.5),
        CovenantType.min_interest_coverage(2.0),
        CovenantType.min_fixed_charge_coverage(1.2),
        CovenantType.max_total_leverage(6.0),
        CovenantType.max_senior_leverage(4.0),
        CovenantType.min_asset_coverage(1.5),
        CovenantType.negative("no additional senior debt"),
        CovenantType.affirmative("deliver audited financials within 90 days"),
        CovenantType.custom("net_working_capital", "minimum", 25.0),
        CovenantType.custom("capex_to_sales", "maximum", 0.08),
        CovenantType.basket("restricted_payments", 5_000_000),
        CovenantType.min_dscr(1.25),
        CovenantType.max_net_debt_to_ebitda(3.5),
        CovenantType.max_capex(10_000_000),
        CovenantType.min_liquidity(2_500_000),
    ])


def _consequences() -> Any:
    return wire([
        CovenantConsequence.default(),
        CovenantConsequence.rate_increase(200),
        CovenantConsequence.cash_sweep(0.5),
        CovenantConsequence.block_distributions(),
        CovenantConsequence.require_collateral("first lien on receivables"),
        CovenantConsequence.accelerate_maturity("2027-12-31"),
    ])


def _covenant_modifiers() -> Any:
    base = leverage()
    return wire([
        base,
        base.with_cure_period(30),
        base.with_cure_period(30).with_cure_period(None),
        base.with_consequence(CovenantConsequence.rate_increase(200)),
        base.with_scope("incurrence"),
        base.with_springing_condition(SpringingCondition("revolver_utilization", "minimum", 0.35)),
    ])


def _spec_modifiers() -> Any:
    net_leverage = CovenantSpec(
        Covenant(CovenantType.max_net_debt_to_ebitda(3.5), Tenor.parse("3M"), "max_net_leverage"), "net_debt"
    )
    schedule = ThresholdSchedule([("2026-01-01", 4.5), ("2026-07-01", 4.0)])
    return {
        "with_metric": wire([leverage_spec(), net_leverage]),
        "with_denominator_metric": wire(net_leverage.with_denominator_metric("adjusted_ebitda")),
        "with_threshold_schedule": wire(leverage_spec().with_threshold_schedule(schedule)),
        "threshold_for": [
            schedule.threshold_for(date) for date in ("2025-12-31", "2026-01-01", "2026-06-30", "2026-07-01")
        ],
    }


def _engine_evaluate() -> Any:
    engine = CovenantEngine.from_specs(covenants.cov_lite(7.0, 4.5))
    return {
        "pass": wire(engine.evaluate({"total_leverage": 5.0, "senior_leverage": 3.0}, "2026-03-31")),
        "breach": wire(engine.evaluate(json.dumps({"total_leverage": 7.5, "senior_leverage": 3.0}), "2026-03-31")),
        "specs": wire(engine.specs),
        "waivers": wire(engine.waivers),
        "breach_history": wire(engine.breach_history),
    }


def _engine_track() -> Any:
    engine = CovenantEngine()
    engine.add_spec(CovenantSpec(leverage().with_cure_period(30), "debt_to_ebitda"))
    engine.validate()
    breached = wire(engine.evaluate_and_track({"debt_to_ebitda": 5.0}, "2026-03-31", "maintenance"))
    history = wire(engine.breach_history)
    engine.add_waiver(
        CovenantWaiver("max_leverage", "2026-06-01", amended_threshold=5.5, description="amendment no. 1")
    )
    amended = wire(engine.evaluate({"debt_to_ebitda": 5.0}, "2026-06-30"))
    restored = CovenantEngine.from_json(engine.to_json())
    return {
        "breached": breached,
        "history": history,
        "amended": amended,
        "waivers": wire(restored.waivers),
        "engine": wire(restored),
    }


def _engine_evaluate_series() -> Any:
    """Group the long frame back into the ``{as_of, reports}`` rows Rust (and WASM) return."""
    engine = CovenantEngine.from_specs([leverage_spec()])
    frame = engine.evaluate_series(SERIES)
    rows: list[dict[str, Any]] = []
    for record in frame.to_dict("records"):
        if not rows or rows[-1]["as_of"] != record["as_of"]:
            rows.append({"as_of": record["as_of"], "reports": {}})
        rows[-1]["reports"][record["covenant"]] = {
            key: record[key] for key in ("covenant_type", "passed", "actual_value", "threshold", "headroom", "details")
        }
    return rows


def _forecast() -> Any:
    stochastic = CovenantForecastConfig(stochastic=True, volatility=0.2, reference_date="2025-12-31")
    engine = CovenantEngine.from_specs([leverage_spec()])
    return {
        "deterministic": wire(covenants.forecast_covenant(leverage_spec(), SERIES)),
        "stochastic": wire(covenants.forecast_covenant(leverage_spec(), SERIES, stochastic)),
        "monte_carlo": wire(
            covenants.forecast_covenant(
                leverage_spec(),
                SERIES,
                CovenantForecastConfig(
                    stochastic=True,
                    volatility=0.2,
                    num_paths=2000,
                    random_seed=7,
                    antithetic=True,
                    reference_date="2025-12-31",
                ),
            )
        ),
        "breaches": wire(covenants.forecast_breaches(engine, SERIES)),
        "breaches_stochastic": wire(
            covenants.forecast_breaches(
                engine,
                SERIES,
                CovenantForecastConfig(
                    stochastic=True,
                    volatility=0.2,
                    reference_date="2025-12-31",
                    breach_probability_threshold=0.2,
                ),
            )
        ),
        "with_scope": wire(CovenantForecastConfig(stochastic=True, volatility=0.2).with_scope("incurrence")),
    }


CASES: dict[str, Callable[[], Any]] = {
    "covenant_types": _covenant_types,
    "consequences": _consequences,
    "covenant_modifiers": _covenant_modifiers,
    "spec_modifiers": _spec_modifiers,
    "templates": lambda: wire([
        covenants.lbo_standard(5.0, 1.5, 1.2, 10_000_000),
        covenants.cov_lite(7.0, 4.5),
        covenants.real_estate(1.25, 0.08, 0.65),
        covenants.project_finance(1.3, 1.15, 5_000_000, 4.0),
    ]),
    "engine_evaluate": _engine_evaluate,
    "engine_track_waive_and_round_trip": _engine_track,
    "engine_evaluate_series": _engine_evaluate_series,
    "forecast": _forecast,
}


def test_python_cases_cover_the_golden_file() -> None:
    assert sorted(CASES) == sorted(GOLDEN)


@pytest.mark.parametrize("name", sorted(set(CASES) - {"engine_evaluate_series"}))
def test_python_matches_the_wasm_golden(name: str) -> None:
    assert_same(CASES[name](), GOLDEN[name])


def test_evaluate_series_frame_holds_the_rust_reports() -> None:
    """Python flattens Rust ``evaluate_series`` rows; the flattened fields must match WASM's."""
    expected = [
        {
            "as_of": row["as_of"],
            "reports": {
                key: {
                    field: report.get(field)
                    for field in ("covenant_type", "passed", "actual_value", "threshold", "headroom", "details")
                }
                for key, report in row["reports"].items()
            },
        }
        for row in GOLDEN["engine_evaluate_series"]
    ]
    assert_same(_engine_evaluate_series(), expected)


def test_series_validation_is_rust_owned() -> None:
    spec = leverage_spec()
    with pytest.raises(ValueError, match="metric series must contain at least one dated row"):
        covenants.forecast_covenant(spec, SERIES.iloc[0:0])
    with pytest.raises(ValueError, match="metric series contains duplicate date 2026-06-30"):
        covenants.forecast_covenant(spec, pd.concat([SERIES.iloc[[0]], SERIES.iloc[[0]]]))
    with pytest.raises(KeyError, match="debt_to_ebitda"):
        covenants.forecast_covenant(spec, SERIES[["ebitda"]])


def test_forecast_config_json_fields_default_in_rust() -> None:
    assert CovenantForecastConfig.from_json("{}") == CovenantForecastConfig()
    partial = CovenantForecastConfig.from_json('{"stochastic": true, "volatility": 0.2}')
    assert partial == CovenantForecastConfig(stochastic=True, volatility=0.2)
    with pytest.raises(ValueError, match="unknown field"):
        CovenantForecastConfig.from_json('{"paths": 10}')
