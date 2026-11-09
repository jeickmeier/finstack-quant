# CORE-MARKET-03: Continuous-zero shocks and dated frozen-source conditioning

- Decision status: **Accepted — complete factor-source shock and conditioning package adopted 2026-11-08.**
- Implementation status: **Observed** in the owning paths identified below; the accepted factor/view contract is not implemented.
- Validation status: **Source/reference evidence and independent financial/clock/consumer reviews inspected; standalone rational/high-precision expression examples executed 2026-11-08; production qualification pending.** Existing Rust/host tests were inspected or located, not executed. Expression checks do not certify production behavior or a universal accuracy bound.
- Audit date: **2026-11-05**. Source revision: `7e82fd03ad98346727769e88dd5ac5d84b56e9fc`.
- Scope: the implementation listed here; the [slice inventory](README.md) and [coverage ledger](coverage.json) identify exact file coverage.

## Scope and accepted authority

This decision owns direct discount/projection rate-shock units and geometry,
complete-evaluator composition, and dated conditioning of a frozen source.
It preserves accepted [CORE-MARKET-01](discount-evaluation.md#accepted-decision-explicit-discount-sources-and-checked-rate-extraction),
[CORE-MARKET-02](forward-projection.md#accepted-decision-explicit-index-projection-factors-and-contractual-rates),
[ADR0045's clocks](../conventions/0045-financial-clocks-and-date-boundaries.md),
[rate units](../core-conventions/018-rate-units.md) and numerical interpolation
contracts. The accepted discount and index projection roles remain distinct.
Their explicit choices, positive-factor admission, contractual accrual and
analytic derivative rules are not reopened.

Quote shocks with recalibration, stochastic future curves, theta decomposition,
market-context-wide aging and observed fixing availability remain owned by
calibration, models, [record20](context-lifecycle.md) and products. This package
does not adopt their Draft methodologies. Shared bucket geometry is specified
here, but other asset families retain their own shocked financial quantities,
units and domain constraints. [Record06](calibration-provenance.md) owns
calibration provenance and fit-state disclosure.

## Foundational findings

Accepted02 replaces raw forward-rate seeds with projection factors. Adding bp
to factor pillars, scaling their values as if they were rates, or retaining the
old seed/grid roll would change the financial operation. A common explicit
continuous-zero shock supplies a coherent factor-based replacement.

Generic date rebasing also fails for some admitted source clocks. Under US
30/360, B=2025-01-29, v=2025-01-30 and T=2025-01-31 give
tau_B(v)=1/360 and tau_B(T)=2/360, while tau_v(T)=0. A new scalar curve on
tau_v cannot preserve both original dated factors. Frozen-source conditioning
must retain the original dated mapping rather than sum pairwise clock offsets.

Finally, a partition of unity in bucket weights guarantees first-order
sensitivity additivity. It does not guarantee that separate finite bumped PV
changes sum exactly to a parallel finite bumped PV change.

## Accepted decision: explicit zero-rate shocks and dated conditioning

Accepted on **2026-11-08** after financial, clock and consumer review.
Runtime/caller/host reconciliation and independent production qualification
remain pending. Observed implementation evidence remains separate below.

1. **Name the financial operation and preserve the factor role.** A direct
   rate-curve shock is an additive continuously compounded zero-rate parameter
   shock, for either a discount or an explicitly selected index projection
   factor source. It is not a quote recalibration or a universal additive
   change to every simple index fixing. Require the curve role and ID at
   dispatch; reject missing or ambiguous resolution instead of cross-namespace
   precedence. Preserve accepted02's index/source association. Full context
   targeting and other asset-family shocks remain separately owned.

2. **Admit explicit rate units and literal geometry.** Reuse BumpSpec with
   explicit mode, units, value and shape. Support Additive RateBp (value/10000),
   Percent (percentage points, value/100) and Fraction (annual decimal rate).
   Reject Multiplicative and Factor for both rate-factor roles; remove the old
   parallel multiplicative forward-seed path. All values and coordinates must
   be finite. Validate the complete request and supported units before zero
   shortcuts, normalize signed zero, and reject a nonzero magnitude lost to
   zero during unit conversion. Triangular neighbors satisfy p<k<n where
   present, with finite representable positive widths; signed coordinates are
   allowed. Use one checked exact-width weight owner without the 1e-10 floor,
   epsilon snapping or minimum spacing. Interior weights are zero outside
   [p,n], rise linearly from0 to1 on [p,k] and fall to0 on [k,n]. A missing left
   neighbor gives weight1 left of k; a missing right neighbor gives weight1
   right of k; a sole bucket is Parallel. Preserve exact endpoint weights and
   report arithmetic failures rather than widen the bucket or clip the result.

3. **Anchor each new shock on the declared source coordinate.** Retain the
   original source base B and complete clock t=tau_B(T). Let the current
   conditioning reference be v, with a=tau_B(v); initially v=B and a=0.
   The new shock uses x=t-a and multiplies the complete current factor by
   exp(-b*x*w(x)). For an unconditioned source this is C_b(t)=C(t)exp(-b*t*w(t)).
   The coordinate is a difference of original-source year fractions, including
   signed pre-reference coordinates. It is not silently replaced by tau_v(T),
   coupon/index accrual, model time or calendar tenor. Bucket inputs and host
   docs must state that coordinate. At x=0 the factor remains1 after admission;
   this does not identify a zero-rate quote at zero.

4. **Retain one complete evaluator and a merged adjustment.** Keep original
   factor interpolation I, its source pillars/clock/style/tails, the current
   reference date, and a finite merged piecewise-linear adjustment A. For
   shocks j anchored at a_j, A is the piecewise derivative of
   -sum_j b_j*(t-a_j)*w_j(t-a_j). Evaluate the selected complete view as
   C_v(T)=[I(t)/I(a)]*exp(integral_a^t A(u)du). Reuse the existing small
   adjustment/interpolation owners; do not refit reporting pillars or build a
   recursive stack of curve wrappers. Split and integrate on actual adjustment
   pieces with checked compensated accumulation; preserve exact breakpoints,
   including coincident ones, without tolerance merging. Older shocks retain
   their anchors when the reference changes. A new shock is anchored at the
   current reference. The retained canonical I still has I(0)=1; a conditioned
   view is explicitly a dated view, not a canonical scalar curve with new
   DayCount(v,T) coordinates. No second seed/grid or multiplicative mode.

5. **Admit the selected complete factors and compute the requested result.**
   At a factor query admit I(t) and I(a) under the original tails/context, then
   require the final C_v(T) to be finite and positive. Validate even for T=v
   before returning1. Compose the factor ratio and exponential with checked
   stable scaling; an unrequested standalone quotient or exponential need not
   be representable when the final requested factor is. Do not require a
   hypothetical shocked curve normalized at an inactive old origin: that is
   not the selected evaluator. Conversely, interval/rate queries must admit
   both complete selected-view endpoint factors before using accepted01/02's
   stable factor-log difference, expm1 and explicit contractual alpha. A local
   adjustment integral must not bypass a failed absolute-factor endpoint.
   Positive subnormals remain valid; zero underflow, infinity, NaN and required
   nonzero results lost to zero are checked failures. Negative rates need no
   clipping or relaxation preset. Check required reporting samples and new
   shape breakpoints within the retained source's admitted time coverage before
   publishing a replacement; geometry outside that coverage does not extend
   Error tails or require an out-of-coverage factor query. These finite checks do
   not establish whole-domain positivity or representability. Failed changes
   leave the original curve/context unchanged.

6. **Differentiate the actual shocked evaluator.** On the retained source-year
   coordinate, instantaneous_forward=-I'/I-A. Thus a triangular zero shock
   changes the instantaneous rate by b*(w+x*w'), not just b*w. Apply accepted01's
   complete-evaluator one-sided rules at all source and adjustment breaks;
   a reference date/reporting knot does not create a new interpolation endpoint.
   Use the same admitted source and selected complete-factor domain for local
   queries, with checked derivative/quotient range. A parallel continuous-zero
   shift also shifts the instantaneous forward by b, but its effect on a term
   rate is generally nonlinear. For a projected interval, the growth factor
   is multiplied by exp(b*[x_E*w(x_E)-x_S*w(x_S)]); annualize its growth
   increment once on that index's explicit alpha. No instantaneous or term-rate
   shock API is added under an ambiguous forward-shock label.

7. **Implement roll as dated frozen-source conditioning.** A conditioning
   request names the new reference date w explicitly. It preserves the original
   source B, clock/context, interpolation, tails and accumulated A, and replaces
   the reference with w. Before changing it, admit the current complete factor
   C_v(w); the new view satisfies C_w(T)=C_v(T)/C_v(w) in real arithmetic.
   Flatten state to the same I/A plus the new reference, never nested normalized
   curves or summed day-count offsets. Repeated conditioning has the same
   mathematical final evaluator when all required transitions are admitted;
   intermediate range/domain failures can make a particular path unavailable.
   Return a dated conditioned view with source_base and reference_date clearly
   distinct. It does not inherit a fresh numeric-clock/zero-rate API or masquerade
   as a canonical source with base_date=w. This constructs a new selected
   evaluator, distinct from requesting a relative factor from the old view.
   Old-view relative-factor queries retain accepted01/02's admission of both
   old complete endpoint factors. After construction, new-view queries apply
   point5 to I(t), I(tau_B(w)) and the requested new complete factor; they do
   not require the old view's unrequested C_v(T). The quotient identity is a
   real-arithmetic statement, not a promise of identical numerical domains.
   Reuse checked numeric primitives and the smallest necessary dated adapter;
   keep raw numeric source
   coordinates confined to their original owner. Products continue to resolve
   valuation/fixing cutoffs separately from this normalization date.

8. **Keep retained coverage and composition explicit.** Support forward and
   backward references, including beyond the final reporting pillar, whenever
   the current complete evaluator admits the reference under its original
   tails/calendar coverage. Do not fail solely because reporting pillars have
   expired, discard retained source points, fabricate a terminal pillar or
   silently refit. Error tails and unusable linear extensions still fail.
   Existing source/adjustment breaks retain their dates and derivative sides.
   Additive shocks with fixed anchors commute in real arithmetic; use
   deterministic checked accumulation without promising bitwise order
   independence or exact cancellation after inverse shocks. Conditioning after
   an old shock retains that old dated shape. Applying a newly anchored bucket
   after conditioning is generally a different operation. A new parallel shock
   is the special case whose coordinate increment is independent of the anchor.
   State the ordered operations in replay; a source replacement is a new source
   and explicitly drops the prior transform rather than pretending to preserve it.

9. **Qualify bucket sensitivities and risk provenance.** A partition-of-unity
   claim requires one coherent ordered bucket set with matching neighbors and
   flat outer wings. Then composing all equal-size bucket log-factor shocks
   reproduces the parallel shock in real arithmetic, and first-order directional
   derivatives add. Separate finite repricing differences need not add because
   of valuation nonlinearity, cross terms, solver noise and floating-point error.
   Report finite-difference method/bump and qualify any residual tolerance at
   the risk owner; do not assert exact summed finite DV01. Direct parameter risk
   and quote-recalibrated hedge risk must be selected and reported distinctly.
   Retained calibration recipes describe original construction, not proof that
   transformed or conditioned sources still fit the original quote set.

10. **Reconcile actual scenario, calibration and host routes.** Replace direct
    raw forward-node bp bumps with this complete-factor transform. Quote-risk
    routes must use explicit discount/index dependencies and return their fitted
    factor source; currency-prefix/sole-curve guesses and raw-rate delta overlays
    cannot survive as implicit alternatives. Any discrepancy-preserving overlay
    requires a separately selected, qualified financial definition. Propagate
    checked failures through solver callbacks instead of NaN. Keep Rust canonical
    and reconcile Python/WASM inputs, date-only view behavior, explicit role,
    source-coordinate metadata, units, errors, schema, generated declarations
    and parity in the same coherent implementation slices. Existing *_bp names
    must state continuous-zero parameter risk or explicit quote risk in their
    contract. Do not publish an incomplete mixture of old and new meanings.

11. **Persist authoritative source and view state strictly.** Store original
    canonical I source state, original clock/calendar, merged A, current dated
    reference and explicit role/association at their owners. Rebuild numerical
    caches and normalization; supplied caches and derived reporting factors are
    not authoritative. Validate finite ordered adjustment pieces, source-domain
    consistency, reference admission and complete required samples atomically.
    Reject missing/unknown/retired fields and old seed/numeric-offset/multiplicative
    labels without aliases or migration. Normalize identity state without
    interpreting approximate equality as cancellation. Runtime reconciliation,
    independent numerical/financial qualification and any separate model/context
    aging approvals remain pending after methodology adoption.

## Rationale and alternatives

One continuous-zero parameter shock uses the same factor mathematics for both
accepted rate-source roles and preserves the original interpolation off pillars.
Literal bucket geometry removes the current tiny-width disagreement without
introducing a new tolerance policy. A dated reference over the retained clock
supports every admitted source clock without a clock-remapping framework or a
false new scalar origin. It is a frozen-data calculation, not a forecast of
future market curves or a complete passage-of-time valuation scenario.

Instantaneous-forward shifts, direct simple-index quote shifts, factor-node
perturbations and quote bumps followed by recalibration are different financial
operations. This package does not add them as interchangeable generic modes.
A freshly rebased scalar curve could only be exact on qualified clocks; retaining
one dated view avoids a second roll strategy and its clock-dependent dispatch.

## Observed choice and rationale

Discount and forward curves retain the original interpolator and compose an analytic adjustment rather than perturbing only visible pillars and refitting. This preserves the requested shock between sparse knots and after rolling. The implementation contains both this continuous method and knot-weighted methods for other curve families; equivalence is not universal.

## Observed algorithm and controls

`BumpSpec` maps additive rate basis points to value/10,000, percentage points to value/100, fractions unchanged, and multiplicative factors unchanged. Other standard mode/unit pairs are rejected. Values must be finite; triangular neighbors, when present, must be finite and strictly ordered around the target. Negative time buckets are not prohibited. First buckets are flat at weight one to the left of target, last buckets to the right, interior weights rise/fall linearly and vanish outside neighboring buckets. With neither neighbor the constructor selects Parallel. The generic weight helper floors denominators at 1e-10.

For an unrolled discount curve an additive continuous-zero shock is `D_b(t)=D(t)*exp(-b*t*w(t))`. The shared adjustment stores its derivative `d[-b*t*w(t)]/dt` as piecewise-linear segments; its integral is accumulated using Neumaier compensation. For forward curves the added function is `b*w(t)` itself. Breakpoints from successive shocks are merged, sorted and deduplicated; redundant identical linear pieces are dropped. A parallel multiplicative forward shock scales both source scale and existing additive adjustments. Multiplicative discount shocks and multiplicative forward key-rate shocks are unsupported.

With original source interpolator `I`, offset `o`, and adjustment `a`, discount evaluation is `I(o+t)/I(o)*exp(integral(o,o+t,a))`, with exact one at zero. Forward evaluation is `scale*I(o+t)+a(o+t)`. Rolling advances offset using the curve's signed day-count fraction and refreshes discount normalization; it preserves previous shocks at their source times. Newly requested shocks are anchored to the current offset. Visible pillars are updated for reporting. Serialization carries source points and transform; rebuilding with replacement knots deliberately discards the old transform. Restoring a transform checks visible samples against evaluation at relative tolerance 1e-12 times `max(abs(actual),abs(sample),1)`.

## Observed failure and limitations

Discount shocks require positive finite DFs at visible pillars and at a triangular peak, then set `allow_non_monotonic=true` and remove the forward floor. Forward shocks require finite values at those locations. Neither proves finite/domain-valid results for every possible time or extrapolation horizon. Roll drops expired public knots and fails if no usable future pillars remain, even if the source interpolator can extrapolate. Negative rolls are accepted where the rebuilt state remains valid.

The continuous adjustment uses exact bucket widths while the generic knot-weight helper floors widths at 1e-10. Thus extremely close valid buckets can produce different shock weights in continuous discount/forward paths versus hazard/price/inflation knot paths. This is a concrete consistency gap, not an approved approximation.

## Observed alternatives, evidence

Knot-only refitting is implemented by other families and is cheaper conceptually but changes interpolation between pillars. Quote bump plus full recalibration is an economically different risk definition. Neither should silently replace this definition. [Tuckman–Serrat](../../../REFERENCES.md#tuckman-serrat-fixed-income) supplies risk context; exact shock-shape and serialization policies are repository choices.

Owning code: [bump specification](../../../../finstack-quant/core/src/market_data/bumps.rs), [piecewise-linear adjustment](../../../../finstack-quant/core/src/market_data/term_structures/common/adjustment.rs), [discount evaluation](../../../../finstack-quant/core/src/market_data/term_structures/discount_curve/evaluation.rs), [discount transformation](../../../../finstack-quant/core/src/market_data/term_structures/discount_curve/transform.rs), [forward evaluation](../../../../finstack-quant/core/src/market_data/term_structures/forward_curve/evaluation.rs), [forward transformation](../../../../finstack-quant/core/src/market_data/term_structures/forward_curve/transform.rs). Inspected [discount tests](../../../../finstack-quant/core/tests/market_data/curves/discount.rs) check off-pillar exponential formulas and bucket partition; inspected [forward tests](../../../../finstack-quant/core/src/market_data/term_structures/forward_curve/tests.rs) verify a narrow shock missed by the Simpson source grid still has exact area. Composition/serde tests are invariants, not external risk benchmarks.

## Caller and primary-reference evidence

| Inspected owner | Reconciliation obligation |
| --- | --- |
| [Discount transform](../../../../finstack-quant/core/src/market_data/term_structures/discount_curve/transform.rs), [evaluation](../../../../finstack-quant/core/src/market_data/term_structures/discount_curve/evaluation.rs) and [forward transform](../../../../finstack-quant/core/src/market_data/term_structures/forward_curve/transform.rs) | Replace pairwise numeric-offset rebasing/expired-pillar reconstruction with the dated reference; use one factor transform for both roles. |
| [Bump specification](../../../../finstack-quant/core/src/market_data/bumps.rs), [adjustment](../../../../finstack-quant/core/src/market_data/term_structures/common/adjustment.rs) | Share literal weights, checked normalization and finite arithmetic; correct the exact finite-DV01 additivity claim. |
| [Context bump dispatch](../../../../finstack-quant/core/src/market_data/context/ops_bump.rs) | Preserve explicit rate role and reject ambiguity; keep context-wide targeting policy record20-owned. |
| [Scenario adapters](../../../../finstack-quant/scenarios/src/adapters/curves.rs), [recalibration overlays](../../../../finstack-quant/calibration/src/recalibration/rates.rs) | Remove raw-rate node/delta arithmetic and dependency guesses; distinguish complete-factor parameter shocks from quote recalibration and propagate callback failures. |
| [Python context](../../../../finstack-quant-py/src/bindings/core/market_data/context.rs), [WASM context](../../../../finstack-quant-wasm/src/api/core/market_context.rs) | Replace shape/expired-grid roll promises with the supported dated conditioning and source-coordinate contract. |

The [US 30/360 implementation](../../../../finstack-quant/core/src/dates/daycount/thirty360.rs)
provides the concrete non-additive-clock counterexample; this decision preserves
the accepted day-count formula rather than modifying it to make rolling additive.

QuantLib explicitly distinguishes [zero-rate spread structures](https://raw.githubusercontent.com/lballabio/QuantLib/master/ql/termstructures/yield/zerospreadedtermstructure.hpp)
from [instantaneous-forward spread structures](https://raw.githubusercontent.com/lballabio/QuantLib/master/ql/termstructures/yield/forwardspreadedtermstructure.hpp).
Its [implied term structure](https://raw.githubusercontent.com/lballabio/QuantLib/master/ql/termstructures/yield/impliedtermstructure.hpp)
normalizes by an old-source reference factor but uses a numerical time offset
and a live underlying source. That is precedent for factor-ratio conditioning,
not qualification of this decision's immutable/date-mapping guarantees.
[Strata's generic parallel shift](https://strata.opengamma.io/apidocs/com/opengamma/strata/market/curve/ParallelShiftedCurve.html)
acts on curve y-values; applying it to factor pillars would have a different
meaning from this rate shock. Exact bucket geometry, composition and supported
view boundaries are explicit repository decisions requiring qualification.

## Independent examples and required verification

On 2026-11-08 standalone Fraction arithmetic checked exact units, geometry,
clock and exponent identities. Decimal at precision100 evaluated exponential
expressions. No library module was imported and no production suite was run.

| Example | Independent result / implication |
| --- | --- |
| Additive 1bp, 0.01 percentage points, or decimal0.0001 | All normalize to b=1/10000. |
| Bucket p=0,k=1e-12,n=2e-12, query t=5e-13 | Exact weight is1/2; the observed 1e-10 floor gives1/200. This is a geometry change, not a harmless numerical guard. |
| Rising bucket p=1,k=2,n=3 at x=3/2 | w=1/2 and w'=1; zero shift b/2 implies instantaneous shift2b. |
| Base period growth1.05, alpha=1, parallel b=1bp over one source year | New simple rate is1.05*exp(0.0001)-1≈0.050105005250175; the change is about1.0500525bp, not exactly1bp. |
| Two bucket weights1/2 and x=b*T>0 | Separate losses minus parallel loss equal (1-exp(-x/2))²>0, although the first derivatives add and composed factor shocks agree. |
| US30/360 B=2025-01-29,v=Jan30,T=Jan31 | Old coordinates1/360 and2/360 differ; the new-base tau_v(T)=0 collides with v. Fresh scalar rebasing cannot preserve both. |
| Flat I=1; current source coordinate a=100; new parallel b=10; t=a+1/100 | Complete selected view factor is exp(-1/10). A hypothetical old-origin normalized factor exp(-1000) underflows; that inactive normalization is not required. |
| First bucket target1,next2; old reference0, new reference1/2, maturity3/2 | Shock-then-condition exponent increment is -b/4; condition-then-new-shock is -b. Nonparallel order changes the requested financial operation. |
| I(t)=1, conditioning reference a=1/2 then c=3/4, A=-b constant | Final factor at t=1 is exp(-b/4), equal to direct conditioning at c in real arithmetic when transitions are admitted. |

These are bounded formula checks, not production defect reproductions or a
universal error bound. Independent qualification must cover:

- Strict unit/mode/shape and typed-target admission, tiny/nonfinite/close bucket
  inputs, exact joins/wings, normalization underflow, zero identities and
  coherent bucket-set partition versus arbitrary triples.
- Positive/negative/zero rates; stable final factor composition when an isolated
  exponential or ratio is unrepresentable; complete endpoint admission, extreme
  source factors/alpha and derivative range; no hidden clipping or callback NaN.
- Analytic one-sided derivatives at source/adjustment/coincident breaks, retained
  tails and reporting-only/reference points; off-pillar shock shape and narrow
  adjustments against independent expressions.
- Original-clock dated mapping, US30/360 collisions, Bus/252 calendar/context,
  spot/pre-base references, backward and beyond-grid conditioning, Error tails,
  expired reporting grids and admitted-transition composition limits.
- Shock-before/after-condition order, fixed-anchor addition and cancellation,
  strict state/cache reconstruction, atomic failures, source replacement and
  host/serialization behavioral parity.
- Independent direct-parameter sensitivities versus quote-recalibrated risk,
  finite-difference convergence/residual tolerances, explicit index dependencies,
  and original-fit provenance versus transformed-source market qualification.

## Follow-up tracking

Pending decisions, implementation reconciliation and validation work are tracked in the
[core-market follow-ups](../FOLLOW_UPS.md#core-market-data).
