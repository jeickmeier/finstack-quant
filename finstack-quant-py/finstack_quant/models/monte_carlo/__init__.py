"""Monte Carlo convenience bindings: engine, pricers, Greek estimators.

Bindings for the core convenience subset of the ``finstack-quant-models`` Rust
crate, including selected non-GBM process wrappers such as Heston. Advanced
Rust process, discretization, RNG, and payoff types are not surfaced as
standalone Python types yet; their parameters are passed directly as numeric
arguments to the exposed pricer constructors and methods.

Greek estimators (``finite_diff_delta``, ``finite_diff_delta_crn``, ``finite_diff_gamma``,
``finite_diff_gamma_crn``) and unbiased two-pass LSMC pricing
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
GbmPathSummary = _mc.GbmPathSummary


simulate_gbm_paths = _mc.simulate_gbm_paths
heston_satisfies_feller = _mc.heston_satisfies_feller

EuropeanPricer = _mc.EuropeanPricer
PathDependentPricer = _mc.PathDependentPricer
LsmcPricer = _mc.LsmcPricer

price_heston_call = _mc.price_heston_call
price_heston_put = _mc.price_heston_put

# Finite-difference Greeks. The `_crn` variants compute true paired
# common-random-number standard errors and are typically 1–2 orders of
# magnitude tighter than the conservative independence-bound stderr
# returned by the non-CRN variants — prefer them for hedge-ratio sizing.
finite_diff_delta = _mc.finite_diff_delta
finite_diff_delta_crn = _mc.finite_diff_delta_crn
finite_diff_gamma = _mc.finite_diff_gamma
finite_diff_gamma_crn = _mc.finite_diff_gamma_crn


__all__: list[str] = [
    "Estimate",
    "EuropeanPricer",
    "GbmPathSummary",
    "LsmcPricer",
    "MoneyEstimate",
    "PathDependentPricer",
    "finite_diff_delta",
    "finite_diff_delta_crn",
    "finite_diff_gamma",
    "finite_diff_gamma_crn",
    "heston_satisfies_feller",
    "price_heston_call",
    "price_heston_put",
    "simulate_gbm_paths",
]
