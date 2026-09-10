"""Independent finite-pool Gaussian expected-loss reference.

Run: uv run --no-project --with scipy==1.18.1 python scripts/golden/tranche_loss_reference.py
Uses SciPy's binomial CDF identity and QUADPACK adaptive Gauss-Kronrod
integration, with a separate composite Gauss-Legendre cross-check.
https://docs.scipy.org/doc/scipy/reference/generated/scipy.integrate.quad.html
No finstack pricing or probability functions are used.
"""

import json
import math
import warnings

import numpy as np
from scipy.integrate import IntegrationWarning, quad
from scipy.special import ndtr, ndtri, roots_legendre
from scipy.stats import binom

warnings.simplefilter("error", IntegrationWarning)
cases = [
    (125, 0.01, 0.3, 0.03),
    (125, 0.05, 0.7, 0.03),
    (125, 0.10, 0.95, 0.07),
    (125, 0.05, 0.99, 0.03),
    (125, 0.001, 0.3, 0.1),
    (10, 0.05, 0.3, 0.03),
    (125, 0.08, 0.95, 0.15),
    (125, 0.08, 0.95, 0.30),
    (125, 0.08, 0.99, 0.15),
    (125, 0.08, 0.99, 0.30),
]
rows = []
for n, pd, rho, cap in cases:
    unit_loss = 0.6 / n
    count_below_cap = min(n, math.floor(cap / unit_loss))
    threshold = ndtri(pd)

    def integrand(z, threshold=threshold, rho=rho, unit_loss=unit_loss, n=n, count_below_cap=count_below_cap, cap=cap):
        """Conditional binomial capped loss multiplied by normal density."""
        conditional_pd = ndtr((threshold - math.sqrt(rho) * z) / math.sqrt(1 - rho))
        conditional_loss = unit_loss * n * conditional_pd * binom.cdf(
            count_below_cap - 1, n - 1, conditional_pd
        ) + cap * binom.sf(count_below_cap, n, conditional_pd)
        return conditional_loss * np.exp(-z * z / 2) / math.sqrt(2 * math.pi)

    value, error = quad(integrand, -10, 10, epsabs=1e-13, epsrel=1e-12, points=list(range(-9, 10)), limit=500)
    nodes, weights = roots_legendre(32)
    check = sum(0.125 * np.dot(weights, integrand(a + 0.125 * (nodes + 1))) for a in np.arange(-10, 10, 0.25))
    if error > 1e-13 or abs(value - check) > 1e-13:
        raise ValueError(f"Reference not converged: value={value}, error={error}, check={check}")
    rows.append({
        "n": n,
        "pd": pd,
        "correlation": rho,
        "cap": cap,
        "recovery": 0.4,
        "expected_loss": value,
        "estimated_error": error,
        "independent_legendre": float(check),
    })
print(
    json.dumps(
        {
            "source": "SciPy 1.18.1 binomial CDF; QUADPACK and composite GL32",
            "gaussian_tail_mass_bound": math.erfc(10 / math.sqrt(2)),
            "cases": rows,
        },
        indent=2,
    )
)
