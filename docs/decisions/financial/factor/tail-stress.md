# FACTOR-009: Historical tail stress attribution

**Decision status:** Accepted 2026-11-17 for shared fractional tail attribution, scenario evidence and signed summaries.

**Implementation status:** Whole-scenario stress details and summaries are implemented. Shared fractional weights, stable scenario identities and gain-tail semantics remain pending.

**Validation status:** All six focused stress tests passed on 2026-11-17 against current behavior. An exact example verified fractional tie weights and position closure. Shared weighting and revised gain-tail results remain to be qualified.

**Audit date:** 2026-11-05. **Source revision:** `7e82fd03ad98346727769e88dd5ac5d84b56e9fc`.

## Accepted decision

1. Share the [historical risk](position-historical-risk.md) admission and fractional tail selection, including explicit confidence, minimum tail mass and equal probability mass across exact boundary ties. List every scenario with positive tail weight. Retain its stable ID, signed portfolio and position P&L, and applied weight. Order by ascending portfolio P&L and then scenario ID.
2. Reconcile weighted position P&L to the raw portfolio tail mean. Report each position's minimum P&L among selected scenarios. With tail mass `2.5`, two full observations and two tied boundary observations carry weights `1, 1, 0.25, 0.25`. Report four selected scenarios and tail mass `2.5`.
3. Preserve signed scenario and mean P&L in gain, zero and loss tails. Report signed position loss shares only when aggregate tail mean is negative; shares are undefined for flat or positive means. Check finite arithmetic and closure before publication.

## Observed choice and rationale

Report the worst whole-number tail scenarios and their position P&Ls as an explanatory stress view. The observed method differs from fractional-mass historical ES and must not be presented as the same allocation.

## Input domain and conventions

Scenario-major finite P&Ls and position IDs share a currency/horizon. Confidence defaults to 0.95. A nonempty input requires at least one full expected tail observation within the 1e-9 snap tolerance. Empty dimensions with empty storage return an empty zero report before confidence validation.

## Algorithm stages

1. Validate shape and finite input, sum portfolio P&L per scenario and stable-sort ascending.
2. Use the shared snapped ceil tail count K. Threshold is min(P&L at K-1,0). Keep exactly the first K scenarios with original indices and position-order vectors.
3. Average each position over these K scenarios, and compute its minimum P&L within the selected portfolio tail.
4. Compute percent of tail loss as -average position P&L divided by negative average portfolio P&L, or zero if the denominator magnitude is <= machine epsilon.
5. Sort the position summary by descending absolute average tail P&L, breaking ties by position ID; preserve original position order separately for scenario vectors.

## Selection and controls

The historical report wrapper computes both historical decomposition and this stress report. Standalone stress permits one tail scenario whereas historical decomposition requires two. No fraction of the Kth scenario is used; exact boundary ties are selected by stable input order rather than shared probability mass.

## Limitations, failures, and fallbacks

For fractional tail mass and ties, averages can differ from historical ES components and depend on boundary-tie scenario order. All-gain tails retain positive averages and ratio-based shares even though the VaR threshold is zero. `worst_scenario_pnl` is worst only within the portfolio tail, not over every scenario. Duplicate IDs and nonfinite aggregate sums are not explicitly rejected.

## Alternatives and tradeoffs

Shared fractional weights make the explanatory breakdown reconcile to the historical tail mean. Scenario P&L remains visible at its original value with a separate applied weight. Stable IDs distinguish scenario order from tail membership.

## Implementation and evidence

- [src/risk/position.rs](../../../../finstack-quant/factor/src/risk/position.rs) — inspected implementation.
- [src/risk/mod.rs](../../../../finstack-quant/factor/src/risk/mod.rs) — inspected implementation.

[Stress tests](../../../../finstack-quant/factor/src/risk/position.rs) passed for current whole-scenario averages, boundary selection and underspecified tails. An exact fixture with portfolio P&Ls `−120`, `−80`, `−40`, `−40` at the accepted weights above yields raw ES `−88`; its position means `−56` and `−32` reconcile. Qualify stable tie output, raw tail closure and gain and zero tails in the accepted implementation.

## Follow-up tracking

Implementation reconciliation and qualification are tracked in the
[scope follow-ups](../FOLLOW_UPS.md#factor-models).
