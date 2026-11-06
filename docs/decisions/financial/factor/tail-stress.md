# FACTOR-009: Whole-scenario tail stress attribution

**Decision status:** Draft — observed implementation; adoption pending.

**Implementation status:** Observed for the stated source scope; limitations and discrepancies below remain open.

**Validation status:** Evidence inspected or located, not executed. No numerical qualification is implied.

**Audit date:** 2026-11-05. **Source revision:** `7e82fd03ad98346727769e88dd5ac5d84b56e9fc`.

## Choice and rationale

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

Sharing the fractional ES weights would reconcile aggregate ES but would reduce the simplicity of a list of actual scenarios. Reporting all boundary ties would remove arbitrary selection but change count/weights. Both are different products and require an explicit choice.

## Implementation and evidence

- [src/risk/position.rs](../../../../finstack-quant/factor/src/risk/position.rs) — inspected implementation.
- [src/risk/mod.rs](../../../../finstack-quant/factor/src/risk/mod.rs) — inspected implementation.

[src/risk/position.rs](../../../../finstack-quant/factor/src/risk/position.rs) tests locate multi-scenario arithmetic averages, rational boundary snapping and underspecified-tail rejection. Historical ES tie tests do not establish stress tie invariance. No independent stress attribution reference is attached to the exact whole-count rule.

## Follow-up tracking

Implementation, adoption and validation work is tracked in the
[scope follow-ups](../FOLLOW_UPS.md#factor-models). This record does not
authorize a runtime change or certify the observed method.
