# FEATURES-019: Complete-pair rolling covariance, correlation and beta

**Decision status:** Draft — observed implementation; adoption pending for this scoped choice. Existing accepted decisions retain their own scope.

**Implementation status:** Observed in the exact source paths and symbols below; discrepancies and limitations are stated explicitly.

**Validation status:** Source and the named test bodies inspected, not executed. This record does not certify numerical correctness or host parity.

**Audit date / revision:** 2026-11-05 / `7e82fd03ad98346727769e88dd5ac5d84b56e9fc`. See [scope and source ledger](README.md).

## Scope and selection

`transform_timeseries_pairwise` selects `RollingCov`, `RollingCorr` or `RollingBeta`. `values` is dependent y and `other` is regressor x for beta. Required arrays are row aligned and share entity/order sorting; no join or benchmark date intersection is performed. Window defaults to 1, min_periods to window, and at least two finite pairs are always required.

## Ordered computation

1. In each inclusive trailing row window, retain a row only if both y and x are finite. Require at least max(min_periods,2) retained pairs. A missing current pair does not prohibit an aggregate from being emitted from earlier pairs in the window.
2. Scale and center y and x separately, retaining max-absolute scales a and b. Call core sample covariance on the two centered arrays; core uses its online covariance accumulator with denominator n-1. Call core sample variance for denominators.
3. Covariance returns `(cov_normalized*a)*b`. Units are y-units times x-units; sample division occurs before restoring scales.
4. Correlation returns `clamp((cov_normalized/std_y)/std_x,-1,1)` when both normalized SDs are positive; otherwise 0. The scales cancel and output is dimensionless. Clamp suppresses finite rounding excursions beyond correlation bounds.
5. Beta returns `((cov_normalized/var_x)*a)/b` when normalized x variance is positive; otherwise 0. It has y-units/x-units and equals a conventional dimensionless return beta only when both series use compatible return units. Constant y with variable x gives zero slope; constant x gives the explicit zero substitution.

Fewer than two pairs means None, whereas zero variance means 0. A nonfinite emitted covariance/beta fails the whole call. Rescaling multiplication/division order can overflow or underflow before the final mathematically scaled value, and finite underflow is not detected by final validation.

## Rationale, alternatives and evidence

The source specifies complete pairs, sample covariance and dependent-variable direction. Proposed rationale is alignment without imputation and unit-stable correlation. Pairwise deletion may produce different populations across multiple separately computed pairs; this API does not construct or repair a joint covariance matrix. Undefined/error zero-variance outputs, regularized beta and a common complete-case universe are alternatives, not implemented selections.

- Owning [pairwise path](../../../../finstack-quant/features/src/multi.rs): `transform_timeseries_pairwise`, `pairwise_value`; delegated [core statistics](../../../../finstack-quant/core/src/math/stats.rs) `covariance`, `variance`. [Welford reference](../../../REFERENCES.md#welford-1962) is background for the delegated accumulation, not a validation of feature rescaling.
- Inspected [transform tests](../../../../finstack-quant/features/tests/transforms.rs) expect correlation .9819805060619657 and beta 9/14 for a three-point pair; [audit regressions](../../../../finstack-quant/features/tests/audit_regressions.rs) check self-correlation/beta scale invariance and row gaps. The inspected feature tests do not include an independent rolling-covariance numeric fixture.

No such extreme case was executed.

## Follow-up tracking

Pending choices, implementation reconciliation, and qualification work are tracked
in the [feature follow-ups](../FOLLOW_UPS.md#features).
