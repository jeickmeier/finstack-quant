# ATTRIBUTION-012: Yield-share credit carry and hierarchy-anchor allocation

- Decision status: Draft (observed implementation; adoption pending).
- Implementation status: Observed for the source scope listed below.
- Validation status: Source evidence inspected; named test evidence located, not executed. No numerical accuracy certification.
- Audit date: 2026-11-05. Source revision: `7e82fd03ad98346727769e88dd5ac5d84b56e9fc`.

## Choice and inputs

With credit factor model, eligible issuer metadata and a carry detail containing coupon income, split gross carry between rates and credit using an approximate risky-yield share. Use only the first declared opening hazard curve and first opening discount curve. The lookup date is instrument expiry, or five years from opening using metadata calendar/calendar_id (default `usny`) and Following adjustment. Calendar lookup is strict; date-add failure falls back to truncated 5×365.25 days, then opening date if that also overflows.

## Algorithm

1. Read continuous discount zero rate r and instantaneous hazard h at lookup date; individual lookup failures become zero with notes. Define spread approximation `s=h*(1-recovery)` (credit triangle).
2. Denominator is stable when `abs(r+s) > 1e-12*max(abs(r),s,0.001)`. If s>0 and total yield is nonpositive or unstable, set credit share q=1 with a note. Otherwise q=`clamp(s/(r+s),0,1)` for a stable denominator, or zero.
3. Split coupon income and pull-to-par with q; assign all roll-down to rates. Funding remains outside these gross rates/credit carry totals. Preserve original aggregate carry and annotate coupon/roll SourceLines.
4. Allocate credit carry using factor-model anchor spread: generic=`beta_pc*anchor_pc`, each level=`beta_level*anchor_bucket`, plus issuer adder-at-anchor. Missing beta/anchor entries contribute zero. If the sum has absolute value above 1e-15, multiply each component by credit-carry-total/sum. Otherwise allocate all credit carry to the adder. Per-level/per-issuer detail follows the same options as credit P&L.

## Conventions and limitations

This is a yield-share presentation, not a revaluation of credit versus rates carry or a calibrated recovery model. The first-curve choice and assigning every roll-down dollar to rates are explicit approximations. Hierarchy proportions use anchor levels, not observed factor moves; negative anchor components can produce signed allocations. Postprocessing errors become outer request notes and do not change headline credit/carry. Some SourceLines may already be mutated when a later detail construction errors.

## Rationale and alternatives

The source cites credit-triangle motivation; [O'Kane](../../../REFERENCES.md#o-kane-2008) is background for the spread approximation, not approval of this carry allocation. Repricing risk-free/hazard counterfactual carry or term-by-term cashflow allocation are unimplemented alternatives. Tests `carry_partition_includes_pull_to_par`, `per_factor_allocation_sums_to_credit_carry_total`, and `negative_total_yield_routes_coupon_to_credit` are formula/identity checks; independent economic benchmarks remain open.

## Source and evidence

- [credit_decomposition.rs](../../../../finstack-quant/attribution/src/credit_decomposition.rs) — owning implementation.
- [credit_cascade.rs](../../../../finstack-quant/attribution/src/credit_cascade.rs) — owning implementation.
- [detail.rs](../../../../finstack-quant/attribution/src/detail.rs) — owning implementation.

## Follow-up tracking

Implementation, adoption and validation work is tracked in the
[scope follow-ups](../FOLLOW_UPS.md#attribution). This record does not
authorize a runtime change or certify the observed method.
