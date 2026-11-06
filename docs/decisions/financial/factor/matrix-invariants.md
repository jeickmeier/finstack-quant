# FACTOR-002: Covariance and sensitivity matrix invariants

**Decision status:** Draft — observed implementation; adoption pending.

**Implementation status:** Observed for the stated source scope; limitations and discrepancies below remain open.

**Validation status:** Evidence inspected or located, not executed. No numerical qualification is implied.

**Audit date:** 2026-11-05. **Source revision:** `7e82fd03ad98346727769e88dd5ac5d84b56e9fc`.

## Choice and rationale

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

Strict positive definiteness would reject legitimate rank-deficient factor universes. Explicit repair could permit noisy estimates but must disclose the distortion; repair is a separate calibration choice. Fallible missing-ID lookups and setters would strengthen boundaries at a public API cost.

## Implementation and evidence

- [src/covariance.rs](../../../../finstack-quant/factor/src/covariance.rs) — inspected implementation.
- [src/sensitivity_matrix.rs](../../../../finstack-quant/factor/src/sensitivity_matrix.rs) — inspected implementation.

[src/covariance.rs](../../../../finstack-quant/factor/src/covariance.rs) tests locate PSD, asymmetric, duplicate-axis and serde checks. [src/sensitivity_matrix.rs](../../../../finstack-quant/factor/src/sensitivity_matrix.rs) tests include nonfinite rejection, ragged rows, zero-factor roundtrips and out-of-range access. Core numerical policy is recorded in [CORE-NUMERICS-07](../core-numerics/matrix-factorization.md); constructor/invariant evidence is not independent covariance-estimator evidence.

## Follow-up tracking

Implementation, adoption and validation work is tracked in the
[scope follow-ups](../FOLLOW_UPS.md#factor-models). This record does not
authorize a runtime change or certify the observed method.
