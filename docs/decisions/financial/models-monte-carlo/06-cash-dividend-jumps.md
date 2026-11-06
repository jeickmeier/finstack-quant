# MODELS-MONTE-CARLO-06: Exact GBM subintervals and discrete dividend jumps

- Decision status: Draft — observed implementation; adoption pending.
- Implementation status: Observed in process/gbm_dividends.rs and discretization/exact_gbm_dividends.rs.
- Validation status: Runtime inspected and named test evidence located; selected test bodies inspected; tests not executed.
- Audit date: 2026-11-05. Source revision: `7e82fd03ad98346727769e88dd5ac5d84b56e9fc`.
- Scope and coverage: [slice inventory](README.md), [file ledger](coverage.json).

## Choice and domain

GBM with explicit cash/proportional dividends requires the dedicated exact-dividend scheme. Parameters retain continuous yield q as well; callers determine whether that represents residual yield. Cash dividends are nonnegative amounts per share; proportional dividends are fractions in [0,1]. The process sorts finite nonnegative times and rejects duplicates; `simulate_paths` more strictly rejects time-zero events.

## Algorithm and selection

At each outer step select dividends in (t,t+dt], using four machine epsilons of relative endpoint tolerance. Split into diffusion subintervals and zero-duration jumps. Each positive subinterval above 1e-10 consumes a distinct normal, evolves exact GBM, then applies its jump: cash `max(S-D,0)` or proportional `S*max(1-p,0)`. Allocate `number_of_dividends+1` factor slots per outer step as the maximum random budget. With no dividends in the interval, consume z[0] for the whole step. Cash depletion makes zero absorbing.

## Limitations and alternatives

Events at time zero are excluded by the interval selector and need explicit prior application in low-level use. Tiny subintervals are skipped; event-boundary tolerance and factor over-allocation affect reproducibility. The cash floor changes expected dividend payments when D exceeds stock value. No escrowed-dividend or forward-adjustment alternative is implicitly substituted.

## Evidence

[Process](../../../../finstack-quant/models/src/monte_carlo/process/gbm_dividends.rs), [stepper](../../../../finstack-quant/models/src/monte_carlo/discretization/exact_gbm_dividends.rs); unit tests cover cash/proportional/multiple jumps and positivity. `gbm_with_dividends_forward_drops_by_each_dividend` in [simulation tests](../../../../finstack-quant/models/src/monte_carlo/simulate/tests.rs) uses a direct deterministic carried-dividend expectation, including an event inside a coarse step. [Black–Scholes](../../../REFERENCES.md#black-scholes-1973) is diffusion background; ordinary forward checks do not qualify floor-sensitive cases.

## Follow-up tracking

Adoption questions, implementation reconciliation and validation work are tracked in the
[scope follow-ups](../FOLLOW_UPS.md#monte-carlo-models).
