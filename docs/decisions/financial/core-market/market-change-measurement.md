# CORE-MARKET-22: Explicit market-change quantities, coordinates and risk pairing

- Decision status: **Accepted — complete comparison quantity/coordinate/aggregation package adopted 2026-11-08.**
- Implementation status: **Observed** in the owning paths below; the accepted typed measurements, strict comparability, numerical and caller/host reconciliation are unimplemented.
- Validation status: **Primary/source evidence and independent basis, credit/inflation, numerical and consumer/host reviews inspected; 30 standalone rational/140-digit Decimal/binary64/domain examples executed 2026-11-08; production qualification pending.** Existing tests were inspected or located, not executed. Formula examples do not certify pricing, calibration, attribution accuracy or complete financial replay.
- Audit date: **2026-11-05**. Source revision: `7e82fd03ad98346727769e88dd5ac5d84b56e9fc`.
- Scope: the implementation listed here; the [slice inventory](README.md) and [coverage ledger](coverage.json) identify exact file coverage.

## Scope and accepted authority

This record owns comparison of selected market quantities, explicit sample
coordinates, ordered pointwise changes and optional equal-weight diagnostic
summaries. It preserves [accepted core-market01–21](README.md), including
[exact typed context selection](context-lifecycle.md#accepted-decision-typed-targets-and-atomic-context-lifecycle)
and [hierarchy action selection](hierarchy-selection.md#accepted-decision-typed-selection-and-explicit-ordered-overlap).
A measurement is not a shock, calibrated quote, sensitivity, model forecast or
claim that two sources represent the same economic risk.

Reuse [accepted factor log-difference arithmetic09](basis-spreads.md#accepted-decision-source-derived-dated-zero-rate-differences),
[hazard/quote/recovery ownership05](hazard-quotes-and-risk.md#accepted-decision-explicit-cds-observations-hazard-risk-and-recovery),
[CPI growth10](inflation-projection.md#accepted-decision-explicit-cpi-levels-and-anchored-annual-parameter-shocks),
[monthly fixing identity11](inflation-observations.md#accepted-decision-exact-monthly-fixing-snapshots-and-explicit-availability)
and [scalar identity12](scalar-series-and-fixings.md#accepted-decision-validated-values-literal-sampling-and-qualified-exact-fixings).
[Money precision](../core-conventions/012-money-decimal-precision.md#accepted-decision-monetary-precision-rounding-and-currency-identity),
[FX resolution](../core-conventions/013-fx-resolution-and-shocks.md#accepted-decision-fx-resolution-state-and-relative-shocks)
and [scaled compensated aggregation](../core-numerics/sample-statistics.md#accepted-decision-checked-sums-moments-quantiles-and-mean-uncertainty)
remain their existing canonical owners.

Actual CDS quote evaluation, product reference resolution, horizon/carry
decomposition, volatility measurement and P&L allocation retain their financial
owners. In particular, [attribution pairing](../attribution/08-metric-inputs-and-move-pairing.md),
[rates/credit allocation](../attribution/09-metric-rates-and-credit.md) and
[other factors](../attribution/10-metric-other-factors.md) retain their remaining
Draft methodological choices. Core does not acquire a dependency on calibration,
valuations or attribution.

## Foundational findings

- Equal numeric tenors on different source bases can identify different dates.
  An unchanged sloped source can therefore show a nonzero rolling-horizon
  difference. The comparison basis must be chosen before computing a move.
- Basis points specify a scale, not an economic quantity. Continuous-zero
  rates, native intensities, par CDS spreads, annual CPI growth and relative
  CPI levels cannot share one inferred risk factor because all can use10000.
- Latest CPI observations can be different months, vintages or index bases.
  Their difference does not identify a revision, publication surprise or
  comparable fixed-period change.
- A mean discards shape and implicitly defines sample weights. Missing points,
  repeated coordinates and default grids must not silently change that meaning.
- Required movement can disappear through tolerance snapping, a rounded
  quotient-minus-one, monetary float conversion or ordinary summation. Stable
  arithmetic must preserve the requested quantity rather than substitute zero.

## Accepted decision: explicit comparable quantities and ordered measurement rows

The complete package was accepted on **2026-11-08** following independent
basis, credit/inflation, numerical and consumer/host reviews. Runtime/caller/host
reconciliation and independent financial qualification remain pending;
acceptance does not certify the observed implementation.

1. **Select the quantity and both sources explicitly.** Use small family-owned
   Rust queries/params and ordered result rows, reusing accepted20's exact typed
   source targets and existing dates, clocks, currency and quote representations.
   The request names the quantity, earlier/later source association, coordinates
   and output scale. Equal IDs alone do not establish comparability; validate
   actual role, currency/index, collateral/funding where relevant, quote form,
   source clock and series/rebasing identity at the owning boundary.

   Remove storage-type credit dispatch, discount-then-forward retries, projected-
   CPI-versus-fixing availability preference and same-ID namespace fallback.
   Do not inspect inactive alternatives or infer a new quantity after a selected
   measurement fails. An explicitly qualified comparison may use different source
   IDs, retaining both actual identities; renaming is not silently inferred.
   No generic measurement registry, callback engine or new host class is required.

2. **Require resolved coordinates and expose the comparison basis.** The
   canonical factor-rate comparison uses the same explicit calendar anchor V
   and maturity T on both snapshots, with T>V and one complete reporting day
   count/context giving finite positive alpha. Each actual source maps those
   dates on its own retained clock. Do not substitute the same numeric tenor on
   each source, infer a reference date from context, or turn a displayed1Y into
   calendar dates without its actual convention owner.

   Native parameter/point and level queries identify their exact original
   coordinates or calendar/reference periods, with their actual source-clock
   meanings. A rolling-horizon/evolution analysis with different per-side dates
   must declare those dates and its aging-inclusive meaning at its financial
   owner; it is not silently produced by this fixed-calendar factor query.
   No generic rolling-horizon mode or universal carry formula is introduced.
   Conditioning does not publish fixings, freeze a live provider or change
   original source identity merely to make a comparison possible.

3. **Keep the supported quantities distinct.** A result identifies its
   quantity and scale rather than publishing an unqualified change_bp.

   | Quantity | Canonical measured change | Required comparison meaning |
   | --- | --- | --- |
   | Discount or index-projection continuous-zero rate | Later minus earlier dated continuous-zero rate; annual decimal, or bp once | Same V/T and explicit common reporting alpha; preserve the selected factor role and economic association |
   | Native hazard intensity | Later minus earlier lambda; per source-year, or explicitly intensity bp once | Compatible retained original clock/origin and exact named point/ending parameter; no CDS-spread inference |
   | Supplied CDS or other typed quote | Later minus earlier value in that quote's actual native units | Exact matched quote/contract/basis; full implied quote generation stays at its pricing owner |
   | Projected CPI level | Later minus earlier native index level | Same explicit date/reference coordinate and index/rebasing/seasonal identity |
   | Projected CPI annual growth | Later minus earlier checked source-clock annual decimal growth; optional rate bp | Explicit common endpoints and compatible retained original-clock interval meaning under accepted10 |
   | Observed monthly CPI level | Native index-point difference or explicitly labelled unannualized relative-level change | Explicit periods/vintages, same series/rebasing/seasonal identity and qualified availability under accepted11 |
   | FX quote | Explicit ordered-pair proportional change; decimal fraction or percentage points | Complete per-snapshot query/date/policy and the same directed quotation meaning |
   | Opaque scalar/native price | Native difference or explicitly requested algebraic proportional change | Same scalar kind and actual native unit; Money currency agreement and checked Decimal ownership |

   A native point measurement is not automatically a source-parameter shock
   coordinate. Hazard point averages or integrated hazard divided by an arbitrary
   reporting year are not hazard01 parameters. A continuous-zero comparison for
   an instrument's credit-role discount source remains that explicit quantity,
   not a CDS spread or automatic CS01 coordinate. Price/VolIndex source-level,
   volatility, correlation and other unrequested diagnostics retain their own
   owners; this record adds no inferred conversion between those quantities.

4. **Reuse one checked dated factor-difference calculation.** For each snapshot
   s, let ell_s=ln D_s(T)-ln D_s(V), using its actual selected discount or
   projection-factor evaluator. Report later-minus-earlier
   delta_z=(ell_0-ell_1)/alpha. Reuse accepted09's four-endpoint scaled,
   cancellation-aware log cross-ratio and accepted01/02/03/07 source/view
   admission. All four absolute endpoints must be finite positive within their
   actual domains; source underflow is not rescued by a finite formal ratio.
   Validate V/T, reporting context and alpha before identities. Do not separately
   annualize enormous rates, construct unrequested relative factors or subtract
   rounded cached logs. Convert to bp once with checked output range.
   This is the accepted comparison definition, not a claim that the current
   native-zero API or every sensitivity already uses that reporting clock.

5. **Remove inferred CDS proxies and presence-driven inflation measurement.**
   Reconcile the existing removal of cds_quote_bp, ParInterp, core par-spread
   measurement and Hazard-versus-Discount credit dispatch under accepted05.
   Core may subtract already qualified typed quote observations or compare
   native intensities. Generating an implied CDS quote requires the full
   declared CDS contract, dates, dependencies and cash-recovery assumptions at
   the canonical CDS/pricer owner. Recipe presence is historical evidence;
   accepted06 still governs request-time replay/current-fit qualification.
   Neither equal quote IDs/pillars nor lambda*(1-R) proves equal quote meaning.

   CPI projection uses accepted10's checked positive-level/growth evaluator,
   including tau_B(T)-tau_B(A) on each retained original clock, not a fresh
   DayCount(A,T), raw quotient/powf or valuation rebase. A native-growth
   comparison requires compatible clock/origin semantics for those coordinates;
   clock changes cannot be reported as pure market movement. If the retained
   interval meanings differ, the native-growth request errors rather than
   silently using a common-report annualization. Such a separately defined
   report quantity remains its owner's responsibility. Annual-growth parameter
   risk and modeled CPI level changes are separate measurements.

6. **Make observation comparisons period- and knowledge-aware.** Replace
   latest-print-to-latest-print selection with explicitly named monthly
   reference periods and the exact selected snapshot version, index/rebasing/
   seasonal identity and knowledge cutoff K on each side. Qualify actual
   availability under accepted11; advancing valuation alone does not advance K.
   Same-period changed vintages are labelled revisions. Explicitly requested
   cross-period level evolution retains both months and is not called a revision
   or an annualized rate. A newly published month missing in the opening fixing
   snapshot is unavailable for an observed-versus-observed comparison; do not
   substitute the prior month's latest print or a projected level.

   Forecast-versus-actual publication surprise, mixed observed/projected
   contractual reference CPI and return/indexation calculations remain explicit
   upper-owner operations using their qualified inputs. Do not automatically
   feed a level revision/evolution into the same Inflation01 used for an annual
   growth-parameter shock. No new vintage database or automatic revision model
   is needed: compare the actual qualified snapshots supplied.

7. **Use exact algebraic scalar and explicit FX changes.** For finite generic
   values, native delta is x1-x0; algebraic proportional delta is
   (x1-x0)/x0 with an exactly nonzero baseline. Negative and tiny nonzero
   baselines are admitted for that explicitly named algebraic quantity.
   Remove the1e-15 cutoff; no automatic positive-price return, log return or
   denominator replacement is inferred. Zero baseline errors even for0→0;
   an unrequested native difference need not be representable for a requested
   proportional difference to succeed.

   Near equality, preserve the actual difference before division instead of
   subtracting a rounded x1/x0 from1. For extreme scales/signs, use checked
   equivalent scaled/ratio algebra to avoid an unnecessary overflowing
   difference or ratio. A genuinely unrepresentable requested finite output
   errors, as does required nonzero arithmetic lost to zero without a qualified
   stable reformulation. Validate inputs before equal-value zero shortcuts.

   For FX, require same directed base/quote pair and complete explicit per-side
   query/date/policy, delegating resolution once to accepted FX013. Retain the
   actual resolved quote/path/provenance needed to identify the comparison.
   Required controls validate first; valid same-currency quote movement is zero
   without requiring unused FX matrices. Do not infer inverse-pair negation,
   average reciprocal quotes or silently default a missing policy. Both finite
   positive quote operands and the final requested movement require admission.

8. **Preserve Money through the actual requested operation.** Money native
   differences use accepted012's exact checked same-currency subtraction and
   return Money; do not subtract rounded amount() views or silently convert
   currencies. Unitless-versus-Money and different currencies error. The caller
   owns any explicitly requested conversion before comparison.

   Monetary algebraic proportional change reuses accepted012's dimensionless
   Money/Money ratio and numerical budget, without a bounded Decimal quotient
   intermediate. Near equality, form the exact admitted monetary difference
   before that ratio to preserve small movements at large levels. Where an
   unrequested absolute difference would overflow, use equivalent ratio/scaled
   algebra rather than requiring it. Keep final model-f64 conversion explicit;
   do not round movement to a currency's minor units, a tolerance or display
   precision. Exact subtraction of representable inputs that produces an
   unrepresentable required Money output errors at the existing monetary owner.

9. **Use one strict ordered row set for vectors and summaries.** Requests are
   nonempty ordered unique sample lists with explicit coordinates. Curve grids
   have their family's strict coordinate order/domain; multi-source/quote rows
   retain declared order and full typed row identity. Reject duplicate full
   coordinates before collection; do not sort/deduplicate, skip nonpositive
   points, infer weights from repetitions, or sample only the baseline knots.
   An origin point is valid only for quantities whose source/quantity admits
   it; a factor rate still requires a positive dated interval. Union/intersection
   construction and risk grids are explicit caller choices with required source
   admission, not implicit reconciliation.

   Remove TenorSamplingMethod's automatic Standard/Dynamic selection and the
   core default standard grid paths. Existing caller menus may resolve actual
   contractual/calendar coordinates explicitly at their owner; no grid is
   universally liquid or suitable because it has a standard-looking tenor list.
   Pointwise and mean routes use exactly the same checked row calculation.
   Remove1e-14 magnitude-based equality snapping and per-route tolerance/skip
   differences. Exact admitted equality may produce0; otherwise retain actual
   representable movement or return the existing computation error.

10. **Limit aggregation to an explicit homogeneous diagnostic.** An optional
    arithmetic mean consumes the exact qualified binary64 row set, in order,
    with equal weight1/N and the same quantity/unit/comparison basis. Use
    accepted numerics09's canonical scaled compensated mean; do not require
    an unrequested raw sum to fit, or write a second local numerical kernel.
    Rows and their basis/population remain available alongside the summary.
    A zero signed mean can hide a twist; it does not establish no market change,
    a parallel shift or a risk-weighted movement. Do not average Money by first
    converting its rows to floats; monetary aggregation retains its own owner.
    New arbitrary weight/normalization and curve-fitting frameworks are not
    part of this package.

    Every required row must succeed. Missing, wrong-kind, incompatible,
    unavailable or nonfinite rows error the requested comparison/summary with
    exact source/coordinate context. Do not publish a successful partial mean,
    omit a required source, or substitute zero after failure. Explicit optional
    detail may have an unavailable status at its output owner; it is never
    silently inserted as an unchanged row or included under a changed count.

11. **Pair risk by quantity and actual coordinates, not scale alone.**
    Attribution/risk consumers must use producer-defined source/role, exact
    parameter or quote coordinates, timing/clock, currency and bump unit.
    Parameter hazard01 pairs with that intensity basis; quote CS01 requires the
    actual matched/recalibrated quote basis. Native spot Delta pairs with native
    price movement; percent-axis risks pair with an explicitly proportional move.
    Per-pair FX exposures cannot be collapsed to one primary pair's movement
    merely because their sensitivities share a display unit.

    An unweighted mean times aggregate DV01/CS01/Inflation01, or one scalar
    curvature proxy, is not certified by valid measurements. The allocation
    owner must qualify its exact mapping or explicit approximation; buckets,
    uniform-move conditions, model revaluation and error/status policy retain
    their stated authorities. CORE22 does not select a new P&L decomposition,
    risk-weighting scheme, mixed-gamma approximation or residual tolerance.

12. **Reconcile real consumers, transport and errors in place.** Cover core
    diff queries, shared numerical/Money/FX owners and actual attribution
    metrics/Taylor/credit-cascade helpers. Remove discount/forward retries,
    first-scalar/primary-pair assumptions where they claim complete measurement,
    error-to-absence/zero paths and duplicate CPI arithmetic for required rows.
    Retain existing stronger quote-ID/pillar binding and extend it to actual
    contract/quantity comparability; do not replace it with a coarser mean.

    Current core diff helpers have no direct standalone Python/WASM export.
    Reconcile existing attribution request/result APIs, schema, errors, stubs,
    declarations, parity contract and examples when changed, rather than
    inventing new host measurement classes. Preserve literal ordered rows,
    typed targets, quantities/units, actual coordinates/clocks, selected source/
    observation/query facts and summary basis in existing report structures.
    Shared strict bounded loading and canonical content identity apply; no
    host-side financial arithmetic, migration reader or fallback codec is added.

## Observed implementation and discrepancies

diff.rs uses a default9-point numeric-year grid, baseline-only Dynamic knots or
Custom values validated only for finite nonempty input. Mean sampling skips
t<=0, applies a1e-14 relative equality snap and sums naively. Per-tenor routes
neither skip nor snap in the same way. Duplicate coordinates alter weights.
Equal numeric t is evaluated on each source's own base/clock.

Hazard movement uses cds_quote_bp/ParInterp, and credit movement chooses spread
versus discount-zero meaning from baseline storage type. CPI growth duplicates
raw ratio/powf arithmetic; observed inflation compares the latest raw print from
each source; paired availability chooses projected versus observed meaning.
These conflict with already accepted source/quote/knowledge boundaries and
require reconciliation, not adoption of those old proxies.

FX movement uses default policy, always requires both matrices and omits an
explicit final-ratio range check. Scalar proportional movement rejects
abs(x0)<1e-15; native scalar kind/currency checks already exist, but Money
amount() conversion occurs before subtraction. Required finite input pairs can
still have an unrepresentable output, and rounded monetary views can erase an
otherwise exact movement.

Actual attribution consumers retry discount versus forward, omit some missing/
invalid measurements, use means for aggregate sensitivities, and use primary
scalar/FX proxies. Some per-bucket credit paths already retain exact quote IDs
and pillar dates, which must be preserved. Volatility has a different
measurement owner and is not silently absorbed into this core package.
These are inspected implementation/contract findings; their complete production
financial impact was not reproduced by this review.

## Independent examples and qualification limits

On **2026-11-08**,30 standalone exact-rational/140-digit Decimal/binary64/domain
cases passed, covering the quantity/coordinate and arithmetic examples below.
They do not import production modules or run Rust/host pricing, replay,
attribution or risk.

- With unchanged K(t)=0.01t+0.005t², a one-year zero at origin is1.5%; after
  moving the reference to1, the next one-year zero is2.5%. The apparent100bp
  difference is rolling-horizon evolution. Comparing the same interval[1,2]
  on the same retained source gives0.
- The same factor exp(-0.05), annualized on365/360 versus365/365, produces
  different own-clock rates (about6.849315bp apart). Common reporting
  annualization produces0 change.
- Lambda0.01→0.02 is+100 intensity bp, while changing R0.4→0.8 makes the
  triangle proxy60→40bp. Fixed lambda0.02 with R0.4→0.2 gives a40bp proxy
  change with zero intensity movement; neither is a full CDS quote fixture.
- Latest February303→March306 gives about99.009901 relative-level bp;
  January300→301 revision gives33.333333bp. Rebasing105→210 manufactures
  an apparent10000bp if basis identity is ignored. CPI100→121 over two years
  gives10% annual growth versus21% cumulative growth.
- Native scalar-100→-80 gives+20 while its algebraic proportional change
  is-20%. Tiny1e-20→2e-20 is+100%. Opposite extreme binary64 values can
  have a finite-200% proportional move despite overflowing absolute difference.
  Finite positive FX operands can instead require an unrepresentable movement.
- Exact Money10^20→10^20+1 has native delta1 and proportional1e-18%;
  both rounded float amount views can be equal. A1e-14 equality snap erases
  a representable approximately0.009536743bp move in another large-level case.
- Mean[MAX,MAX] remains MAX despite raw-sum overflow; compensation retains
  the residual of[1e16,1,-1e16]. Repeated coordinates change equal weighting.
  Opposite bucket movements+50/-50 with sensitivities200/100 give5000, while
  aggregate300 times mean0 gives0. Native Delta2 on spot200→210 gives20;
  percent substitution gives10. Two FX sensitivities100/200 with pair
  moves+1%/-1% give-100, while joint300 times primary+1% gives300.

Independent qualification must cover actual typed/economic source association,
fixed calendar/reporting versus native original clocks and conditioning,
source range/admission and cancellation; full quote and CPI period/vintage/
knowledge identity, projection/observation separation and supported actual risk
coordinates; tiny/negative/zero/extreme scalar domains, exact Money and complete
FX query/range behavior; strict row grids, duplicates/order, same point/mean
calculator, scaled mean and required errors; actual bucket/aggregate/Taylor/
cascade/host/schema/metadata contracts against independent financial fixtures.
Abstract expressions and synthetically uniform moves do not certify those
results, full CDS quote correctness or P&L attribution accuracy.

## Evidence and alternatives

Owners: [core diff and inline tests](../../../../finstack-quant/core/src/market_data/diff.rs),
[metric input context](../../../../finstack-quant/attribution/src/metrics_based/context.rs),
[rate/credit/CPI pairing](../../../../finstack-quant/attribution/src/metrics_based/shifts.rs),
[rate allocation](../../../../finstack-quant/attribution/src/metrics_based/rates.rs),
[credit allocation](../../../../finstack-quant/attribution/src/metrics_based/credit.rs),
[FX allocation](../../../../finstack-quant/attribution/src/metrics_based/fx.rs),
[inflation allocation](../../../../finstack-quant/attribution/src/metrics_based/inflation.rs),
[credit cascade](../../../../finstack-quant/attribution/src/credit_cascade.rs),
[Taylor](../../../../finstack-quant/attribution/src/taylor.rs),
[missing-measurement handling](../../../../finstack-quant/attribution/src/helpers.rs),
[Python attribution](../../../../finstack-quant-py/src/bindings/attribution/entry.rs)
and [WASM attribution](../../../../finstack-quant-wasm/src/api/attribution/mod.rs).
Existing [metric attribution tests](../../../../finstack-quant/attribution/src/metrics_based/tests.rs)
and [stored QuantLib parity fixtures](../../../../finstack-quant/attribution/tests/attribution/quantlib_parity.rs)
were inspected or located, not executed or independently regenerated.

[QuantLib's implied-term-structure API](https://raw.githubusercontent.com/lballabio/QuantLib/master/ql/termstructures/yield/impliedtermstructure.hpp)
distinguishes an implied reference date from the original source. This supports
making reference-date changes explicit, not adopting its scalar offset algorithm
or claiming a financial comparison benchmark. The
[BLS CPI FAQ](https://www.bls.gov/cpi/questions-and-answers.htm) distinguishes
monthly/index/seasonal identities and series-dependent revisions; it illustrates
why a latest print is not a universal comparison coordinate. It does not select
this library's CPI risk mapping or rules for all statistical agencies.

Fixed-calendar comparison, explicit native quantities, algebraic signed changes,
strict row failure and equal-weight diagnostic summaries are this contract's
design choices. Rolling-horizon reports, full quote repricing, risk-weighted
movements and portfolio attribution retain their actual owners; they are not
silently inferred from stored IDs, default tenors or successful arithmetic.

## Follow-up tracking

Runtime reconciliation and independent qualification remain pending and are
tracked in the [core-market follow-ups](../FOLLOW_UPS.md#core-market-data).
