"""
Monte Carlo convenience bindings (``finstack-quant-models``).

Exposes path simulation for the built-in processes, Markov and
path-dependent (:func:`simulate_paths`), GBM and Heston pricers, and finite-difference Greek
estimators. Processes and discretization schemes are selected by the plain-data
spec passed to :func:`simulate_paths`; the Rust process, discretization, RNG
and payoff types are not surfaced as standalone Python types, and the pricers
take their parameters directly as numeric arguments.

Examples
--------
>>> from finstack_quant.models.monte_carlo import heston_satisfies_feller
>>> heston_satisfies_feller(2.0, 0.04, 0.3)
True
"""

from __future__ import annotations
from finstack_quant.models.monte_carlo._results import (
    Estimate as Estimate,
    LrmGreeks as LrmGreeks,
    MoneyEstimate as MoneyEstimate,
    PathSummary as PathSummary,
)
from finstack_quant.models.monte_carlo._engine import (
    heston_satisfies_feller as heston_satisfies_feller,
    price_heston as price_heston,
    simulate_paths as simulate_paths,
)
from finstack_quant.models.monte_carlo._pricers import (
    EuropeanPricer as EuropeanPricer,
    LsmcPricer as LsmcPricer,
    PathDependentPricer as PathDependentPricer,
)
from finstack_quant.models.monte_carlo._greeks import (
    finite_diff_delta as finite_diff_delta,
    finite_diff_gamma as finite_diff_gamma,
)

__all__ = [
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
