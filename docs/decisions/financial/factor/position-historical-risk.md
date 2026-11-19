# FACTOR-008: Historical tail mass and ES-proportional VaR allocation

**Decision status:** Accepted 2026-11-17 for empirical boundary VaR, fractional ES and ES-proportional position allocations.

**Implementation status:** Boundary VaR, fractional ES, tie sharing and ES-proportional VaR allocation are implemented in part. Explicit inputs, unsnapped tail mass, raw statistics and checked reconciliation remain pending.

**Validation status:** All 18 tests selected by the historical filter passed on 2026-11-17 against current behavior. An independent exact example verified fractional tail VaR and ES. The accepted boundary and gain-tail rules still need qualification.

**Audit date:** 2026-11-05. **Source revision:** `7e82fd03ad98346727769e88dd5ac5d84b56e9fc`.

## Accepted decision

1. Admit a checked equal-weight scenario panel with unique ordered position IDs, finite P&L in one currency and horizon, and explicit confidence. A nonempty portfolio requires at least two observation-equivalents of tail mass. Report actual tail mass and retain the distinction between calculation eligibility and statistical adequacy.
2. Set `m = (1 − confidence) × N` using the admitted numeric inputs, preserving consequential fractional mass without near-integer snapping. Use the empirical boundary at rank `ceil(m)` for VaR. For ES, give full weight to scenarios below the boundary and share the remaining mass equally across exact boundary ties. Use the same weights for portfolio and position ES. With mass `2.5` and the three worst observations `−100`, `−50` and `−10`, raw VaR is `−10` and raw ES is `−62`.
3. Retain the raw signed tail statistics and report loss measures as `min(raw, 0)`. When VaR represents a loss, scale signed position ES allocations by `VaR / ES` and label the result ES-proportional VaR allocation. Use checked arithmetic without a small-ES cutoff. A zero reported measure has zero risk allocations and undefined shares. Historical marginals and removal risk are unavailable. Check finite results and allocation closure before publication.

## Observed choice and rationale

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

[Historical position-risk tests](../../../../finstack-quant/factor/src/risk/position.rs) passed against the current tail boundary, ties, replication and gain-floor behavior. Eight scenarios and confidence `11/16` give mass `5/2` exactly in the independent example above. Qualify the accepted tail boundary, small nonzero losses, gain-tail outputs, reconciliation and sample disclosure.

## Follow-up tracking

Implementation reconciliation and qualification are tracked in the
[scope follow-ups](../FOLLOW_UPS.md#factor-models).
