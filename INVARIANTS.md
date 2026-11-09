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
