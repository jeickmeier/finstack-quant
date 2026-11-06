# FEATURES-004: Shared scaled statistics and Type-7 interpolation

**Decision status:** Draft — observed implementation; adoption pending for this scoped choice. Existing accepted decisions retain their own scope.

**Implementation status:** Observed in the exact source paths and symbols below; discrepancies and limitations are stated explicitly.

**Validation status:** Source and the named test bodies inspected, not executed. This record does not certify numerical correctness or host parity.

**Audit date / revision:** 2026-11-05 / `7e82fd03ad98346727769e88dd5ac5d84b56e9fc`. See [scope and source ledger](README.md).

## Scope and selection

Private helpers in `types.rs` supply centering, mean, sample/population standard deviations and continuous quantiles to the selected transforms. These are feature-layer implementations, not direct aliases of every similarly named core statistic. Callers filter finite inputs; quantile callers sort ascending with `total_cmp` and validate probabilities before entry.

## Ordered computation

1. Let `a=max(abs(x))`. `scaled_centered` returns `(0, zeros)` when `a<=0`; otherwise compute `u_i=x_i/a`, `m=KahanSum(u)/n`, and return `a` with `u_i-m`. Empty input also takes the zero branch.
2. `mean` returns None for empty input and 0 for all-zero data. Otherwise return `clamp(KahanSum(x/a)/n,-1,1)*a`. Scaling avoids a raw sum overflow and the clamp constrains the computed mean to the scale bounds.
3. Sample standard deviation needs at least two values and uses denominator `n-1`; population deviation needs at least one and uses `n`. Both compute `a*sqrt(KahanSum(centered_i²)/denominator)`. Representable input does not guarantee representable sample standard deviation; the outer result check rejects nonfinite output.
4. Type-7 quantile returns None for empty input and the sole value for n=1. Otherwise `p=q*(n-1)`, `j=floor(p)`, `k=ceil(p)`, `w=p-j`, and `Q=(1-w)*x_j+w*x_k`. This convex interpolation avoids explicitly subtracting widely separated endpoints. The helper itself does not validate probability/sort order; all observed callers provide the preconditions.

Input/output units are inherited: means/quantiles/SD retain input units; normalized centered values are dimensionless. Tiny relative spreads may be rounded away in the initial normalization; underflow to finite zero is not caught by output validation. Kahan accumulation is used for these helpers but not for every downstream reduction.

## Rationale and alternatives

Comments document unit-independent scaling and a continuous Type-7 quantile. [Kahan 1965](../../../REFERENCES.md#kahan-1965) is a numerical foundation, not provenance for each chosen clamp or interpolation convention. Proposed rationale is avoiding avoidable scale overflow while preserving a familiar finite-sample convention. Alternatives include centered compensated/rebased moments and discrete empirical quantiles; they differ at ties, short samples and extreme scales. No historical adoption rationale for the clamp or Type-7 choice was found.

## Evidence

- Owning [shared helpers](../../../../finstack-quant/features/src/types.rs): `scaled_centered`, `mean`, `std`, `sample_std`, `population_std`, `quantile_cont`; delegated [Kahan kernel](../../../../finstack-quant/core/src/math/summation.rs).
- Inspected [audit regressions](../../../../finstack-quant/features/tests/audit_regressions.rs): unit-scale sweeps from 1e-150 to 1e150 and a 1e308 mean/opposite-sign median. Scale comparisons often reuse the same production method at another scale; they establish invariance expectations, not an independent oracle.
- Inspected [transform tests](../../../../finstack-quant/features/tests/transforms.rs) contain hand-sized sample-SD and interpolated-winsor expectations, generally with absolute tolerance 1e-12. Inline constant tests in `types.rs` check the MAD reciprocal identity and a pinned normal quartile; they do not independently derive it.

No numerical experiment or external-reference authentication was performed.

## Follow-up tracking

Pending choices, implementation reconciliation, and qualification work are tracked
in the [feature follow-ups](../FOLLOW_UPS.md#features).
