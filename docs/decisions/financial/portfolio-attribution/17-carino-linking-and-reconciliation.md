# PORTFOLIO-ATTRIBUTION-017: Carino linking and supplied-attribution validation

- ID: PORTFOLIO-ATTRIBUTION-017
- Decision status: Draft (observed implementation; adoption pending).
- Implementation status: Observed for the stated owning source scope; discrepancies are named below.
- Validation status: Evidence inspected/located, not executed; independent accuracy qualification remains open.
- Audit date: 2026-11-05.
- Source revision: `7e82fd03ad98346727769e88dd5ac5d84b56e9fc`.

## Choice and domain

A shared Carino kernel links additive single-period effects to compounded active return. Brinson and Campisi provide detailed sector rows; grid attribution supplies three aggregate rows/effects. This method smooths timing interaction across effects, so a linked effect is not a direct sum of monetary P&L. Period ordering is the supplied sequence; the API does not enforce contiguous dated periods.

## Algorithm and numerical controls

1. Validate caller-supplied period returns/effects as finite and reconcile each period's effect sums to its declared totals and active return before linking. Brinson/Campisi require identical sector labels and ordering; grid linking validates top-level totals only and permits changing grids.
2. Compound portfolio and benchmark growth by direct products, giving `R_P=product(1+rP_t)-1` and `R_B=product(1+rB_t)-1`. Empty periods error. Every individual/horizon return must be greater than `-1` when its coefficient is evaluated.
3. Define `k(p,b)=[log(1+p)-log(1+b)]/(p-b)`. The implementation uses `log1p((p-b)/(1+b))/(p-b)` for stability. If `|p-b|<1e-12`, use the continuous limit `1/(1+p)`.
4. Let horizon coefficient be `K=k(R_P,R_B)`. Link each effect/row with compensated sum `E_linked=sum_t [k(rP_t,rB_t)/K]*E_t`.
5. Supplied effect reconciliation uses a scaled-L1 tolerance `1e-10*max(1,sum absolute effects)` with an overflow-aware scale accumulator. The active-return identity uses `1e-12*max(1,|R_P|,|R_B|,|active|)` and rejects nonfinite expected values/residuals. These tolerances qualify arithmetic consistency, not economic attribution accuracy.
6. The shared weight check requires finite total within `1e-6` of one. Group-return users additionally reject `|net|<=1e-6*gross` for positive-gross signed groups; this is separate from linking's logarithmic domain.

## Alternatives, failure behavior and tradeoffs

Arithmetic summation does not reconcile to compounded active return; geometric attribution and other linking algorithms distribute interaction differently. Carino provides an additive linked identity with a continuous equal-return limit. Direct products can overflow/underflow over extreme or long horizons, and the coefficient's near-equality threshold is absolute rather than relative. The kernel checks equal row count; public adapters impose the stronger named-row checks where described.

A valid single-period Brinson or Campisi report can fail linking if its total return is at or below `-100%`. Effect magnitudes may be large and offset; tolerances scale to gross effect magnitude rather than a near-zero net amount. No generic correction rewrites inconsistent caller-supplied reports to reconcile them.

## Evidence

[Shared kernel, validation helpers and inline tests](../../../../finstack-quant/portfolio/src/brinson.rs), [Campisi adapter](../../../../finstack-quant/portfolio/src/fi_attribution.rs), [grid adapter](../../../../finstack-quant/portfolio/src/grid_attribution.rs) are owning sources. [Carino](../../../REFERENCES.md#carino-1999) is the repository formulation reference. Formula/identity tests were inspected/located, not executed; no independent multi-period engine comparison is claimed.

## Follow-up tracking

Implementation, adoption and validation work is tracked in the
[scope follow-ups](../FOLLOW_UPS.md#portfolio-attribution). This record does not
authorize a runtime change or certify the observed method.
