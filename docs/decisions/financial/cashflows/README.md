# Cashflows methodology audit

Audit date: **2026-11-05**. Source revision: **`7e82fd03ad98346727769e88dd5ac5d84b56e9fc`**.

[CASHFLOWS-001 — Contractual periods, calendars and day-count context](01-schedule-periods.md#accepted-decision-contractual-periods-calendars-and-day-count-context)
is **Accepted on 2026-11-08** after complete whole-record review.
[CASHFLOWS-002 — Coupon and payment program compilation](02-program-compilation.md#accepted-decision-coupon-and-payment-program-compilation)
is **Accepted on 2026-11-08** after complete whole-record review.
[CASHFLOWS-003 — Dated opening principal, signed cash and atomic event phases](03-principal-and-emission-order.md#accepted-decision-dated-opening-principal-signed-cash-and-atomic-event-phases)
is **Accepted on 2026-11-08** after complete whole-record review.
[CASHFLOWS-004 — Explicit amortization laws and live-balance installments](04-amortization.md#accepted-decision-explicit-amortization-laws-live-balance-installments-and-monetary-boundaries)
is **Accepted on 2026-11-08** after complete whole-record review.
[CASHFLOWS-005 — Parent-coupon accrual, exact splits and capitalization](05-coupon-cash-and-pik.md#accepted-decision-parent-coupon-accrual-exact-splits-and-capitalization)
is **Accepted on 2026-11-08** after complete whole-record review.
[CASHFLOWS-006 — Resolved term observations and exact rate controls](06-floating-rate-projection.md#accepted-decision-resolved-term-observations-exact-rate-controls-and-strict-failures)
is **Accepted on 2026-11-08** after complete whole-record review.
[CASHFLOWS-007 — Overnight blocks, cumulative growth and parent allocation](07-overnight-compounding.md#accepted-decision-resolved-overnight-blocks-cumulative-growth-and-parent-allocation)
is **Accepted on 2026-11-09** after complete whole-record review.
[CASHFLOWS-008 — Parent-bound replay, exact full settlement and timing ownership](08-floating-replay.md#accepted-decision-parent-bound-replay-exact-full-settlement-and-explicit-timing-ownership)
is **Accepted on 2026-11-09** after complete whole-record review.
[CASHFLOWS-009 — Explicit fee parents, exact amounts and literal tier selection](09-fees.md#accepted-decision-explicit-fee-parents-exact-amounts-and-literal-tier-selection)
is **Accepted on 2026-11-09** after complete whole-record review.
[CASHFLOWS-010 — Checked mortality and prepayment assumptions](10-prepayment-and-mortality.md#accepted-decision-checked-monthly-mortality-explicit-model-clocks-and-benchmark-boundaries)
is **Accepted on 2026-11-09** after complete whole-record review.
[CASHFLOWS-011 — Explicit default targets, strict capacity and recovery authority](11-default-and-recovery-assumptions.md#accepted-decision-explicit-default-targets-strict-capacity-and-one-recovery-authority)
is **Accepted on 2026-11-09** after complete whole-record review.
[CASHFLOWS-012 — Validated schedules, balances and explicit cash views](12-schedule-lifecycle.md#accepted-decision-validated-complete-schedules-explicit-balances-and-separate-cash-views)
is **Accepted on 2026-11-09** after complete whole-record review.
[CASHFLOWS-013 — Cash-coupon quotation and entitlement boundaries](13-accrued-interest.md#accepted-decision-original-parent-cash-quotation-and-inclusive-first-ex-dates)
is **Accepted on 2026-11-09** after complete whole-record review;
[CASHFLOWS-014 — Exact nominal reporting and explicit repayment life](14-aggregation-and-life.md#accepted-decision-exact-nominal-reporting-and-explicit-repayment-life)
is **Accepted on2026-11-09** after complete whole-record review;
[CASHFLOWS-015 — Product-owned credit valuation and qualified PV reporting](15-credit-adjusted-pv.md#accepted-decision-product-owned-credit-valuation-and-qualified-pv-reporting)
is **Accepted on2026-11-09** after complete whole-record review;
[CASHFLOWS-016 — Qualified hypothetical fixing materialization](16-fixing-materialization.md#accepted-decision-qualified-hypothetical-fixing-materialization)
is **Accepted on2026-11-09** after complete whole-record review.
**All16 records are Accepted; none remain Proposed or Draft.**
Implementation status is **Observed within the defined file scope**; runtime
reconciliation and independent qualification remain pending. Production validation
is **Evidence inspected/located, not executed**; independent reference examples
are reported separately in records001–016. Existing code/tests establish observed
behavior, not financial adoption or universal correctness. Outstanding work is
tracked in the [cashflow follow-ups](../FOLLOW_UPS.md#cashflows).

The complete accepted001 package inherits accepted core date/grid/day-count rules,
removes broad presets and generic CDS overrides, and retains original nominal
phase/context, explicit calendar/reset/accrual roles and coincident payment
identity. It composes ICMA nominal slices, preserves full-context ACT365L and
explicit termination facts, guards immutable period queries and unifies the
single-period helper with canonical grid admission. The narrow core015 metadata endpoint clarification was accepted with this
package on2026-11-08 and applied to that existing record in place. Program/window/coupon/replay/reporting decisions retain their
separate owning records; this package changes no runtime or generated artifact.

The complete accepted002 package selects independent atomic coupon/payment
windows explicitly, including re-anchoring from redundant cuts and strict001
anchor/context admission. It replaces sticky errors with atomic fallible updates,
requires globally valid strict payment nesting, explicit settlement/exchange and
owner observation cutoff, and removes implicit Cash tails/curve-date inference.
Fixed period-start rates, margin FROM and payment UNTIL remain distinct disclosed
operators. Exact monetary state, genuine source errors and complete local plan
publication retain accepted authority; actual helpers/specs/hosts/schema/parity
require reconciliation. Broader amount/principal/fixing/lifecycle choices remain
with their own records. The complete package is accepted; runtime and independent
financial/host/geometry qualification remain pending.

The complete accepted003 package requires an explicit dated opening stock and
covered economic history, required signed cash independent of principal effects,
atomic same-date principal batches and unique paired terminal dates. It discloses
coupon/PIK/event/amortization/fee/redemption phases and the explicit
automatic par-exchange recipe without universal market claims. It preserves exact
rows and monetary state and identifies replay/export/host reconciliation; amount,
fixing, amortization and fee laws keep their owners. The whole package is
accepted; runtime/host reconciliation and independent qualification remain pending.

The complete accepted004 package requires an explicit amortization rule and exact
Decimal fractions, defines one actual-coupon cadence and economic/payment mapping,
protects LinearTo's literal final target and replaces cached LinearBetween draw
adjustments with live-balance/remaining-date installments. It discloses all seven
laws, no implicit financial rounding and exact par cash/principal effects, with
strict admission and existing consumer/host reconciliation. The package is
accepted; runtime/host reconciliation and independent qualification remain pending.

The complete accepted005 package selects original-parent cumulative accrual
increments instead of re-anchored balance-slice day counts, one whole-parent
monetary restoration and one exact cash-share/residual-PIK allocation. It preserves
internal signed contributions and truthful parent authority, requires nonnegative
generated PIK per parent, and emits enabled cash/noncash components including
zero with the accepted dates/effects. It identifies strict nested coupon ingress
and actual replay/host/consumer reconciliation without adopting later rate,
observation, entitlement or reporting laws. The package is accepted; runtime/host reconciliation and independent
qualification remain pending.

The complete accepted006 package inherits index-specific factor projection and
qualified observed identity, separates index annualization from coupon weights,
and unifies exact published-fixing versus explicitly unpublished-projection
selection across builder/RateSpec. It removes error-driven substitutes and
calendar/tenor/preset inference, requires explicit ordinary-floater controls and
one bounded Decimal contractual-rate stage, and retains truthful provenance and
product/host reconciliation. Rates feed accepted005 monetary amounts without a
float round trip; overnight/model owners retain their decisions. The package
is accepted; runtime/host reconciliation and independent qualification remain pending.

The complete accepted007 package separates actual overnight index periods from
coupon carry weights, selects full-business-block lookback/shift/cutoff geometry
and proportional shifted partial weights, and preserves frozen quotes/active
carry in checked native-growth replay. It discloses model and monetary admission
limits, a fixed full-parent clock bridge and compound-rate contributions under
changing principal. Period reference caps need not cap weighted cash; partial
state cannot manufacture future bound adjustments. Actual input/state/host and
cross-engine qualification retain their separate owners. The package is accepted; runtime/host/state reconciliation and independent
qualification remain pending.

The complete accepted008 package binds state to its immutable original parent,
separates qualified observation progress from exact principal capture, and keeps
one constant-principal full-amount profile through005/006/007. It removes generic
partial accrual and arbitrary segment settlement, preserves repeatable nonposting
getters and distinguishes qualified forecasts from actual early-payment knowledge.
Actual builder/bond/host handoffs are identified without adopting exercise,
entitlement or lifecycle rules. The package is accepted; runtime and independent
complete-path qualification remain pending.

The complete accepted009 package selects the limited periodic-fee grid explicitly,
renames balance methods Opening/Integrated, uses each fee parent's original
accrual weights and one complete exact Money restoration, and defines static
undrawn clipping before weighting. It retains literal fixed cash/date/currency,
signed/enabled zero rows and strict nullable tier lookup without automatic
product policy. Existing builder/product/host handoffs are identified; runtime reconciliation and independent complete-path qualification remain pending.

The complete accepted010 package selects strict stable monthly/annual mortality
conversion, immutable whole-model admission and one-based applied-month queries.
It adopts primary PSA saturation with preserved near-cap survival, removes ABS's
epsilon depletion and its original-dollar guarantee, and names Lockout/annual
CPR vector assumptions explicitly. Generic clamping and unchecked PSA helpers
are removed, with actual product/host/clock reconciliation identified. Default/
recovery models and asset cashflow ordering retain their owners. Runtime reconciliation and independent complete-path qualification remain pending.

The complete accepted011 package separates conditional rates from original-
reference targets, requires explicit actual-capacity context and selects strict
represented-model feasibility instead of hidden clipping. It defines SdaAge,
normalized annual timing weights and direct target increments, with cumulative
views kept separate. Exclusive recovery sources preserve the primary rate/
severity and economic-default vintage while actual Money/date/queue owners retain
their rules. Existing consumer/host/parity reconciliation and precise primary
source limits are identified; runtime reconciliation and independent
complete-path qualification remain pending.

The complete accepted012 package preserves complete source schedules and removes
implicit residual rewriting, separating explicit native settlement views from
principal replay. It uses optional dated opening/through coverage, exact atomic
economic-date closing stock and guarded immutable construction, while removing
fictional per-row balances, generic scaling, placeholder success and lossy ingress.
Restricted native collection merge and actual product-owned composition retain
truthful context without invented scalar principal. Runtime/caller/host
reconciliation and independent complete-path qualification remain pending.

The complete accepted013 package selects linear cash-coupon quotation from the
actual original-parent authority, explicit currency and owner-qualified full cash
amount. Inclusive per-parent first-ex dates define full-claim subtraction even
before accrual starts; earning, PIK and actual settlement remain separately owned.
It removes generic compounding/configuration, flattened-row inference and offset
helpers, using one final Money restoration and matching one-shot/index/host paths.
Primary product-clause scope and actual accounting/exercise/quote consumers are
identified; runtime reconciliation and independent complete-path
qualification remain pending.

The complete accepted014 package uses exact native Money sums and the existing
validated PeriodPlan, retaining full calendar context with sparse ordered totals.
It separates typed payment-year cash ladders from qualified native-PV reporting
at the valuation owner. It replaces generic WAL with explicit dated par weights,
ACT/365F and same-day/nullable rules, correcting SIFMA attribution while preserving
product-specific clocks and classification. Actual consumer/host/state
reconciliation and independent qualification remain pending; the complete package is accepted.

The complete accepted015 package removes cashflow-kind credit valuation,
inferred FIFO recovery and schedule PV shortcuts. Ordinary qualified cash
reuses core016 discounting; product owners supply meaningful native PV
attribution for014's existing exact aggregation. Strict source/expectation/
claim/date authority, explicit unsupported decomposition, local probability
annotations and canonical-price reconciliation retain their actual owners.
Runtime/caller/host reconciliation and independent qualification remain pending;
the complete015 package is accepted.

The complete accepted016 package selects explicit owner-qualified hypothetical
observations instead of an observation-date window, with separate required keys
and finite raw candidates, exact conflicts and qualified existing-data precedence.
It preserves the market together with all retained model-observation/source roles,
requires configured series/policies and stages a strict atomic merge. Actual
publication/model/dependency preparation, successive roll support and host/state
qualification retain their owners. The complete016 package is accepted; runtime and
independent complete-path qualification remain pending.

## Scope and completeness

The assigned slice contains **35 Rust source files** under `finstack-quant/cashflows/src`: **30 algorithm-bearing files and 5 support files**, with no test-only or generated source files. Every file was read and classified exactly once in [coverage.json](coverage.json). Inline tests were located, selected formula/state examples inspected, and production functions following inline test blocks were included. Sixteen records cover the meaningful algorithm choices below; source-file count is not used as a substitute for algorithm coverage.

This scope includes schedule construction, coupon/program compilation, economic/cash timing, principal and amortization, term/overnight rates, replay, fees, credit assumption calculators, accrued interest, balance reconstruction, residual normalization, nominal/PV aggregation, WAL and fixing materialization. No inverse solver, stochastic RNG, regression fit, calibration optimizer or exercise decision is implemented in these owned files. The replay APIs support exercise-date consumers but do not choose exercise. Discount/hazard/forward curve kernels and holiday/day-count arithmetic are delegated to core; product-specific defaults/prepayment cashflow loops and swap-style overnight engines belong to their owning slices.

The [crate manifest](../../../../finstack-quant/cashflows/Cargo.toml) was also inspected: default features enable JSON Schema and schema-resource validation; there is no feature-selected alternative financial algorithm. Serde/spec field defaults and calendar IDs are material inputs and are documented in the records. `schema.rs`, schema CLI and module exports are support because they package schemas/examples or re-export algorithms; `specs/mod.rs` is algorithm-bearing because it implements probability-vector selection/validation. Generated JSON schemas are not counted as Rust sources. [tests/main.rs](../../../../finstack-quant/cashflows/tests/main.rs) registers the cited consolidated integration-test modules.

## Methodology inventory

| Calculation / supported instruments | Implemented method / material variants | Actual selection | Decision record | Implementation / validation |
|---|---|---|---|---|
| Contractual date grids for fixed/floating legs | Plain tenor, front/back stubs, EOM, IMM, quarterly CDS twentieth grid | ScheduleParams; CDS overrides tenor/stub | [CASHFLOWS-001](01-schedule-periods.md) | date_generation + periods; grid/context tests located |
| Accrual versus cash/reset dates | Optional adjusted accrual; payment BDC then business-day lag; reset subtracts business days | Bond presets unadjusted; swap presets adjusted; fixing calendar can differ | [001](01-schedule-periods.md) | Shared endpoints, zero-lag and collapsed-date branches inspected |
| Stub and partial day-count context | Raw-grid stub classification; ICMA quasi coupons; ACT/365L fixed denominator; terminal flag only at final endpoint | DayCount/frequency/roll/context | [001](01-schedule-periods.md) | period_accrual and context helpers; ICMA/leap tests located |
| Market schedule presets | USD generic/SOFR/corporate/Treasury, EUR €STR/government, GBP SONIA, JPY TONA | Explicit convenience constructors | [001](01-schedule-periods.md) | Exact preset table inspected |
| Coupon/payment windows | Sorted cut points; independent per-window grids; exactly one coupon; nested payment override | Fluent builder and JSON variants | [CASHFLOWS-002](02-program-compilation.md) | compiler + coupon_api; overlap/window tests located |
| Step-up versus floating margin programs | Fixed steps apply by raw period start; floating steps rebuild windows from each date | Explicit program variant | [002](02-program-compilation.md) | Step-up and margin-step branches inspected |
| Cash/PIK payment program boundaries | Supplied split runs until each date; cash after final date | payment_split_program or explicit windows | [002](02-program-compilation.md) | Step-date validation and selection inspected |
| JSON dispatch and defaults | Strict fields, canonical Rust builder, curve-base cutoff, initial/final exchange default | build_cashflow_schedule_json/build spec | [002](02-program-compilation.md) | json.rs inspected; strictness/roundtrip evidence located |
| Economic principal events | Explicit par delta independent of cash price/payment date; pre-issue history retained | add_principal_event | [CASHFLOWS-003](03-principal-and-emission-order.md) | Checked Decimal state; dated draw/repayment regressions located |
| Cashflow emission ordering | Coupons → PIK → events → amortization → fees → installment update → redemption | Fixed internal pipeline | [003](03-principal-and-emission-order.md) | Pipeline read; PIK/terminal-date tests located |
| Initial funding/final redemption | Negative issue funding and positive final balance; redemption follows terminal coupon payment | InitialAndFinal default; None explicit | [003](03-principal-and-emission-order.md) | Effective-date gates and no-coupon fallback inspected |
| Scheduled principal reduction | None, LinearTo, StepRemaining, original-percent, remaining-percent, CustomPrincipal | One AmortizationSpec | [CASHFLOWS-004](04-amortization.md) | Preparation, bounds and Decimal emission inspected |
| Windowed equal-principal amortization | Capture live opening balance; later draws over remaining dates; final full sweep | LinearBetween | [004](04-amortization.md) | Start/end alignment and later-draw branches; regressions located |
| Fixed coupon computation | Segmentwise `B×r×year_fraction`, checked Decimal products | Fixed/StepUp compiled leg | [CASHFLOWS-005](05-coupon-cash-and-pik.md) | Balance segments and terminal context inspected |
| Coupon split and capitalization | Cash/Pik/Split; negative cash allowed; fixed negative PIK rejected; floating signed segments with nonnegative net | CouponType plus payment override | [005](05-coupon-cash-and-pik.md) | Signed floating PIK formula test inspected |
| Term forward projection | Embedded-tenor forward at reset; convert curve to contract annualization | Simple/unset compounding; curve ID | [CASHFLOWS-006](06-floating-rate-projection.md) | rate_helpers and emission; reset/basis tests located |
| Floating rate transformation | Index bounds → spread/gearing → all-in bounds; spread-inclusive or affine gearing | Explicit flags; spread-inclusive default | [006](06-floating-rate-projection.md) | Checked params and infallible helpers inspected |
| Fixing cutoff and missing data | Exact past fixing, same-day preference/project, future fixings ignored; Error/SpreadOnly/FixedRate | build_as_of or curve base; Error default | [006](06-floating-rate-projection.md) | Supplied-series validation and full-coupon fallback inspected |
| Facility/tranche term RateSpec | Same projection kernel; fixing required on/before valuation; any explicit compounding rejected | RateSpec::try_rate_for_period | [006](06-floating-rate-projection.md) | Distinct branch read |
| Overnight observation dates | In-arrears, lookback, shifted observations/weights, cutoff, leading weekend carry | Explicit compounding method/calendar | [CASHFLOWS-007](07-overnight-compounding.md) | Full compiler read; carry formula tests inspected |
| Overnight numerical aggregation | Weighted arithmetic average or product of simple daily factors; index-basis conversion | SimpleAverage versus compounded variants | [007](07-overnight-compounding.md) | Positive factors, finite rates, basis checks inspected |
| Overnight floor/cap placement | Daily index constraints before product or period constraints after aggregation | Daily default; Period explicit | [007](07-overnight-compounding.md) | Daily/period examples inspected |
| Overnight changing principal | Difference cumulative prefix amounts; final period-bound adjustment spread over accrual time | Builder's fixed internal algorithm | [007](07-overnight-compounding.md) | Uniform-adjustment and signed-PIK test formulas inspected |
| Incremental overnight replay | Replace partial carry factor; cache same fixing; monotone transactional advance | Reusable accumulator | [007](07-overnight-compounding.md) | Weekend checkpoint formula inspected; atomicity tests located |
| Floating coupon state and settlement | Fix rate separately from capturing notional; require complete replay; split finite amount | CompiledFloatingCoupon | [CASHFLOWS-008](08-floating-replay.md) | State/domain evidence located; full parent-bound/exact package accepted2026-11-09 |
| Replay accrued amount | Stored notional × elapsed fraction × replayed/fixed all-in rate | Explicit compiled-coupon method | [008](08-floating-replay.md) | Observed guard/context inspected; generic monetary method removal accepted2026-11-09 |
| Fixed/periodic fee amount | Raw dated fixed amount; annual bp on drawn or floored undrawn balance | FeeSpec; PointInTime default/TWA explicit | [CASHFLOWS-009](09-fees.md) | Observed formula inspected; complete explicit fee-parent package accepted2026-11-09 |
| Fee tier selection | Inclusive greatest threshold; whole-input rate; zero below first | evaluate_fee_tiers | [009](09-fees.md) | Observed ordering evidence; strict domain/nullable selection accepted2026-11-09 |
| CPR/CDR ↔ monthly mortality | Survival-equivalent power via expm1/log; checked and explicit clamped APIs | Shared helper selected by caller | [CASHFLOWS-010](10-prepayment-and-mortality.md) | Observed boundary/roundtrip evidence; strict stable conversion/removal package accepted2026-11-09 |
| Prepayment assumption curves | Constant CPR, PSA ramp, CMBS inclusive lockout, ABS depletion, annual-CPR vector | Explicit PrepaymentModelSpec | [010](10-prepayment-and-mortality.md) | Observed formulas/indexing inspected; complete primary-backed prepayment package accepted2026-11-09 |
| Conditional default curves | Constant/vector CDR and SDA ramp/plateau/decline | Explicit DefaultModelSpec | [CASHFLOWS-011](11-default-and-recovery-assumptions.md) | Observed landmarks inspected; complete explicit default/recovery package accepted2026-11-09 |
| Original-balance default targets | Cumulative loss/severity or annual timing shares; divide increment by surviving balance | CumulativeLoss/Timing; optional actual survival | [011](11-default-and-recovery-assumptions.md) | Observed par-build/validation evidence; direct targets and strict capacity accepted2026-11-09 |
| Recovery-rate assumptions | Constant rate or one-minus-severity vector; tail hold; direct clamp/fallback | RecoveryModelSpec; lag stored for consumer | [011](11-default-and-recovery-assumptions.md) | Observed branches read; full strict recovery package accepted2026-11-09 |
| Schedule canonicalization/validation | Total row order; separate structural/economic validation; principal tolerances | Constructors, serde, explicit validate | [CASHFLOWS-012](12-schedule-lifecycle.md) | Observed lifecycle paths inspected; full immutable/explicit-view package accepted2026-11-09 |
| Outstanding principal replay | Economic-date order; explicit delta overrides inferred kind; skip initial funding once | Schedule balance views | [012](12-schedule-lifecycle.md) | Observed replay evidence; dated exact closing authority accepted2026-11-09 |
| Public remaining-cashflow view | Rebase movements before as_of; preserve pending cash or economic-only rows; include on-date cash | Non-overridable provider normalization | [012](12-schedule-lifecycle.md) | Observed normalization inspected; deletion/separate cash view accepted2026-11-09 |
| Schedule merge/scale | Union rows/calendars; conservative representation precedence; agreed metadata only; scale cash/deltas | Explicit utility calls | [012](12-schedule-lifecycle.md) | Observed composition inspected; restricted merge/scaling removal accepted2026-11-09 |
| Schedule accrued interest | Metadata buckets; actual balance segments; linear or fractional compounded growth | Linear/include-PIK defaults; explicit frequency | [CASHFLOWS-013](13-accrued-interest.md) | Observed branches read; explicit cash-quotation/parent/Money package accepted2026-11-09 |
| Ex-coupon entitlement | Calendar/business-day subtraction; strict date-after-ex rule; rebate every coupon segment | Optional ExCouponRule, max 366 days | [013](13-accrued-interest.md) | Observed boundary evidence; inclusive owner first-ex/full-coupon subtraction accepted2026-11-09 |
| Nominal period aggregation | Half-open windows, sparse buckets, currency separation, Neumaier sums | Explicit periods; no FX | [CASHFLOWS-014](14-aggregation-and-life.md) | Observed boundary/order/currency branches; complete exact Money/PeriodPlan package accepted2026-11-09 |
| Calendar-year cash/PV ladder | Group all rows by cash year; principal/nonprincipal classification; caller PVs | Free arrays or single-currency schedule wrapper | [014](14-aggregation-and-life.md) | Observed array checks; typed native cash/separate owner PV package accepted2026-11-09 |
| Weighted average life | Future positive principal weighted ACT/365F; schedule uses par delta/economic date | WAL helper/schedule method | [014](14-aggregation-and-life.md) | Observed par/date cases; explicit repayment_life/owner-clock package accepted2026-11-09 |
| Survival-adjusted cash PV | Relative discount and conditional survival; on/past-date/noncash zero; post-default cash unweighted | Optional hazard; curve IDs resolved by caller | [CASHFLOWS-015](15-credit-adjusted-pv.md) | Observed row/period formulas; generic credit/PV shortcut removal and qualified owner reporting accepted2026-11-09 |
| Estimated principal recovery PV | FIFO funded lots; default mass over funded interval; recovery at repayment date | Positive optional recovery; hazard required | [015](15-credit-adjusted-pv.md) | Observed interval formula; inferred FIFO recovery removal/product claim ownership accepted2026-11-09 |
| Realized-forward fixing roll | Existing exact fixings win; inclusive crossed endpoints; raw projections; conflict tolerance | materialize_fixings | [CASHFLOWS-016](16-fixing-materialization.md) | Observed normalization/window/merge; explicit required keys/exact conflicts/persistent hypothetical authority accepted2026-11-09 |

## Evidence limits

No runtime sources, tests, fixtures or selected financial methods were changed. No Rust build, test or clippy task was run for this documentation audit. Local documentation checks verify source coverage, record references and links; they do not establish mathematical accuracy. Evidence distinguishes direct formula checks, state/roundtrip/invariant tests and comparisons to the same production helper. Bibliographic anchors were checked for existence; linked standards were not independently revalidated clause by clause.

## Follow-up tracking

The [cashflow follow-ups](../FOLLOW_UPS.md#cashflows) own adoption questions, implementation discrepancies and validation work. Individual records retain the observed algorithms, evidence and scope limitations.
