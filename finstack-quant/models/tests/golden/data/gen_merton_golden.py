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
- cds_par_spread: `MertonModel::cds_par_spread` against QuantLib
  `MidPointCdsEngine` run on a `SurvivalProbabilityCurve` whose quarterly nodes
  are the model's survival function (closed-form 1 - N(-d2) for a terminal
  barrier, the QuantLib Black-Cox hitting probability for a first-passage one).
- cds_calibration: `MertonModel::from_cds_spread` fed the QuantLib par spread
  of a known terminal-barrier model; the expected asset volatility is the one
  the spread was generated from.
- hazard_curve: `MertonModel::to_hazard_curve` survival at its pillars against
  the same survival function, and between pillars against QuantLib
  `HazardRateCurve` (backward-flat hazard) built from the pillar survivals.
- mc_bond: the Merton Monte Carlo bond engine (fixed-income crate) against the
  closed-form exogenous-recovery risky bond value, with the standard error of
  the estimator computed analytically from the outcome distribution.
- report: `merton_report` fields against the references above.

Conventions
-----------
- Rates, payout rates and dividend yields are continuously compounded annual
  decimals; times are year fractions. QuantLib dates use Actual/365 Fixed with
  whole years of 365 days so the year fraction is exactly T. The model-level
  sections (cds_par_spread onwards) need quarter and month steps, which are not
  whole days on that axis, so they run QuantLib on Actual/360 with 360-day
  years: a time t is the date `TODAY + 360 t` days.
- Jump model: dS/S = (r - q - lambda*k) dt + sigma dW + (J - 1) dN,
  ln J ~ N(mu_jump, sigma_jump^2), k = exp(mu_jump + sigma_jump^2/2) - 1.
"""

from __future__ import annotations

import itertools
import json
import math
from pathlib import Path

import QuantLib as ql

OUT = Path(__file__).parent / "merton_reference.json"
GENERATED_AT = "2026-11-24T00:00:00Z"

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


DC360 = ql.Actual360()


def expiry(t: float, basis: int = 365) -> ql.Date:
    days = round(basis * t)
    assert abs(days - basis * t) < 1e-9, f"T must be a whole number of days on a {basis}-day year"
    return TODAY + ql.Period(days, ql.Days)


def flat(rate: float, dc: ql.DayCounter = DC) -> ql.YieldTermStructureHandle:
    return ql.YieldTermStructureHandle(ql.FlatForward(TODAY, rate, dc))


# Black-Cox first passage against QuantLib binary barrier options
BLACK_COX_REFERENCE = (
    "QuantLib AnalyticBinaryBarrierEngine: down-and-in cash-or-nothing call + put "
    "(cash 1, paid at expiry) divided by the discount factor; growing barrier "
    "mapped to dividend yield payout_rate + barrier_growth_rate"
)


def hitting_probability(
    v: float, b: float, r: float, q: float, sigma: float, g: float, t: float, basis: int = 365
) -> float:
    dc = DC if basis == 365 else DC360
    vol = ql.BlackVolTermStructureHandle(ql.BlackConstantVol(TODAY, ql.NullCalendar(), sigma, dc))
    process = ql.BlackScholesMertonProcess(ql.QuoteHandle(ql.SimpleQuote(v)), flat(q + g, dc), flat(r, dc), vol)
    total = 0.0
    for option_type in (ql.Option.Call, ql.Option.Put):
        option = ql.BarrierOption(
            ql.Barrier.DownIn,
            b,
            0.0,
            ql.CashOrNothingPayoff(option_type, b, 1.0),
            ql.AmericanExercise(TODAY, expiry(t, basis), True),
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

# Model-level sections
# The suite's adjudicated absolute tolerance on one default probability (see
# black_cox). Every tolerance below is this bound propagated through the
# quantity's formula, evaluated on the reference values.
EPS_PD = 5e-9
ACT365F_TO_ACT360 = 365.0 / 360.0


def d2_terminal(v: float, sigma: float, b: float, r: float, q: float, t: float) -> float:
    return (math.log(v / b) + (r - q - 0.5 * sigma * sigma) * t) / (sigma * math.sqrt(t))


def model_pd(v: float, sigma: float, b: float, r: float, q: float, g: float | None, t: float) -> float:
    """Risk-neutral default probability by t.

    `g is None` is the terminal barrier, N(-d2). Otherwise it is the Black-Cox
    hitting probability of the barrier B e^{g t}, from QuantLib on the 360-day
    axis.
    """
    if g is None:
        return norm_cdf(-d2_terminal(v, sigma, b, r, q, t))
    return hitting_probability(v, b, r, q, sigma, g, t, basis=360)


def model_fields(v: float, sigma: float, b: float, r: float, q: float, g: float | None) -> dict:
    return {
        "asset_value": v,
        "asset_vol": sigma,
        "debt_barrier": b,
        "rate": r,
        "payout_rate": q,
        "barrier_growth_rate": g,
    }


SURVIVAL_SOURCE = (
    "Merton survival 1 - N(-d2) evaluated in gen_merton_golden.py for a terminal barrier, "
    "QuantLib AnalyticBinaryBarrierEngine hitting probability (as in black_cox) for a first-passage one"
)


# CDS par spread against QuantLib MidPointCdsEngine
def quantlib_cds_legs(survival: list[float], r: float, recovery: float) -> tuple[float, float]:
    """Protection leg per unit of loss given default, and risky annuity per unit spread.

    `survival[i]` is the survival probability at 0.25 i years, `survival[0] = 1`.
    QuantLib runs on Actual/360 with 90-day quarters, so each quarter is exactly
    0.25 on its time axis, the 45-day midpoint is exact, and its premium accrual
    is 0.25 per quarter.
    """
    periods = len(survival) - 1
    dates = [TODAY + ql.Period(90 * i, ql.Days) for i in range(periods + 1)]
    curve = ql.SurvivalProbabilityCurve(dates, survival, DC360)
    for date, value in zip(dates, survival, strict=True):
        assert abs(curve.survivalProbability(date) - value) < 1e-15
    running = 0.01
    cds = ql.CreditDefaultSwap(
        ql.Protection.Buyer,
        1.0,
        running,
        ql.Schedule(dates),
        ql.Unadjusted,
        DC360,
        True,  # settles accrual
        True,  # pays at default time
        TODAY,  # protection start
        None,  # face-value claim
        DC360,  # last period accrues like the others (no extra day)
        True,
        TODAY,
    )
    cds.setPricingEngine(
        ql.MidPointCdsEngine(ql.DefaultProbabilityTermStructureHandle(curve), recovery, flat(r, DC360))
    )
    protection = cds.defaultLegNPV() / (1.0 - recovery)
    annuity = -cds.couponLegNPV() / running

    # Guard the QuantLib setup against the O'Kane sum written out by hand.
    dt, own_protection, own_annuity = 0.25, 0.0, 0.0
    for i in range(1, periods + 1):
        mass = survival[i - 1] - survival[i]
        df_mid = math.exp(-r * (i - 0.5) * dt)
        own_protection += df_mid * mass
        own_annuity += dt * math.exp(-r * i * dt) * survival[i] + 0.5 * dt * df_mid * mass
    assert abs(protection / own_protection - 1.0) < 1e-12, (protection, own_protection)
    assert abs(annuity / own_annuity - 1.0) < 1e-12, (annuity, own_annuity)
    return protection, annuity


def cds_reference(
    v: float, sigma: float, b: float, r: float, q: float, g: float | None, maturity: float, recovery: float
) -> tuple[float, float]:
    """Par spread on this crate's ACT/360-on-ACT/365F accrual, and its tolerance.

    QuantLib's annuity accrues 0.25 per quarter; this crate accrues
    0.25 * 365/360. The annuity is linear in the accrual, so the par spread is
    QuantLib's divided by 365/360.

    Tolerance: a survival error of at most EPS_PD at every node moves the
    protection sum by at most EPS_PD (Abel summation, discount factors
    decreasing) and the annuity by at most EPS_PD * (sum of accruals + half an
    accrual), so
    |ds| <= LGD * EPS_PD * (1/A + P * (accrual * (n + 0.5)) / A^2).
    """
    periods = round(maturity * 4)
    assert abs(periods - maturity * 4) < 1e-12, "maturity must be a whole number of quarters"
    survival = [1.0] + [1.0 - model_pd(v, sigma, b, r, q, g, 0.25 * i) for i in range(1, periods + 1)]
    assert all(later < earlier for earlier, later in itertools.pairwise(survival)), survival
    protection, ql_annuity = quantlib_cds_legs(survival, r, recovery)
    annuity = ql_annuity * ACT365F_TO_ACT360
    lgd = 1.0 - recovery
    spread = lgd * protection / annuity
    accrual = 0.25 * ACT365F_TO_ACT360
    tolerance = lgd * EPS_PD * (1.0 / annuity + protection * accrual * (periods + 0.5) / annuity**2)
    return spread, tolerance


CDS_REFERENCE = (
    "QuantLib MidPointCdsEngine (Actual/360, 90-day quarters, flat continuously compounded discount, "
    "accrual settled at default, protection from the valuation date) on a SurvivalProbabilityCurve whose "
    "quarterly nodes are the " + SURVIVAL_SOURCE + "; par spread = default leg / (coupon leg per unit "
    "spread), divided by 365/360 for this crate's premium accrual"
)
# (id, asset_value, asset_vol, barrier, rate, payout, barrier_growth or None, maturity, recovery)
CDS_CASES = [
    ("terminal_5y", 100.0, 0.25, 80.0, 0.04, 0.00, None, 5.0, 0.40),
    ("terminal_1y", 100.0, 0.30, 85.0, 0.03, 0.00, None, 1.0, 0.40),
    ("terminal_payout_3y", 100.0, 0.35, 70.0, 0.05, 0.02, None, 3.0, 0.25),
    ("terminal_10y", 100.0, 0.20, 75.0, 0.02, 0.01, None, 10.0, 0.60),
    ("first_passage_5y", 100.0, 0.25, 70.0, 0.04, 0.00, 0.00, 5.0, 0.40),
    ("first_passage_growing_3y", 100.0, 0.30, 65.0, 0.03, 0.01, 0.02, 3.0, 0.35),
]
for cid, v, sigma, b, r, q, g, maturity, recovery in CDS_CASES:
    spread, tolerance = cds_reference(v, sigma, b, r, q, g, maturity, recovery)
    case(
        "cds_par_spread",
        cid,
        reference=CDS_REFERENCE,
        **model_fields(v, sigma, b, r, q, g),
        maturity=maturity,
        recovery=recovery,
        expected={"par_spread": spread, "tolerance_abs": tolerance},
    )

# CDS calibration round trip: the QuantLib spread of a known model must give back its volatility
CALIBRATION_REFERENCE = (
    "quote = the cds_par_spread reference (QuantLib MidPointCdsEngine) of a terminal-barrier model with the "
    "stated asset volatility; expected asset_vol is that generating volatility"
)
BRENT_TOLERANCE = 1e-10  # from_cds_spread's documented coordinate tolerance on sigma
# (id, asset_value, asset_vol, barrier, rate, payout, maturity, recovery)
CALIBRATION_CASES = [
    ("terminal_5y", 100.0, 0.25, 80.0, 0.04, 0.00, 5.0, 0.40),
    ("terminal_payout_3y", 100.0, 0.35, 70.0, 0.05, 0.02, 3.0, 0.25),
]
for cid, v, sigma, b, r, q, maturity, recovery in CALIBRATION_CASES:
    spread, spread_tolerance = cds_reference(v, sigma, b, r, q, None, maturity, recovery)
    bump = 1e-4
    up, _ = cds_reference(v, sigma + bump, b, r, q, None, maturity, recovery)
    down, _ = cds_reference(v, sigma - bump, b, r, q, None, maturity, recovery)
    slope = (up - down) / (2.0 * bump)
    assert slope > 0.0, slope
    case(
        "cds_calibration",
        cid,
        reference=CALIBRATION_REFERENCE,
        cds_spread_bp=spread * 10_000.0,
        recovery=recovery,
        debt_barrier=b,
        rate=r,
        maturity=maturity,
        asset_value=v,
        payout_rate=q,
        expected={
            "asset_vol": sigma,
            # Spread tolerance mapped to volatility through the reference
            # slope ds/dsigma, plus the solver's own stopping width.
            "tolerance_abs": spread_tolerance / slope + BRENT_TOLERANCE,
            "spread_slope": slope,
        },
    )

# Hazard curve export
HAZARD_REFERENCE = (
    "pillar survival: " + SURVIVAL_SOURCE + "; off-pillar survival: QuantLib HazardRateCurve "
    "(backward-flat hazard, Actual/360 on 360-day years) built from the hazard rates "
    "-ln(S_i / S_{i-1}) / (t_i - t_{i-1}) of the pillar survivals"
)
# (id, asset_value, asset_vol, barrier, rate, payout, barrier_growth or None, tenors, off-pillar times)
HAZARD_CASES = [
    ("terminal", 100.0, 0.25, 80.0, 0.04, 0.00, None, [1.0, 3.0, 5.0, 7.0, 10.0], [0.5, 2.0, 4.0, 6.0, 8.5]),
    ("terminal_payout", 100.0, 0.35, 70.0, 0.05, 0.02, None, [0.5, 1.0, 2.0, 5.0], [0.25, 0.75, 1.5, 3.5]),
    ("first_passage", 100.0, 0.30, 65.0, 0.03, 0.01, 0.02, [1.0, 2.0, 3.0, 5.0], [0.5, 1.5, 2.5, 4.0]),
]
for cid, v, sigma, b, r, q, g, tenors, off_pillar in HAZARD_CASES:
    survival = [1.0 - model_pd(v, sigma, b, r, q, g, t) for t in tenors]
    assert all(later < earlier for earlier, later in zip([1.0, *survival], survival, strict=False)), survival
    hazards = [
        -math.log(s_now / s_prev) / (t_now - t_prev)
        for t_prev, s_prev, t_now, s_now in zip([0.0, *tenors], [1.0, *survival], tenors, survival, strict=False)
    ]
    # BackwardFlat: the value at node i holds on (t_{i-1}, t_i]; node 0 is unused.
    hazard_curve = ql.HazardRateCurve([TODAY, *(expiry(t, 360) for t in tenors)], [hazards[0], *hazards], DC360)
    for t, s_ref in zip(tenors, survival, strict=True):
        assert abs(hazard_curve.survivalProbability(expiry(t, 360)) - s_ref) < 1e-14
    case(
        "hazard_curve",
        cid,
        reference=HAZARD_REFERENCE,
        **model_fields(v, sigma, b, r, q, g),
        tenors=tenors,
        recovery=0.40,
        off_pillar_times=off_pillar,
        expected={
            "pillar_survival": survival,
            "pillar_tolerance_abs": EPS_PD,
            "off_pillar_survival": [hazard_curve.survivalProbability(expiry(t, 360)) for t in off_pillar],
            # S(t) = S_{i-1}^{1-w} S_i^w: an EPS_PD error in each pillar moves
            # it by at most EPS_PD * S(t) / S_i <= EPS_PD / S_last.
            "off_pillar_tolerance_abs": EPS_PD / survival[-1],
        },
    )

# Merton Monte Carlo bond engine against the closed-form risky bond
MC_REFERENCE = (
    "closed-form exogenous-recovery risky bond in gen_merton_golden.py: each cash coupon is paid if the firm "
    "has survived to its date, recovery_rate x notional is paid at the end of the monitoring step in which "
    "default occurs, and notional is paid at maturity on survival; all discounted at the flat rate. "
    "Survival is the " + SURVIVAL_SOURCE + ". The standard error is the exact standard deviation of that "
    "discrete outcome distribution divided by sqrt(num_paths)"
)
MC_SEED = 20261124
MC_PATHS = 200_000


def risky_bond(
    v: float,
    sigma: float,
    b: float,
    r: float,
    q: float,
    g: float | None,
    coupon_rate: float,
    frequency: int,
    maturity: float,
    recovery: float,
    steps_per_year: int,
) -> tuple[float, float, float]:
    """Mean and standard deviation of the path PV (percent of par), and the default probability."""
    steps = round(maturity * steps_per_year)
    assert abs(steps - maturity * steps_per_year) < 1e-12
    assert steps_per_year % frequency == 0, "coupon dates must be on the monitoring grid"
    dt = maturity / steps
    coupon_every = steps_per_year // frequency
    coupon = 100.0 * coupon_rate / frequency
    if g is None:
        # Terminal barrier: default can only be recognised at maturity.
        survival = [1.0] * steps + [1.0 - model_pd(v, sigma, b, r, q, None, maturity)]
    else:
        survival = [1.0] + [1.0 - model_pd(v, sigma, b, r, q, g, k * dt) for k in range(1, steps + 1)]
    outcomes: list[tuple[float, float]] = []  # (probability, PV)
    coupons_paid = 0.0
    for k in range(1, steps + 1):
        t = k * dt
        df = math.exp(-r * t)
        # Default during step k: coupons through step k-1 were paid, recovery lands at t.
        outcomes.append((survival[k - 1] - survival[k], coupons_paid + recovery * 100.0 * df))
        if k % coupon_every == 0:
            coupons_paid += coupon * df
    outcomes.append((survival[steps], coupons_paid + 100.0 * math.exp(-r * maturity)))
    assert abs(sum(p for p, _ in outcomes) - 1.0) < 1e-12
    mean = sum(p * pv for p, pv in outcomes)
    variance = sum(p * (pv - mean) ** 2 for p, pv in outcomes)
    return mean, math.sqrt(variance), 1.0 - survival[steps]


# (id, asset_value, asset_vol, barrier, rate, payout, barrier_growth or None,
#  coupon_rate, coupon_frequency, maturity, recovery)
MC_CASES = [
    ("terminal_zero_coupon", 100.0, 0.25, 80.0, 0.04, 0.00, None, 0.00, 1, 5.0, 0.40),
    ("terminal_coupon", 100.0, 0.30, 85.0, 0.03, 0.01, None, 0.06, 2, 3.0, 0.40),
    ("first_passage_zero_coupon_zero_recovery", 100.0, 0.25, 70.0, 0.04, 0.00, 0.00, 0.00, 1, 5.0, 0.00),
    ("first_passage_coupon", 100.0, 0.30, 65.0, 0.03, 0.01, 0.02, 0.05, 2, 3.0, 0.40),
]
for cid, v, sigma, b, r, q, g, coupon_rate, frequency, maturity, recovery in MC_CASES:
    steps_per_year = 12
    mean, std_dev, pd = risky_bond(v, sigma, b, r, q, g, coupon_rate, frequency, maturity, recovery, steps_per_year)
    if g is None and coupon_rate == 0.0:
        # The textbook form: e^{-rT} (1 - PD (1 - R)).
        assert abs(mean - 100.0 * math.exp(-r * maturity) * (1.0 - pd * (1.0 - recovery))) < 1e-12
    standard_error = std_dev / math.sqrt(MC_PATHS)
    case(
        "mc_bond",
        cid,
        reference=MC_REFERENCE,
        **model_fields(v, sigma, b, r, q, g),
        notional=100.0,
        coupon_rate=coupon_rate,
        coupon_frequency=frequency,
        maturity=maturity,
        recovery=recovery,
        steps_per_year=steps_per_year,
        num_paths=MC_PATHS,
        seed=MC_SEED,
        antithetic=False,
        expected={
            "dirty_price_pct": mean,
            "standard_error": standard_error,
            "tolerance_abs": 3.0 * standard_error,
            "default_rate": pd,
            "default_rate_tolerance_abs": 3.0 * math.sqrt(pd * (1.0 - pd) / MC_PATHS),
        },
    )

# merton_report against the references above
REPORT_REFERENCE = (
    "default probability: " + SURVIVAL_SOURCE + "; distance to default: d2; implied spread: "
    "-ln(1 - PD (1 - R)) / T; debt spread and implied equity: the Merton equations as in equity_inversion; "
    "CDS par spread: QuantLib MidPointCdsEngine as in cds_par_spread; default point: KMV short-term debt "
    "plus half of long-term debt (Crosbie & Bohn, 2003)"
)


def check(field: str, value: float, tolerance: float, relative: bool, index: int | None = None) -> dict:
    return {"field": field, "index": index, "value": value, "tolerance": tolerance, "relative": relative}


def report_model_checks(
    v: float,
    sigma: float,
    b: float,
    r: float,
    q: float,
    g: float | None,
    horizons: list[float],
    maturity: float,
    recovery: float,
) -> list[dict]:
    lgd = 1.0 - recovery
    checks = []
    for i, t in enumerate(horizons):
        pd = model_pd(v, sigma, b, r, q, g, t)
        checks.append(check("default_probabilities", pd, EPS_PD, False, i))
        if g is None or g == 0.0:
            # d2 is elementary arithmetic; 1e-12 relative is a few ulps of headroom.
            checks.append(check("distance_to_default", d2_terminal(v, sigma, b, r, q, t), 1e-12, True, i))
        # d/dPD of -ln(1 - PD LGD)/T, times EPS_PD.
        checks.append(
            check("implied_spread", -math.log(1.0 - pd * lgd) / t, EPS_PD * lgd / ((1.0 - pd * lgd) * t), False, i)
        )
        if g is None:
            equity, _ = merton_equity(v, sigma, b, r, q, t)
            debt = v * math.exp(-q * t) - equity
            # dD = dE, bounded by EPS_PD on each of N(d1) and N(d2).
            equity_tolerance = EPS_PD * (v * math.exp(-q * t) + b * math.exp(-r * t))
            checks.append(
                check(
                    "debt_spread",
                    -math.log(debt / (b * math.exp(-r * t))) / t,
                    equity_tolerance / (debt * t),
                    False,
                    i,
                )
            )
    spread, spread_tolerance = cds_reference(v, sigma, b, r, q, g, maturity, recovery)
    checks.append(check("cds_par_spread", spread, spread_tolerance, False))
    if g is None:
        equity, equity_vol = merton_equity(v, sigma, b, r, q, maturity)
        equity_tolerance = EPS_PD * (v * math.exp(-q * maturity) + b * math.exp(-r * maturity))
        d1 = d2_terminal(v, sigma, b, r, q, maturity) + sigma * math.sqrt(maturity)
        checks.append(check("implied_equity.value", equity, equity_tolerance, False))
        # sigma_E = N(d1) e^{-qT} sigma V / E: relative error <= EPS_PD / N(d1) + dE / E.
        checks.append(
            check(
                "implied_equity.vol",
                equity_vol,
                equity_vol * (EPS_PD / norm_cdf(d1) + equity_tolerance / equity),
                False,
            )
        )
    return checks


# (id, asset_value, asset_vol, barrier, rate, payout, barrier_growth or None, horizons, maturity, recovery)
REPORT_MODEL_CASES = [
    ("model_terminal", 100.0, 0.30, 70.0, 0.04, 0.01, None, [1.0, 3.0, 5.0], 5.0, 0.40),
    ("model_first_passage", 100.0, 0.25, 60.0, 0.03, 0.00, 0.00, [1.0, 5.0], 5.0, 0.40),
]
for cid, v, sigma, b, r, q, g, horizons, maturity, recovery in REPORT_MODEL_CASES:
    case(
        "report",
        cid,
        reference=REPORT_REFERENCE,
        source={"kind": "model", **model_fields(v, sigma, b, r, q, g)},
        horizons=horizons,
        maturity=maturity,
        recovery=recovery,
        checks=report_model_checks(v, sigma, b, r, q, g, horizons, maturity, recovery),
        # A first-passage model has no endogenous debt spread.
        rejected=[] if g is None else ["debt_spread"],
    )

# Equity source: Hull's Example 24.3 firm with its debt of 10 split as 8 due
# within a year and 4 beyond, so the KMV default point is 8 + 4/2 = 10.
e, se, short_debt, long_debt, r, q, t = 3.0, 0.80, 8.0, 4.0, 0.05, 0.0, 1.0
default_point = short_debt + 0.5 * long_debt
v, sv = solve_assets(e, se, default_point, r, q, t)
d2 = d2_terminal(v, sv, default_point, r, q, t)
case(
    "report",
    "equity_kmv_default_point",
    reference=REPORT_REFERENCE + "; asset value and volatility: " + HULL_REFERENCE,
    source={
        "kind": "equity",
        "equity_value": e,
        "equity_vol": se,
        "short_term_debt": short_debt,
        "long_term_debt": long_debt,
        "rate": r,
        "payout_rate": q,
        "maturity": t,
    },
    horizons=[t],
    maturity=t,
    recovery=0.40,
    checks=[
        # 8 + 4/2 is exact in binary floating point.
        check("default_point", default_point, 0.0, False),
        check("model.debt_barrier", default_point, 0.0, False),
        # The equity_inversion tolerance: the production fixed point stops at a 1e-8 relative step.
        check("model.asset_value", v, 1e-6, True),
        check("model.asset_vol", sv, 1e-6, True),
        check("distance_to_default", d2, 1e-6, True, 0),
        check("default_probabilities", norm_cdf(-d2), 1e-6, True, 0),
    ],
    rejected=[],
)

# Write suite
suite = {
    "meta": {
        "suite_id": "merton_reference",
        "description": (
            "External reference fixtures for the Merton (1974) structural credit "
            "model (KMV equity inversion, distance to default, default probability, "
            "endogenous debt spread), the Black-Cox (1976) first-passage default "
            "probability, Merton (1976) jump-diffusion European prices, and the "
            "model-level outputs built on them: CDS par spread and calibration, "
            "hazard-curve export, the Monte Carlo bond engine and merton_report."
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
                "cds_par_spread": "https://github.com/lballabio/QuantLib/blob/master/ql/pricingengines/credit/midpointcdsengine.cpp",
                "cds_calibration": CALIBRATION_REFERENCE,
                "hazard_curve": "https://github.com/lballabio/QuantLib/blob/master/ql/termstructures/credit/interpolatedhazardratecurve.hpp",
                "mc_bond": MC_REFERENCE,
                "report": REPORT_REFERENCE,
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
                "model_level_time": "cds_par_spread, cds_calibration, hazard_curve, mc_bond and report run QuantLib on Actual/360 with 360-day years, so quarter and month steps are whole days and the year fraction of TODAY + 360 t days is exactly t",
                "barrier_growth_rate": "null is the terminal barrier (default only if V_T < B); a number g is the first-passage barrier B e^{g t}",
                "cds_par_spread": "decimal per annum. Quarterly premium grid in year fractions (no IMM dates, calendar or settlement lag); flat continuously compounded discount at the model's risk-free rate; survival probabilities on a year-fraction axis; premium accrual = year fraction x 365/360 (ACT/360 premium on an ACT/365F axis); protection and half-period accrual on default discounted at the period midpoint; recovery a fixed fraction of notional. QuantLib's Actual/360 accrual of 0.25 per quarter is mapped by dividing its par spread by 365/360",
                "cds_calibration": "terminal barrier, geometric Brownian assets; quote in basis points; asset value, barrier, rate, payout and recovery held fixed while asset volatility is solved",
                "hazard_curve": "piecewise-constant hazard, the rate of each pillar holding on (t_{i-1}, t_i]; survival queried by year fraction from the curve base date; the recovery stored on the curve does not enter survival",
                "mc_bond": "prices in percent of par, dirty. Geometric Brownian assets with drift r - q, 12 steps per year, no antithetic sampling, num_paths independent paths from the stated seed. Cash coupons of notional x coupon_rate / frequency on a regular grid ending at maturity; a coupon is paid only if the firm has survived its date. Exogenous recovery: recovery x notional paid at the end of the step in which default is recognised (maturity for a terminal barrier). First-passage default uses the engine's default Brownian-bridge crossing test, which is exact in distribution for a barrier that is flat or exponentially growing in log space. Discounting is flat at the model's risk-free rate",
                "report": "each check names a MertonReport field (index = position in the horizon vector); `rejected` lists measures the model must refuse, reported as null with an entry in `errors`. Grades are not asserted: the master scale has no external reference here. implied_equity is not asserted for a first-passage model, where equity is not a plain call",
            },
            "tolerances": {
                "kmv_published": "half a unit of the last printed digit: 5e-5 for volatility, DD and PD (4 decimals), 500 for asset value (5 significant figures at 1e7 scale)",
                "equity_inversion": "1e-6 relative: the reference is converged to 1e-13; the production fixed point stops at a 1e-8 relative step",
                "black_cox": "5e-9 absolute: the crate's adjudicated closed-form tolerance against QuantLib's normal CDF (see vol_models_quantlib)",
                "jump_bates_limit": "1e-7 relative: QuantLib's own Bates-vs-Merton tolerance is 2e-8, widened for the 128-term COS expansion",
                "jump_quantlib_table": "1e-2 absolute: the tolerance QuantLib applies to this table, whose values are printed to 2 decimals",
                "jump_series": "1e-6 absolute: an a priori bound equal to the one this crate's COS-vs-Black-Scholes unit tests assert for the default 128-term expansion",
                "eps_pd": "5e-9 absolute on one default probability, the black_cox tolerance; the model-level tolerances below propagate it through each formula on the reference values and are fixed by the generator before any comparison",
                "cds_par_spread": "absolute, per case: LGD x eps_pd x (1/A + P x accrual x (n + 0.5) / A^2), with P the protection sum per unit loss, A the risky annuity and n the number of quarters. A survival error of eps_pd per node moves P by at most eps_pd (Abel summation) and A by at most eps_pd x (sum of accruals + half an accrual). The QuantLib legs are asserted against a hand-written O'Kane sum at 1e-12 relative, so reference error is negligible",
                "cds_calibration": "absolute on asset volatility, per case: the cds_par_spread tolerance divided by the reference slope ds/dsigma (central difference of the QuantLib spread, bump 1e-4), plus the solver's 1e-10 coordinate tolerance",
                "hazard_curve": "pillars: eps_pd absolute. Off-pillar: eps_pd / S(last pillar) absolute, the bound on a log-linear interpolate of two pillar survivals each off by eps_pd",
                "mc_bond": "three standard errors, absolute in percent of par. The standard error is not taken from the simulation: it is the exact standard deviation of the closed-form outcome distribution divided by sqrt(num_paths). default_rate uses 3 x sqrt(PD (1 - PD) / num_paths). Seed, path count and tolerance are fixed here; a miss is reported, not re-seeded",
                "report": "per check. Default probability eps_pd; distance to default 1e-12 relative (elementary arithmetic); implied spread eps_pd x LGD / ((1 - PD x LGD) T); equity value eps_pd x (V e^{-qT} + B e^{-rT}), debt spread that divided by (D T), equity volatility sigma_E x (eps_pd / N(d1) + dE / E); CDS spread as in cds_par_spread. Equity-source checks use the equity_inversion tolerance of 1e-6 relative; the default point and barrier are exact",
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
