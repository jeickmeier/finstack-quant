"""Monte Carlo model golden builders for products that already have a closed-form fixture.

Each fixture reprices an existing instrument with Finstack's Monte Carlo model
and compares it with a QuantLib value. The tolerance is three standard errors
of a plain (no variance reduction) estimator at the fixture's path count,
plus three standard errors of the QuantLib reference where that is itself a
simulation. Finstack's estimators use variance reduction, so the bound is
conservative; it is fixed from the payoff's own dispersion before any
comparison with Finstack.
"""

from __future__ import annotations

from collections.abc import Callable
import copy
import math
from typing import Any

import QuantLib as ql  # type: ignore[import-not-found]  # noqa: N813

from .common import VALUATION_DATE, ql_date
from .options import build_arithmetic_asian_option, build_geometric_asian_option

CAPTURED_ON = "2026-11-24"
SPOT, STRIKE, RATE, DIVIDEND_YIELD, VOLATILITY = 100.0, 100.0, 0.04, 0.01, 0.20
FIXING_DATES = ["2026-07-30", "2026-10-30", "2027-01-30", "2027-04-30"]
EXPIRY = FIXING_DATES[-1]
GEOMETRIC_PATHS = 200_000
ARITHMETIC_PATHS = 1_000_000
REFERENCE_SAMPLES = 1 << 20
REFERENCE_SEED = 42


def _normal_cdf(x: float) -> float:
    return ql.CumulativeNormalDistribution()(x)


def _times() -> list[float]:
    day_count = ql.Actual365Fixed()
    return [day_count.yearFraction(ql_date(VALUATION_DATE), ql_date(date)) for date in FIXING_DATES]


def _geometric_payoff_moments() -> tuple[float, float]:
    """Mean and standard deviation of the undiscounted geometric-average call payoff."""
    times = _times()
    count = len(times)
    mean_log = math.log(SPOT) + (RATE - DIVIDEND_YIELD - 0.5 * VOLATILITY**2) * sum(times) / count
    variance_log = VOLATILITY**2 * sum(min(a, b) for a in times for b in times) / count**2
    sigma = math.sqrt(variance_log)
    d2 = (mean_log - math.log(STRIKE)) / sigma
    first = math.exp(mean_log + 0.5 * variance_log) * _normal_cdf(d2 + sigma) - STRIKE * _normal_cdf(d2)
    second = (
        math.exp(2.0 * mean_log + 2.0 * variance_log) * _normal_cdf(d2 + 2.0 * sigma)
        - 2.0 * STRIKE * math.exp(mean_log + 0.5 * variance_log) * _normal_cdf(d2 + sigma)
        + STRIKE**2 * _normal_cdf(d2)
    )
    return first, math.sqrt(second - first**2)


def _discount() -> float:
    return math.exp(-RATE * _times()[-1])


def _retarget(base: dict[str, Any], *, name: str, description: str, product: str, paths: int) -> dict[str, Any]:
    fixture = copy.deepcopy(base)
    fixture["model"] = "monte_carlo_gbm"
    meta = fixture["metadata"]
    meta["name"] = name
    meta["description"] = description
    meta["regen_command"] = f"uv run python -m scripts.golden.quantlib.generate --product {product}"
    meta["captured_on"] = meta["last_reviewed_on"] = CAPTURED_ON
    spec = fixture["instrument"]["instrument"]["spec"]
    spec["id"] = name.upper().replace("_", "-")
    spec["instrument_pricing_overrides"] = {"model_config": {"mc_paths": paths}}
    return fixture


def build_geometric_asian_option_monte_carlo() -> dict[str, Any]:
    """Build the geometric Asian call fixture priced by Monte Carlo."""
    base = build_geometric_asian_option()
    mean, deviation = _geometric_payoff_moments()
    discount = _discount()
    reference = base["expected"]["npv"]
    if abs(discount * mean - reference) > 1e-9:
        msg = f"geometric payoff mean {discount * mean} disagrees with QuantLib {reference}"
        raise RuntimeError(msg)
    bound = 3.0 * discount * deviation / math.sqrt(GEOMETRIC_PATHS)
    fixture = _retarget(
        base,
        name="spx_geometric_asian_call_1y_monte_carlo_quantlib",
        description="One-year discrete geometric-average equity call priced by Monte Carlo against the closed form.",
        product="geometric_asian_option_monte_carlo",
        paths=GEOMETRIC_PATHS,
    )
    fixture["metadata"]["source_detail"] = (
        f"QuantLib {ql.__version__} AnalyticDiscreteGeometricAveragePriceAsianEngine value {reference!r}; the "
        f"lognormal payoff has standard deviation {deviation!r} before discounting (closed form, checked "
        "against the QuantLib price through its mean)."
    )
    fixture["tolerances"] = {
        "npv": {
            "abs": bound,
            "tolerance_reason": (
                f"Three standard errors of a plain {GEOMETRIC_PATHS}-path estimator, from the payoff's closed-form "
                "standard deviation; Finstack's control variate can only reduce the error."
            ),
        }
    }
    return fixture


def _arithmetic_reference(*, control_variate: bool) -> tuple[float, float]:
    evaluation_date = ql_date(VALUATION_DATE)
    ql.Settings.instance().evaluationDate = evaluation_date
    day_count = ql.Actual365Fixed()
    process = ql.BlackScholesMertonProcess(
        ql.QuoteHandle(ql.SimpleQuote(SPOT)),
        ql.YieldTermStructureHandle(ql.FlatForward(evaluation_date, DIVIDEND_YIELD, day_count, ql.Continuous)),
        ql.YieldTermStructureHandle(ql.FlatForward(evaluation_date, RATE, day_count, ql.Continuous)),
        ql.BlackVolTermStructureHandle(ql.BlackConstantVol(evaluation_date, ql.NullCalendar(), VOLATILITY, day_count)),
    )
    option = ql.DiscreteAveragingAsianOption(
        ql.Average.Arithmetic,
        [ql_date(date) for date in FIXING_DATES],
        ql.PlainVanillaPayoff(ql.Option.Call, STRIKE),
        ql.EuropeanExercise(ql_date(EXPIRY)),
    )
    option.setPricingEngine(
        ql.MCDiscreteArithmeticAPEngine(
            process,
            "pseudorandom",
            controlVariate=control_variate,
            requiredSamples=REFERENCE_SAMPLES,
            seed=REFERENCE_SEED,
        )
    )
    return option.NPV(), option.errorEstimate()


def build_arithmetic_asian_option_monte_carlo() -> dict[str, Any]:
    """Build the arithmetic Asian call fixture priced by Monte Carlo."""
    reference, reference_error = _arithmetic_reference(control_variate=True)
    _, plain_error = _arithmetic_reference(control_variate=False)
    deviation = plain_error * math.sqrt(REFERENCE_SAMPLES)
    bound = 3.0 * (reference_error + deviation / math.sqrt(ARITHMETIC_PATHS))
    fixture = _retarget(
        build_arithmetic_asian_option(),
        name="spx_arithmetic_asian_call_1y_monte_carlo_quantlib",
        description="One-year discrete arithmetic-average equity call priced by Monte Carlo against QuantLib's.",
        product="arithmetic_asian_option_monte_carlo",
        paths=ARITHMETIC_PATHS,
    )
    fixture["metadata"]["source_detail"] = (
        f"QuantLib {ql.__version__} MCDiscreteArithmeticAPEngine, pseudorandom, geometric control variate, "
        f"{REFERENCE_SAMPLES} samples, seed {REFERENCE_SEED}: value {reference!r}, error estimate "
        f"{reference_error!r}. The same engine without the control variate gives a discounted payoff standard "
        f"deviation of {deviation!r}."
    )
    fixture["expected"] = {"npv": reference}
    fixture["tolerances"] = {
        "npv": {
            "abs": bound,
            "tolerance_reason": (
                "Three standard errors of the QuantLib reference plus three standard errors of a plain "
                f"{ARITHMETIC_PATHS}-path estimator, from QuantLib's own payoff standard deviation; Finstack's "
                "control variate can only reduce the error."
            ),
        }
    }
    return fixture


MONTE_CARLO_BUILDERS: dict[str, tuple[str, Callable[[], dict[str, Any]]]] = {
    "geometric_asian_option_monte_carlo": (
        "asian_option/spx_geometric_asian_call_1y_monte_carlo_quantlib.json",
        build_geometric_asian_option_monte_carlo,
    ),
    "arithmetic_asian_option_monte_carlo": (
        "asian_option/spx_arithmetic_asian_call_1y_monte_carlo_quantlib.json",
        build_arithmetic_asian_option_monte_carlo,
    ),
}
