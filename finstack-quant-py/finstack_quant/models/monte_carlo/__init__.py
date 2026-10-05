"""Monte Carlo convenience bindings: engine, pricers, Greek estimators.

Bindings for the core convenience subset of the ``finstack-quant-models`` Rust
crate. ``simulate_paths`` simulates any built-in Markov process (GBM, Heston,
Hull-White, CIR, ...) under a chosen discretization scheme from a plain-data
spec. The Rust process, discretization, RNG, and payoff types are not surfaced
as standalone Python types; the pricers take their parameters directly as
numeric arguments.

Greek estimators (``finite_diff_delta``, ``finite_diff_gamma``) and unbiased two-pass LSMC pricing
(``LsmcPricer.price_american_put_unbiased`` /
``price_american_call_unbiased``) wrap the Rust crate's variance-reduction
machinery for hedge-ratio sizing and bias-mitigated American option
valuation respectively.

Examples:
--------
>>> from finstack_quant.models.monte_carlo import heston_satisfies_feller
>>> heston_satisfies_feller(2.0, 0.04, 0.3)
True
"""

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

price_heston_call = _mc.price_heston_call
price_heston_put = _mc.price_heston_put

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
    "price_heston_call",
    "price_heston_put",
    "simulate_paths",
]
