# FACTOR-002: Covariance and sensitivity matrix invariants

**Decision status:** Accepted 2026-11-17 for checked covariance and sensitivity matrices.

**Implementation status:** Ordered storage and constructor and serde validation are implemented in part. Exact symmetry, mixed-scale PSD admission, checked mutation and revised lookup behavior remain pending.

**Validation status:** All 343 Factor Rust tests and 30 focused matrix tests passed on 2026-11-17 against current behavior. The accepted admission, mutation and lookup rules still need qualification.

**Audit date:** 2026-11-05. **Source revision:** `7e82fd03ad98346727769e88dd5ac5d84b56e9fc`.

## Accepted decision

1. Require exact, unique, nonblank factor and position IDs and preserve their declared order. Admit finite entries and valid empty shapes. Check dimensions and capacity before allocation. Reject invalid writes without changing matrix state. Apply the same validation to construction and restoration. [Factor basis contract](factor-configuration.md).
2. Require exact numerical symmetry, nonnegative variances and zero covariance on each zero-variance axis. Check the positive-variance block on a unit-diagonal basis with the core PSD gate and zero pivot tolerance. Admit computed rank-deficient PSD inputs and reject computed indefiniteness. Retain the supplied covariance entries. Repair remains an estimator operation. [Core matrix contract](../core-numerics/matrix-factorization.md#accepted-decision-checked-matrix-factors-solves-and-correlation-admission).
3. Use typed errors for unknown IDs and out-of-range indices. A known zero-variance factor has zero variance and covariance, with undefined correlation. Preserve that distinction through host access and restoration.

## Observed choice and rationale

Use dense row-major covariance and positions-by-factors sensitivities, with exact factor-axis checks before risk. Covariance construction accepts positive semidefinite matrices through the core pivoted-Cholesky kernel; it does not repair a submitted covariance.

## Input domain and conventions

Covariance data has n² finite entries and unique ordered factor IDs. Sensitivity rows carry monetary exposure per factor coordinate; `from_rows` requires one row per position, one value per factor, finite entries, and a non-overflowing dimensions product. Empty axes are supported. Position identifiers and sensitivity factor identifiers are not independently deduplicated by `from_rows`.

## Algorithm stages

1. Build the covariance ID index and reject duplicates.
2. Compare off-diagonal symmetry with tolerance `SYMMETRY_TOLERANCE * max(abs(diagonal))`; call core `cholesky_correlation` for finite shape and PSD validation.
3. Cache standard deviations as sqrt(positive diagonal), otherwise zero, without replacing stored covariance entries.
4. Report correlation as covariance/(sigma_i sigma_j), or zero when either cached standard deviation is zero.
5. Deserialize covariance through the same constructor and rebuild caches. Deserialize sensitivity nested rows through `from_rows`, flatten them, and serialize back to rows.

## Selection and controls

ID-based covariance/variance/correlation lookups return zero for an unknown ID. Index-based covariance accessors assert bounds. Sensitivity readers return errors for invalid indices; `set_delta` asserts bounds but accepts a nonfinite value. Factor-risk evaluation consequently rescans sensitivities for finiteness.

## Limitations, failures, and fallbacks

PSD is judged with the core relative pivot tolerance, so tolerance-sized negatives can survive in the stored matrix even though cached standard deviations are zero. Symmetry tolerance does not symmetrize the input. `zeros` and covariance initial shape arithmetic use unchecked `usize` products, unlike `from_rows`. Missing-ID zero lookup may hide a caller alignment error if the caller bypasses exact-axis risk entry points.

## Alternatives and tradeoffs

Zero pivot tolerance permits computed semidefinite structure. Finite-precision arithmetic can still reject a mathematically PSD input. Unit-diagonal admission avoids making a large variance axis the tolerance scale for unrelated small axes. The covariance values used by risk remain in their supplied coordinates.

## Implementation and evidence

- [src/covariance.rs](../../../../finstack-quant/factor/src/covariance.rs) — inspected implementation.
- [src/sensitivity_matrix.rs](../../../../finstack-quant/factor/src/sensitivity_matrix.rs) — inspected implementation.

[Covariance](../../../../finstack-quant/factor/src/covariance.rs) and [sensitivity matrix](../../../../finstack-quant/factor/src/sensitivity_matrix.rs) tests passed against current behavior. Qualify exact symmetry, mixed-scale and rank-deficient PSD cases, zero-axis couplings, empty dimensions, checked writes and missing-ID behavior under the accepted contract.

## Follow-up tracking

Implementation reconciliation and qualification are tracked in the
[scope follow-ups](../FOLLOW_UPS.md#factor-models).
