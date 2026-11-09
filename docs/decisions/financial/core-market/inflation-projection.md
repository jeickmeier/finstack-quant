# CORE-MARKET-10: Explicit CPI sources, annual growth and parameter risk

- Decision status: **Accepted — complete CPI source/growth/shock package adopted 2026-11-08.**
- Implementation status: **Observed** in the owning paths identified below; the accepted source, caller and state contract is unimplemented.
- Validation status: **Primary/source evidence and independent source/clock, numerical/shock and consumer reviews inspected; standalone rational/high-precision examples executed 2026-11-08; production qualification pending.** Existing Rust/host tests were inspected or located, not executed. Expression checks do not qualify production indexation, calibration or a universal accuracy bound.
- Audit date: **2026-11-05**. Source revision: `7e82fd03ad98346727769e88dd5ac5d84b56e9fc`.
- Scope: the implementation listed here; the [slice inventory](README.md) and [coverage ledger](coverage.json) identify exact file coverage.

## Scope and accepted authority

This record owns a projected CPI/RPI index-level source, its source-clock growth
diagnostic, direct source-parameter risk and frozen dated reuse. It preserves
accepted [scalar interpolation](../core-numerics/value-interpolation.md),
[annual compounding](../core-numerics/compounding.md),
[rate units](../core-conventions/018-rate-units.md),
[ADR0045 clocks](../conventions/0045-financial-clocks-and-date-boundaries.md)
and [record03 bucket geometry](rate-shocks.md#accepted-decision-explicit-zero-rate-shocks-and-dated-conditioning).
[Record08](price-curves.md) supplies the absolute-level reuse boundary;
discount/projection factor normalization remains a different operation.

[Record11's observed-CPI and knowledge package](inflation-observations.md#accepted-decision-exact-monthly-fixing-snapshots-and-explicit-availability)
was accepted on **2026-11-08**, with runtime reconciliation and qualification
pending. [Publication/reference resolution](../valuations-framework/inflation-references.md),
[bond indexation](../valuations-other-fixed-income/04-inflation-indexation.md),
[inflation swaps](../valuations-rates/13-inflation-swaps.md),
[options](../valuations-rates/14-inflation-options.md) and
[calibration](../calibration-targets/06-inflation-calibration.md) retain their
remaining Draft choices. This package does not adopt their pricing measures,
convexity, denominators, publication rules, floors or solver/fit conventions.

## Foundational findings

A projected source level, a published monthly observation and a contract's daily
reference CPI are different inputs. The current curve stores a default lag and
offers a months/12 shift and implicit principal-style ratio. Actual pricing
already resolves calendar reference anchors and applies contractual daily
weights. Monthly Linear source pillars do not make the numeric lag equivalent.

The source's origin level is supplied financial data or a modeling assumption.
It is not a universal100 normalization or automatically a published fixing.
Freezing an absolute CPI projection at a later valuation date does not divide
all levels by a new CPI or turn an interpolated level into an observed fixing.

The current annual-CAGR shock also materializes a decimal rate and then adds1.
Severe deflation can round that rate to-1 even when the source level is positive;
reconstructing it can destroy CPI on a zero shock. Direct CPI calculations must
not depend on an unrequested annual quote being representable.

## Accepted decision: explicit CPI levels and anchored annual parameter shocks

Accepted on **2026-11-08** after independent source/clock, numerical/shock and
consumer review. Runtime/caller/host reconciliation and independent production
qualification remain pending; observed implementation stays separate below.

1. **Retain one explicitly associated positive index-level source.** Keep
   InflationCurve for finite strictly positive absolute index points, including
   positive subnormals. Bind its ID at existing market/convention owners to the
   exact index/series, reference-period interpretation, rebasing and seasonal-
   adjustment basis, and applicable currency association. Observations and
   projections must agree on that identity; ID spelling alone is insufficient.
   Describe levels as supplied/implied projections, without claiming a physical
   expectation, universal pricing measure, real-rate subtraction identity or
   general QuantLib/Bloomberg equivalence. Products own those model claims.

2. **Require an explicit source anchor and all construction choices.** Require
   base date B, complete source clock/calendar, scalar style and tails, plus
   strictly ordered finite nonnegative-time pillars beginning with exact
   (0,I0) and at least one positive-time pillar. I0>0 is explicitly supplied
   in that grid. Remove default100, automatic origin insertion and duplicated
   base_cpi builder/wire state; derive the base-level accessor from the origin.
   This anchor is the chosen annual-parameter reference, not a mathematical
   CPI normalization or proof of observation. Do not sort, snap, deduplicate or
   impose a relative knot-gap tolerance. Support Linear, CubicHermite/PCHIP and
   positive LogLinear; reject MonotoneConvex and PiecewiseQuadraticForward.
   Use literal Flat/Extend/Error tails without old-name aliases. Flat holds
   CPI level, giving zero incremental tail growth; Extend follows the actual
   scalar working-space continuation and is not universally flat inflation.
   Missing clock/calendar context errors; reject source clocks needing coupon
   metadata the source cannot store. No clock/style/tail is inferred or defaulted.

3. **Use checked unlagged source queries.** Numeric CPI requires finite t>=0;
   dated CPI requires date>=B and maps it through the same stored complete
   source clock/context. Admit the actual query and tails before shortcuts.
   Return exact stored knot levels, and otherwise only finite positive levels.
   Nonpositive interpolation/extension, NaN, infinity and zero-underflow error
   without a style switch, clamp or base-CPI substitution. Negative-time
   history is outside this modeled source, including under Flat tails. A
   source's base date is distinct from current valuation/fixing eligibility.

4. **Remove product lag and implicit principal indexation from core.** Delete
   indexation_lag_months, cpi_with_lag and index_ratio, their defaults and old
   host/wire fields. Products/convention resolution supply actual calendar
   reference periods/dates, selected monthly/daily interpolation and contractual
   base reference CPI. Apply each lag/seasonal treatment once. Reconcile the
   existing contract/index/curve lag fallback so removing curve metadata cannot
   silently choose no lag; unresolved or out-of-range required conventions
   error. Source day count, a months/12 approximation and numerical origin are
   not publication eligibility. Keep observed versus projected selection at its
   actual owner, with missing required known fixings remaining errors rather
   than source/base-level substitutions. Accepted11 owns the monthly observation,
   knowledge and shared reference-calculation boundary; actual product mapping
   and remaining resolver/model choices retain their separate owners. Contractual
   indexation ratios, rounding and floors use their actual owners and denominator.

5. **Keep annual growth a checked source-clock diagnostic.** For finite
   0<=t1<t2 with finite positive delta=t2-t1 and admitted endpoints, retain
   inflation_rate=expm1(log(I(t2)/I(t1))/delta). Use accepted private stable
   positive-level log differences, not an overflowing/underflowing quotient or
   subtraction of rounded cached logs. No minimum interval, historical clamp,
   zero-time inverse or error-to-zero recovery. Require a finite representable
   returned annual decimal rate r>-1; rounding onto-1 is a quote-range failure
   while the source levels can remain valid. Equal admitted endpoint levels
   imply0; required nonzero arithmetic lost to zero must be reformulated or fail.
   Any dated convenience maps both dates on the original source clock and uses
   their positive coordinate difference, not a fresh pairwise day count under
   nonadditive clocks. A contractual quote's actual annualization/compounding
   remains explicit at its owner; this diagnostic is not automatically ZC/YoY
   swap basis, a payable coupon or a stochastic expectation.

6. **Define direct risk on original annual-growth parameters.** The immutable
   original source anchor is I0 at B. For each original positive pillar t_i,I_i,
   define mathematically h_i=log(I_i/I0)/t_i and g_i=exp(h_i)>0. Weights w_i
   sample accepted03's exact geometry at original t_i; frozen dated reuse does
   not change those coordinates or create a new anchor/parameter. Support
   Additive RateBp (value/10000), Percent (percentage points, value/100) and
   Fraction (annual decimal increment), with d_i=b*w_i and g_i'=g_i+d_i>0.
   Support Multiplicative Factor with finite k>0, setting
   g_i'=g_i*m_i, m_i=(1-w_i)+k*w_i. This scales gross annual growth, not the
   net decimal rate: r=.02,k=1.1 gives r'=.122. Reject other mode/unit pairs.
   The origin is pinned as a parameterization choice, not because it is a fixing.
   After valuation advances, a projection shock may change modeled I(v); do not
   pin a newly interpolated reference level. Published contractual anchors still
   resolve from observed history and are unaffected by a projection-source shock.
   Individual targets must exactly and uniquely identify positive source
   pillars; reject origin/unknown/duplicate targets, with no near-match or tail
   no-op. Triangles use exact shared widths/weights. Validate complete units,
   values, geometry and targets even for zero/identity requests. Preserve source
   ID unless the caller explicitly chooses a replacement ID; no misleading
   bp-labelled IDs for gross-factor changes.

7. **Calculate shocked CPI directly and reinterpolate once.** Reuse accepted
   periodic-spread arithmetic with annual frequency1 privately. For additive
   d_i, a useful equivalent expression is

   ```text
   log I_i' = log I_i + t_i*log1p(d_i*exp(-h_i)),
   requiring exp(h_i)+d_i > 0.
   ```

   This is a mathematical identity, not permission to materialize an overflowing
   exp(-h_i) or multiply zero by infinity. Use scaled signed/log-domain
   addition/subtraction and stable complete-result reconstruction as required.
   Gross-factor changes use log I_i'=log I_i+t_i*log(m_i); preserve exact weight0
   and weight1 scale endpoints, including tiny k, instead of computing k-1 then
   losing k. Do not require the unused decimal annual rate, gross factor, level
   ratio or power to be representable. An admitted identity preserves each
   exact source pillar without a rate round trip. Validate all changed positive
   finite levels and required caches atomically, rebuild the same scalar style/
   tails from the same source grid, and propagate failures. Ordinary rounded
   unchanged levels do not prove zero economic sensitivity. This operation is
   source-parameter risk; off-pillar annual growth need not shift by b*w(t).
   Finite separate bucket PV changes need not add exactly; first-order/bump
   acceptance and actual price/risk accuracy remain with financial owners.
   No public log-CPI framework or arbitrary-precision runtime is added.

8. **Freeze absolute levels on the original source clock.** For any requested
   eligible reference date T, reuse I_B(tau_B(T)) unchanged, with an independent
   valuation reference v. Remove accumulated scalar offsets, interpolated new
   base CPI, origin insertion, expired-pillar deletion and backward refits. Do
   not normalize by I(v), change B or require an unrequested I(v) merely to use
   an otherwise eligible future level. Preserve complete original source state
   even beyond its last reporting pillar when its tails admit the query.
   Validate reference-date/context requirements without pretending v is a new
   CPI observation or publication cutoff. With original parameter coordinates,
   source shocks and frozen dated reuse commute in their mathematical meaning;
   a new calibration/anchor is an explicit new construction, not a hidden roll.
   Broader context/theta aging and new fixing incorporation remain separately owned.

9. **Reconcile source, contract and calibration anchors together.** The current
   inflation target shifts valuation date by a contract lag and assigns its
   effective contract reference CPI to the source origin. A blended daily
   reference CPI is not automatically a raw monthly observation at that shifted
   date. Keep explicit source I0/B/reference-period identity separate from the
   contract's start/issue denominator; do not rename one into the other or patch
   missing observations by rebasing the source. Reconcile factories and trial
   curves, explicit clock/style/tails, schema/preflight/quotes and full actual
   pricers together. Remove near-zero knot deletion/repair where it conflicts
   with exact source admission. The target's bounds, seed domain, solver and
   fit policy remain Draft. Direct annual-parameter risk is distinct from
   changing actual market quotes and recalibrating through qualified dependencies;
   [record06](calibration-provenance.md) retains its historical/current-fit boundary.

10. **Restore one strict source and update actual exposed consumers.** Source
    state stores original B/clock/context and explicit anchored grid/style/tails;
    market/transport state retains the authoritative association at its existing
    owner. There is no duplicate CPI0, curve lag, roll offset or derived current-
    origin state. Rebuild required caches and reject
    missing/unknown/invalid state without defaults, repairs, aliases or migrations.
    Reconcile context/scenario/attribution/market-change annual-growth formulas,
    including swallowed errors and zero sentinels, as well as calibration,
    prior-market/cache state, Python/WASM classes, stubs/declarations, schemas,
    parity and examples. Bindings convert types and propagate established errors;
    no separate host calculation or compatibility convenience API is retained.

## Observed implementation and discrepancies

Current defaults are CPI0=100, Act/365F, three-month lag, LogLinear and FlatZero.
Construction admits ordered negative knots but cpi(t<=0) returns base CPI;
an origin is inserted when absent. MonotoneConvex alone is explicitly blocked.
Date mapping supplies default day-count context. CPI lookup is infallible;
index_ratio checks the denominator then returns lagged source/base division.
Annual growth subtracts endpoint logs and checks finiteness but admits rounded-1.

Shocks reconstruct annual net rate and then gross growth, pin origin, and rebuild
source pillars. Rolled mapping deliberately uses current rolled coordinates and
current base CPI; the issue is changing parameter anchor/grid, not an accidental
sign error. Shared roll can insert that anchor using relative tolerance and drop
original state crossing its origin. Rolling fails when all future knots expire.
These are observed behaviors, not adopted financial choices.

Actual inflation-reference and bond consumers already request unlagged CPI at
calendar reference anchors and apply contract-month weights. Their existence
supports removing the core conveniences but does not independently certify
their publication, indexation or option assumptions.

## Independent expression evidence and qualification limits

On **2026-11-08**, twelve standalone Fraction/200-digit Decimal cases were
executed without production imports. They check definitions/counterexamples:

- CPI300 to306 gives annual growth2% over1year and4.04% over half a year.
- For B=2025-01-01 Act/365F, settlement2025-04-15 and Jan/Feb CPI300/306,
  subtracting3/12 source years samples302.467741935; actual calendar reference
  interpolation with weight14/30 gives302.8, even with monthly Linear pillars.
- Binary64 levels1e300 and its next larger neighbor have log ratio approximately
  1.487016908477783e-16; subtraction of their ordinary rounded logs gives0.
- For I0=1,I1=exp(-40)>0,t=1, the annual decimal quote rounds to-1 in binary64;
  the valid unchanged CPI survives a direct identity, unlike rate reconstruction.
- At1year, I0=300,I1=306 and additive100bp give309. Gross factor1.1 instead
  gives336.6, or12.2% annual growth. These were checked separately.
- Additive annual-parameter shocks followed by LogLinear level interpolation
  differ from a continuous additive annual-growth shift between source pillars.
- For I0=1e300,I1=1e-300,t=1, adding1e-300 to annual growth gives CPI
  1+1e-300 mathematically, even though the original gross1e-600 underflows.
- With I0=1,I1=1e300,t=1 and gross factor1e-300, the new CPI is1;
  computing1+(k-1) at full weight instead loses the positive multiplier.
- On US30/360, B=2025-01-29,v=Jan30,T=Jan31 and I(t)=300+360*t give
  I(v)=301,I(T)=302. Rebuilding on fresh tau_v(T)=0 returns301 instead of302.
- With Flat CPI tail after I(1)=306, growth from1to2 is0. With source I0=300,
  projected330 and contractual base reference CPI250, principal ratio is1.32,
  whereas dividing by the source anchor gives1.10. These were checked separately.

Independent production qualification must cover exact index/anchor identity,
context-dependent/nonadditive clocks, negative/pre-origin and Error queries,
scalar styles/tails, positive-subnormal/MAX levels, nearby logs, tiny horizons,
annual quote -1/range boundaries and direct-shock intermediate range. Qualify
additive shifted-domain cancellation, tiny gross factors, exact targets/buckets,
atomic no-ops/failures, source-grid reinterpolation and frozen/shock composition.
Verify actual publication/fixing/denominator consumers, calibration source-versus-
contract anchors and strict host/state/parity changes with independent product
references. These examples and same-pricer fit tests do not certify production
pricing, historical knowledge, vendor agreement or a uniform accuracy bound.

## Evidence and alternatives

Inspected owners: [InflationCurve](../../../../finstack-quant/core/src/market_data/term_structures/inflation.rs),
[shared roll](../../../../finstack-quant/core/src/market_data/term_structures/common/roll.rs),
[reference helpers](../../../../finstack-quant/valuations/core/src/instruments/common_impl/helpers.rs),
[bond indexation](../../../../finstack-quant/valuations/fixed-income/src/inflation_linked_bond/indexation.rs),
[inflation target](../../../../finstack-quant/calibration/src/targets/inflation.rs),
[Python binding](../../../../finstack-quant-py/src/bindings/core/market_data/curves/inflation.rs)
and [WASM binding](../../../../finstack-quant-wasm/src/api/core/market_curves.rs).
Inline CPI/CAGR/shock/serde tests and calibration integration tests were inspected
or located, not executed; they do not independently qualify market conventions.

[31 CFR356 Appendix B, 2025 edition, I.B.2–3](https://www.govinfo.gov/content/pkg/CFR-2025-title31-vol2/pdf/CFR-2025-title31-vol2-part356-appB.pdf#page=5)
defines TIPS reference-month interpolation and an issue/dated-date denominator;
it also owns product rounding, which is not added to source interpolation.
[BLS series identification](https://www.bls.gov/cpi/factsheets/cpi-series-ids.htm)
distinguishes the index population, seasonal status, publication frequency, area
and items. These references support the source/contract identity distinction;
this package's anchor, supported styles and parameter-risk design are explicit
architectural choices. [Deacon–Derry–Mirfendereski](../../../REFERENCES.md#deacon-derry-mirfendereski-2004)
and [Kerkhof](../../../REFERENCES.md#kerkhof-2005) provide broader model context.

A continuous zero-inflation function shock, a future source reanchored at v,
seasonal calibration and actual quote recalibration are different operations.
They are not introduced as modes or fallbacks for this source-parameter package.

## Follow-up tracking

Runtime reconciliation and independent qualification are
tracked in the [core-market follow-ups](../FOLLOW_UPS.md#core-market-data).
