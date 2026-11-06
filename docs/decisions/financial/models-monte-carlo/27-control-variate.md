# MODELS-MONTE-CARLO-27: Control-variate coefficient and adjusted uncertainty

- Decision status: Draft — observed implementation; adoption pending.
- Implementation status: Observed in control_variate.rs.
- Validation status: Runtime inspected and named test evidence located; selected test bodies inspected; tests not executed.
- Audit date: 2026-11-05. Source revision: `7e82fd03ad98346727769e88dd5ac5d84b56e9fc`.
- Scope and coverage: [slice inventory](README.md), [file ledger](coverage.json).

## Choice and algorithm

The standalone adjustment consumes precomputed target/control means, sample variances, covariance, known control expectation and independent sample count. If control variance>1e-10 choose beta=covariance/control_variance, otherwise beta=0. Adjust mean by `target_mean-beta*(control_mean-known_mean)` and variance by `target_variance-2 beta covariance+beta² control_variance`. Floor the adjusted variance at zero, then report sqrt(variance/n) and a normal 1.96 standard-error interval. Fewer than two samples return the original mean with NaN uncertainty rather than error.

## Scope, conventions and selection

All moments and known expectation must be in consistent discounted/undiscounted units; the function does not discount or check covariance validity. The caller supplies sample count, including any antithetic/replicate interpretation. This utility is not automatically selected by European, Asian or generic engine paths. No pilot sample, coefficient fitting provenance or extra degree-of-freedom adjustment is tracked.

## Failure behavior and alternatives

Every negative residual variance, including materially impossible covariance inputs, is silently floored, potentially reporting zero uncertainty. Finite/moment admissibility is not validated. Estimated-beta uncertainty and same-sample coefficient bias are not reflected. Fixed/pilot/cross-fitted beta and multi-control regression are unimplemented alternatives.

## Evidence

[Implementation/tests](../../../../finstack-quant/models/src/monte_carlo/control_variate.rs) locate synthetic adjustment, covariance and insufficient-sample cases. These establish algebraic behavior only. [Glasserman](../../../REFERENCES.md#glasserman-2004-monte-carlo) is background; algebraic tests do not establish product-specific unbiased control expectations or variance reduction.

## Follow-up tracking

Adoption questions, implementation reconciliation and validation work are tracked in the
[scope follow-ups](../FOLLOW_UPS.md#monte-carlo-models).
