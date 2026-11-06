# FEATURES-017: Centered unit-gross, rank and inverse-risk signal weights

**Decision status:** Draft — observed implementation; adoption pending for this scoped choice. Existing accepted decisions retain their own scope.

**Implementation status:** Observed in the exact source paths and symbols below; discrepancies and limitations are stated explicitly.

**Validation status:** Source and the named test bodies inspected, not executed. This record does not certify numerical correctness or host parity.

**Audit date / revision:** 2026-11-05 / `7e82fd03ad98346727769e88dd5ac5d84b56e9fc`. See [scope and source ledger](README.md).

## Scope and selection

`LongShortWeights` accepts no parameters. `rank_to_weights` explicitly chains closed cross-sectional ranks into this transform; `risk_scaled_weights` first divides by row-aligned positive volatility. Each exact time partition is normalized independently. These are signal-to-weight helpers, not portfolio optimizers: no holdings, covariance matrix, transaction costs, leverage budget or exposure constraints other than demeaning/gross normalization are inputs.

## Ordered computation

1. For each finite partition, compute scaled centered values `d_i=x_i/a - mean(x/a)` through the shared helper. Let `G=sum(abs(d_i))`, using plain addition.
2. Return `d_i/G` when G>0; otherwise return 0 for each finite row. Missing rows remain None. In exact arithmetic a nonconstant partition has net zero and gross one, with 0.5 positive and negative gross; output checks enforce finiteness, not explicit tolerance bounds on these identities.
3. `rank_to_weights` uses the lowest-tie closed rank from [FEATURES-015](15-cross-sectional-ranks-and-normal-scores.md), then the same centering/normalization. It is not a long-top-bucket/short-bottom-bucket allocation. Constant ranks produce zero weights.
4. `risk_scaled_weights` first validates all lengths and scans every finite volatility for negativity; any negative value fails, even if the paired signal is missing. A finite signal and finite vol>0 produce raw `signal/vol`. Zero, missing or nonfinite vol gives missing; negative infinity is excluded as nonfinite rather than treated as a finite negative volatility. Validate raw ratios for finiteness, then apply LongShortWeights.

Weights are signed dimensionless fractions relative to unit gross, not dollar amounts. Volatility must use common units/horizon for an economically meaningful comparison, but those are caller conventions, not checked metadata. Inverse-risk demeaning can change a signal's raw sign; the resulting weights are not simply proportional to signal/vol.

## Rationale and alternatives

Source documentation specifies the demeaning and gross denominator. Proposed rationale is a neutral research signal allocation. Long-only inverse-volatility weights, covariance risk parity, prior-weight neutrality or equal gross by rank bucket are different methods and are not selected. No fallback rescales overflowing raw risk ratios before division; they cause validation failure even if a theoretically normalized allocation could be representable.

## Evidence

- Owning [long-short kernel](../../../../finstack-quant/features/src/cross_sectional.rs) `long_short_weights`; [compositions](../../../../finstack-quant/features/src/multi.rs) `rank_to_weights`, `risk_scaled_weights`.
- Inspected [transform tests](../../../../finstack-quant/features/tests/transforms.rs): hand centered weights, flat-rank zeros and risk-scaled equal groups. Inspected [audit regressions](../../../../finstack-quant/features/tests/audit_regressions.rs): scale invariance and rejection of a finite negative volatility. Typical tolerances are 1e-12 absolute in transform tests and 1e-11*max(1,abs(expected)) in audit tests.

Raw overflow is an observed check; avoidable failure for a representable final allocation has not been reproduced.

## Follow-up tracking

Pending choices, implementation reconciliation, and qualification work are tracked
in the [feature follow-ups](../FOLLOW_UPS.md#features).
