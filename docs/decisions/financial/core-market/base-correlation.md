# CORE-MARKET-18: Base-correlation interpolation, shape diagnostics, and clipping

- Decision status: **Draft — observed implementation, adoption pending.**
- Implementation status: **Observed** in the owning paths identified below; discrepancies are stated explicitly.
- Validation status: **Evidence inspected or located, not executed.** Formula checks, invariants, and round trips are distinguished below; no production accuracy certification is implied.
- Audit date: **2026-11-05**. Source revision: `7e82fd03ad98346727769e88dd5ac5d84b56e9fc`.
- Scope: the implementation listed here; the [slice inventory](README.md) and [coverage ledger](coverage.json) identify exact file coverage.

## Choice and rationale

`BaseCorrelationCurve` linearly interpolates base-correlation quotes by tranche detachment. It accepts nonmonotonic correlation levels and reports shape warnings separately. Monotonic correlation is not imposed as a proxy for tranche no-arbitrage.

## Algorithm and domain

At least two `(detachment,correlation)` points are sorted by detachment. Detachment is a **percentage of pool notional in [0,100]**, while correlation is a dimensionless fraction in [0,1]. Both must be finite; shared interpolation validation rejects duplicate knots. Interpolation is fixed Linear with FlatZero endpoint levels; there is no time axis, interpolation override, calibration or tranche expected-loss calculation here.

`validate_shape` scans stored quotes. Correlation outside [0.02,0.98] emits a boundary warning; a drop greater than 1e-9 from one knot to the next records a nonmonotonicity violation. Maximum magnitude is the largest preceding-minus-current correlation. The builder does not call this diagnostic as a blocking condition.

Generic shocks accept parallel standard additive values or multiplicative factors, then use `clamp(corr*factor+add,0,1)`. The generic standard unit resolver also accepts RateBp despite the narrower wording in the local unsupported-bump message. Bucket shocks add a decimal correlation-point amount, selecting detachments within an absolute 1e-10 match tolerance; omitted filter selects all, empty filter none. Values are clamped to [0,1] and rebuilt. The result is not projected to monotone shape, positive-semidefinite matrices, or no-arbitrage tranche losses.

## Failure, alternatives

Nonfinite bump/filter input errors. Negative multiplicative factors are permitted by the standard resolver and can clamp correlations to zero. Since correlation 0 and 1 are admitted, pricing consumers must implement or reject their limiting cases explicitly. Clipping makes risk shifts nonlinear near boundaries and is an observed behavior, not an adopted stress policy. Out-of-range query detachments use constant boundary correlation.

A bounded transformed-parameter shock, rejection instead of clipping, or loss-space arbitrage calibration would be different methods.

## Evidence

Owning code: [base_correlation.rs](../../../../finstack-quant/core/src/market_data/term_structures/base_correlation.rs). Relevant tests are located in [base correlation tests](../../../../finstack-quant/core/tests/market_data/curves/base_correlation.rs) and the inspected [context storage regression](../../../../finstack-quant/core/src/market_data/context/curve_storage.rs), which preserves stressed nonmonotonic values through rename/rebuild. They validate shape/transport contracts, not tranche arbitrage. Model alternatives are in [ADR 0031](../models-and-valuations-credit/0031-tranche-loss-model-selection.md); references [Laurent–Gregory](../../../REFERENCES.md#laurent-gregory-2005-factor-copulas) and [Gibson](../../../REFERENCES.md#gibson-2004-synthetic-cdos) concern downstream loss models.

## Follow-up tracking

Pending decisions, implementation reconciliation and validation work are tracked in the
[core-market follow-ups](../FOLLOW_UPS.md#core-market-data).
