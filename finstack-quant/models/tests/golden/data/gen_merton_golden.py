#!/usr/bin/env python3
"""Generate external reference fixtures for the Merton structural and jump models.

Regenerate with:

    uv run --no-project --with QuantLib==1.43 python gen_merton_golden.py

Output: merton_reference.json (same directory).

This script never imports finstack. Every expected value is either transcribed
from a published source or computed here by an independent implementation.

Sections
--------
- kmv_published: the five-firm example in the MATLAB `mertonmodel`
  documentation (single-point approach, Maturity = 1 year). Printed inputs and
  printed outputs are transcribed; tolerances are half a unit of the last
  printed digit.
- equity_inversion: Hull (2022), Example 24.3 (E = 3, sigma_E = 80%, D = 10,
  r = 5%, T = 1) plus payout and multi-year cases. Only the zero-payout
  cases are externally anchored: the payout cases use the payout convention
  this crate documents, so they pin solver accuracy, not the convention. Expected values come from
  the damped Newton solve of the two Merton equations below, converged to
  1e-13. The Hull case is cross-checked at generation time against an
  independent R replication (Lozano, "The Merton model" tutorial).
- black_cox: first-passage default probability from QuantLib
  `AnalyticBinaryBarrierEngine` (Haug's binary-barrier formulas). A down-and-in
  cash-or-nothing call plus put at the same strike pays 1 at expiry if the
  barrier was touched, so its undiscounted value is the risk-neutral hitting
  probability. A barrier growing at rate g is mapped to a flat barrier by
  carrying the asset at r - q - g, i.e. QuantLib dividend yield q + g.
- jump_bates_limit: Merton (1976) European prices from QuantLib `BatesEngine`
  with vol-of-vol 1e-4 and theta = v0, the near-degenerate setup of QuantLib's
  own `testAnalyticAndMcVsJumpDiffusion` (tolerance 2e-8 relative there).
- jump_quantlib_table: the first 27 rows of the reference table in QuantLib's
  `test-suite/jumpdiffusion.cpp` (Haug's parametrisation, values recomputed by
  QuantLib, tolerance 1e-2). The installed Python wheel does not export
  `JumpDiffusionEngine`, so the rows are transcribed.
- jump_series: Merton's Poisson-weighted Black-Scholes series, implemented
  here, on cases with a non-zero mean jump and a dividend yield.

Conventions
-----------
- Rates, payout rates and dividend yields are continuously compounded annual
  decimals; times are year fractions. QuantLib dates use Actual/365 Fixed with
  whole years of 365 days so the year fraction is exactly T.
- Jump model: dS/S = (r - q - lambda*k) dt + sigma dW + (J - 1) dN,
  ln J ~ N(mu_jump, sigma_jump^2), k = exp(mu_jump + sigma_jump^2/2) - 1.
"""

from __future__ import annotations

import json
import math
from pathlib import Path

import QuantLib as ql

OUT = Path(__file__).parent / "merton_reference.json"
GENERATED_AT = "2026-11-23T00:00:00Z"

cases: list[dict] = []


def case(model: str, cid: str, **kw) -> None:
    cases.append({"model": model, "id": f"{model}_{cid}", **kw})


def norm_cdf(x: float) -> float:
    return 0.5 * math.erfc(-x / math.sqrt(2.0))


# KMV single-point example, MATLAB mertonmodel documentation
MATLAB_REFERENCE = (
    "MathWorks Risk Management Toolbox documentation, mertonmodel, example "
    "'Compute the Probability of Default Using the Single-Point Approach to the "
    "Merton Model' (MertonData, Maturity 1 year); printed values transcribed"
)
# (equity, equity_vol, liability, rate, drift, PD, DD, A, Sa)
MATLAB_ROWS = [
    (2.6406e07, 0.7103, 4.0e07, 0.05, 0.0306, 0.0638, 1.5237, 6.4210e07, 0.3010),
    (2.6817e07, 0.3929, 3.5e07, 0.05, 0.0300, 0.0008, 3.1679, 6.0109e07, 0.1753),
    (3.9770e07, 0.3121, 3.5e07, 0.05, 0.0310, 0.0000, 4.4298, 7.3063e07, 0.1699),
    (2.9470e07, 0.4595, 3.2e07, 0.05, 0.0302, 0.0026, 2.7916, 5.9906e07, 0.2263),
    (2.5280e07, 0.6181, 4.0e07, 0.05, 0.0305, 0.0344, 1.8196, 6.3231e07, 0.2511),
]
for i, (e, se, liab, r, mu, pd, dd, a, sa) in enumerate(MATLAB_ROWS, start=1):
    case(
        "kmv_published",
        f"matlab_firm{i}",
        reference=MATLAB_REFERENCE,
        equity=e,
        equity_vol=se,
        liability=liab,
        rate=r,
        drift=mu,
        t=1.0,
        expected={
            "asset_value": a,
            "asset_value_tolerance_abs": 500.0,
            "asset_vol": sa,
            "distance_to_default": dd,
            "default_probability": pd,
            "tolerance_abs": 5e-5,
        },
    )


# Equity inversion: independent solve of the two Merton equations
def merton_equity(v: float, sigma: float, b: float, r: float, q: float, t: float) -> tuple[float, float]:
    """Equity value and equity volatility of a call on assets with payout q."""
    d1 = (math.log(v / b) + (r - q + 0.5 * sigma * sigma) * t) / (sigma * math.sqrt(t))
    d2 = d1 - sigma * math.sqrt(t)
    equity = v * math.exp(-q * t) * norm_cdf(d1) - b * math.exp(-r * t) * norm_cdf(d2)
    return equity, norm_cdf(d1) * math.exp(-q * t) * sigma * v / equity


def solve_assets(e: float, se: float, b: float, r: float, q: float, t: float) -> tuple[float, float]:
    """Damped Newton on (ln V, ln sigma_V) with a central-difference Jacobian."""
    x = [math.log(e + b * math.exp(-r * t)), math.log(se * e / (e + b * math.exp(-r * t)))]

    def residual(p: list[float]) -> tuple[float, float]:
        me, mse = merton_equity(math.exp(p[0]), math.exp(p[1]), b, r, q, t)
        return me / e - 1.0, mse / se - 1.0

    for _ in range(200):
        f0, f1 = residual(x)
        if max(abs(f0), abs(f1)) < 1e-13:
            return math.exp(x[0]), math.exp(x[1])
        h = 1e-6
        cols = []
        for j in range(2):
            up = list(x)
            dn = list(x)
            up[j] += h
            dn[j] -= h
            ru, rd = residual(up), residual(dn)
            cols.append(((ru[0] - rd[0]) / (2 * h), (ru[1] - rd[1]) / (2 * h)))
        det = cols[0][0] * cols[1][1] - cols[1][0] * cols[0][1]
        dx0 = (-f0 * cols[1][1] + f1 * cols[1][0]) / det
        dx1 = (-f1 * cols[0][0] + f0 * cols[0][1]) / det
        scale = min(1.0, 0.5 / max(abs(dx0), abs(dx1), 1e-300))
        x = [x[0] + scale * dx0, x[1] + scale * dx1]
    raise RuntimeError("Merton equity inversion did not converge")


HULL_REFERENCE = (
    "Hull (2022), Example 24.3 (inputs, as cited by the replication); expected values from the independent Newton solve in "
    "gen_merton_golden.py, cross-checked against the R replication at "
    "https://mlozanoqf.github.io/tutorial_arf/02-merton.html"
)
SOLVE_REFERENCE = (
    "independent Newton solve of the Merton equity and equity-volatility equations in gen_merton_golden.py"
)
# (id, reference, equity, equity_vol, debt, rate, payout, T)
INVERSION = [
    ("hull_24_3", HULL_REFERENCE, 3.0, 0.80, 10.0, 0.05, 0.0, 1.0),
    ("payout_1y", SOLVE_REFERENCE, 50.0, 0.45, 80.0, 0.03, 0.02, 1.0),
    ("low_leverage_1y", SOLVE_REFERENCE, 120.0, 0.40, 60.0, 0.04, 0.0, 1.0),
    ("payout_5y", SOLVE_REFERENCE, 10.0, 0.60, 40.0, 0.02, 0.01, 5.0),
]
for cid, ref, e, se, b, r, q, t in INVERSION:
    v, sv = solve_assets(e, se, b, r, q, t)
    d2 = (math.log(v / b) + (r - q - 0.5 * sv * sv) * t) / (sv * math.sqrt(t))
    debt_value = v * math.exp(-q * t) - e
    spread = -math.log(debt_value / (b * math.exp(-r * t))) / t
    if cid == "hull_24_3":
        # Values printed by the R replication of the textbook example.
        for ours, printed, half_ulp in (
            (v, 12.3954, 5e-5),
            (sv, 0.2123047, 1e-7),
            (d2, 1.1408256, 1e-7),
            (norm_cdf(-d2), 0.1269712, 1e-7),
        ):
            # 1e-7 rather than 5e-8: the replication is an optimiser output.
            assert abs(ours - printed) < half_ulp, (ours, printed)
        assert abs(debt_value - 9.3954) < 5e-5
        assert abs(spread - 0.012366) < 5e-7
    case(
        "equity_inversion",
        cid,
        reference=ref,
        equity=e,
        equity_vol=se,
        debt=b,
        rate=r,
        payout_rate=q,
        t=t,
        expected={
            "asset_value": v,
            "asset_vol": sv,
            "distance_to_default": d2,
            "default_probability": norm_cdf(-d2),
            "debt_spread": spread,
            "tolerance_rel": 1e-6,
        },
    )


# QuantLib helpers
TODAY = ql.Date(15, 1, 2026)
ql.Settings.instance().evaluationDate = TODAY
DC = ql.Actual365Fixed()


def expiry(t: float) -> ql.Date:
    days = round(365 * t)
    assert abs(days - 365 * t) < 1e-12, "T must be a whole number of 365-day years"
    return TODAY + ql.Period(days, ql.Days)


def flat(rate: float) -> ql.YieldTermStructureHandle:
    return ql.YieldTermStructureHandle(ql.FlatForward(TODAY, rate, DC))


# Black-Cox first passage against QuantLib binary barrier options
BLACK_COX_REFERENCE = (
    "QuantLib AnalyticBinaryBarrierEngine: down-and-in cash-or-nothing call + put "
    "(cash 1, paid at expiry) divided by the discount factor; growing barrier "
    "mapped to dividend yield payout_rate + barrier_growth_rate"
)


def hitting_probability(v: float, b: float, r: float, q: float, sigma: float, g: float, t: float) -> float:
    vol = ql.BlackVolTermStructureHandle(ql.BlackConstantVol(TODAY, ql.NullCalendar(), sigma, DC))
    process = ql.BlackScholesMertonProcess(ql.QuoteHandle(ql.SimpleQuote(v)), flat(q + g), flat(r), vol)
    total = 0.0
    for option_type in (ql.Option.Call, ql.Option.Put):
        option = ql.BarrierOption(
            ql.Barrier.DownIn,
            b,
            0.0,
            ql.CashOrNothingPayoff(option_type, b, 1.0),
            ql.AmericanExercise(TODAY, expiry(t), True),
        )
        option.setPricingEngine(ql.AnalyticBinaryBarrierEngine(process))
        total += option.NPV()
    return total * math.exp(r * t)


# (asset_value, barrier, rate, payout, asset_vol, barrier_growth, T)
BLACK_COX = [
    (100.0, 80.0, 0.05, 0.00, 0.25, 0.00, 1.0),
    (100.0, 80.0, 0.05, 0.02, 0.25, 0.00, 5.0),
    (100.0, 70.0, 0.04, 0.00, 0.30, 0.03, 3.0),
    (100.0, 90.0, 0.03, 0.01, 0.15, 0.02, 2.0),
    (100.0, 60.0, 0.02, 0.00, 0.20, 0.00, 10.0),
    (100.0, 50.0, 0.05, 0.03, 0.40, 0.05, 7.0),
    (150.0, 100.0, 0.01, 0.04, 0.35, 0.00, 2.0),
    (100.0, 40.0, 0.06, 0.00, 0.18, 0.02, 1.0),
]
for i, (v, b, r, q, sigma, g, t) in enumerate(BLACK_COX, start=1):
    pd = hitting_probability(v, b, r, q, sigma, g, t)
    assert 0.0 <= pd <= 1.0, pd
    case(
        "black_cox",
        f"case{i}",
        reference=BLACK_COX_REFERENCE,
        asset_value=v,
        debt_barrier=b,
        rate=r,
        payout_rate=q,
        asset_vol=sigma,
        barrier_growth_rate=g,
        t=t,
        expected={"default_probability": pd, "tolerance_abs": 5e-9},
    )


# Merton (1976) jump diffusion
def bs_price(s: float, k: float, r: float, q: float, vol: float, t: float, call: bool) -> float:
    d1 = (math.log(s / k) + (r - q + 0.5 * vol * vol) * t) / (vol * math.sqrt(t))
    d2 = d1 - vol * math.sqrt(t)
    value = s * math.exp(-q * t) * norm_cdf(d1) - k * math.exp(-r * t) * norm_cdf(d2)
    return value if call else value - s * math.exp(-q * t) + k * math.exp(-r * t)


def merton_series(
    s: float, k: float, r: float, q: float, sigma: float, mu_j: float, sigma_j: float, lam: float, t: float, call: bool
) -> float:
    """Merton's series: Poisson(lambda*(1+k)*t)-weighted Black-Scholes prices."""
    kbar = math.exp(mu_j + 0.5 * sigma_j * sigma_j) - 1.0
    mean = lam * (1.0 + kbar) * t
    total = 0.0
    for n in range(400):
        weight = math.exp(-mean + n * math.log(mean) - math.lgamma(n + 1.0))
        vol_n = math.sqrt(sigma * sigma + n * sigma_j * sigma_j / t)
        rate_n = r - lam * kbar + n * (mu_j + 0.5 * sigma_j * sigma_j) / t
        total += weight * bs_price(s, k, rate_n, q, vol_n, t, call)
    return total


def jump_case(model: str, cid: str, reference: str, params: dict, expected: dict) -> None:
    case(model, cid, reference=reference, **params, expected=expected)


BATES_REFERENCE = (
    "QuantLib BatesEngine (Gauss-Laguerre order 144) on BatesProcess with "
    "kappa 0.5, theta = v0, vol-of-vol 1e-4, rho 0: the near-degenerate Merton-76 "
    "limit used by QuantLib test-suite/batesmodel.cpp testAnalyticAndMcVsJumpDiffusion"
)
V0 = 0.0433
BATES_JUMP = {"sigma": math.sqrt(V0), "mu_jump": -0.2, "sigma_jump": 0.2, "lambda": 2.0}
for t in (1.0, 3.0, 5.0):
    for strike in (80.0, 95.0, 110.0):
        process = ql.BatesProcess(
            flat(0.10), flat(0.04), ql.QuoteHandle(ql.SimpleQuote(100.0)), V0, 0.5, V0, 1.0e-4, 0.0, 2.0, -0.2, 0.2
        )
        engine = ql.BatesEngine(ql.BatesModel(process), 144)
        prices = {}
        for name, option_type in (("call", ql.Option.Call), ("put", ql.Option.Put)):
            option = ql.EuropeanOption(ql.PlainVanillaPayoff(option_type, strike), ql.EuropeanExercise(expiry(t)))
            option.setPricingEngine(engine)
            prices[name] = option.NPV()
            # Guard the reference: QuantLib's own Bates-vs-Merton tolerance.
            series = merton_series(100.0, strike, 0.10, 0.04, math.sqrt(V0), -0.2, 0.2, 2.0, t, name == "call")
            assert abs(prices[name] - series) / series < 2e-8, (t, strike, name, prices[name], series)
        jump_case(
            "jump_bates_limit",
            f"k{strike:g}_t{t:g}",
            BATES_REFERENCE,
            {"spot": 100.0, "strike": strike, "rate": 0.10, "dividend_yield": 0.04, "t": t, **BATES_JUMP},
            {"call": prices["call"], "put": prices["put"], "tolerance_rel": 1e-7},
        )

QL_TABLE_REFERENCE = (
    "QuantLib test-suite/jumpdiffusion.cpp reference table (after Haug, Option "
    "Pricing Formulas, 1998, p. 9; values recomputed by QuantLib, tolerance 1e-2); "
    "rows transcribed. Haug inputs: total vol v, jump share gamma, zero mean jump: "
    "sigma_jump = v*sqrt(gamma/lambda), sigma = v*sqrt(1-gamma), mu_jump = -sigma_jump^2/2"
)
# All rows: call, spot 100, q 0, r 0.08, total vol 0.25, gamma 0.25.
QL_TABLE = {
    80.0: [20.67, 21.74, 23.63, 20.65, 21.70, 23.61, 20.64, 21.70, 23.61],
    90.0: [11.00, 12.74, 15.40, 10.98, 12.75, 15.42, 10.98, 12.75, 15.42],
    100.0: [3.42, 5.88, 8.95, 3.51, 5.96, 9.02, 3.53, 5.97, 9.03],
}
for strike, values in QL_TABLE.items():
    grid = [(lam, t) for lam in (1.0, 5.0, 10.0) for t in (0.10, 0.25, 0.50)]
    for (lam, t), value in zip(grid, values, strict=True):
        total_vol, gamma = 0.25, 0.25
        sigma_j = total_vol * math.sqrt(gamma / lam)
        sigma = total_vol * math.sqrt(1.0 - gamma)
        mu_j = -0.5 * sigma_j * sigma_j
        # Guard the transcription: a typo fails here, not in the Rust test.
        series = merton_series(100.0, strike, 0.08, 0.0, sigma, mu_j, sigma_j, lam, t, True)
        assert abs(series - value) <= 1e-2, (strike, lam, t, series, value)
        jump_case(
            "jump_quantlib_table",
            f"k{strike:g}_lam{lam:g}_t{t:g}",
            QL_TABLE_REFERENCE,
            {
                "spot": 100.0,
                "strike": strike,
                "rate": 0.08,
                "dividend_yield": 0.0,
                "t": t,
                "sigma": sigma,
                "mu_jump": mu_j,
                "sigma_jump": sigma_j,
                "lambda": lam,
            },
            {"call": value, "tolerance_abs": 1e-2},
        )

SERIES_REFERENCE = (
    "Merton (1976) series: Poisson(lambda*(1+k)*T)-weighted Black-Scholes prices, "
    "independent implementation in gen_merton_golden.py (400 terms)"
)
# (spot, strike, rate, dividend, sigma, mu_jump, sigma_jump, lambda, T)
SERIES = [
    (100.0, 100.0, 0.05, 0.00, 0.20, -0.10, 0.15, 1.0, 1.0),
    (100.0, 90.0, 0.05, 0.02, 0.20, -0.10, 0.15, 1.0, 1.0),
    (100.0, 110.0, 0.05, 0.02, 0.20, -0.10, 0.15, 1.0, 1.0),
    (100.0, 100.0, 0.03, 0.01, 0.15, 0.05, 0.10, 3.0, 0.5),
    (100.0, 120.0, 0.03, 0.01, 0.15, 0.05, 0.10, 3.0, 2.0),
    (100.0, 80.0, 0.02, 0.00, 0.30, -0.25, 0.30, 0.5, 3.0),
    (50.0, 55.0, 0.04, 0.03, 0.25, -0.05, 0.20, 2.0, 0.25),
    (100.0, 100.0, 0.05, 0.00, 0.20, 0.00, 0.00, 0.0, 1.0),
]
for i, (s, k, r, q, sigma, mu_j, sigma_j, lam, t) in enumerate(SERIES, start=1):
    if lam == 0.0:
        call, put = (bs_price(s, k, r, q, sigma, t, flag) for flag in (True, False))
    else:
        call, put = (merton_series(s, k, r, q, sigma, mu_j, sigma_j, lam, t, flag) for flag in (True, False))
    jump_case(
        "jump_series",
        f"case{i}",
        SERIES_REFERENCE,
        {
            "spot": s,
            "strike": k,
            "rate": r,
            "dividend_yield": q,
            "t": t,
            "sigma": sigma,
            "mu_jump": mu_j,
            "sigma_jump": sigma_j,
            "lambda": lam,
        },
        {"call": call, "put": put, "tolerance_abs": 1e-6},
    )

# Write suite
suite = {
    "meta": {
        "suite_id": "merton_reference",
        "description": (
            "External reference fixtures for the Merton (1974) structural credit "
            "model (KMV equity inversion, distance to default, default probability, "
            "endogenous debt spread), the Black-Cox (1976) first-passage default "
            "probability, and Merton (1976) jump-diffusion European prices."
        ),
        "reference_source": {
            "name": "MATLAB mertonmodel documentation; Hull (2022) Example 24.3; QuantLib (Python)",
            "version": ql.__version__,
            "vendor": "MathWorks; QuantLib Project",
            "url": "https://www.quantlib.org/",
            "extra": {
                "kmv_published": "https://www.mathworks.com/help/risk/mertonmodel.html",
                "equity_inversion": HULL_REFERENCE,
                "black_cox": "https://github.com/lballabio/QuantLib/blob/master/ql/pricingengines/barrier/analyticbinarybarrierengine.cpp",
                "jump_bates_limit": "https://github.com/lballabio/QuantLib/blob/master/test-suite/batesmodel.cpp",
                "jump_quantlib_table": "https://github.com/lballabio/QuantLib/blob/master/test-suite/jumpdiffusion.cpp",
                "jump_series": "Merton, R. (1976), Option pricing when underlying stock returns are discontinuous, JFE 3",
            },
        },
        "generated": {
            "at": GENERATED_AT,
            "by": "gen_merton_golden.py",
            "command": "uv run --no-project --with QuantLib==1.43 python gen_merton_golden.py",
            "environment": {"quantlib": ql.__version__},
        },
        "status": "certified",
        "schema_version": 1,
        "extra": {
            "conventions": {
                "rates": "continuously compounded annual decimals, including payout rates and dividend yields",
                "time": "year fractions; QuantLib dates are whole 365-day years under Actual/365 Fixed",
                "valuation_date": "none: every model input is a year fraction. QuantLib evaluation date "
                + TODAY.ISO()
                + " only anchors its flat term structures",
                "units": "asset, equity and debt values share one currency unit; volatilities, rates, spreads and probabilities are decimals; option prices are per unit of underlying",
                "kmv_published": "debt barrier = Liability; DD and PD use the physical Drift; Maturity 1 year",
                "equity_inversion": "E = V e^{-qT} N(d1) - B e^{-rT} N(d2), sigma_E = N(d1) e^{-qT} sigma_V V / E; DD and PD are risk-neutral (d2, N(-d2)); debt spread = -ln((V e^{-qT} - E)/(B e^{-rT}))/T. The payout terms follow this crate's documented convention; only the payout_rate = 0 cases are externally anchored",
                "black_cox": "risk-neutral probability that V_t touches B e^{g t} before T, continuous monitoring",
                "jump": "dS/S = (r - q - lambda k) dt + sigma dW + (J - 1) dN, ln J ~ N(mu_jump, sigma_jump^2), k = exp(mu_jump + sigma_jump^2/2) - 1; discounted prices per unit of underlying",
            },
            "tolerances": {
                "kmv_published": "half a unit of the last printed digit: 5e-5 for volatility, DD and PD (4 decimals), 500 for asset value (5 significant figures at 1e7 scale)",
                "equity_inversion": "1e-6 relative: the reference is converged to 1e-13; the production fixed point stops at a 1e-8 relative step",
                "black_cox": "5e-9 absolute: the crate's adjudicated closed-form tolerance against QuantLib's normal CDF (see vol_models_quantlib)",
                "jump_bates_limit": "1e-7 relative: QuantLib's own Bates-vs-Merton tolerance is 2e-8, widened for the 128-term COS expansion",
                "jump_quantlib_table": "1e-2 absolute: the tolerance QuantLib applies to this table, whose values are printed to 2 decimals",
                "jump_series": "1e-6 absolute: an a priori bound equal to the one this crate's COS-vs-Black-Scholes unit tests assert for the default 128-term expansion",
            },
        },
    },
    "cases": cases,
}

OUT.write_text(json.dumps(suite, indent=2) + "\n")
counts: dict[str, int] = {}
for c in cases:
    counts[c["model"]] = counts.get(c["model"], 0) + 1
print(f"wrote {OUT.name}: {len(cases)} cases {counts}")
