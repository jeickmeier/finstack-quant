# CORE-MARKET-12: Checked scalar data, explicit sampling and exact fixing identity

- Decision status: **Accepted — complete scalar/series/fixing package adopted 2026-11-08.**
- Implementation status: **Observed** in the owning paths below; the accepted validated owner, sampling and fixing contract is unimplemented.
- Validation status: **Source/primary evidence and independent scalar/numerical, fixing/consumer and host/shock reviews inspected; standalone rational/date/range examples executed 2026-11-08; production qualification pending.** Existing Rust/host tests were inspected or located, not executed. Examples do not certify production sampling, publication eligibility or pricing accuracy.
- Audit date: **2026-11-05**. Source revision: `7e82fd03ad98346727769e88dd5ac5d84b56e9fc`.
- Scope: the implementation listed here; the [slice inventory](README.md) and [coverage ledger](coverage.json) identify exact file coverage.

## Scope and accepted authority

This record owns generic scalar/date-series admission and sampling, native
scalar/series transformations and the small exact rate-fixing lookup boundary.
It preserves accepted [Money precision](../core-conventions/012-money-decimal-precision.md),
[rate units](../core-conventions/018-rate-units.md),
[observation versus projection identity](forward-projection.md#accepted-decision-explicit-index-projection-factors-and-contractual-rates),
[ADR0045 publication eligibility](../conventions/0045-financial-clocks-and-date-boundaries.md)
and [record11's separate monthly CPI source](inflation-observations.md#accepted-decision-exact-monthly-fixing-snapshots-and-explicit-availability).
Generic samples are not a replacement for CPI fixing snapshots.

Financial units/domains, publication phases and selected revision knowledge
retain their actual product/data owners. [Term coupons](../cashflows/06-floating-rate-projection.md),
[overnight projection](../valuations-framework/overnight-projection.md),
[CMS](../valuations-rates/11-cms-convexity.md),
[fixing materialization](../cashflows/16-fixing-materialization.md),
and [scenario roll](../scenarios/13-market-time-roll.md) retain their remaining
Draft choices. This package does not adopt their forecasts, same-day policies
or pricing models. Accepted [context dispatch20](context-lifecycle.md) owns
typed selection, source lifecycle and state integrity.

## Foundational findings

MarketScalar's public Unitless(f64) variant bypasses its finite constructors.
ScalarTimeSeries silently sorts input, defaults Step and carries its last value
indefinitely; Linear can overflow internally for a finite exact midpoint.
Neither optional currency nor the name Unitless establishes financial units.

Percent also changes meaning with storage: a request10 changes Unitless100 to
100.1, Money100 to110 and a currency-tagged numeric series100 to100.1. A generic
data transform must not decide between percentage points and a relative move.

Finally, the fixing helper accepts a supplied series without checking its ID.
Its naming confuses observation IndexId with projection CurveId, and CMS keys
use a fuzzy floating tenor while omitting the actual observed swap benchmark.
Exact date matching is necessary but does not establish identity or publication.

## Accepted decision: validated values, literal sampling and qualified exact fixings

Accepted on **2026-11-08** after independent scalar/numerical, fixing/consumer
and host/shock review. Runtime/caller/host reconciliation and independent
qualification remain pending; observed implementation stays separate below.

1. **Use one validated immutable scalar owner.** Keep MarketScalar with private
   variant storage and checked constructors for a finite signed numeric value
   or an admitted Money amount. Remove public Unitless(f64)/Price construction
   that bypasses admission; expose typed get_* inspection without mutable raw
   storage. No new family of financial-number wrappers is needed. Exact Money
   amounts/currencies retain the accepted Decimal contract through construction,
   arithmetic and state; never convert an amount to f64 to evade that contract.
   Numeric scalars admit signed/zero/subnormal finite values. Rates, probabilities,
   nonnegative spots and other financial bounds remain the consuming owner's
   responsibility, with explicit source/unit association rather than ID inference.

2. **Keep generic dated samples small and explicit.** Retain ScalarTimeSeries
   with strictly date-ordered unique inputs and finite signed binary64 values.
   Reject unsorted/duplicate/nonfinite input and canonical state; do not sort,
   merge, fill or coerce missing observations. Require explicit Step or Linear
   interpolation and explicit right-tail Flat or Error. Reject Extend; all
   queries before the first sample error. Remove default Step and implicit last-
   value carry. Empty history is valid and all its value queries return Missing;
   no fictitious fixing is needed. One sample is valid: its exact date is
   evaluable, and only an explicitly Flat right tail permits later sampled
   values. Linear does not invent a slope from one sample. Missing financial
   history may still be represented by no configured series; neither absence
   nor an empty collection authorizes a projected substitute.

3. **Keep units and monetary precision honest.** Optional currency identifies
   a numeric series' currency association; its values remain binary64 samples,
   not exact Money. Other quotation/physical/rate units and source identity are
   explicitly resolved at existing market/data owners. No currency tag turns
   a sample into a financial rate or fixes a decimal/percent ambiguity. Actual
   payments/settlement and exact monetary data use accepted Money conversions
   and rounding at their owners. Do not add a generic unit registry or interpret
   all untagged samples as rates because they are called Unitless.

4. **Separate checked sampling from exact observation retrieval.** Use canonical
   get_value(date) for the chosen Step/Linear policy and get_value_exact(date)
   for one actual stored date; remove the old value_on/value_on_exact aliases.
   Exact lookup ignores interpolation/tails and errors on any missing date.
   At a stored date return that exact sample. Within the grid, Step returns the
   latest prior sample; Linear uses the adjacent actual calendar-day distance,
   with w=(d-d0)/(d1-d0). Compute day differences in checked sufficiently wide
   integer arithmetic before conversion. Use stable checked signed convex
   arithmetic, not an overflowing y1-y0 or an unscaled endpoint sum. Return
   finite results and propagate numerical failures; no clipping, tolerance
   matching or error-to-zero. Flat holds the final level literally, without a
   freshness guarantee. Missing-date fixing access never uses that carry.
   Remove obsolete generic monthly lookup once accepted11 owns exact monthly
   observations. Generic samples encode neither business-day observation grids,
   publication/revision knowledge, permissible gaps nor maximum data ages.

5. **Define two generic native-value transformations.** Retain parallel-only
   BumpSpec operations Additive/Fraction, meaning a native stored-unit delta b,
   and Multiplicative/Factor, meaning a literal finite signed factor k. Apply
   x'=x+b or x'=x*k consistently to numeric scalars, Money and generic samples.
   Zero/negative k are admitted at this generic signed boundary; financial
   consumers still enforce their actual domains. Reject generic RateBp and all
   Percent mode/unit pairs instead of reinterpreting them by storage variant.
   A financial adapter explicitly converts bp, percentage-point or relative-
   percent intent using its authoritative representation and existing checked
   arithmetic; no new generic percent API or old-pair alias is added.

   Money native deltas/factors supplied as genuine f64 first satisfy accepted
   shortest-text Decimal admission, then use checked Decimal addition or
   multiplication. Finite f64 does not imply an admissible Money scalar; a
   mathematically representable final product cannot rescue an invalid factor.
   Exact Decimal monetary changes retain their actual Money operation rather
   than detouring through a binary64 BumpSpec. Numeric/series transforms require
   finite outputs; a nonzero product lost to zero is an underflow error, while
   an explicit zero factor and exact cancellation are valid. Validate complete
   source/request mode, units, parallel shape and values before identity or
   empty-history shortcuts. Keep changes atomic, preserving ID/metadata/grid/
   interpolation/tails unless an explicit replacement ID is requested. Whole-
   history transforms are labelled hypothetical datasets, not new publications.

6. **Resolve fixings by observation identity, independently of projection.**
   Retain one canonical FIXING:<observation_id> namespace built from the
   configured observed IndexId, never its current projection CurveId. Require
   a nonempty exact resolved observation identity; an IndexId wrapper alone does
   not validate registration or financial meaning. Rebinding a projection curve
   must not rename historical observations. Keep one fixing_series_id(IndexId)
   helper; its formatting does not prove association. Resolve and verify the
   exact expected series key before lookup, including when a series reference
   is supplied directly. Wrong-series/currency/unit association errors rather
   than returning a same-date value. Preserve causes for missing source/date
   and failed qualification instead of flattening every failure into not-found.
   Historical rate values are finite signed annual decimal index quotes, before
   spread/gearing/caps/floors and without reannualization; other fixing families
   retain their own positivity/orientation/unit rules.

7. **Give CMS its actual observed benchmark identity.** Require an explicitly
   configured CMS fixing IndexId distinct from both underlying rate-index and
   projection-source IDs. Its owner binds the actual reference swap/tenor,
   currency, leg conventions, publication setting/run and applicable benchmark
   variant. Verify agreement with the resolved contract, including overrides;
   a custom swap cannot silently consume another benchmark's history. Remove
   cms_fixing_series_id(f64 tenor) and fuzzy month/year formatting, saturation
   or raw floating-year identity. Existing CMS owners resolve supported positive
   calendar month/year tenors exactly; do not infer months within1e-9. Use the
   configured observed ID with the same core exact helper. No new CMS registry,
   generated convention hash or compatibility key lookup is introduced.

8. **Keep publication qualification explicit at the existing owners.** The
   exact core helper establishes identity/date matching, not whether a value
   is historically eligible. Remove its decorative as_of argument; owning
   product/adapters qualify the requested observation, selected source/version,
   information cutoff/phase and publication convention before consuming it,
   under accepted ADR0045. A past value date can remain unpublished; a stored
   future or revised value is not automatically usable. Missing/unknown
   eligibility must error, rather than become permission to project through an
   .ok() branch. Published required observations missing from the configured
   source error. Explicitly ineligible/unpublished observations project only
   where that operation permits projection; otherwise error. Do not derive a
   universal <as_of, <=as_of, series-presence or same-day rule here. Supply
   snapshots/provenance consistent with the owning request; no generic vintage
   database, timestamp engine or copied monthly-CPI snapshot framework is added.

9. **Keep fixing histories outside ordinary generic bump dispatch.** Reserve
   the exact FIXING: namespace for configured fixing datasets and reject generic
   series bumping of those bindings in both direct Bumpable and context dispatch,
   including identity/empty requests. That refusal is a mutation boundary;
   the prefix alone never proves observation identity or publication authority.
   Explicit corrections/hypothetical historical changes require a new labelled
   dataset with selected provenance through their owner. Reconcile pre-roll
   projected fixing materialization: a model realization must not become actual
   publication authority merely by insertion into the same namespace or by
   advancing valuation time. Its scenario/conflict/knowledge policy stays Draft.

10. **Restore strict data and reconcile actual exposed paths together.** Store
    explicit validated scalar kind/value or series ID/currency, exact ordered
    finite samples and required interpolation/right-tail policy. Rebuild checked
    storage without defaults, repairs, aliases, migrations or old-field readers.
    Reconcile context pattern matches/insertion/getters, generic scenario and
    spot-risk unit conversion, attribution/market-change and calibration market-
    datum transport; term/overnight/TRS/futures/loan/FX/CMS fixing resolution,
    raw projected-fixing metadata and materialization; Python/WASM constructors,
    context methods, stubs/declarations, schemas/parity and examples. Keep host
    Money exact-input versus model-f64 differences explicit. Rust owns behavior;
    no standalone host MarketScalar class or host-side financial calculation
    is needed. Broader namespace precedence, metadata freshness and full product
    pricing choices retain their existing owners/status.

## Observed implementation and discrepancies

MarketScalar's constructors/wire reject nonfinite unitless values, but direct
public enum construction and context insertion bypass that invariant. Money
uses its separate monetary representation. Series construction sorts arbitrary
dates, rejects empty/duplicate exact dates/nonfinite samples, accepts one point
and defaults Step. Linear uses y0+w*(y1-y0) without a post-result check; both
modes hold the last sample indefinitely. Exact-date lookup is separate.

Generic bumps accept additive bp/Percent/Fraction or Factor. Percent adds p/100
for numeric scalars/series but scales Money by1+p/100. Series currency does not
change its numeric arithmetic. Context bump dispatch can modify fixing series.
Fixing keys use projection curve/index interchangeably; CMS snaps near-month
floating tenors and can format invalid or saturated inputs. The exact helper
accepts any supplied series ID and uses as_of only in diagnostics. Actual
callers use differing date/presence rules. These behaviors are observed, not
adopted choices.

## Independent examples and qualification limits

On **2026-11-08**, ten standalone rational/calendar/range cases were evaluated
with Fraction, Decimal, datetime and standard-library binary64 expressions,
without importing production code:

- Endpoints -MAX/MAX have exact midpoint0, while current-style subtraction
  overflows. Equal MAX endpoints have exact midpointMAX, while summing first
  overflows. These were checked as separate cases.
- January31 to February29,2024 spans29 calendar days. February14 has weight
  14/29; interpolating -10 to19 gives4. No business-day/day-count clock is used.
- Samples5% on January2 and6% on January5 give Step5% and Linear approximately
  5.333333% on January3, while exact lookup has no observation on that date.
- Percent10 currently gives100.1 for numeric100 but110 for Money100. Native
  delta.1 and literal factor1.1 have consistent respective meanings across
  those representations. A separate rate example distinguishes5%+1percentage
  point=6% from a relative1% move to5.05%.
- Tenors1.0 and1.0+1e-12 both enter the current1Y key despite differing inputs.
  This is a formatting collision, not permission to accept either as an exact
  calendar tenor through a floating approximation.
- An illustrative rate value date2025-01-03, release2025-01-06 and information
  date2025-01-05 show that a past lookup key can remain unpublished. This is
  a stipulated calendar example, not a supplied historical release dataset.
- Money amount1e28 times factor1e-29 has mathematical result.1, but the factor
  lies outside accepted Decimal scalar admission; valid final magnitude does
  not rescue it. Numeric series remain subject to their distinct f64 boundary.
- The smallest positive subnormal times.5 rounds to0 in binary64 despite a
  nonzero exact product; explicit multiplication by0 is a genuine zero. This
  was the tenth independent requested-output range case.

Independently qualify the actual scalar construction/wire/context invariant;
strict ordering, empty/single/exact/left/right policy and stable signed/range
sampling; units, Money conversion/rounding, finite/underflow/identity and atomic
whole-history transforms; observation-ID and wrong-series rejection, CMS tenor/
convention/run identity, selected publication/revision eligibility and missing-
versus-ineligible errors; hypothetical materialization/provenance and all actual
Rust/host/state consumers. Algebra, same-date retrieval and serialization round
trips do not certify publication, freshness or contractual pricing accuracy.

## Evidence and alternatives

Inspected owners: [scalars/series](../../../../finstack-quant/core/src/market_data/scalars/primitives.rs),
[exact fixing helpers](../../../../finstack-quant/core/src/market_data/fixings.rs),
[context bump dispatch](../../../../finstack-quant/core/src/market_data/context/ops_bump.rs),
[CMS reference conventions](../../../../finstack-quant/valuations/rates/src/cms_common.rs),
[fixing materialization](../../../../finstack-quant/cashflows/src/fixings.rs),
[Python scalars](../../../../finstack-quant-py/src/bindings/core/market_data/scalars.rs)
and [WASM scalars](../../../../finstack-quant-wasm/src/api/core/market_scalars.rs).
Existing constructor, Step/Linear, exact fixing, scalar overflow, state and
scenario tests were inspected or located, not executed.

[New York Fed publication/revision information](https://www.newyorkfed.org/markets/reference-rates/additional-information-about-reference-rates)
distinguishes rate value dates, subsequent publication and eligible revisions.
[ICE Swap Rate descriptions](https://www.ice.com/iba/ice-swap-rate)
identify benchmark variants, currencies, tenors and publication settings. These
support the explicit identity/eligibility distinction; they do not prescribe
this library's generic interpolation or shock API. [Missing-data ADR0009](../conventions/0009-missing-and-invalid-data.md),
[ISDA 2021](../../../REFERENCES.md#isda-2021-definitions) and
[ARRC SOFR guidance](../../../REFERENCES.md#arrc-sofr-users-guide) provide the
remaining source/product context.

Sorting ingestion, bounded stale-data carry, typed financial percent/bp shocks
and hypothetical fixing scenarios retain explicit owners. They are not hidden
modes or fallbacks inside these generic value and exact-date primitives.

## Follow-up tracking

Runtime reconciliation and independent qualification are
tracked in the [core-market follow-ups](../FOLLOW_UPS.md#core-market-data).
