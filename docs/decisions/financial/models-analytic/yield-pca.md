# MODELS-ANALYTIC-28: Yield-change PCA, reconstruction and standardized scenarios

**Decision status:** Draft (observed implementation, adoption pending).

**Implementation status:** Observed in the source scope identified below; discrepancies are identified explicitly.

**Validation status:** Evidence inspected/located, not executed. Formula comparisons, round trips and invariants have different evidential strength; none establishes approval or universal accuracy.

**Audit date:** 2026-11-05. **Source revision:** `7e82fd03ad98346727769e88dd5ac5d84b56e9fc`.

## Scope and input domain

Historical finite yield panels or explicit changes with≥2 tenors and≥2 changes. Explicit change-only API synthesizes tenor labels1..N instead of accepting real maturities.

## Choice and rationale

Centered sample-covariance PCA uses symmetric eigendecomposition without standardizing tenor variances. Scenarios scale component loadings by square-root eigenvalues; this is empirical change risk, not a calibrated term-structure law.

## Algorithm

Form adjacent level differences, subtract each tenor sample mean, covariance=C′C/(m−1). Reject trace<1e−30 or nonfinite. SymmetricEigen decomposes covariance; sort descending, clamp negative eigenvalues0, orient each eigenvector so its first loading>|1e−12| in magnitude is positive. Scores=centered changes×loadings; explained fractions use sum of clamped eigenvalues. Reconstruction uses first k scores/loadings then restores mean change. Scenario Δy=Σ shock_k√λk loading_k; apply_scenario adds to base yields without mean drift. components_for_threshold returns first cumulative fraction≥threshold, otherwise all.

## Selection and controls

All N components retained internally; truncated view validates1..N. Scenario can use fewer components (including empty); rejects too many/nonfinite shocks. Deserialized view checks finite aligned shapes, descending nonnegative eigenvalues and bounded monotone fractions, but not their algebraic consistency or orthonormality.

## Conventions

Input yields/changes in consistent decimal units; shocks in per-observation standard deviations. No annualization or elapsed-date weighting; arbitrary date gaps are adjacent observations. Reconstruction returns changes, not yield levels.

## Limitations, failure and fallback

All negative eigenvalues clamp without size-based error threshold. Near-equal eigenvalues leave eigenspace orientation unstable despite sign convention. Threshold accepts out-of-range/nonfinite input and can return first/all rather than error. Historical covariance is unweighted; no time decay, missing-data model or forecasting dynamics.

## Alternatives and tradeoffs

Correlation-matrix PCA or exponentially weighted covariance would change factor dominance; not available selectors here. NS/VAR is a separate implemented forecast model with different assumptions.

## Implementation and validation evidence

[pca.rs](../../../../finstack-quant/models/src/rates/dtsm/pca.rs) and [types.rs](../../../../finstack-quant/models/src/rates/dtsm/types.rs). Inline synthetic-factor, reconstruction, raw-change/panel equivalence and scenario checks are algebraic evidence; [dtsm_serde.rs](../../../../finstack-quant/models/tests/dtsm_serde.rs) checks wire state. Reference: [Litterman–Scheinkman](../../../REFERENCES.md#litterman-scheinkman-1991).

## Follow-up tracking

Adoption questions and implementation or validation work are tracked in the
[analytic-model follow-ups](../FOLLOW_UPS.md#analytic-models).
