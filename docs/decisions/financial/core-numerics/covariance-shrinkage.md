# CORE-NUMERICS-11: Ledoit–Wolf covariance shrinkage toward a scaled identity

- Decision status: **Draft — observed implementation, adoption pending**.
- Implementation status: **Observed**, within the path scope below.
- Validation status: **Evidence inspected/located, not executed**. Test roles and limits are stated below; this record is not a correctness certification.
- Audit date: **2026-11-05**; source revision: `7e82fd03ad98346727769e88dd5ac5d84b56e9fc`. [Slice scope and coverage](README.md).

## Scope and input domain

`math/linalg/mod.rs::ledoit_wolf_shrinkage`: finite row-major T×N observations, T≥2, N≥1. It estimates covariance in input units squared, without timestamp alignment, return conversion, factor selection or missing-data imputation.

## Choice and rationale

The explicit estimator shrinks a demeaned population-normalized sample covariance toward its mean diagonal. Common scaling before second/fourth moments avoids overflow and preserves shrinkage under uniform rescaling. Identity-target suitability is an adoption choice; heterogeneous units are not standardized independently.

## Algorithm

1. For each column, divide observations by that column's maximum absolute value, compute a Neumaier mean, restore the scale, and subtract the mean. Reject nonfinite centered observations.
2. Divide all centered values by one common maximum absolute centered value. This common scale preserves cross-column relative variances.
3. Compute `S=XᵀX/T`, `mu=trace(S)/N`, `d²=Σ_ij(S_ij−mu I_ij)²/N`, and `bbar²=Σ_t Σ_ij(x_ti x_tj−S_ij)²/(N T²)`.
4. Set `b²=min(bbar²,d²)`, `delta=b²/d²` if d²>0 else 0. Form `Σ*=delta mu I+(1−delta)S`.
5. Restore each entry through `(normalized*scale)*scale`, avoiding a separately overflowing scale². Reject nonfinite restored covariance. Return covariance and delta.

## Selection, controls and conventions

This is a direct API, not a fallback on failed Cholesky. All arithmetic and folds are serial; there is no configurable target, shrinkage override or degrees-of-freedom adjustment. S divides by T, unlike the sample covariance helper's T−1. Row order and column order are caller-supplied.

## Limitations, failure and fallback

Shape multiplication is checked; all observations must be finite. All-constant data yields zero covariance/delta=0. An identity-like S has d²=0 and returns S. A PSD estimate is not necessarily strictly positive definite (for example all-zero data); no condition-number guarantee or confidence interval is returned. No matrix repair is invoked.

## Alternatives and tradeoffs

Raw sample covariance is retained at delta=0; identity target dominates at delta=1. Correlation shrinkage, constant-correlation targets, factor models and robust estimates are not choices of this API. Its O(TN²) dense computation is appropriate only within the caller's size budget.

## Evidence

- [Implementation and inline `ledoit_wolf_tests`](../../../../finstack-quant/core/src/math/linalg/mod.rs), [Ledoit–Wolf reference](../../../REFERENCES.md#ledoitwolf2004).
- `ledoit_wolf_matches_hand_worked_two_factor_example` checks delta=17/18 and off-diagonal −1/12 (1e-14/1e-13 tolerances). Extreme-rescaling and column-shift cases test invariance; unrepresentable covariance errors are tested. These do not validate statistical performance out of sample.

The mathematical implementation alone does not approve a portfolio or factor risk model.

## Follow-up tracking

Adoption questions, implementation reconciliation and missing evidence are tracked in the
[core-numerics follow-ups](../FOLLOW_UPS.md#core-numerics).
