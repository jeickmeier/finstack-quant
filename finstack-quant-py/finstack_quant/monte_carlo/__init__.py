"""Monte Carlo convenience bindings: engine, pricers, analytical helpers.

Bindings for the core convenience subset of the ``finstack-quant-monte-carlo`` Rust
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
"""

import sys

from finstack_quant.finstack_quant import monte_carlo as _mc

MoneyEstimate = _mc.MoneyEstimate
Estimate = _mc.Estimate

TimeGrid = _mc.TimeGrid

McEngine = _mc.McEngine

EuropeanPricer = _mc.EuropeanPricer
PathDependentPricer = _mc.PathDependentPricer
LsmcPricer = _mc.LsmcPricer

black_scholes_call = _mc.black_scholes_call
black_scholes_put = _mc.black_scholes_put

price_european_call = _mc.price_european_call
price_european_put = _mc.price_european_put
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

_key = "finstack_quant.monte_carlo"
if _key not in sys.modules:
    sys.modules[_key] = sys.modules[__name__]

__all__: list[str] = [
    "Estimate",
    "EuropeanPricer",
    "LsmcPricer",
    "McEngine",
    "MoneyEstimate",
    "PathDependentPricer",
    "TimeGrid",
    "black_scholes_call",
    "black_scholes_put",
    "finite_diff_delta",
    "finite_diff_delta_crn",
    "finite_diff_gamma",
    "finite_diff_gamma_crn",
    "price_european_call",
    "price_european_put",
    "price_heston_call",
    "price_heston_put",
]
