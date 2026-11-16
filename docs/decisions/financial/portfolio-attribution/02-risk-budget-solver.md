# PORTFOLIO-ATTRIBUTION-002: Long-only covariance risk-budget iteration

- ID: PORTFOLIO-ATTRIBUTION-002
- Decision status: Accepted 2026-11-16 for checked long-only covariance risk budgeting.
- Implementation status: Observed for the stated owning source scope. Runtime and host reconciliation with the accepted contract are pending.
- Validation status: Independent exact arithmetic and coordinate-descent examples were checked. Allocation tests were located. Runtime, host and numerical-range qualification of the accepted contract is pending.
- Audit date: 2026-11-05.
- Source revision: `7e82fd03ad98346727769e88dd5ac5d84b56e9fc`.

## Accepted decision for checked covariance risk budgeting

1. Require one exact nonnegative decimal budget `b_i` per ordered sleeve, with an exact total of one. Define long-only, fully invested model weights `w` and risk shares `RC_i=w_i(Σw)_i/(wᵀΣw)`, the fractional Euler contributions to portfolio volatility. An exact zero budget excludes the sleeve and gives it exactly zero weight, risk share and capital. Require positive final weights and risk shares for positive-budget sleeves. A single active sleeve gets weight and risk share one after full validation.
2. Require the caller's finite square covariance with both axes matching the nonempty ordered sleeve IDs exactly. Preserve return basis, currency treatment, horizon, cutoff and source provenance. Check dimensions, cell counts and workspace before quadratic allocation. Require numerical symmetry exactly, nonnegative diagonals, and zero couplings for each zero-variance sleeve. Check the entire positive-variance block on the unit-diagonal basis using the core PSD gate with zero pivot tolerance. Require the active principal block to pass strict positive-pivot Cholesky. This permits singular inactive blocks and removes support for singular active blocks, including the existing rank-one fixture. Use the supplied covariance without estimation, shrinkage or repair. [Accepted core matrix rules](../core-numerics/matrix-factorization.md#accepted-decision-checked-matrix-factors-solves-and-correlation-admission).
3. Use deterministic cyclic coordinate descent on `0.5 yᵀRy − Σ_i b_i log(y_i)` for positive `y`, with `σ_i=sqrt(Σ_ii)` and `R_ij=Σ_ij/(σ_i σ_j)` on the active block. Initialize `y_i=sqrt(b_i)` and sweep in sleeve order. For each coordinate solve the positive root of `y_i²+c_i y_i−b_i=0`, where `c_i=Σ_(j≠i) R_ij y_j`. Compute the off-diagonal dot with checked reductions. With `h=c_i/2` and `d=hypot(h,sqrt(b_i))`, evaluate `b_i/(d+h)` for `h>=0` and `d-h` for `h<0` using scale-aware arithmetic. Recover weights proportional to `y_i/σ_i` without unsafe intermediate products or reciprocals. Remove the fixed-point update and the absolute variance and trial-contribution floors. Negative intermediate contributions are valid solver states. [Coordinate descent and quadratic logarithmic formulation](https://arxiv.org/pdf/1311.4057). [Existence and uniqueness](https://arxiv.org/html/2211.07212).
4. Require a caller-supplied relative risk-share tolerance strictly between zero and one and a sweep budget from 1 through 10,000. Recompute risk from the effective weights and supplied covariance using checked scaled arithmetic. Require positive modeled variance on that basis, weight and risk-share sums within `8*EPSILON` of one, and `max_i abs(RC_i/b_i−1)` within tolerance over active sleeves. Permit a valid initial allocation to pass with zero sweeps. Distinguish invalid inputs, numerical range failures and exhaustion. Return no partial allocation on failure.
5. Pass the effective unrounded weights to [record 003](03-capital-apportionment.md). Signed and zero capital still require a valid solution. Report target and achieved shares, residuals, active status, covariance provenance, controls and actual sweeps in sleeve order. Use the same checked Rust contract for the existing Python and WASM surfaces. Require the source evidence needed to recheck weights and risk shares during restoration.

Independent arithmetic checks used equal budgets and covariance `[[1, -1.5], [-1.5, 4]]`. The current starting weights give `(-0.25, 1.25)` risk shares. Weights `(2/3, 1/3)` give the requested `(0.5, 0.5)` and remain the solution under a common `1e-20` covariance scale. An independent coordinate-descent check with unit variances and budgets `(0.05, 0.95)` met relative tolerance `1e-10` in 67 sweeps at correlation `-0.9` and exhausted 10,000 sweeps at `-0.9999`. A positive-definite input can exhaust the sweep budget.

## Observed choice and domain at the audit revision

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

Implementation and independent validation are tracked in the
[scope follow-ups](../FOLLOW_UPS.md#portfolio-attribution).
