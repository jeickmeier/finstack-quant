# Core numerics methodology audit

This slice covers **65 assigned Rust source files** in core numerics,
interpolation, expression evaluation, validation, configuration, wire encoding,
canonical identity and schema tooling: **58 algorithm-bearing, 5 support and 2
dedicated test files**. The exact path ledger is [coverage.json](coverage.json).
All runtime implementations in that ledger were inspected, including runtime
items following test declarations/helpers in Brownian bridge, schema and wire
modules. Inline tests were inspected selectively for the evidence roles stated
in each record; dedicated tests are not counted as production algorithms.

Audit date: **2026-11-05**. Source revision:
`7e82fd03ad98346727769e88dd5ac5d84b56e9fc`. This documentation-only audit changes
no algorithms, defaults, runtime tests, fixtures or accepted decisions.

The **26 records are Draft** observations awaiting scoped adoption. Implementation
status is **Observed** within the listed path scope. Validation status is
**Evidence inspected/located, not executed**: no numerical suites or builds ran.
Formula fixtures, dependency comparisons, roundtrips, self-consistency checks and
statistical sanity tests are distinguished. Shared numerical functions do not
select a product model or establish market fitness. The algorithm inventory
below is more granular than the file ledger: files can own several independent
choices, and one coherent choice can span several files.

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
