# MODELS-MONTE-CARLO-25: Paired common-random-number delta and gamma

- Decision status: Draft — observed implementation; adoption pending.
- Implementation status: Observed in greeks/finite_diff.rs and greeks/gbm_european.rs.
- Validation status: Runtime inspected and named test evidence located; selected test bodies inspected; tests not executed.
- Audit date: 2026-11-05. Source revision: `7e82fd03ad98346727769e88dd5ac5d84b56e9fc`.
- Scope and coverage: [slice inventory](README.md), [file ledger](coverage.json).

## Choice and domain

Central finite-difference spot Greeks apply to one-dimensional processes with splittable pseudorandom streams, a fixed path count and disabled capture/auto-stop. Initial spot and relative bump must be positive finite. The actual bump is `h=max(S*bump,1e-8)` and S−h must remain at least 1e-12; there is no asymmetric boundary fallback. Convenience defaults use 1% spot bump, 10,000 streams, 50 steps, seed 42 and antithetics.

## Algorithm and controls

For each path ID clone the same split stream for all initial-state scenarios. Simulate the discounted payoff (including the same antithetic pair mean when enabled) at S+h and S−h for delta; also at S for gamma. Update online statistics with per-path `(V_up-V_down)/(2h)` or `(V_up-2V_base+V_down)/h²`. This estimates covariance-aware standard errors from paired differences, rather than combining independent price errors. Execution here is a serial per-path loop even when engine config requests parallel execution. Convenience results use normal multiplier 1.96 for CI, while generic `Estimate::from_stats` uses Student-t. Reported simulated-path count counts stochastic legs, not multiplied valuation stencils.

## Limitations and alternatives

The estimator targets a finite stencil, not the exact local derivative; MC uncertainty excludes bump bias. Payoff/internal process parameters are fixed as initial spot changes, so spot-linked contract fields must be rebuilt by the caller if their risk convention requires it. Low-level validation checks scheme pairing but omits the engine's expected-step-count check. LRM is an implemented alternative; pathwise derivatives, adaptive bumps and Richardson extrapolation are unimplemented in this slice.

## Evidence

[Paired stencil](../../../../finstack-quant/models/src/monte_carlo/greeks/finite_diff.rs), [GBM convenience](../../../../finstack-quant/models/src/monte_carlo/greeks/gbm_european.rs); located tests compare analytic deterministic derivatives, Black–Scholes stencils, invalid-boundary rejection and antithetic consistency. Black–Scholes comparisons are production-formula evidence, not external benchmarks. [Glasserman](../../../REFERENCES.md#glasserman-2004-monte-carlo) provides sensitivity background. No parameter-wide bias/variance study or tests were executed in this audit.

## Follow-up tracking

Adoption questions, implementation reconciliation and validation work are tracked in the
[scope follow-ups](../FOLLOW_UPS.md#monte-carlo-models).
