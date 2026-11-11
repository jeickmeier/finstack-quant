"""Pytest-benchmark controls for selected finstack-quant Python binding domains.

Run the complete suite against an optimized extension::

    mise run python-bench

Run with pytest-benchmark::

    uv run pytest finstack-quant-py/benchmarks/bench_bindings.py -m "perf and not slow" --benchmark-only

Run the slow release-scale controls as well::

    uv run pytest finstack-quant-py/benchmarks/bench_bindings.py -m perf --benchmark-only

Run with verbose output::

    uv run pytest finstack-quant-py/benchmarks/bench_bindings.py -m perf --benchmark-only -v

Save and compare a portfolio baseline::

    scripts/materialization_bench.sh python-baseline
    scripts/materialization_bench.sh python-compare
"""

from __future__ import annotations

from datetime import date, timedelta
from functools import lru_cache
from itertools import accumulate
import json
import math
import os
from pathlib import Path
import shutil
import subprocess  # nosec B404 # fixed Cargo benchmark command, no shell

import numpy as np
import pytest

from finstack_quant.analytics import Performance, PerformancePanel
from finstack_quant.core.currency import Currency
from finstack_quant.core.dates import DayCount, Tenor
from finstack_quant.core.market_data import DiscountCurve, ForwardCurve, FxMatrix, MarketContext
from finstack_quant.core.math import linalg, stats
from finstack_quant.core.money import Money
from finstack_quant.core.types import Rate
from finstack_quant.margin import (
    CsaSpec,
    FundingConfig,
    MarginUtilization,
    NettingSetId,
    VmCalculator,
)
from finstack_quant.models import bs_price
from finstack_quant.models.correlation import (
    CopulaSpec,
    CorrelatedBernoulli,
    LatentFactorSpec,
    LatentSingleFactor,
    RecoverySpec,
    correlation_bounds,
    validate_correlation_matrix,
)
from finstack_quant.models.factor.risk import build_stress_attribution, position_risk_report
from finstack_quant.models.monte_carlo import (
    EuropeanPricer,
    LsmcPricer,
    PathDependentPricer,
)
from finstack_quant.portfolio import (
    InstrumentArtifactCache,
    Portfolio,
    PortfolioValuation,
    aggregate_full_cashflows,
    aggregate_metrics,
    attribute_portfolio_pnl,
    parse_portfolio_spec_json,
    replay_portfolio,
    scenario_pnl,
    scenario_pnl_batch,
    value_portfolio,
)
from finstack_quant.scenarios import (
    CurveKind,
    OperationSpec,
    ScenarioSpec,
    build_from_template,
    compose_scenarios,
    list_builtin_templates,
    list_template_components,
    validate_scenario_spec,
)
from finstack_quant.statements import (
    Evaluator,
    FinancialModelSpec,
    ModelBuilder,
    NormalizationConfig,
    normalize,
    parse_and_compile,
    parse_formula,
)
from finstack_quant.statements_analytics import (
    DependencyTracer,
    backtest_forecast,
    evaluate_scenario_set,
    explain_formula,
    goal_seek,
    run_sensitivity,
    run_variance,
)
from finstack_quant.valuations import ValuationResult
from finstack_quant.valuations.instruments import list_standard_metrics, price_instrument, validate_instrument_json

from . import _ccm


def _ccm_bench(benchmark, operation, validate, *, parameters, boundary):
    return _ccm._run_checked(
        benchmark,
        operation,
        validate,
        parameters=parameters,
        boundary=boundary,
        identity=_ccm._runtime_identity(),
    )


# Shared data

RETURNS_10K: list[float] = [0.0004 + (i % 17) * 1e-5 for i in range(10_000)]
RETURNS_10K_ALT: list[float] = [0.0003 + (i % 13) * 1.2e-5 for i in range(10_000)]
PRICES_10K: list[float] = list(accumulate(RETURNS_10K, lambda p, r: p * (1.0 + r), initial=100.0))
PRICES_10K_ALT: list[float] = list(accumulate(RETURNS_10K_ALT, lambda p, r: p * (1.0 + r), initial=100.0))

DATES_252 = [date(2024, 1, 1) + timedelta(days=i) for i in range(252)]
DATES_10K = [date(2000, 1, 1) + timedelta(days=i) for i in range(10_000)]

_PERFORMANCE_10K = Performance.from_panel(
    PerformancePanel(
        dates=DATES_10K,
        series=[PRICES_10K[:10_000], PRICES_10K_ALT[:10_000]],
        kind="prices",
        ticker_names=["ASSET", "BENCH"],
    )
)

DATA_10K: list[float] = [float(i) * 0.01 for i in range(10_000)]

_REPO_ROOT = Path(__file__).resolve().parents[2]
_MATERIALIZATION_FIXTURE_DIR = _REPO_ROOT / "target" / "materialization-benchmarks"
_CARGO = shutil.which("cargo")


def _materialization_sample_count() -> int:
    raw = os.environ.get("FQ_MATERIALIZATION_P95_SAMPLES", "100")
    try:
        samples = int(raw)
    except ValueError as error:
        raise ValueError(f"FQ_MATERIALIZATION_P95_SAMPLES must be a finite integer, got {raw!r}") from error
    minimum = 1 if os.environ.get("FQ_MATERIALIZATION_SMOKE") == "1" else 100
    if samples < minimum:
        raise ValueError(f"FQ_MATERIALIZATION_P95_SAMPLES must be at least {minimum}, got {samples}")
    return samples


_MATERIALIZATION_BENCH_ROUNDS = _materialization_sample_count()


@lru_cache(maxsize=2)
def _materialization_fixture_bytes(name: str) -> bytes:
    _regenerate_materialization_fixtures()
    fixture_path = _MATERIALIZATION_FIXTURE_DIR / f"{name}.json"
    if not fixture_path.exists():
        raise FileNotFoundError(
            "materialization benchmark fixtures are missing; run "
            "`cargo run --release -p finstack-quant-portfolio --example "
            "materialization_fixtures -- target/materialization-benchmarks`"
        )
    return fixture_path.read_bytes()


@lru_cache(maxsize=1)
def _regenerate_materialization_fixtures() -> None:
    if _CARGO is None:
        raise RuntimeError("cargo is required to regenerate materialization fixtures")
    subprocess.run(  # nosec B603 # noqa: S603 -- resolved executable and fixed argument vector
        [
            _CARGO,
            "run",
            "--release",
            "--quiet",
            "-p",
            "finstack-quant-portfolio",
            "--example",
            "materialization_fixtures",
            "--",
            str(_MATERIALIZATION_FIXTURE_DIR),
        ],
        cwd=_REPO_ROOT,
        check=True,
    )


def _record_materialization_report(benchmark, report) -> None:
    benchmark.extra_info["input_bytes"] = report.input_bytes
    benchmark.extra_info["unique_artifacts"] = report.unique_instruments
    benchmark.extra_info["positions"] = report.positions
    benchmark.extra_info["dependencies"] = report.dependencies
    benchmark.extra_info["cache_hits"] = report.cache_hits
    benchmark.extra_info["phase_nanos"] = report.phase_nanos


SPD_5X5: list[list[float]] = [
    [4.0, 2.0, 1.0, 0.5, 0.25],
    [2.0, 5.0, 2.0, 1.0, 0.5],
    [1.0, 2.0, 6.0, 2.0, 1.0],
    [0.5, 1.0, 2.0, 7.0, 2.0],
    [0.25, 0.5, 1.0, 2.0, 8.0],
]

CORR_5X5_FLAT: list[float] = [
    1.0,
    0.3,
    0.2,
    0.1,
    0.05,
    0.3,
    1.0,
    0.3,
    0.2,
    0.1,
    0.2,
    0.3,
    1.0,
    0.3,
    0.2,
    0.1,
    0.2,
    0.3,
    1.0,
    0.3,
    0.05,
    0.1,
    0.2,
    0.3,
    1.0,
]

DEPOSIT_INSTRUMENT_JSON = json.dumps({
    "schema": "finstack_quant.instrument/1",
    "instrument": {
        "type": "deposit",
        "spec": {
            "id": "DEP-1",
            "notional": {"amount": "1000000", "currency": "USD"},
            "start_date": "2025-01-15",
            "maturity": "2025-06-15",
            "day_count": "act_360",
            "fixed_rate": "0.05",
            "discount_curve_id": "USD-OIS",
            "attributes": {},
        },
    },
})

PORTFOLIO_SPEC_JSON = json.dumps({
    "id": "bench-portfolio",
    "as_of": "2025-01-15",
    "base_currency": "USD",
    "entities": {"ENTITY-1": {"id": "ENTITY-1"}},
    "positions": [
        {
            "position_id": "POS-1",
            "entity_id": "ENTITY-1",
            "instrument_id": "DEP-1",
            "instrument_spec": json.loads(DEPOSIT_INSTRUMENT_JSON)["instrument"],
            "quantity": 1.0,
            "unit": "units",
        }
    ],
})


def _build_portfolio_spec_json(n_positions: int) -> str:
    """Build a `PortfolioSpec` JSON string with ``n_positions`` deposits."""
    positions = []
    for i in range(n_positions):
        pid = f"POS-{i}"
        instr = f"DEP-{i}"
        positions.append({
            "position_id": pid,
            "entity_id": "ENTITY-1",
            "instrument_id": instr,
            "instrument_spec": {
                "type": "deposit",
                "spec": {
                    "id": instr,
                    "notional": {"amount": str(1_000_000 + i), "currency": "USD"},
                    "start_date": "2025-01-15",
                    "maturity": "2025-06-15",
                    "day_count": "act_360",
                    "fixed_rate": "0.05",
                    "discount_curve_id": "USD-OIS",
                    "attributes": {},
                },
            },
            "quantity": 1.0,
            "unit": "units",
        })
    return json.dumps({
        "id": f"bench-portfolio-{n_positions}",
        "as_of": "2025-01-15",
        "base_currency": "USD",
        "entities": {"ENTITY-1": {"id": "ENTITY-1"}},
        "positions": positions,
    })


def _build_bench_market(rate: float = 0.05, as_of: date = date(2025, 1, 15)) -> MarketContext:
    """Build a minimal `MarketContext` with a flat USD-OIS discount curve.

    Uses a knot schedule equivalent to ``rate`` out to 2Y at ``as_of``.
    """
    # Flat act/360 rate ≈ df = exp(-rate * t) for a reasonable approximation.
    knots = [(t, math.exp(-rate * t)) for t in (0.0, 0.25, 0.5, 1.0, 2.0)]
    curve = DiscountCurve("USD-OIS", as_of, knots)
    return MarketContext().insert(curve)


_BENCH_SPEC_JSON_500 = _build_portfolio_spec_json(500)
_BENCH_MARKET = _build_bench_market()
_BENCH_MARKET_JSON = _BENCH_MARKET.to_json()
_BENCH_MARKET_T1 = _build_bench_market(0.0525, date(2025, 1, 16))


def _assert_deposit_valuation(valuation, n_positions: int, *, standard_risk: bool = True) -> None:
    """Check the complete deposit book outside the pricing timer."""
    assert len(valuation) == n_positions
    assert valuation.base_currency == "USD"
    assert math.isfinite(valuation.total_value)
    assert not valuation.degraded_positions
    positions = valuation.position_values
    assert len(positions) == n_positions
    for position in positions.values():
        assert position.risk_metrics_complete
        assert position.risk_error is None
        assert not position.inapplicable_metrics
        assert math.isfinite(position.value_native.amount)
        assert math.isfinite(position.value_base.amount)
        result = position.valuation_result
        assert result is not None
        assert result["value"]["currency"] == "USD"
        measures = result["measures"]
        assert all(math.isfinite(value) for value in measures.values())
        if standard_risk:
            assert "dv01" in measures
        else:
            assert not measures


def _assert_deposit_workflow(result, n_positions: int) -> None:
    valuation, metrics, cashflows = result
    _assert_deposit_valuation(valuation, n_positions)
    assert not metrics.skipped_metrics
    assert not metrics.degraded_positions
    assert len(metrics.by_position) == n_positions
    assert "dv01" in metrics.aggregated
    assert math.isfinite(metrics.aggregated["dv01"]["total"])
    assert not cashflows.issues
    by_position = cashflows.by_position
    events = cashflows.events
    assert set(by_position) == {f"POS-{index}" for index in range(n_positions)}
    # The valuation date equals the contractual start, so the initial exchange
    # remains alongside the separately classified maturity interest/principal.
    expected_flows = [("2025-01-15", "notional"), ("2025-06-15", "fixed"), ("2025-06-15", "notional")]
    assert len(events) == 3 * n_positions
    position_events = []
    for index in range(n_positions):
        position_id = f"POS-{index}"
        flows = by_position[position_id]
        assert sorted((event["date"], event["kind"]) for event in flows) == expected_flows
        for event in flows:
            assert event["position_id"] == position_id
            assert event["instrument_id"] == f"DEP-{index}"
            assert event["amount"]["currency"] == "USD"
            amount = float(event["amount"]["amount"])
            assert math.isfinite(amount)
            if event["kind"] == "notional":
                sign = -1 if event["date"] == "2025-01-15" else 1
                assert amount == sign * (1_000_000 + index)
            else:
                assert amount > 0.0
        position_events.extend(flows)

    def event_order(event):
        return event["date"], event["position_id"], event["kind"]

    assert sorted(events, key=event_order) == sorted(position_events, key=event_order)


def _assert_deposit_scenario_pnl(result, n_positions: int) -> None:
    assert result.currency == "USD"
    assert math.isfinite(result.total)
    assert len(result.by_position) == n_positions
    assert all(math.isfinite(value) for value in result.by_position.values())


def _build_curve_scenario_jsons(n_scenarios: int) -> tuple[str, ...]:
    """Build validated parallel-shift scenarios before benchmark timing begins."""
    return tuple(
        ScenarioSpec(
            f"bench-parallel-{index}",
            [
                OperationSpec.curve_parallel_bp(
                    CurveKind.discount(),
                    "USD-OIS",
                    float(index + 1),
                )
            ],
            priority=index,
        ).to_json()
        for index in range(n_scenarios)
    )


def _build_replay_snapshots_json(n_snapshots: int) -> str:
    """Build a dated, gently rising-rate replay path before benchmark timing."""
    start = date(2025, 1, 15)
    snapshots = [
        (
            f'{{"date":"{(start + timedelta(days=index)).isoformat()}",'
            f'"market":{_build_bench_market(0.05 + index * 0.00005, start + timedelta(days=index)).to_json()}}}'
        )
        for index in range(n_snapshots)
    ]
    return f"[{','.join(snapshots)}]"


_RISK_MATRIX_SIZE = 256
_RISK_POSITION_IDS = [f"RISK-{i}" for i in range(_RISK_MATRIX_SIZE)]
_RISK_WEIGHTS = [1.0 / _RISK_MATRIX_SIZE] * _RISK_MATRIX_SIZE
_RISK_COVARIANCE_LIST = [
    [0.04 if row == col else 0.002 for col in range(_RISK_MATRIX_SIZE)] for row in range(_RISK_MATRIX_SIZE)
]

_HISTORICAL_POSITION_COUNT = 200
_HISTORICAL_SCENARIO_COUNT = 1_000
_HISTORICAL_POSITION_IDS = [f"HIST-{i}" for i in range(_HISTORICAL_POSITION_COUNT)]
_HISTORICAL_PNLS_LIST = [
    [((position + 1) * ((scenario % 31) - 15)) / 10_000.0 for scenario in range(_HISTORICAL_SCENARIO_COUNT)]
    for position in range(_HISTORICAL_POSITION_COUNT)
]
_HISTORICAL_PNLS_NUMPY = np.asarray(_HISTORICAL_PNLS_LIST, dtype=np.float64)


def _build_model_spec() -> FinancialModelSpec:
    """Build a small model spec via ModelBuilder (correct wire format)."""
    b = ModelBuilder("bench-model")
    b.periods("2025Q1..Q2", None)
    b.value("revenue", [("2025Q1", 100.0), ("2025Q2", 110.0)])
    b.value("cogs", [("2025Q1", 60.0), ("2025Q2", 65.0)])
    b.compute("gross_profit", "revenue - cogs")
    return b.build()


_MODEL_SPEC = _build_model_spec()
_MODEL_JSON = _MODEL_SPEC.to_json()

SENSITIVITY_CONFIG_JSON = json.dumps({
    "mode": "diagonal",
    "parameters": [
        {
            "node_id": "revenue",
            "period_id": "2025Q1",
            "base_value": 100.0,
            "perturbations": [-10.0, -5.0, 0.0, 5.0, 10.0],
        }
    ],
    "target_metrics": ["gross_profit"],
})

_EVALUATOR = Evaluator()
_EVAL_RESULT = _EVALUATOR.evaluate(_MODEL_SPEC)
_EVAL_RESULT_JSON = _EVAL_RESULT.to_json()

VARIANCE_CONFIG_JSON = json.dumps({
    "baseline_label": "base",
    "comparison_label": "comparison",
    "metrics": ["gross_profit"],
    "periods": ["2025Q1", "2025Q2"],
})


def _build_comparison_model_spec() -> FinancialModelSpec:
    """Build a comparison model for variance analysis."""
    b = ModelBuilder("bench-comparison")
    b.periods("2025Q1..Q2", None)
    b.value("revenue", [("2025Q1", 105.0), ("2025Q2", 115.0)])
    b.value("cogs", [("2025Q1", 62.0), ("2025Q2", 67.0)])
    b.compute("gross_profit", "revenue - cogs")
    return b.build()


_COMPARISON_SPEC = _build_comparison_model_spec()
_COMPARISON_RESULT = _EVALUATOR.evaluate(_COMPARISON_SPEC)
_COMPARISON_RESULT_JSON = _COMPARISON_RESULT.to_json()

SCENARIO_SET_JSON = json.dumps({
    "scenarios": {
        "upside": {
            "overrides": {"revenue": 120.0},
        },
        "downside": {
            "overrides": {"revenue": 80.0},
        },
    },
})


# Core domain


@pytest.mark.perf
class TestCoreBenchmarks:
    """Core primitives: currency, money, dates, curves, math."""

    def test_currency_creation(self, benchmark) -> None:
        _ccm_bench(
            benchmark,
            lambda: Currency("USD"),
            _ccm._currency,
            parameters={"currency": "USD"},
            boundary="Currency parsing and wrapper construction are timed.",
        )

    def test_money_add_sub(self, benchmark) -> None:
        usd = Currency("USD")
        a = Money(100.0, usd)
        b = Money(1.0, usd)

        def _add_sub():
            x = a + b
            return x - b

        _ccm_bench(
            benchmark,
            _add_sub,
            lambda value: _ccm._money(value, 100.0),
            parameters={"currency": "USD", "left": 100.0, "right": 1.0, "operations": "(left + right) - right"},
            boundary="Inputs exist before timing; both Money arithmetic operations and result wrappers are timed.",
        )

    def test_daycount_year_fraction(self, benchmark) -> None:
        day_count = DayCount.ACT_360
        start = date(2024, 1, 1)
        end = date(2025, 1, 1)
        _ccm_bench(
            benchmark,
            lambda: day_count.year_fraction(start, end),
            _ccm._scalar(366.0 / 360.0),
            parameters={"day_count": "act_360", "start": start.isoformat(), "end": end.isoformat(), "actual_days": 366},
            boundary="DayCount and dates exist before timing; date conversion and year-fraction calculation are timed.",
        )

    def test_discount_curve_df(self, benchmark) -> None:
        curve = DiscountCurve(
            "USD-BENCH",
            date(2024, 1, 1),
            [(0.0, 1.0), (1.0, 0.95), (5.0, 0.75), (10.0, 0.50)],
            interp="monotone_convex",
            day_count="act_365f",
        )
        knots = [(0.0, 1.0), (1.0, 0.95), (5.0, 0.75), (10.0, 0.50)]
        assert curve.interp_style == "monotone_convex"
        for time, discount in knots:
            _ccm._close(curve.df(time), discount)
        _ccm_bench(
            benchmark,
            lambda: curve.df(2.5),
            _ccm._discount,
            parameters={
                "id": "USD-BENCH",
                "base_date": "2024-01-01",
                "knots": knots,
                "interp": "monotone_convex",
                "day_count": "act_365f",
                "time_years": 2.5,
            },
            boundary="Curve construction is outside timing; one numeric-time DF query and result conversion are timed.",
        )

    def test_cholesky_5x5(self, benchmark) -> None:
        _ccm_bench(
            benchmark,
            lambda: linalg.cholesky_decomposition(SPD_5X5),
            lambda value: _ccm._cholesky(value, SPD_5X5),
            parameters={"dimension": 5, "matrix": SPD_5X5},
            boundary="Input matrix exists before timing; conversion, factorization and the complete 5x5 returned factor are timed.",
        )

    def test_forward_curve_rate(self, benchmark) -> None:
        curve = ForwardCurve(
            "USD-SOFR-3M",
            0.25,
            date(2024, 1, 1),
            [(0.0, 0.05), (1.0, 0.052), (5.0, 0.055), (10.0, 0.06)],
            interp="linear",
        )
        assert curve.interp_style == "linear"
        _ccm_bench(
            benchmark,
            lambda: curve.rate(2.5),
            _ccm._scalar(0.052 + (2.5 - 1.0) / (5.0 - 1.0) * (0.055 - 0.052)),
            parameters={
                "id": "USD-SOFR-3M",
                "base_date": "2024-01-01",
                "tenor_years": 0.25,
                "knots": [(0.0, 0.05), (1.0, 0.052), (5.0, 0.055), (10.0, 0.06)],
                "interp": "linear",
                "day_count": curve.day_count,
                "reset_lag": curve.reset_lag,
                "time_years": 2.5,
                "rate_units": "decimal",
            },
            boundary="Forward-curve construction is outside timing; one numeric-time rate lookup and result conversion are timed.",
        )

    def test_fx_matrix_rate(self, benchmark) -> None:
        fx = FxMatrix(None)
        fx.set_quotes([("USD", "EUR", 0.92)])
        ref_date = date(2024, 6, 15)
        _ccm_bench(
            benchmark,
            lambda: fx.rate("USD", "EUR", ref_date, "cashflow_date"),
            _ccm._fx,
            parameters={
                "quotes": [["USD", "EUR", 0.92]],
                "base": "USD",
                "quote": "EUR",
                "date": ref_date.isoformat(),
                "policy": "cashflow_date",
                "quote_units": "EUR per USD",
            },
            boundary="Matrix and quote setup are outside timing; the direct FX lookup and full FxRateResult conversion are timed.",
        )

    def test_stats_mean_variance(self, benchmark) -> None:
        def _mean_var():
            m = stats.mean(DATA_10K)
            v = stats.variance(DATA_10K)
            return m, v

        _ccm_bench(
            benchmark,
            _mean_var,
            _ccm._mean_variance,
            parameters={"count": 10_000, "values": "i * 0.01 for i in 0..10000", "variance_denominator": "n - 1"},
            boundary="The 10,000-item Python list exists before timing; two input conversions and mean/sample-variance calculations are timed.",
        )

    def test_tenor_parsing(self, benchmark) -> None:
        _ccm_bench(
            benchmark,
            lambda: Tenor.parse("3M"),
            _ccm._tenor,
            parameters={"text": "3M", "months": 3},
            boundary="Tenor parsing and returned wrapper construction are timed; accessor validation is outside timing.",
        )

    def test_rate_conversions(self, benchmark) -> None:
        def _round_trip():
            r = Rate(0.05)
            p = r.as_percent
            b = r.as_bp
            r2 = Rate.from_percent(p)
            r3 = Rate.from_bp(b)
            return r2, r3

        _ccm_bench(
            benchmark,
            _round_trip,
            _ccm._rates,
            parameters={"decimal_rate": 0.05, "percent": 5.0, "basis_points": 500},
            boundary="Rate construction, percent/BP access and both round-trip constructors are timed.",
        )


# Analytics domain


@pytest.mark.perf
class TestAnalyticsBenchmarks:
    """Performance analytics: returns, drawdowns, risk metrics."""

    def test_performance_construction(self, benchmark) -> None:
        n = 252
        dates = [date(2024, 1, 1) + timedelta(days=i) for i in range(n)]
        prices = [100.0 + i * 0.1 for i in range(n)]
        panel = PerformancePanel(dates=dates, series=[prices], kind="prices", ticker_names=["BENCH"])
        benchmark(Performance.from_panel, panel)

    def test_sharpe(self, benchmark) -> None:
        benchmark(_PERFORMANCE_10K.sharpe)

    def test_to_drawdown_series(self, benchmark) -> None:
        benchmark(_PERFORMANCE_10K.get_drawdown_series)

    def test_rolling_sharpe(self, benchmark) -> None:
        benchmark(_PERFORMANCE_10K.rolling_sharpe, 0, 63)

    def test_volatility(self, benchmark) -> None:
        benchmark(_PERFORMANCE_10K.volatility)

    def test_simple_returns(self, benchmark) -> None:
        benchmark(_PERFORMANCE_10K.get_returns)

    def test_comp_sum(self, benchmark) -> None:
        benchmark(_PERFORMANCE_10K.cumulative_returns)

    def test_value_at_risk(self, benchmark) -> None:
        benchmark(_PERFORMANCE_10K.value_at_risk)

    def test_expected_shortfall(self, benchmark) -> None:
        benchmark(_PERFORMANCE_10K.expected_shortfall)

    def test_skewness_kurtosis(self, benchmark) -> None:
        def _skew_kurt():
            s = _PERFORMANCE_10K.skewness()
            k = _PERFORMANCE_10K.kurtosis()
            return s, k

        benchmark(_skew_kurt)

    def test_sortino(self, benchmark) -> None:
        benchmark(_PERFORMANCE_10K.sortino)

    def test_beta(self, benchmark) -> None:
        benchmark(_PERFORMANCE_10K.beta)

    def test_tracking_error(self, benchmark) -> None:
        benchmark(_PERFORMANCE_10K.tracking_error)

    def test_drawdown_details(self, benchmark) -> None:
        benchmark(_PERFORMANCE_10K.drawdown_details, 0, 5)

    def test_calmar(self, benchmark) -> None:
        benchmark(_PERFORMANCE_10K.calmar)

    def test_period_stats(self, benchmark) -> None:
        benchmark(_PERFORMANCE_10K.period_stats, 0, "monthly")


# Correlation domain


@pytest.mark.perf
class TestCorrelationBenchmarks:
    """Copula and factor model computations."""

    def test_copula_build_and_conditional(self, benchmark) -> None:
        copula = CopulaSpec.gaussian().build()
        assert copula.num_factors == 1
        assert copula.model_name == "One-Factor Gaussian Copula"

        def _build_and_query():
            copula = CopulaSpec.gaussian().build()
            return copula.conditional_default_prob(-1.5, [0.0], 0.3)

        _ccm_bench(
            benchmark,
            _build_and_query,
            _ccm._scalar(_ccm._normal_cdf(-1.5 / math.sqrt(0.7)), abs_tol=_ccm._COPULA_ABS_TOL),
            parameters={
                "copula": "gaussian",
                "model_name": copula.model_name,
                "factors": 1,
                "default_threshold": -1.5,
                "factor_realization": [0.0],
                "correlation": 0.3,
            },
            boundary="Specification creation, concrete copula construction and one conditional-PD query are timed.",
        )

    def test_correlated_bernoulli(self, benchmark) -> None:
        def _construct_and_query():
            cb = CorrelatedBernoulli(0.3, 0.5, 0.2)
            return cb.joint_probabilities()

        _ccm_bench(
            benchmark,
            _construct_and_query,
            _ccm._bernoulli,
            parameters={"p1": 0.3, "p2": 0.5, "correlation": 0.2, "output_order": ["p11", "p10", "p01", "p00"]},
            boundary="Bernoulli-pair construction and conversion of all four joint probabilities are timed.",
        )

    def test_recovery_model(self, benchmark) -> None:
        def _build_and_query():
            spec = RecoverySpec.constant(0.4)
            model = spec.build()
            return model.conditional_recovery(-1.0)

        _ccm_bench(
            benchmark,
            _build_and_query,
            _ccm._scalar(0.4),
            parameters={"model": "constant", "recovery_fraction": 0.4, "factor_realization": -1.0},
            boundary="Recovery specification, model construction and one conditional-recovery query are timed.",
        )

    def test_factor_model(self, benchmark) -> None:
        assert LatentFactorSpec.single_factor(0.15, 0.5).build().num_factors == 1

        def _build_and_query():
            spec = LatentFactorSpec.single_factor(0.15, 0.5)
            model = spec.build()
            return model.diagonal_factor_contribution(0, 1.0)

        _ccm_bench(
            benchmark,
            _build_and_query,
            _ccm._scalar(0.15),
            parameters={
                "model": "single_factor",
                "volatility": 0.15,
                "mean_reversion": 0.5,
                "factor_index": 0,
                "z": 1.0,
            },
            boundary="Factor specification/model construction and one diagonal-factor query are timed.",
        )

    def test_single_factor_model_construction(self, benchmark) -> None:
        _ccm_bench(
            benchmark,
            lambda: LatentSingleFactor(0.15, 0.5),
            _ccm._factor,
            parameters={"volatility": 0.15, "mean_reversion": 0.5, "factors": 1},
            boundary="Single-factor model construction is timed; complete accessor validation is outside timing.",
        )

    def test_correlation_bounds(self, benchmark) -> None:
        _ccm_bench(
            benchmark,
            lambda: correlation_bounds(0.3, 0.5),
            _ccm._bounds,
            parameters={"p1": 0.3, "p2": 0.5, "output_order": ["rho_min", "rho_max"]},
            boundary="Fréchet correlation-bound computation and the complete pair conversion are timed.",
        )

    def test_validate_correlation_matrix(self, benchmark) -> None:
        assert len(CORR_5X5_FLAT) == 25
        for row in range(5):
            assert CORR_5X5_FLAT[row * 5 + row] == 1.0
            for column in range(5):
                _ccm._finite(CORR_5X5_FLAT[row * 5 + column])
                assert CORR_5X5_FLAT[row * 5 + column] == CORR_5X5_FLAT[column * 5 + row]
        with pytest.raises(ValueError, match="entries, got 3"):
            validate_correlation_matrix([1.0, 0.3, 0.3], 2)
        _ccm_bench(
            benchmark,
            lambda: validate_correlation_matrix(CORR_5X5_FLAT, 5),
            _ccm._none,
            parameters={"dimension": 5, "matrix_flat_row_major": CORR_5X5_FLAT},
            boundary="The flat matrix exists before timing; input conversion and complete correlation validation are timed.",
        )


# Monte Carlo domain


@pytest.mark.perf
class TestMonteCarloBenchmarks:
    """Option pricing: analytical and simulation."""

    def test_black_scholes_call(self, benchmark) -> None:
        _ccm_bench(
            benchmark,
            lambda: bs_price(100.0, 100.0, 0.05, 0.0, 0.2, 1.0, True),
            _ccm._scalar(_ccm._bs_reference(), abs_tol=_ccm._BS_ABS_TOL),
            parameters={
                "spot": 100.0,
                "strike": 100.0,
                "rate": 0.05,
                "dividend_yield": 0.0,
                "volatility": 0.2,
                "time_to_expiry": 1.0,
                "option_type": "call",
                "rates": "continuous decimal",
                "time_units": "years",
            },
            boundary="Scalar argument conversion, Black-Scholes price and scalar result conversion are timed.",
        )

    def test_lsmc_american_put(self, benchmark) -> None:
        pricer = LsmcPricer(
            num_paths=5_000,
            seed=42,
            use_parallel=False,
            num_steps=50,
            basis="laguerre",
            basis_degree=3,
            antithetic=True,
        )
        assert pricer.num_paths == 5_000
        assert pricer.seed == 42
        assert pricer.use_parallel is False
        assert pricer.antithetic is True
        assert pricer.basis == "laguerre"
        assert pricer.basis_degree == 3

        def _price():
            return pricer.price_american(
                {
                    "spot": 100.0,
                    "strike": 100.0,
                    "rate": 0.05,
                    "dividend_yield": 0.0,
                    "volatility": 0.3,
                    "time_to_expiry": 1.0,
                    "option_type": "put",
                },
                currency="USD",
            )

        _ccm_bench(
            benchmark,
            _price,
            lambda value: _ccm._estimate(
                value,
                independent=5_000,
                physical=10_000,
                lower_reference=_ccm._bs_reference(volatility=0.3, is_call=False),
            ),
            parameters={
                "spot": 100.0,
                "strike": 100.0,
                "rate": 0.05,
                "dividend_yield": 0.0,
                "volatility": 0.3,
                "time_to_expiry": 1.0,
                "option_type": "put",
                "currency": "USD",
                "independent_estimators": 5_000,
                "physical_paths": 10_000,
                "steps": 50,
                "exercise_steps": list(range(1, 51)),
                "seed": 42,
                "parallel": False,
                "antithetic": True,
                "basis": "laguerre",
                "basis_degree": 3,
                "retained_spot_values": 510_000,
                "estimator": "in-sample Bermudan LSMC",
            },
            boundary="Pricer construction is outside timing; market-dict conversion, all path generation/storage, backward induction and full MoneyEstimate conversion are timed.",
        )

    def test_european_pricer(self, benchmark) -> None:
        pricer = EuropeanPricer(num_paths=10_000, seed=42, use_parallel=False)
        assert pricer.num_paths == 10_000
        assert pricer.seed == 42
        assert pricer.use_parallel is False

        def _price():
            return pricer.price_gbm(
                {
                    "spot": 100.0,
                    "strike": 100.0,
                    "rate": 0.05,
                    "dividend_yield": 0.0,
                    "volatility": 0.2,
                    "time_to_expiry": 1.0,
                    "option_type": "call",
                },
                num_steps=252,
                currency="USD",
            )

        _ccm_bench(
            benchmark,
            _price,
            lambda value: _ccm._estimate(value, independent=10_000, physical=10_000, reference=_ccm._bs_reference()),
            parameters={
                "spot": 100.0,
                "strike": 100.0,
                "rate": 0.05,
                "dividend_yield": 0.0,
                "volatility": 0.2,
                "time_to_expiry": 1.0,
                "option_type": "call",
                "currency": "USD",
                "independent_estimators": 10_000,
                "physical_paths": 10_000,
                "steps": 252,
                "seed": 42,
                "parallel": False,
                "antithetic": False,
                "capture": False,
                "discretization": "exact_gbm",
            },
            boundary="Pricer construction is outside timing; market-dict conversion, full exact-GBM simulation and full MoneyEstimate conversion are timed.",
        )

    def test_path_dependent_asian(self, benchmark) -> None:
        pricer = PathDependentPricer(
            num_paths=5_000,
            seed=42,
            use_parallel=False,
            antithetic=False,
            use_sobol=False,
            use_brownian_bridge=False,
        )
        assert pricer.num_paths == 5_000
        assert pricer.seed == 42
        assert pricer.use_parallel is False
        assert pricer.antithetic is False
        assert pricer.use_sobol is False
        assert pricer.use_brownian_bridge is False

        def _price():
            return pricer.price_asian(
                {
                    "spot": 100.0,
                    "strike": 100.0,
                    "rate": 0.05,
                    "dividend_yield": 0.0,
                    "volatility": 0.2,
                    "time_to_expiry": 1.0,
                    "option_type": "call",
                },
                num_steps=50,
                currency="USD",
            )

        _ccm_bench(
            benchmark,
            _price,
            lambda value: _ccm._estimate(value, independent=5_000, physical=5_000),
            parameters={
                "spot": 100.0,
                "strike": 100.0,
                "rate": 0.05,
                "dividend_yield": 0.0,
                "volatility": 0.2,
                "time_to_expiry": 1.0,
                "option_type": "call",
                "currency": "USD",
                "independent_estimators": 5_000,
                "physical_paths": 5_000,
                "steps": 50,
                "fixing_steps": list(range(1, 51)),
                "averaging": "arithmetic",
                "seed": 42,
                "parallel": False,
                "antithetic": False,
                "sobol": False,
                "brownian_bridge": False,
                "capture": False,
                "discretization": "exact_gbm",
            },
            boundary="Pricer construction is outside timing; market-dict conversion, all exact-GBM paths and arithmetic fixing accumulation, and full MoneyEstimate conversion are timed.",
        )

    def test_black_scholes_put(self, benchmark) -> None:
        _ccm_bench(
            benchmark,
            lambda: bs_price(100.0, 100.0, 0.05, 0.0, 0.2, 1.0, False),
            _ccm._scalar(_ccm._bs_reference(is_call=False), abs_tol=_ccm._BS_ABS_TOL),
            parameters={
                "spot": 100.0,
                "strike": 100.0,
                "rate": 0.05,
                "dividend_yield": 0.0,
                "volatility": 0.2,
                "time_to_expiry": 1.0,
                "option_type": "put",
                "rates": "continuous decimal",
                "time_units": "years",
            },
            boundary="Scalar argument conversion, Black-Scholes price and scalar result conversion are timed.",
        )


# Margin domain


@pytest.mark.perf
class TestMarginBenchmarks:
    """VM/IM margin calculations."""

    def test_csa_spec_construction(self, benchmark) -> None:
        benchmark(CsaSpec.usd_regulatory)

    def test_vm_calculate(self, benchmark) -> None:
        csa = CsaSpec.usd_regulatory()
        calc = VmCalculator(csa)
        benchmark(calc.calculate, 1_000_000.0, 0.0, "USD", "2024-06-15")

    def test_netting_set_id(self, benchmark) -> None:
        def _create_ids():
            b = NettingSetId.bilateral("CPTY-1", "CSA-001")
            c = NettingSetId.cleared("LCH")
            return b, c

        benchmark(_create_ids)

    def test_funding_config(self, benchmark) -> None:
        benchmark(FundingConfig, 50.0, 30.0)

    def test_margin_utilization(self, benchmark) -> None:
        benchmark(MarginUtilization, 1_200_000.0, 1_000_000.0, "USD")


# Statements domain


@pytest.mark.perf
class TestStatementsBenchmarks:
    """Financial model spec parsing, building, and evaluation."""

    def test_from_json(self, benchmark) -> None:
        benchmark(FinancialModelSpec.from_json, _MODEL_JSON)

    def test_model_builder(self, benchmark) -> None:
        def _build():
            b = ModelBuilder("bench")
            b.periods("2025Q1..Q2", None)
            b.value("revenue", [("2025Q1", 100.0), ("2025Q2", 110.0)])
            b.value("cogs", [("2025Q1", 60.0), ("2025Q2", 65.0)])
            b.compute("gross_profit", "revenue - cogs")
            return b.build()

        benchmark(_build)

    def test_evaluator(self, benchmark) -> None:
        ev = Evaluator()
        benchmark.pedantic(ev.evaluate, args=(_MODEL_SPEC,), rounds=20, warmup_rounds=2)

    def test_parse_formula(self, benchmark) -> None:
        benchmark(parse_formula, "revenue * 1.05 + cogs")

    def test_parse_and_compile(self, benchmark) -> None:
        benchmark(parse_and_compile, "revenue + cogs")

    def test_normalization(self, benchmark) -> None:
        config = NormalizationConfig("gross_profit")

        def _normalize():
            return normalize(_EVAL_RESULT, config)

        benchmark(_normalize)


# Statements Analytics domain


@pytest.mark.perf
class TestStatementsAnalyticsBenchmarks:
    """Sensitivity analysis and forecast backtesting."""

    def test_run_sensitivity_json(self, benchmark) -> None:
        benchmark.pedantic(
            run_sensitivity,
            args=(_MODEL_JSON, SENSITIVITY_CONFIG_JSON),
            rounds=20,
            warmup_rounds=2,
        )

    def test_run_sensitivity_typed(self, benchmark) -> None:
        benchmark.pedantic(
            run_sensitivity,
            args=(_MODEL_SPEC, SENSITIVITY_CONFIG_JSON),
            rounds=20,
            warmup_rounds=2,
        )

    def test_backtest_forecast(self, benchmark) -> None:
        actual = [float(i) for i in range(100)]
        forecast = [float(i) + 0.5 for i in range(100)]
        benchmark(backtest_forecast, actual, forecast)

    def test_run_variance_json(self, benchmark) -> None:
        benchmark.pedantic(
            run_variance,
            args=(_EVAL_RESULT_JSON, _COMPARISON_RESULT_JSON, VARIANCE_CONFIG_JSON),
            rounds=20,
            warmup_rounds=2,
        )

    def test_run_variance_typed(self, benchmark) -> None:
        benchmark.pedantic(
            run_variance,
            args=(_EVAL_RESULT, _COMPARISON_RESULT, VARIANCE_CONFIG_JSON),
            rounds=20,
            warmup_rounds=2,
        )

    def test_evaluate_scenario_set_json(self, benchmark) -> None:
        benchmark.pedantic(
            evaluate_scenario_set,
            args=(_MODEL_JSON, SCENARIO_SET_JSON),
            rounds=20,
            warmup_rounds=2,
        )

    def test_evaluate_scenario_set_typed(self, benchmark) -> None:
        benchmark.pedantic(
            evaluate_scenario_set,
            args=(_MODEL_SPEC, SCENARIO_SET_JSON),
            rounds=20,
            warmup_rounds=2,
        )

    def test_goal_seek_json(self, benchmark) -> None:
        def _seek():
            return goal_seek(
                _MODEL_JSON,
                {
                    "target_node": "gross_profit",
                    "target_period": "2025Q1",
                    "target_value": 50.0,
                    "driver_node": "revenue",
                    "driver_period": "2025Q1",
                    "update_model": False,
                },
            )

        benchmark.pedantic(_seek, rounds=10, warmup_rounds=1)

    def test_goal_seek_typed(self, benchmark) -> None:
        def _seek():
            return goal_seek(
                _MODEL_SPEC,
                {
                    "target_node": "gross_profit",
                    "target_period": "2025Q1",
                    "target_value": 50.0,
                    "driver_node": "revenue",
                    "driver_period": "2025Q1",
                    "update_model": False,
                },
            )

        benchmark.pedantic(_seek, rounds=10, warmup_rounds=1)

    def test_dependency_tracer_json(self, benchmark) -> None:
        def _trace():
            tree = DependencyTracer(_MODEL_JSON).dependency_tree("gross_profit")
            deps = DependencyTracer(_MODEL_JSON).direct_dependencies("gross_profit")
            return tree, deps

        benchmark(_trace)

    def test_dependency_tracer_typed(self, benchmark) -> None:
        def _trace():
            tree = DependencyTracer(_MODEL_SPEC).dependency_tree("gross_profit")
            deps = DependencyTracer(_MODEL_SPEC).direct_dependencies("gross_profit")
            return tree, deps

        benchmark(_trace)

    def test_explain_formula_json(self, benchmark) -> None:
        benchmark(explain_formula, _MODEL_JSON, _EVAL_RESULT_JSON, "gross_profit", "2025Q1")

    def test_explain_formula_typed(self, benchmark) -> None:
        benchmark(explain_formula, _MODEL_SPEC, _EVAL_RESULT, "gross_profit", "2025Q1")


# Portfolio domain


@pytest.mark.perf
class TestPortfolioBenchmarks:
    """Portfolio JSON pipeline: parse + build."""

    def test_from_materialization_cold_5000_unique(self, benchmark) -> None:
        """Cold-cache binding path for 5,000 unique schedule-rich artifacts."""
        payload = _materialization_fixture_bytes("materialization-a-5000-unique")
        _, report = Portfolio.from_materialization(payload, InstrumentArtifactCache(5_000))
        _record_materialization_report(benchmark, report)

        def setup():
            return (payload, InstrumentArtifactCache(5_000)), {}

        benchmark.pedantic(
            Portfolio.from_materialization,
            setup=setup,
            rounds=_MATERIALIZATION_BENCH_ROUNDS,
            warmup_rounds=0,
        )

    def test_from_materialization_cold_5000_dedup(self, benchmark) -> None:
        """Cold-cache binding path for 5,000 positions over 50 artifacts."""
        payload = _materialization_fixture_bytes("materialization-b-5000-50")
        _, report = Portfolio.from_materialization(payload, InstrumentArtifactCache(50))
        _record_materialization_report(benchmark, report)

        def setup():
            return (payload, InstrumentArtifactCache(50)), {}

        benchmark.pedantic(
            Portfolio.from_materialization,
            setup=setup,
            rounds=_MATERIALIZATION_BENCH_ROUNDS,
            warmup_rounds=0,
        )

    def test_from_materialization_warm_5000_dedup(self, benchmark) -> None:
        """Warm-cache binding path for 5,000 positions over 50 artifacts."""
        payload = _materialization_fixture_bytes("materialization-b-5000-50")
        cache = InstrumentArtifactCache(50)
        Portfolio.from_materialization(payload, cache)
        _, report = Portfolio.from_materialization(payload, cache)
        _record_materialization_report(benchmark, report)
        benchmark.pedantic(
            Portfolio.from_materialization,
            args=(payload, cache),
            rounds=_MATERIALIZATION_BENCH_ROUNDS,
            warmup_rounds=0,
        )

    def test_parse_and_build_one_deposit(self, benchmark) -> None:
        def _parse_build():
            spec = parse_portfolio_spec_json(PORTFOLIO_SPEC_JSON)
            return Portfolio.from_spec(spec).to_json()

        assert len(json.loads(_parse_build())["positions"]) == 1
        result = benchmark.pedantic(_parse_build, rounds=10, warmup_rounds=1)
        assert len(json.loads(result)["positions"]) == 1

    def test_parse_one_deposit_portfolio_spec_json(self, benchmark) -> None:
        result = benchmark(parse_portfolio_spec_json, PORTFOLIO_SPEC_JSON)
        assert len(json.loads(result)["positions"]) == 1


@pytest.mark.perf
class TestPortfolioCompoundWorkflow:
    """Deposit-only compound workflows through the Python binding.

    These measure the realistic calling pattern (value + metrics + cashflows)
    with JSON inputs and with the typed :class:`Portfolio` /
    :class:`MarketContext` fast path.
    """

    def test_json_path_500_deposits_value_metrics_cashflows(self, benchmark) -> None:
        """JSON inputs, 500 deposits: every call re-parses + rebuilds."""

        def _run():
            val = value_portfolio(_BENCH_SPEC_JSON_500, _BENCH_MARKET_JSON, strict_risk=True)
            agg = aggregate_metrics(val, _BENCH_MARKET_JSON)
            cf = aggregate_full_cashflows(_BENCH_SPEC_JSON_500, _BENCH_MARKET_JSON)
            return val, agg, cf

        _assert_deposit_workflow(_run(), 500)
        result = benchmark.pedantic(_run, rounds=5, warmup_rounds=1)
        _assert_deposit_workflow(result, 500)

    def test_typed_pipeline_500_deposits_value_metrics_cashflows(self, benchmark) -> None:
        """Typed inputs, 500 deposits: Portfolio + MarketContext built once."""
        portfolio = Portfolio.from_spec(_BENCH_SPEC_JSON_500)

        def _run():
            val = value_portfolio(portfolio, _BENCH_MARKET, strict_risk=True)
            agg = aggregate_metrics(val, _BENCH_MARKET)
            cf = aggregate_full_cashflows(portfolio, _BENCH_MARKET)
            return val, agg, cf

        _assert_deposit_workflow(_run(), 500)
        result = benchmark.pedantic(_run, rounds=5, warmup_rounds=1)
        _assert_deposit_workflow(result, 500)

    def test_typed_deposit_portfolio_from_spec_500(self, benchmark) -> None:
        """Cost of building a typed Portfolio from a 500-deposit spec."""
        result = benchmark(Portfolio.from_spec, _BENCH_SPEC_JSON_500)
        assert len(result) == 500

    def test_value_500_deposits_prebuilt_inputs(self, benchmark) -> None:
        """Typed input and output path, avoiding valuation JSON serialization."""
        portfolio = Portfolio.from_spec(_BENCH_SPEC_JSON_500)
        _assert_deposit_valuation(value_portfolio(portfolio, _BENCH_MARKET, strict_risk=True), 500)
        result = benchmark(value_portfolio, portfolio, _BENCH_MARKET, strict_risk=True)
        _assert_deposit_valuation(result, 500)

    def test_value_500_deposits_json_inputs(self, benchmark) -> None:
        """Pure value_portfolio with JSON inputs and typed output, 500 deposits."""
        _assert_deposit_valuation(value_portfolio(_BENCH_SPEC_JSON_500, _BENCH_MARKET_JSON, strict_risk=True), 500)
        result = benchmark(value_portfolio, _BENCH_SPEC_JSON_500, _BENCH_MARKET_JSON, strict_risk=True)
        _assert_deposit_valuation(result, 500)


@pytest.mark.perf
class TestPortfolioReleaseControls:
    """Deposit-only release-scale controls for the typed portfolio bindings."""

    @pytest.mark.parametrize(
        "n_positions",
        [40, pytest.param(120, marks=pytest.mark.slow)],
        ids=["40-deposits", "120-deposits"],
    )
    def test_metrics_based_attribution(self, benchmark, n_positions: int) -> None:
        """Metrics attribution over typed books of 40 and 120 deposits."""
        portfolio = Portfolio.from_spec(_build_portfolio_spec_json(n_positions))

        def _run():
            return attribute_portfolio_pnl(
                portfolio,
                _BENCH_MARKET,
                _BENCH_MARKET_T1,
                "2025-01-15",
                "2025-01-16",
                "metrics_based",
            )

        preflight = _run()
        assert len(preflight.by_position) == n_positions
        assert not preflight.result_invalid
        assert math.isfinite(preflight.total_pnl.amount)
        result = benchmark.pedantic(_run, rounds=5, warmup_rounds=1)
        assert len(result.by_position) == n_positions
        assert not result.result_invalid
        assert math.isfinite(result.total_pnl.amount)

    @pytest.mark.parametrize(
        "n_scenarios",
        [10, pytest.param(100, marks=pytest.mark.slow)],
        ids=["10-scenarios", "100-scenarios"],
    )
    def test_scenario_pnl_batch_500_deposits(self, benchmark, n_scenarios: int) -> None:
        """Batched scenarios over a typed 500-deposit portfolio."""
        portfolio = Portfolio.from_spec(_BENCH_SPEC_JSON_500)
        scenarios_json = _build_curve_scenario_jsons(n_scenarios)
        batch_json = f"[{','.join(scenarios_json)}]"
        preflight = scenario_pnl_batch(portfolio, batch_json, _BENCH_MARKET)
        assert len(preflight) == n_scenarios
        for item in preflight:
            _assert_deposit_scenario_pnl(item.pnl, 500)
        result = benchmark.pedantic(
            scenario_pnl_batch,
            args=(portfolio, batch_json, _BENCH_MARKET),
            rounds=5,
            warmup_rounds=1,
        )
        assert len(result) == n_scenarios
        for item in result:
            _assert_deposit_scenario_pnl(item.pnl, 500)

    @pytest.mark.parametrize(
        "n_scenarios",
        [10, pytest.param(100, marks=pytest.mark.slow)],
        ids=["10-scenarios", "100-scenarios"],
    )
    def test_scenario_pnl_repeated_500_deposits(self, benchmark, n_scenarios: int) -> None:
        """Repeated single-scenario control for the corresponding batch shape."""
        portfolio = Portfolio.from_spec(_BENCH_SPEC_JSON_500)
        scenarios_json = _build_curve_scenario_jsons(n_scenarios)

        def _run_repeated():
            return tuple(scenario_pnl(portfolio, scenario_json, _BENCH_MARKET) for scenario_json in scenarios_json)

        preflight = _run_repeated()
        assert len(preflight) == n_scenarios
        for item in preflight:
            _assert_deposit_scenario_pnl(item.pnl, 500)
        result = benchmark.pedantic(_run_repeated, rounds=5, warmup_rounds=1)
        assert len(result) == n_scenarios
        for item in result:
            _assert_deposit_scenario_pnl(item.pnl, 500)

    def test_replay_500_deposits_pv_only_20_snapshots(self, benchmark) -> None:
        """PV-only 500-deposit replay through twenty prebuilt snapshots."""
        portfolio = Portfolio.from_spec(_BENCH_SPEC_JSON_500)
        snapshots_json = _build_replay_snapshots_json(20)

        def _run():
            return replay_portfolio(
                portfolio,
                snapshots_json,
                ('{"mode":"pv_only","valuation_options":{"strict_risk":true,"metrics":{"mode":"only","metrics":[]}}}'),
            )

        def _check(result):
            assert not result.skipped_dates
            assert len(result.steps) == 20
            for step in result.steps:
                valuation = PortfolioValuation.from_json(json.dumps(step["valuation"]))
                _assert_deposit_valuation(valuation, 500, standard_risk=False)

        _check(_run())
        _check(benchmark.pedantic(_run, rounds=5, warmup_rounds=1))

    @pytest.mark.slow
    def test_value_3000_deposits_standard_risk(self, benchmark) -> None:
        """Strict standard-risk valuation for a typed 3,000-deposit portfolio."""
        portfolio = Portfolio.from_spec(_build_portfolio_spec_json(3_000))
        _assert_deposit_valuation(value_portfolio(portfolio, _BENCH_MARKET, strict_risk=True), 3_000)
        result = benchmark.pedantic(
            value_portfolio,
            args=(portfolio, _BENCH_MARKET),
            kwargs={"strict_risk": True},
            rounds=5,
            warmup_rounds=1,
        )
        _assert_deposit_valuation(result, 3_000)

    @pytest.mark.parametrize(
        "n_positions",
        [3_000, pytest.param(25_000, marks=pytest.mark.slow)],
        ids=["3000-deposits", "25000-deposits"],
    )
    def test_value_deposit_portfolio_pv_only(self, benchmark, n_positions: int) -> None:
        """Deposit-only PV control with typed inputs and typed output."""
        portfolio = Portfolio.from_spec(_build_portfolio_spec_json(n_positions))
        preflight = value_portfolio(portfolio, _BENCH_MARKET, strict_risk=True, metrics=[])
        _assert_deposit_valuation(preflight, n_positions, standard_risk=False)
        result = benchmark.pedantic(
            value_portfolio,
            args=(portfolio, _BENCH_MARKET),
            kwargs={"strict_risk": True, "metrics": []},
            rounds=5,
            warmup_rounds=1,
        )
        _assert_deposit_valuation(result, n_positions, standard_risk=False)


@pytest.mark.perf
class TestPortfolioRiskInputBenchmarks:
    """Portfolio risk bindings across list and contiguous NumPy inputs."""

    def test_parametric_report_256x256(self, benchmark) -> None:
        request = {
            "position_ids": _RISK_POSITION_IDS,
            "source": {"kind": "parametric", "weights": _RISK_WEIGHTS, "covariance": _RISK_COVARIANCE_LIST},
        }
        benchmark(position_risk_report, request)

    def test_historical_report_200x1000(self, benchmark) -> None:
        request = {
            "position_ids": _HISTORICAL_POSITION_IDS,
            "source": {"kind": "historical", "pnls": _HISTORICAL_PNLS_LIST},
        }
        benchmark(position_risk_report, request)

    def test_stress_attribution_list_200x1000(self, benchmark) -> None:
        benchmark(
            build_stress_attribution,
            _HISTORICAL_POSITION_IDS,
            _HISTORICAL_PNLS_LIST,
        )

    def test_stress_attribution_numpy_200x1000(self, benchmark) -> None:
        benchmark(
            build_stress_attribution,
            _HISTORICAL_POSITION_IDS,
            _HISTORICAL_PNLS_NUMPY,
        )


# Valuations domain


@pytest.mark.perf
class TestValuationsBenchmarks:
    """Deposit JSON validation, successful-result round-trip and metric listing."""

    def test_validate_deposit_instrument_json(self, benchmark) -> None:
        result = benchmark(validate_instrument_json, DEPOSIT_INSTRUMENT_JSON)
        assert json.loads(result)["instrument"]["type"] == "deposit"

    def test_list_standard_metrics(self, benchmark) -> None:
        result = benchmark(list_standard_metrics)
        assert "dv01" in result

    def test_deposit_valuation_result_round_trip(self, benchmark) -> None:
        valuation = price_instrument(DEPOSIT_INSTRUMENT_JSON, _BENCH_MARKET, "2025-01-15", metrics=["dv01"])
        assert valuation.instrument_id == "DEP-1"
        assert valuation.currency == "USD"
        assert math.isfinite(valuation.price)
        assert math.isfinite(valuation.metrics["dv01"])

        def _round_trip():
            return ValuationResult.from_json(valuation.to_json())

        assert _round_trip() == valuation
        result = benchmark(_round_trip)
        assert result == valuation


# Scenarios domain


@pytest.mark.perf
class TestScenariosBenchmarks:
    """Template registry and scenario parsing."""

    def test_list_builtin_templates(self, benchmark) -> None:
        benchmark(list_builtin_templates)

    def test_build_from_template(self, benchmark) -> None:
        templates = list_builtin_templates()
        if not templates:
            pytest.skip("no built-in templates available")
        benchmark(build_from_template, templates[0])

    def test_parse_and_validate(self, benchmark) -> None:
        templates = list_builtin_templates()
        if not templates:
            pytest.skip("no built-in templates available")
        spec_json = build_from_template(templates[0]).to_json()

        def _parse_validate():
            parsed = ScenarioSpec.from_json(spec_json)
            validate_scenario_spec(parsed.to_json())

        benchmark(_parse_validate)

    def test_scenario_spec_constructor(self, benchmark) -> None:
        operations = [OperationSpec.stmt_forecast_assign("revenue", 120.0)]
        benchmark(
            ScenarioSpec,
            "bench-scenario",
            operations,
            "Bench",
            "A benchmark scenario",
        )

    def test_compose_scenarios(self, benchmark) -> None:
        specs = [
            ScenarioSpec(
                "s1",
                [OperationSpec.stmt_forecast_assign("revenue", 120.0)],
                priority=0,
            ),
            ScenarioSpec(
                "s2",
                [OperationSpec.stmt_forecast_assign("cogs", 70.0)],
                priority=1,
            ),
        ]
        benchmark(compose_scenarios, specs)

    def test_list_template_components(self, benchmark) -> None:
        templates = list_builtin_templates()
        if not templates:
            pytest.skip("no built-in templates available")
        benchmark(list_template_components, templates[0])
