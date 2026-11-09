# CORE-MARKET-11: Monthly CPI fixings, knowledge snapshots and reference resolution

- Decision status: **Accepted — complete monthly-fixing/availability package adopted 2026-11-08.**
- Implementation status: **Observed** in the owning paths below; the accepted source/snapshot/resolver contract is unimplemented.
- Validation status: **Primary/source evidence and independent observation, publication/seasonality and consumer reviews inspected; standalone rational/date/range examples executed 2026-11-08; production qualification pending.** Existing Rust/host tests were inspected or located, not executed. Examples do not certify production indexation, historical vintages or a universal accuracy bound.
- Audit date: **2026-11-05**. Source revision: `7e82fd03ad98346727769e88dd5ac5d84b56e9fc`.
- Scope: the implementation listed here; the [slice inventory](README.md) and [coverage ledger](coverage.json) identify exact file coverage.

## Scope and accepted authority

This record owns exact observed monthly CPI/RPI identity, selected fixing
snapshots and availability admission, and the shared calendar-reference
calculation boundary. It preserves accepted [record10's CPI source contract](inflation-projection.md#accepted-decision-explicit-cpi-levels-and-anchored-annual-parameter-shocks),
[ADR0045's information cutoff](../conventions/0045-financial-clocks-and-date-boundaries.md)
and [calendar arithmetic](../core-conventions/007-tenor-date-arithmetic.md).
Projected levels, observations and contractual base reference CPI remain distinct.

[Shared reference resolution](../valuations-framework/inflation-references.md),
[bond indexation/floors](../valuations-other-fixed-income/04-inflation-indexation.md),
[swaps](../valuations-rates/13-inflation-swaps.md),
[options](../valuations-rates/14-inflation-options.md) and
[calibration](../calibration-targets/06-inflation-calibration.md) retain their
remaining product/model/solver choices. This package selects the source and
reference calculation below without adopting those remaining choices.

## Foundational findings

The current source is a generic dated series with optional lag, interpolation,
seasonal multiplication and release metadata. Arbitrary labels within a month
can alter calendar-time lookup; duplicate reference months and nonpositive CPI
are not rejected at construction. Release metadata even changes which
interpolation branch valuation uses. A monthly observation's identity must not
depend on those incidental labels or the presence of a schedule.

The resolver treats missing publication dates as publication on the reference
date. Direct source value_on ignores publication altogether. A later revised
value can also be attached to its original first-release date because no fixing
version is identified. Data presence is insufficient evidence of knowledge.

Finally, multiplying an observed CPI by a model seasonal factor changes the
published fixing. Normalizing twelve factors to annual product1 does not cure
that change. Published seasonally adjusted and unadjusted series are distinct
source identities, not two lookup modes for one fixing history.

## Accepted decision: exact monthly fixing snapshots and explicit availability

Accepted on **2026-11-08** after independent observation, publication/seasonality
and consumer review. Runtime/caller/host reconciliation and independent
financial qualification remain pending; observed implementation stays separate below.

1. **Retain one raw monthly observation source.** Keep InflationIndex with
   exact monthly observations, separate from ScalarTimeSeries interpolation.
   Require canonical first-of-month Date keys, strictly month-ordered unique
   input and finite strictly positive index points, including subnormals.
   Do not sort, relabel month-end/release dates, deduplicate or repair values.
   Ingestion explicitly converts an external reference-period label before
   construction; a release date is never used as its reference month. Missing
   months are allowed and remain missing. Empty history is valid, allowing an
   identified projection-only market to carry release facts without a dummy print.

2. **Require an explicitly selected immutable fixing snapshot.** Bind exact
   index/series, currency association, rebasing and seasonal-adjustment identity,
   source/version-selection provenance and declared knowledge cutoff K. Keep
   the authoritative association at existing data/market owners and retain it
   with canonical transport state; do not infer it from an ID. Each selected
   observation carries its actual available_on date for that value version,
   admitted on/after its reference-month start and on/before K. First release,
   a specified revised/final vintage and a contract-frozen value are different
   selections; the requested contract must agree with the supplied selection.
   A preliminary value cannot satisfy a final-value requirement. A later
   revision must not inherit an earlier version's availability date.
   Upstream owners supply the selected snapshot; core stores one selected
   value per month, not a generic revision database or automatic latest selector.
   An unavailable required historical/contractual version errors.

3. **State the information cutoff and date precision honestly.** Snapshot K
   is the explicit observation knowledge date, separate from valuation v and
   reference month. This package uses inclusive day-level availability
   available_on<=K under a declared end-of-day information phase. It does not
   represent before-release intraday knowledge or manufacture timestamps/time
   zones. Operations needing finer release phases must refuse this insufficient
   date-only input. Historical point-in-time claims require a suitably bounded
   K and verified version provenance; K>v is a later-informed restatement.
   K<v is explicitly earlier information, not automatically the complete data
   available at v. Changing v alone never publishes a forecast or revises a
   fixing. A different cutoff/selection requires an explicitly supplied new
   snapshot; do not relabel current data as an old snapshot or consult wall time.

4. **Separate actual availability from expected release facts.** Retain an
   explicit ordered monthly expected-release calendar independently of fixing
   presence, including for empty histories. Required future facts identify the
   same index/version regime and cannot precede the reference-month start;
   they are supplied assumptions with provenance, not actual releases or a
   calendar inferred from currency, lag, reference date or average delay.
   Each assumption carries its own known_on date, admitted on/before K; a
   reschedule learned after K cannot be used in that historical snapshot.
   Later-informed calendars require a new cutoff and appropriately labelled
   restatement, just like later-informed observation versions.
   An eligible actual selected observation wins even if an old schedule expected
   a later release. If no eligible observation exists, require a release fact:
   expected_release>K permits projection under that snapshot's stated assumption;
   expected_release<=K means a required fixing is missing/overdue and errors.
   Missing facts never imply known or unpublished. A changed release assumption
   requires an explicit updated snapshot. An expected future date is not proof
   that no unscheduled early release occurred; actual provenance remains required.
   Keep those facts distinct in state/results rather than one ambiguous
   publication_date meaning both actual and scheduled availability.

5. **Make source lookup exact and remove product conventions.** Provide one
   checked get_observation(reference_month) returning the exact selected record
   in that immutable snapshot; missing months error. Validate canonical month
   keys before shortcuts; callers verify snapshot/selection agreement before
   consuming the record. There is no carry,
   interpolation, extrapolation, lag, seasonal multiplication or projected
   fallback inside this source. Delete with_lag, with_interpolation,
   with_seasonality, value_on, ref_cpi_months_lag, ratio and their stored/default
   convention fields and host twins. Keep InflationLag as an explicitly supplied
   contractual date-shift type at actual product/convention owners. Removing
   source metadata must not silently select no lag or default interpolation.
   No public bare-value convenience may bypass the selected knowledge contract.

6. **Resolve each required anchor once at the existing shared owner.** The
   valuation helper takes explicit contract date, lag, Step/Linear policy,
   requested fixing selection and matching knowledge snapshot K. Monthly CPI
   identity is intrinsic for None/Days/Months lags and zero-lag spellings; adding
   release metadata must not change the calculation. For Months(L), start from
   the first of the contract month minus L calendar months; let w=(day-1)/D,
   with day/D from the original contract month. Step or exact w=0 needs only
   the first reference month. Linear with w>0 needs it and the next month.
   For None/Days, first apply the actual calendar-day shift (zero for None),
   then use that shifted date's month/day/D for the same monthly calculation.
   Checked calendar underflow/range failures propagate; months/12 is not used.
   Equal zero shifts must have equal financial results, without alternate
   reference identity chosen by parser spelling or optional metadata.

   Resolve each actually needed monthly anchor through points2–4. Use an exact
   eligible observation or accepted10's matching unlagged projection at that
   reference-month date; known/overdue-but-missing values never project. Then
   compute the checked positive convex interpolation (1-w)*I0+w*I1, preserving
   exact endpoint behavior and stable finite range. Do not query an unused
   zero-weight second month. Retain observed/projected provenance and actual/
   expected release facts in the existing resolved result; the whole reference
   is known only when all contributing anchors are observed. No independent
   core reference interpolator or generic fixing resolver framework is added.

7. **Keep ratios, rounding, expiry and special contingencies explicit.**
   Products divide by their actual positive contractual base reference CPI,
   with a finite positive checked requested ratio; source identity and version/
   rebasing must agree. Neither a source origin nor an arbitrary latest print
   substitutes for the denominator. Core observations/reference interpolation
   do not round to a product's published number of decimals or apply principal
   floors. A product needing an unknown reference's publication/option horizon
   uses explicit required release facts and its owned clock/model; remove
   reference-date-as-release-date fallbacks. This record does not select a
   floor/convexity model or aggregate-release option approximation. Official
   delayed-index substitutions, revisions, discontinuations and rebasing
   contingencies require the actual contractual/publisher authority; no generic
   projection or last-print fallback supplies an announced replacement fixing.

8. **Keep seasonality and hypothetical shocks outside observed fixings.**
   Remove automatic twelve-factor multiplication from observed history rather
   than introducing normalized factors as a repair. An authentic published SA
   series remains eligible under its exact identity and selected vintage; an
   NSA contract cannot consume it merely because the numbers are close.
   Forecast seasonal models and seasonal calibration retain their own financial
   owners/status and must produce correctly associated projected levels without
   double application. A revision or explicitly hypothetical historical-data
   scenario is a new labelled dataset, never a silent source bump or an actual
   published-fixing claim. Accepted10 projection shocks leave observed records
   unchanged. This package introduces no seasonality model or generic fixing bump.

9. **Reconcile every actual consumer and strict state path together.** Store
   canonical identity/selection/cutoff, exact ordered positive observation
   records with actual availability and distinct expected-release/known-on facts. Reject
   missing required fields, noncanonical/duplicate months, future actual records
   relative to K and malformed provenance/state without defaults, aliases,
   migrations or old-field readers. Reconcile the central helper and independent
   swap/cap/bond lag/interpolation fallbacks; preflight's source-lag/interpolation
   comparisons and lagged/seasonally changed base CPI; calibration's source versus
   contractual anchors; attribution/context/cache/wire and Python/WASM stubs,
   declarations, schemas, parity and examples. Bindings convert types and use
   existing errors, without a separate host resolver. Accepted22 replaces the
   latest-print comparison with explicit period/vintage/knowledge identity;
   unrelated latest observations do not imply an aligned revision. Broader
   scenario projected fixings and time evolution remain separately owned; they cannot mutate K
   into actual observation authority merely by advancing the valuation date.

## Observed implementation and discrepancies

InflationIndex wraps ScalarTimeSeries: it sorts labels, rejects duplicate exact
dates/nonfinite values but admits nonpositive levels and multiple dates within
one reference month. Defaults are Step, no lag and no seasonality. Monthly
lookup requires exactly one observation anywhere in each month; no/day-lag
paths instead use dated series interpolation and can carry the final print.
The monthly formula uses the contract-month day weight and does not apply the
stored lag twice. The generic ratio rejects only an exactly zero denominator.

Stored publication dates use first-of-month keys and inclusive Date values, but
direct value_on bypasses them. The shared helper selects monthly mode only for
Months lag or nonempty publication metadata and uses reference date when a
release date is missing. The parser maps 0M to None while 00M becomes Months(0),
which can change the selected branch despite an equal date shift. Seasonal
factors multiply raw monthly anchors without changing source identity. Current
state contains neither selected-version availability nor a knowledge cutoff.

Bond floor pricing uses known/unpublished state and may substitute a reference
date when release timing is missing. Calibration additionally compares source
lag/interpolation and a seasonally transformed reference value with its inputs.
These are observed behaviors requiring coordinated reconciliation, not adopted
market conventions. Existing tests are implementation evidence only.

## Independent examples and qualification limits

On **2026-11-08**, nine standalone rational/calendar/range cases were executed
with Fraction, Decimal and datetime, without importing production code:

- January300/February306 and an April15 three-month reference give302.8 using
  weight14/30. A February29,2024 reference uses28/29 of its selected monthly
  difference, not28/30 from a lagged thirty-day month.
- At April1 the second-month weight is exactly0; the reference is300 without
  requiring February. This was a separate endpoint case.
- At weight14/30, using an ineligible future February print900 gives580 rather
  than302.8 from projected306. With knowledge date before the version's release,
  that actual record is inadmissible to the observed snapshot.
- Factors1.1 for January and1/1.1 for February have product1, yet turn the same
  raw reference302.8 into approximately305.8181818. Normalization does not
  preserve the fixing.
- An initial January300 released2025-02-13 and revised301 released2026-02-13
  have different availability at K=2025-04-15. The revised value is ineligible
  there; a contract retaining the initial release continues to require300 even
  after the revision. A release reschedule learned2025-04-16 is likewise
  ineligible at that K. These are dated selection examples, not vintage data.
- Equal binary64 MAX anchors have the same finite exact convex midpoint, while
  forming their unscaled sum first overflows. A ratio of MAX to the smallest
  positive subnormal is unrepresentable despite valid individual levels; this
  was checked as a separate requested-ratio range case.
- A reference302.8 divided by contractual250 gives1.2112. Common explicit
  rebasing of numerator and denominator preserves it; mixing rebasing bases
  changes it. This was the ninth independent case.

Production qualification must cover exact/canonical monthly labels, gaps/empty
histories, positive-subnormal/MAX levels, index/seasonal/rebasing/version identity,
actual-versus-expected release facts and same-day inclusive boundaries; missing/
overdue facts, eligible early release, preliminary/final/revised snapshots and
historical cutoff errors; Step/Linear/month/day/zero lags, leap months/calendar
limits and unused endpoint requirements; stable interpolation/ratio range and
product rounding/contingencies at their owners. Independently verify actual
source/contract calibration inputs, fixing-versus-projection decisions, floor
expiry consumers and strict host/state/cache/parity. A self-consistent snapshot,
same-pricer fit or round trip does not prove real publication/vintage provenance,
market repricing accuracy or historical point-in-time eligibility.

## Evidence and alternatives

Inspected source: [InflationIndex](../../../../finstack-quant/core/src/market_data/scalars/inflation_index.rs),
[generic series storage](../../../../finstack-quant/core/src/market_data/scalars/primitives.rs),
[reference helpers](../../../../finstack-quant/valuations/core/src/instruments/common_impl/helpers.rs),
[bond indexation](../../../../finstack-quant/valuations/fixed-income/src/inflation_linked_bond/indexation.rs),
[calibration preflight](../../../../finstack-quant/calibration/src/validation/preflight.rs),
[inflation target](../../../../finstack-quant/calibration/src/targets/inflation.rs),
[Python scalar bindings](../../../../finstack-quant-py/src/bindings/core/market_data/scalars.rs)
and [WASM scalar bindings](../../../../finstack-quant-wasm/src/api/core/market_scalars.rs).
Existing monthly-reference, publication, seasonality, helper and serialization
tests were inspected or located, not executed.

[31 CFR356 Appendix B, 2025 edition, I.B.2–4](https://www.govinfo.gov/content/pkg/CFR-2025-title31-vol2/pdf/CFR-2025-title31-vol2-part356-appB.pdf#page=5)
specifies TIPS reference interpolation, denominator, unrevised/rebasing treatment
and publisher-announced delayed-index contingencies. Those are product-specific
rules, not permission to transform raw monthly source records. [BLS series identity](https://www.bls.gov/cpi/factsheets/cpi-series-ids.htm)
distinguishes seasonal status and other series attributes; [BLS seasonal guidance](https://www.bls.gov/cpi/seasonal-adjustment/using-seasonally-adjusted-data.htm)
describes revisions to adjusted historical data and the suitability of adjusted
versus unadjusted series. These support the identity/version distinction. The
selected immutable snapshot, date-level cutoff and explicit overdue-error
contract are architectural choices, not universal publisher prescriptions.

A general historical vintage database, automatic final/latest-value selector,
intraday release engine and forecast seasonal model are not introduced. The
minimum complete source is one explicitly supplied eligible fixing snapshot
with independent release assumptions and strict actual-consumer resolution.

## Follow-up tracking

Runtime reconciliation and independent qualification are
tracked in the [core-market follow-ups](../FOLLOW_UPS.md#core-market-data).
