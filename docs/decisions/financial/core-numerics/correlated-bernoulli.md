# CORE-NUMERICS-16: Feasible correlated Bernoulli joint laws

- Decision status: **Draft — observed implementation, adoption pending**.
- Implementation status: **Observed**, within the path scope below.
- Validation status: **Evidence inspected/located, not executed**. Test roles and limits are stated below; this record is not a correctness certification.
- Audit date: **2026-11-05**; source revision: `7e82fd03ad98346727769e88dd5ac5d84b56e9fc`. [Slice scope and coverage](README.md).

## Scope and input domain

`math/probability.rs`: two Bernoulli events with decimal marginal probabilities p1,p2∈[0,1] and requested Pearson correlation rho∈[-1,1], all finite. This is a two-event joint law, not a general high-dimensional copula.

## Choice and rationale

The implementation projects requested correlation onto the Fréchet-feasible interval while retaining both requested and effective correlation. This keeps joint probabilities compatible with marginals. Whether projection is permitted in a financial model is not implied by the constructor succeeding.

## Algorithm

1. Compute v_i=p_i(1−p_i). If either variance is below 1e-10, assign effective rho=0 and independent joint masses, while preserving requested rho.
2. Otherwise compute rho bounds from `(max(p1+p2−1,0)−p1 p2)/sqrt(v1 v2)` and `(min(p1,p2)−p1 p2)/sqrt(v1 v2)`, each clamped into [-1,1]. Clamp requested rho to those bounds.
3. Let c=rho_eff sqrt(v1 v2). Set P11=p1p2+c, P10=p1(1−p2)−c, P01=(1−p1)p2−c and P00=(1−p1)(1−p2)+c.
4. A supplied u∈[0,1] selects outcomes in order (1,1), (1,0), (0,1), (0,0), using strict comparisons with cumulative mass. Thus u=1 always selects the final outcome even if its mass is zero.
5. Conditional `P(X2=1|X1=1)=P11/p1` and its symmetric counterpart use the other marginal when the conditioning event has zero probability.

## Selection, controls and conventions

Projection and near-degeneracy threshold are fixed; no strict-rejection option exists. Randomness is supplied as a uniform scalar; the type itself has no seed or RNG. `correlation_bounds` returns (0,0) for the near-degenerate branch.

## Limitations, failure and fallback

Invalid input ranges error, but infeasible correlations within [-1,1] are silently changed. Tiny nonzero marginal variances below threshold lose requested dependence. Conditional values on impossible events are defined by a fallback convention, not Bayes' rule. Boundary roundoff in joint masses is not separately repaired or reported.

## Alternatives and tradeoffs

Strict feasibility rejection, a warning-bearing constructor, or conditional-probability parameterization would expose different contracts; none is available. Explicit effective-correlation access lets consumers record the projection, provided they choose to inspect it.

## Evidence

- [Implementation and inline tests](../../../../finstack-quant/core/src/math/probability.rs). `test_joint_probabilities_marginals`, `test_correlation_bounds`, and `test_correlated_bernoulli_reports_effective_correlation` verify algebraic/projection invariants; uniform-partition tests check fixed outcome boundaries.
- [Probability context](../../../REFERENCES.md#glasserman-2004-monte-carlo); a direct Fréchet-bound adoption/reference entry is not established in the inspected catalog.

## Follow-up tracking

Adoption questions, implementation reconciliation and missing evidence are tracked in the
[core-numerics follow-ups](../FOLLOW_UPS.md#core-numerics).
