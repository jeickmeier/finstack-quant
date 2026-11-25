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

import numpy as np
import QuantLib as ql  # type: ignore[import-not-found]  # noqa: N813

from .common import (
    SCHEMA,
    VALUATION_DATE,
    constant_vol_surface,
    flat_discount_curve,
    market_snapshot,
    metadata,
    ql_date,
    tolerance,
)
from .fx_exotics import build_fx_barrier_option
from .options import (
    build_arithmetic_asian_option,
    build_barrier_option,
    build_fixed_lookback_option,
    build_floating_lookback_option,
    build_geometric_asian_option,
)

CAPTURED_ON = "2026-11-24"
SPOT, STRIKE, RATE, DIVIDEND_YIELD, VOLATILITY = 100.0, 100.0, 0.04, 0.01, 0.20
FIXING_DATES = ["2026-07-30", "2026-10-30", "2027-01-30", "2027-04-30"]
EXPIRY = FIXING_DATES[-1]
GEOMETRIC_PATHS = 200_000
ARITHMETIC_PATHS = 1_000_000
REFERENCE_SAMPLES = 1 << 20
REFERENCE_SEED = 42
CONTINUOUS_PATHS = 100_000
CLIQUET_PATHS = 100_000
LOOKBACK_SAMPLES = 1 << 22
LOOKBACK_SEED = 42


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


def _call_payoff_root_second_moment(*, forward: float, strike: float, volatility: float, expiry: float) -> float:
    """Root of the second moment of an undiscounted vanilla call payoff under Black-Scholes."""
    deviation = volatility * math.sqrt(expiry)
    d2 = (math.log(forward / strike) - 0.5 * deviation**2) / deviation
    second = (
        forward**2 * math.exp(deviation**2) * _normal_cdf(d2 + 2.0 * deviation)
        - 2.0 * strike * forward * _normal_cdf(d2 + deviation)
        + strike**2 * _normal_cdf(d2)
    )
    return math.sqrt(second)


def _continuous_monitoring_fixture(
    base: dict[str, Any],
    *,
    name: str,
    description: str,
    product: str,
    deviation: float,
    deviation_source: str,
    scale: float = 1.0,
) -> dict[str, Any]:
    """Retarget a continuously monitored closed-form fixture to Monte Carlo.

    Finstack samples the barrier crossing or the running extremum between
    simulation dates from the exact Brownian-bridge law, so its estimator has
    no monitoring bias against the continuous closed form and the tolerance
    is statistical only.
    """
    reference = base["expected"]["npv"]
    bound = 3.0 * scale * deviation / math.sqrt(CONTINUOUS_PATHS)
    fixture = _retarget(base, name=name, description=description, product=product, paths=CONTINUOUS_PATHS)
    fixture["metadata"]["source_detail"] = (
        f"{base['metadata']['source_detail']} Reference value {reference!r}. Discounted payoff standard "
        f"deviation per unit at most {deviation!r}: {deviation_source}"
    )
    fixture["tolerances"] = {
        "npv": {
            "abs": bound,
            "tolerance_reason": (
                f"Three standard errors of a plain {CONTINUOUS_PATHS}-path estimator from the payoff's own "
                "dispersion. Monitoring between simulation dates is sampled exactly, so there is no "
                "discretisation term."
            ),
        }
    }
    return fixture


def build_barrier_option_monte_carlo() -> dict[str, Any]:
    """Build the continuous down-and-out call fixture priced by Monte Carlo."""
    expiry = _times()[-1]
    forward = SPOT * math.exp((RATE - DIVIDEND_YIELD) * expiry)
    deviation = math.exp(-RATE * expiry) * _call_payoff_root_second_moment(
        forward=forward, strike=STRIKE, volatility=VOLATILITY, expiry=expiry
    )
    return _continuous_monitoring_fixture(
        build_barrier_option(),
        name="spx_down_out_call_1y_monte_carlo_quantlib",
        description="One-year continuous down-and-out equity call priced by Monte Carlo against the closed form.",
        product="barrier_option_monte_carlo",
        deviation=deviation,
        deviation_source=(
            "the knock-out payoff never exceeds the vanilla call payoff, whose root second moment is closed form."
        ),
    )


def build_fx_barrier_option_monte_carlo() -> dict[str, Any]:
    """Build the continuous EUR/USD up-and-out call fixture priced by Monte Carlo."""
    base = build_fx_barrier_option()
    spot, strike, domestic, foreign, volatility = 1.10, 1.10, 0.04, 0.025, 0.10
    expiry = ql.Actual365Fixed().yearFraction(ql_date(VALUATION_DATE), ql.Date(30, 7, 2026))
    deviation = math.exp(-domestic * expiry) * _call_payoff_root_second_moment(
        forward=spot * math.exp((domestic - foreign) * expiry), strike=strike, volatility=volatility, expiry=expiry
    )
    return _continuous_monitoring_fixture(
        base,
        name="eurusd_up_out_call_3m_monte_carlo_quantlib",
        description="Three-month continuous EUR/USD up-and-out call priced by Monte Carlo against the closed form.",
        product="fx_barrier_option_monte_carlo",
        deviation=deviation,
        deviation_source=(
            "the knock-out payoff never exceeds the vanilla call payoff, whose root second moment is closed form."
        ),
        scale=float(base["instrument"]["instrument"]["spec"]["notional"]["amount"]),
    )


def _lookback_payoff_deviation(*, strike: float | None) -> float:
    """Discounted payoff standard deviation of a continuous lookback call, by exact sampling.

    The terminal log-price and its running extremum are drawn from their
    exact joint law (a normal draw and one uniform), with a fixed seed. A
    fixed-strike call pays ``max(maximum - strike, 0)``; a floating-strike
    call (``strike`` None) pays ``terminal - minimum``.
    """
    expiry = _times()[-1]
    generator = np.random.default_rng(LOOKBACK_SEED)
    drift = (RATE - DIVIDEND_YIELD - 0.5 * VOLATILITY**2) * expiry
    variance = VOLATILITY**2 * expiry
    terminal = drift + math.sqrt(variance) * generator.standard_normal(LOOKBACK_SAMPLES)
    root = np.sqrt(terminal**2 - 2.0 * variance * np.log(generator.random(LOOKBACK_SAMPLES)))
    if strike is None:
        payoff = SPOT * (np.exp(terminal) - np.exp(0.5 * (terminal - root)))
    else:
        payoff = np.maximum(SPOT * np.exp(0.5 * (terminal + root)) - strike, 0.0)
    return float(math.exp(-RATE * expiry) * payoff.std())


def _lookback_fixture(base: dict[str, Any], *, kind: str, strike: float | None) -> dict[str, Any]:
    return _continuous_monitoring_fixture(
        base,
        name=f"spx_{kind}_lookback_call_1y_monte_carlo_quantlib",
        description=f"One-year continuous {kind}-strike lookback call priced by Monte Carlo against the closed form.",
        product=f"{kind}_lookback_option_monte_carlo",
        deviation=_lookback_payoff_deviation(strike=strike),
        deviation_source=(
            f"sample deviation of {LOOKBACK_SAMPLES} exact draws of the terminal price and its running "
            f"extremum (numpy default_rng seed {LOOKBACK_SEED})."
        ),
    )


def build_fixed_lookback_option_monte_carlo() -> dict[str, Any]:
    """Build the continuous fixed-strike lookback call fixture priced by Monte Carlo."""
    return _lookback_fixture(build_fixed_lookback_option(), kind="fixed", strike=105.0)


def build_floating_lookback_option_monte_carlo() -> dict[str, Any]:
    """Build the continuous floating-strike lookback call fixture priced by Monte Carlo."""
    return _lookback_fixture(build_floating_lookback_option(), kind="floating", strike=None)


def build_cliquet_option() -> dict[str, Any]:
    """Build an additive cliquet fixture whose caps never bind.

    With a zero local floor and caps far out of reach the payoff is the sum
    of the positive parts of the period returns, paid at expiry: a strip of
    forward-start at-the-money calls on the return, each a Black formula on
    the forward ratio. The period returns are independent under a flat
    volatility, so the payoff variance is the sum of the period variances.
    """
    times = _times()
    notional = 1_000_000.0
    discount = _discount()
    mean = 0.0
    variance = 0.0
    previous = 0.0
    for time in times:
        period = time - previous
        forward = math.exp((RATE - DIVIDEND_YIELD) * period)
        value = ql.blackFormula(ql.Option.Call, 1.0, forward, VOLATILITY * math.sqrt(period), 1.0)
        root_second = _call_payoff_root_second_moment(forward=forward, strike=1.0, volatility=VOLATILITY, expiry=period)
        mean += value
        variance += root_second**2 - value**2
        previous = time
    reference = notional * discount * mean
    deviation = discount * math.sqrt(variance)
    bound = 3.0 * notional * deviation / math.sqrt(CLIQUET_PATHS)
    name = "spx_additive_cliquet_1y_quantlib"
    reason = (
        f"Three standard errors of a plain {CLIQUET_PATHS}-path estimator, from the closed-form variance of the "
        "independent period payoffs; Finstack's antithetic sampling can only reduce the error."
    )
    return {
        "schema": SCHEMA,
        "metadata": metadata(
            name=name,
            domain="equity.cliquet_option",
            description="One-year quarterly additive cliquet with a zero local floor and caps that never bind.",
            product="cliquet_option",
            source_detail=(
                f"QuantLib {ql.__version__} blackFormula per reset period on the forward ratio with unit strike, "
                f"summed and discounted from expiry: {reference!r}. Discounted payoff standard deviation per unit "
                f"notional {deviation!r}."
            ),
        ),
        "kind": "pricing",
        "model": "monte_carlo_gbm",
        "market": market_snapshot(
            [flat_discount_curve("USD-OIS", RATE)],
            prices={
                "SPX-DIVYIELD": {"unitless": DIVIDEND_YIELD},
                "SPX-SPOT": {"price": {"amount": str(SPOT), "currency": "USD"}},
            },
            surfaces=[constant_vol_surface("SPX-CLIQUET-VOL-QL", VOLATILITY, strikes=[50.0, SPOT, 150.0])],
        ),
        "instrument": {
            "schema": "finstack_quant.instrument/1",
            "instrument": {
                "type": "cliquet_option",
                "spec": {
                    "id": name.upper().replace("_", "-"),
                    "underlying_ticker": "SPX",
                    "reset_dates": FIXING_DATES,
                    "expiry": EXPIRY,
                    "local_cap": 10.0,
                    "local_floor": 0.0,
                    "global_cap": 100.0,
                    "global_floor": 0.0,
                    "notional": {"amount": str(int(notional)), "currency": "USD"},
                    "day_count": "act_365f",
                    "discount_curve_id": "USD-OIS",
                    "spot_id": "SPX-SPOT",
                    "vol_surface_id": "SPX-CLIQUET-VOL-QL",
                    "path_model": "atm_term_gbm",
                    "div_yield_id": "SPX-DIVYIELD",
                    "initial_level": None,
                    "past_fixings": [],
                    "payoff_type": "additive",
                    "instrument_pricing_overrides": {"model_config": {"mc_paths": CLIQUET_PATHS}},
                    "attributes": {"tags": ["golden", "quantlib"], "meta": {}},
                },
            },
        },
        "expected": {"npv": reference},
        "tolerances": {"npv": tolerance(bound, reason)},
    }


MONTE_CARLO_BUILDERS: dict[str, tuple[str, Callable[[], dict[str, Any]]]] = {
    "cliquet_option": ("cliquet_option/spx_additive_cliquet_1y_quantlib.json", build_cliquet_option),
    "barrier_option_monte_carlo": (
        "barrier_option/spx_down_out_call_1y_monte_carlo_quantlib.json",
        build_barrier_option_monte_carlo,
    ),
    "fx_barrier_option_monte_carlo": (
        "fx_barrier_option/eurusd_up_out_call_3m_monte_carlo_quantlib.json",
        build_fx_barrier_option_monte_carlo,
    ),
    "fixed_lookback_option_monte_carlo": (
        "lookback_option/spx_fixed_lookback_call_1y_monte_carlo_quantlib.json",
        build_fixed_lookback_option_monte_carlo,
    ),
    "floating_lookback_option_monte_carlo": (
        "lookback_option/spx_floating_lookback_call_1y_monte_carlo_quantlib.json",
        build_floating_lookback_option_monte_carlo,
    ),
    "geometric_asian_option_monte_carlo": (
        "asian_option/spx_geometric_asian_call_1y_monte_carlo_quantlib.json",
        build_geometric_asian_option_monte_carlo,
    ),
    "arithmetic_asian_option_monte_carlo": (
        "asian_option/spx_arithmetic_asian_call_1y_monte_carlo_quantlib.json",
        build_arithmetic_asian_option_monte_carlo,
    ),
}
