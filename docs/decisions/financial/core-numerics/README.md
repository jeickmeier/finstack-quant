# Core numerics methodology audit

This slice covers **65 assigned Rust source files** in core numerics,
interpolation, expression evaluation, validation, configuration, wire encoding,
canonical identity and schema tooling: **58 algorithm-bearing, 5 support and 2
dedicated test files**. The exact path ledger is [coverage.json](coverage.json).
All runtime implementations in that ledger were inspected, including runtime
items following test declarations/helpers in schema and wire modules. Inline tests were inspected selectively for the evidence roles stated
in each record; dedicated tests are not counted as production algorithms.

Audit date: **2026-11-05**. Source revision:
`7e82fd03ad98346727769e88dd5ac5d84b56e9fc`. Methodology adoption is stated below; this documentation review changes
no runtime algorithms, defaults, tests or fixtures.

Of the **26 records**, [01 — Compounding](compounding.md#accepted-decision-explicit-compounding-conversion-and-periodic-spreads),
[02 — Value interpolation](value-interpolation.md#accepted-decision-validated-scalar-interpolation-and-boundary-semantics),
[03 — Forward interpolation](forward-interpolation.md#accepted-decision-natural-log-discount-and-hagan-west-interpolation),
[04 — Quadrature](quadrature.md#accepted-decision-checked-quadrature-budgets-and-measures),
[05 — Brent](brent-root.md#accepted-decision-checked-brent-solving-and-bounded-search)
and [06 — Levenberg–Marquardt](levenberg-marquardt.md#accepted-decision-checked-damped-least-squares-and-outcomes)
are **Accepted on 2026-11-07**. [07 — Matrix factorization](matrix-factorization.md#accepted-decision-checked-matrix-factors-solves-and-correlation-admission),
[08 — Nearest correlation](nearest-correlation.md#accepted-decision-explicit-nearest-correlation-projection-and-repair-limits),
[09 — Sample statistics](sample-statistics.md#accepted-decision-checked-sums-moments-quantiles-and-mean-uncertainty),
[10 — Realized variance](realized-variance.md#accepted-decision-explicit-log-returns-and-realized-variance),
[11 — Covariance shrinkage](covariance-shrinkage.md#accepted-decision-checked-scaled-identity-covariance-shrinkage),
[12 — Distribution functions](distribution-functions.md#accepted-decision-checked-special-functions-and-distribution-domains),
[13 — Binomial mass](binomial-mass.md#accepted-decision-checked-full-binomial-probability-mass),
[14 — Beta/Gamma sampling](beta-gamma-sampling.md#accepted-decision-checked-beta-and-gamma-sampling),
[15 — Chi-squared inversion](chi-squared-inversion.md#accepted-decision-checked-chi-squared-log-tail-inversion),
[16 — Correlated Bernoulli](correlated-bernoulli.md#accepted-decision-strict-correlated-bernoulli-laws),
[17 — RNG/bridge/Poisson](pseudorandom-and-bridge.md#accepted-decision-checked-random-draws-bridge-and-poisson-counts),
[18 — Sobol](sobol-sequences.md#accepted-decision-finite-period-sobol-points-and-prefix-hash-scrambling),
[19 — Fractional covariance](fractional-covariance.md#accepted-decision-checked-fractional-increment-covariance),
[20 — Time grids](time-grids.md#accepted-decision-checked-simulation-grids-and-exact-event-knots),
[21 — Piecewise coefficients](piecewise-coefficients.md#accepted-decision-checked-piecewise-coefficients-and-exponential-integration),
[22 — Scalar expressions](expression-evaluation.md#accepted-decision-checked-scalar-expressions-missing-values-and-decimal-quantization),
[23 — Configuration and metadata](rounding-and-config.md#accepted-decision-explicit-rounding-config-local-tolerances-and-producer-metadata),
[24 — Numeric wire and canonical identity](numeric-wire-and-canonical.md#accepted-decision-strict-numeric-wire-and-canonical-content-identity),
[25 — Contracts, validation and diagnostics](contracts-and-diagnostics.md#accepted-decision-bounded-contract-loading-validation-and-diagnostics),
and [26 — Schema publication and compact views](schema-publication.md#accepted-decision-canonical-schemas-compact-views-and-bounded-publication)
are **Accepted on 2026-11-08**. **All 26 core-numerics records are accepted;
none remain Draft or Proposed.** Implementation status is **Observed**
within each listed path scope. Production validation remains **Evidence
inspected/located, not executed**: no numerical suites or builds ran. Independent
documentation/reference examples are reported separately in the owning record.
Formula fixtures, dependency comparisons, roundtrips, self-consistency checks and
statistical sanity tests are distinguished. Shared numerical functions do not
select a product model or establish market fitness. The algorithm inventory
below is more granular than the file ledger: files can own several independent
choices, and one coherent choice can span several files. Inventory rows retain
observed source behavior; accepted contracts do not imply runtime reconciliation.

Accepted record 01 requires explicit compounding and exact integer frequency,
fallible nonnegative-time conversions, unidentified zero-time inverse errors and
explicit known-rate limiting conversion. It selects stable direct quote conversion,
checked target domains/ranges, instantaneous spread semantics and strict periodic
z-spread identities without frequency repair or implicit host continuous defaults.
Its formula/precision limitations and actual consumer reconciliation remain
separate from product-owned quote/clock/spread conventions. The package is
accepted; runtime/host reconciliation and independent qualification remain pending.

Accepted record 02 retains Linear/LogLinear/the specified PCHIP recipe behind one
validated immutable public interpolator, removes raw/helper bypasses and numeric
spacing/default/positivity-policy assumptions, and selects explicit Flat/Extend/Error
boundaries, exact stored-knot identities, derivative sides and fallible queries.
Its strict canonical state/restore and actual MVA/ECL/curve/host reconciliation
preserve explicit owner-supported constants and separately owned forward/financial
economics. The complete package is accepted; runtime and independent
numerical/consumer qualification remain pending.

Accepted record 03 retains the fixed natural log-discount spline and unsmoothed
Hagan-West regional method with its optional positivity collar omitted. It selects
scaled natural solving without an absolute pivot cutoff, exact regional/axis rules
without a fixed epsilon, actual one-sided derivatives and source-anchored Flat/Extend/
Error tails. Positive DFs do not certify nonnegative/continuous forwards. Accepted record 02
state/error contracts apply; financial origin/clock/curve/calibration and vendor
qualification remain separately owned. The complete package is accepted;
runtime/host/consumer and independent numerical qualification remain pending.

Accepted record 04 covers every fixed/adaptive/scalar/pair/Gaussian/grid API,
retaining explicit rules/orders while making callbacks/results fallible and
adding explicit adaptive evaluation budgets. It selects oriented finite integrals,
validated zero-width identities, conserving component-wise absolute estimator
budgets, stable checked sampling/weighted sums and strict reusable grids. Estimated
accuracy remains distinct from true error and product qualification. The complete
package is accepted; table provenance, runtime/caller reconciliation and
independent numerical/financial qualification remain pending.

Accepted record 05 retains one safeguarded Brent kernel with canonical fallible
callbacks, immutable explicit controls and separately explicit bounded-search
inputs. It selects full finite admission before callbacks, immediate invalid-sample
failure, deterministic bounded expansion, stable bracket/proposal arithmetic,
truthful work diagnostics and exact-computed-zero/coordinate-width termination.
Financial domains/continuity/root selection and residual acceptance stay with
callers; accepted record 017 remains unchanged. The complete package is accepted;
runtime/host/caller reconciliation and independent qualification remain pending.

Accepted record 06 selects one augmented column-pivoted QR LM step using the
existing nalgebra dependency, with the existing norm-defined damping metric and
explicit disclosed damping heuristic. It requires private validated residual/step
tolerances and attempt budget, canonical fallible fixed-shape residual/Jacobian
callbacks, explicitly selected checked three-point finite differences, stable norms
and checked scaling/resources. Inspectable residual-target, small-step, damping-limit
and budget outcomes remain separate from caller acceptance, with actual attempts,
accepted steps and matching diagnostics. The complete package is accepted;
runtime/caller/host reconciliation and independent qualification remain pending.

Accepted record 07 selects exact symmetric full-matrix admission, ordinary
positive-pivot SPD factorization/nonzero-diagonal solves without arbitrary
singularity floors, and explicitly tolerance-truncated generic cholesky_psd/
PsdFactor with restored units and original variable/innovation order. It requires
strict separate correlation structure, checked factor application/arithmetic,
bounded explicitly controlled sorted-pair eigen solving, truthful errors and
coordinated existing host/caller reconciliation. Approximate PSD admission is
not exact PSD/rank/reconstruction or financial-risk certification. The complete
package is accepted; runtime/host/consumer and independent numerical
qualification remain pending. Repair and shrinkage stay with records 08 and 11.

Accepted record 08 selects one explicit unweighted Frobenius Higham/Dykstra
projection for finite square inputs, with stable symmetric/unit-diagonal
initialization, six validated explicit controls including an original-input
maximum repair distance, three relative stopping quantities and accepted 07
structure/PSD admission. It removes the approximate-factor early return and
extra final PSD/rescaling/floor, returning only the admitted stopped iterate.
Finite feasibility may require extra budget after stopping metrics pass; finite
results are not nearest-objective/PSD certificates. Existing hosts/callers and
financial repair permission/provenance require reconciliation. The complete
package is accepted and production qualification remains pending.

Accepted record 09 selects checked source-order Kahan/Neumaier sums and canonical
Result/Option statistical availability without zero/NaN duplicate families. It
requires private shifted/scaled centered online state, atomic update/merge with
exact-count admission, requested-quantity evaluation, strict normalized Pearson
and identified control beta, checked R7/sorted/mutable quantiles and explicit
finite-only filters. Student-t mean uncertainty has explicit alpha and direct
scaled SE/half-width, with reporting/inference and Monte Carlo policies separated.
The complete package is accepted; runtime/host/caller and independent
numerical/statistical qualification remain pending. Record 10 retains price/log
return and realized-variance decisions; record 12 retains distribution primitives.

Accepted record 10 selects checked positive-price log returns and five explicit
annualized squared-log estimators, with no method/frequency defaults. One OHLC
request uses all target bars and an explicit preceding close for CloseToClose/
YangZhang (minima one/two bars), while standalone close levels cover N-1 intervals.
It specifies practical GK/YZ variants, nonnegative range algebra, estimator-specific
zeros, Result/Option availability and stable complete-result annualization without
negative floors. Canonical host/caller/schema and product-window/default boundaries
remain explicit. The complete package is accepted; production numerical,
host and financial qualification remain pending.

Accepted record 11 fixes centered population-normalized Ledoit–Wolf shrinkage
toward mu*I, the complete Frobenius intensity and genuine tie/cap conventions.
It requires private shifted/scaled centering, mixed-unit moment preservation,
scaled norm/complement evaluation, complete checked output and typed errors,
without a ridge, repair or conditioning guarantee. Existing host pairs and credit
annualization/correlation/error/provenance need coordinated reconciliation;
statistical model suitability stays with owners. The complete package is accepted;
exact-rational examples are independent reference evidence, not
production numerical or financial qualification.

Accepted record 12 selects one checked scalar Result surface for all ten special/
distribution functions, strict inputs with legitimate extended-real limits,
positive-real log-Gamma, safe normal standardization/final density, the fixed
log-Gaussian-tail profile and same-function unit-scale Student-t. It requires a
narrow private checked beta adaptation with140 continued-fraction and1024 inverse
work limits, stable geometry/reconstruction, truthful errors and coordinated
existing host/CI/copula/log-reflection reconciliation. No new public controls,
normal cutoff or financial probability policy is selected. The complete package
is accepted; bounded high-precision references are not production
qualification.

Accepted record 13 selects strict finite probability inputs, full n+1 binomial
count output and a fixed inclusive one-million-trial dense resource limit. It
replaces forward logs/conditional correction with exact represented-p mode
selection, fused outward relative weights, once-only compensated normalization
and an explicit8*EPS final-mass admission gate. Scratch preflight preservation,
computed-error discard, Rust-only exposure and actual conditional-credit
ownership remain explicit. Normalized mass does not certify tail accuracy or
payoff-weighted loss. The complete package is accepted; exact-rational
examples are reference evidence, not production qualification.

Accepted record 14 retains the Marsaglia–Tsang/Gamma-ratio beta law with strict
shape/RNG admission, the exact uniform shortcut and a declared no-redraw uniform0
profile. It replaces floors/raw sums/Bernoulli fallback with bounded stable MT
acceptance, deferred sampled log-Gamma components and smaller-tail logistic
restoration. Per-Gamma100,000 proposals, draw order/state advancement, truthful
failures and Rust-only/BetaRecovery/general-RNG ownership remain explicit. The
complete package is accepted; bounded arithmetic references and primary
method provenance do not certify the production distribution or recovery model.

Accepted record 15 selects strict closed probability endpoints with validated
positive df and a fixed1e8 interior numerical ceiling. Interior results must be
positive finite and pass a1e-8 log-tail gate recomputed at the actual returned q.
It selects bounded private logarithmic Gamma regions, preserved small-shape
normalization/stable large-shape prefactors, the finite eta/U bracket and explicit
shared-Brent256/1e-13 controls, removing residual/mass clips, hidden dependency
failures and interior-zero/nearest-rounding claims. Rust-only scope and actual
Student-t mixing ownership remain explicit. The complete package is accepted;
bounded references and computational admission are not true-error/financial
certificates.

Accepted record 16 selects one immutable strict two-event law with required
nullable correlation: Some only for interior marginals, None only for exact
deterministic marginals. It removes projection/variance floors and requested/
effective duplicates, preserves scaled feasibility/cell/query information,
selects genuine conditional availability and latent half-open sampling, and
reconciles short canonical get_*/sample names and existing nullable host shapes.
Rounded bounds/cells and numerical gates are not exact reconstruction or
multivariate/financial certificates. The complete package is accepted;
runtime/host and independent qualification remain pending.

Accepted record17 selects one explicit PCG seed/stream mapping and53-bit uniform
grid, checked reflected Box–Muller and fallible general normals with explicit
cache/state semantics. It retains checked capped bridge topology, whole finite
grid/shock admission, pinned DFS order and stable conditional arithmetic, with
rounded-level limits. Poisson uses one u64/safe-count, shared-log-Gamma integer
search with107-probe/adjacent-inequality admission. Full caller propagation,
Rust-only scope and financial/QMC/TimeGrid ownership remain explicit. The complete
package is accepted; primary/pinned sources and bounded references do not
certify streams, numerical range, convergence or financial fitness.

Accepted record 18 retains fixed 40-dimensional/32-bit D6 directions, natural
binary order and the exact interior midpoint grid, with explicit None/Some seeds
and an honestly named prefix hash permutation. It selects checked whole-point
fills, hard period exhaustion, atomic generator-state commits and removal of the
incompatible scalar model adapter. Prefix bijection preserves applicable dyadic
net properties but does not establish canonical Owen randomization or uncertainty.
Recorded table/golden provenance and bounded arithmetic references are now linked;
runtime/caller and independent statistical/financial qualification remain open.
The complete package is accepted; runtime/caller reconciliation and independent
qualification remain pending.

Accepted record 19 selects unit-normalized stationary fBM increments, typed
Hurst values and checked scalar/matrix results on explicit nonnegative forward
intervals/strict grids. It removes level-covariance subtraction, uses direct
Brownian/self and stable touching identities, and selects one bounded scaled/
compensated general profile with an explicit cancellation-refusal gate. Exact
mirroring and positive diagonal do not certify rounded PSD; no repair is added.
Actual fBM caller/factor/Schur-error chains, embedded Hurst schema and separate
model clocks/normalization/defaults remain explicit. The complete package is
accepted; runtime and independent qualification remain pending.

Accepted record 20 selects one immutable zero-origin knot invariant, actual
adjacent dts, a pinned explicit-count uniform profile and validated zero-step
identity. Required events use exact-coordinate union without floor, snapping or
drops; exact lookup and checked get_* access replace epsilon searches/panics.
Canonical uniform-layout recognition does not certify equal rounded dts or a
uniform-only model. Generic date/exercise snapping helpers are removed; actual
model/product clocks, timing displacement, event multiplicity and numerical
eligibility stay with owners. Core remains Rust-only with existing model wire
shapes. The complete package is accepted; runtime/caller/host reconciliation and
independent numerical/financial qualification remain pending.

Accepted record 21 retains right-continuous nonnegative step coefficients and
validated immutable state, with checked short get_value/get_times/get_values and
integrate APIs. It removes duration/kappa deadbands and evaluates the signed-
kappa analytic variance kernel with a stable maximum-weight endpoint identity,
deferred component scales and a two-pass compensated positive total. Genuine
zero and final range failures are explicit. Reconcile actual OU/Hull–White/tree/
stepper/host failure chains without NaN-to-zero substitution, while preserving
model/wire shape and separate clocks/calibration/parameter/price ownership. The
complete package is accepted; runtime and independent numerical/financial
qualification remain pending.

Accepted record 22 retains a bounded immutable eager scalar DAG, with complete
static arities/controls, explicit row count and strict exact column admission.
Runtime NaN alone is missing; infinities and invalid finite calculations error,
comparisons propagate missing and logic uses explicit three-valued tables.
Eager unselected numeric errors still fail the call. It selects exact binary-input
decimal quantization through bounded existing rational primitives, checked clamp,
atomic resource-aware results and complete composite/shared-helper reconciliation.
Narrow statements round/clamp admission is explicit; other language, host/shared-
AST and financial rounding policies remain owner boundaries. The complete package
is accepted; runtime and independent qualification remain pending.

Accepted record 23 inherits accepted monetary rounding and selects coherent
optional-mode configuration/version-2 snapshots, retiring shared Money/generic
zero classifiers and universal epsilon authority with explicit owner/protected-
tolerance reconciliation. It keeps opaque domain-owned extensions, simplifies
embedded registries to fixed builders/load-only caches, and removes the obsolete
generic override path. Applied rounding and execution metadata become optional
producer facts; fresh stamps no longer imply policy use or serial execution,
and request overlays are removed. Existing hosts/wire/registry/cash/attribution/
pricing consumers require coordinated reconciliation. The complete package is
accepted; runtime and independent qualification remain pending.

Accepted record 24 retains finite bounded wire domains and makes annotated
count read/write ranges symmetric, Decimal input exact, date strings strict and
sentinels field-selected/exact, closing direct finite-arm bypasses. It keeps
independent version axes and serialized identity, selects explicit recursive
UTF-8 key ordering, the frozen binary64 profile, duplicate/float-key/private-
pseudo-struct rejection, a fixed 96-container writer ceiling and single-pass
c2 SHA-256 identity. Existing generic ingress must reject lost integer/duplicate
information before Value; actual host signed-zero/numeric-kind and composite
cache payload consumers require reconciliation. No JCS/economic-equivalence,
authentication or replay certification is implied. The complete package is
accepted; runtime and independent writer/host/financial qualification are pending.

Accepted record 25 selects one lossless bounded loading path followed by owner
validation, full-input depth/cardinality enforcement, exact scoped-v1 markers
and strict shape boundaries. It separates report fatality from retained detail,
including zero-cap/merge behavior; keeps exact Money thresholds and owner recovery
rules; validates registry/table state and preserves nullable/nonfinite/exact-i64
exports. It selects deterministic bounded hints/causes, finite fixed-cap traces,
qualified ordered execution and fail-closed poisoned locks. Actual loaders,
calibration reports, Arrow/host exports, trace/lock consumers and schemas require
coordinated reconciliation. The complete package is accepted; runtime and
independent numerical/financial/state/host qualification remain pending.

Accepted record 26 retains original local schema graphs in one closed offline
corpus, with explicit2020-12 Deserialize settings and independent schema/runtime
admission. It removes generic externalization/pruning/union collapse and example
rounding, selects unambiguous registry identity and exact qualified examples,
and makes deterministic publication ownership/staging/index-last limits explicit.
The lossy llm profile becomes a labeled compact description-only view; original
canonical validation uses bounded accepted025 reports without lifted-branch
heuristics. Actual domain registries, generators, hosts, declarations/parity and
approved artifacts require coordinated reconciliation. The complete package is
accepted, completing this area's methodology review; runtime and independent
schema/host/filesystem/financial qualification remain pending.

## Algorithm inventory

| Calculation / supported instruments | Implemented method / material variants | Actual selection | Decision record | Implementation / validation |
| --- | --- | --- | --- | --- |
| Scalar rate ↔ DF conversion | Continuous, periodic and simple compounding; stable log1p/expm1 | Explicit variant; continuous enum default | [01 — Compounding](compounding.md) | `math/compounding.rs`; known values and inverse/zero-tenor cases |
| Additive periodic z-spread discounting | Base DF → periodic yield → add spread → DF | Dedicated wrapper always periodic; floating frequency normalized to integer | [01](compounding.md) | Same owner; denominator/zero-tenor evidence |
| Scalar interpolation and boundary policy | Linear, log-linear, PCHIP; value-flat/derivative extension/NaN | Runtime enum or generic strategy; defaults Linear/Strict/FlatZero | [02 — Value interpolation](value-interpolation.md) | `interp/{generic,types,strategies,utils}.rs`; knot/derivative/serde cases |
| PCHIP derivative construction | Harmonic interior slopes, sign/endpoint limiters | CubicHermite strategy | [02](value-interpolation.md) | Same owner; shape and exact-knot evidence |
| Natural log-discount spline | Tridiagonal natural cubic in −log DF; quadratic forwards | PiecewiseQuadraticForward | [03 — Forward interpolation](forward-interpolation.md) | `interp/strategies.rs`; hand-derived 3-knot polynomial |
| Hagan–West forwards | Discrete forward estimation, local positivity projection, four integrated shape branches | MonotoneConvex | [03](forward-interpolation.md) | Same owner; four-region rational fixtures |
| Finite-interval quadrature | Composite Simpson; fixed Gauss–Legendre 2/4/8/16 | Explicit API/order/panels | [04 — Quadrature](quadrature.md) | `math/integration.rs`; analytic polynomials/sine |
| Adaptive finite-interval integration | Adaptive Simpson or one-versus-two-panel Legendre; aggregate error budget | Explicit tol/depth | [04](quadrature.md) | Same owner; budget/depth failure cases |
| Paired adaptive Simpson | Shared nodes; component-wise uncorrected Simpson/error totals | Existing single absolute tol applied to each component; explicit depth | [04](quadrature.md) | `adaptive_simpson_pair`; shared-mesh/global-estimator evidence located, not executed |
| Standard-normal expectations | Gauss–Hermite 5/7/10/15/20 with density scaling | Explicit order | [04](quadrature.md) | Same owner; analytic normal moments |
| Scalar inverse solve | Bound expansion then Brent interpolation/bisection | Default x tolerance 1e-12, 100 loops; explicit bracket alternative | [05 — Brent](brent-root.md) | `math/solver.rs`; known roots, failure and one-sided bracket cases |
| Nonlinear least-squares fit | LM damped normal equations; analytic or central-difference Jacobian | Explicit solver, default residual tolerance 1e-8, 100 loops | [06 — LM](levenberg-marquardt.md) | `math/solver_multi.rs`; linear fits/scale and status cases |
| Positive-definite solves | Scaled ordinary Cholesky and forward/back substitution | Explicit factor/solve calls | [07 — Factors](matrix-factorization.md) | `math/linalg/mod.rs`; reconstruction/known solves |
| Semidefinite correlation factors | Pivoted residual factorization with rank truncation; original-order factor application | Explicit correlation factor API | [07](matrix-factorization.md) | Same owner; rank deficiency/scale/permutation cases |
| Correlation validation and eigensystems | Unit/symmetry/bounds/PSD validation; nalgebra symmetric eigen | Explicit calls | [07](matrix-factorization.md) | Same owner; eigen identities/correlation boundaries |
| Correlation repair | Higham/Dykstra-style projection, early valid-matrix return, final rescaling | Explicit repair only, 200 iterations / 1e-10 default | [08 — Nearest correlation](nearest-correlation.md) | `linalg/nearest_correlation.rs`; analytic equicorrelation optimum |
| Scalar summation | Kahan and Neumaier compensation | Explicit helpers; callers choose | [09 — Sample statistics](sample-statistics.md) | `math/summation.rs`; cancellation fixtures |
| Sample/population moments and controls | Welford means/M2/cross moments, merge formulas, covariance/correlation/control beta | Explicit batch/online APIs | [09](sample-statistics.md) | `math/stats.rs`; known moments/merge/constant-series checks |
| Empirical quantiles and finite helpers | R7 linear order-statistic interpolation; sorted fast path; finite extrema/count | Explicit API | [09](sample-statistics.md) | Same owner; hand R7 examples and NaN cases |
| Sampling uncertainty | Student-t mean confidence intervals using n−1 df | Alpha explicit; half-width helper fixes 95% | [09](sample-statistics.md) | Same owner; Cauchy special case and tabulated critical value |
| Close-to-close realized variance | Annualized mean squared log returns, no demeaning | Dedicated function; enum default CloseToClose | [10 — Realized variance](realized-variance.md) | `math/stats.rs`; invalid/default cases |
| Range-based realized variance | Parkinson, Garman–Klass, Rogers–Satchell, Yang–Zhang | Explicit method; OHLC omitted method defaults Yang–Zhang | [10](realized-variance.md) | Same owner; formula-reconstruction per-bar cases |
| Covariance estimation | Ledoit–Wolf population-normalized S toward scaled identity with analytic shrinkage | Explicit API, common-scale moments | [11 — Shrinkage](covariance-shrinkage.md) | `math/linalg/mod.rs`; hand 17/18 shrinkage fixture and rescaling |
| Normal/Student-t CDF/PDF/inverse | statrs wrappers; inverse saturation; direct normal PDF | Explicit scalar APIs | [12 — Distribution functions](distribution-functions.md) | `math/special_functions.rs`; tabulations/roundtrips/domain checks |
| Log normal tail | Ordinary/log1p branches plus asymptotic Mills-series tail at x≤−10 | Internal branch | [12](distribution-functions.md) | Same owner; high-precision-labelled values at −10/−40/−100 |
| Homogeneous binomial count distribution | Log PMF recurrence plus mass normalization | Explicit n,p; p outside [0,1] saturates except NaN | [13 — Binomial mass](binomial-mass.md) | `math/distributions.rs`; small formulas and statrs comparison |
| Beta/Gamma draws | Marsaglia–Tsang rejection; shape augmentation; Gamma ratio; endpoint fallback | Explicit RNG/shapes; 100,000 rejection cap | [14 — Beta/Gamma](beta-gamma-sampling.md) | Same owner; moments/small-shape/cap cases |
| Chi-squared inverse CDF | Log-tail/log-x Brent inversion with representability and residual checks | Explicit p,df; internal tol 1e-13 / 256 iterations | [15 — Chi-squared](chi-squared-inversion.md) | Same owner; df=2 closed form, df=4 expansion, transformed quadrature |
| Two-event probability law | Fréchet-bounded Pearson projection; independent near-degenerate law | Constructor always projects infeasible correlation | [16 — Bernoulli](correlated-bernoulli.md) | `math/probability.rs`; marginals/bounds/outcome partitions |
| Pseudorandom normals | PCG64 seed/stream; cached Box–Muller pair | Explicit RNG constructor | [17 — Random transforms](pseudorandom-and-bridge.md) | `math/random.rs`; repeatability/moment cases |
| Brownian path coordinate transform | Terminal-first recursive conditional bridge | Explicit bridge and grid | [17](pseudorandom-and-bridge.md) | `random/brownian_bridge.rs`; conditional/reference-path cases |
| Poisson count mapping | Low-lambda recurrence; CDF/survival integer bracket and bisection | Explicit lambda,u; branch at lambda=30 / count 200 | [17](pseudorandom-and-bridge.md) | `random/poisson.rs`; count/quantile cases |
| Low-discrepancy point generation | 40-dimensional 32-bit Sobol; optional prefix hash scrambling; midpoint normal inverse | Seed 0 unscrambled; finite period 2^32 | [18 — Sobol](sobol-sequences.md) | `random/sobol.rs`; state/direct-definition, wrap/skip/seed checks |
| Fractional increment covariance | Four-corner fractional Brownian covariance | Explicit raw h/grid; separate validated Hurst wrapper | [19 — Fractional covariance](fractional-covariance.md) | `math/fractional.rs`; Brownian limit/symmetry |
| Simulation time grids | Uniform/custom grids; required-time merge and absolute spacing policy | Explicit constructor and grid density | [20 — Time grids](time-grids.md) | `math/time_grid.rs`; endpoints/merge/validation |
| Event timing on uniform grids | Nearest-step rounding, out-of-domain filtering or date-clamping | Separate time/date helpers | [20](time-grids.md) | Same owner; date/time mapping cases |
| Piecewise variance coefficient integration | Right-continuous nonnegative steps and analytic exp-weighted square integral | Explicit coefficient curve/kappa/anchor | [21 — Piecewise coefficients](piecewise-coefficients.md) | `math/piecewise.rs`; knot-crossing/near-zero-kappa formulas |
| Scalar expression compilation | Iterative depth≤511 postorder DAG and structural deduplication | `CompiledExpr::new`, scalar functions only | [22 — Expressions](expression-evaluation.md) | `expr/{ast,context,dag}.rs`; shared-node/rejection cases |
| Scalar expression evaluation | Arena elementwise arithmetic/functions, NaN padding, exact logic, half-away round | Explicit scalar evaluator; first column defines length | [22](expression-evaluation.md) | `expr/{eval,eval_functions}.rs`, `math/mod.rs`; operator/function cases |
| Effective rounding/zero configuration | Bankers default, per-currency scales, absolute money/generic tolerance | Explicit config/overrides; default generic 1e-10 | [23 — Configuration](rounding-and-config.md) | `config.rs`, `math/units.rs`; defaults/range checks |
| Registry and result provenance | Cached embedded document versus whole-document override; metadata ownership merge | Matching config extension; otherwise embedded | [23](rounding-and-config.md) | `embedded_registry.rs`, `config.rs`; override/stamp cases |
| Numeric wire formats | Bounded finite f64, Decimal strings, explicit nonfinite sentinels, 32-bit output counts | Wrapper/annotation-selected per field | [24 — Wire/canonical](numeric-wire-and-canonical.md) | `wire/*`; schema and roundtrip/rejection cases |
| Content identity | Ordered compact JSON and domain/version-separated SHA-256 | Explicit canonical APIs | [24](numeric-wire-and-canonical.md) | `canonical*`; byte/hash fixtures |
| Contract parse and failure policy | Byte/depth/version/structure stages, bounded reports and explicit domain helpers | Explicit loader/limits/validators | [25 — Contracts](contracts-and-diagnostics.md) | `contract/*`, `validation.rs`, `serde_guard.rs`; ordering/cap checks |
| Table/trace/error semantics | Constructor row/name checks; capped traces; stable error kinds; Levenshtein hints | Explicit APIs; hints never substitute an ID | [25](contracts-and-diagnostics.md) | `table.rs`, `explain.rs`, `error/*`; representation/diagnostic cases |
| Ordered parallel execution | Rayon indexed collection then first input-order error; WASM serial collection | Compile-target selected | [25](contracts-and-diagnostics.md) | `parallel.rs`; ordered-output/error cases |
| Schema generation and publication | Rust-derived schemas; assertion-view externalization; deterministic sync/index | Explicit registry and write/check/list | [26 — Schemas](schema-publication.md) | `schema/{registry,externalize,generator,mod}.rs`, CLI; structural/drift cases |
| LLM schema projection | Budgeted handles, description compaction, enum/tuple/required-shape transforms | Explicit profile, 16 KiB/240 defaults | [26](schema-publication.md) | `schema/llm.rs`; projection cases |
| Schema diagnostic expansion | Tag-aware/fewest-error union branch explanation, max depth four | Optional jsonschema-validate feature | [26](schema-publication.md) | `schema/diagnostics.rs`; union/reference failure cases |

## Evidence boundaries and follow-up tracking

The records separate observed numerical mechanisms from intended financial use.
Adoption questions, implementation reconciliation and missing evidence are maintained
in the [core-numerics follow-ups](../FOLLOW_UPS.md#core-numerics).

The coverage ledger is a bounded source audit, not a correctness certification,
performance study, new financial-policy adoption, or an audit of every downstream
caller and host wrapper. Product-specific defaults and acceptance criteria belong
to their owning slice records even when they call these kernels.
