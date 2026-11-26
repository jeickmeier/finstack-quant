"""Hull-White Bermudan swaption priced by least-squares Monte Carlo.

The reference is the finite-difference value the Hull-White tree fixture
already uses. The tolerance is fixed here before any comparison: three
standard errors of a plain estimator at the fixture's path count, plus the
reference's own error. The dispersion comes from an independent simulation in
this module, rounded up to two significant figures. The regression's
exercise-policy bias is not in the bound, so a failure is a finding about
that bias.
"""

from __future__ import annotations

from collections.abc import Callable
from datetime import date
import math
from typing import Any

import numpy as np

from .common import VALUATION_DATE
from .short_rate import (
    EXERCISE_DATES,
    FLAT_RATE,
    MEAN_REVERSION,
    NOTIONAL,
    SIGMA,
    STRIKE,
    build_hull_white_bermudan_swaption,
)

CAPTURED_ON = "2026-11-25"
PATHS = 50_000
DEVIATION_SAMPLES = 1 << 16
DEVIATION_SEED = 42
SWAP_END = date(2032, 4, 30)
REFERENCE_ERROR = 1e-6


def _round_up(value: float) -> float:
    """Round a positive number up to two significant figures."""
    scale = 10.0 ** (math.floor(math.log10(value)) - 1)
    return math.ceil(value / scale) * scale


def _integrated_variance(start: float, end: float) -> float:
    """Variance of the Hull-White factor's time integral contribution between two times."""
    span = end - start
    a = MEAN_REVERSION
    return SIGMA**2 / a**2 * (span + 2.0 / a * math.exp(-a * span) - 0.5 / a * math.exp(-2.0 * a * span) - 1.5 / a)


def _payoff_deviations() -> tuple[float, float]:
    """Dispersion of the Bermudan payoff and of its 1bp sensitivity.

    The payoff of any exercise policy on a path is at most the largest
    discounted positive swap value over the exercise dates, so that maximum's
    standard deviation bounds the estimator's. The sensitivity is the same
    quantity's central difference for a 1bp parallel shift on common paths.
    The Hull-White factor takes exact daily Gaussian steps.
    """
    valuation = date.fromisoformat(VALUATION_DATE)
    days = [(date.fromisoformat(exercise) - valuation).days for exercise in EXERCISE_DATES]
    pay_days = [*days[1:], (SWAP_END - valuation).days]
    rng = np.random.default_rng(DEVIATION_SEED)
    step = 1.0 / 365.0
    decay = math.exp(-MEAN_REVERSION * step)
    step_deviation = SIGMA * math.sqrt((1.0 - decay**2) / (2.0 * MEAN_REVERSION))
    factor = np.zeros(DEVIATION_SAMPLES)
    integral = np.zeros(DEVIATION_SAMPLES)
    states: list[tuple[np.ndarray, np.ndarray]] = []
    for day in range(1, days[-1] + 1):
        previous = factor
        factor = previous * decay + step_deviation * rng.standard_normal(DEVIATION_SAMPLES)
        integral = integral + 0.5 * (previous + factor) * step
        if day in days:
            states.append((factor, integral))

    def bond(rate: float, state: np.ndarray, now: float, then: float) -> np.ndarray:
        loading = (1.0 - math.exp(-MEAN_REVERSION * (then - now))) / MEAN_REVERSION
        convexity = 0.5 * (
            _integrated_variance(now, then) - _integrated_variance(0.0, then) + _integrated_variance(0.0, now)
        )
        return np.exp(-rate * (then - now) - loading * state + convexity)

    def payoff(shift: float) -> np.ndarray:
        rate = FLAT_RATE + shift
        best = np.zeros(DEVIATION_SAMPLES)
        for index, (state, integrated) in enumerate(states):
            now = days[index] / 365.0
            discount = np.exp(-rate * now - integrated - 0.5 * _integrated_variance(0.0, now))
            fixed = np.zeros(DEVIATION_SAMPLES)
            for later in range(index, len(days)):
                accrual = (pay_days[later] - days[later]) / 365.0
                fixed = fixed + accrual * bond(rate, state, now, pay_days[later] / 365.0)
            swap = 1.0 - bond(rate, state, now, pay_days[-1] / 365.0) - STRIKE * fixed
            best = np.maximum(best, discount * np.maximum(swap, 0.0))
        return NOTIONAL * best

    sensitivity = (payoff(1e-4) - payoff(-1e-4)) / 2.0
    return _round_up(float(payoff(0.0).std())), _round_up(float(sensitivity.std()))


def build_hull_white_bermudan_swaption_monte_carlo() -> dict[str, Any]:
    """Build the Bermudan payer swaption fixture priced by least-squares Monte Carlo."""
    fixture = build_hull_white_bermudan_swaption()
    name = "usd_hull_white_5nc1_bermudan_payer_swaption_monte_carlo_quantlib"
    product = "hull_white_bermudan_swaption_monte_carlo"
    deviation, sensitivity_deviation = _payoff_deviations()
    fixture["model"] = "monte_carlo_hull_white_1f"
    meta = fixture["metadata"]
    meta["name"] = name
    meta["description"] = "Hull-White one-factor Bermudan payer swaption priced by least-squares Monte Carlo."
    meta["regen_command"] = f"uv run python -m scripts.golden.quantlib.generate --product {product}"
    meta["captured_on"] = meta["last_reviewed_on"] = CAPTURED_ON
    meta["source_detail"] += (
        " The largest discounted positive swap value over the exercise dates has standard deviation at most "
        f"{deviation!r}, and its 1bp sensitivity at most {sensitivity_deviation!r}, from {DEVIATION_SAMPLES} "
        "independent exact Hull-White paths."
    )
    spec = fixture["instrument"]["instrument"]["spec"]
    spec["id"] = name.upper().replace("_", "-")
    config = spec["instrument_pricing_overrides"]["model_config"]
    del config["tree_steps"]
    config["mc_paths"] = PATHS
    expected = fixture["expected"]
    del expected["hw_sigma_vega"]
    root = math.sqrt(PATHS)
    reason = (
        f"Three standard errors of a plain {PATHS}-path estimator, from the dispersion of the largest discounted "
        "positive swap value over the exercise dates (which bounds any exercise policy's payoff), plus the "
        "finite-difference reference's 1e-6 relative error. The regression's exercise-policy bias is not in "
        "the bound."
    )
    fixture["tolerances"] = {
        "dv01": {
            "abs": 3.0 * sensitivity_deviation / root + REFERENCE_ERROR * abs(expected["dv01"]),
            "tolerance_reason": (
                f"{reason} DV01 is a 1bp parallel shift on common paths, so the dispersion is that of the "
                "per-path central difference."
            ),
        },
        "npv": {"abs": 3.0 * deviation / root + REFERENCE_ERROR * abs(expected["npv"]), "tolerance_reason": reason},
    }
    return fixture


SHORT_RATE_MONTE_CARLO_BUILDERS: dict[str, tuple[str, Callable[[], dict[str, Any]]]] = {
    "hull_white_bermudan_swaption_monte_carlo": (
        "bermudan_swaption/usd_hull_white_5nc1_bermudan_payer_swaption_monte_carlo_quantlib.json",
        build_hull_white_bermudan_swaption_monte_carlo,
    ),
}
