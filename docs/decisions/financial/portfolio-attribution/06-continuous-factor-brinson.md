# PORTFOLIO-ATTRIBUTION-006: Continuous-factor Brinson attribution with benchmark completeness

- ID: PORTFOLIO-ATTRIBUTION-006
- Decision status: Draft (observed implementation; adoption pending).
- Implementation status: Observed for the stated owning source scope; discrepancies are named below.
- Validation status: Evidence inspected/located, not executed; independent accuracy qualification remains open.
- Audit date: 2026-11-05.
- Source revision: `7e82fd03ad98346727769e88dd5ac5d84b56e9fc`.

## Choice and domain

This method accepts externally estimated factor returns and exposures, then separates factor allocation from active residual selection. It does not fit factor returns. Inputs are `n>0` assets, `k>0` factors, flat row-major `X` of length `n*k`, factor-return vector `f`, asset returns `r`, portfolio and benchmark weights, and labels. Parallel dimensions and every numerical entry are validated; each weight vector sums to one within `1e-6`. Short weights and duplicate labels are not categorically rejected.

## Algorithm

1. Compute portfolio and benchmark returns `R_P=wP'r`, `R_B=wB'r` with compensated accumulation and active weights `a=wP-wB`.
2. Compute active factor exposures `d=X'a`. The allocation effect is `sum_j d_j*f_j`; emit each factor's active exposure, factor return and contribution in factor input order.
3. Compute each asset's fitted return `X_i f` and residual `epsilon_i=r_i-X_i f`.
4. Require benchmark completeness `|wB'epsilon| <= 1e-7*max(1,|R_B|)`. Otherwise fail with guidance to use the analytics constrained least-squares fit. This prevents an unexplained benchmark residual from being silently relabeled selection.
5. Calculate selection directly as `sum_i a_i*epsilon_i`, rather than subtracting allocation from active return. Emit asset contributions in input order. With complete benchmark fitting, allocation plus selection reconciles to active return.

Binary sector exposure inputs reproduce the centered sector allocation and combined selection-plus-interaction under the stated completeness condition. No separate interaction term is reported for general continuous factors.

## Alternatives and evidence

Unconstrained OLS often leaves a nonzero benchmark residual; reporting that separately or imposing an equality constraint are alternatives. This API chooses an explicit completeness precondition. Factor model design, exposure standardization and constrained fit numerics belong to analytics, while this record owns acceptance and decomposition.

[Source and inline tests](../../../../finstack-quant/portfolio/src/factor_brinson.rs) cover identities, dimensions and incomplete-fit rejection. [Jeet–Partani](../../../REFERENCES.md#jeet-partani-2023) supplies the repository's cited formulation and constrained-fit context; independent numerical comparison was not executed.

No estimation provenance is embedded by this function.

## Follow-up tracking

Implementation, adoption and validation work is tracked in the
[scope follow-ups](../FOLLOW_UPS.md#portfolio-attribution). This record does not
authorize a runtime change or certify the observed method.
