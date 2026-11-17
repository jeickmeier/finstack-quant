# PORTFOLIO-ATTRIBUTION-006: Continuous-factor Brinson attribution with benchmark completeness

- ID: PORTFOLIO-ATTRIBUTION-006
- Decision status: Accepted 2026-11-16 for checked continuous-factor attribution.
- Implementation status: Observed for the stated owning source scope. Runtime, caller and host reconciliation with the accepted contract are pending.
- Validation status: Independent complete and incomplete fits, sector equivalence, rescaling, cancellation and output-range examples were checked. Runtime, host and numerical-range qualification of the accepted contract are pending. The paper's identity was verified. Publisher full text was unavailable for review.
- Audit date: 2026-11-05.
- Source revision: `7e82fd03ad98346727769e88dd5ac5d84b56e9fc`.

## Accepted decision for checked continuous-factor attribution

1. Require one dated, positive-length valuation interval, one reporting currency and compatible total-return, valuation, FX and fee bases. Use one realized period return per asset shared by both sides. Require exact signed decimal opening weights summing to one on each side and explicit cash and financing where applicable. Bind opening exposures and same-period factor returns to ordered, unique, nonblank asset and factor IDs. Check dimensions and workspace size. Retain benchmark, model, exposure and fit provenance. State each factor return per unit of its declared exposure.
2. Set active weights `a=wP-wB`, active factor exposures `d=Xᵀa` and asset residuals `ε=r-Xf`. Compute allocation directly as `sum_j d_j*f_j` and specific selection directly as `sum_i a_i*ε_i`. Retain ordered undemeaned factor contributions and ordered asset contributions. Consume supplied coefficients without automatic intercepts, refitting or a separate interaction effect. Collinearity belongs to factor fitting when the coefficients are supplied. [Jeet and Partani](../../../REFERENCES.md#jeet-partani-2023).
3. Require a caller-supplied nonnegative absolute tolerance in period-return units. Accept when `abs(wBᵀε)` is within tolerance, inclusively. Zero requests exact completeness. Remove `1e-7*max(1,abs(R_B))`. Report the applied tolerance, signed benchmark residual `wBᵀε` and gross absolute weighted residual `sum_i abs(wB_i*ε_i)` to expose cancellation. Acceptance uses the signed aggregate. Active-return closure holds for any supplied coefficient vector, so it does not establish benchmark completeness. [Analytics 014](../analytics/14-svd-and-constrained-regression.md) owns constrained fitting and its separate feasibility check.
4. Use bounded widened arithmetic on the exact decimal weights and exact binary fractions of admitted exposures and returns. Derive active return independently from the original inputs and verify allocation plus selection against it before projecting each required output once to binary64. Reject unrepresentable required outputs. Validate the entire request even when the weights are identical. Rust owns calculation, error classification and source-backed restoration across hosts.
5. For a one-hot sector partition use each held benchmark sector's benchmark-weighted return and use the whole-benchmark return for a genuinely absent benchmark sector. Total factor allocation then equals centered Brinson–Fachler allocation, and grouped specific contribution equals selection plus interaction. Individual factor rows remain undemeaned. Aggregate completeness alone does not establish this reduction. A signed group with zero net weight and nonzero contribution can be represented at the asset level. Its sector return is outside [record 005](05-brinson-fachler.md#accepted-decision-for-checked-single-period-brinsonfachler-attribution).

An independent two-asset check gave `+156.25 bp` active return from `-156.25 bp` allocation and `+312.5 bp` specific selection, with zero benchmark residual. A zero factor return still reconciled to `+156.25 bp` active return but left `937.5 bp` of benchmark return unexplained.

## Observed choice and domain at the audit revision

This method accepts externally estimated factor returns and exposures, then separates factor allocation from active residual selection. It does not fit factor returns. Inputs are `n>0` assets, `k>0` factors, flat row-major `X` of length `n*k`, factor-return vector `f`, asset returns `r`, portfolio and benchmark weights, and labels. Parallel dimensions and every numerical entry are validated; each weight vector sums to one within `1e-6`. Short weights and duplicate labels are not categorically rejected.

## Algorithm

1. Compute portfolio and benchmark returns `R_P=wP'r`, `R_B=wB'r` with compensated accumulation and active weights `a=wP-wB`.
2. Compute active factor exposures `d=X'a`. The allocation effect is `sum_j d_j*f_j`; emit each factor's active exposure, factor return and contribution in factor input order.
3. Compute each asset's fitted return `X_i f` and residual `epsilon_i=r_i-X_i f`.
4. Require benchmark completeness `|wB'epsilon| <= 1e-7*max(1,|R_B|)`. Otherwise fail with guidance to use the analytics constrained least-squares fit. This prevents an unexplained benchmark residual from being silently relabeled selection.
5. Calculate selection directly as `sum_i a_i*epsilon_i`, rather than subtracting allocation from active return. Emit asset contributions in input order. Allocation plus selection reconciles algebraically for any factor returns.

The observed binary-sector fixture also uses the benchmark sector returns as coefficients. A complete benchmark fit alone is insufficient for that reduction. The continuous-factor result has no separate interaction term.

## Alternatives and evidence

Unconstrained OLS often leaves a nonzero benchmark residual; reporting that separately or imposing an equality constraint are alternatives. This API chooses an explicit completeness precondition. Factor model design, exposure standardization and constrained fit numerics belong to analytics, while this record owns acceptance and decomposition.

[Source and inline tests](../../../../finstack-quant/portfolio/src/factor_brinson.rs) cover identities, dimensions and incomplete-fit rejection. [Jeet–Partani](../../../REFERENCES.md#jeet-partani-2023) supplies the cited formulation and constrained-fit context. Independent checks used local derivations and fixtures. No production tests ran for this review.

No estimation provenance is embedded by this function.

## Follow-up tracking

Implementation and independent validation are tracked in the
[scope follow-ups](../FOLLOW_UPS.md#portfolio-attribution).
