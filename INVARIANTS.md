# finstack Workspace Invariants

This catalogue retains detailed cross-crate financial, numerical, and API
contracts. [Accepted decisions](docs/decisions/README.md) own the agreed
policy, scope, and rationale; this file owns the finer operational rules
referenced by those decisions. API-specific details remain with their
designated documentation and serialized-contract sources.

**Policy status and evidence status are separate.** An accepted decision
governs only its stated scope. Related ADR links do not independently approve
every existing method, default, or detailed clause. Existing requirements
remain in force unless explicitly superseded; an unmapped requirement is not
permission to change or discard it.

**Required** identifies a normative rule. **Evidence** identifies supporting
types, source, schemas, or tests and the scope they cover. Source inspection
and the presence of a test do not establish complete runtime verification.
Partial or unverified coverage is stated separately from the requirement.

The [invariant-to-decision map](docs/decisions/INVARIANT_MAP.md) identifies a
corresponding accepted decision for every normative clause, including rules
in prose and tables. The invariant-detail review is complete; acceptance
establishes the stated contracts, not complete implementation or validation.

Outstanding implementation, reconciliation, and verification work belongs in
[FOLLOW_UPS.md](docs/decisions/FOLLOW_UPS.md). Resolve conflicts through the
specific accepted decision and authorized scope, as described in
[section 9](#9-authority-and-enforcement); do not change expectations merely
to match the current code.

---

## 1. Money and model numerics

**Authority:** [ADR 0020](docs/decisions/financial/conventions/0020-numerical-representation-and-rounding.md).
**Scope:** monetary representation, arithmetic, conversion, and rounding;
model `f64` numerics remain distinct from contractual monetary amounts.

### 1.1 Representation

**Decision:** [0020](docs/decisions/financial/conventions/0020-numerical-representation-and-rounding.md)
is accepted for representation, Money construction without implicit currency
quantization, explicit rounding, and non-finite monetary-input rejection.
[Clause map M01–M05](docs/decisions/INVARIANT_MAP.md#1-money-and-model-numerics).

`finstack_quant_core::money::Money` stores
`rust_decimal::Decimal` plus a `Currency`. `Money::new` accepts `f64` and returns `Result`; `Money::amount()` returns `f64`.
`Money::from_decimal` and `Money::amount_decimal()` provide the lossless
Decimal path.

**Required:**

- Curves, rates, volatilities, correlations, returns, greeks, optimizers, and
  Monte Carlo paths MAY use `f64`.
- **New** accounting, ledger, settlement, regulatory-reporting, and
  margin-dispute paths MUST use `Decimal` or `Money` without an intermediate
  `f64` round-trip. Existing aggregation gaps are tracked in
  [monetary follow-up](docs/decisions/FOLLOW_UPS.md#implementation-and-lifecycle-follow-up);
  they do not permit new `f64` settlement-grade paths.
- Wrapping an `f64` result in `Money` gives it currency semantics and Decimal
  storage; it does **not** make the preceding calculation Decimal-exact.
- `Money::new` preserves sub-minor-unit precision. It does
  not imply ISO-4217 quantization. Currency-scale rounding MUST be requested
  through configuration or applied explicitly when an amount is finalized.
- Non-finite monetary inputs MUST be rejected.

**Evidence:** [Money storage, constructors, and regression tests](finstack-quant/core/src/money/types.rs)
support the Decimal representation, explicit conversion, and non-finite-input
rejection. This does not certify every downstream monetary calculation.

The [accepted monetary precision/currency contract](docs/decisions/financial/core-conventions/012-money-decimal-precision.md#accepted-decision-monetary-precision-rounding-and-currency-identity)
requires exact Decimal/integer/text/wire admission, shortest-roundtrip-text float
admission, normalized zero and alphabetic currency identity. Money sums are exact
or errors; bounded scalar/FX transforms round once to the greatest fitting scale,
nearest-even with a half-quantum bound, rejecting out-of-range magnitude and
nonzero underflow. Float views require correctly rounded nearest-even conversion;
Money ratios require the stated `4*f64::EPSILON` relative-error budget without a
bounded Decimal quotient. Financial rounding requires explicit scale/mode;
missing ingest overrides preserve precision, while display defaults remain
presentation only. FX conversion uses explicit positive Decimal rates, with exact
unit rates required for same-currency conversion. Currency metadata and unique-only
numeric lookup use a frozen, reviewed historical-aware catalogue. These are
accepted requirements; backend accuracy, data/host/caller reconciliation and
independent qualification remain pending. [Clause map M10](docs/decisions/INVARIANT_MAP.md#1-money-and-model-numerics).

### 1.2 Arithmetic and aggregation

**Decisions:** [0020](docs/decisions/financial/conventions/0020-numerical-representation-and-rounding.md)
is accepted for arithmetic boundaries;
[0042](docs/decisions/standards/0042-numerical-accumulation-and-comparisons.md)
is accepted for floating-point accumulation and its finite-input requirements.
[Clause map M06–M09](docs/decisions/INVARIANT_MAP.md#1-money-and-model-numerics).

**Required:**

- Use Decimal arithmetic where rounding affects a contractual amount, posted
  balance, settlement instruction, regulatory submission, or dispute.
- `f64` aggregation is acceptable for model PV, risk, and analytical totals
  when the boundary and rounding policy are documented.
- For floating-point totals, use compensated summation for mixed-sign cash
  flows, large dynamic ranges, or totals whose floating-point error can affect
  a reported result. Use
  `finstack_quant_core::math::summation::{neumaier_sum, NeumaierAccumulator}`.
  Use `OnlineStats` for online mean and variance, not as a substitute for
  compensated totals. Required Decimal monetary paths remain Decimal.
- Validate finiteness before Kahan/Neumaier accumulation. Compensation does
  not make `NaN` or infinity safe.

**Evidence:** the shared [summation](finstack-quant/core/src/math/summation.rs)
and [statistics](finstack-quant/core/src/math/stats.rs) modules provide the
numerical primitives. Their existence does not establish that every caller
validates finiteness or uses the required arithmetic. The concrete monetary
paths needing classification and correction are recorded in
[FOLLOW_UPS.md](docs/decisions/FOLLOW_UPS.md#implementation-and-lifecycle-follow-up).

---

### 1.3 FX resolution, state and shocks

The [accepted FX resolution/state/shock contract](docs/decisions/financial/core-conventions/013-fx-resolution-and-shocks.md#accepted-decision-fx-resolution-state-and-relative-shocks)
requires explicit point-date policies and routing, with no generic PeriodAverage.
Matrix authority precedes providers, scoped pins precede globals within each layer,
and direct orientation precedes reciprocal within the winning scope. Identity is
exactly one; stored identity quotes are rejected. Observed caches preserve exact
ordered query results and do not normalize independently supplied directions.

Single-pivot fallback is permitted only after an explicitly missing provider quote.
Other failures and leg causes remain errors; requested reciprocal/cross results
must be finite and positive without clipping or unrequested reciprocal admission.
Provider/matrix generations must be coherent and never reused, with bounded retries
and the stated limits on unversioned provider calls.

Snapshots must be complete, validated and faithfully restorable or explicitly
unsupported. Fresh restoration and bulk writes are atomic and reject duplicate
normalized keys; merge-style restoration and silent empty provider snapshots are
removed. Relative pair shocks use an explicit reference query and frozen complete
baseline, applying the factor once to each independently resolved direction;
selected dependent crosses recompute. Invalid/ineffective nonzero shocks fail;
unsupported faithful shock capture cannot silently flatten the scenario.

These are accepted requirements; runtime/host/caller reconciliation and independent
provider/state/risk qualification remain pending.
[Clause map M11–M14](docs/decisions/INVARIANT_MAP.md#1-money-and-model-numerics).

### 1.4 Dated NPV and continuous discounting

The [accepted discounting/NPV contract](docs/decisions/financial/core-conventions/016-discounting.md#accepted-decision-dated-npv-selection-arithmetic-and-continuous-discounting)
requires explicit valuation date and same-day inclusion, full currency/finiteness
validation before filtering, and no unused curve queries for excluded, same-day
or zero cash. Empty Money input errors without a currency; empty scalar input is
zero. Nonzero future cash uses the source's dated relative-factor interface with
its admitted clock/context/domain and required finite-positive endpoint factors
and quotient. Caller owns coherent source/currency/entitlement selection; no local
clock, curve-ID currency or default settlement-phase inference is permitted.

Money NPV follows 012's factor admission, bounded products and exact-or-error sums,
with explicit financial rounding at the owner's boundary. Scalar NPV uses checked
binary64 products and source-order shared Neumaier accumulation, rejecting nonfinite
intermediates and nonzero-product underflow. Preserve input order and additive rows;
no implicit netting, permutation-invariant admission or uniform relative-PV accuracy
is promised. The flat helper uses a finite continuous decimal rate and finite
nonnegative horizon, validates inputs before exact zero identities and requires
positive finite output, allowing negative rates/subnormal factors but rejecting
zero underflow. Exponential/model accuracy and owner-specific financial budgets
require independent qualification, without an unproved global error guarantee.
Runtime/product/host/numeric reconciliation remains pending.
[Clause map M15–M18](docs/decisions/INVARIANT_MAP.md#1-money-and-model-numerics).

### 1.5 IRR and XIRR

The [accepted IRR/XIRR contract](docs/decisions/financial/core-conventions/017-irr.md#accepted-decision-irr-and-xirr-timing-eligibility-and-numerical-acceptance)
requires effective decimal rates greater than -1 from explicitly prepared economic
cash. Periodic IRR preserves original period spacing through zeros; canonical XIRR
uses actual days divided by 365, with one canonical checked rounded net per date
used consistently for origin, one-switch eligibility, objective and residual.
Remove public guesses/generalized-clock inputs. Underdetermined and unsupported
streams error; more-than-one-switch rejection does not prove multiple roots.

Use the full finite log-growth domain and shared fallible safeguarded Brent with
100 iterations and operational log tolerance 1e-12. Handle finite rate endpoints
directly and send every result through finite coordinate/rate checks plus a
recomputed finite gross-PV residual <=1e-8 at the returned rate. No fake infinity,
NaN success, endpoint clamp, alternative root or bypass is permitted. These are
operational numerical criteria, not uniform true rate-error or exact-netting
certification. Product-specific yields/initial-terminal paths and reporting remain
owner-selected; remove automatic periodic IRR inferred from flattened captured
payoff components. Runtime/solver/product/host reconciliation and independent
qualification remain pending.
[Clause map M19–M22](docs/decisions/INVARIANT_MAP.md#1-money-and-model-numerics).

### 1.6 Rate and Bps units

The [accepted Rate/Bps contract](docs/decisions/financial/core-conventions/018-rate-units.md#accepted-decision-rate-and-bps-units-admission-and-precision)
requires finite signed binary64 decimal Rate values and exact whole signed i32
basis points, with normalized zero and no inferred economic floor, clock,
compounding or risk-bump role. Numeric conversions use one checked binary64 unit
operation; Rate bp construction/extraction preserves fractions and the implicit
rounded/saturating quantizer is removed. Quote text applies its unit before one
nearest-even final float conversion, independently of Money's bounded Decimal
admission; text and numeric inputs need not have identical rounded results.

Validate finite scalar operands before shortcuts, reject nonfinite results and
nonzero multiply/divide/scaling underflow, and retain nonzero signed subnormals.
Bps integer arithmetic is exact or errors, including nonintegral division and
minimum-i32 overflow. Default Rate display is shortest decimal text; canonical
wire remains numeric decimal Rate versus whole-bp Bps, without recovering intent
already lost by a numeric decoder. Product-specific rounding/whole-bp eligibility
and 012 monetary boundaries remain separately owned. Runtime/host/product
reconciliation and independent conversion qualification remain pending.
[Clause map M23–M26](docs/decisions/INVARIANT_MAP.md#1-money-and-model-numerics).

### 1.7 Credit ratings and scorecard scales

The [accepted ratings/scorecard contract](docs/decisions/financial/core-conventions/019-credit-ratings.md#accepted-decision-generic-ratings-and-explicit-scorecard-scales)
keeps a consecutive generic AAA-through-D library ladder with NR outside rated
quality comparisons and all notch arithmetic. Equality of NR is status identity;
no total financial order, PD, agency provenance or equal-risk spacing is implied.
Grade predicates and the D default marker have literal meanings; unsupported
agency states, suffixes and hybrid aliases error. Canonical wire labels remain
separate from human aliases; Moody-style D rendering errors and NR stays status text.

Scorecard numbers are explicit caller-owned finite [0,100] utility, higher better.
Remove embedded numeric defaults, implicit scale selection and fallback policies;
resolve one explicit immutable registry/selected scale snapshot with source data.
Complete best-to-worst bands cover [0,100], boundaries belong to the better band,
and every representative classifies to its own band. Normalized IDs/labels and
threshold/minimum references are strict; NR aliases cannot be numeric bands.

Construct and restore private validated state atomically; preserve complete model
values/order/identity/source in snapshots and distinguish lossy presentation tables.
Metric weighting/missing-factor policy, empirical PD/transition/recovery/WARF,
actual agency/default state and product/regulatory eligibility stay separately
owned. Runtime/container/consumer/host reconciliation and independent quantitative
and production qualification remain pending.
[Clause map M27–M30](docs/decisions/INVARIANT_MAP.md#1-money-and-model-numerics).

### 1.8 Compounding and periodic spreads

The [accepted scalar compounding contract](docs/decisions/financial/core-numerics/compounding.md#accepted-decision-explicit-compounding-conversion-and-periodic-spreads)
requires explicit Continuous/Simple/Periodic quotation and exact positive u32
periodic frequency, without a generic default or tenor/payment-frequency inference.
Rates and additive spreads are finite annual decimals on a caller-owned common
nonnegative clock/interval; positive finite DFs may exceed1 or be subnormal.
Periodic/simple quote and returned-rate domains are checked.

A validated zero-time rate produces DF1, but inverse rate extraction requires
positive time; DF1 at zero does not identify a rate. Known-rate cross-conversion
retains its explicitly defined t->0+ limit. Stable direct log accumulation avoids
unnecessary DF round-trip information loss and uses checked range/underflow handling;
valid output DF rounding to1 is distinct from lost requested rate/shift information.
No NaN, fabricated zero, arbitrary tenor floor or unchanged-factor failure fallback.

Instantaneous spread restatement remains distinct from finite Simple zero-spread
addition. Periodic z-spreads use typed frequency and shared stable log shifting;
zero z preserves an admitted DF, t=0 requires DF1 and negative t errors. Check
shifted domains and positive finite final factors; no clipping or frequency repair.
Keep strict separate text/wire forms and existing host surfaces, propagate errors,
and reconcile source-owned clock/context and actual curve/scenario/product callers.
Mathematical equivalence is not exact round-trip or portable ULP/relative-error
certification; runtime/host/caller reconciliation and independent qualification
remain pending. Product quote, spread, coupon and solver acceptance stay separately
owned. [Clause map M31–M34](docs/decisions/INVARIANT_MAP.md#1-money-and-model-numerics).

### 1.9 Scalar interpolation and boundaries

The [accepted scalar interpolation contract](docs/decisions/financial/core-numerics/value-interpolation.md#accepted-decision-validated-scalar-interpolation-and-boundary-semantics)
selects explicit Linear/LogLinear/the specified PCHIP CubicHermite and Flat/Extend/
Error policies behind one immutable validated public Interpolator. Internal
strategies cannot be paired publicly with different arrays. Admit finite strictly
ordered distinct axes without a relative gap floor and finite signed Linear/PCHIP
ordinates; LogLinear intrinsically requires positives. Remove ValidationPolicy,
shared defaults and repair/unchecked alternate paths; owners retain finance domains.

Return stored exact-knot/flat-tail values directly. Linear/LogLinear derivatives
at interior knots are left-sided and endpoints use their inward derivative;
PCHIP uses its nodal slope, is C1 inside and generally not C2. Extensions are
literal value/log-space rules, not a universal zero/forward-rate convention.
Queries, required caches and results are checked and fallible; stable blends,
log differences and normalized PCHIP arithmetic avoid false flats/range loss.
No NaN, clipping, epsilon snapping, style replacement or global ULP guarantee.

Restore complete private style/knots/values/policy state atomically and rebuild
caches. Remove interp_knots_flat; reuse canonical interpolation for 2+ profiles
and preserve explicitly owner-supported validated singleton constants. Private
monotonicity diagnostics do not certify no arbitrage or repair data. Shared record 03
integration is mechanical; forward-specific economics and curve/host/calibration
acceptance remain separately owned. Runtime/consumer/host reconciliation and
independent numerical qualification remain pending.
[Clause map M35–M38](docs/decisions/INVARIANT_MAP.md#1-money-and-model-numerics).

### 1.10 Forward interpolation

The [accepted forward-interpolation contract](docs/decisions/financial/core-numerics/forward-interpolation.md#accepted-decision-natural-log-discount-and-hagan-west-interpolation)
retains fixed natural cubic in Y=-lnD and unsmoothed Hagan-West with its optional
positivity collar omitted. Positive finite DFs may increase or exceed 1 on any
supplied finite ordered coordinate grid; normalization/origin/dates/clock and
financial eligibility remain owner contracts, not synthetic generic method inputs.

Natural Y/DF are C2 in-range and forwards are piecewise quadratic/C1, with
Y''=f'=0 endpoints and two-point LogLinear reduction. Use a scaled checked
tridiagonal solve without absolute-EPS pivot rejection. Preserve its global fit
and possible negative/overshooting forwards; no smoothing or positivity repair.

Hagan-West preserves interval averages through matching regional g/G, with exact
sign/ratio classification and no fixed near-zero epsilon or automatic rate-regime
projection. Exact zero-axis limits can create forward jumps. At knots use actual
left/inward DF derivatives, not incompatible provisional node targets. Extend
uses the actual inward endpoint forward; Flat and exact knots return source DFs.

Apply accepted record 02 immutable/strict restored state and fallible common API, stable
required-cache/requested-quantity evaluation and positive finite DF output checks.
No NaN, zero-underflow factor, clipping, implicit method replacement or universal
continuity/positivity/no-arbitrage/ULP claim. Runtime/host/consumer reconciliation
and independent numerical qualification remain pending; calibration/risk budgets
and curve policy stay separately owned.
[Clause map M39–M42](docs/decisions/INVARIANT_MAP.md#1-money-and-model-numerics).

### 1.11 Checked quadrature

The [accepted quadrature contract](docs/decisions/financial/core-numerics/quadrature.md#accepted-decision-checked-quadrature-budgets-and-measures)
retains explicit Simpson/scalar-and-pair adaptive, fixed-order Legendre/Hermite
and reusable-grid APIs with canonical fallible callbacks/results. Validate finite
bounds and controls before exact zero-width/no-callback identities; reversed
integrals use canonical orientation and negate the value. Grid data deliberately
requires a positive interval and finite positive scaled weights/checked allocation.

Adaptive Simpson returns uncorrected refined totals with /15 difference proxies;
adaptive Legendre uses raw refined/coarse differences. Conserved finite nonnegative
absolute budgets and final summed-estimator gates apply per component, with two
explicit pair tolerances/shared refinement. Depth limits alone do not establish
success; hard explicit callback budgets and bounded traversal prevent unbounded
refinement/stack use. Truthful errors identify actual work, component and sampled
coordinate where known, preserving callback categories without sentinel results.

Use stable geometry and checked scaled compensated accumulation, with no collapsed
needed nodes, nonfinite successes or detectably unusable arithmetic masquerading
as zero/convergence. Hermite's public integral is a standard-normal expectation;
raw getters retain physicists' measure. Supported tables require provenance and
independent qualification, not product precision presets. Estimated tolerance is
not a true-error certificate, including zero estimates on missed smooth features.

Keep the Rust-only surface and existing scalar/tuple/error mechanisms. Callers
own measures, endpoints/breakpoints, tails/transforms, nested work/accuracy budgets
and financial acceptance. Runtime/caller/table reconciliation and independent
numerical/financial qualification remain pending.
[Clause map M43–M46](docs/decisions/INVARIANT_MAP.md#1-money-and-model-numerics).

### 1.12 Checked Brent solving and bounded search

The [accepted Brent contract](docs/decisions/financial/core-numerics/brent-root.md#accepted-decision-checked-brent-solving-and-bounded-search)
uses one safeguarded derivative-free kernel with canonical fallible callbacks,
validated immutable explicit positive coordinate tolerance and update budget.
Remove duplicate infallible entry points, defaults and mutable configuration.
Validate finite inputs before callbacks or zero shortcuts. Sample ordered initial
endpoints with immediate first-error/nonfinite rejection, cache matching residuals,
and compare finite signs directly; singleton success requires computed zero.

Search has explicit finite domain, admitted centre, positive half-width and
expansion budget. Preserve deterministic frozen-width left-first expansion
(unclipped width factor five), with no guess clamping, hidden scan or retry.
Search failure does not prove root absence. Safeguarded secant/IQI proposals use
internal bisection when unusable; stable geometry must not fabricate convergence
from overflow, underflow or repeated coordinates. Unmet convergence without a
representable new point is stagnation failure.

Stop on computed zero or the retained coordinate half-width criterion, not a
currency/residual tolerance. Only width-based termination implies the stated
full-bracket-width bound. Count actual search and new Brent evaluations, test the
last permitted update, and preserve truthful matching x/residual diagnostics.
No finite sentinel, callback-error recovery or last-iterate exhaustion success.

Callers own continuity, domains, root selection and acceptance after returned
value transformations, including every computed-zero/endpoint exit. Preserve
accepted IRR/XIRR controls and final gate. Keep the Rust-only scope and shared
kernel, without universal root/residual/ULP/convergence-order claims.
Runtime/caller reconciliation and independent qualification remain pending.
[Clause map M47–M50](docs/decisions/INVARIANT_MAP.md#1-money-and-model-numerics).

### 1.13 Checked damped least squares and outcomes

The [accepted LM contract](docs/decisions/financial/core-numerics/levenberg-marquardt.md#accepted-decision-checked-damped-least-squares-and-outcomes)
keeps one dense local least-squares method, including rectangular/rank-deficient
systems, with explicit immutable validated positive residual/step tolerances and
attempt budget. Canonical residual and supplied-Jacobian callbacks are fallible,
fixed-shape and completely finite; remove the optional derivative trait, automatic
switching, defaults/builders and missing-entry/error sentinels. Explicit numerical
derivatives use checked actual-spacing three-point central differences, without
hidden one-sided/shrunk probes or callback-error recovery.

Replace LM normal equations with one augmented column-pivoted QR step using the
existing dependency, checked scaling and triangular/permutation handling. Preserve
the norm-defined damping metric and disclosed bounded factor-ten heuristic; no
fallback registry or general linear-algebra adoption. Use stable required norms,
finite candidate arithmetic and checked dimensions/work/resources. Invalid data,
callbacks or QR arithmetic fail rather than become ordinary damping rejections.

Only ConvergedResidualNorm asserts the computed residual target. Small-step,
damping-limit and exhausted-budget outcomes retain the finite last accepted state
without declaring a financial fit. Count actual attempts, accepted steps and core
residual/Jacobian calls separately; optional last-step/damping diagnostics refer to
the last attempt, and the accepted residual gate includes the final permitted one.

Callers retain financial units, weights, transforms, domains, initialization and
selection/restart policy, with finite final repricing/quote gates after coordinate
transforms. Keep the Rust-only scope and enclosing host mappings. No blanket
parameter-scale invariance, global optimum, identification or universal accuracy
claim. Runtime/caller/host reconciliation and independent qualification remain
pending; dependency allocation does not imply universal out-of-memory recovery.
[Clause map M51–M54](docs/decisions/INVARIANT_MAP.md#1-money-and-model-numerics).

### 1.14 Checked matrix factors, solves and correlation admission

The [accepted matrix contract](docs/decisions/financial/core-numerics/matrix-factorization.md#accepted-decision-checked-matrix-factors-solves-and-correlation-admission)
requires finite numerically exact symmetry for full SPD/PSD/eigen matrices,
checked shapes/resources and matched empty identities with valid controls.
Explicit lower-factor operations ignore the unused upper triangle. Ordinary
Cholesky requires positive finite computed pivots; provided-factor solves require
nonzero finite signed diagonals. Remove arbitrary singularity/normal-minimum
floors; eligibility is not a conditioning or forward-accuracy certificate.

Use generic cholesky_psd/PsdFactor naming and explicit relative truncation
tolerance in [0,1). Check the entire stopped Schur block, restore source units
after normalization, preserve original variable and innovation-column labels,
and break pivot ties by smallest original index. Positive tolerance can discard
negative directions and change the covariance; no exact PSD/rank/reconstruction
certificate. Immutable finite raw factors define real-arithmetic covariance,
not unit correlation or guaranteed representable covariance entries.

Correlation validation separately requires exact unit diagonal, symmetry and
coefficients in [-1,1], followed by the explicit operational PSD gate. Eigen
solving requires explicit deflation tolerance and positive bounded iterations,
finite sorted eigenvalues with matched vector columns, and no eigen clipping.
Use stable checked arithmetic/application, truthful input versus computation
errors and existing host matrix shapes/mappings. Discard error-output buffers
before state commit; no new atomic hot-path/report or fallback framework.

Keep repair/shrinkage, financial covariance/CRN strategy and risk/error acceptance
with their owners. No automatic jitter, identity substitute, projection or
pseudoinverse, no unique eigenvector basis/sign, and no universal accuracy/bitwise
claim. Runtime/caller/host reconciliation and independent qualification remain
pending.
[Clause map M55–M58](docs/decisions/INVARIANT_MAP.md#1-money-and-model-numerics).

### 1.15 Explicit nearest-correlation projection and repair limits

The [accepted explicit repair contract](docs/decisions/financial/core-numerics/nearest-correlation.md#accepted-decision-explicit-nearest-correlation-projection-and-repair-limits)
selects the unweighted full Frobenius nearest-correlation problem and one
Higham/Dykstra path. Finite square input is symmetrized stably and initialized
with unit diagonal, preserving the mathematical minimizer; retain original input
for displacement. Six immutable validated controls explicitly select outer/eigen
tolerances and budgets, numerical PSD admission and maximum repair distance.
Remove defaults, optional host controls and arbitrary near-input gates.

Use bounded full spectral PSD projection and the PSD-cone Dykstra correction,
with three relative Frobenius stopping tests and explicit zero-ratio rules.
Actual candidates must pass accepted record 07 structure/PSD admission; ordinary
finite infeasibility continues within budget, while computation errors propagate.
Remove approximate-factor early success and final PSD/rescaling/floor changes.
Return the admitted stopped iterate only after the original-input distance gate;
distance refusal is a distinct domain error, not numerical nonconvergence.

Count all completed iterations, including infeasible stopped candidates, and test
the final permitted one. Use checked stable arithmetic/resources and truthful
errors with immutable input/new matrix-only output, matched empty identities and
no partial repaired result/report framework. Strict feasibility may exhaust the
budget even when a mathematical limit exists; zero distance permits no computed
change and does not imply a bitwise fixed-point guarantee for valid inputs.

Owners retain financial repair permission, source/output/order/control provenance
and price/risk acceptance in existing artifacts. No automatic strategy approval,
shrinkage, fallback, exact PSD or nearest-within-tolerance certificate follows.
Runtime/caller/host reconciliation and independent qualification remain pending.
[Clause map M59–M62](docs/decisions/INVARIANT_MAP.md#1-money-and-model-numerics).

### 1.16 Checked sums, moments, quantiles and mean uncertainty

The [accepted sample-statistics contract](docs/decisions/financial/core-numerics/sample-statistics.md#accepted-decision-checked-sums-moments-quantiles-and-mean-uncertainty)
requires literal finite equally weighted observations and explicit equal-length
indexwise pairs. Canonical Result/Option estimates separate finite availability,
genuine insufficient/degenerate cases and errors; remove insufficient-zero and
parallel NaN estimator families. Explicit finite min/max/count filters retain
their separate purpose. Kahan/Neumaier remain named source-order checked folds,
with atomic accumulator additions and no exact/permutation-invariant guarantee.

Use private shifted/scaled compensated centered state with Welford/Chan algebra,
genuine constant-history evidence, atomic checked updates/merges and exact-count
admission through min(usize::MAX,2^53). Evaluate each requested mean, moment, SD,
SE or normalized ratio without requiring unrequested unscaled sums, moments,
variances or scale products. Pearson requires both axes genuinely varying and
finite output within [-1,1] without a blanket clamp; beta requires an identified
varying control. Keep sample/population denominators and availability explicit.

R7 has strict p in [0,1], checked binary64 rank residual/convex interpolation,
exact support identities and complete sorted-input validation. Mutable scratch
may remain reordered after later calculation failure. Student-t mean intervals
use explicit alpha in (0,1), sample SE and direct half-width; remove the core
95% shortcut, propagate tail/range errors and reject collapsed positive-width
bounds. Genuine eligible constants have point intervals without asserting
population certainty. IID/nondegenerate-Gaussian and other inferential assumptions
remain distinct from calculation and sequential-stopping/product policy.

Reconcile canonical getters, Result/Option and host/caller surfaces without new
online host classes or state framework. Accepted analytics/NPV/Monte Carlo owners
retain their stated meanings, labelled confidence and units. No annualization,
implicit filtering, exact rounding, cross-platform/merge-bit identity or universal
coverage/accuracy certificate. Runtime/caller/host and independent qualification
remain pending.
[Clause map M63–M66](docs/decisions/INVARIANT_MAP.md#1-money-and-model-numerics).

### 1.17 Explicit log returns and realized variance

The [accepted log/realized-variance contract](docs/decisions/financial/core-numerics/realized-variance.md#accepted-decision-explicit-log-returns-and-realized-variance)
requires positive finite prices, complete coherent OHLC bars and explicit
positive annualization with no daily252, date-index or method inference. Checked
log_returns preserves adjacent differences, including wide/nearby stable ratios,
with no NaN holes or skipped observations. Variance estimates use Result/Option
availability after full request validation; log transforms return a complete vector.

Keep five explicit estimators and one seven-field OHLC request. CloseToClose and
YangZhang require a separate preceding close and use all n target bars; standalone
N close levels retain N-1 intervals. Close is an uncentered squared-log second
moment. Parkinson, practical GK19a and RS use equivalent nonnegative algebra;
YZ uses all-n overnight/intraday sample variances, mean RS and the disclosed
practical fixed weight. Minima are two standalone levels, one anchored close/
range bar and two YZ bars; estimator-specific genuine zeros remain available.

Use stable complete-result scaling/averaging and accepted count/moment rules.
Remove negative-result allowances/floors; invalid requests and computation/range
failures have truthful distinct categories. No partial output, hidden anchor,
default/substituted estimator or covariance/volatility/horizon reinterpretation.
Reconcile canonical host parameters, availability, method-schema defaults and
actual callers without restoring hidden enum/financial defaults.

Owners retain sampling/adjustment/window, contractual252, payoff/forecast and
financial precision policy. Drift, opening-gap and efficiency claims remain
model-conditional; discrete extrema, jumps and dependence are not automatically
qualified. Accepted analytics frequency/return/terminal-loss semantics remain
separate. Runtime/caller/host and independent qualification remain pending.
[Clause map M67–M70](docs/decisions/INVARIANT_MAP.md#1-money-and-model-numerics).

### 1.18 Checked scaled-identity covariance shrinkage

The [accepted covariance-shrinkage contract](docs/decisions/financial/core-numerics/covariance-shrinkage.md#accepted-decision-checked-scaled-identity-covariance-shrinkage)
requires finite row-major T-by-N data with T>=2,N>=1, checked shapes/resources
and exact-count admission. Always estimate column means and use population /T
covariance S, fixed mu=trace(S)/N and target mu*I. Full Frobenius second/fourth
deviations define the estimated intensity with the statistical upper cap; genuine
identity ties return S with canonical intensity0, without an epsilon or ridge.

Reuse private shifted/scaled centered calculations, preserve original relative
column units and consequential mixed-scale moments, compare scaled norms and
retain the complementary sample weight without subtracting rounded intensity
from1. Require only the complete requested result to fit; unresolved required
arithmetic or detectably lost required positive output is a computation failure.
No false underflow-based constant/isotropy classification or clipping/repair.

Keep the simple covariance/intensity result with complete finite exactly mirrored
covariance, nonnegative diagonal and intensity in[0,1]. Separate invalid requests
from numerical failures. Mathematical PSD, trace/eigenvalue identities and common
scaling invariance do not promise exact rounded PSD, invertibility, adequate
conditioning, bitwise reconstruction or finite-sample statistical optimality.
No generic eigen/factor post-gate, hidden loading or automatic fallback.

Reconcile existing host pairs, errors and rectangular docs. Financial owners
retain complete-case selection, annualization, stable covariance-to-correlation
normalization, zero-variance policy, model acceptance and needed estimator/
sample-count/order/intensity provenance. Runtime/caller/host reconciliation and
independent numerical/statistical/financial qualification remain pending.
[Clause map M71–M74](docs/decisions/INVARIANT_MAP.md#1-money-and-model-numerics).

### 1.19 Checked special functions and distribution domains

The [accepted special-function/distribution contract](docs/decisions/financial/core-numerics/distribution-functions.md#accepted-decision-checked-special-functions-and-distribution-domains)
selects one canonical Result scalar surface for all ten erf/log-Gamma/normal/
Student-t operations. Reject NaN and invalid finite parameters/probabilities;
validate complete requests before identities. Forward variate infinities have
explicit limits. Inverse finite p in[0,1] has exact endpoint infinities, median0
and finite interior results or computation/range failure. Positive-real log-Gamma
has finite x>0 only, exact zeros at1,2 and same-function positive recurrence.

Retain the standard normal profiles and evaluate general-normal differences,
standardization and final density/Jacobian without avoidable intermediate range
loss. The fixed log-Gaussian-tail profile has switch-10, decreasing asymptotic
terms and at most99 updates. Ordinary final probability/density/erf endpoint
rounding and positive-tail log0 are distinct from false intermediate saturation;
unrepresentable finite-input negative log magnitude errors rather than returning
an undeclared infinity. Series bounds do not certify total floating-point error.

Student-t is centered unit scale with any finite df>0, without a normal cutoff
or financial variance normalization. Select one narrow private checked beta
adaptation: at most140 continued-fraction iterations with abs(update-1)<=2^-53,
and1024 total inverse outer/inner loop bodies with the disclosed inherited
AS64/AS109 stopping profile. Check transformed geometry, log-Gamma ratios,
progress, work and safe signed quantile restoration; unresolved arithmetic or
convergence fails explicitly. Preserve source/license provenance, with no public
beta framework, catch-panic path, input clipping or alternate solver fallback.

Require each stated output range, truthful input/computation errors and existing
scalar host/parity reconciliation. Preserve separately owned CI/inference,
copula/RNG endpoints, PD/delta/model policy and log-reflection financial claims.
No universal rounding, inverse accuracy, bit identity, all-input success or
financial fitness certificate. Runtime/caller/host reconciliation and independent
numerical/statistical/financial qualification remain pending.
[Clause map M75–M78](docs/decisions/INVARIANT_MAP.md#1-money-and-model-numerics).

### 1.20 Checked full binomial probability mass

The [accepted full-binomial contract](docs/decisions/financial/core-numerics/binomial-mass.md#accepted-decision-checked-full-binomial-probability-mass)
retains one allocating/scratch Result kernel with all n+1 count probabilities
in ascending order. Require finite p in[0,1] and integer 0<=n<=1,000,000 before
all identities, checked shape/bytes and fallible reservation. Admitted n0/p0/p1
have their exact dense identities. The fixed cap is a resource policy, not a
statistical or portfolio-size recommendation.

Choose the upper exact mode floor((n+1)*p) against actual binary64 input bits
using private bounded integer arithmetic. Seed modal relative weight1 and recurse
outward through the full support. Single-rounded ordinary and FMA complement
products preserve ordered nonnegative numerator/denominator coefficients; form
complete ratios before weight products without odds overflow or premature
underflow. No forward log anchor, coefficient clamp or alternate approximation.

Use checked source-order Neumaier normalization once, finite nonnegative ordered
weights and Z in[1,n+1]. Successful cells are finite in[0,1], with positive modal
mass and outward monotonicity; compensated final mass satisfies
abs(M-1)<=8*f64::EPSILON. This gate is not a pointwise/tail accuracy certificate.
No repeat normalization, residual redistribution, probability floor or hidden
repair. Representational zeros are permitted without heuristic support truncation;
do not infer impossible counts, exact stored mass1 or negligible weighted tails.

Preflight input/resource or reservation failure preserves scratch values/length;
computation failure after writes leaves scratch unspecified and discarded.
Distinguish input from computation/resource errors with bounded work and no
successful partial result. Preserve Rust-only exposure and separately owned
conditional credit dependence, homogeneity/routing, exposure/recovery, pool
counts, approximations and tail-weighted financial acceptance. Runtime/caller
reconciliation and independent numerical/statistical/financial qualification
remain pending.
[Clause map M79–M82](docs/decisions/INVARIANT_MAP.md#1-money-and-model-numerics).

### 1.21 Checked beta and Gamma sampling

The [accepted beta/Gamma-sampling contract](docs/decisions/financial/core-numerics/beta-gamma-sampling.md#accepted-decision-checked-beta-and-gamma-sampling)
retains one Result beta sampler with finite positive shapes, a caller RNG and
private Gamma machinery. Validate both shapes before draws, every uniform as
finite in[0,1) and every requested standard normal as finite. Exact Beta(1,1)
uses one checked uniform. Statistical distribution/independence are RNG-owner
assumptions. Actual uniform0 has explicit log0 provenance, with no floor/redraw:
one represented-zero Gamma yields an endpoint, two such paths error.

Keep Marsaglia–Tsang proposals with stable c=(1/sqrt(d))/3, consistent FMA base/
log handling, the strict squeeze and same-law cancellation-aware threshold.
The small-t remainder uses the specified64-term compensated profile; preserve
scaled final information and truthful failures without a cube floor or silent
numerical rejection. Each Gamma has100,000 proposals and shape augmentation one
extra uniform; nonshortcut beta has at most200,000 normal and200,002 uniform
trait calls, not a bound on RNG-internal work. Preserve declared draw order and
advanced RNG state on failure, without retry/reseed/rollback or old-seed promises.

Keep sampled log-Gamma scale/local/boost components until the joint difference;
do not round away consequential offsets or overflowing-but-cancelling boost
quotients. Restore the smaller logistic tail and complement once. Permit genuine
final binary64 endpoints, but remove raw Gamma sums, extra Bernoulli replacement,
clipping, arbitrary shape/endpoint approximations and mean/normal fallbacks.
Unresolved required arithmetic is a computation failure, not a fabricated draw.

Require finite beta in[0,1] and distinct parameter versus RNG/numerical/convergence
errors. Preserve Rust-only generic sampling and private Gamma scope, reconciling
actual BetaRecovery finite derived-shape construction/restore, seeded hosts and
error/docs behavior. General RNG/stream/normal transforms and financial recovery/
correlation/calibration acceptance remain separately owned. Fixed budgets and
bounded reference calculations certify neither exact distribution/tails nor
portable bits or all-shape success. Runtime/caller and independent numerical/
statistical/financial qualification remain pending.
[Clause map M83–M86](docs/decisions/INVARIANT_MAP.md#1-money-and-model-numerics).

### 1.22 Checked chi-squared log-tail inversion

The [accepted chi-squared contract](docs/decisions/financial/core-numerics/chi-squared-inversion.md#accepted-decision-checked-chi-squared-log-tail-inversion)
retains one scalar Result central-law quantile with finite p in[0,1] and finite
df>0. Validate df before exact p0/+0 and p1/+infinity identities. Interior
evaluation has fixed df<=1e8 and must return positive finite q or a computation
failure; do not turn underflowed shape/quantile into interior0 or claim nearest
subnormal rounding. df is a real shape input, not an inferred observation count.

Select private checked logarithmic Gamma regions with at most100,000 updates
and disclosed1e-15 stopping proxies. Preserve virtual positive half arguments,
scaled small-shape correction and requested complements. The fixed16-term
Gamma1p, five-term Stirling and64-term log1pmx profiles retain consequential
normalization/prefactor information. Remove near-zero cutoffs, ordinary-mass
underflow shortcuts, clipped log masses/residuals and swallowed dependency errors.
Bounds/finite states are not global true-error or convergence certificates.

Use the finite log(eta)/log(U) bracket with U=df+64sqrt(df)+128, checked endpoint
signs and explicit shared Brent controls1e-13/256. The analytic Gamma-MGF bound
motivates the real bracket; it does not certify floating evaluator signs. Recheck
the requested smaller-tail log residual at the actual restored positive q and
require finite absolute residual<=1e-8. Solver/latent-root success alone is
insufficient. Refuse unresolved or quantization-limited results without relaxing
the gate, returning zero, substituting a mean/normal or adding another inverse.

Distinguish input/interior-domain errors from numerical/work/bracket/solve/output
failures. Preserve Rust-only core exposure and actual Student-t positive mixing,
open-uniform/error propagation and separately owned nu/scale/RNG/correlation
policy. This central law does not supply an exact noncentral CIR transition.
Runtime/caller reconciliation and independent numerical/statistical/financial
qualification remain pending.
[Clause map M87–M90](docs/decisions/INVARIANT_MAP.md#1-money-and-model-numerics).

### 1.23 Strict correlated Bernoulli laws

The [accepted correlated-Bernoulli contract](docs/decisions/financial/core-numerics/correlated-bernoulli.md#accepted-decision-strict-correlated-bernoulli-laws)
retains one immutable private two-event law. Require finite marginals in[0,1]
and an explicit nullable correlation: Some finite feasible rho iff both are
interior, None iff a marginal is exactly deterministic. Remove near-degeneracy
thresholds, rho0 sentinels and automatic projection. Exact deterministic
marginals still define their unique joint law while Pearson is undefined.

Enforce Fréchet geometry for actual represented inputs, preserving products,
affine boundaries and consequential combined terms. Rounded correlation bounds
are numerical views, not a constructor oracle; unresolved feasibility fails
computation rather than relaxing tolerance. Complete finite cells satisfy the
specified8*EPS mass/marginal gates without clipping or renormalization. These
gates do not certify exact reconstruction, realized rho or per-cell/tail accuracy.
Keep underlying structural-zero and nonzero/factored information for queries.

Remove requested/effective duplication and use canonical get_* and sample names.
Conditional queries return Result/Option, None only for exact zero conditioning
probability, with valid ratios computed before destructive rounding. Sampling
accepts finite u in[0,1), follows order11/10/01/00 and preserved latent cuts,
and distinguishes structural zeros from positive intervals erased in rounded
views. Unresolved decisions error rather than select impossible outcomes.

Preserve atomic construction, immutable queries, required None/null and existing
host placement/data shapes with centralized input/computation errors and no old
aliases or new state framework. No current Rust financial consumer delegates to
this core type; tree coupling, multivariate compatibility and financial model
acceptance stay separate. Runtime/host reconciliation and independent numerical/
statistical/financial qualification remain pending.
[Clause map M91–M94](docs/decisions/INVARIANT_MAP.md#1-money-and-model-numerics).

### 1.24 Checked random draws, bridge and Poisson counts

The [accepted RNG/bridge/Poisson contract](docs/decisions/financial/core-numerics/pseudorandom-and-bridge.md#accepted-decision-checked-random-draws-bridge-and-poisson-counts)
selects one explicit PCG seed/stream initialization and top53-bit[0,1) uniform
grid, with the existing library's odd-increment transition. Streams, seeds and
period do not certify independence/non-overlap. Checked reflected Box–Muller has
no floors/redraws, and normal(mean,std_dev) is fallible with finite parameters,
nonnegative scale, a no-draw std0 identity and fused final restoration. Preserve
declared standard-spare/clone and error-consumption behavior without rollback,
compatibility aliases or a new public replay/state framework.

Bridge construction is fallible with inclusive100,000 steps, private immutable
topology and explicit finite full grids/shocks. Keep terminal-first depth-first
index order, safe conditional mean/direct SD/complete level arithmetic and
preflight-output preservation versus computed-error discard. Rounded levels do
not preserve every mathematical increment; actual standardized consumers need
the same adjacent knots and retained information or truthful failure. Generic
bridge ordering is no QMC-rate, variance-priority or barrier-monitoring certificate.

Select one u64 Poisson quantile with strict finite lambda>=0,u in[0,1), complete
validation before zero identities and count ceiling2^53-1. Reuse checked bounded
log-Gamma predicates, one capped integer bracket/bisection with at most107 probes,
and both neighboring inverse inequalities. Remove sequential/dependency branches,
count saturation, interpolated/normal approximations and hidden retries. Kernel/
boundary/range uncertainty fails explicitly; the ceiling does not ensure every
mathematical count is numerically resolvable.

Keep the generic core APIs Rust-only. Propagate new fallibility through actual
sampling/Philox/model/statement/covenant/toggle/Sobol/count-allocation chains,
without sentinels, replacements or unused antithetic work. Models retain stream
assignment, clock/intensity conversions, QMC/monitoring/discretization and risk
acceptance; record20 retains TimeGrid choices. Runtime/caller reconciliation and
independent numerical/statistical/financial qualification remain pending.
[Clause map M95–M98](docs/decisions/INVARIANT_MAP.md#1-money-and-model-numerics).

### 1.25 Finite-period Sobol points and prefix hash scrambling

The [accepted Sobol contract](docs/decisions/financial/core-numerics/sobol-sequences.md#accepted-decision-finite-period-sobol-points-and-prefix-hash-scrambling)
selects fixed dimensions 1..40, 32-bit D6 directions and natural binary index
order. Require one checked constructor with explicit None/Some seed mode,
including Some(0), and the specified modular output-prefix hash permutation.
Keep exact interior midpoints and checked canonical inverse normals, with finite
grid/tail limits. Recorded table/checksum/golden evidence is distinct from fresh
upstream authentication and production execution.

The prefix bijection permutes same-shape dyadic boxes and occupancy counts,
preserving applicable finite net properties. It does not establish independent
uniform node permutations, continuous-integral unbiasedness, Owen variance rates
or valid error bars. Retain point zero; no clamp, burn-in, thinning, automatic
power-of-two rounding or seed-label independence claim.

Publish the u64 period 2^32 and enforce exhaustion at next-index P. Checked whole-
point fills and relative skip preflight full shape/period/resource admission;
empty fills and skip zero are identities, including exhaustion. Reset explicitly
replays configuration; clone copies state. Use a bounded local cursor/cache and
commit generator state only after fill success; discard unspecified output after
a computed failure. Preserve typed errors without modulo wrap, sentinel seeds,
partial-point/scalar compatibility, fabricated draws or universal replay claims.

Remove the obsolete generic model Sobol stream adapter, preserve the dedicated
whole-point driver/private precomputed stream and propagate checked budgets and
failures throughout. Generic APIs remain Rust-only; actual blocks, seed laws,
replicate design/uncertainty, coordinate order, bridges and financial acceptance
stay with owners. Runtime/caller reconciliation and independent numerical/
statistical/financial qualification remain pending.
[Clause map M99–M102](docs/decisions/INVARIANT_MAP.md#1-money-and-model-numerics).

### 1.26 Checked fractional increment covariance

The [accepted fractional-covariance contract](docs/decisions/financial/core-numerics/fractional-covariance.md#accepted-decision-checked-fractional-increment-covariance)
selects standard unit-normalized fBM, variance delta^(2H), one private validated
HurstExponent with finite 0<H<1/get_value and faithful constructor-validated {h}
state/schema. Both existing helpers take that type and return checked scalar/
DMatrix results. Admit complete finite nonnegative forward scalar intervals and
strict matrix grids, with validated zero-width and empty/singleton identities.
No raw-H bypass, endpoint clipping/default, minimum gap or false differentiability.

Use stationary endpoint distances rather than level covariance subtraction,
canonical whole-interval ordering, direct Brownian overlap/self-variance and the
stable sign-preserving touching identity. The general profile preserves log/
exponent geometry, uses ordered Neumaier normalized-power sums and requires
|S|>8*EPSILON*B before complete scaled restoration. This fixed operational
refusal gate is not a power-function error bound; mathematically valid inputs
may fail. Preserve finite signed/subnormal results, positive true variances and
known covariance signs, with explicit cancellation/range failures rather than
unresolved zeros, infinities, epsilon snapping or fabricated independence.

Check dense size/layout and allocation, evaluate one triangle and mirror exactly,
and publish a complete finite positive-diagonal matrix or a contextual error.
No rounded-PSD/factorization certificate, O(n^3) admission gate, jitter, clipping,
normalization or automatic repair is added. Accepted 07 factor/solve ownership
remains distinct from covariance construction.

Reconcile actual fBM typed-H/getter/error, factor/Schur and fallible-generation
chains and existing embedded Hurst wire bounds without new generic host bindings
or compatibility aliases. Models retain clocks, units, amplitudes, Volterra versus
stationary kernels, samplers/defaults and financial acceptance. Runtime/caller/
schema reconciliation and independent numerical/statistical/financial qualification
remain pending.
[Clause map M103–M106](docs/decisions/INVARIANT_MAP.md#1-money-and-model-numerics).

### 1.27 Checked simulation grids and exact event knots

The [accepted time-grid contract](docs/decisions/financial/core-numerics/time-grids.md#accepted-decision-checked-simulation-grids-and-exact-event-knots)
selects one private immutable finite zero-origin model-year knot invariant,
normalized +0 and derived adjacent positive dts/horizon, without spacing floors
or nominal-step replacement. Admit [0] and uniform(0,0), with inconsistent
zero/positive horizon/count pairs rejected. Pin positive uniform endpoints and
use the specified two-stage binary64 T*(i/N) profile; generated collapse errors
rather than reducing counts or nudging times.

Required-time construction takes explicit N, validates all events in[0,T] and
the canonical base before exact-coordinate union. No tolerance merging, dropped
events or terminal snapping. Preserve owner event identities/order/multiplicity
separately, with one checked exact knot lookup and checked indexed get_* access.
Canonical is_uniform recognition does not certify equal rounded dts, small
relative perturbation or a uniform-only scheme. Check exact-index/candidate/
capacity arithmetic and fallible allocation without importing the bridge cap.

Remove generic date/exercise snapping helpers without compatibility copies.
Owners declare clocks/context/date bounds, any permitted timing displacement,
collisions and eligibility; future dates at model time zero are not automatically
expired. Build one actual grid for simulation, precomputed kernels, bridges,
payoffs and results; reconcile actual getter/error/density-count consumers and
uniform-only numerical admission together. Core stays Rust-only with existing
model TimeGridSpec wire shapes and narrower owner admission. No new core serde,
host class, fallback grid or implicit product timing policy.

Runtime/caller/host reconciliation and independent numerical/event/geometry/
financial qualification remain pending; generic validation does not certify
monitoring, exercise, scheme suitability or price/Greek convergence.
[Clause map M107–M110](docs/decisions/INVARIANT_MAP.md#1-money-and-model-numerics).

### 1.28 Checked piecewise coefficients and exponential integration

The [accepted piecewise-coefficient contract](docs/decisions/financial/core-numerics/piecewise-coefficients.md#accepted-decision-checked-piecewise-coefficients-and-exponential-integration)
selects one private immutable nonnegative right-continuous coefficient schedule,
normalized zero, strict knots and constructor-validated existing wire state.
Use finite nonnegative checked get_value queries, flat finite final extrapolation,
read-only get_times/get_values and one short integrate method without aliases.
Correct left-continuity labels without changing the implemented partition law.

The mathematical integral is sigma(u)^2 exp(-2*kappa*(anchor-u)) over a validated
0<=start<=end<=anchor with finite signed scalar kappa. Only exact empty intervals
or zero active coefficients produce zero. Remove duration/near-kappa deadbands;
use exact zero-kappa integration and the sign-stable maximum-weight endpoint/
expm1 factor for nonzero kappa, retaining complete products and scales rather
than requiring squares/exponentials/segment contributions to fit separately.

Use a private two-pass source-order compensated positive-total accumulation and
restore only the completed result. Individually underflowing pieces can sum to
representable variance; preserve positive subnormal totals and reject positive
final underflow/overflow or unresolved arithmetic. No NaN/infinity, floor, clamp,
quadrature/alternate-kernel fallback or manufactured deterministic volatility.
Bound partition traversal and retain contextual input versus computation errors.
Analytic integration is not exact floating evaluation or a universal error bound.

Propagate checked queries/integrals through OU/Hull–White/tree/exact-stepper and
existing model host APIs, removing Err-to-NaN-to-zero paths. Generic core remains
Rust-only with existing embedded curve wire shape and faithful schema. Models
retain clocks, coefficient units, parameter positivity, schedule precedence,
additional variance transformations, calibration and financial error budgets.
Runtime/caller/host/schema reconciliation and independent numerical/financial
qualification remain pending.
[Clause map M111–M114](docs/decisions/INVARIANT_MAP.md#1-money-and-model-numerics).

### 1.29 Checked scalar expressions, missing values and decimal quantization

The [accepted scalar-expression contract](docs/decisions/financial/core-numerics/expression-evaluation.md#accepted-decision-checked-scalar-expressions-missing-values-and-decimal-quantization)
selects one immutable iterative structural-only eager DAG with finite literals,
exact scalar arities/static controls, depth0..511 and an inclusive100,000 visited
AST-occurrence budget before deduplication. Keep ordered operands, no algebraic
rewrite, explicit eval row count and exact context/column lengths. Bound compiler/
arena/layout work and qualify raw-AST ingress/clone/serde/cleanup separately.

Runtime NaN alone is permitted missing data; infinities in any supplied input
error. Missing-first numeric operations, exact finite comparisons and explicit
strong three-valued logic do not disguise invalid finite calculations as missing.
Those failures error even in an unselected eager conditional/boolean branch.
Ordinary binary64 underflow remains numeric, not financial-zero certification.
Preflight all names/shapes/static controls even at zero rows, then execute no
numeric rows for an empty result. Return complete finite-or-missing output or an
error without partial output, input mutation or cross-call cached data.

Select exact represented-binary64 decimal quantization with half-away integer
target and one nearest-even binary64 restoration, using existing bounded rational
primitives and full-i32 digit tails. Reject nonfinite final restoration, not merely
an exact target slightly above MAX. Preserve zero signs and no scale-overflow
fallback or Decimal/string reinterpretation. Replace clamp_or_nan with a checked
finite ordered-bound clamp, keeping explicit evaluator missing-first behavior.

Reconcile composite one-row/ingress/state use and all statements/bond-future/
waterfall shared-helper errors. Statements round/clamp reject evaluated infinite
operands, then propagate permitted NaN, otherwise call checked finite kernels;
other statements period/control/lazy/comparison policies remain separately owned.
Core stays Rust-only; shared AST/host docs do not imply scalar-DAG execution.
Money quantization, financial units/formulas/rounding points and price/risk budgets
remain with owners. Runtime/caller/host and independent language/numerical/
financial qualification remain pending.
[Clause map M115–M118](docs/decisions/INVARIANT_MAP.md#1-money-and-model-numerics).

### 1.30 Explicit rounding config, local tolerances and producer metadata

The [accepted configuration/metadata contract](docs/decisions/financial/core-numerics/rounding-and-config.md#accepted-decision-explicit-rounding-config-local-tolerances-and-producer-metadata)
inherits accepted monetary explicit-mode/scale and absent-ingest precision rules.
Keep coherent guarded optional-mode config and immutable version-2 snapshots;
output scales remain presentation controls. Remove shared Money/display-derived
zero classification, ToleranceConfig/generic_epsilon and universal ZERO_TOLERANCE
authority. Preserve actual nonzero cash until selected financial rounding, while
owners select justified local comparisons and retain protected controls rather
than blindly substituting exact zero or copied epsilons.

Opaque config extension keys retain exact declared grammar/duplicate rejection;
content/version/merge authority stays with each consuming domain. Bind embedded
registry builders once and retain immutable cached Ok/returned Err. Remove the
obsolete generic override path/per-load validators, without reintroducing retired
rating defaults or certifying bundled assumptions or arbitrary Clone isolation.

Result metadata records producer facts, not requested-policy enforcement. Applied
rounding and parallel execution are optional, with absence unrecorded rather than
no rounding/serial. Fresh results_meta() records only known compiled profile/core
package version. Remove request overlays/implicit OR and unverified applied FX
inference; producers own actual scoped claims and same-scope conflicts error.
Keep strict private validated snapshots/state, existing host types and immutable
replay inputs under accepted provenance authority, without a new metadata/archive
framework. No library label or timestamp omission certifies complete replay.

Reconcile all zero/epsilon consumers, fixed registry calls, actual policy use,
valuation stamping and host/schema/parity changes together. Numerical controls,
materiality, financial data/merge policy and price/replay acceptance remain with
owners. Runtime and independent numerical/financial/provenance qualification
remain pending.
[Clause map M119–M122](docs/decisions/INVARIANT_MAP.md#1-money-and-model-numerics).

### 1.31 Strict numeric wire and canonical content identity

The [accepted wire/identity contract](docs/decisions/financial/core-numerics/numeric-wire-and-canonical.md#accepted-decision-strict-numeric-wire-and-canonical-content-identity)
requires finite named f64 domains, symmetric annotated u32/i32 integer-token
counts, accepted exact normalized Decimal strings, strict supported Gregorian
date strings and exact field-selected nonfinite tokens. Validate direct finite
variants and numeric generic alternatives at every boundary. Do not infer
financial result availability, units or validity from a wire wrapper. Schema,
rounding snapshot, canonical, extension and artifact version axes stay separate.

Canonical identity preserves integer-versus-float representation, signed floating
zero, strings, array order, omitted/null distinctions and metadata. Explicitly
sort decoded object keys by UTF-8 bytes; freeze the admitted numeric/compact byte
profile, including exact f32 widening. Reject floating map keys, duplicate final
names/collisions, reserved RawValue/Number pseudo-structs and unsupported numeric
feature profiles. Original raw/host ingress rejects integer/duplicate/nonfinite
loss before generic Value materialization; hashing cannot recover lost history.

Use the single active c2 profile and SHA-256 of `finstack-canon/` + `c2` + NUL +
canonical bytes, retaining the version separately from the digest label. Serialize
the original frozen deterministic value once per call, cap emitted container
depth at96 and check growth without untrusted-length allocation or partial hashes.
Retain owner size/work limits and truthful input/computation failures. No c1
fallback, migration, generic string/economic normalization, authentication,
universal sandbox/allocator recovery or complete-replay guarantee is implied.

Reconcile actual host text/native-number/negative-zero/exact-integer paths,
sentinel revival, date/count/schema/ledger consumers and complete typed cache-key
payloads rather than digest-plus-Debug suffixes. Existing Rust authority remains
canonical; no duplicate host writer. Independent exact bytes/digests, feature,
boundary, depth and host qualification remain pending alongside runtime work.
[Clause map M123–M126](docs/decisions/INVARIANT_MAP.md#1-money-and-model-numerics).

### 1.32 Bounded contract loading, validation and diagnostics

The [accepted loading/diagnostic contract](docs/decisions/financial/core-numerics/contracts-and-diagnostics.md#accepted-decision-bounded-contract-loading-validation-and-diagnostics)
requires complete-input lossless/byte/depth admission, scoped exact version and
closed shape checks, then actual owner semantic/build validation before usable
publication. Keep explicit operational limits and depth0..96 without nested
exceptions; actual owners enforce all supplied artifact/position cardinalities
before full allocation/build. Structure-only decode is not financial acceptance.

Guard report state/caps/restoration and persist fatal status independently of
first-N retained diagnostics. Cap-zero warnings stay nonfatal and errors fatal;
merges preserve child fatal/truncated facts even without retained details. Keep
truthful codes/context, escaped JSON Pointers and complete same host reports.
Retain explicit generic predicates, accepted exact Decimal Money thresholds,
owner recovery restrictions and consistent exact registry admission/building.

Guard immutable table state and actual exports, preserving literal column order,
nullable missing versus present nonfinite values and exact i64 native BigInt.
Suggestions remain deterministic bounded advisory hints; owned error meanings
follow explicit classification and bounded cause rendering. Trace capacity is
finite/fixed, truncation monotone and amounts finite diagnostic projections,
without exact-cash, generic PV-identity, complete-evidence or replay claims.

Keep ordered outputs/lowest-index callback errors with qualified native/WASM
execution histories. Remove blanket poisoned-lock continuation: every current
calendar/FX lock fails closed with Computation errors and no substituted state.
Reconcile actual loaders, cardinalities, calibration merges, registries, exports,
trace/error/lock consumers and host/schema/parity state together. Runtime and
independent numerical/financial/state/scheduling qualification remain pending.
[Clause map M127–M130](docs/decisions/INVARIANT_MAP.md#1-money-and-model-numerics).

### 1.33 Canonical schemas, compact views and bounded publication

The [accepted schema/publication contract](docs/decisions/financial/core-numerics/schema-publication.md#accepted-decision-canonical-schemas-compact-views-and-bounded-publication)
keeps explicit draft2020-12 Deserialize wire-admission schemas under Rust
ownership. Preserve original local definitions, literals, applicators and URI
scopes in one complete immutable offline corpus; remove generic externalization,
pruning, union collapse and their packager/sharing machinery. Require exact
metadata, independent version axes, duplicate-free resources and unambiguous
selectors, with explicit deny-all retrieval fallback. Build and validate the
complete working corpus locally before caching, without recursive initialization.

Publish exact owner-qualified examples without generic float rounding or signed-
zero changes; validate final examples against canonical schemas and actual typed
owner rules. One explicitly ordered pretty renderer supplies promised bytes and
index facts, separately from c2 content hashing. Preflight complete owned output
sets, canonical paths and an existing resolved base; reject links/special files
inside ownership and propagate I/O errors. Stage complete files, replace each
atomically, delete only inventoried owned stales and publish the index last.
Partial commit failures remain failures requiring check; no full-set transaction,
race-proof filesystem or crash-durability guarantee is implied.

Replace llm/project_llm/profile knobs and lossy provider/handle transforms with
canonical and compact/compact_schema. The labeled compact-v1 view shortens only
known schema-location descriptions, preserving all assertions, IDs, references,
defaults, examples and literal data. Preserve the canonical ID for resolution,
reject reserved-label collisions and never register/substitute the view as a
canonical resource. No provider compatibility or total-size guarantee is claimed.

Canonical validation uses original compiled context and accepted bounded reports
with validity independent of retained detail. Remove lifted-branch heuristics and
parallel Failure-vector authority. Known-format checks, unsupported annotations,
schema shape and typed financial admission remain distinct. Reconcile actual
registries/generators/fixtures/hosts/features/schema/parity and approved outputs;
all26 methodology records are accepted, while runtime/publication and independent
schema/financial qualification remain pending.
[Clause map M131–M134](docs/decisions/INVARIANT_MAP.md#1-money-and-model-numerics).

### 1.34 Contractual cashflow periods, calendars and day-count context

The [accepted cashflow-period contract](docs/decisions/financial/cashflows/01-schedule-periods.md#accepted-decision-contractual-periods-calendars-and-day-count-context)
requires explicit guarded conventions and the shared core Natural/EOM/QuarterlyIMM
selector. Remove broad preset factories, generic CDS overrides and prior-roll
start substitution. Preserve original anchor/direction/phase/nominal indices and
long-stub boundaries; classify stubs from that provenance, not clamped local
endpoint stepping. Core owns raw grids and kernels; products own their regimes.

Keep raw nominal and actual accrual dates distinct. Resolve calendars for their
actual schedule/fixing roles, preserve selected shared adjusted anchors and reject
unrequested accrual gaps/overlaps or collapse. Payment adjustment precedes its
nonnegative business-day lag; distinct periods may share dates without merging.
Reset is absent or derived from actual accrual start with explicit fixing-calendar
business-day lag; zero preserves its anchor, without extra roll or provider fallback.
Retain supported reset<=payment and separate observation/publication ownership.

Compose ICMA from complete actual nominal references, including long/adjusted/IMM
slices; preserve full actual ACT365L period and explicit termination facts. Remove
factor-only prorating/inferred denominator/context replacement. Return immutable
fully computed periods with context-owned partial queries and literal valid zero
fractions. The single-period helper requires exactlyone canonical generated coupon.
Internal calculation splits preserve actual coupon/event/rounding identity;
composite cash omits untruthful single-slice metadata and retains owner evidence.
The core015 clarification identifies metadata endpoints as actual calculation
dates, with raw nominal references separate and no formula/single-context change.

Use checked date/count/resource/accumulation boundaries and complete requested
period results or errors, keeping deliberate couponless programs separately owned.
Reconcile actual compiler/emitter/replay/accrual/product/host/schema users without
compatibility paths. Program cuts, coupon amounts, PIK, index observation and
reporting choices retain their owners; runtime and independent qualification remain
pending.
[Clause map M135–M138](docs/decisions/INVARIANT_MAP.md#1-money-and-model-numerics).

### 1.35 Coupon and payment program compilation

The [accepted program-compilation contract](docs/decisions/financial/cashflows/02-program-compilation.md#accepted-decision-coupon-and-payment-program-compilation)
selects independent atomic raw coupon/payment windows: every supplied boundary
regenerates its own admitted period grid, even for equal settings. Preserve each
window's original nominal phase/context and validate its anchors under001; no
implicit global-grid continuation, cut coalescing or date repair. Require exact
coupon coverage, or no active coupon/payment windows after validated permitted
empty-edge omission. Payment windows form a globally valid disjoint/touching or
properly nested family; reject equal/crossing windows before unique innermost
selection, with no insertion-order priority.

Configuration is fallible and atomic, without sticky error history; symbolic
horizons resolve at build. Validate all supplied payloads before omission/selection.
Fixed steps sample each raw period start, floating margins are effective FROM
window dates, and payment_splits_until applies the supplied split UNTIL a raw
boundary. No implicit Cash tail: remaining coupons use their explicit base split.
Require explicit coupon settlement and principal exchange, and owner as_of for
active floating through one build path without curve-base cutoff inference.

Preserve actual calendar/market causes and exact Money through shared build
state, event and redemption handoffs. Fee/principal processing dates do not cut
coupon grids automatically; period context remains authoritative over global
reporting labels. Check aggregate resources and publish complete local results
or errors without claiming provider-side rollback or live-input reproducibility.
Reconcile existing fluent/spec/JSON/product/host/schema/parity paths without
legacy aliases or parallel compilers. Principal, coupon, observation and lifecycle
formulas retain their owners; runtime and independent qualification remain pending.
[Clause map M139–M142](docs/decisions/INVARIANT_MAP.md#1-money-and-model-numerics).

### 1.36 Dated opening principal and atomic cashflow phases

The [accepted principal/event contract](docs/decisions/financial/cashflows/03-principal-and-emission-order.md#accepted-decision-dated-opening-principal-signed-cash-and-atomic-event-phases)
requires exact nonnegative opening stock and an explicit opening_date before all
modeled economic activity and required actual accrual/history intervals. Stock is
known immediately before opening-date movements; it is not issue-closing stock.
Preserve complete chronological pre-issue/issue history without inferred origins
or live-stock fallback. Closing stock at a date applies on [date,next_date), so
end-date movements do not change the coupon just accrued.

Explicit events require literal signed settlement cash, signed principal delta,
economic date and a core015 principal-capable kind. Retain zero/duplicate rows;
PIK/write-downs require zero cash and matching event/economic dates. No par/sign
inference, cash netting or kind-based balance guesses. Apply all explicit deltas
at one economic date as an exact atomic batch, independent of insertion/output
order; validate the representable nonnegative stock after each stock-changing
phase, without cross-phase cancellation or intermediate-sum false overflow.

The disclosed phases are ending coupons, admitted generated PIK, explicit principal
batch, scheduled amortization, periodic fees, terminal redemption and closing
history. Accepted004 removes the obsolete cached installment-update slot. PIK timing here is actual accrual end and needs
product qualification; eligibility and amount laws retain their owners. Select
one terminal coupon's actual-economic/payment pair, or raw maturity for a deliberate
couponless program. Actual terminal date bounds principal economic activity;
settlement remains independent. Explicit InitialAndFinal settles -initial at issue
with zero delta and +remaining at terminal payment with delta -remaining. None
suppresses automatic exchanges/closure and can retain stock; never count initial
stock twice or fabricate a residual write-off.

Use exact Money/Decimal state and checked atomic complete publication. Reconcile
guarded existing construction/restoration, complete opening/economic metadata,
replay, product/export consumers and host/schema/parity paths without compatibility
inference. Amortization, coupon, fee and lifecycle formulas retain their owners;
runtime and independent qualification remain pending.
[Clause map M143–M146](docs/decisions/INVARIANT_MAP.md#1-money-and-model-numerics).

### 1.37 Explicit amortization laws and live-balance installments

The [accepted amortization contract](docs/decisions/financial/cashflows/04-amortization.md#accepted-decision-explicit-amortization-laws-live-balance-installments-and-monetary-boundaries)
requires an explicit rule including None, exact same-currency amounts and Decimal
fractions in [0,1] under the member fraction rather than pct. Original B0 is the
declared pre-movement opening stock; live B follows same-date admitted PIK and
explicit principal batches. Generated cadence is distinct actual coupon accrual
ends, with one unique payment mapping per economic date and no fee/slice/cash-date
cadence inference. Dated activity lies in [issue,actual terminal]; LinearBetween
requires an aligned end and nonempty (start,end]. Empty dated programs and exact
zero fractions remain valid; grid rules still require their admitted cadence.

LinearTo fixes I=(B0-F)/n, pays min(I,max(B-F,0)) before the final date, then requires
B>=F and pays exact B-F to retain F after amortization, before separate redemption.
StepRemaining requires a live funded target and pays exact B-target. Original
fraction pays min(B0*fraction,B); remaining fraction pays B*fraction, without an
implicit final sweep. LinearBetween pays B/k at each due boundary, including
today's stock and remaining-date count; its end sweeps B exactly. Remove cached
installments, funding-kind detectors and post-payment updates. CustomPrincipal
sums all nonnegative same-date amounts exactly and rejects overpayment. None
suppresses scheduled reduction only; later activity/exchanges retain003 authority.

Emit exact par cash +P and delta -P with the actual economic/payment dates; omit
only exact-zero generated rows. Rule-specific caps are mathematical definitions,
not permission for a common emitter to repair invalid payments. Preserve exact
Money/Decimal state and accepted bounded transforms, truthful range/underflow
errors and complete atomic admission/publication. Select no implicit financial
payment rounding; contractual quantization requires its actual owner schedule.
Reconcile existing construction/restoration, original/live bases, term-loan/bond
adapters, economic history and host/schema/parity paths without defaults, float
monetary detours or compatibility inference. Runtime and independent qualification
remain pending.
[Clause map M147–M150](docs/decisions/INVARIANT_MAP.md#1-money-and-model-numerics).

### 1.38 Parent-coupon accrual, exact splits and capitalization

The [accepted coupon amount/allocation contract](docs/decisions/financial/cashflows/05-coupon-cash-and-pik.md#accepted-decision-parent-coupon-accrual-exact-splits-and-capitalization)
uses each original contractual coupon as its amount/allocation unit. Retain covered
half-open economic history, original period/context and signed internal balance/
rate contributions; internal nominal or balance cuts do not force new settlement
or principal events. For fixed/term allocation, admit original-start cumulative
accrual prefixes once to Decimal and subtract them exactly. Weights must be
nonnegative, context-valid and telescope at that admitted authority; do not
re-anchor nonadditive day counts at balance changes or redefine001's direct query.
OneOne assigns its unit to the first nonempty interval. Preserve legitimate zero
weights without manufacturing missing index/rate values or projection authority.

Restore total Money once from the complete exact admitted parent contribution
sum using the accepted bounded profile, not per-piece Money or binary64 amounts.
Split takes one exact cash_fraction in [0,1], with derived exact PIK complement.
Allocate cash once, derive PIK by exact residual and require exact conservation,
truthful range errors and both-share nonzero-underflow guards. Remove redundant
fraction inputs, competing tolerances and automatic normalization. Generated PIK
must be nonnegative per parent; signed internal terms do not become independent
admitted events or offset another invalid coupon's eligibility. Explicit003 signed
events retain their own literal effects.

Emit each enabled parent cash/PIK component including exact zero; PIK is cash0
with explicit delta and date=principal_date=actual whole-parent end. Cash settles
independently; original stub/observation identity is preserved. Retain actual
parent prefixes/context/contributions/provenance for required queries and omit
false optional single-slice rates, references or resets. No implicit financial
rounding, rate premium, floor, election or timing model. Reconcile strict nested
coupon specs, canonical exact admission, complete atomic work and existing Rust/
compiled settlement/replay/product/statement/host/schema/parity consumers. Rates,
observations, state, entitlement and reporting retain their owners; runtime and
independent qualification remain pending.
[Clause map M151–M154](docs/decisions/INVARIANT_MAP.md#1-money-and-model-numerics).

### 1.39 Resolved term observations and exact rate controls

The [accepted term-rate contract](docs/decisions/financial/cashflows/06-floating-rate-projection.md#accepted-decision-resolved-term-observations-exact-rate-controls-and-strict-failures)
inherits index-specific factor projection and exact observed identity. Require
resolved IndexId/currency, fixing/effective/maturity dates, positive index alpha
and qualified publication/source-version context. Use one observation per parent,
explicit method/reset/calendar authority and conditional projection association;
no dummy CurveId for a fixing-only request. Remove reset-point seed queries,
coupon-basis rate rescaling, common reset-frequency/tenor hints, approximate
matching, implicit convention bundles and curve-base/date-only eligibility rules.

A required published observation uses its exact configured fixing without an
unused curve. Explicitly unpublished observations may project through their
selected source under this operation; unknown eligibility and missing required
published values error. No generic failure-triggered rate substitutes. Ordinary
term raw quotes require finite positive simple gross under their index alpha,
including a post-model-admission check; all-in coupon rates have their separately
selected cash/PIK domain. Preserve actual source and calculation failure causes.

Use exact Decimal contractual spread/bounds, explicit strictly positive gearing
and explicit spread-placement law. Apply index bounds, complete gearing/spread
expression and all-in bounds in checked exact transient arithmetic, then restore
one bounded Decimal contractual annual rate. Do not restore an internal clamp,
round control operands independently or let a cap rescue invalid raw admission.
Raw-index and final-rate admission can fail even if a downstream coupon could
otherwise fit.005 consumes admitted R directly before its separate parent-Money
stage. Fixed RateSpec quotes remain Decimal; one canonical get_rate resolver
replaces divergent builder/direct paths and any requested model view is explicit.

Retain observed/projected role and actual index/date/source provenance atomically;
only real projections populate projected fixing metadata. Reconcile guarded
existing specs/hosts/state, real product reset grids, factor/fixing users and
schemas/parity without fallback aliases or duplicate rate logic. Overnight,
replay, materialization and product models retain their owners; runtime and
independent qualification remain pending.
[Clause map M155–M158](docs/decisions/INVARIANT_MAP.md#1-money-and-model-numerics).

### 1.40 Overnight blocks, cumulative growth and parent allocation

The [accepted overnight contract](docs/decisions/financial/cashflows/07-overnight-compounding.md#accepted-decision-resolved-overnight-blocks-cumulative-growth-and-parent-allocation)
requires a positive parent interval, explicit method, one observation calendar,
Act360/Act365F index basis and Daily/Period index-bound placement. Actual resolved
index value periods/quotes remain distinct from coupon weights; leading closed-
day carry uses the preceding genuine fixing. Lookback moves observations only.
Observation shift pairs full business blocks and prorates shifted weight by the
actual overlap/full original block length, retaining exact fractional geometry.
Cutoff requires k<block_count and freezes quotes without merging factor blocks.
Control counts lie in0..=i32::MAX with checked date/reach arithmetic and at most
100,000 compiled observation blocks per parent.

Use checked source-neutral raw/base model growth, with stable sums/products and
truthful cancellation/lost-increment refusal. Cache each actual observation quote
once within replay; extend its original active carry factor without checkpoint
compounding or rounded factor removal. Empty prefixes have growth0 and unavailable
annualized rates. Required model-to-Decimal growth admission may error despite a
possible large-notional rescue; no silent alternate numerical/financial mode.

Allocate through the fixed parent bridge kappa=A_E/tau_E and exact residual
q=R-g*H_E/tau_E, using one admitted growth/prefix authority. Per-unit contributions
are g*kappa*deltaH+q*deltaA, entering005's complete exact parent amount sum. Constant
principal gives admitted R*A_E; changing principal uses a compound-RATE payoff,
not unpaid-interest-balance accounting. Period bounds constrain the reference
rate and need not cap actual weighted cash. Partial replay must not fabricate a
future final-bound adjustment or confer information not available to the owner.

Keep exact transient bridge/weights, qualified model/admission stages and actual
parent/source provenance. Preserve immutable schedule-bound state, atomic local
updates and truthful external callback/RNG rollback limits. Reconcile existing
Rust-only replay and host specs, source/monetary consumers and actual product
engines without assuming their separate conventions/models are interchangeable.
Runtime and independent qualification remain pending.
[Clause map M159–M162](docs/decisions/INVARIANT_MAP.md#1-money-and-model-numerics).

### 1.41 Parent-bound floating replay, full settlement and timing ownership

The [accepted floating replay contract](docs/decisions/financial/cashflows/08-floating-replay.md#accepted-decision-parent-bound-replay-exact-full-settlement-and-explicit-timing-ownership)
uses immutable001/005 original-parent context and006/007 observation/control
authority, including legitimate zero full accrual. Create every Term/Overnight
state from its compiled owner; cloned owners/states preserve binding, separately
compiled parents cannot exchange states, and every coupon operation validates
binding before shortcuts. Remove duplicate period/economics authority, approximate
context matching and unbound Default.

Capture one exact nonnegative same-currency Money balance at logical actual S
after the applicable003 phases, independently of fixing/observation progress.
Observation can precede capture. The constant-principal continuation profile
requires that balance over the whole parent; changing stock uses003 covered
history through the same005/007 parent authority or errors. Separate terminating
branches retain their actual payoff owners.

Use repeatable nonposting get_settlement with all required qualified observations
and full overnight coverage through E.005 owns the exact full-parent amount,
cash/PIK conservation, nonnegative generated PIK and enabled zero components;
return truthful exact values, explicit noncash pik_delta and parent/source
provenance. Remove settle_index_rate and generic accrued_amount. Native prefixes
cannot establish a partial payoff or borrow an unknown final q. Calculation
does not pay cash, capitalize PIK, change N, advance actual time or retire claims.

Distinguish authorized labelled forecasts/scenarios from an actual contractual
amount already known for early payment. Required principal/rate/information
phases remain owner-qualified; future path storage and approximate model steps
supply no permission. Local state transitions/errors are atomic without rolling
back external callback/provider/RNG effects. Keep guarded state Rust-only and
reconcile actual builder/bond/host exclusions, explicit principal effects and
source errors. Lifecycle, exercise and accrued-interest entitlement retain their
owning records. Runtime and independent complete-path qualification remain open.
[Clause map M163–M166](docs/decisions/INVARIANT_MAP.md#1-money-and-model-numerics).

### 1.42 Explicit fee parents, exact amounts and literal tier selection

The [accepted fee contract](docs/decisions/financial/cashflows/09-fees.md#accepted-decision-explicit-fee-parents-exact-amounts-and-literal-tier-selection)
requires explicit periodic base/quote/frequency/day-count/payment-calendar/BDC/
stub/balance-method inputs. Its selected limited recipe uses a Natural grid,
unadjusted accrual, no EOM and zero payment lag over issue→raw maturity. Preserve
each original fee parent's context, independent grid and declared termination;
coupon cuts do not reset fees, and fee dates do not determine amortization cadence.

Opening uses covered post-phase stock at S; Integrated uses covered stock over
[S,E_fee) and exact differences of admitted original-fee-parent accrual prefixes.
Do not require unused future stock for Opening or substitute live closing stock.
Keep full ICMA/ACT365L context, nonadditive/OneOne consequences and legitimate
zero weights. Undrawn uses exact static same-currency K>=0 and max(K-B,0) per
sampled interval before weighting; this is not a credit-limit admission policy.

Keep signed fractional annual Decimal bp. Evaluate the complete transient exact
base*bp/10000*weight sum and restore Money once, with final overflow/nonzero-zero
underflow errors, without intermediate bounded rate/base/contribution requirements
or float Money detours. Emit one Fee cash row per parent including enabled zero,
with no principal effect, truthful owner provenance and optional slice metadata.
Fixed cash retains literal date/currency/signed Money/zero/duplicate identity,
including pre-issue/post-maturity dates, without implicit FX, BDC, lag or history.

Tier lookup requires literal Decimal utilization/thresholds in[0,1], strict supplied
threshold ordering, whole-input inclusive selection and complete validation.
Return None for no match, distinct from Some(0); product owners select actual
no-fee/missing-coverage behavior. No default balance method, old-name alias,
automatic tier-to-periodic link, dynamic commitment or product fee floor is implied.
Keep fee admission/build publication locally atomic and reconcile existing actual
term-loan/revolver/spec/host/schema/parity callers, including conversion errors.
Runtime and independent complete-path qualification remain open.
[Clause map M167–M170](docs/decisions/INVARIANT_MAP.md#1-money-and-model-numerics).

### 1.43 Checked monthly mortality and explicit benchmark boundaries

The [accepted mortality/prepayment contract](docs/decisions/financial/cashflows/10-prepayment-and-mortality.md#accepted-decision-checked-monthly-mortality-explicit-model-clocks-and-benchmark-boundaries)
uses strict finite binary64 conditional probabilities and twelve-step survival
equivalence. Annual/monthly conversions use stable log1p/expm1, exact0/1 and
positive zero, validate before endpoints and reject invalid inputs or lost positive
zero results. Final upper-endpoint rounding is an approximation, not exact
certainty, an exact inverse or a portable accuracy guarantee. Remove permissive
clamped conversion and public unchecked psa_cpr.

Admit one immutable complete model through guarded constructors/restoration,
including future lockout parameters and every vector entry. Require positive
one-based applied months1..=u32::MAX, with actual date/loan-age/pool mapping owned
by the product. Constant CPR remains explicit; Lockout replaces CMBS-named legal
claims, and annual_cpr vectors retain input order and a disclosed hold-last tail.

PSA uses its published annual cap min(speed*min(month,30)/500,1), with finite
nonnegative speed and an exact represented-input cap comparison. Preserve stable
positive pre-cap annual survival through the private conversion instead of
rounding derived CPR to1 first. ABS is a loan-age/count benchmark; exact month*p
selects terminal depletion without epsilon or rounded-product repair. Use stable
checked pre-terminal denominator/division and an explicit absorbing-one extension;
neither this nor the benchmark promises constant original-pool-dollar cash.

Return model probabilities without selecting principal, default/prepayment/
amortization/interest order, recovery cash dates, shocks, irregular-period scaling
or extra product caps. Shared CDR/MDR conversion follows the scalar contract;
011 default/recovery models retain their own status. Reconcile actual PAC/MBS/
credit-pool clocks and error transport, guarded Rust/host/schema/parity state and
validating host constructor twins without obsolete aliases or error-to-zero paths.
Retain precise source/configuration provenance. Runtime and independent complete-
path numerical/host/product qualification remain open.
[Clause map M171–M174](docs/decisions/INVARIANT_MAP.md#1-money-and-model-numerics).

### 1.44 Explicit default targets, strict capacity and recovery authority

The [accepted default/recovery contract](docs/decisions/financial/cashflows/11-default-and-recovery-assumptions.md#accepted-decision-explicit-default-targets-strict-capacity-and-one-recovery-authority)
separates conditional Constant/SdaAge/annual-CDR vector models from original-
reference CumulativeLoss/Timing targets. Guard complete immutable input state.
Applied MDR/default-vintage months are positive; cumulative get_target retains0
as the true origin and returns None for conditional models. Use one canonical
mdr(month,available_fraction), requiring None for conditional variants and actual
Some(S) for targets, without inferred survival or mdr_with_survival.

SdaAge uses the selected primary age profile with exact global3*speed<=500 and
preserved annual survival, without claiming full SDA maturity/liquidation rules.
CumulativeLoss uses original loss percentages/reference severity; F may exceed1.
Timing uses exact positive-total annual_weights normalization and checked complete-
year reach. Compute monthly demand directly from admitted original inputs, not
rounded cumulative differences. Require represented-model demand<=actual supplied
eligible/reference S, with zero/zero no-op and strict shortfall errors; no epsilon,
clipping, hidden backlog or catch-up. Query-only numerical checks retain complete
expressions through the requested result. Ratio rounding can make a boundary
unavailable without certifying insufficient actual Money collateral.

Recovery has exclusive Constant rate or Vector severity authority and explicit
lag_months. Keep checked primary/complement getters, full input validation and
hold-last default-vintage selection. Remove dormant fallback rates, clamps and
infallible queries. Preserve the primary before complement rounding; later Money
use still requires core012's fallible admission and owned allocation rules.
Reference loss-conversion severity is not automatically actual recovery, so gross
default targets do not guarantee realized net-loss attainment/timing.

Actual fixed reference, eligible exposure, event phase, recovery dates/calendar/
queue/terminal tails and full asset allocation remain product-owned. Preserve
errors and economic default vintage rather than reconstructing baselines or
backdating from receipt dates. Reconcile existing Rust/spec/host/schema/parity and
validating constructors without aliases or new frameworks. Runtime and independent
complete-path numerical/host/product qualification remain open.
[Clause map M175–M178](docs/decisions/INVARIANT_MAP.md#1-money-and-model-numerics).

### 1.45 Complete schedules, explicit balances and separate cash views

The [accepted schedule contract](docs/decisions/financial/cashflows/12-schedule-lifecycle.md#accepted-decision-validated-complete-schedules-explicit-balances-and-separate-cash-views)
publishes immutable complete source rows/metadata with explicit optional principal
opening/through coverage. None declares untracked native events, never inferred
zero/USD stock; Some carries exact nonnegative opening Money and validates every
nonzero effect in its single currency/range. Retain foreign cash and literal zero
effects, while scalar queries require actual tracked context. Remove inception
recipes, representative day-count authority, funding/sign inference and validation
bypasses. Full constructors/restoration validate before publishing or filtering.

Canonical row order is presentation only and retains zeros/duplicates. Replay
explicit principal deltas in exact economic-date batches, requiring nonnegative
representable closing stock without tolerance/floor/display cleanup. Include the
opening-date closing point and later nonzero-effect dates; range-check sorted
queries including duplicates.003 producer phases remain separately validated;
remove fictional per-row balance pairs and unguarded float lookup.

Providers return complete validated source schedules without normalize_public,
rebasing, delta rewrites, synthetic events or cash-date history loss. Canonical
get_settlements has explicit cutoff/inclusion and uses the core015 cash whitelist,
retaining native signed/zero cash. Source information date, cash selection and
economic movement remain distinct; no settlement/claim-retirement/zero-value
proof follows from date filters. Remove successful Placeholder state/default tags;
NoResidual requires an explicitly empty represented scope.

Changes use complete owner reconstruction rather than editing bypasses or generic
scale_amounts. Generic merge only accepts compatible untracked Contractual/
NoResidual native collections; actual tracked/Projected/signed-leg owners retain
composition authority. Preserve parent/observation metadata and exact Money, remove
invented adapter inputs and lossy DataFrame reconstruction, and reconcile actual
product/export/host/schema/parity callers without new lifecycle frameworks.
Runtime and independent complete-path qualification remain open.
[Clause map M179–M182](docs/decisions/INVARIANT_MAP.md#1-money-and-model-numerics).

### 1.46 Original-parent cash quotation and inclusive first-ex dates

The [accepted cash-quotation contract](docs/decisions/financial/cashflows/13-accrued-interest.md#accepted-decision-original-parent-cash-quotation-and-inclusive-first-ex-dates)
selects linear allocation of an actual005 full cash component using the same001
parent/context authority, explicit currency assertion and qualified immutable
source references. It is quotation, not actual earning/accounting/termination.
Remove generic Compounded/AccrualMethod/AccrualConfig/include_pik/global frequency,
flat-row period/factor/principal guesses and ExCouponRule offset helpers.

Require S<E<=P, complete original-parent admitted prefixes A(t), and explicit
optional inclusive first_ex_date X<=P. None deliberately selects cum treatment,
not unknown entitlement. For t<P, f is0 before/at S, A(t)/A_E inside and1 at/after E;
contribution is C*(f-I_ex), with I_ex=1 when X exists and X<=t. At/after P reset
quotation to0. Pre-start detached cash therefore contributes-C. A_E=0/C=0 is a
validated zero case; nonzero C with A_E=0 is unsupported. Preserve signed cash,
full-coupon subtraction in the active ex window and distinct parent identities.

Compute the complete exact same-currency expression and restore Money once,
without intermediate fraction/ordinary-accrual/rebate/per-parent rounding, float
money sums or implicit minor-unit rounding. Keep one-shot accrued_interest and
immutable AccrualIndex::get_accrued on one authority, returning Rust Money including
explicit-currency empty zero. Preserve known/projected status through the same
parent owners; no future007 C/q knowledge follows from a date or stored path.

Actual coupon entitlement, principal/redemption, PIK claims, nonlinear earning,
statement recognition and exercise/payoff remain owner-selected. Reconcile bond/
loan/LSMC/statements and exact Rust/host/schema/parity paths without aliases or a
parallel coupon/provenance framework. Verified DMO and frozen LSE clauses support
specific product mappings, not a universal ex-date or settlement convention.
Runtime and independent complete-path qualification remain open.
[Clause map M183–M186](docs/decisions/INVARIANT_MAP.md#1-money-and-model-numerics).

### 1.47 Exact nominal reporting and explicit repayment life

The [accepted nominal-reporting and repayment-life contract](docs/decisions/financial/cashflows/14-aggregation-and-life.md#accepted-decision-exact-nominal-reporting-and-explicit-repayment-life)
uses exact whole-Money addition with wide cancelling intermediates and exactly
representable final totals, without float sums, transform Q or implicit currency
rounding. Period reporting takes the existing validated nonempty contiguous
PeriodPlan, retaining its one calendar/fiscal authority and flags, with half-open
selection and full-input validation. Preserve duplicate/zero contributions,
plan/currency order and sparse absent-versus-contributed-zero totals. Retain the
full plan once; guard construction, strict restoration and typed in-plan lookups
without Default, map-only state or raw-map Deref. cash_total requires explicit
currency over all inputs and returns that currency's zero for empty input.

Calendar ladders accept typed cash-only rows, rejecting supplied noncash events;
schedule wrappers validate their full source before preparing cash views. Group
literal signed cash by payment Gregorian year and native currency, returning
exact principal_cash/other_cash without par substitution, inferred cutoffs or
single-currency proof. Preserve contributed zero rows and guard supported years,
ordered unique keys and exact Money state. Qualified native-PV year reporting
stays separately labelled at the actual valuation owner, retaining selected-row,
currency, valuation-date and source association with genuine errors, never
nominal/zero fallbacks.

Remove generic WAL inference and CashFlowSchedule::wal. Rust-only repayment_life
takes owner-selected dated nonnegative exact par weights, as_of, currency and
include_as_of, validating all weights before filtering. None means no eligible
positive repayments; Some(0) means included positive repayments all today. Compute
sum(P*actual_days)/(365*sum(P)) with exact transient weights and one checked
nearest-even binary64 restoration; intermediate totals need not fit Money.
ACT/365F is the disclosed house statistic, not universal SIFMA WAL. Product
repayment/date/sign/completeness and path-estimator conventions retain owners.
Reconcile actual export/product/host/schema/parity consumers without aliases or
new frameworks. Runtime and independent complete-path qualification remain open.
[Clause map M187–M190](docs/decisions/INVARIANT_MAP.md#1-money-and-model-numerics).

### 1.48 Product-owned credit valuation and qualified PV reporting

The [accepted credit/reporting boundary](docs/decisions/financial/cashflows/15-credit-adjusted-pv.md#accepted-decision-product-owned-credit-valuation-and-qualified-pv-reporting)
removes generic cashflow-kind survival weighting, inferred opening/FIFO funded
recovery, automatic hazard recovery and schedule/private pv_by_period shortcuts.
Actual product/model owners qualify expectation, contingency, signed obligations,
unpaid recoverable claims, overlap, recovery basis/timing and value attribution.
Cashflow kinds, optional tracked principal and remaining repayments do not prove
those claims. Preserve signed cash and supported sign reversal on the same
admitted computation, without universal monetary linearity or nonlinear-price
antisymmetry. Introduce no replacement generic credit or exposure framework.

Reuse accepted core016 literally for ordinary qualified cash, including explicit
as_of/include_today, whole-input and required positive dated factor admission,
source clocks and checked Money arithmetic. Require actual native-currency
sources and coherent selected model/market snapshots, without foreign-curve or
first-dependency inference. Credit owners use accepted market04 local conditional
and interval probabilities; remove absolute-survival/default inference,
endpoint-ratio/clamp/previous-row and context-free clock annotations.

Keep actual source cash/events distinct from expected amounts and PV attribution.
Preserve admitted native PV Money without float round trips; aggregate values
once through014's existing exact PeriodPlan authority using qualified owner report
dates, not invented repayment dates or additional credit/discount factors.
Do not fabricate cash/recovery rows, zero contributions, residual plugs or rescale
values to force total agreement. Require explicitly supported product/model
attribution through existing export admission, defaulting unsupported otherwise,
while preserving canonical pricing and qualified native-PV reports.

Retain comparison to the same canonical model/date/currency/snapshot and applicable
scenario/reporting view; failed or incomplete attribution cannot report successful
reconciliation. Total agreement alone does not prove allocation or adopt the
observed tolerance. Reconcile actual export/product/Rust/host/schema/parity and
removed defaults/members without aliases. Runtime and independent complete-path
qualification remain open.
[Clause map M191–M194](docs/decisions/INVARIANT_MAP.md#1-money-and-model-numerics).

### 1.49 Qualified hypothetical fixing materialization

The [accepted fixing-materialization contract](docs/decisions/financial/cashflows/16-fixing-materialization.md#accepted-decision-qualified-hypothetical-fixing-materialization)
selects one hypothetical frozen-forward realization policy, never actual
publication by date advancement or FIXING: insertion. Retain only successful
qualified raw finite projections, before coupon controls, with actual observed
IndexId/interval/units/currency/CMS benchmark/FX direction and frozen pre-roll
source/information authority. Remove nullable placeholders, fallback repair,
series-string identity and FX notional-ratio reconstruction.

Take a qualified paired source, explicit required observation keys and candidate
projections. Remove observation-date windows and equal-date shortcuts; actual
owners select required knowledge, including publication-lagged value dates.
Each key needs eligible retained data or a genuine projection, otherwise error;
reject unrequested candidates. Explicitly known empty dependencies differ from
unsupported discovery or fake empty payoff schedules. Validate all inputs before
precedence or identity, normalize signed zero, collapse only exact consistent
observations and reject distinct values or contradictory provenance. Preserve
qualified retained exact values without overwrite; mere presence is insufficient.

Return the market paired with complete retained hypothetical observation/role/
source records, including earlier realizations. Preserve and consume that
qualification through actual owner requests, reports/envelopes, state and later
rolls; missing provenance never implies Published. Require configured target
series with explicit policies, genuine exact-lookup failures and one complete
staged immutable merge. Failures leave original market/data/provenance unchanged;
new hypotheses are distinct datasets, not publication revisions.

Empty/idempotent calls still prove required-key completeness. Limit partition
invariance to compatible fixed authority and globally consistent unions; no
whole-scenario rollback or product-value/carry guarantee follows. Reconcile actual
rate/overnight/CMS/FX, structured-credit, scenario/horizon/theta/metric and host/
schema/parity consumers without aliases or new generic registries. Broader
publication, scenario-ordering and transaction policies retain their owners.
Runtime and independent complete-path qualification remain open.
[Clause map M195–M198](docs/decisions/INVARIANT_MAP.md#1-money-and-model-numerics).

### 1.50 Checked spot BSM pricing and native Greek bundles

The [accepted spot BSM contract](docs/decisions/financial/models-analytic/spot-vanilla-and-greeks.md#accepted-decision-checked-spot-prices-native-derivatives-and-explicit-composite-results)
selects constant-coefficient European per-unit pricing with coherent scalar
price/rate/relative-volatility/model-year inputs. Validate finite positive S/K,
signed r/q and nonnegative T/sigma before exact-expiry or deterministic identities.
Keep the checked expiry-payoff helper's separate zero-spot domain. Use stable
log/scaled m and total-standard-deviation geometry, OTM Gaussian tails, parity,
ATM and deterministic positive differences, without unrequested-intermediate
range gates, price floors or false-zero repair. Lost known-positive outputs,
unresolved geometry/cancellation and nonfinite computations error; no all-finite-
input or universal accuracy guarantee is implied.

Keep independent bs_price and bs_greeks Result<Option<BsGreeks>>, with None only
for valid T0 or sigma0 after full validation. Regular inputs compute a complete
bundle or error. Use native partials delta=V_S, gamma=V_SS, vega=V_sigma,
theta=-V_T and separate r/q rhos holding other scalar parameters fixed. Remove
/100, theta-day denominators/defaults and fabricated singular Greeks. Model theta
is not dated carry; delta is unadjusted spot risk with no universal magnitude1
bound. Scale complete Gaussian products before tail loss; theta can round to0
through signed cancellation. Guard immutable/restored regular bundles with finite
fields, positive gamma/vega and coherent nonzero delta/rho signs, without Default
or unchecked mutations. Valid state does not certify formula provenance.

Expose checked kernels and needed fallible shared geometry only, removing public
unchecked paths. Retain option_calculation explicitly as price/IV-plus-Greeks:
vol echoes supplied or solved volatility, whole Greeks are nullable under the
model's authority and regular computation failure fails the composite. Price-only
callers use bs_price. Forward02/inverse03 laws retain their own adoption.

Actual equity/FX/dividend/smile/curve/lifecycle and replication/tree owners retain
model/position/payment and risk-chain-rule selection, preserving public report
units by explicit Rust conversion. Reconcile parameter/result state, actual
Rust/product/Python/WASM/UI/schema/parity surfaces without aliases, host finance
logic or numerical fallback. Runtime and independent complete-path qualification
remain open.
[Clause map M199–M202](docs/decisions/INVARIANT_MAP.md#1-money-and-model-numerics).

### 1.51 Unit forward option prices and native Greek bundles

The [accepted forward-model contract](docs/decisions/financial/models-analytic/forward-normal-shifted.md#accepted-decision-unit-forward-prices-and-native-model-greek-bundles)
selects explicit Black76, arithmetic-normal and fixed shifted-Black scalar laws
on one UNIT-numeraire price/Greek scale. Remove Black-only df, unchecked annuity
and raw/scalar sibling APIs. Actual product owners apply their qualified discount
OR annuity and notional once, distinguishing per-unit-rate annuity from a genuine
one-bp amount on the same notional basis; unit partials do not supply curve/smile
or multiplier chain rules.

Validate complete finite domains before exact T0/vol0 intrinsic: positive Black
coordinates, signed normal F/K, nonnegative model time/volatility and finite signed
shift with mathematical transformed coordinates in(0,MAX]. Preserve original F/K/s
through shifted admission/geometry and F−K payoff/parity, without requiring exact
one-f64 sums or silently substituting a rounded contract. Normal volatility has
actual underlying units; relative volatility and clocks retain their own meaning.
Use stable scaled/log/tail calculations or explicit refusal, not tiny-volatility
intrinsic switches, negative-time clamps, floors, NaN or false-zero repair.

Keep independent checked unit prices and Result<Option<ForwardGreeks>> bundles.
None means valid zero time/volatility only; regular requests compute native forward
delta/gamma/actual-volatility vega or error. Guard immutable/restored bundles with
finite nonzero |delta|<=1 and positive gamma/vega. Preserve original-coordinate
parity and appropriate Black/shifted bounds, without a normal F/K upper cap.
The requested unit output must fit even if a later external multiplier could
rescue a mathematical final PV; unrequested intermediate overflow need not fail
when the requested unit output is resolvable. No arbitrary final-PV guarantee.

Reconcile shared calculator/target-price unit normalization, fallible needed Black
geometry and complete shifted bundle, leaving inverse03 solver laws separate.
Update actual cap/floor/CMS/replication/commodity/listed/range-accrual and calibration/
IV users with real errors and explicit owner report conversions. Propagate exact
names, nullable bundles and removed fields/defaults across Rust/host/UI/schema/
parity without compatibility or host financial logic. Runtime and independent
complete-path qualification remain open.
[Clause map M203–M206](docs/decisions/INVARIANT_MAP.md#1-money-and-model-numerics).

### 1.52 Explicit bounded volatility inversion

**Detailed decision:** [MODELS-ANALYTIC-03](docs/decisions/financial/models-analytic/implied-volatility-solvers.md#accepted-decision-explicit-bounded-model-inversion-and-paired-premium-acceptance) was accepted on 2026-11-10.

Use one fallible scalar inverse with explicit spot BSM, Black76, Bachelier or
shifted Black terms and a frozen input request. Accept original spot or unit
forward premiums in their native units. Require positive time and literal
model bounds in the original geometry. Check the exclusive Black upper bound
first. Exact lower equality selects zero IV and all interior requests require
positive returned IV. Unresolved comparisons raise computation errors.

Require immutable, validated maximum volatility, annual volatility tolerance,
iteration budget, absolute native premium tolerance and relative premium
tolerance. Run the accepted Brent method on the supplied annual-volatility
interval [0,max_vol], using the bounded objective from the checked same-law log
OTM premium. Keep the original OTM target and propagate numerical errors.
The core stopping criterion governs solver work. On every exit validate the
actual returned volatility and both original premium and independently
evaluated OTM time-value residuals. Computed residuals bound neither input or
model error nor volatility error under poor conditioning.

Use the same API and required controls in the composite with its accepted Greek
availability. Preserve actual product inverse objectives and normalization.
Reconcile existing Rust, Python, WASM, UI, schema and parity surfaces and retire
obsolete inverse routes. Runtime and independent complete-path qualification
remain open. [Clause map M207–M210](docs/decisions/INVARIANT_MAP.md#1-money-and-model-numerics).

### 1.53 Discrete Asian fixing and payment laws

**Detailed decision:** [MODELS-ANALYTIC-04](docs/decisions/financial/models-analytic/asian-moments.md#accepted-decision-for-discrete-asian-pricing) was accepted on 2026-11-10.

Use one checked scalar price with explicit averaging and equal-weight fixing
slots. Preserve finite observed levels and positive prepared expectations in a
common payment measure. Geometric history is positive. Arithmetic history can
be signed. Require positive ordered projected model times at or before payment,
preserve exact ties, and use the separate positive payment discount once for the
discounted per-unit result. Dated owners establish chronology, eligibility and
history coverage. Retire count sentinels and implicit averaging.

Price the full seasoned geometric lognormal marginal using positive interval
sums. For arithmetic, retain the deterministic observed contribution in the
payoff and match the first two moments of the positive future contribution
using the covariance sum. Preserve exact support, deterministic and shared
Brownian-time reductions, shifted bounds and parity. The two-moment fit has
limited price accuracy at distinct stochastic times.

Keep stable scaled geometry through final discounting and report unresolved
variance, sign or premium as computation errors. Reuse the laws for actual
equity and commodity inputs, matching the MC control to the sampled payoff.
Keep payment and observation evidence with the dated owner and reconcile
existing Rust, host, schema and parity surfaces. Runtime and independent
complete-path qualification remain open.
[Clause map M211–M214](docs/decisions/INVARIANT_MAP.md#1-money-and-model-numerics).

### 1.54 Continuous barrier premiums and first-passage cash

**Detailed decision:** [MODELS-ANALYTIC-05](docs/decisions/financial/models-analytic/barriers-and-rebates.md#accepted-decision-for-continuous-barrier-premiums-and-cash) was accepted on 2026-11-10.

Admit checked explicit flat GBM inputs with continuous inclusive monitoring
on the remaining model interval. The dated owner qualifies the current
observation and earlier history. Preserve exact singleton and deterministic
crossing, and compute the requested hit or surviving option premium from the
reflection law. Preserve structural zeros, direct positive values and
no-rebate in/out parity.

Use the strike-free first-passage law for reported touch and conditional cash.
Select hit-at-hit, hit-at-expiry or no-hit-at-expiry explicitly. Preserve
independent hit and survival tails and final amount/discount scale. Use the
real-root formula when its discriminant is nonnegative. For a negative
discriminant use the same expectation with the selected discount coordinate,
scaled estimator budget and bounded core Simpson work. Propagate precision
and convergence failures.

Dated asset and FX owners retain active monitoring, relevant terminal
observations, quantity, currency and separate outstanding payment evidence.
Reconcile actual FX touch and asset/FX barrier callers, and keep the checked
existing host price route. Runtime and independent complete-path
qualification remain open.
[Clause map M215–M218](docs/decisions/INVARIANT_MAP.md#1-money-and-model-numerics).

### 1.55 Continuous lookback premiums and extrema

**Detailed decision:** [MODELS-ANALYTIC-06](docs/decisions/financial/models-analytic/lookback-extrema.md#accepted-decision-for-continuous-lookback-premiums) was accepted on 2026-11-10.

Use one checked continuous lookback price with explicit fixed or floating
terms and a qualified relevant extremum through the eligible current
observation. Fixed call and floating put require a maximum at least spot.
Fixed put and floating call require a minimum at most spot. Require explicit
net carry and an independent payment discount. Return the discounted
per-underlying-unit result. Preserve the exact singleton and deterministic
path payoffs after complete admission.

Assemble all four regular prices from positive vanilla and reflection terms
and the fixed locked payoff where applicable. Use the analytic derivative
only at exact zero carry. Preserve original signed nonzero carry in a checked
divided difference and keep final-price scale. Propagate unresolved precision
and avoid floors, sentinels and silent repair.

Dated owners qualify history, eligible observations and actual payment state,
resolve known claims from their required terminal facts and preserve native
currency and quantity. Separate payoff terms from volatility lookup and
scenario observation treatment. Reconcile the live asset pricer and existing
host surfaces. Runtime and independent complete-path qualification remain open.
[Clause map M219–M222](docs/decisions/INVARIANT_MAP.md#1-money-and-model-numerics).

### 1.56 Fixed-conversion quanto pricing

**Detailed decision:** [MODELS-ANALYTIC-07](docs/decisions/financial/models-analytic/quanto-drift.md#accepted-decision-for-fixed-conversion-quanto-premiums) was accepted on 2026-11-10.

Use one checked European quanto terminal law with positive underlying and
strike, prepared foreign log carry, independent payout discount and common
asset/FX variance horizon. FX is payout currency per unit of asset currency.
Reciprocal log shocks reverse the correlation. Subtract the signed total
covariance rho sigma_asset sigma_FX T once from foreign log carry.

Keep exact zero-time, zero-asset-volatility and zero-covariance distinctions.
Use stable log-forward, covariance and discounted Black arithmetic in the
requested pre-conversion scale. Preserve parity, positivity and the actual
dependence of drift on asset volatility, and propagate numerical failures.

Return the domestic-discounted scalar in foreign payoff units per underlying
unit. The product applies literal contractual conversion and quantity once.
Dated owners retain source orientation, clocks, fixing, outstanding payments,
sizing and quote/curve risk. Reconcile actual Rust and host callers. Runtime
and independent complete-path qualification remain open.
[Clause map M223–M226](docs/decisions/INVARIANT_MAP.md#1-money-and-model-numerics).

### 1.57 Checked Heston probability transforms

**Detailed decision:** [MODELS-ANALYTIC-08](docs/decisions/financial/models-analytic/heston-characteristic-function.md#accepted-decision-for-the-checked-heston-transform) was accepted on 2026-11-10.

Use immutable validated pricing-measure Heston state and the existing strict
pricing wrapper. Admit nonnegative initial variance and vol-of-vol, positive
mean reversion and long-run variance, and open correlation endpoints. Use
inclusive exact Feller comparison as a diagnostic. Preserve other algorithms'
positive-parameter requirements until their owners qualify broader inputs.

Use the two typed real-frequency cash and share contours of carry-normalized
log spot. Preserve finite-horizon share tilting for either sign of its variance
drift, exact zero-time and zero-frequency normalization, and exact integrated
deterministic variance at zero vol-of-vol.

Use one checked affine logarithm from the decaying principal-log branch
without the g ratio. Preserve scaled discriminant, origin and small-time
remainders and representable components. Reject unresolved arithmetic and
phase without zero sentinels, clipping or magnitude cutoffs. Fourier weights,
truncation and price acceptance remain with record 09.

Reconcile actual pricing, market state, PDE, MC, rough and host consumers
without inferring their broader financial or numerical domains. Runtime and
independent complete-path qualification remain open.
[Clause map M227–M230](docs/decisions/INVARIANT_MAP.md#1-money-and-model-numerics).

### 1.58 Bounded Heston Fourier pricing

**Detailed decision:** [MODELS-ANALYTIC-09](docs/decisions/financial/models-analytic/heston-fourier-integration.md#accepted-decision-for-bounded-heston-fourier-pricing) was accepted on 2026-11-10.

Use one checked scalar Heston price with explicit payoff choice. Validate the
whole request before side-specific expiry intrinsic or exact deterministic
variance pricing. For positive vol-of-vol, integrate the selected OTM time
value with the checked cash and share logarithms and scaled discounted legs.
Preserve positive resolved time value, payoff bounds and final monetary scale.

Use the conditional integrated-variance envelope with conservative
arithmetic to select the cutoff. Then refine order-16 Gauss–Legendre meshes
at fixed cutoff. Require the selected consecutive raw-probability, relative
time-value and upper-margin gates within the cumulative work budget. The
analytic envelope bounds truncation. Mesh and arithmetic checks provide
operational resolution only. An unresolved wing can also prevent an ITM
price from qualifying.

Preserve actual numerical errors. Keep one production scalar route and
coordinate the existing host price calls. Dated equity owners retain actual
expiry, fixing, outstanding cash, parameter sources, clocks and quantity.
Runtime and independent complete-path qualification remain open.
[Clause map M231–M234](docs/decisions/INVARIANT_MAP.md#1-money-and-model-numerics).

### 1.59 Checked COS pricing

**Detailed decision:** [MODELS-ANALYTIC-10](docs/decisions/financial/models-analytic/cos-inversion.md#accepted-decision-for-checked-cos-pricing) was accepted on 2026-11-10.

Use one checked COS engine for the three named scalar Black, Merton and
variance-gamma pricing calls. Require initial terms, initial window multiplier
and absolute discounted per-unit premium tolerance. Validate the full
request before exact expiry, deterministic and support payoffs.

Evaluate the requested bounded payoff under the cash measure for a put or
the reflected share measure for a call. Preserve original payoff geometry,
centered coefficients, compensated summation and final native scale.

Use cumulants only to seed the window. Select the window by a conservative
omitted-mass bound and the term count by a separate conservative series
bound. Evaluate one final spectrum of at most 65,536 terms. Require the
pre-rounding premium budget, raw payoff bounds and positive numerical
resolution. Propagate range and work failures without price clipping.

Reconcile the actual scalar hosts, schema and notebooks. Record 11 owns the
underlying law and contour inputs. Production enclosures, pricing and
complete-path qualification remain open.
[Clause map M235–M238](docs/decisions/INVARIANT_MAP.md#1-money-and-model-numerics).

### 1.60 Checked characteristic laws for COS

**Detailed decision:** [MODELS-ANALYTIC-11](docs/decisions/financial/models-analytic/jump-characteristic-laws.md#accepted-decision-for-checked-characteristic-laws) was accepted on 2026-11-10.

Use private validated Black, Merton and variance-gamma laws to prepare
the cash put or reflected share call distribution of forward-normalized
log return. Keep deterministic carry and discount with the scalar price.
Validate the exact model domains before expiry and inactive-jump reductions.

Preserve total variance, selected tilted parameters, centered cumulants,
log moments, real-frequency transforms and model-specific COS envelopes.
Use stable compensation and remainder formulas and conservative moment
bounds. Preserve atoms and lattice support for Merton and strict
variance-gamma martingale and moment-domain signs from original inputs.
Propagate unresolved geometry, phase, range and enclosure errors.

Retire public raw CF state and standalone schema registrations. Reconcile
the actual scalar engine, benchmark, hosts and notebooks. Record 10 retains
pricing work and premium acceptance. Production numerical and complete-path
qualification remain open.
[Clause map M239–M242](docs/decisions/INVARIANT_MAP.md#1-money-and-model-numerics).

### 1.61 Checked rough Heston pricing

**Detailed decision:** [MODELS-ANALYTIC-12](docs/decisions/financial/models-analytic/rough-heston-riccati.md#accepted-decision-for-checked-rough-heston-pricing) was accepted on 2026-11-10.

Use the checked positive stochastic rough domain and fractional model
clock. Admit the complete scalar market, explicit payoff and two
premium tolerances before evaluating exact zero-time intrinsic. Keep
one private fractional Riccati solver and normalized Lewis half contour.

Use stable positive product-integration weights, a guarded implicit
root, checked residuals and both affine time integrals. Enclose the
continuous omitted tail analytically and check time and frequency
discretization separately at common cutoffs. Bound cumulative work
and accept raw time value only within the selected resolution gates.
Propagate hard, convergence and range failures.

Reconcile the actual equity model configuration and product path.
Retire the standalone arbitrary solver, transform and unused inverse.
Keep the direct scalar model helper Rust-only. Independent finite-H
price, production enclosure and complete-path qualification remain open.
[Clause map M243–M246](docs/decisions/INVARIANT_MAP.md#1-money-and-model-numerics).

### 1.62 Checked SABR asymptotic quotes

**Detailed decision:** [MODELS-ANALYTIC-13](docs/decisions/financial/models-analytic/sabr-expansions.md#accepted-decision-for-explicit-checked-sabr-quotes) was accepted on 2026-11-10.

Use immutable canonical SABR parameters with literal beta and open
correlation. Require an explicit Normal, Black or shifted Black quote,
qualified model time and matching displacement. Exact beta-zero Normal
uses signed original coordinates. All other cases require positive
admitted effective coordinates and preserve the original strike gap.

Use the exact CEV distance and one checked skew factor for the selected
short-time leading term. Apply the stated Hagan midpoint time correction.
Preserve exact ATM and constant-volatility limits, and propagate unresolved
correction, output and range failures without floors or retagging.

Use the same checked formula and convention in prepared evaluation, fits,
smiles and product sources. Reconcile existing schema and hosts. Calibration,
smile repair and off-grid policies retain their owners. Production quote,
fit and complete-path qualification remain open.
[Clause map M247–M250](docs/decisions/INVARIANT_MAP.md#1-money-and-model-numerics).

### 1.63 Checked SABR calibration

**Detailed decision:** [MODELS-ANALYTIC-14](docs/decisions/financial/models-analytic/sabr-calibration.md#accepted-decision-for-explicit-checked-sabr-calibration) was accepted on 2026-11-10.

Fit one expiry at literal beta with explicit quote, displacement and
mode. Keep original strike identity and the accepted SABR expansion.
Use vega-weighted relative-volatility residuals and a separate native
absolute error cap for every original quote.

Require checked solver controls and fit intervals. Use nine ordered
starts and literal diagnostics. A selected ATM pin requires an
observed K=F quote, an increasing supplied alpha bracket, checked
Brent and a returned-quote residual check. Reprice each final candidate
and rank only candidates that pass every native gate.

Preserve actual work, fit failures, quote and displacement provenance
and the actual equity, swaption, schema and host contracts. Production
solver, independent market and publication qualification remain open.
[Clause map M251–M254](docs/decisions/INVARIANT_MAP.md#1-money-and-model-numerics).

### 1.64 Checked SABR smile and finite-grid repair

**Detailed decision:** [MODELS-ANALYTIC-15](docs/decisions/financial/models-analytic/sabr-smile-repair.md#accepted-decision-for-checked-sabr-smile-and-finite-grid-repair) was accepted on 2026-11-10.

Bind one checked quote convention for the immutable smile. Use the
selected ATM-volatility forward-delta proxy and check the actual
returned strike and smaller delta tail. Use unit forward call prices
for sampled bounds, both vertical inequalities and unequal-grid
convexity, including the applicable exact Black support anchor.

Use the greatest exact down-only sampled price grid and a native
per-node move cap. Preserve unchanged quotes and exact intrinsic.
Use the accepted inverse for changed interior targets and reprice
the actual returned quotes before accepting the sampled result.

Keep repair opt-in and retain the immutable source state. Reconcile
the actual Python, WASM and cube-slice UI consumers. Production
inverse, arithmetic and complete-path qualification remain open.
[Clause map M255–M258](docs/decisions/INVARIANT_MAP.md#1-money-and-model-numerics).

### 1.65 Checked volatility-source evaluation and materialization

**Detailed decision:** [MODELS-ANALYTIC-16](docs/decisions/financial/models-analytic/volatility-source-interpolation.md#accepted-decision-for-explicit-volatility-source-evaluation-and-materialization) was accepted on 2026-11-10.

Resolve the selected source and its specific inputs, then return the checked
annual sigma, exact convention, source clock and evaluated coordinates.
Use explicit strict or flat grid boundaries, exact active support and
checked native sigma or total-variance interpolation. Shifted quote blends
require a common displacement.

SABR ParameterSpace blends a common-beta stencil with consistent shift
presence. TotalVariance evaluates active expiry rows at the common absolute
strike and their own construction forwards. Pricing qualifies the forward
basis and actual quote domain.

Materialize strict samples with explicit output interpolation, source
clock and provenance. Compare explicit, compatible reference quotes in
native sigma units. Runtime, host and independent qualification remain open.
[Clause map M259–M262](docs/decisions/INVARIANT_MAP.md#1-money-and-model-numerics).

### 1.66 Checked FX delta smiles and strike materialization

**Detailed decision:** [MODELS-ANALYTIC-17](docs/decisions/financial/models-analytic/fx-delta-smiles.md#accepted-decision-for-checked-fx-delta-smiles-and-strike-materialization) was accepted on 2026-11-10.

Use the accepted ordered-pair FX quote profile and qualified delivery
forward. Check the smaller forward-delta tail at each returned strike
and require strict semantic pillar order. Preserve all supplied labels.

Direct evaluation uses strict expiry support and complete affine ATM,
RR and BF reconstruction at the requested expiry and forward. Use linear
strike interpolation with an explicit strict or flat strike boundary.

Materialize the exact union of original strikes with qualified row forwards,
explicit output interpolation and construction provenance. Reconcile active
pricing, risk, host and state routes. Independent qualification remains open.
[Clause map M263–M266](docs/decisions/INVARIANT_MAP.md#1-money-and-model-numerics).

### 1.67 Checked raw SVI and slice fit acceptance

**Detailed decision:** [MODELS-ANALYTIC-18](docs/decisions/financial/models-analytic/svi-fit-and-density.md#accepted-decision-for-checked-raw-svi-and-slice-fit-acceptance) was accepted on 2026-11-10.

Use immutable raw SVI with exact nonnegative minimum, left slope at
most two and right slope below two. Evaluate requested variance or
annual sigma with checked complete-result arithmetic. Use analytic
Durrleman g where defined and report finite sampled coverage.

Use explicit seeded checked LM on unweighted total variance, with the
analytic flat case for equal volatilities. Replay every quote in native
annual Black sigma and require both the per-quote cap and the supplied
finite density scan including quote coordinates. Preserve actual work.

The fitted slice and published strike grid have separate replay and
acceptance. Reconcile the equity target and existing host and state
paths. Independent qualification remains open.
[Clause map M267–M270](docs/decisions/INVARIANT_MAP.md#1-money-and-model-numerics).

### 1.68 Checked Dupire extraction and local-volatility grids

**Detailed decision:** [MODELS-ANALYTIC-19](docs/decisions/financial/models-analytic/dupire-local-volatility.md#accepted-decision-for-checked-dupire-extraction-and-local-volatility-grids) was accepted on 2026-11-10.

Use a qualified Black strike surface under deterministic proportional
carry. Choose explicit extraction coordinates, fixed-moneyness support
and smoothing. Use actual unequal-grid interior time derivatives, adjacent
time endpoint secants and local quadratics in log-moneyness.

Require positive center variance and density with nonnegative sampled
time slope. Evaluate the complete scaled local sigma and publish the
whole requested grid with source and forward provenance.

Use an immutable local sigma grid on elapsed time and absolute spot
with explicit strict or flat boundaries. Reconcile simulation clocks,
carry, host state and checked lookup. Independent extraction, vanilla
replay and simulation qualification remain open.
[Clause map M271–M274](docs/decisions/INVARIANT_MAP.md#1-money-and-model-numerics).

### 1.69 Checked surface-arbitrage diagnostics and coverage

**Detailed decision:** [MODELS-ANALYTIC-20](docs/decisions/financial/models-analytic/surface-arbitrage-diagnostics.md#accepted-decision-for-checked-surface-arbitrage-diagnostics-and-coverage) was accepted on 2026-11-10.

Use explicit diagnostic selection, qualified row forwards and native
thresholds. Check sampled Black calls with the exact support anchor.
Compare adjacent expiries at the union of both rows' fixed-moneyness
knots on common support. Check the two Dupire signs independently.

Report tested scope, unavailable coverage and all native findings.
Any breach fails the selected criteria. Passing requires complete
selected coverage and zero breaches. Display selection leaves the
native verdict intact. Reconcile actual hosts and the pricing
workbench. Independent qualification remains open.
[Clause map M275–M278](docs/decisions/INVARIANT_MAP.md#1-money-and-model-numerics).

### 1.70 Checked variance replication and explicit Black wings

**Detailed decision:** [MODELS-ANALYTIC-21](docs/decisions/financial/models-analytic/variance-replication.md#accepted-decision-for-checked-variance-replication-and-explicit-black-wings) was accepted on 2026-11-10.

Use a qualified forward, horizon and two-sided Black strike strip. Use
checked volatility samples, checked undiscounted OTM premiums and the
exact forward split with one ATM premium. Require explicit midpoint
refinement acceptance in annual-variance units.

Choose finite-strip coverage or full flat-Black wings. Compute modeled
wings through the positive finite-domain identity with bounded quadrature.
Preserve exact zero, checked errors, native contributions and actual work.

Reconcile equity and FX product callers and preserve their dated,
observation and settlement ownership. Independent numerical and product
qualification remain open.
[Clause map M279–M282](docs/decisions/INVARIANT_MAP.md#1-money-and-model-numerics).

### 1.71 Checked price-equivalent ATM volatility conversion

**Detailed decision:** [MODELS-ANALYTIC-22](docs/decisions/financial/models-analytic/atm-volatility-conversion.md#accepted-decision-for-checked-price-equivalent-atm-volatility-conversion) was accepted on 2026-11-10.

Match the original at-the-forward unit premium with complete validation
at positive expiry. Preserve exact effective shifted levels and genuine
zero or exact pricing-law identity. Invert Normal directly and Black
using the smaller probability, with a strict mathematical cap.

Preserve premium and cap-gap scale through the inverse and replay the
actual returned sigma. Apply explicit absolute premium and relative
smaller-component tolerances. Propagate unresolved range or precision
and reconcile existing Python and WASM paths.
[Clause map M283–M286](docs/decisions/INVARIANT_MAP.md#1-money-and-model-numerics).

### 1.72 Checked Hull–White analytic kernels and payment measures

**Detailed decision:** [MODELS-ANALYTIC-23](docs/decisions/financial/models-analytic/hull-white-analytic-kernels.md#accepted-decision-for-checked-hullwhite-analytic-kernels-and-payment-measures) was accepted on 2026-11-10.

Use one checked Gaussian law for constant or scheduled volatility,
with exact Ho–Lee and deterministic limits. Preserve scale through
conditional moments, affine bonds and original-leg bond options.

Require explicit term fixing, accrual and payment geometry. Use the
deterministic multiplicative projection basis and signed payment
shift, and price on gross accumulation with exact support limits.
The continuous short-rate accumulation law has its own payment
adjustment. Contractual discrete overnight schedules require their
own qualified model or explicit approximation.

Use the exact quote-dependent standalone simple-rate futures bias
and the price-equivalent term ATM Normal quote with actual-sigma
replay. Reconcile valuation, calibration and existing host callers.
[Clause map M287–M290](docs/decisions/INVARIANT_MAP.md#1-money-and-model-numerics).

### 1.73 Checked Hull–White clocks, curve fit and term projection

**Detailed decision:** [MODELS-ANALYTIC-24](docs/decisions/financial/models-analytic/rate-clock-and-projection.md#accepted-decision-for-checked-model-clocks-curve-fit-and-term-projection) was accepted on 2026-11-10.

Use the explicit ACT/365F model origin and exact dated coordinates.
Compose each curve's own signed clock and context, rebase the admitted
discount source and use the actual one-sided composed derivative.

Fit the centered Gaussian model with the exact deterministic shift and
its checked interval integral. Use the equivalent centered conditional
bond law and the actual term fixing and index accrual with the
deterministic multiplicative gross basis. Preserve known fixings as rates.

Select coherent active parameters, preserve exact dated sigma knots and
qualify schedule rolling and scenario replay. Reconcile valuation,
calibration, risk and existing hosts.
[Clause map M291–M294](docs/decisions/INVARIANT_MAP.md#1-money-and-model-numerics).

### 1.74 Exact joint Hull–White paths and sampling

**Detailed decision:** [MODELS-ANALYTIC-25](docs/decisions/financial/models-analytic/hw1f-monte-carlo-discounting.md#accepted-decision-for-exact-joint-hullwhite-paths-and-sampling) was accepted on 2026-11-10.

Step the centered rate and integrated short rate with the same joint
Gaussian law. Use checked scheduled covariance roots and accumulate
the bank in log units. Discount path cash at its actual payment time.

Preserve exact grid and event identities. Require explicit Philox
controls and keep both ordered shocks together for each antithetic
partner. Use one observation per pricing base group, checked Student-t
sampling uncertainty and actual work counts. Reconcile valuation, risk
and existing hosts, with correlated driver and exercise policy choices
owned by their respective records.
[Clause map M295–M298](docs/decisions/INVARIANT_MAP.md#1-money-and-model-numerics).

### 1.75 Checked issuer-call policy training and pricing

**Detailed decision:** [MODELS-ANALYTIC-26](docs/decisions/financial/models-analytic/hw1f-issuer-lsmc.md#accepted-decision-for-checked-issuer-call-policy-training-and-pricing) was accepted on 2026-11-10.

Use exact ordered decisions, owner-supplied eligibility and actual
dated branch cash. Minimize holder value in decision-time units,
retain common cash once and continue at equality.

Fit checked full-rank continuation on eligible decision-time
training state. Fail sparse live fits. Freeze the backward-trained
policy and price it forward on independent groups, keeping antithetic
partners together. Report the fixed policy's value, sampling
uncertainty and actual work. Reconcile callable range accrual, risk
and existing host routes.
[Clause map M299–M302](docs/decisions/INVARIANT_MAP.md#1-money-and-model-numerics).

## 2. Determinism and floating point

**Authority:** [ADR 0021](docs/decisions/standards/0021-reproducibility-guarantees.md)
defines the reproducibility guarantees;
[ADR 0014](docs/decisions/project/0014-financial-reference-evidence.md) governs
reference evidence and tolerances. The stream, ordering, reduction, and
comparison rules below retain their detailed scope.

### 2.1 Reproducibility tiers

**Detailed decision:** [0043](docs/decisions/standards/0043-deterministic-ordering-and-monte-carlo-execution.md)
is accepted for target-specific baselines and concrete result/error ordering under
accepted ADR 0021. [Clause map D01–D06](docs/decisions/INVARIANT_MAP.md#2-determinism-and-floating-point).

Every public stochastic API declares the bit-reproducible, seed-reproducible,
or statistically reproducible guarantee defined in
[ADR 0021](docs/decisions/standards/0021-reproducibility-guarantees.md), including
its supported target and execution conditions.

**Per-target reproducibility (native vs wasm32).** A guarantee within one
build target does not establish cross-target identity. Native (Python) and wasm32 (WASM) builds use different
platform implementations of transcendental functions (`exp`, `ln`, `powf`,
statrs CDFs, nalgebra decompositions), so any `f64` result that uses them may
differ at the ulp level between targets. Monte Carlo and finite-difference or
bump metrics can amplify this. A Greek dominated by numerical noise (for
example a vega bumped off a near-zero vol) can be exactly zero on one target
and non-zero on the other. Calibration
diagnostics (iteration counts, multi-start winners) can differ the same way.
Therefore:

- a Python↔WASM behavioural parity test MUST compare `f64` outputs with an
  explicit justified tolerance unless cross-target bit identity is demonstrated
  for the supported conditions and mathematical-library implementations;
- baselines captured on one target (for example UI fixtures captured from the
  wasm32 build) must be recaptured on that same target, recording the relevant
  build, runtime, and execution conditions needed to interpret or reproduce
  the baseline; the target name alone does not establish reproducibility;
- what MUST agree across targets is everything that is not `f64` rounding:
  result structure, keys, ordering, error kinds, and **which item an error
  names**. Parallel maps MUST return the lowest-index failing item's error on every
  target and thread count (`finstack_quant_core::parallel::try_map_ordered`),
  and hash-map iteration order (which can differ between 64-bit native and
  wasm32) MUST NOT determine results, floating-point reductions or error selection.

**Evidence:** [ordered parallel mapping and its tests](finstack-quant/core/src/parallel.rs)
implement lowest-index error selection. The
[EWMA implementation](finstack-quant/features/src/timeseries.rs) uses
`libm::hypot` to reduce target-dependent behavior; that choice alone does not
prove all cross-target guarantees. Coverage and previously reported numerical
differences are tracked in
[FOLLOW_UPS.md](docs/decisions/FOLLOW_UPS.md#implementation-and-lifecycle-follow-up).

### 2.2 Random number generation

**Decisions:** [0021](docs/decisions/standards/0021-reproducibility-guarantees.md)
is accepted for explicit seeds and declared guarantees;
[0043](docs/decisions/standards/0043-deterministic-ordering-and-monte-carlo-execution.md)
is accepted for the concrete RNG and execution conditions.
[Clause map D07–D09](docs/decisions/INVARIANT_MAP.md#2-determinism-and-floating-point).

**Required:** library code MUST use explicit seeds. Do not use
`thread_rng()` or `rand::random()`.

**Required for the primary Monte Carlo engine's bit-identity guarantee:**
within the supported target and fixed declared inputs and configuration,
bit-identical serial/Rayon results require all of the following:

- a splittable stream such as `PhiloxRng::split(path_id)` or
  `substream(path_id)`;
- a work partition determined by path count and declared chunk configuration,
  never by worker or thread count. Default chunking is a pure function of path
  count; a caller-selected chunk size is held constant for the comparison;
- a fixed merge order independent of rayon thread count; and
- adaptive confidence-interval stopping disabled.

Philox alone is not the guarantee. Non-splittable sequences such as the
current Sobol implementation do not satisfy this parallel tier. Halton
multi-start calibration is deterministic by construction.

**Evidence:** the [Philox stream](finstack-quant/models/src/monte_carlo/rng/philox.rs),
[engine partition and merge logic](finstack-quant/models/src/monte_carlo/engine/pricing.rs),
and [serial/parallel and thread-count tests](finstack-quant/models/src/monte_carlo/engine/tests.rs)
support the named same-target conditions. The tests also reject quasi-random
streams in the generic engine; this is not a claim of support for every RNG
or proof of cross-machine or native/WASM bit identity.

### 2.3 Map ordering

**Detailed decision:** [0043](docs/decisions/standards/0043-deterministic-ordering-and-monte-carlo-execution.md)
is accepted for ordered public maps and private hash-map use.
[Clause map D10–D11](docs/decisions/INVARIANT_MAP.md#2-determinism-and-floating-point).

**Required:**

- `std::collections::HashMap` uses a per-instance random seed and exposes
  arbitrary iteration order.
- `finstack_quant_core::FxHashMap` uses deterministic hashing
  for a given insertion sequence, but its iteration order is still not a
  serialization or API contract.
- Any map order visible through serialization, files, public iterators,
  snapshots, or golden tests MUST use `IndexMap` for insertion order or
  `BTreeMap` for sorted order. Populate insertion-ordered maps in a
  deterministic logical order; inserting from hash iteration or worker
  completion order does not satisfy this requirement.

Hash maps MAY be used for internal caches, intern tables, and lookups whose
order cannot escape.

**Evidence:** the [core exports](finstack-quant/core/src/lib.rs) expose
`FxHashMap` and `FxHashSet`. Public ordering and reduction coverage require
caller-level evidence; the hasher's identity is not an ordering guarantee.

### 2.4 Reductions

**Detailed decisions:** [0042](docs/decisions/standards/0042-numerical-accumulation-and-comparisons.md)
is accepted for accumulation accuracy;
[0043](docs/decisions/standards/0043-deterministic-ordering-and-monte-carlo-execution.md)
is accepted for deterministic execution under the promised reproducibility tier.
[Clause map D12–D14](docs/decisions/INVARIANT_MAP.md#2-determinism-and-floating-point).

Floating-point addition is commutative for finite ordinary operands but is
not associative. A parallel `+` fold is therefore not automatically
deterministic.

**Required:**

- Reductions promising bit identity across supported execution modes or
  thread counts MUST use deterministic partitions and a fixed merge tree
  for the same declared inputs and configuration.
- `OnlineStats::merge`, Welford/Chan statistics, and compensated sums still
  depend on merge order; use them inside that fixed tree when such bit
  identity is promised.
- Compensated summation improves accuracy. It does not independently provide
  bit reproducibility.
- Do not use an arbitrary term-count threshold as a substitute for analysing
  sign cancellation, dynamic range, and materiality.

### 2.5 Comparisons

**Decisions:** [0014](docs/decisions/project/0014-financial-reference-evidence.md)
is accepted for justified tolerances and
[0038](docs/decisions/standards/0038-substantive-api-documentation.md) for accurate
API references; [0042](docs/decisions/standards/0042-numerical-accumulation-and-comparisons.md)
is accepted for tolerance semantics, exact numerical equality, and bitwise
comparison of bit-identity guarantees.
[Clause map D15–D17](docs/decisions/INVARIANT_MAP.md#2-determinism-and-floating-point).

**Required:** approximate model results MUST use a documented tolerance with
explicit semantics: absolute, relative, notional-scaled, probability, rate,
percentage, money half-unit, or another named domain tolerance.

Exact numerical `f64` equality is allowed only when its IEEE semantics are
intended, including exact-zero sentinels and values known to come from the
same deterministic operation. Bit-reproducibility tests MUST compare
`to_bits()` representations; ordinary `==` does not establish bit identity.
Do not cite a shared comparison helper unless that helper exists in the
public API.

### 2.6 Portfolio evaluation execution

**Authority:** [ADR 0022](docs/decisions/architecture/0022-portfolio-evaluation-state.md).
**Detailed decision:** [0046](docs/decisions/architecture/0046-portfolio-execution-operational-contract.md)
accepts the operational conditions below.
[Clause map P01–P13](docs/decisions/INVARIANT_MAP.md#26-portfolio-evaluation-execution).

**Scope:** portfolio valuation, metrics-based attribution, scenario P&L,
scenario batches, replay, and factor-stress endpoint P&L. Ordinary position
valuation MUST use the request-scoped portfolio evaluation executor.
Public entry points and host-language bindings are adapters into
that executor; they MUST NOT own a second position-pricing loop, valuation
assembler, calibration cache, or scheduling policy.

**Required:**

- A request registers immutable market states separately from immutable
  portfolio/instrument states. A changed market, effective valuation date, or
  instrument set receives a distinct state identity.
- Each market state creates fresh rate and hazard recalibration caches. Cloned pricing
  options MAY share those caches only within the same immutable state; caches
  MUST NOT cross market states or requests.
- Evaluation jobs are distinguished by their exact requested metric menu, strict-risk
  policy, and base-currency policy. Each position computes its structurally
  applicable subset; inapplicable metrics MUST be explicitly reported, never
  represented as successful zeroes. Strict risk fails when an applicable
  requested metric fails. An empty explicit metric menu is PV-only and
  MUST NOT be promoted to the standard risk set. A degraded best-effort result
  MUST NOT satisfy a strict request. A workflow that explicitly rejects
  cross-currency positions MUST validate native currency before any FX lookup.
- A caller MAY explicitly supply an immutable prior result to seed a new
  request. Selective reuse requires compatible evaluation profiles, portfolio
  shape and relevant pricing configuration, plus an established relationship
  between parent and updated markets and an authoritative change set. If
  compatibility or the change set cannot be established, perform fresh
  valuation. This input is distinct from an executor retaining valuation
  caches across requests.
- FX invalidation MUST distinguish instrument repricing from
  base-currency conversion refresh so a reused native PV never leaves a stale
  base PV. Without an authoritative FX path manifest, any changed FX quote
  conservatively invalidates every position with any FX dependency because a
  cross may be triangulated through that quote. When conversion inputs change,
  refresh base-currency conversion for every reused position; proven unchanged
  conversion inputs do not require redundant refresh.
- One portfolio execution wave selects at most one Rayon axis: states, positions, or
  factors. Nested state-by-position or factor-by-position fan-out is forbidden.
  Instrument-internal Monte Carlo threading remains a separate coordination
  and performance-verification concern.
- Parallel work collects indexed results before a serial logical-order scan.
  Position/state order, first-error selection, and Neumaier aggregation order
  therefore remain deterministic.
- Evaluation state and caches are request-local. Do not add a global,
  thread-local, process-wide, persistent, or cross-request valuation cache.
  Immutable decoded-instrument input caches, static immutable registries, and
  UI presentation caches are outside this market-dependent financial-cache scope.
- Python adapters MAY hold typed borrow guards while extracting plain Rust
  references outside detached closures; native closures MUST capture only Rust
  state. This boundary does not require cloning whole typed portfolios.
  Native computation, JSON parsing, portfolio materialization, and native result
  serialization MUST release Python attachment after copying any required Python
  string input. Python object extraction and construction remain attached.
- A migrated workflow MUST delete its superseded compute path. Do not retain a
  runtime legacy/new switch, dormant fallback engine, or duplicate scheduler.
  Financially distinct reducers may remain, but any ordinary portfolio
  valuation they require enters through the shared executor.

**Evidence:** the [executor](finstack-quant/portfolio/src/evaluation/executor.rs),
[request-owned state](finstack-quant/portfolio/src/evaluation/state.rs), and
[evaluation keys and invalidation inputs](finstack-quant/portfolio/src/evaluation/plan.rs)
provide implementation evidence. Reuse checks do not independently establish
pricing-configuration or originating-market compatibility. The shared
`deserialize_json_with_context` helper also detaches without the required
string copy; this is a policy-compliance gap, not a demonstrated safety defect.
Complete workflow, cache-invalidation, model-internal scheduling, and binding
coverage remains subject to the
[portfolio follow-up](docs/decisions/FOLLOW_UPS.md#implementation-and-lifecycle-follow-up).

---

## 3. Sign and perspective conventions

**Authority:** [ADR 0007](docs/decisions/financial/conventions/0007-units-and-sign-conventions.md).
**Scope:** the perspectives and boundary conversions below; an operation's
explicit convention governs its inputs and outputs.

Signs describe a perspective, not an intrinsic property of an amount.
Interfaces MUST name their perspective and MUST NOT pass an amount between
perspectives without an explicit conversion.

### 3.1 Canonical perspectives

**Detailed decision:** [0044](docs/decisions/financial/conventions/0044-financial-perspectives-and-boundary-conversions.md)
accepts the named perspectives and explicit boundary conversions.
[Clause map S01–S02](docs/decisions/INVARIANT_MAP.md#3-sign-and-perspective-conventions).

These are canonical conceptual labels; they are not all Rust types yet.

| Perspective | Positive means | Primary uses |
|-------------|----------------|--------------|
| `EconomicCashFlow` | cash received by the entity or holder being valued | NPV, XIRR, DCF, instrument and Monte Carlo cash flows |
| `PortfolioExternalFlow` | client contribution into the portfolio | Modified Dietz / GIPS-style performance |
| `StatementMagnitude` | non-negative magnitude; the formula carries direction | CapEx, opex, tax, dividends, increase/decrease buckets |
| `RollForwardActivity` | signed additive change in `ending = beginning + Σ change` | corkscrews and additive reconciliations |
| `RegulatoryCollateral` | net collateral held by the bank | SA-CCR variation margin and NICA |

`AmountOrScalar` distinguishes currency amounts from unitless scalars; it does
not encode a sign perspective.

### 3.2 Required boundary conversions

**Detailed decision:** [0044](docs/decisions/financial/conventions/0044-financial-perspectives-and-boundary-conversions.md)
accepts each conversion and side convention below. Accepted
[0005](docs/decisions/standards/0005-python-wasm-parity.md),
[0007](docs/decisions/financial/conventions/0007-units-and-sign-conventions.md), and
[0038](docs/decisions/standards/0038-substantive-api-documentation.md) govern
equivalent host polarity; [0010](docs/decisions/financial/valuations-framework/0010-metric-identifiers.md)
governs metric meaning.
[Clause map S03–S12](docs/decisions/INVARIANT_MAP.md#3-sign-and-perspective-conventions).

- Dietz flows use `PortfolioExternalFlow`: contribution in is positive.
  XIRR/MWR uses `EconomicCashFlow`: contribution in is negative. Conversion
  requires `xirr_amount = -dietz_amount`.
- Real-estate CapEx schedules contain positive `StatementMagnitude` outflows.
  The pricer owns the single negation into `EconomicCashFlow`. Passing
  already-negative CFS CapEx into that schedule is invalid because it
  double-negates the outflow. `RealEstateAsset::validate()` rejects negative
  `capex_schedule` amounts.
- Retained-earnings dividends default to `MagnitudePositive`: a non-negative
  magnitude is subtracted by the identity. An explicitly selected
  `InflowPositive` convention instead adds the signed dividend directly.
  Dividend `+10` under the default and `-10` under the signed convention both
  reduce retained earnings by `10`. Callers MUST NOT infer the convention from
  the value or silently apply `abs()`. Corkscrew equity reductions are negative
  `RollForwardActivity`.
- Roll-forward `increases` and `decreases` are positive magnitude buckets;
  converting to a corkscrew produces `+increase` and `-decrease`.
- SA-CCR collateral and NICA are signed net amounts. Positive means the bank
  holds collateral. Margined replacement cost uses
  `max(V - C, TH + MTA - NICA, 0)`, where `C` is total recognized net collateral
  after applicable adjustments and already includes NICA. Component inputs
  MUST be explicitly defined and non-overlapping; NICA MUST NOT be added to
  total `C` a second time. The observed public `collateral` field is variation
  margin excluding NICA, and the kernel adds `nica` to form `C`. The
  [accepted explicit VM/NICA package](docs/decisions/financial/conventions/0044-financial-perspectives-and-boundary-conversions.md#accepted-amendment-explicit-collateral-components)
  requires public `variation_margin` and `nica` inputs with one derived `C`
  for RC/PFE; the old name and default paths require reconciliation. Full
  margined/unmargined methodology qualification remains open; numeric fields
  alone cannot prove upstream economic non-overlap.
- CDS option Call means payer protection; Put means receiver protection.
- Portfolio quantity greater than zero means long. P&L attribution is
  positive for a gain to the long holder. Sensitivity signs follow the
  relevant `MetricId` documentation.

Binding documentation MUST state the same polarity as Rust.
Avoid asset-class-ambiguous `long`/`short` aliases for `PayReceive`; prefer
`pay`, `receive`, or an explicit protection side.

**Evidence:** [real-estate validation](finstack-quant/valuations/src/instruments/equity/real_estate/types.rs),
[retained-earnings conventions](finstack-quant/statements/src/checks/builtins/retained_earnings.rs),
[corkscrew magnitude buckets](finstack-quant/statements-analytics/src/extensions/corkscrew/mod.rs),
and [SA-CCR replacement cost](finstack-quant/margin/src/regulatory/sa_ccr/replacement_cost.rs)
provide operation-specific implementation evidence. Retained earnings supports
the accepted explicit dividend selector, but negative magnitude inputs currently
produce warnings rather than rejection; enforcement remains a follow-up. The
SA-CCR public `collateral` field is documented as variation margin excluding
NICA, matching the kernel, which adds `config.nica`; a caller who supplies total
collateral in `collateral` would still double count NICA, so the kernel does
not enforce non-overlap. The kernel also implements
unmargined `max(V - C, 0)`; accepting the sign policy does not qualify either
branch's complete methodology. Non-overlap verification, named conversion
helpers or typed wrappers where needed, and cross-boundary verification remain
tracked in
[FOLLOW_UPS.md](docs/decisions/FOLLOW_UPS.md#implementation-and-lifecycle-follow-up).

---

### 3.3 Cash settlement and principal events

The [accepted core cash/balance/accrual contract](docs/decisions/financial/core-conventions/015-cashflow-semantics.md#accepted-decision-cash-settlement-principal-effects-and-accrual-metadata)
requires signed settlement cash in `amount`, including owner-qualified projected
cash. PIK/write-down events carry exact zero cash and explicit principal delta/date;
all principal-like rows require both fields, including deliberate zero, and other
kinds reject them. Positive delta increases the declared outstanding balance,
independently of cash perspective. Do not infer deltas from amounts/kinds, count
opening principal twice or confuse funding/redemption cash with par balance effects.
Use literal exhaustive kinds and coupon/repayment-like classifications; actual
repayment measures require their owner-selected eligibility and explicit par effects.

Payment, balance-effective, reset and accrual dates retain distinct meanings.
Noncash event date equals its explicit principal date, with no balance-date fallback.
Reset is an index observation satisfying the supported reset<=event/payment constraint,
not a margin-call or publication timestamp. Complete optional single-slice accrual
metadata records actual calculation endpoints, including explicit accrual adjustment,
separately from raw nominal references. It reuses the complete day-count context;
never fabricate whole-coupon provenance for a composite calculation. Zero accrual factors are literal
valid values; optional finite signed rates do not universally reconstruct cash.

Publish complete immutable validated values and restore/update atomically. Full
event interchange preserves principal dates/context; cash views exclude noncash,
while balance replay applies explicit deltas at their economic dates. Same-day
calculation sequencing remains product-owned rather than enum-order inference.
Runtime/product/host/state reconciliation and independent qualification remain open;
NPV inclusion/precision are governed separately by accepted record 016.
[Clause map S13–S16](docs/decisions/INVARIANT_MAP.md#3-sign-and-perspective-conventions).

### 3.4 Barrier classification and conditional payments

The [accepted shared barrier contract](docs/decisions/financial/core-conventions/020-barrier-classification.md#accepted-decision-barrier-classification-touch-and-conditional-payment-timing)
retains the four in/out and up/down classifications, Up/Down direction and
AtHit/AtExpiry timing with exhaustive predicates. Remove generic UpAndOut/AtHit
Default and inherited required-field fallbacks; relevant choices are explicit,
while fixed-timing or absent-cash contracts need no meaningless selector.

At an eligible observation, inclusive touch means x>=H for Up and x<=H for Down,
with exact comparisons and no epsilon or automatic level shift. Direction does
not establish current spot position or historical absence of a hit. Owners
supply monitoring windows, numerical validity, units and required history;
strict-condition contracts cannot silently become inclusive. No numerical hit
checker, monitoring/event framework or universal positivity rule is added here.

Timing schedules independently entitled cash and does not establish settlement.
First recognized hit, expiry, payment lag/date, discounting and any no-hit rebate
remain explicit owner contracts; unsupported or inapplicable selectors error or
are structurally excluded rather than silently ignored. An expiry-only zero
return is not proof that all pending cash/PV is zero. Keep strict canonical wire
values and the existing host payload/dispatcher boundaries without duplicate
classes or merging distinct monitoring/structural-credit/touch concepts.

Runtime/product/model/host reconciliation and independent qualification remain
pending; this contract does not adopt downstream pricing algorithms or accuracy.
[Clause map S17–S20](docs/decisions/INVARIANT_MAP.md#3-sign-and-perspective-conventions).

## 4. Date, day-count, and time conventions

**Authority:** [ADR 0007](docs/decisions/financial/conventions/0007-units-and-sign-conventions.md)
and [ADR 0008](docs/decisions/financial/conventions/0008-financial-defaults-and-cagr.md)
protect explicit conventions and defaults. The clock roles and boundary
rules below retain their existing scope; these links do not adopt every
instrument model or observed default.

A single instrument can require distinct clocks. Do not infer one clock from
another.

The [accepted actual-day kernel contract](docs/decisions/financial/core-conventions/001-actual-day-counts.md#accepted-decision-actual-day-conventions)
defines seven explicit variants, their context and signed/zero boundaries,
and numerical/error rules. The [accepted coupon-context contract](docs/decisions/financial/core-conventions/002-coupon-context-day-counts.md#accepted-decision-coupon-context-day-counts)
adds explicit contained ICMA reference periods and composition, end-year
ACT/365L and caller-supplied tenor context. The [accepted synthetic contract](docs/decisions/financial/core-conventions/003-thirty360.md#accepted-decision-synthetic-day-count-conventions)
adds five explicit 30/360 variants, required termination status and one public
interval API with consistent signed/zero semantics. These kernels do not select an
instrument's contractual basis. Runtime reconciliation and independent
qualification remain open.

The [accepted holiday-engine contract](docs/decisions/financial/core-conventions/004-holiday-engine.md#accepted-decision-holiday-engine-and-data-boundaries)
defines nominal-year rule semantics, full-day/weekend scope, fallible required-data
boundaries, exact cache equivalence and strict frozen-input/provenance controls.
The [accepted jurisdiction package](docs/decisions/financial/core-conventions/005-jurisdiction-calendars.md#accepted-decision-jurisdiction-calendar-purposes-and-coverage)
defines the 26 retained purposes, retirement of two generic IDs, legal epochs
and bounded published/forecast coverage. Runtime/data reconciliation and
independent qualification remain open.

The [accepted rolling/joint-calendar contract](docs/decisions/financial/core-conventions/006-business-day-adjustments.md#accepted-decision-rolling-and-joint-calendar-conventions)
requires explicit selection, six defined roll rules, checked searches without
the 100-day cutoff, deliberate identity/offset/count endpoints and joint
all-member coverage/error/provenance. Product-owned defaults retain their scope.

The [accepted tenor/date-arithmetic contract](docs/decisions/financial/core-conventions/007-tenor-date-arithmetic.md#accepted-decision-tenor-and-date-arithmetic-conventions)
separates unit-preserving positive tenors, checked calendar and completed-month
arithmetic, actual dated/contextual time, explicit approximations, nominal payment
ratios and quote compounding. Epoch days are signed civil-day counts; host date
and numeric conversion failures remain explicit. Removed inference paths and
affected financial callers require reconciliation and independent qualification.

The [accepted schedule contract](docs/decisions/financial/core-conventions/008-schedule-generation.md#accepted-decision-schedule-grids-events-and-failure-boundaries)
requires explicit anchored grid/stub choices, strict regular-anchor admission
for EOM/quarterly IMM and separate period-indexed payment/fixing recipes.
Raw grids do not select every product's actual accrual adjustment. Complete
atomic errors replace fallback/repair success; result/state shape and the
100,000-final-date resource boundary are validated. Product callers, removed
CDS inference and host/state paths require reconciliation and qualification.

The [accepted roll-date contract](docs/decisions/financial/core-conventions/009-contractual-roll-dates.md#accepted-decision-unadjusted-roll-dates-and-product-boundaries)
defines unadjusted all-month, quarterly and selected semiannual lattices with
explicit strict/inclusive searches and finite-domain errors. Product owners
select actual expiry, settlement and CDS tenor/regime dates; generic option-expiry
helpers are removed. Runtime/host/caller reconciliation and independent product
date qualification remain open.

The [accepted published-settlement contract](docs/decisions/financial/core-conventions/010-sifma-settlement.md#accepted-decision-published-settlement-dates-classes-and-coverage)
requires explicit SIFMA class and exact maintained Settlement cells, with checked
required-data errors and necessary-month strict-next semantics. Published cells
are independent of the bond-trading holiday calendar. Default/heuristic classes
and ordinal estimates are removed; frozen source/coverage and reviewed refreshes
require data/runtime/host reconciliation and independent qualification.

The [accepted FX quote/value-date contract](docs/decisions/financial/core-conventions/011-fx-date-and-quote-conventions.md#accepted-decision-explicit-fx-quote-metadata-and-value-dates)
preserves ordered quote units and explicit pip metadata, removes generic
spot/market inference and retains checked reciprocal arithmetic. Forward-value
dates require explicit calendar/roll choices, Natural or conditional business-EOM
behavior and open, strictly increasing value dates. Actual spot derivation,
product conventions and caller/host qualification retain their owning scope.

The [accepted reporting-period/fiscal/LTM contract](docs/decisions/financial/core-conventions/014-fiscal-periods-ltm.md#accepted-decision-reporting-periods-fiscal-context-and-ltm-windows)
requires positive half-open Gregorian/ISO and explicit-opening fiscal partitions,
actual-anchor clamping, literal fiscal labels and short fiscal week53. Retain one
explicit calendar context in validated plans; remove generic fiscal defaults and
country presets. Parsing, ranges, actuals cutoffs and temporal operations are strict
and context-aware, with representable bounds and a 100,000-period limit. ID ordering
is structural and total, not cross-context chronology; state validates canonical
bounds, continuity, uniqueness and actuals-prefix flags atomically.

Statistical annualization factors are separate from calendar counts and reporting
windows. Explicit observation frequency and real initial valuations remain required;
remove synthetic prior-date inference. LTM uses 12/4/2/1 complete M/Q/H/A reporting
periods or every calendar day in the literal twelve-month daily window; weekly LTM
and implicit business-day completeness are excluded. Require a present target,
precise missing-slot counts and distinct malformed/date errors. Coverage does not
certify eligible values/units or actual-only history, and incomplete windows cannot
be partial-summed or annualized. Runtime/host/caller/state reconciliation and
independent qualification remain pending.
[Clause map C18–C21](docs/decisions/INVARIANT_MAP.md#4-date-day-count-and-time-conventions).

### 4.1 Clock roles

**Detailed decision:** [0045](docs/decisions/financial/conventions/0045-financial-clocks-and-date-boundaries.md)
accepts separate clocks and their conventions; ADR 0008's dated-CAGR
default remains accepted.
[Clause map C01–C06](docs/decisions/INVARIANT_MAP.md#4-date-day-count-and-time-conventions).

| Clock | Rule |
|-------|------|
| Discount | Obtain the exact relative discount factor from curve dates and the curve's own base date/day-count. |
| Projection | Locate dates on the projection curve's clock; express forwards using the contractual index accrual fraction. |
| Accrual | Compute coupon accrual from the instrument or leg day-count. |
| Option/volatility | Use the calibration's reference date and time basis, with calendar context where required. |

Act/360 is commonly an accrual convention; it is not a universal rates-option
time basis. The full date-to-expiry mapping MUST be explicit through surface
metadata or a documented model contract, including its reference date and time
basis. The accepted [ADR 0045 mapping and information-cutoff package](docs/decisions/financial/conventions/0045-financial-clocks-and-date-boundaries.md#accepted-amendment-volatility-clocks-and-fixing-eligibility)
requires the shared `VolatilityClock`, explicit active-source resolution,
calibration-origin admission and supported same-interval variance preservation.
Implementation and independent qualification remain open. Do not infer a
universal fallback. Existing rate-option, equity, FX, and commodity defaults
require operation-specific evidence rather than an inferred universal basis;
the default inventory is tracked in
[FOLLOW_UPS.md](docs/decisions/FOLLOW_UPS.md#implementation-and-lifecycle-follow-up).

The accepted default for dated-window CAGR is specified by
[ADR 0008](docs/decisions/financial/conventions/0008-financial-defaults-and-cagr.md). It does
not redefine frequency-based annualization, XIRR, or an instrument's model
clock; implementation reconciliation is tracked with that decision.

### 4.2 Discount-factor bridge

**Detailed decision:** [0045](docs/decisions/financial/conventions/0045-financial-clocks-and-date-boundaries.md)
accepts the discount-factor bridge within the conditions below.
[Clause map C07](docs/decisions/INVARIANT_MAP.md#4-date-day-count-and-time-conventions).

For curve base date `b`, valuation date `v`, and payment date `T`, obtain the
relative discount factor from the supplied curve. When a single-time model
needs a continuously compounded rate on model time `t_model`, derive it from
that factor:

```text
relative_df = D(b, T) / D(b, v)
r_model = -ln(relative_df) / t_model
exp(-r_model * t_model) = relative_df
```

Both curve factors, `relative_df`, and `t_model` MUST be finite and positive.
The bridge preserves the factor mathematically; comparisons use justified floating-point tolerances.
It does not establish equivalence of entire model dynamics. Zero-time, expired,
and invalid-input behavior MUST follow explicit operation-specific contracts.
Discount with the curve factor directly whenever the model permits.

**Evidence:** `zero_rate_from_df` in
[the shared pricing helpers](finstack-quant/valuations/core/src/instruments/common_impl/helpers.rs)
and the [discount curve's date APIs](finstack-quant/core/src/market_data/term_structures/discount_curve/curve.rs)
provide the conversion primitives. The rate helper does not validate finite
model time and returns zero for `t <= 0` before validating the factor. Its
presence does not establish input enforcement or adoption by every pricer;
coverage is tracked in
[FOLLOW_UPS.md](docs/decisions/FOLLOW_UPS.md#implementation-and-lifecycle-follow-up).

### 4.3 Date boundaries

**Detailed decisions:** [0045](docs/decisions/financial/conventions/0045-financial-clocks-and-date-boundaries.md)
accepts dates, settlement, fixing-cutoff requirements, survival alignment and
the numerical-evidence requirement below;
[0048](docs/decisions/project/0048-reference-provenance-and-version-pinning.md)
accepts the detailed provenance, measurement-evidence and version-pinning standards.
[Clause map C08–C13](docs/decisions/INVARIANT_MAP.md#4-date-day-count-and-time-conventions).

**Required:**

- Valuation `as_of`, curve `base_date`, trade date, spot/settlement date,
  fixing date, and payment date MUST remain distinct.
- Rebase seasoned curves with relative date-based discount factors.
- Contractual settlement MUST apply the relevant spot lag, calendars,
  business-day convention, and end-of-month rule.
- Fixing requirements MUST follow an explicit valuation/publication cutoff.
  Required historical fixings MUST use the configured observations; missing
  required fixings MUST fail rather than silently project from a curve.
  Same-day treatment remains operation-specific and requires reconciliation.
- Credit survival time and discount time MAY use different day-counts and
  MUST be bridged through date-based survival probabilities and discount
  factors.

**Evidence:** the [autocallable](finstack-quant/valuations/equity/src/autocallable/pricer.rs)
and [cliquet](finstack-quant/valuations/equity/src/cliquet_option/pricer.rs)
Monte Carlo pricers measure model/vol time on the instrument day count and
use date-based curve discount factors. The named regression
`test_vol_clock_independent_of_curve_day_count` in
[the autocallable tests](finstack-quant/valuations/equity/tests/autocallable/test_day_count_basis.rs)
provides a bounded example, not complete clock-independence or cliquet
coverage. The cashflow fixing helper uses an available same-day fixing, while
the term cap/floor pricer assumes start-of-day and projects it; acceptance does
not settle that discrepancy. Verification gaps and the full volatility
date-to-expiry mapping are
tracked in [FOLLOW_UPS.md](docs/decisions/FOLLOW_UPS.md#implementation-and-lifecycle-follow-up).

Do not publish numerical error bands such as basis-point drift unless a
reproducible benchmark or golden test defines the setup and provenance.
Published measurements require recorded execution evidence identifying the
revision, command, inputs, comparator, error units and relevant build, runtime
and execution conditions. Distinguish measured error, sampling uncertainty and
test tolerance. Evidence for a named setup does not establish an accuracy bound
over a broader domain.

---

## 5. Error handling

**Authority:** [ADR 0036](docs/decisions/standards/0036-safe-rust-and-recoverable-failures.md)
defines safety and recoverable failures;
[ADR 0006](docs/decisions/standards/0006-structured-errors.md) defines error
meaning across interfaces. **Scope:** project library and binding code.

### 5.1 Enforced lints

**Decisions:** [0036](docs/decisions/standards/0036-safe-rust-and-recoverable-failures.md)
and [0034](docs/decisions/standards/0034-code-standards-authority.md) are accepted
for safety and shared enforcement;
[0049](docs/decisions/standards/0049-explicit-public-error-matching.md) is accepted
for explicit error classification and canonical delegation.
[Clause map E01–E03](docs/decisions/INVARIANT_MAP.md#5-error-handling).

**Evidence:** the [core library attributes](finstack-quant/core/src/lib.rs)
forbid unsafe code and deny `unwrap_used`, `expect_used`, `panic`, and
`unreachable` on production paths. The
[Python](finstack-quant-py/src/lib.rs) and [WASM](finstack-quant-wasm/src/lib.rs)
binding attributes forbid unsafe code and deny `unwrap_used`, `expect_used`,
and `panic`; they do not currently deny `unreachable`.
[Workspace configuration](Cargo.toml) denies `match_wild_err_arm`. That lint
checks `Err(_)` arms taking drastic actions such as panic; it does not enforce
exhaustive matching of arbitrary error enums. Broader classification coverage
remains a verification and enforcement follow-up.

**Required:** explicitly name project-owned public error variants where code
assigns their meaning, including category, structured code, required diagnostic
fields, conversions and specialized handling. Named variants may share a
branch. Both `_` and catch-all bindings such as `other` must not silently
assign a default meaning to future variants. Canonical classifiers and
conversions in the owning crate must enumerate their variants, including
nested error variants where those determine the mapping.

Unchanged propagation through `?`, forwarding, formatting and delegation to
Rust-owned `kind()`/`code()` contracts do not require duplicate variant tables.
A `#[non_exhaustive]` enum requires a catch-all outside its defining crate,
including sibling workspace crates. Keep the explicit classification in the
owner and preserve it through downstream delegation; do not claim that adding
a variant must break every consumer's compilation. Independently specialized
handling retains its own review responsibility.

Required unknown-variant handling for third-party non-exhaustive errors must
have a narrow, documented failure mapping appropriate to the operation and
retain original diagnostic/source context. It must not silently return
success, infer a known cause from message text, or turn a recoverable failure
into a panic. Existing generic adapter fallbacks are not approved merely
because a wildcard is required by the language.

Intentional panic-based assertions in test, example, and benchmark targets
follow ADR 0036. Crate inheritance and complete panic-path coverage are
separate verification questions tracked in
[FOLLOW_UPS.md](docs/decisions/FOLLOW_UPS.md#implementation-and-lifecycle-follow-up).

### 5.2 Required failure behavior

[Clause map E04–E05](docs/decisions/INVARIANT_MAP.md#5-error-handling) records
the accepted decisions for the requirements below.

Return the recoverable domain errors and diagnostic context required by
[ADR 0036](docs/decisions/standards/0036-safe-rust-and-recoverable-failures.md),
preserving their structured meaning under ADR 0006. Error documentation names
the public error callers can handle under
[ADR 0038](docs/decisions/standards/0038-substantive-api-documentation.md).

Use the actual public enums, such as
[`PricingError`](finstack-quant/valuations/core/src/pricer/errors.rs) and
[`finstack_quant_scenarios::Error`](finstack-quant/scenarios/src/error.rs).
Do not describe an error type that does not exist. Centralized host adapters
are defined in [Python errors](finstack-quant-py/src/errors.rs) and
[WASM utilities](finstack-quant-wasm/src/utils/mod.rs).

### 5.3 Indexing

**Decision:** [0036](docs/decisions/standards/0036-safe-rust-and-recoverable-failures.md)
is accepted for checked bounds and locally established kernel invariants.
[Clause map E06](docs/decisions/INVARIANT_MAP.md#5-error-handling).

**Required:** caller-controlled shapes and indices require checked access or
explicit precondition validation under ADR 0036. Use `.get()`,
`.first()`, checked split operations, or explicit precondition validation.
Numerical kernels may index directly only when the invariant is established
locally and covered by tests.

**Evidence status: partial.** `indexing_slicing` is not denied workspace-wide;
panic-family lints alone do not establish bounds safety. The required audit
and regression work is tracked in
[FOLLOW_UPS.md](docs/decisions/FOLLOW_UPS.md#implementation-and-lifecycle-follow-up).

---

## 6. Test discipline

Testing policy is maintained in the governing decisions:

| Policy | Authority |
| --- | --- |
| Behavior and invariant coverage; regressions that fail without the fix and identify the failure mode | [ADR 0039](docs/decisions/standards/0039-behavior-and-invariant-tests.md) |
| Independently justified expected values and protected tolerances | [ADR 0014](docs/decisions/project/0014-financial-reference-evidence.md) |
| Evidence for every promised execution mode and thread-count class; tolerance tests do not prove bit identity | [ADR 0021](docs/decisions/standards/0021-reproducibility-guarantees.md) |
| Shared definitions for scoped and full verification | [ADR 0034](docs/decisions/standards/0034-code-standards-authority.md) |
| Case-attributable golden metadata and permitted fixture/provenance locations | [ADR 0048](docs/decisions/project/0048-reference-provenance-and-version-pinning.md) |

[Clause map T01–T07](docs/decisions/INVARIANT_MAP.md#6-test-discipline) records
the accepted testing policy and reference details; enforcement remains subject
to verification.

The detailed reference-evidence conventions retained here are:

- Golden values MUST record provenance, methodology/version, valuation date,
  conventions, inputs, tolerances and their rationale, and expected units.
  Metadata must be attributable to each case. It may be inherited from fixture
  metadata, the test header, or the golden-suite README, with explicit
  case-specific overrides. Identify genuinely inapplicable fields with a
  reason rather than inventing dates or versions; methodology, inputs, units
  and tolerance rationale remain required.
- Rust and Python golden suites may share fixtures. Do not require all goldens
  to live directly under a Rust `<crate>/tests/` directory. Shared host runs do
  not create independent reference sources, and fixture location or metadata
  completeness does not establish financial correctness.

The [canonical tasks](mise.toml) implement scoped and full test runs;
[CONTRIBUTING.md](CONTRIBUTING.md) describes the workflow. Remaining evidence
and coverage work is tracked in
[FOLLOW_UPS.md](docs/decisions/FOLLOW_UPS.md#implementation-and-lifecycle-follow-up).

---

## 7. Public API, bindings, and deprecation

### 7.1 Host-surface parity

**Authority:** [ADR 0005](docs/decisions/standards/0005-python-wasm-parity.md)
defines equivalent host capabilities and the protected zero-backlog policy;
[ADR 0003](docs/decisions/standards/0003-canonical-public-names.md) defines
canonical names. Explicit justified host exceptions remain supported.

[Clause map H01–H04](docs/decisions/INVARIANT_MAP.md#7-public-api-bindings-and-deprecation)
also identifies the accepted documentation and independent-protection decisions.

A financial capability selected for host exposure has corresponding Python
and WASM interfaces under ADR 0005. This does not require exposing every
internal Rust function. Coordinated changes update the affected Rust API,
PyO3 registration, Python stubs and exports, WASM bindings and facade
declarations, examples, and parity contracts in the same change set.

**Evidence:** the [parity ledger](finstack-quant-py/parity_contract.toml),
[ledger tests](finstack-quant-py/tests/parity/test_parity_ledger.py), and
[WASM facade surface](finstack-quant-wasm/facade-surface.json) provide surface
coverage. Behavioral coverage and protection against self-modified ceilings
remain separate requirements under ADRs 0005 and 0016, tracked in
[FOLLOW_UPS.md](docs/decisions/FOLLOW_UPS.md#implementation-and-lifecycle-follow-up).

### 7.2 Deprecation policy

**Authority:** [ADR 0004](docs/decisions/project/0004-development-compatibility.md).
Approved development replacements remove obsolete paths rather than retaining
callable compatibility layers. Changes to established contracts still require
the authorization defined by [ADR 0015](docs/decisions/project/0015-authorized-change-scope.md).

[Clause map H05–H06](docs/decisions/INVARIANT_MAP.md#7-public-api-bindings-and-deprecation)
records the accepted replacement policy and required final-release decision.

The final-release compatibility strategy remains a separate decision required
before the first final release. Its unresolved support window, deprecation,
and migration mechanisms are tracked in
[FOLLOW_UPS.md](docs/decisions/FOLLOW_UPS.md#implementation-and-lifecycle-follow-up).

**Evidence status: legacy enforcement remains.** The
[annotation checker](scripts/check_deprecated_annotations.py) still runs from
`rust-doc`; its annotation-format checks do not implement the accepted
development replacement policy. Its remaining reconciliation is tracked in
[FOLLOW_UPS.md](docs/decisions/FOLLOW_UPS.md#reconciliation-items).

---

## 8. Credit factor model schema versioning

**Authority:** [ADR 0012](docs/decisions/standards/0012-serialized-contracts.md)
governs the wire contract, [ADR 0004](docs/decisions/project/0004-development-compatibility.md)
governs development replacements, and
[ADR 0041](docs/decisions/standards/0041-type-invariants-and-validation-boundaries.md)
governs declared validity and explicit validation boundaries.

**Detailed decision:** [0047](docs/decisions/architecture/0047-credit-factor-wire-and-validation-contract.md)
accepts the concrete wire and loader contract, including the required typed
`schema` field and marker rejection during deserialization. Implementation
and semantic-validation coverage remain subject to verification.
[Clause map W01–W09](docs/decisions/INVARIANT_MAP.md#8-credit-factor-model-schema-versioning).

The retained `CreditFactorModel` contract uses a namespaced string marker
rather than the numeric marker used by other result types:

```
finstack_quant.credit_factor_model/1
```

**Required:**

- Require the serialized `schema` field, typed as `CreditFactorModelSchema`,
  with `CreditFactorModelSchema::CURRENT` identifying the exact marker above.
  Reject missing, malformed and unsupported markers during deserialization;
  this does not promise first-error ordering when other fields are malformed.
  Do not retain `schema_version` as an alias or a second format.
- The v1 root is closed: unknown root fields are rejected by the Rust wire
  type and schema. The marker identifies this contract version exactly.
- The schema remains at
  [credit_factor_model.schema.json](finstack-quant/models/schemas/factor_model/1/credit_factor_model.schema.json),
  with the published URI under `https://finstack_quant.dev/schemas/factor_model/1/`.
  Moving Rust ownership must not change that stable identifier.
- Ordinary Rust deserialization does not call `CreditFactorModel::validate()`
  or establish complete semantic validity. Consumers MUST validate the current
  state, or use a validating loader, before financial use and after relevant
  mutation. Python/WASM JSON loaders and calibration entry points MUST retain
  this boundary. `from_json` performs semantic validation; `from_slice_strict`
  additionally applies resource limits and structured contract diagnostics.
  Semantic validation alone does not establish resource-limited loading.
- Authorized development root changes replace v1 in
  place and MUST update every in-workspace consumer and generated artifact in
  the same slice. No compatibility branch or alternate marker is retained.
  Final-release compatibility remains the separate ADR 0004 decision.
- Only explicitly open nested extension types that omit both
  `deny_unknown_fields` and schema-level `additionalProperties: false` may
  accept new keys without a root version bump. `CreditCalibrationDiagnostics`
  is one such extension point: unknown keys are accepted and discarded, without
  arbitrary extension-data round-trip preservation. Known fields retain their
  requiredness and meaning; `fold_ups` is used during decomposition. Additions
  that change computation require a coordinated semantic contract change.
- Field removal, type change, semantic change, required-field addition, or
  root-key addition MUST be made at the canonical Rust serde type, followed by
  deterministic v1 schema regeneration and complete in-workspace propagation.

**Evidence:** the [factor hierarchy](finstack-quant/factor/src/credit/hierarchy.rs)
currently uses `CreditFactorModelSchema::CURRENT` in the typed `schema` field;
plain deserialization rejects unsupported marker values. The same source's
`from_slice_strict` and `from_json` loaders also call semantic validation.
Its unit tests and the [schema-contract tests](finstack-quant/factor/tests/schema_contract.rs)
exercise closed roots and intentionally open diagnostics.

ADR 0047 supersedes the former `SCHEMA_VERSION`, `schema_version`, and
validation-only marker-rejection descriptions. Remaining work includes
current-state validation at consumer boundaries and after mutation, coverage
of computational `fold_ups` fields, and guidance reconciliation. The inspected
validator does not check `diagnostics.fold_ups`; decomposition checks an
out-of-range level later. Located validation calls and tests do not prove
complete enforcement. The Serde open-object guidance, Python macro wording
and root-field test wording now follow ADR 0047; the remaining enforcement
work is recorded in
[FOLLOW_UPS.md](docs/decisions/FOLLOW_UPS.md#reconciliation-items).

---

## 9. Authority and enforcement

The ownership model is defined by
[ADR 0013](docs/decisions/project/0013-documentation-sources.md) and
[ADR 0034](docs/decisions/standards/0034-code-standards-authority.md):

| Source | Responsibility |
| --- | --- |
| Accepted ADR | Agreed policy, scope, and rationale. |
| This catalogue or the designated API documentation | Precise operational contracts at the appropriate scope. |
| Canonical Rust wire types, parity ledger, and derived schemas | The designated structural sources under [ADR 0012](docs/decisions/standards/0012-serialized-contracts.md) and [ADR 0005](docs/decisions/standards/0005-python-wasm-parity.md), with their generated representations. |
| Types, lints, configuration, and tests | Implementation and enforcement evidence for those requirements. |
| [FOLLOW_UPS.md](docs/decisions/FOLLOW_UPS.md) | Outstanding implementation, reconciliation, verification, and unresolved scope. |

**Detailed evidence decision:**
[0048](docs/decisions/project/0048-reference-provenance-and-version-pinning.md)
accepts the catalogue-wide evidence attribution and status rules below.

Each detailed rule has one canonical home; other documents summarize and link
to it. A specifically accepted decision resolves a conflicting rule within
its scope. Where no accepted decision resolves the conflict, preserve the
competing evidence and obtain the scoped decision required by
[ADR 0015](docs/decisions/project/0015-authorized-change-scope.md). Code,
generated output, or passing self-modified tests cannot authorize a change.

Evidence claims identify their backing type, lint, schema, or test and the
conditions covered. Keep policy acceptance, implementation observation and
executed validation distinct. Source inspection or a located test does not
establish a passing run or complete enforcement. This distinction does not
require methodology-style status headers in every document.
The [clause map G01–G09](docs/decisions/INVARIANT_MAP.md#9-authority-and-enforcement)
records the corresponding decisions, including the catalogue's evidence and
scope rules. Every detailed invariant has an accepted correspondence; a related
link does not expand that decision's scope or approve unrelated methodologies.
Implementation, reconciliation and verification remain in FOLLOW_UPS.md.

Incomplete enforcement is not behavior callers can assume already holds.
Keep migration tasks and temporary verification status in FOLLOW_UPS.md, with
a short limitation here when it affects safe use of the contract. Independent
protection of contracts and their checks remains governed by
[ADR 0016](docs/decisions/project/0016-independent-contract-protection.md).

---

## 10. References

**Decisions:** [0014](docs/decisions/project/0014-financial-reference-evidence.md)
and [0026](docs/decisions/financial/governance/0026-methodology-decisions-and-inventory.md)
are accepted for financial evidence and methodology documentation;
[0048](docs/decisions/project/0048-reference-provenance-and-version-pinning.md)
is accepted for exact edition, effective calibration/parameter, convention
and benchmark pinning. A bibliography entry does not adopt a methodology or
authorize upgrading the implementation to that edition.
[Clause map R01–R02](docs/decisions/INVARIANT_MAP.md#10-references).

- [Semantic Versioning 2.0.0](https://semver.org/spec/v2.0.0.html) — public
  API and pre-1.0 compatibility semantics.
- [NVIDIA, Floating Point and IEEE 754](https://docs.nvidia.com/cuda/archive/12.4.1/floating-point/index.html)
  — non-associativity, FMA, and reduction-order reproducibility.
- [JSON Schema object reference](https://json-schema.org/understanding-json-schema/reference/object)
  — `additionalProperties` and closed-object behavior.
- [BCBS 279, *The standardised approach for measuring counterparty credit risk exposures*](https://www.bis.org/publ/bcbs279.pdf),
  paragraphs 143–145 — NICA and margined replacement cost.
- [2020 Global Investment Performance Standards for Firms](http://www.gipsstandards.org/wp-content/uploads/2021/03/2020_gips_standards_firms.pdf)
  — external cash flows and time-weighted performance.
- [ISDA SIMM v2.8+2512](https://www.isda.org/2026/06/12/isda-publishes-isda-simm-methodology-version-2-8-2512/)
  — effective 11 July 2026. SIMM code and goldens MUST pin their implemented
  methodology and effective parameter set, including any overrides, rather
  than relying on “current”. Use a published calibration tag where available;
  otherwise identify the exact edition and immutable source/parameter set.
  The implemented and test-pinned edition is historical v2.6; citing v2.8+2512
  here does not adopt it or authorize a methodology upgrade.
- [AFMA, *Interest Rate Options Conventions* (June 2025)](https://www.afma.com.au/getattachment/Standards/Market-Conventions/Sections/Content/Interest-Rate-Options-Conventions-2025-06.pdf?lang=en-AU)
  — market-specific Actual/365 quotation practice; not a universal global
  volatility-time rule.
- Hagan, P. S., & West, G. (2006) — monotone-convex interpolation.
- Brigo & Mercurio (2006), Andersen (2008) — HW1F and QE-Heston.

Model-specific modules and golden fixtures MUST cite the exact edition,
calibration, convention, and benchmark used, with genuinely inapplicable
fields identified explicitly. Parameter provenance includes the effective
overrides or an immutable identity for the resolved parameter set. A wire-format
schema version does not identify the financial methodology or calibration;
this policy does not require a new runtime metadata API. The inspected SIMM
implementation exposes historical v2.6; reconciliation of bibliography and
provenance guidance remains in
[FOLLOW_UPS.md](docs/decisions/FOLLOW_UPS.md#implementation-and-lifecycle-follow-up).
