# Long-only covariance risk-budget iteration

- ID: PORTFOLIO-ATTRIBUTION-002
- Decision status: Draft (observed implementation; adoption pending).
- Implementation status: Observed for the stated owning source scope; discrepancies are named below.
- Validation status: Evidence inspected/located, not executed; independent accuracy qualification remains open.
- Audit date: 2026-11-05.
- Source revision: `7e82fd03ad98346727769e88dd5ac5d84b56e9fc`.

## Choice and domain

RiskBudget seeks nonnegative fully invested sleeve weights whose fractional contributions to variance equal caller-supplied budgets. The input covariance is an `n × n` matrix in the same return units and horizon as the intended risk budgets; no covariance estimator or annualization is performed. This fixed-point method and its rejection of nonpositive risk contributions are observed choices.

## Algorithm and numerical controls

1. Require finite matrix entries, exact shape, and pairwise absolute symmetry differences no greater than `1e-10`. Call the core pivoted Cholesky/PSD validator (`cholesky_correlation`); despite that helper's name, this path passes a covariance matrix and does not impose unit diagonal.
2. Require finite nonnegative budgets summing to one within `1e-9`. Remove exactly zero-budget sleeves, assigning them zero weight. One active sleeve receives weight one; full-portfolio contribution calculation still follows.
3. Initialize active weights from their budgets and normalize. Compute `m=Sigma*w` using compensated row sums; `v=w' Sigma w`; reject `v<=1e-9`. Fractional contributions are `RC_i=w_i*m_i/v`.
4. If `max_i |RC_i-budget_i|<=1e-10`, return weights in original sleeve order. Otherwise require every active contribution greater than `1e-9`, update `w_i <- w_i*sqrt(budget_i/RC_i)`, normalize and repeat.
5. Stop after 10,000 iterations with an explicit convergence error. Expand the active solution back to the full vector; recalculate and report contribution fractions; apportion capital separately.

The variance threshold is an absolute threshold in the caller's squared-return units. Multiplying every covariance entry by a small positive constant can therefore change acceptance even though theoretical fractional-risk weights are scale invariant. Negative correlations can yield a nonpositive marginal contribution and fail despite a PSD covariance.

## Alternatives and tradeoffs

Convex risk-budget formulations, coordinate descent and constrained nonlinear solvers may cover wider covariance domains. This implementation has a small deterministic state and explicit stopping criteria but provides no global convergence certificate. Covariance shrinkage, regularization and nearest-PSD repair are not automatically applied. The Cholesky result is validation evidence, not a repair step.

## Evidence

[Owning solver and inline tests](../../../../finstack-quant/portfolio/src/allocation.rs), [integration fixtures](../../../../finstack-quant/portfolio/tests/weight_allocation.rs), and [matrix factorization methodology](../core-numerics/matrix-factorization.md) establish implementation and local invariant evidence. [Meucci](../../../REFERENCES.md#meucci-risk-and-asset-allocation) and [Tasche](../../../REFERENCES.md#tasche-2008-capital-allocation) give risk-allocation context; no source-to-source independent solver benchmark is claimed.

Extreme finite-input overflow is not independently qualified.

## Follow-up tracking

Implementation, adoption and validation work is tracked in the
[scope follow-ups](../FOLLOW_UPS.md#portfolio-attribution). This record does not
authorize a runtime change or certify the observed method.
