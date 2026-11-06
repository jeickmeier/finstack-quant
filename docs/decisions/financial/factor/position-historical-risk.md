# FACTOR-008: Historical tail mass and ES-proportional VaR allocation

**Decision status:** Draft — observed implementation; adoption pending.

**Implementation status:** Observed for the stated source scope; limitations and discrepancies below remain open.

**Validation status:** Evidence inspected or located, not executed. No numerical qualification is implied.

**Audit date:** 2026-11-05. **Source revision:** `7e82fd03ad98346727769e88dd5ac5d84b56e9fc`.

## Choice and rationale

Use empirical portfolio P&L order statistics for VaR, fractional probability mass for ES, and allocate VaR proportionally to ES components. Historical component VaR is therefore a selected allocation rule, not a direct derivative or conditional-at-VaR estimate.

## Input domain and conventions

Input is scenario-major flat position P&L in one common currency/horizon. `flatten_position_pnls` transposes caller position-major nested rows after rectangular-shape checks. Confidence defaults to 0.95, must lie in (0.5,1), and every P&L must be finite. No scenario probabilities, recency weights or sampling inference are supplied.

## Algorithm stages

1. Sum each scenario across positions, then stable-sort portfolio P&Ls ascending. Set raw tail mass m=(1-alpha)N; snap within 1e-9 of an integer, otherwise take ceil(m) for the boundary rank. Require at least two boundary-count scenarios; warn if below 30.
2. VaR is min(P&L at rank ceil(m)-1,0).
3. For ES, include all observations strictly below the boundary with weight 1. Split the remaining mass m-count_below equally across all observations tied at the boundary. Divide weighted portfolio/position sums by m.
4. Floor a positive portfolio ES to zero and zero components when its raw ES is positive.
5. Allocate component VaR_i = component ES_i * (VaR/ES), using zero ratio when |ES|<=1e-12. Relative fields use the same magnitude cutoff; historical marginals, incremental VaR and Euler residual are None.

## Selection and controls

Tail-count snapping is shared with stress attribution, but the latter uses whole-scenario weights. Zero positions/scenarios return zero before the minimum-tail and P&L checks, after confidence/shape validation. Exact P&L ties share boundary mass, preserving allocation under scenario replication and tie reordering.

## Limitations, failures, and fallbacks

The ES-proportional allocation can differ materially from true VaR marginal allocation, especially for nonlinear positions. A tiny ES can zero VaR components despite a nonzero VaR. Finite row entries do not guarantee finite portfolio or tail sums; no post-accumulation finite guard is present. IDs are not deduplicated. The ceil-tail minimum is not a statistical adequacy test.

## Alternatives and tradeoffs

Conditional VaR-band attribution, smoothed quantile derivatives, or bump-and-recompute VaR provide different marginal interpretations and stability tradeoffs. Weighted historical sampling would need an explicit probability contract. Interpolated R7 quantiles would change the boundary estimator and are not silently interchangeable.

## Implementation and evidence

- [src/risk/position.rs](../../../../finstack-quant/factor/src/risk/position.rs) — inspected implementation.
- [src/risk/mod.rs](../../../../finstack-quant/factor/src/risk/mod.rs) — inspected implementation.

[src/risk/position.rs](../../../../finstack-quant/factor/src/risk/position.rs) contains inspected fractional-tail examples: 250 observations at 99% use mass 2.5 and yield ES -62 for losses -100,-50,-10; boundary ties distribute component ES independently of input order. Replication invariance is an invariant test, not independent market validation. [Hallerbach](../../../REFERENCES.md#hallerbach-2003-decomposing-var) is a conceptual comparison, not proof of the ES-proportional rule.

## Follow-up tracking

Implementation, adoption and validation work is tracked in the
[scope follow-ups](../FOLLOW_UPS.md#factor-models). This record does not
authorize a runtime change or certify the observed method.
