# PORTFOLIO-ATTRIBUTION-011: Linear metric objectives, average constraints and turnover

- ID: PORTFOLIO-ATTRIBUTION-011
- Decision status: Draft (observed implementation; adoption pending).
- Implementation status: Observed for the stated owning source scope; discrepancies are named below.
- Validation status: Evidence inspected/located, not executed; independent accuracy qualification remains open.
- Audit date: 2026-11-05.
- Source revision: `7e82fd03ad98346727769e88dd5ac5d84b56e9fc`.

## Choice and lowering

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

Implementation, adoption and validation work is tracked in the
[scope follow-ups](../FOLLOW_UPS.md#portfolio-attribution). This record does not
authorize a runtime change or certify the observed method.
