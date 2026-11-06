# Equity Black–Scholes and Heston PDE adapters

- **Record:** `VALUATIONS-EQUITY-AND-COMPOSITION-09`.
- **Decision status:** Draft — observed implementation; adoption pending.
- **Audit date/source:** 2026-11-05, `7e82fd03ad98346727769e88dd5ac5d84b56e9fc`.
- **Implementation status:** Observed in the owned paths listed below; shared kernels are identified explicitly.
- **Validation status:** Source and linked evidence inspected or located; no numerical tests executed in this audit. Located tests establish the stated local behavior only, not independent accuracy across the domain.

## Choice and rationale

PDE valuation is explicitly selected. The adapters choose concrete domains/grids and interpolation points before calling shared 1D theta or 2D modified Craig–Sneyd solvers; these domain choices materially affect instrument values.

## Scope and input domain

1D Black–Scholes supports European and American options; Bermudan is rejected, as is American exercise with future cash dividends. European discrete dividends use escrowed spot. 2D Heston supports European exercise without future cash dividends and requires strict market Heston parameters.

## Algorithm

For 1D, use a log-spot sinh grid with 200 points concentrated at log strike. The domain spans the lesser/greater log spot/strike plus/minus `5σ√T+|(r−q)T|`. Solve with four Rannacher startup intervals and 100 time steps; American exercise supplies interior intrinsic payoff constraints. Interpolate at log spot and scale by quantity. At zero volatility, bypass the PDE: maximize discounted deterministic payoff at maturity, also at zero and any interior stationary time for American exercise.

For 2D, use 200 log-spot and 80 variance points, 100 time steps and two startup damping intervals. Log tails span at least spot factors 0.05 to 10 and widen to six standard deviations of mean integrated variance. The variance upper bound is the maximum of 1.5, five times long-run variance, 1.5 times initial variance and a six-sigma variance-tail bound; grids concentrate around log spot and min(initial,long-run variance). Solve Heston ADI and interpolate at `(log S,v0)`. Reject results outside `[0,S exp(-qT)]` for calls or `[0,K exp(-rT)]` for puts except `128×epsilon×max(bound,1)` roundoff, which is clamped.

## Selection, controls and conventions

Registry keys are `PdeCrankNicolson1D` and `PdeAdi2D`. Counts are adapter defaults; ordinary instrument model overrides do not expose these grid sizes. Both use expiry-equivalent constant r/q; lifecycle claims resolve before PDE setup.

## Limitations, failures and alternatives

The 1D adapter does not apply the explicit European arbitrage-bound check used by 2D. Fixed default grids can fail or be inaccurate in extreme tails and short expiries. A bounded result is not convergence evidence. Black/tree, Fourier and MC engines are implemented alternatives; no automatic fallback substitutes them on a PDE failure.

## Evidence

[ADI reference](../../../REFERENCES.md#in-t-hout-welfert-2009); inline cases in [pde_pricer.rs](../../../../finstack-quant/valuations/equity/src/equity_option/pde_pricer.rs) and [pde2d_pricer.rs](../../../../finstack-quant/valuations/equity/src/equity_option/pde2d_pricer.rs) and [test_alt_models.rs](../../../../finstack-quant/valuations/tests/instruments/equity_option/test_alt_models.rs) locate deterministic-limit, grid-bound and cross-engine comparisons. Cross-engine agreement can share input assumptions; production convergence budgets remain unqualified.

## Follow-up tracking

Pending decisions, implementation reconciliation and validation work are tracked in
the [equity and composition follow-ups](../FOLLOW_UPS.md#equity-and-composition).

## Owned source files

- [pde_pricer.rs](../../../../finstack-quant/valuations/equity/src/equity_option/pde_pricer.rs)
- [pde2d_pricer.rs](../../../../finstack-quant/valuations/equity/src/equity_option/pde2d_pricer.rs)
