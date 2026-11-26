"""Heston Monte Carlo golden builders.

Each fixture prices an instrument the suite already references under
Black-Scholes on Finstack's Heston Monte Carlo model, against a QuantLib
Heston value that involves no simulation: the semi-analytic European and
discrete geometric Asian engines, and the finite-difference barrier engine.

The tolerance is fixed here before any comparison: three standard errors of a
plain estimator at the fixture's path count, plus the reference's own error
where it has one. The payoff's standard deviation comes from an independent
simulation in this module (full-truncation Euler, daily steps), rounded up to
two significant figures so the fixture does not depend on the last digits of
a sample statistic. Finstack's time-stepping bias (quadratic-exponential
scheme, 252 steps a year) is not in the bound, so a failure is a finding
about that bias.
"""

from __future__ import annotations

from collections.abc import Callable
import copy
import math
from typing import Any

import numpy as np
import QuantLib as ql  # type: ignore[import-not-found]  # noqa: N813

from .common import VALUATION_DATE, ql_date
from .options import build_barrier_option, build_geometric_asian_option
from .stochastic_vol import (
    DIVIDEND_YIELD as INDEX_DIVIDEND_YIELD,
    HESTON,
    RATE as INDEX_RATE,
    SPOT as INDEX_SPOT,
    build_heston_equity_call,
)

CAPTURED_ON = "2026-11-25"
PATHS = 50_000
DEVIATION_SAMPLES = 1 << 18
DEVIATION_SEED = 42
DAYS = 365
FIXING_DAYS = (91, 183, 275, 365)
# Spot, strike, rate and dividend yield of the Black-Scholes Asian and barrier fixtures.
SPOT, STRIKE, RATE, DIVIDEND_YIELD, BARRIER = 100.0, 100.0, 0.04, 0.01, 80.0
EXPIRY = ql.Date(30, 4, 2027)
# Finite-difference grids (time, spot, variance) for the barrier reference and its refinement.
BARRIER_GRID = (200, 400, 100)
BARRIER_FINE_GRID = (400, 800, 200)


def _process(spot: float, rate: float, dividend_yield: float) -> ql.HestonProcess:
    evaluation_date = ql_date(VALUATION_DATE)
    ql.Settings.instance().evaluationDate = evaluation_date
    day_count = ql.Actual365Fixed()
    return ql.HestonProcess(
        ql.YieldTermStructureHandle(ql.FlatForward(evaluation_date, rate, day_count, ql.Continuous)),
        ql.YieldTermStructureHandle(ql.FlatForward(evaluation_date, dividend_yield, day_count, ql.Continuous)),
        ql.QuoteHandle(ql.SimpleQuote(spot)),
        HESTON["v0"],
        HESTON["kappa"],
        HESTON["theta"],
        HESTON["sigma_v"],
        HESTON["rho"],
    )


def _round_up(value: float) -> float:
    """Round a positive number up to two significant figures."""
    scale = 10.0 ** (math.floor(math.log10(value)) - 1)
    return math.ceil(value / scale) * scale


def _payoff_deviation(
    payoff: Callable[[np.ndarray], np.ndarray], *, spot: float, rate: float, dividend_yield: float
) -> float:
    """Standard deviation of a discounted one-year payoff of daily Heston spot paths.

    `payoff` maps the simulated spots on the fixing days (one column per day)
    to the undiscounted payoff of each path.
    """
    rng = np.random.default_rng(DEVIATION_SEED)
    dt = 1.0 / DAYS
    log_spot = np.full(DEVIATION_SAMPLES, math.log(spot))
    variance = np.full(DEVIATION_SAMPLES, HESTON["v0"])
    observed = []
    correlation = HESTON["rho"]
    for day in range(1, DAYS + 1):
        first = rng.standard_normal(DEVIATION_SAMPLES)
        second = correlation * first + math.sqrt(1.0 - correlation**2) * rng.standard_normal(DEVIATION_SAMPLES)
        positive = np.maximum(variance, 0.0)
        log_spot += (rate - dividend_yield - 0.5 * positive) * dt + np.sqrt(positive * dt) * first
        variance += (
            HESTON["kappa"] * (HESTON["theta"] - positive) * dt + HESTON["sigma_v"] * np.sqrt(positive * dt) * second
        )
        if day in FIXING_DAYS:
            observed.append(np.exp(log_spot))
    values = math.exp(-rate) * payoff(np.column_stack(observed))
    return _round_up(float(values.std()))


def _retarget(
    base: dict[str, Any],
    *,
    name: str,
    description: str,
    product: str,
    reference: float,
    source_detail: str,
    deviation: float,
    quantity: float = 1.0,
    reference_error: float = 0.0,
    reference_error_source: str = "",
) -> dict[str, Any]:
    fixture = copy.deepcopy(base)
    fixture["model"] = "monte_carlo_heston"
    meta = fixture["metadata"]
    meta["name"] = name
    meta["description"] = description
    meta["source_detail"] = (
        f"{source_detail} Reference value {reference!r}. Discounted payoff standard deviation per unit at most "
        f"{deviation!r}, from {DEVIATION_SAMPLES} independent full-truncation Euler paths with daily steps."
    )
    meta["regen_command"] = f"uv run python -m scripts.golden.quantlib.generate --product {product}"
    meta["captured_on"] = meta["last_reviewed_on"] = CAPTURED_ON
    spec = fixture["instrument"]["instrument"]["spec"]
    spec["id"] = name.upper().replace("_", "-")
    spec["instrument_pricing_overrides"] = {"model_config": {"mc_paths": PATHS}}
    prices = fixture["market"]["data"]["prices"]
    for key, value in HESTON.items():
        prices[f"HESTON_{key.upper()}"] = {"unitless": value}
    fixture["market"]["data"]["prices"] = dict(sorted(prices.items()))
    fixture["expected"] = {"npv": reference}
    statistical = 3.0 * quantity * deviation / math.sqrt(PATHS)
    reason = f"Three standard errors of a plain {PATHS}-path estimator from the payoff's own dispersion"
    if reference_error > 0.0:
        reason += f", plus the reference's own error of {reference_error!r} ({reference_error_source})"
    fixture["tolerances"] = {
        "npv": {
            "abs": statistical + reference_error,
            "tolerance_reason": f"{reason}. Finstack's time-stepping bias is not in the bound.",
        }
    }
    return fixture


def build_heston_equity_call_monte_carlo() -> dict[str, Any]:
    """Build the at-the-money Heston equity call priced by Monte Carlo."""
    base = build_heston_equity_call()
    strike = base["instrument"]["instrument"]["spec"]["strike"]
    quantity = base["instrument"]["instrument"]["spec"]["quantity"]
    deviation = _payoff_deviation(
        lambda spots: np.maximum(spots[:, -1] - strike, 0.0),
        spot=INDEX_SPOT,
        rate=INDEX_RATE,
        dividend_yield=INDEX_DIVIDEND_YIELD,
    )
    return _retarget(
        base,
        name="spx_heston_atm_call_1y_monte_carlo_quantlib",
        description="One-year at-the-money Heston equity call priced by Monte Carlo against the semi-analytic value.",
        product="heston_equity_call_monte_carlo",
        reference=base["expected"]["npv"],
        source_detail=base["metadata"]["source_detail"],
        deviation=deviation,
        quantity=quantity,
    )


def build_heston_geometric_asian_option_monte_carlo() -> dict[str, Any]:
    """Build the discrete geometric Asian call priced by Heston Monte Carlo."""
    evaluation_date = ql_date(VALUATION_DATE)
    process = _process(SPOT, RATE, DIVIDEND_YIELD)
    option = ql.DiscreteAveragingAsianOption(
        ql.Average.Geometric,
        [evaluation_date + days for days in FIXING_DAYS],
        ql.PlainVanillaPayoff(ql.Option.Call, STRIKE),
        ql.EuropeanExercise(EXPIRY),
    )
    option.setPricingEngine(ql.AnalyticDiscreteGeometricAveragePriceAsianHestonEngine(process))
    deviation = _payoff_deviation(
        lambda spots: np.maximum(np.exp(np.log(spots).mean(axis=1)) - STRIKE, 0.0),
        spot=SPOT,
        rate=RATE,
        dividend_yield=DIVIDEND_YIELD,
    )
    return _retarget(
        build_geometric_asian_option(),
        name="spx_heston_geometric_asian_call_1y_monte_carlo_quantlib",
        description="One-year discrete geometric-average equity call priced by Heston Monte Carlo.",
        product="heston_geometric_asian_option_monte_carlo",
        reference=option.NPV(),
        source_detail=(
            f"QuantLib {ql.__version__} AnalyticDiscreteGeometricAveragePriceAsianHestonEngine: spot {SPOT}, strike "
            f"{STRIKE}, four quarterly fixings, flat continuous Act/365F rate {RATE} and dividend yield "
            f"{DIVIDEND_YIELD}, v0 {HESTON['v0']}, kappa {HESTON['kappa']}, theta {HESTON['theta']}, vol-of-vol "
            f"{HESTON['sigma_v']}, rho {HESTON['rho']}."
        ),
        deviation=deviation,
    )


def _heston_barrier(grid: tuple[int, int, int]) -> float:
    option = ql.BarrierOption(
        ql.Barrier.DownOut,
        BARRIER,
        0.0,
        ql.PlainVanillaPayoff(ql.Option.Call, STRIKE),
        ql.EuropeanExercise(EXPIRY),
    )
    option.setPricingEngine(ql.FdHestonBarrierEngine(ql.HestonModel(_process(SPOT, RATE, DIVIDEND_YIELD)), *grid))
    return option.NPV()


def build_heston_barrier_option_monte_carlo() -> dict[str, Any]:
    """Build the continuous down-and-out call priced by Heston Monte Carlo."""
    reference = _heston_barrier(BARRIER_FINE_GRID)
    refinement = abs(reference - _heston_barrier(BARRIER_GRID))
    deviation = _payoff_deviation(
        lambda spots: np.maximum(spots[:, -1] - STRIKE, 0.0),
        spot=SPOT,
        rate=RATE,
        dividend_yield=DIVIDEND_YIELD,
    )
    return _retarget(
        build_barrier_option(),
        name="spx_heston_down_out_call_1y_monte_carlo_quantlib",
        description="One-year continuous down-and-out equity call priced by Heston Monte Carlo.",
        product="heston_barrier_option_monte_carlo",
        reference=reference,
        source_detail=(
            f"QuantLib {ql.__version__} FdHestonBarrierEngine on a {BARRIER_FINE_GRID} (time, spot, variance) grid: "
            f"spot {SPOT}, strike {STRIKE}, barrier {BARRIER}, flat continuous Act/365F rate {RATE} and dividend "
            f"yield {DIVIDEND_YIELD}, v0 {HESTON['v0']}, kappa {HESTON['kappa']}, theta {HESTON['theta']}, "
            f"vol-of-vol {HESTON['sigma_v']}, rho {HESTON['rho']}. The knock-out payoff never exceeds the "
            "vanilla call payoff, whose dispersion bounds it."
        ),
        deviation=deviation,
        reference_error=refinement,
        reference_error_source=f"its change from the {BARRIER_GRID} grid",
    )


HESTON_MONTE_CARLO_BUILDERS: dict[str, tuple[str, Callable[[], dict[str, Any]]]] = {
    "heston_equity_call_monte_carlo": (
        "equity_option/spx_heston_atm_call_1y_monte_carlo_quantlib.json",
        build_heston_equity_call_monte_carlo,
    ),
    "heston_geometric_asian_option_monte_carlo": (
        "asian_option/spx_heston_geometric_asian_call_1y_monte_carlo_quantlib.json",
        build_heston_geometric_asian_option_monte_carlo,
    ),
    "heston_barrier_option_monte_carlo": (
        "barrier_option/spx_heston_down_out_call_1y_monte_carlo_quantlib.json",
        build_heston_barrier_option_monte_carlo,
    ),
}
