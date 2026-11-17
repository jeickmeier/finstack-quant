# PORTFOLIO-ATTRIBUTION-011: Linear metric objectives, average constraints and turnover

- ID: PORTFOLIO-ATTRIBUTION-011
- Decision status: Accepted 2026-11-16 for checked continuous linear optimization.
- Implementation status: Observed for the stated owning source scope. Runtime, solver verification and host reconciliation with the accepted contract are pending.
- Validation status: Independent net/gross/turnover and fixed and invalid average-denominator examples were checked. Production solver, reconstruction and host qualification remain pending.
- Audit date: 2026-11-05.
- Source revision: `7e82fd03ad98346727769e88dd5ac5d84b56e9fc`.

## Accepted decision for checked linear optimization

1. Build linear objectives and bounds from the exact scopes and compatible coefficients admitted by [record 010](10-optimization-decision-space.md#accepted-decision-for-checked-optimization-decision-space). Include fixed held contributions in objectives and constraints, including known additive risk on a zero-PV holding. Preserve original coefficients, units and stable constraint IDs. Display labels may repeat without replacing diagnostic rows.
2. Keep signed net budget, gross exposure and full L1 turnover separate. Remove the synthesized `sum(weights)=1` row. An explicit net target may be positive, zero or negative. Gross upper caps measure the sum of absolute target exposures. Turnover caps measure the full sum of absolute target-minus-opening changes, including entries, exits and reversals. Use the frozen normalized basis or its corresponding monetary amounts. TargetScale requires a qualified common monetary map for these controls. Recompute achieved gross and turnover from the accepted targets. [Leverage and turnover formulations](https://docs.mosek.com/portfolio-cookbook/markowitz.html#turnover-constraints).
3. Define a value-weighted average over actual target base-currency PV in the selected scope. Require nonnegative PV contributions in that scope and an explicit positive minimum basket value or a fixed positive basket value. For numerator `N`, denominator `D` and bound `c`, include the denominator requirement in the LP together with `N-c*D OP 0`. An empty or zero-value basket cannot satisfy an average constraint.
4. Allow an average objective only when the same scoped positive denominator is fixed to a known value by an explicit model equality. Optimize the resulting linear objective and report the true `N/D`, including when the fixed denominator differs from one. Decision-dependent average objectives are outside this LP contract.
5. Retain the pinned continuous `good_lp`/`microlp` backend. Check model size and finite coefficients before solving and allow an explicit positive time limit. Only a backend Optimal status proceeds to independent verification. Recompute the objective and every original constraint on reconstructed target sizes, including exact held quantities, physical bounds, average denominators, net, gross and turnover. Apply explicit absolute tolerances in the relevant row units where authorized. Structural requirements and unrelaxed limits must pass. Backend numerical tolerances do not define portfolio policy.
6. Distinguish a verified solution, backend-reported infeasibility or unboundedness, a stopped solve, and computation or verification failure. Unsuccessful outcomes carry typed diagnostics. Numerical results, target allocations and trades are absent. Use one checked Rust result contract across hosts. [Record 016](16-optimization-results-and-rebalance.md#accepted-decision-for-checked-optimization-results-and-rebalance) owns admission for rebalance.

Independent examples checked a book with `130` long and `30` short, giving net `100` and gross `160`. Moving to `110` long and `10` short retains net `100`, reduces gross to `120` and produces `40` full turnover. A separate average example showed that the observed post-solve check can reject a negative-denominator optimum even when a feasible positive-denominator solution exists.

## Observed choice and lowering at the audit revision

The implementation uses `good_lp` with the `microlp` backend selected in the portfolio manifest. It solves continuous weights with frozen valuation coefficients; no integer lots, solver-independent optimality certificate, custom tolerance or time-limit option is supplied here. Validation of scalar bounds precedes lowering: metric RHS finite, weight bounds finite and ordered, budget and turnover finite and nonnegative. Duplicate Budget or MaxTurnover constraints error.

## Algorithm and numerical semantics

- WeightedSum objectives and bounds use `sum w_i*m_i` over their filter. Missing-data policy and candidates follow [decision construction](10-optimization-decision-space.md).
- An unfiltered ValueWeightedAverage objective is lowered to the same numerator. It requires an exact explicit budget `1.0`, or the synthesized sum-of-weights-one budget. Any other explicit budget errors. A filtered average objective is rejected because its denominator is decision dependent.
- An average bound is lowered to `sum_matched w_i*(m_i-rhs) OP 0`. The mask excludes filtered-out and Exclude-missing positions. After solving, a matched net weight below `-1e-9` errors because a negative denominator reverses an inequality. A near-zero denominator is accepted as vacuous; reported average slack is NaN at absolute denominator at most `1e-9`.
- Without explicit Budget, ValueWeight/NotionalWeight receive `sum w=1`; UnitScaling errors even if other bounds could make it finite. UnitScaling's budget sums heterogeneous multipliers/quantities, not invested capital.
- Each held variable is fixed at current weight; other variables receive the intersected bounds. For turnover, introduce `t_i>=0`, `t_i>=w_i-w0_i`, `t_i>=w0_i-w_i`, then `sum t_i<=cap`. This is full L1 turnover, with no one-half multiplier and no notional/transaction-cost conversion.
- Build the selected maximize/minimize linear objective and solve. Infeasible, unbounded and backend failures become structured result statuses with NaN objective/turnover and empty solution maps. Invalid inputs propagate errors instead. No infeasibility irreducible-set diagnosis is available here.

## Discrepancies and limitations

Under Exclude, an unfiltered average objective sets missing coefficients to zero while frozen missing positions still consume budget. It therefore reports the included numerator, not `sum_included w*m / sum_included w`. With positive fixed omitted weight the optimization direction can remain the same but the objective's value is scaled. The exact-one budget guard alone does not restore the advertised denominator.

Average-bound validation occurs after optimization; the model does not constrain a negative filtered denominator to be nonnegative during the solve. It can reject the returned optimum instead of finding another admissible optimum. A zero-net signed basket can have a nonzero numerator, so the lowered row is not universally vacuous despite near-zero-denominator reporting. Apart from this denominator check there is no general post-solve finite/constraint-residual verification layer.

## Alternatives and evidence

Linear-fractional transformations, explicit denominator constraints, nonlinear programs or forbidding signed average baskets would express different admissibility contracts. The current small LP supports fast interpretable linear constraints but requires clear weight/metric units.

[LP implementation and inline tests](../../../../finstack-quant/portfolio/src/optimization/lp_solver.rs), [constraint validation](../../../../finstack-quant/portfolio/src/optimization/constraints.rs), [backend selection](../../../../finstack-quant/portfolio/Cargo.toml), [integration fixes](../../../../finstack-quant/portfolio/tests/test_optimization_fixes.rs) are owning and regression evidence; no solve was executed.

## Follow-up tracking

Implementation and independent qualification are tracked in the
[scope follow-ups](../FOLLOW_UPS.md#portfolio-attribution).
