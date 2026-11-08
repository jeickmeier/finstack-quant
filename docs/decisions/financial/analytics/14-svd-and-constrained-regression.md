# ANALYTICS-014: Normalized SVD least squares and single-equality factor-return correction

**Decision status:** Accepted for the complete normalized full-rank SVD and single-equality constrained-regression package below (2026-11-07).

**Implementation status:** Implemented in the analytics decision reconciliation dated 2026-11-08. The accepted sections below define the current contract.

**Validation status:** Scoped implementation checks and their limits are recorded in the [analytics inventory](README.md#implementation-evidence). A passing numerical fixture establishes its stated case; it does not certify a population estimator or every supported input.

**Audit provenance:** The original source audit was performed on 2026-11-05 at `7e82fd03ad98346727769e88dd5ac5d84b56e9fc`. Its historical discrepancies are superseded by the accepted contracts and this implementation. The [source ledger](coverage.json) retains source ownership and dated inspection fingerprints.

## Accepted decision: normalized SVD and constrained regression

**Accepted:** 2026-11-07.

This package covers the shared full-rank solve, exact numerical
rank policy, constrained objective, input/units, scaling, redundant/infeasible
cases, result validation and error contract. The [accepted record-13 package](13-multifactor-regression.md#accepted-decision-multifactor-regression)
remains authoritative for multifactor statistical interpretation and field
availability; this record does not change its estimator or introduce a
different fit when a requested model cannot be supported.

### Dimensions, statistical domain and output

- For the shared solve, require a finite real `n x p` design and one or more
  finite right-hand sides with exactly `n` rows, `n >= p >= 1`. For the public
  constrained function, require `n_assets = n >= 1`, `n_factors = p >= 1`,
  checked `n*p`, exactly that many row-major exposures, and exactly `n`
  returns and holdings. Reject invalid host integers, empty/mismatched shapes,
  nonfinite entries and unsupported rank; never shorten or fill arrays.
- Pair assets by position. Callers own asset identity/order, matching periods,
  currency, exposure units and the meaning of supplied holdings. These are
  finite statistical observations, without a wealth-return lower bound or
  terminal-history rule. The public constrained solve does not insert an
  intercept; callers include one only as an explicit independent column.
  Record 13 inserts its own intercept for its distinct time-series model.
- Permit signed, unnormalized and all-zero holdings. Holdings impose the one
  aggregate identity, not residual-loss weights. An asset with zero holdings
  still participates in the unweighted fit. Uniform nonzero holdings scaling,
  including a sign reversal, leaves the problem unchanged; dividing by a
  positive maximum absolute holding is numerical scaling, not economic
  normalization to a unit-sum portfolio.
- Return exactly `p` finite coefficients in factor-column order, using the
  existing Rust vector, Python list and WASM typed-array surfaces. Coefficient
  units are return units per exposure unit; dimensionless exposures yield
  observation-period return fractions. No cash, frequency, annualization,
  coefficient bound, NaN/infinity sentinel or new result carrier is needed.
  The caller/downstream attribution owns economic plausibility and closure
  under its separately declared reporting tolerances.

### Shared normalized SVD and numerical rank

Retain nalgebra SVD and robust column-L2 normalization, using bounded binary/
norm scaling and compensated sums so finite nonzero columns do not vanish or
overflow merely because their units are extreme. Reverse column scaling in
the returned coefficients. This changes numerical coordinates, not the
unweighted objective or factor definitions. Do not explicitly form normal
equations or add a new numerical library.

Validate every design/target entry explicitly; a maximum-value reduction is
not a finiteness check. Keep two-stage scaling when combining scale and norm
would round a subnormal norm inadequately, as well as when it would overflow.
Never normalize individual rows, which would change the residual-loss
weights and therefore the model.

Use the library's fallible bounded SVD with convergence tolerance
`5 * epsilon` and a maximum of 1,000 total internal iterations. Exhaustion
or decomposition failure is a calculation error, not a rank classification,
panic or an unbounded retry. These internal operating controls require
convergence/performance evidence for the intended sizes and may be revised without
changing the model or exposing public tuning. The [nalgebra SVD API](https://docs.rs/nalgebra/0.35.0/nalgebra/linalg/struct.SVD.html)
supports this bounded calculation.

Use the roundoff-based rank cutoff:

```text
epsilon = f64 machine epsilon
tau_rank = epsilon * max(n, p) * sigma_max
rank = count(sigma_j > tau_rank)  # on column-L2-normalized design
require rank == p
```

Require valid finite singular values and successful decomposition before rank
classification. Reject zero columns and numerical rank deficiency, including
underdetermined designs. Count only singular values strictly above the cutoff;
do not solve a rank-truncated model, drop columns, return minimum-norm
coefficients or introduce ridge regularization. The dimension/roundoff rule
follows the threshold documented by [NumPy's SVD rank routine](https://numpy.org/doc/stable/reference/generated/numpy.linalg.matrix_rank.html);
applying it to this normalized design and rejecting deficient fits remain
this API's choices, rather than NumPy result parity.

This internal gate detects numerical resolvability, not economically useful
factor independence, uncertain-data rank or an accuracy guarantee. A full-rank
but ill-conditioned design can have sensitive coefficients. No separate
financial multicollinearity cap or user-tunable tolerance is introduced.

Scale each right-hand side independently as needed, handling an all-zero
target exactly. Preserve representable requested coefficients when target
projections, scaled-coordinate coefficients or intermediate scaling products
would otherwise overflow/underflow. Retain a single reusable SVD for multiple
right-hand sides and constraint projection; do not require unrequested
auxiliary coefficients to be representable in original units.

### Constrained objective and correction

Retain precisely one equality-constrained unweighted problem:

```text
minimize ||r - X*f||_2^2   subject to   w' * X*f = w' * r

f_hat = argmin ||r - X*f||_2^2
g = argmin ||w - X*g||_2^2
P = orthogonal projector onto columns of X
y0 = X*f_hat = P*r
h = X*g = P*w
delta = w' * (r - y0)
D = w' * h = ||h||_2^2
f = f_hat + (delta / D) * g  # when correction is needed and h != 0
```

These identities describe the unique constrained minimizer for the supported
full-column-rank model; they do not require forming `P`, `X'X`, an inverse,
unscaled `g` or the multiplier literally. Use the existing SVD in scaled
projection coordinates when needed. The correction minimizes the original
residual loss under the equality; it is not necessarily the smallest
Euclidean change to the coefficient vector. No automatic weighted least
squares or alternative constraint is
substituted.

For exposures `[(0,1), (1,0), (0,1)]`, returns `[5%, 2%, 1%]` and holdings
`[0.6, 0.3, 0.1]`, unconstrained coefficients are `[2%, 3%]`. The constrained
coefficients are approximately `[2.89552239%, 4.04477612%]`, satisfying
`0.3*f_1 + 0.7*f_2 = 3.7%`. This is a local algebraic example, not a claimed
independent reproduction of a paper exhibit.

### Redundant constraints, projection resolution and zero cases

Use an internal weighted-gross relative equality budget `eta = 1e-12`:

```text
gross = sum(abs(w_i) * (abs(r_i) + abs(y_i)))
abs(w' * (r - y)) <= eta * gross
```

Evaluate it with separately scaled data and compensated products/sums;
do not permit an overflowing bound to make an invalid result pass. A zero
scale passes only for zero mismatch. Returns on zero-weight assets do not
loosen this identity check, while those assets still enter the unweighted
objective. The budget concerns gross weighted return amounts,
not exact algebraic equality, accuracy relative to a possibly cancelling net
return, or a portfolio basis-point/materiality threshold. If OLS already
passes, its constrained approximation needs no
correction, but must still pass final result checks.

For `X = [1e-14, 1]`, `w = [1, 0]` and `r = [2e-14, 1]`, the constrained
coefficient is `2`. An OLS coefficient near `1` does not reproduce the first
asset's weighted return; the unheld second asset must not inflate the
identity's tolerance enough to accept that mismatch.

| Constraint case after valid full-rank design | Accepted outcome |
| --- | --- |
| All holdings zero | Constraint is redundant; return the validated OLS fit. |
| All returns zero | Return zero coefficients after validating the model; no auxiliary `g` or multiplier may force a spurious overflow error. |
| Genuinely zero `P*w`, equality already satisfied within the declared precision | Redundant constraint; return validated OLS. |
| Genuinely zero `P*w`, incompatible nonzero target | Infeasible equality; explicit model error. |
| Resolved nonzero `P*w` | Apply the constrained correction when needed, preserving any representable supported result. |
| Projection/denominator remains numerically unresolved | Adequately rescale/recompute, then fail with a calculation error if unresolved; do not assert mathematical infeasibility. |

Remove the separate fixed `1e-10` projection/cosine rejection. A tiny nonzero
projection can define a feasible constraint, potentially with large but valid
coefficients. It is not proof of unreachability. Relative roundoff/cancellation
diagnostics may trigger adequate same-problem reconstruction, but do not
introduce another magnitude floor that automatically discards a resolved
direction. Evaluate `D` through stable projection geometry: it is nonnegative;
negative computed values or a nonzero direction whose squared norm collapses
must not be accepted using `abs(D)` or replaced by zero.

Full-rank model validation remains required in zero-return/zero-holdings
cases. For example, `X = [1e-320]`, `r = [0]`, `w = [1]` has requested
solution `[0]`; an unrepresentable conceptual `g = 1e320` is irrelevant to
that valid result. No coefficients are clipped solely for being very large,
negative or economically implausible.

### Final feasibility, optimality and failure checks

Check every successful path, including unchanged OLS and corrected output:

- Define `y = X*f` from the actual final original-unit coefficients after
  all rescaling and rounding. Final checks must evaluate that returned vector
  adequately; a satisfactory ideal projection before coefficient conversion
  is not a substitute. Apply the same rule to returned shared-OLS solutions.
- Require finite requested coefficients and evaluate fitted/projection data
  adequately in scaled coordinates. Avoidable nonfinite intermediates are
  not grounds to discard representable requested outputs.
- Re-evaluate the final weighted equality using the same `eta = 1e-12`
  weighted-gross test above. Do not return a corrected vector merely because the
  multiplier and coefficients are finite.
- Require the least-squares optimality condition as well as feasibility.
  For shared OLS, the projection of `r - y0` onto the design span must have
  norm at most `eta * (||r|| + ||y0||)`. For a nonredundant constrained fit,
  let `u = h/||h||` and `v = y - y0`; require
  `||v - u*(u'v)|| <= eta * (||r|| + ||y|| + ||y0||)`.
  For a redundant constraint, require `||y - y0||` within that same bound.
  These are scaled projection/KKT conditions for this original objective;
  they avoid requiring a representable multiplier or forming normal
  equations. A zero reference scale requires a zero check residual.
- If a check fails, attempt adequately scaled same-estimator reconstruction;
  persistent failure errors the call. No partial vector, zero substitution,
  automatic estimator change or repair of an invalid result is returned.

Invalid dimensions/nonfinite inputs and unsupported rank or genuinely
infeasible equality are request/model errors. Decomposition failure,
unresolved projection, unrepresentable requested outputs and failed final
checks are calculation errors. Preserve those distinctions through existing
host error mapping; success contains no undefined coefficients. There is no
new configuration, compatibility path or serialized status model.

The equality/optimality budgets and bounded convergence controls are explicit internal numerical acceptance
choices whose accuracy is limited to the stated numerical evidence. Passing them gives neither a
forward coefficient-error bound nor a guarantee of meaningful exposures in
ill-conditioned models. Relevant numerical boundary cases include rank boundaries, column/
target/holdings scaling, signed and zero holdings, exact/redundant/infeasible
constraints, resolved tiny projections, cancellation, auxiliary overflow,
zero targets, final feasibility/optimality and Rust/Python/WASM agreement.
Use independently derived constrained solutions or reference algorithms;
production-code self-comparison does not certify accuracy.

The [LAPACK equality-constrained least-squares discussion](https://netlib.org/lapack/lug/node85.html)
supports the objective and the importance of conditioning/error analysis,
without claiming that this implementation runs LAPACK or shares its solver.
The existing [Jeet–Partani reference](../../../REFERENCES.md#jeet-partani-2023)
remains cited attribution context; Appendix A and Exhibit 8 were not
authenticated in this review. Keep local derivations labelled as such.
Multiple equalities/inequalities, regularization, weighted loss and broader
factor-Brinson economic conventions remain separately scoped.

## Implementation evidence

[regression.rs](../../../../finstack-quant/analytics/src/regression.rs). The [crate integration tests](../../../../finstack-quant/analytics/tests/main.rs), inline numerical regressions and [schema contracts](../../../../finstack-quant/analytics/tests/schema_contract.rs) exercise this scope. See the [inventory validation record](README.md#implementation-evidence) for executed checks and evidence limits.
