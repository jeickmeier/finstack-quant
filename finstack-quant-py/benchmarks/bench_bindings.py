"""Comprehensive pytest-benchmark suite for all finstack-quant Python binding domains.

Run with pytest-benchmark::

    uv run pytest finstack-quant-py/benchmarks/bench_bindings.py -m "perf and not slow" --benchmark-only

Run the slow release-scale controls as well::

    uv run pytest finstack-quant-py/benchmarks/bench_bindings.py -m perf --benchmark-only

Run with verbose output::

    uv run pytest finstack-quant-py/benchmarks/bench_bindings.py -m perf --benchmark-only -v

Save and compare a portfolio baseline::

    mise run python-bench-portfolio-baseline
    mise run python-bench-portfolio-compare
"""

from __future__ import annotations

from datetime import date, timedelta
from itertools import accumulate
import json
import math

import numpy as np
import pytest

from finstack_quant.analytics import Performance
from finstack_quant.core.currency import Currency
from finstack_quant.core.dates import DayCount, Tenor
from finstack_quant.core.market_data import DiscountCurve, ForwardCurve, FxMatrix, MarketContext
from finstack_quant.core.math import count_consecutive, linalg, stats
from finstack_quant.core.money import Money
from finstack_quant.core.types import Rate
from finstack_quant.margin import (
    CsaSpec,
    FundingConfig,
    MarginUtilization,
    NettingSetId,
    VmCalculator,
    XvaConfig,
)
from finstack_quant.monte_carlo import (
    EuropeanPricer,
    LsmcPricer,
    McEngine,
    PathDependentPricer,
    TimeGrid,
    black_scholes_call,
    black_scholes_put,
    price_european_call,
)
from finstack_quant.portfolio import (
    Portfolio,
    aggregate_full_cashflows,
    aggregate_metrics,
    attribute_portfolio_pnl,
    build_portfolio_from_spec,
    build_stress_attribution,
    historical_var_decomposition_typed,
    parametric_var_decomposition_typed,
    parse_portfolio_spec,
    replay_portfolio,
    scenario_pnl,
    scenario_pnl_batch,
    value_portfolio,
    value_portfolio_typed,
)
from finstack_quant.scenarios import (
    build_from_template,
    build_scenario_spec,
    compose_scenarios,
    list_builtin_templates,
    list_template_components,
    parse_scenario_spec,
    validate_scenario_spec,
)
from finstack_quant.statements import (
    Evaluator,
    FinancialModelSpec,
    ModelBuilder,
    NormalizationConfig,
    normalize,
    parse_formula,
    validate_formula,
)
from finstack_quant.statements_analytics import (
    DependencyTracer,
    backtest_forecast,
    direct_dependencies,
    evaluate_scenario_set,
    explain_formula,
    goal_seek,
    run_sensitivity,
    run_variance,
)
from finstack_quant.valuations.correlation import (
    CopulaSpec,
    CorrelatedBernoulli,
    LatentFactorSpec,
    LatentSingleFactor,
    RecoverySpec,
    correlation_bounds,
    validate_correlation_matrix,
)
from finstack_quant.valuations.instruments import list_standard_metrics, validate_instrument_json

# ---------------------------------------------------------------------------
# Shared data
# ---------------------------------------------------------------------------

RETURNS_10K: list[float] = [0.0004 + (i % 17) * 1e-5 for i in range(10_000)]
RETURNS_10K_ALT: list[float] = [0.0003 + (i % 13) * 1.2e-5 for i in range(10_000)]
PRICES_10K: list[float] = list(accumulate(RETURNS_10K, lambda p, r: p * (1.0 + r), initial=100.0))
PRICES_10K_ALT: list[float] = list(accumulate(RETURNS_10K_ALT, lambda p, r: p * (1.0 + r), initial=100.0))

DATES_252 = [date(2024, 1, 1) + timedelta(days=i) for i in range(252)]
DATES_10K = [date(2000, 1, 1) + timedelta(days=i) for i in range(10_000)]

_PERFORMANCE_10K = Performance.from_arrays(
    DATES_10K,
    [PRICES_10K[:10_000], PRICES_10K_ALT[:10_000]],
    ["ASSET", "BENCH"],
)

DATA_10K: list[float] = [float(i) * 0.01 for i in range(10_000)]

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
    "type": "deposit",
    "spec": {
        "id": "DEP-1",
        "notional": {"amount": 1000000.0, "currency": "USD"},
        "start_date": "2025-01-15",
        "maturity": "2025-06-15",
        "day_count": "Act360",
        "quote_rate": 0.05,
        "discount_curve_id": "USD-OIS",
        "attributes": {},
    },
})

PORTFOLIO_SPEC_JSON = json.dumps({
    "id": "bench-portfolio",
    "as_of": "2025-01-15",
    "base_ccy": "USD",
    "entities": {"ENTITY-1": {"id": "ENTITY-1"}},
    "positions": [
        {
            "position_id": "POS-1",
            "entity_id": "ENTITY-1",
            "instrument_id": "DEP-1",
            "instrument_spec": {
                "type": "deposit",
                "spec": {
                    "id": "DEP-1",
                    "notional": {"amount": 1000000.0, "currency": "USD"},
                    "start_date": "2025-01-15",
                    "maturity": "2025-06-15",
                    "day_count": "Act360",
                    "quote_rate": 0.05,
                    "discount_curve_id": "USD-OIS",
                    "attributes": {},
                },
            },
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
                    "notional": {"amount": 1_000_000.0 + i, "currency": "USD"},
                    "start_date": "2025-01-15",
                    "maturity": "2025-06-15",
                    "day_count": "Act360",
                    "quote_rate": 0.05,
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
        "base_ccy": "USD",
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


def _build_curve_scenario_jsons(n_scenarios: int) -> tuple[str, ...]:
    """Build validated parallel-shift scenarios before benchmark timing begins."""
    return tuple(
        build_scenario_spec(
            f"bench-parallel-{index}",
            (
                '[{"kind":"curve_parallel_bp","curve_kind":"discount",'
                f'"curve_id":"USD-OIS","discount_curve_id":null,"bp":{float(index + 1)}}}]'
            ),
            priority=index,
        )
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
_RISK_COVARIANCE_NUMPY = np.asarray(_RISK_COVARIANCE_LIST, dtype=np.float64)

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
    "mode": "Diagonal",
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


# ===================================================================
# Core domain
# ===================================================================


@pytest.mark.perf
class TestCoreBenchmarks:
    """Core primitives: currency, money, dates, curves, math."""

    def test_currency_creation(self, benchmark) -> None:
        benchmark(Currency, "USD")

    def test_money_add_sub(self, benchmark) -> None:
        usd = Currency("USD")
        a = Money(100.0, usd)
        b = Money(1.0, usd)

        def _add_sub():
            x = a + b
            return x - b

        benchmark(_add_sub)

    def test_daycount_year_fraction(self, benchmark) -> None:
        dc = DayCount.ACT_360
        start = date(2024, 1, 1)
        end = date(2025, 1, 1)
        benchmark(dc.year_fraction, start, end)

    def test_discount_curve_df(self, benchmark) -> None:
        curve = DiscountCurve(
            "USD-BENCH",
            date(2024, 1, 1),
            [(0.0, 1.0), (1.0, 0.95), (5.0, 0.75), (10.0, 0.50)],
            day_count="act_365f",
        )
        benchmark(curve.df, 2.5)

    def test_cholesky_5x5(self, benchmark) -> None:
        benchmark(linalg.cholesky_decomposition, SPD_5X5)

    def test_forward_curve_rate(self, benchmark) -> None:
        curve = ForwardCurve(
            "USD-SOFR-3M",
            0.25,
            date(2024, 1, 1),
            [(0.0, 0.05), (1.0, 0.052), (5.0, 0.055), (10.0, 0.06)],
        )
        benchmark(curve.rate, 2.5)

    def test_fx_matrix_rate(self, benchmark) -> None:
        fx = FxMatrix()
        fx.set_quote("USD", "EUR", 0.92)
        ref_date = date(2024, 6, 15)
        benchmark(fx.rate, "USD", "EUR", ref_date)

    def test_stats_mean_variance(self, benchmark) -> None:
        def _mean_var():
            m = stats.mean(DATA_10K)
            v = stats.variance(DATA_10K)
            return m, v

        benchmark(_mean_var)

    def test_tenor_parsing(self, benchmark) -> None:
        benchmark(Tenor.parse, "3M")

    def test_rate_conversions(self, benchmark) -> None:
        def _round_trip():
            r = Rate(0.05)
            p = r.as_percent
            b = r.as_bps
            r2 = Rate.from_percent(p)
            r3 = Rate.from_bps(b)
            return r2, r3

        benchmark(_round_trip)


# ===================================================================
# Analytics domain
# ===================================================================


@pytest.mark.perf
class TestAnalyticsBenchmarks:
    """Performance analytics: returns, drawdowns, risk metrics."""

    def test_performance_construction(self, benchmark) -> None:
        n = 252
        dates = [date(2024, 1, 1) + timedelta(days=i) for i in range(n)]
        prices = [100.0 + i * 0.1 for i in range(n)]
        benchmark(Performance.from_arrays, dates, [prices], ["BENCH"])

    def test_sharpe(self, benchmark) -> None:
        benchmark(_PERFORMANCE_10K.sharpe)

    def test_to_drawdown_series(self, benchmark) -> None:
        benchmark(_PERFORMANCE_10K.drawdown_series)

    def test_rolling_sharpe(self, benchmark) -> None:
        benchmark(_PERFORMANCE_10K.rolling_sharpe, 0, 63)

    def test_volatility(self, benchmark) -> None:
        benchmark(_PERFORMANCE_10K.volatility)

    def test_simple_returns(self, benchmark) -> None:
        benchmark(_PERFORMANCE_10K.returns)

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

    def test_count_consecutive(self, benchmark) -> None:
        benchmark(count_consecutive, RETURNS_10K)


# ===================================================================
# Correlation domain
# ===================================================================


@pytest.mark.perf
class TestCorrelationBenchmarks:
    """Copula and factor model computations."""

    def test_copula_build_and_conditional(self, benchmark) -> None:
        def _build_and_query():
            copula = CopulaSpec.gaussian().build()
            return copula.conditional_default_prob(-1.5, [0.0], 0.3)

        benchmark(_build_and_query)

    def test_correlated_bernoulli(self, benchmark) -> None:
        def _construct_and_query():
            cb = CorrelatedBernoulli(0.3, 0.5, 0.2)
            return cb.joint_probabilities()

        benchmark(_construct_and_query)

    def test_recovery_model(self, benchmark) -> None:
        def _build_and_query():
            spec = RecoverySpec.constant(0.4)
            model = spec.build()
            return model.conditional_recovery(-1.0)

        benchmark(_build_and_query)

    def test_factor_model(self, benchmark) -> None:
        def _build_and_query():
            spec = LatentFactorSpec.single_factor(0.15, 0.5)
            model = spec.build()
            return model.diagonal_factor_contribution(0, 1.0)

        benchmark(_build_and_query)

    def test_single_factor_model_construction(self, benchmark) -> None:
        benchmark(LatentSingleFactor, 0.15, 0.5)

    def test_correlation_bounds(self, benchmark) -> None:
        benchmark(correlation_bounds, 0.3, 0.5)

    def test_validate_correlation_matrix(self, benchmark) -> None:
        benchmark(validate_correlation_matrix, CORR_5X5_FLAT, 5)


# ===================================================================
# Monte Carlo domain
# ===================================================================


@pytest.mark.perf
class TestMonteCarloBenchmarks:
    """Option pricing: analytical and simulation."""

    def test_price_european_call_50k(self, benchmark) -> None:
        benchmark.pedantic(
            price_european_call,
            kwargs={
                "spot": 100.0,
                "strike": 100.0,
                "rate": 0.05,
                "div_yield": 0.0,
                "vol": 0.2,
                "expiry": 1.0,
                "num_paths": 50_000,
                "seed": 42,
                "num_steps": 252,
            },
            rounds=5,
            warmup_rounds=1,
        )

    def test_black_scholes_call(self, benchmark) -> None:
        benchmark(black_scholes_call, 100.0, 100.0, 0.05, 0.0, 0.2, 1.0)

    def test_mc_engine_european(self, benchmark) -> None:
        engine = McEngine(10_000, TimeGrid(1.0, 252), seed=42)

        def _price():
            return engine.price_european_call(100.0, 100.0, 0.05, 0.0, 0.2)

        benchmark.pedantic(_price, rounds=5, warmup_rounds=1)

    def test_lsmc_american_put(self, benchmark) -> None:
        pricer = LsmcPricer(num_paths=5_000, seed=42)

        def _price():
            return pricer.price_american_put(
                spot=100.0,
                strike=100.0,
                rate=0.05,
                div_yield=0.0,
                vol=0.3,
                expiry=1.0,
                num_steps=50,
            )

        benchmark.pedantic(_price, rounds=5, warmup_rounds=1)

    def test_european_pricer(self, benchmark) -> None:
        pricer = EuropeanPricer(num_paths=10_000, seed=42)

        def _price():
            return pricer.price_call(
                spot=100.0,
                strike=100.0,
                rate=0.05,
                div_yield=0.0,
                vol=0.2,
                expiry=1.0,
                num_steps=252,
            )

        benchmark.pedantic(_price, rounds=5, warmup_rounds=1)

    def test_path_dependent_asian(self, benchmark) -> None:
        pricer = PathDependentPricer(num_paths=5_000, seed=42)

        def _price():
            return pricer.price_asian_call(
                spot=100.0,
                strike=100.0,
                rate=0.05,
                div_yield=0.0,
                vol=0.2,
                expiry=1.0,
                num_steps=50,
            )

        benchmark.pedantic(_price, rounds=5, warmup_rounds=1)

    def test_black_scholes_put(self, benchmark) -> None:
        benchmark(black_scholes_put, 100.0, 100.0, 0.05, 0.0, 0.2, 1.0)


# ===================================================================
# Margin domain
# ===================================================================


@pytest.mark.perf
class TestMarginBenchmarks:
    """VM/IM margin calculations."""

    def test_csa_spec_construction(self, benchmark) -> None:
        benchmark(CsaSpec.usd_regulatory)

    def test_vm_calculate(self, benchmark) -> None:
        csa = CsaSpec.usd_regulatory()
        calc = VmCalculator(csa)
        benchmark(calc.calculate, 1_000_000.0, 0.0, "USD", 2024, 6, 15)

    def test_netting_set_id(self, benchmark) -> None:
        def _create_ids():
            b = NettingSetId.bilateral("CPTY-1", "CSA-001")
            c = NettingSetId.cleared("LCH")
            return b, c

        benchmark(_create_ids)

    def test_xva_config(self, benchmark) -> None:
        benchmark(XvaConfig)

    def test_funding_config(self, benchmark) -> None:
        benchmark(FundingConfig, 50.0, 30.0)

    def test_margin_utilization(self, benchmark) -> None:
        benchmark(MarginUtilization, 1_200_000.0, 1_000_000.0, "USD")


# ===================================================================
# Statements domain
# ===================================================================


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

    def test_validate_formula(self, benchmark) -> None:
        benchmark(validate_formula, "revenue + cogs")

    def test_normalization(self, benchmark) -> None:
        config = NormalizationConfig("gross_profit")

        def _normalize():
            return normalize(_EVAL_RESULT, config)

        benchmark(_normalize)


# ===================================================================
# Statements Analytics domain
# ===================================================================


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
                target_node="gross_profit",
                target_period="2025Q1",
                target_value=50.0,
                driver_node="revenue",
                driver_period="2025Q1",
                update_model=False,
            )

        benchmark.pedantic(_seek, rounds=10, warmup_rounds=1)

    def test_goal_seek_typed(self, benchmark) -> None:
        def _seek():
            return goal_seek(
                _MODEL_SPEC,
                target_node="gross_profit",
                target_period="2025Q1",
                target_value=50.0,
                driver_node="revenue",
                driver_period="2025Q1",
                update_model=False,
            )

        benchmark.pedantic(_seek, rounds=10, warmup_rounds=1)

    def test_dependency_tracer_json(self, benchmark) -> None:
        def _trace():
            tree = DependencyTracer(_MODEL_JSON).dependency_tree("gross_profit")
            deps = direct_dependencies(_MODEL_JSON, "gross_profit")
            return tree, deps

        benchmark(_trace)

    def test_dependency_tracer_typed(self, benchmark) -> None:
        def _trace():
            tree = DependencyTracer(_MODEL_SPEC).dependency_tree("gross_profit")
            deps = direct_dependencies(_MODEL_SPEC, "gross_profit")
            return tree, deps

        benchmark(_trace)

    def test_explain_formula_json(self, benchmark) -> None:
        benchmark(explain_formula, _MODEL_JSON, _EVAL_RESULT_JSON, "gross_profit", "2025Q1")

    def test_explain_formula_typed(self, benchmark) -> None:
        benchmark(explain_formula, _MODEL_SPEC, _EVAL_RESULT, "gross_profit", "2025Q1")


# ===================================================================
# Portfolio domain
# ===================================================================


@pytest.mark.perf
class TestPortfolioBenchmarks:
    """Portfolio JSON pipeline: parse + build."""

    def test_parse_and_build(self, benchmark) -> None:
        def _parse_build():
            spec = parse_portfolio_spec(PORTFOLIO_SPEC_JSON)
            return build_portfolio_from_spec(spec)

        benchmark.pedantic(_parse_build, rounds=10, warmup_rounds=1)

    def test_portfolio_spec_round_trip(self, benchmark) -> None:
        benchmark(parse_portfolio_spec, PORTFOLIO_SPEC_JSON)


@pytest.mark.perf
class TestPortfolioCompoundWorkflow:
    """Compound workflows — each function used to rebuild the portfolio.

    These measure the realistic calling pattern (value + metrics + cashflows)
    and compare the JSON-string path (old behavior) against the typed
    :class:`Portfolio` / :class:`MarketContext` fast path.
    """

    def test_json_path_value_metrics_cashflows(self, benchmark) -> None:
        """JSON inputs, 500-position portfolio: every call re-parses + rebuilds."""

        def _run():
            val = value_portfolio(_BENCH_SPEC_JSON_500, _BENCH_MARKET_JSON)
            agg = aggregate_metrics(val, "USD", _BENCH_MARKET_JSON, "2025-01-15")
            cf = aggregate_full_cashflows(_BENCH_SPEC_JSON_500, _BENCH_MARKET_JSON)
            return val, agg, cf

        benchmark.pedantic(_run, rounds=5, warmup_rounds=1)

    def test_typed_pipeline_500_value_metrics_cashflows(self, benchmark) -> None:
        """Typed inputs, 500 positions: Portfolio + MarketContext built once."""
        portfolio = Portfolio.from_spec(_BENCH_SPEC_JSON_500)

        def _run():
            val = value_portfolio_typed(portfolio, _BENCH_MARKET)
            agg = aggregate_metrics(val, "USD", _BENCH_MARKET, "2025-01-15")
            cf = aggregate_full_cashflows(portfolio, _BENCH_MARKET)
            return val, agg, cf

        benchmark.pedantic(_run, rounds=5, warmup_rounds=1)

    def test_typed_portfolio_from_spec(self, benchmark) -> None:
        """One-time cost of building a typed Portfolio from a 500-position spec."""
        benchmark(Portfolio.from_spec, _BENCH_SPEC_JSON_500)

    def test_value_portfolio_typed_inputs_json_result_500(self, benchmark) -> None:
        """Typed inputs with the backward-compatible JSON result, 500 positions."""
        portfolio = Portfolio.from_spec(_BENCH_SPEC_JSON_500)
        benchmark(value_portfolio, portfolio, _BENCH_MARKET)

    def test_value_portfolio_typed_result_500(self, benchmark) -> None:
        """Typed input and output path, avoiding valuation JSON serialization."""
        portfolio = Portfolio.from_spec(_BENCH_SPEC_JSON_500)
        benchmark(value_portfolio_typed, portfolio, _BENCH_MARKET)

    def test_value_portfolio_json_500(self, benchmark) -> None:
        """Pure value_portfolio on the JSON path, 500 positions."""
        benchmark(value_portfolio, _BENCH_SPEC_JSON_500, _BENCH_MARKET_JSON)


@pytest.mark.perf
class TestPortfolioReleaseControls:
    """Representative release-scale controls for the typed portfolio bindings."""

    @pytest.mark.parametrize(
        "n_positions",
        [40, pytest.param(120, marks=pytest.mark.slow)],
        ids=["40-positions", "120-positions"],
    )
    def test_metrics_based_attribution(self, benchmark, n_positions: int) -> None:
        """Metrics attribution over typed portfolios at 40 and 120 positions."""
        portfolio = Portfolio.from_spec(_build_portfolio_spec_json(n_positions))
        benchmark.pedantic(
            attribute_portfolio_pnl,
            args=(
                portfolio,
                _BENCH_MARKET,
                _BENCH_MARKET_T1,
                "2025-01-15",
                "2025-01-16",
                "MetricsBased",
            ),
            rounds=1,
            warmup_rounds=0,
        )

    @pytest.mark.parametrize(
        "n_scenarios",
        [10, pytest.param(100, marks=pytest.mark.slow)],
        ids=["10-scenarios", "100-scenarios"],
    )
    def test_scenario_pnl_batch_500(self, benchmark, n_scenarios: int) -> None:
        """One-shot scenarios over a typed 500-position portfolio."""
        portfolio = Portfolio.from_spec(_BENCH_SPEC_JSON_500)
        scenarios_json = _build_curve_scenario_jsons(n_scenarios)
        batch_json = f"[{','.join(scenarios_json)}]"
        benchmark.pedantic(
            scenario_pnl_batch,
            args=(portfolio, batch_json, _BENCH_MARKET),
            rounds=1,
            warmup_rounds=0,
        )

    @pytest.mark.parametrize(
        "n_scenarios",
        [10, pytest.param(100, marks=pytest.mark.slow)],
        ids=["10-scenarios", "100-scenarios"],
    )
    def test_scenario_pnl_repeated_500(self, benchmark, n_scenarios: int) -> None:
        """Repeated single-scenario control for the corresponding batch shape."""
        portfolio = Portfolio.from_spec(_BENCH_SPEC_JSON_500)
        scenarios_json = _build_curve_scenario_jsons(n_scenarios)

        def _run_repeated():
            return tuple(scenario_pnl(portfolio, scenario_json, _BENCH_MARKET) for scenario_json in scenarios_json)

        benchmark.pedantic(_run_repeated, rounds=1, warmup_rounds=0)

    def test_replay_pv_only_20_snapshots(self, benchmark) -> None:
        """PV-only replay through twenty prebuilt dated market snapshots."""
        portfolio = Portfolio.from_spec(_BENCH_SPEC_JSON_500)
        snapshots_json = _build_replay_snapshots_json(20)
        benchmark.pedantic(
            replay_portfolio,
            args=(
                portfolio,
                snapshots_json,
                ('{"mode":"PvOnly","valuation_options":{"strict_risk":false,"metrics":{"mode":"only","metrics":[]}}}'),
            ),
            rounds=1,
            warmup_rounds=0,
        )

    @pytest.mark.slow
    def test_value_portfolio_standard_risk_3000(self, benchmark) -> None:
        """Default standard-risk valuation for a typed 3,000-position portfolio."""
        portfolio = Portfolio.from_spec(_build_portfolio_spec_json(3_000))
        benchmark.pedantic(
            value_portfolio_typed,
            args=(portfolio, _BENCH_MARKET),
            rounds=1,
            warmup_rounds=0,
        )

    @pytest.mark.parametrize(
        "n_positions",
        [3_000, pytest.param(25_000, marks=pytest.mark.slow)],
        ids=["3000-positions", "25000-positions"],
    )
    def test_value_portfolio_pv_only(self, benchmark, n_positions: int) -> None:
        """PV-only valuation control with typed inputs and typed output."""
        portfolio = Portfolio.from_spec(_build_portfolio_spec_json(n_positions))
        benchmark.pedantic(
            value_portfolio_typed,
            args=(portfolio, _BENCH_MARKET),
            kwargs={"metrics": []},
            rounds=1,
            warmup_rounds=0,
        )


@pytest.mark.perf
class TestPortfolioRiskInputBenchmarks:
    """Portfolio risk bindings across list and contiguous NumPy inputs."""

    def test_parametric_typed_list_256x256(self, benchmark) -> None:
        benchmark(
            parametric_var_decomposition_typed,
            _RISK_POSITION_IDS,
            _RISK_WEIGHTS,
            _RISK_COVARIANCE_LIST,
        )

    def test_parametric_typed_numpy_256x256(self, benchmark) -> None:
        benchmark(
            parametric_var_decomposition_typed,
            _RISK_POSITION_IDS,
            _RISK_WEIGHTS,
            _RISK_COVARIANCE_NUMPY,
        )

    def test_historical_typed_list_200x1000(self, benchmark) -> None:
        benchmark(
            historical_var_decomposition_typed,
            _HISTORICAL_POSITION_IDS,
            _HISTORICAL_PNLS_LIST,
        )

    def test_historical_typed_numpy_200x1000(self, benchmark) -> None:
        benchmark(
            historical_var_decomposition_typed,
            _HISTORICAL_POSITION_IDS,
            _HISTORICAL_PNLS_NUMPY,
        )

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


# ===================================================================
# Valuations domain
# ===================================================================


@pytest.mark.perf
class TestValuationsBenchmarks:
    """Instrument validation and metric listing."""

    def test_validate_instrument_json(self, benchmark) -> None:
        benchmark(validate_instrument_json, DEPOSIT_INSTRUMENT_JSON)

    def test_list_standard_metrics(self, benchmark) -> None:
        benchmark(list_standard_metrics)

    def test_valuation_result_round_trip(self, benchmark) -> None:
        validated = validate_instrument_json(DEPOSIT_INSTRUMENT_JSON)
        benchmark(validate_instrument_json, validated)


# ===================================================================
# Scenarios domain
# ===================================================================


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
        spec_json = build_from_template(templates[0])

        def _parse_validate():
            parsed = parse_scenario_spec(spec_json)
            validate_scenario_spec(parsed)

        benchmark(_parse_validate)

    def test_build_scenario_spec(self, benchmark) -> None:
        ops = json.dumps([
            {"kind": "stmt_forecast_assign", "node_id": "revenue", "value": 120.0},
        ])
        benchmark(build_scenario_spec, "bench-scenario", ops, "Bench", "A benchmark scenario")

    def test_compose_scenarios(self, benchmark) -> None:
        specs = json.dumps([
            {
                "id": "s1",
                "operations": [
                    {"kind": "stmt_forecast_assign", "node_id": "revenue", "value": 120.0},
                ],
                "priority": 0,
            },
            {
                "id": "s2",
                "operations": [
                    {"kind": "stmt_forecast_assign", "node_id": "cogs", "value": 70.0},
                ],
                "priority": 1,
            },
        ])
        benchmark(compose_scenarios, specs)

    def test_list_template_components(self, benchmark) -> None:
        templates = list_builtin_templates()
        if not templates:
            pytest.skip("no built-in templates available")
        benchmark(list_template_components, templates[0])
