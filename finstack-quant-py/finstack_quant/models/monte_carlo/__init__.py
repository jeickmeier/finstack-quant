# ruff: noqa: D104
# Module help is compiled from the matching stub and delegated below.

from finstack_quant.finstack_quant import models as _models

_mc = _models.monte_carlo

MoneyEstimate = _mc.MoneyEstimate
Estimate = _mc.Estimate
PathSummary = _mc.PathSummary
LrmGreeks = _mc.LrmGreeks


simulate_paths = _mc.simulate_paths
heston_satisfies_feller = _mc.heston_satisfies_feller

EuropeanPricer = _mc.EuropeanPricer
PathDependentPricer = _mc.PathDependentPricer
LsmcPricer = _mc.LsmcPricer

price_heston = _mc.price_heston

# Finite-difference Greeks. The reported stderr is the paired
# common-random-number standard error of the per-path differences.
finite_diff_delta = _mc.finite_diff_delta
finite_diff_gamma = _mc.finite_diff_gamma


__all__: list[str] = [
    "Estimate",
    "EuropeanPricer",
    "LrmGreeks",
    "LsmcPricer",
    "MoneyEstimate",
    "PathDependentPricer",
    "PathSummary",
    "finite_diff_delta",
    "finite_diff_gamma",
    "heston_satisfies_feller",
    "price_heston",
    "simulate_paths",
]

__doc__ = _models.monte_carlo.__doc__
