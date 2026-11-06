# VALUATIONS-EQUITY-AND-COMPOSITION-11: Equity rough Heston and rough Bergomi adapters

- **Record:** `VALUATIONS-EQUITY-AND-COMPOSITION-11`.
- **Decision status:** Draft — observed implementation; adoption pending.
- **Audit date/source:** 2026-11-05, `7e82fd03ad98346727769e88dd5ac5d84b56e9fc`.
- **Implementation status:** Observed in the owned paths listed below; shared kernels are identified explicitly.
- **Validation status:** Source and linked evidence inspected or located; no numerical tests executed in this audit. Located tests establish the stated local behavior only, not independent accuracy across the domain.

## Choice and rationale

Rough-volatility engines are explicit European alternatives with strict market parameters. Rough Bergomi bootstraps interval forward variance from ATM-forward total variance; rough Heston uses its own variance parameters. Neither is automatically chosen from the shape of an equity smile.

## Scope and input domain

European equity options without future cash dividends. Rough Heston requires `ROUGH_HESTON_V0/KAPPA/THETA/SIGMA_V/RHO/HURST`; rough Bergomi requires `ROUGH_BERGOMI_ETA/HURST/RHO` plus an unshifted Black strike surface or an explicit implied-volatility override. Shared constructors validate Hurst/model domains.

## Algorithm

Rough Heston Fourier constructs `RoughHestonFourierParams` and calls its European transform pricer with r/q/T. The adapter supplies no numerical override. Rough Heston MC constructs the hybrid discretization on a uniform grid, starts `[S,v0]`, uses serial `McEngine`, Philox and a terminal European payoff, discounts by `exp(−rT)` and attaches positive standard error.

Rough Bergomi: an implied-volatility override yields flat forward variance `xi0=σ²`. Otherwise retain positive surface expiries before maturity and append maturity. At each u sample clamped volatility at `S exp((r−q)u)`, form `W(u)=σ(u)^2u`, then set interval variance `[W(u)−W(previous)]/(u−previous)`. Nonfinite/nonpositive intervals error; the resulting piecewise-constant curve conserves each sampled total variance.

Simulate each path using Riemann–Liouville Volterra increments and their matched Brownian driving normals, a RoughBergomi Euler discretization and shared fractional-path engine. Reuse buffers; update online mean/standard error; discount both at the end. A nonfinite sample standard error is replaced with zero before diagnostics, an observed behavior requiring agreement.

## Selection, controls and conventions

Rough MC path count accepts the instrument override; seed is resolved per instrument. Default rough MC budget is 100,000 paths and 100 time steps; a path override above 5,000,000 is rejected. Rough Bergomi uses an explicit serial path loop, not generic target-confidence stopping/antithetic configuration. Surface sampling is ATM-forward at each expiry, not a strike-specific/local-vol smile evolution.

## Limitations, failures and alternatives

ATM strip construction is not calibration of rough parameters, and a positive total-variance strip does not prove an arbitrage-free smile. Uniform left-grid variance sampling creates discretization error across strip breaks. Parameter/transform failures may surface through nonfinite Money construction rather than an adapter-specific convergence report. Standard Heston and Black are available alternatives, not automatic fallback paths.

## Evidence

[rough Bergomi](../../../REFERENCES.md#bayer-friz-gatheral-2016), [rough Heston](../../../REFERENCES.md#el-euch-rosenbaum-2019), [hybrid scheme](../../../REFERENCES.md#bennedsen-lunde-pakkanen-2017); inline `variance_strip_tests` in [rough_bergomi_mc_pricer.rs](../../../../finstack-quant/valuations/equity/src/equity_option/rough_bergomi_mc_pricer.rs) inspect total-variance conservation, invalid surfaces and deterministic-variance refinement. [test_rough_bergomi.rs](../../../../finstack-quant/valuations/tests/instruments/equity_option/test_rough_bergomi.rs) locates simulation cases. These are limit/identity/cross-engine evidence, not independent calibrated-market validation.

## Follow-up tracking

Pending decisions, implementation reconciliation and validation work are tracked in
the [equity and composition follow-ups](../FOLLOW_UPS.md#equity-and-composition).

## Owned source files

- [rough_heston_market.rs](../../../../finstack-quant/valuations/equity/src/equity_option/rough_heston_market.rs)
- [rough_heston_fourier_pricer.rs](../../../../finstack-quant/valuations/equity/src/equity_option/rough_heston_fourier_pricer.rs)
- [rough_heston_mc_pricer.rs](../../../../finstack-quant/valuations/equity/src/equity_option/rough_heston_mc_pricer.rs)
- [rough_bergomi_mc_pricer.rs](../../../../finstack-quant/valuations/equity/src/equity_option/rough_bergomi_mc_pricer.rs)
