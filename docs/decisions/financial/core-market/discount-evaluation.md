# CORE-MARKET-01: Discount factors, zero rates, and relative discounting

- Decision status: **Accepted — complete discount-evaluation package, 2026-11-08.**
- Implementation status: **Observed; reconciliation pending** in the owning paths identified below; discrepancies are stated explicitly.
- Validation status: **Source/reference evidence inspected; independent 200-digit formula and exact-rational examples executed 2026-11-08; production qualification pending.** Existing Rust/host tests were inspected or located, not executed. Expression-level examples do not certify the production implementation or a domain-wide accuracy bound.
- Audit date: **2026-11-05**. Source revision: `7e82fd03ad98346727769e88dd5ac5d84b56e9fc`.
- Scope: the implementation listed here; the [slice inventory](README.md) and [coverage ledger](coverage.json) identify exact file coverage.

## Scope and existing authority

This record owns `DiscountCurve` admission and evaluation, its financial
constructors, zero/interval/instantaneous rate extraction, and the common
`Discounting` relative-factor boundary. It carries the accepted
[compounding](../core-numerics/compounding.md),
[value-interpolation](../core-numerics/value-interpolation.md),
[forward-interpolation](../core-numerics/forward-interpolation.md) and
[separate-clock](../conventions/0045-financial-clocks-and-date-boundaries.md)
contracts into this financial owner. Their accepted choices are not reopened.

[Forward projection](forward-projection.md) owns contractual term/reset growth,
simple-rate annualization and arithmetic averages.
[Rate shocks](rate-shocks.md) owns transformation composition and roll/stress
economics. [Calibration provenance](calibration-provenance.md) owns recipe replay
meaning, and [parametric discounting](parametric-discount.md) owns NS/NSS model
formulas and admissibility. [ADR 0032](../calibration-targets/0032-curve-interpolation-and-calibration.md)
and the calibration target retain instrument fitting and per-market selection.
Changing the shared trait requires mechanical integration with those owners;
it does not adopt their remaining Draft methodology.

## Accepted decision: explicit discount sources and checked rate extraction

Accepted on **2026-11-08** after whole-package review. Runtime, caller and host
reconciliation and independent production qualification remain pending. The
observed implementation below remains separate from the accepted contract.

1. **Require explicit construction choices.** Retain one immutable owned
   `DiscountCurve` with explicit base date, curve day count/calendar, interpolation
   and extrapolation. Require these choices in the builder and generic financial
   factories, canonical state and host inputs; no identifier/currency inference
   or inherited Act/365F, MonotoneConvex or tail default. Keep the five existing
   methods, with their formulas owned by the accepted numerical records. No one
   method is universally approved for every currency, quote set or product.

2. **Separate financial normalization from data repair.** Constructor inputs
   must be finite, strictly time-ordered distinct pillars with t>=0 and D>0.
   Positive subnormal DFs and D>1 are admitted. Do not sort, deduplicate, merge,
   snap or impose a relative minimum knot gap. When the first supplied t>0,
   prepend the exact normalization anchor (0,1); when t=0 is supplied, require
   D=1 exactly. At least one positive-time pillar is required, giving at least
   two points after anchoring. This documented normalization is not a fitted
   short rate. Canonical persisted state must already contain the anchor and
   ordered grid; restoration must not insert or repair it. Required numerical
   widths/caches still have their accepted checked-range limits.

3. **Use positivity as the generic financial domain, without a rate regime.**
   Remove MarketStandard, NegativeRateFriendly and Raw validation presets, their
   stored allow_non_monotonic/forward-floor controls and preset parsing. Do not
   replace them with a new branded preset or hidden currency-dependent bound.
   Negative, zero and positive rates, increasing DFs and mixed pillar shapes are
   eligible. Required DFs must remain finite and strictly positive at evaluation;
   a nonpositive extension or unusable numerical result errors without clipping
   or switching style. Positive pillars and successful sampled queries do not
   prove whole-curve positivity, nonnegative forwards or no arbitrage. Product
   and calibration owners retain explicit, separately qualified financial
   constraints; removing the generic restrictions does not approve their inputs.

4. **Use the accepted literal tail semantics.** Flat/Extend/Error replace the
   obsolete FlatZero/FlatForward/None labels without aliases. Flat holds a
   boundary DF, implying zero instantaneous forward strictly outside that
   boundary. Extend follows the selected method's working-space continuation:
   Linear extends DF secants, CubicHermite its boundary DF derivative, LogLinear
   its log-DF secant, and the two forward-based methods their actual inward
   boundary forward. Extend therefore is not universally a flat financial
   forward. Error rejects outside queries. Exact endpoints remain in-range and
   inherit the numerical owners' derivative sides. No regulatory long-end,
   flat-zero-rate, collateral or curve-role assumption is added.

5. **Preserve a source-owned clock and signed factor queries.** Dates map from
   the curve base with its stored explicit day count and complete required
   context; do not substitute coupon, model or volatility time. The current
   calendar-bearing curve clock carries no coupon/reference-period metadata:
   reject conventions requiring that absent context at construction rather than
   accept a curve whose ordinary dated queries necessarily fail. Bus/252 needs
   its explicit resolved calendar and the accepted calendar coverage/error
   rules. Dated factories use that same context, including before the first
   mapping. Distinct dates can map to equal times; mapped pillar collisions are
   errors rather than merged observations. Numeric factor/relative/instantaneous
   queries accept any finite signed source time under the explicit tail policy.
   A date before a spot-based origin is not automatically invalid; Error may
   reject its mapped negative coordinate. Date-to-source mapping is separate
   from an operation's cashflow eligibility or quote-inversion horizon.

6. **Make the whole evaluation boundary fallible.** `df`, dated DF queries,
   zero-rate queries and `Discounting::df` return Result, using existing input
   versus computation error categories. Validate finite query coordinates and
   allowed domains before exact-knot or identity shortcuts. Return exact stored
   DFs at untransformed knots, including the normalization anchor, with no
   approximate knot matching. Return only finite positive requested factors;
   positive subnormals are valid, zero-underflow/infinity/NaN are failures.
   Remove error-to-NaN, zero, endpoint and unchanged-value recovery paths used
   to conceal failed evaluation. Ordinary correctly rounded results can equal
   1 or an adjacent value; that alone is not a failure. No public unchecked
   twin, public log-DF framework or arbitrary-precision runtime is introduced.

7. **Keep relative factors oriented and validate identity eligibility.** For
   finite admitted source coordinates a,b, use D(b)/D(a). Date helpers first
   map both dates with the same source clock/context. Evaluate/admit the required
   absolute factors even for equal coordinates, so an identity cannot bypass
   coverage or Error-tail admission. Then an exact equal-coordinate request
   returns 1. Reversed endpoints are allowed, and different dates mapping to
   one time also yield 1 after admission. The requested quotient must be finite
   and positive; an overflowing or zero-underflow quotient is a computation
   error even when both absolute factors are individually representable. Do
   not substitute time subtraction on another clock or silently recover an
   invalid absolute factor through a log-domain relative-only result. This
   preserves ADR 0045's absolute-factor and relative-factor boundaries.

8. **Require a positive horizon for zero-rate inversion.** `zero_rate` and its
   dated counterpart require t>0 on the curve clock and explicit compounding.
   Use the accepted scalar inverse conversion and its domain/range checks.
   At t=0, D=1 identifies no zero rate: return a domain error, not 0, a short
   tenor substitute or a derivative disguised as the same quote. A pre-origin
   factor may be valid while its zero-rate inversion is outside this API's
   horizon. Users requesting the local short rate use instantaneous_forward;
   its value at the origin has the explicit source-side derivative meaning below.

9. **Extract interval forwards without an arbitrary minimum tenor.** For
   finite t1<t2 whose endpoint factors are admitted, the continuous decimal
   forward is -log(D(t2)/D(t1))/(t2-t1). Remove min_forward_tenor and its stored
   field/accessors/default, rather than turn 1e-6 into a universal accuracy
   rule. Require a finite positive computed interval and finite returned rate.
   Compute the positive-factor log difference stably: log1p of a suitably
   scaled nearby difference, with log subtraction for widely separated
   magnitudes. Do not first materialize an overflowing/underflowing quotient
   or subtract rounded cached logs of nearly equal factors. A relative-factor
   request can fail while this log-rate request succeeds. Equal admitted
   binary64 endpoint factors give zero; endpoint rounding can lose a very
   short interval's economic signal. Do not invent a rate from a nominal input
   or claim a minimum-tenor removal supplies a uniform accuracy guarantee.
   Required nonzero log/rate results lost to zero or nonfinite arithmetic must
   be stably reformulated or fail, distinct from equal evaluated endpoints.

10. **Use the derivative of the actual evaluated curve.** Define
    instantaneous_forward(t)=-D'(t)/D(t), using the analytic interpolation
    derivative and the same complete evaluator as DF queries. Inherit the
    accepted numerical one-sided rules: left interval at interior knots,
    inward side at the first/last endpoint; the actual limiting derivative
    for degenerate Hagan-West branches. Away from knots/tail joins use the
    ordinary derivative. An untransformed source origin uses the right/inward
    derivative, distinct from unidentified zero-rate inversion. A Flat tail gives zero
    strictly outside; at its boundary the in-range derivative still applies.
    Remove the generic finite-difference epsilon/clamp recipe. Make trait
    dispatch require a checked concrete-owner implementation rather than
    quietly averaging across knots. Parametric/model providers must supply
    their owned derivative contract; this does not adopt NS/NSS or rate-model
    mathematics. Required derivative/quotient range failures remain explicit;
    no promise that every finite mathematical ratio is computable by the
    chosen binary64 derivative path. Retained transforms must differentiate
    their actual evaluator, without reconstructing a different visible-pillar
    curve. At every actual evaluator breakpoint, including an adjustment-only
    kink, take the left limit of the complete evaluator, except at its actual
    retained source's first endpoint, where the complete right/inward limit
    applies. Retained source endpoints and tail joins move with the source-time
    offset; visible reporting pillars and a rolled normalization origin do not
    create new interpolation endpoints or reset those sides. Coincident source
    and adjustment breaks use this one common side for the complete derivative,
    rather than mixing a left source derivative with a right adjustment value.
    A transformed origin therefore uses that same rule if it falls on an
    interior break. Record03 still owns shock geometry/composition and where
    its additional breaks occur; this record owns their local query side.

11. **Keep financial factories as explicit recipes over the same owner.**
    `from_dates` accepts the complete explicit clock, style and tail choices,
    rejects pre-base pillars and enforces strict mapped ordering/admission.
    `from_zero_rates` requires those choices plus explicit compounding; admit
    each finite rate through the accepted scalar conversion before DF
    construction, including periodic-domain checks at an optional t=0 pillar.
    Such an origin quote only supplies D=1: its nominal rate is not retained
    or used as a derivative constraint. `flat` remains the named two-knot
    LogLinear+Extend recipe (0,1),(1,exp(-r)) with an explicit curve clock and
    finite signed continuous decimal rate. Remove the arbitrary [-1,1] rate
    heuristic; require its one-year factor to be representable and positive.
    This recipe interpolates the rounded stored factor: tiny r can round it
    to1, and the nominal r is not separately retained or exactly recovered.
    Do not claim exact exp(-r*t) for every binary64 input/maturity or silently
    add another flat-rate representation. Use canonical parameter objects
    where input count requires them, preserving the builder pattern.

12. **Preserve one strict state and host contract.** Store explicit complete
    financial choices and canonical pillars; rebuild private numerical caches
    through the same admission rules. Reject missing/unknown/retired controls,
    invalid method labels and malformed state; no aliases, migrations or
    default restoration. Admission of a failed replacement leaves the prior
    object unchanged. Keep retained-source/visible-state consistency checks
    while record03 retains their economic definition. Rust owns validation and
    errors; existing Python/WASM construction, results and docs expose the same
    choices and recoverable failures. Existing caller error propagation must
    not turn fallible numerical callbacks back into plausible values. Method
    availability is separate from a calibrated market's adoption or fitness.

## Rationale and alternatives

Explicit selection avoids making an unlabeled direct-construction default a
currency-wide methodology. A positive factor type supports negative-rate
history without an arbitrary universal forward floor; owners requiring further
economic constraints must validate their actual use. Keeping branded presets
would continue conflating a pillar screen with financial certification.

Analytic one-sided extraction preserves the selected interpolant. An automatic
centered difference can cross a knot and report neither chosen side. An
explicit approximate derivative can be useful in another owner's model, but
is not the common discount-source default. Rejecting origin rate inversion is
consistent with accepted scalar conversion, while a separately named local
forward remains available.

The explicit normalized two-knot flat recipe keeps the existing representation
and its rounding limits visible. A separate rate-storing flat curve, generic
clock/derivative provider registry, extra diagnostics framework or automatic
high-precision fallback is not needed for this package.

## Observed choice and rationale

`DiscountCurve` stores positive discount factors (DFs) against a year-fraction clock. Its default is MonotoneConvex interpolation and FlatForward extrapolation. This is an implemented selection, not an approved policy for every currency or market. [ADR 0032](../calibration-targets/0032-curve-interpolation-and-calibration.md) records the wider selection question. Shape preservation is a plausible rationale; the allowed negative-rate regimes must be agreed separately.

## Observed scope and input domain

The builder sorts pillars, inserts `(0,1)` when the earliest time is positive, requires at least two points, finite nonnegative times, finite strictly positive DFs, a unique origin exactly equal to `(0,1)`, and minimum relative knot spacing. Default Act/365F is independent of the identifier. Bus/252 requires a resolved holiday calendar. `flat` accepts only finite continuous rates in [-1,1], explicitly permits increasing DFs for negative rates, uses LogLinear/FlatForward, and disables the forward floor. `from_zero_rates` converts the specified compounding to DFs, then uses the ordinary builder policy; it does not automatically relax the negative-rate policy. `from_dates` defaults to Act/365F and rejects dates before base.

## Observed algorithm and conventions

1. Interpolate DFs using the selected shared kernel. Linear works in DF values; LogLinear in log DFs; CubicHermite uses shape-preserving Hermite slopes; PiecewiseQuadraticForward interpolates log-discount structure through its forward kernel; MonotoneConvex uses its dedicated forward reconstruction. Detailed kernels and local positivity projection are in [value interpolation](../core-numerics/value-interpolation.md) and [forward interpolation](../core-numerics/forward-interpolation.md).
2. `zero_rate(t, compounding)` delegates the inverse DF conversion, returning exactly zero at `t=0` rather than the limiting short rate.
3. `forward(t1,t2) = -ln(DF(t2)/DF(t1))/(t2-t1)` is continuously compounded. It requires finite ordered endpoints, interval at least `min_forward_tenor` (default 1e-6 years), and positive finite endpoint DFs.
4. The `Discounting` trait computes relative discounting as `DF(to)/DF(from)` after each date is mapped from the curve base using its clock/calendar; equal endpoints return one before other evaluation. Reversed dates are permitted.
5. Its default instantaneous forward uses a finite difference of log DFs: epsilon is `clamp(abs(t)*1e-4,1e-6,1e-4)`, start is `max(t-epsilon,0)`, end is `t+epsilon`. `ParametricCurve` also inherits this trait default even though it has a separate analytic `forward_rate` method.

`FlatZero` holds the boundary **stored value**, so on this curve it holds a DF rather than a zero rate. `FlatForward` extends the boundary slope in the interpolation kernel's working space. `None` yields NaN outside the domain; direct `df` and `zero_rate` return floating values rather than checked results. Negative-time queries therefore follow the interpolator's left-tail policy.

## Observed selection, failure, and alternatives

`MarketStandard` rejects increasing pillar DFs (1e-14 tolerance) and imposes a -0.005 adjacent continuously compounded forward floor. `NegativeRateFriendly` permits increases and requires a chosen floor; `Raw` exposes both controls. These are **pillar** checks, not a universal between-pillar arbitrage certificate. Shocked curves deliberately relax these checks; see [rate shocks](rate-shocks.md). Public Rust setters for `min_forward_tenor` and raw `forward_floor` do not themselves enforce finite/positive control values; NaN can disable a comparison. Direct small/negative-time instantaneous-forward queries have no explicit domain check.

Supported interpolation alternatives trade smoothness, locality, and negative-rate admissibility. The observed implementation supports an explicit no-extrapolation policy. The accepted package above selects analytic instantaneous forwards, stable interval log differences, explicit source choices and removal of the obsolete numerical controls; runtime reconciliation remains pending.

## Source and reference evidence

Owning code: [builder](../../../../finstack-quant/core/src/market_data/term_structures/discount_curve/builder.rs), [evaluation API](../../../../finstack-quant/core/src/market_data/term_structures/discount_curve/curve.rs), [validation](../../../../finstack-quant/core/src/market_data/term_structures/discount_curve/validation.rs), [wire/calendar/defaults](../../../../finstack-quant/core/src/market_data/term_structures/discount_curve/mod.rs), [trait implementation](../../../../finstack-quant/core/src/market_data/term_structures/discount_curve/traits.rs), and [relative/finite-difference discounting](../../../../finstack-quant/core/src/market_data/traits.rs).

Inspected tests `forward_rate_analytical_verification` and `parallel_bump_magnitude_verification` in [discount tests](../../../../finstack-quant/core/tests/market_data/curves/discount.rs) assert independent elementary formulas. The same file locates negative-rate, interpolation-style, serialization, and roll regressions; these do not replace external curve/vendor comparisons. [Hagan–West](../../../REFERENCES.md#hagan-west-monotone-convex), [Hull](../../../REFERENCES.md#hull-options-futures), and [Andersen–Piterbarg](../../../REFERENCES.md#andersen-piterbarg-interest-rate-modeling) provide method context, not product approval.

The original [Hagan–West 2008 paper, section 1](https://downloads.dxfeed.com/specifications/dxLibOptions/HaganWest.pdf)
defines the DF, relative-factor, continuous interval and derivative identities.
Its nominal-DF monotonicity argument assumes costless cash carry; it is not a
universal admission rule for negative-rate curves. The
[ECB's negative-rate study, section 2.1](https://www.ecb.europa.eu/pub/pdf/scpwps/ecb.wp2283~2ccc074964.en.pdf)
documents negative OIS rates across several maturities. This supports admitting
negative-rate regimes, without selecting a present market's inputs or a floor.
The [Bank of England's terminology](https://www.bankofengland.co.uk/statistics/yield-curves/terminology-and-concepts)
distinguishes spot, finite forward and instantaneous continuous rates. None of
these sources selects this record's software boundary, derivative side,
tail or factory recipe; those are explicit accepted library decisions.

## Independent examples and required verification

On 2026-11-08, standalone Python standard-library arithmetic used Decimal at
200 digits on exact `Decimal.from_float` inputs and Fraction for the rational
case. It did not import the library or execute Rust/host tests. For the
factor-pair examples in the first two rows, including reversal, dt=1 and the reference is
`-(Decimal.from_float(D2).ln()-Decimal.from_float(D1).ln())`.

| Example | Independently checked result / implication |
| --- | --- |
| D1=`0x0.0000000000001p-1022`, D2=`0x1.7e43c8800759cp+996` (binary64 1e300) | f≈-1435.215599819594967572009495; direct D2/D1 overflows. Reversal underflows the quotient, while f≈+1435.215599819594967572009495 is finite. |
| D1=`0x1.999999999999ap-4`, D2=`0x1.999999999999bp-4` | f≈-1.38777878078144550219584459e-16; direct quotient/log gives -2.2204460492503128e-16. Nearby log1p preserves the small difference in this example. |
| Linear DF pillars (0,1),(1,0.95),(2,0.90),(3,0.80), at t=2 | Exact-decimal left forward=1/18, right=1/9; the centered-difference limit is their average 1/12. A common centered recipe does not implement the selected left derivative. |
| Ideal flat continuous r=-0.01 | D(1)=1.010050167084168057542165457; D(5)/D(2)=1.030454533953516855612439954. Increasing D is consistent with this signed-rate definition. |
| Nominal r=0.05, t1=1, t2=`0x1.0000000000001p+0` | Both independently evaluated binary64 exp(-r*t) factors equal `0x1.e7078b0a726a6p-1`; endpoint extraction yields0. Stable log arithmetic cannot recover information already rounded away. |

The stable expressions were finite and within 2 ULPs of the rounded 200-digit
reference in the three specified factor-pair directions (including reversal).
That is an observed example tolerance, not a portable production or domain-wide
ULP guarantee. The ideal-flat example is distinct from the rounded two-knot
factory representation. These cases establish expression hazards and contract
distinctions, not production defect reproductions or price/risk accuracy.

Qualification of this accepted contract requires independent checks of:

- Ordered/duplicate/close/subnormal/invalid pillars, exact normalization and
  explicit-style/tail/clock admission, including direct versus strict restore.
- Positive/negative/mixed-rate examples for all five styles; actual DF domains
  and Flat/Extend/Error behavior, without whole-curve arbitrage claims.
- Scalar versus dated queries, signed pre-origin factors, source clock/calendar
  coverage and context errors, mapped-date collisions and equal/reversed ratios.
- Origin inverse-rate refusal versus local inward forward, interior and tail
  derivative sides, degenerate Hagan-West branches and actual transformed
  evaluators. Numeric coordinates use the curve clock; dated rate comparisons
  must not silently annualize on a coupon/model clock.
- Near-equal/extreme factor ratios, tiny or overflowing intervals, derivative
  range, normal/subnormal/nonfinite outputs and ordinary endpoint rounding.
- Factory conversion domains, nonzero origin quotes, flat-factor range and
  rate-information loss, malformed canonical state and atomic failed changes.
- Actual trait/pricing/calibration/model/host error propagation and independent
  formula expectations; replay and financial fit acceptance remain distinct.

## Follow-up tracking

Pending decisions, implementation reconciliation and validation work are tracked in the
[core-market follow-ups](../FOLLOW_UPS.md#core-market-data).
