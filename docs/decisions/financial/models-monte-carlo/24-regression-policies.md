# MODELS-MONTE-CARLO-24: LSMC bases, SVD cutoff and standardized polynomial reduction

- Decision status: **Accepted — full package on 2026-11-11**.
- Implementation status: Existing basis families, SVD and standardized policies observed. Checked fitting, replay and caller reconciliation are pending.
- Validation status: Source, selected tests, primary references and independent scaling, rank and polynomial examples reviewed. Runtime, host and financial qualification are pending.
- Audit date: 2026-11-05. Source revision: `7e82fd03ad98346727769e88dd5ac5d84b56e9fc`.
- Scope and coverage: [slice inventory](README.md), [file ledger](coverage.json).

## Accepted decision for checked continuation regression

1. **Use one fixed regression per fit.** Fit equally weighted
   training rows by ordinary least squares. The owner supplies a
   fixed ordered schema of decision-time features, exactly one
   intercept equal to one in the first column, and paired
   responses in one declared unit. Admit finite signed features
   and responses. Validate the full shapes and values, with
   `n >= p >= 1` for `n` rows and `p` columns. A checked predictor
   uses the same schema and response units. Keep group coverage
   and financial response construction in the owning policy.

2. **Select strike-centered powers for standalone holder exercise.**
   Require literal degree `d` from 0 through 4 and use
   `1,z,...,z^d` in ascending order with `z=(S-K)/K` and positive
   finite spot and contractual strike. Degree zero has one
   column. Compute the centered ratio and powers with checked
   arithmetic and remove the absolute strike floor. Remove the
   standalone family selector and the raw powers and unweighted
   Laguerre branches. At equal degree those three bases span
   the same exact polynomial space. Their finite-precision
   conditioning can differ. The exponential weight in the
   original Laguerre example changes the function space.
   [Accepted holder exercise](23-lsmc-exercise.md#accepted-decision-for-checked-holder-exercise-policy-training-and-pricing),
   [basis references](../../../REFERENCES.md#continuation-regression-bases-and-rank).

3. **Require an explicit multivariate feature set and degree.**
   An owner declares the active coordinates in schema order and
   chooses intercept only, linear, full quadratic or full cubic.
   For `m` active coordinates and positive degree `d`, include
   exactly the total-degree terms through `d`, with
   `binom(m+d,d)` columns. Order them as intercept, ascending
   linear coordinates, ascending squares, then pairs `i<j`
   lexicographically. Cubic appends ascending cubes, ordered
   `x_i^2*x_j` for `i!=j` in squared-coordinate then
   linear-coordinate order, and lexicographic triples `i<j<k`.
   Degree zero is an explicit intercept without active coordinates.
   Exclude structurally irrelevant or common coordinates through
   the owner's declared schema. No sampled feature dropping or
   candidate ladder.

4. **Freeze training-only sample standardization.** For each
   declared active coordinate in a standardized fit, use
   `(x-mean)/sample_sd` with equal row weights and variance
   denominator `n-1`. Retain shifted/scaled center and spread
   internally to preserve close differences and avoid restoring
   raw squared deviations or overflowing raw standard deviations.
   A zero spread in a declared active coordinate fails the fit.
   Remove the absolute/relative spread cutoff. Retain positive
   resolved spreads through design construction and rank admission.
   Apply the same transform and monomial ordering to pricing
   inputs. No recomputed moments, clipping or hidden extrapolation
   limit.
   [Accepted centered statistics](../core-numerics/sample-statistics.md#accepted-decision-checked-sums-moments-quantiles-and-mean-uncertainty).

5. **Scale the actual design and response deterministically.**
   Keep the intercept equal to one, divide each other column by
   its training maximum absolute value, and divide the response
   by its training maximum absolute value. A zero non-intercept
   column fails admission. An all-zero response stays zero and
   still requires an admitted design. A required nonzero dense
   fit entry rounded to zero by normalization is a range error.
   Keep scaled coefficients and all training scales with the
   predictor. For an explicitly
   selected intercept-only fit, use the checked training mean
   directly, preserving representable cancellation.
   [Accepted issuer fit](../models-analytic/hw1f-issuer-lsmc.md#accepted-decision-for-checked-issuer-call-policy-training-and-pricing).

6. **Use one bounded full-rank SVD.** For a design with `p>1`
   use the existing nalgebra SVD with both factors, binary64
   machine epsilon as decomposition convergence tolerance, and
   an explicit positive iteration limit. Require an explicit
   finite relative rank tolerance `0 < tau < 1`. Require finite
   factors and positive extreme singular values, with
   `sigma_min/sigma_max > tau` on the scaled design. Equality
   fails. Solve on every admitted singular direction. Remove the
   implicit dimension-based cutoff, truncated pseudoinverse,
   and automatic retries at smaller degree or fewer columns.
   Invalid data, convergence, rank and output failures are
   contextual errors. The rank gate gives no uniform error
   bound on coefficients or extrapolated predictions.
   [SVD references](../../../REFERENCES.md#continuation-regression-bases-and-rank).

7. **Bound work and report actual fit diagnostics.** Check
   dimensions, column counts, all shape and byte products and
   the owner route's declared workspace bound before allocation.
   Account for simultaneously live design and response arrays,
   SVD factors and scratch, and retained transforms. Use fallible
   allocation for controlled storage. Dependency-internal
   allocations are subject to their allocator behavior. Report
   requested degree or schema, actual row and column counts,
   rank threshold and extreme singular values or ratio when
   available. Return no partial policy or fabricated factorization
   diagnostics for an intercept mean.

8. **Publish an immutable checked predictor.** Bind the feature
   identities, order and units, training transforms, column and
   response scales, coefficients and diagnostic provenance.
   Validate exact replay shapes and finite features. Evaluate
   scaled monomials, dot products and the response-unit restoration
   in fixed order with checked scale-aware arithmetic. Preserve
   finite signed predictions, literal zero and representable
   cancellation. Propagate unresolved range and nonfinite errors.
   Do not infer structural constancy from equal observations or
   replace failed fits with realized future cash.

9. **Keep financial choices with the product.** Standalone
   holder dates below the accepted coverage gate are continue-only.
   Once admitted, their fit uses this strict contract. The
   accepted issuer route retains sparse-date errors, its own
   signed state schema, degree bound and structurally common
   intercept rule. Reconcile Hull–White and LMM swaptions to
   the checked solver with owner-selected features, eligibility,
   payoff units and numeraire. Reconcile bond continuation and
   conditional make-whole separately, including the make-whole
   coordinate set that currently carries a literal zero hazard.
   Each product owns its choice of live features, exercise
   behavior and benchmark.
   [Hull–White swaption](../valuations-rates/08-hull-white-swaptions.md),
   [LMM Bermudan](../valuations-rates/09-lmm-bermudans.md),
   [bond policy](../valuations-bonds/07-lsmc-policy.md).

10. **Reconcile the existing host inputs.** The accepted
    standalone Rust/Python/WASM pricing route receives the
    exact integer degree and checked fit controls. Remove the
    obsolete basis-family wire value and defaulted selector.
    Reconcile existing schemas, generated types, stubs, facade,
    parity, defaults and examples together. Keep solver, raw
    feature generation and fitted policy state Rust-owned.

11. **Qualify conditioning and financial fit separately.** Check
    shape, degree and resource rejection, unit rescaling, shifted
    moments, rank near the strict threshold, zero-response rank
    checks, bounded nonconvergence and frozen replay. Use
    independent polynomial and factorization examples for
    numerical correctness. Use held-out exercise and dated
    cash benchmarks for product adequacy. The owner qualifies
    degree, sample count and feature adequacy on its own model.
    [Basis-size and sample evidence](../../../REFERENCES.md#continuation-regression-bases-and-rank).

## Observed behavior and limitations

The [basis code](../../../../finstack-quant/models/src/monte_carlo/pricer/basis.rs)
exposes raw powers, strike-centered powers and unweighted Laguerre
in `S/K`. Laguerre admits degrees 1–4 and requires positive
strike. Raw and normalized powers require positive degree
without a common upper bound. Centered powers reject strike
at or below `1e-14` on the standalone route. The convenience
defaults to degree-three Laguerre. A rate swaption also uses
raw powers on its signed rate state.

The [SVD solver](../../../../finstack-quant/models/src/monte_carlo/pricer/lsq.rs)
passes arrays to nalgebra after checking `n >= p`. Shape
mismatches can panic. It selects an unbounded decomposition and
the cutoff `sigma_max * EPSILON * max(n,p)`. Its singular-design
test expects finite coefficients after truncation.

The [standardized policy](../../../../finstack-quant/models/src/monte_carlo/pricer/polynomial.rs)
uses raw sums and squared deviations. It drops features at sample
SD at or below `1e-12 * max(abs(mean),1)` and tries a candidate
ladder from cubic through full quadratic, additive quadratic,
linear and intercept as applicable. A truncated SVD can accept a
rank-deficient candidate before the ladder reduces it. The
[bond owner](../../../../finstack-quant/valuations/fixed-income/src/bond/pricing/tree/lsmc/policy.rs)
uses the shared quadratic fitter for make-whole and ordinary
continuation. The [Python](../../../../finstack-quant-py/src/bindings/models/monte_carlo/pricers.rs)
and [WASM](../../../../finstack-quant-wasm/src/api/models/monte_carlo.rs)
standalone routes accept the current basis selector.

## Independent review evidence

| Example | Expected outcome under the proposed contract |
| --- | --- |
| `K=100`, spots `50,100,150`, degree 2 | The normalized coefficients `(2,3,4)`, raw coefficients `(3,-0.05,0.0004)` and unweighted Laguerre coefficients `(6,-11,8)` all give `(1.5,2,4.5)` in exact arithmetic. |
| `K=100`, train spots `50,150` and responses `1,5`, degree 1 | Feature and response scales are `0.5,5`. Scaled coefficients are `0.6,0.4`; frozen replay at spot `125` yields `4`. |
| Feature samples `[1e16,1e16+2]`, `[1e-200,2e-200]` and `[-1.7e308,1.7e308]` | Each pair gives standardized coordinates `(-1/sqrt(2),1/sqrt(2))`. Rounded raw centering or unscaled squaring can lose the result. |
| Max-scaled design rows `[1,1]` and `[1,1-2^-30]` | The singular-value ratio is about `2.32830643762e-10`. Threshold `2^-31` rejects and `2^-33` admits. |
| Ten rows alternating `x=-1,+1` with quadratic `[1,x,x^2]` and zero response | Rank is two. The intercept and square coincide, so even with enough rows and a zero response the fit fails. |
| Full quadratic on two independent coordinates, with grid `{-2,-1,0,1,2}^2` | Rank is six. A manufactured target `4+2x-3y+x^2/2+5xy/4` yields `6.495` at `(0.7,-0.4)`. If `y=2x` over six distinct values of `x`, rank is three and the fit fails. |

Selected source tests cover manufactured polynomial recovery,
singular designs and automatic reduction. The examples above
check the proposed numerical contract. Runtime, host and
financial qualification remain pending.

## Follow-up tracking

Adoption questions, implementation reconciliation and validation work are tracked in the
[scope follow-ups](../FOLLOW_UPS.md#monte-carlo-models).
