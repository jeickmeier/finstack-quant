# FEATURES-016: Absolute, sigma and Type-7 cross-sectional clipping

**Decision status:** Draft — observed implementation; adoption pending for this scoped choice. Existing accepted decisions retain their own scope.

**Implementation status:** Observed in the exact source paths and symbols below; discrepancies and limitations are stated explicitly.

**Validation status:** Source and the named test bodies inspected, not executed. This record does not certify numerical correctness or host parity.

**Audit date / revision:** 2026-11-05 / `7e82fd03ad98346727769e88dd5ac5d84b56e9fc`. See [scope and source ledger](README.md).

## Scope and actual selection

`Clip`, `ClipBySigma` and `Winsorize` operate independently per finite cross-section or grouped sub-partition. Missing/nonfinite inputs remain None. Explicit parameters must be finite JSON numbers; omitted absolute clip bounds alone default to negative/positive infinity.

## Ordered variants and failure policy

- **Clip:** parse lower/upper, default (-infinity,+infinity); reject lower>upper, then clamp each finite raw value. Equal bounds are allowed. No population estimator is needed, but the public call still validates row-aligned partition keys. Omitted bounds are allowed sentinels; explicit infinity is not an accepted parameter.
- **ClipBySigma:** parse finite sigma>=0, default 3. Compute scaled arithmetic mean c and population SD s of finite raw observations; form `L=c-sigma*s`, `U=c+sigma*s`. Reject either nonfinite bound before clamping. Sigma=0 collapses to the mean unless forming the bounds itself fails. Singleton/constant finite partitions have zero SD.
- **Winsorize:** parse probabilities lower/upper, defaults .01/.99; require `0<=lower<=upper<=1`. Sort finite values, compute both [Type-7 cutoffs](04-scaled-statistics-and-quantiles.md), and clamp each finite value. Equal quantile probabilities are allowed and empty finite partitions remain missing.

All outputs retain input units. Sigma is dimensionless and uses a population convention. Bounds are recomputed from the same data being clipped; repeated sigma/quantile clipping can change the sample and is not specified as idempotent. Fixed-bound clipping has an unchanged mathematical clamp under reapplication.

## Rationale and alternatives

The documented variants expose three distinct controls; proposed rationale is giving callers an explicit absolute versus statistical bound. There is no automatic robust alternative if sigma bounds overflow and no fallback to a finite extreme. Robust MAD cutoffs, sample-SD bounds, discrete quantiles or one-sided omission for winsorization are different contracts. Finite inputs do not assure finite sigma bounds; the explicit error is part of the implementation.

## Evidence

- Owning [cross-sectional resolve/apply and helpers](../../../../finstack-quant/features/src/cross_sectional.rs): `ResolvedCrossSectionalOp::resolve`, `clip`, `clip_by_sigma`, `winsorize`.
- Inspected [transform tests](../../../../finstack-quant/features/tests/transforms.rs), `transform_cross_sectional_supports_mvp_signal_cleaning_ops`: [1,2,2,4] absolute clipping, sigma=.5 expected limits, and .25/.75 Type-7 winsorization. Numerical comparisons use 1e-12 absolute tolerance.
- Inspected [audit regressions](../../../../finstack-quant/features/tests/audit_regressions.rs) check invalid bounds and probabilities even on empty/not-ready inputs, and direct/grouped dispatch consistency.

No independent robust-statistics benchmark was inspected.

## Follow-up tracking

Pending choices, implementation reconciliation, and qualification work are tracked
in the [feature follow-ups](../FOLLOW_UPS.md#features).
