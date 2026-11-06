# MODELS-MONTE-CARLO-23: LSMC exercise fitting, replay and statistical interpretation

- Decision status: Draft — observed implementation; adoption pending.
- Implementation status: Observed in pricer/lsmc.rs.
- Validation status: Runtime inspected and named test evidence located; selected test bodies inspected; tests not executed.
- Audit date: 2026-11-05. Source revision: `7e82fd03ad98346727769e88dd5ac5d84b56e9fc`.
- Scope and coverage: [slice inventory](README.md), [file ledger](coverage.json).

## Choice and domain

GBM American/Bermudan call/put exercise is approximated on a uniform time grid using regression of continuation on current spot. Every-step convenience schedules include time zero and maturity. Strike/initial spot must be positive finite; the discount rate is a finite continuously compounded scalar, independently supplied from process drift. Registry defaults use Laguerre degree 3, 50 steps, 100,000 independent streams, seed 42 and antithetics. Limits are 100,000 steps, 10 million independent estimators and 64 million stored path values.

## Algorithm and selection

Generate exact GBM rows with path-indexed Philox, adjacent antithetic legs when enabled. Initialize all path cashflows to terminal intrinsic, regardless of whether maturity appears in the supplied exercise list. Walk sorted unique exercise indices backward, excluding zero/maturity. At each date retain strictly ITM paths; regress their currently selected future cashflow discounted back to this date on basis functions. If ITM count≤basis_count+10, skip exercise at this date. Otherwise replace each retained cashflow/time when immediate value strictly exceeds fitted continuation. Regression failure errors. Convert chosen flows to time-zero PVs.

Training computes a global time-zero exercise decision when zero is allowed: immediate intrinsic≥training mean continuation. The ordinary `price` path summarizes the fitted in-sample values, flooring reported mean/CI to intrinsic if allowed, while retaining stochastic standard error. `fit_exercise_policy` freezes date coefficients and the time-zero decision. `price_with_policy` replays them forward, exercising at the first strict superiority; `price_unbiased` requires a different pricing seed. Out-of-sample replay does not floor its realized mean after fitting. Pair means determine independent-estimator uncertainty.

## Limitations and alternatives

In-sample fitting introduces exercise-policy dependence and selection bias not included in the ordinary pathwise CI. Independent pricing estimates a frozen policy's value; the name `unbiased` does not mean an unbiased estimate of the optimal American value, nor does its CI include training/policy error. Skipped dates reduce the exercise opportunity set. Policy validation checks grid step count and basis size, not basis identity, strike, expiry, discount rate, coefficient lengths or ordering of a publicly mutated policy. Cross-fitting, dual upper bounds and adaptive regression are unimplemented alternatives.

## Evidence

[Implementation/tests](../../../../finstack-quant/models/src/monte_carlo/pricer/lsmc.rs) locate few-ITM continuation preservation, time-zero replay, distinct seeds, intrinsic floor, call/european and put/european bounds, and reproducibility. These mix invariants and production formula comparisons. [Glasserman](../../../REFERENCES.md#glasserman-2004-monte-carlo) provides general MC context; the cited general reference does not establish product-specific regression and exercise accuracy. No new tests were run.

## Follow-up tracking

Adoption questions, implementation reconciliation and validation work are tracked in the
[scope follow-ups](../FOLLOW_UPS.md#monte-carlo-models).
