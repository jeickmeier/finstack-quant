# Core financial conventions: implementation methodology audit

Audit date: **2026-11-05**. Source revision: `7e82fd03ad98346727769e88dd5ac5d84b56e9fc`.

This slice inspects **57 assigned Rust files**: 42 algorithm-bearing, 8 support, 3 dedicated test and 4 generated files. It includes the build-time calendar generator because its emitted registry is executed through `OUT_DIR/calendars.rs`. The complete one-file/one-entry map is [coverage.json](coverage.json). It also inspects all 28 calendar JSON definitions, monetary/holiday/SIFMA CSV generation dependencies and the embedded scorecard registry where they control the algorithms.

The **20 records** below are **Draft (observed implementation, adoption pending)**. Implementation status is Observed for each stated scope. Validation status is Evidence inspected/located, not executed: financial unit/integration/golden tests were read or located, but no Rust test, lint or build pass was run. A reference-table generator consistency check passed; it confirms shipped source matches maintained inputs, not that market data or financial choices are correct. No runtime code, tests, fixtures or accepted financial policy were changed.

The inventory separates concrete algorithms, variants, defaults, failure behavior and numerical controls. Calendar definitions are expanded rule by rule in [calendar-selections.md](calendar-selections.md), including exact dates and effective-year bounds. No owned algorithm is deferred for later inspection. Product-level selection outside this slice and primary-source provenance not established by inspected evidence remain explicit open decisions.

Shared policy is indexed in [financial conventions](../conventions/README.md);
this catalogue traces the core implementation within that policy’s stated scope.

## Methodology inventory

| Calculation / supported instruments | Implemented method / material variants | Actual selection | Decision record | Implementation / validation |
|---|---|---|---|--- |
| ACT/360 and ACT/365F accrual | Elapsed days /360 or /365 | Explicit enum | [CORE-CONVENTIONS-001](001-actual-day-counts.md) | Observed formulas; daycount/golden tests located |
| OneOne contractual periods | One per nonempty forward interval | Explicit enum | [CORE-CONVENTIONS-001](001-actual-day-counts.md) | Observed dispatch; zero/range behavior inspected |
| NL/365 accrual | Remove Feb29 in (start,end], divide365 | Explicit enum | [CORE-CONVENTIONS-001](001-actual-day-counts.md) | Endpoint tests inspected/located |
| ACT/ACT ISDA | Calendar-year segments, sequential full-year addition | Explicit enum | [CORE-CONVENTIONS-001](001-actual-day-counts.md) | Formula and floating accumulation regression |
| ACT/ACT AFB | Backward whole-year walk, leap residual denominator | Explicit enum | [CORE-CONVENTIONS-001](001-actual-day-counts.md) | Stored-reference and date examples located |
| Bus/252 accrual | Calendar-business days in [start,end), positive basis default252 | Calendar required; optional basis override | [CORE-CONVENTIONS-001](001-actual-day-counts.md) | Context/holiday/count tests located |
| Signed/zero day counts | Zero short circuit; reverse/negate with termination flag cleared | Separate signed API | [CORE-CONVENTIONS-001](001-actual-day-counts.md) | Dispatch/context inspected |
| Regular ICMA coupon accrual | Exact month/year grid hit, <=512 steps | Frequency required without reference period | [CORE-CONVENTIONS-002](002-coupon-context-day-counts.md) | Regular/stub tests located |
| Irregular ICMA/quasi-coupons | Nominal months inferred from reference days; anchored overlap weighting | Explicit coupon_period | [CORE-CONVENTIONS-002](002-coupon-context-day-counts.md) | EOM and reference-grid tests |
| ACT/365L annual/nonannual | Annual coupon leap-day test; otherwise coupon-end leap year | Frequency and enclosing coupon required | [CORE-CONVENTIONS-002](002-coupon-context-day-counts.md) | Partial-accrual denominator tests |
| US SIA 30/360 | February EOM and conditional ending31 adjustment | Thirty360 enum | [CORE-CONVENTIONS-003](003-thirty360.md) | Formula fixtures/edge cases |
| ISDA bond-basis 30/360 | 31 adjustments without February EOM rule | Raw helper only | [CORE-CONVENTIONS-003](003-thirty360.md) | Raw kernel inspected |
| European/Italian 30/360 | Independent31 clamp; Italian also February >27 | Separate enum/helper variants | [CORE-CONVENTIONS-003](003-thirty360.md) | Stored expected fractions |
| 30E/360 ISDA | EOM start; February termination-end exception | Context flag default false | [CORE-CONVENTIONS-003](003-thirty360.md) | Termination boundary tests |
| Fixed/nth/shift/Easter holidays | Five observation modes; inclusive shifts; Gregorian computus | Compiled rule enum | [CORE-CONVENTIONS-004](004-holiday-engine.md) | Rule examples; Meeus reference |
| Effective/span/China bridge holidays | Year-gated neighboring-year expansion and weekday bridge blocks | JSON rules compiled at build | [CORE-CONVENTIONS-004](004-holiday-engine.md) | Rule materialization tests |
| Lunar/equinox dates | Eight generated tables, 1970–2150; missing->None | Tabulated rule selection | [CORE-CONVENTIONS-004](004-holiday-engine.md) | Generator consistency check passed |
| Holiday cache/counting | Year bitsets and prefix sums; scan outside cache window | Automatic built-in optimization | [CORE-CONVENTIONS-004](004-holiday-engine.md) | Production-scan equivalence tests located |
| Calendar generation/lookup | Missing observed=None, ignore_weekends=false; Effective wrappers; exact/casefold IDs | All calendar JSON files compiled | [CORE-CONVENTIONS-004](004-holiday-engine.md) | Entire build generator inspected |
| Hong Kong general holidays | Historical branches, Sunday substitutions and collision extras | hkhk/hkex | [CORE-CONVENTIONS-005](005-jurisdiction-calendars.md) | Published-date examples and jurisdiction code |
| Japan holidays | Era/Happy-Monday/Olympic rules, citizens and Sunday substitutes | jpto/jpx plus bank year-end closures | [CORE-CONVENTIONS-005](005-jurisdiction-calendars.md) | Stored Cabinet Office 1970–2027 fixture |
| 28 maintained calendar selections | Every fixed/date/year/observation selection in calendar appendix | Explicit banking/fixing/trading/accrual ID | [CORE-CONVENTIONS-005](005-jurisdiction-calendars.md) | All JSON definitions inspected; published fixture roles identified |
| Business-day rolling | Following/Preceding/Modified variants/Nearest following tie | Explicit convention; enum default ModifiedFollowing | [CORE-CONVENTIONS-006](006-business-day-adjustments.md) | Concrete tie and monthly-boundary assertions |
| Joint settlement calendars | Intersection of business days, union of closures | a+b IDs; None -> weekends-only helpers | [CORE-CONVENTIONS-006](006-business-day-adjustments.md) | Composite canonicalization and union tests |
| Signed business-day/weekday stepping | Start-excluded step counts; zero unchanged; weekend block arithmetic | Explicit helper | [CORE-CONVENTIONS-006](006-business-day-adjustments.md) | Forward/backward/closed-calendar tests |
| Month/day arithmetic | Clamped months; checked Julian days; complete-month floor | Explicit helper | [CORE-CONVENTIONS-007](007-tenor-date-arithmetic.md) | Date examples and boundaries |
| Tenor approximations and compounding frequency | 365/52/360/252/365.25 mappings; rounded inverse-tenor compounding | Context-free helpers or actual-date alternative | [CORE-CONVENTIONS-007](007-tenor-date-arithmetic.md) | Conversion tests located |
| Regular/stub coupon grids | Original-anchor forward/backward generation; 5 stub policies | Default monthly/no stub; explicit override | [CORE-CONVENTIONS-008](008-schedule-generation.md) | Schedule expected-grid tests |
| EOM/IMM/CDS schedule modes | EOM snaps interior rolls; IMM thirds; CDS previous20 front accrual | Mode setters default quarterly/ShortBack | [CORE-CONVENTIONS-008](008-schedule-generation.md) | IMM and CDS integration assertions |
| Payment dates | Adjust accrual end then count business-day lag | Lag default0, calendar+convention required to resolve | [CORE-CONVENTIONS-008](008-schedule-generation.md) | payment_lag_shifts_adjusted_period_ends inspected |
| Fixing dates | T-minus business days from unadjusted accrual start | None omits; zero retains start | [CORE-CONVENTIONS-008](008-schedule-generation.md) | fixing_lag_is_t_minus_from_accrual_start inspected |
| Schedule failure/repair policy | Strict; missing-calendar warning; graceful empty; logged monotonic drops | Strict default, explicit alternatives | [CORE-CONVENTIONS-008](008-schedule-generation.md) | Failure branches and warning contracts inspected |
| IMM/CDS roll dates | Strict/inclusive quarterly20/thirdWednesday and semiannual cycles | Dedicated API determines inclusivity | [CORE-CONVENTIONS-009](009-contractual-roll-dates.md) | Known-date and boundary examples |
| Option expiry anchors | ThirdWednesday minus5 calendar days; monthly thirdFriday | Unadjusted helpers | [CORE-CONVENTIONS-009](009-contractual-roll-dates.md) | Good-Friday unadjusted assertion |
| Published TBA settlement | Class/month lookup, missing ->None; next ClassA strictly later | Exact-date API | [CORE-CONVENTIONS-010](010-sifma-settlement.md) | Stored 2025–2027 table inspected |
| Estimated TBA settlement/class selection | A/B/C/D use 9/11/14/16th business day; agency substring/term heuristic | Explicit estimator; A default | [CORE-CONVENTIONS-010](010-sifma-settlement.md) | Estimate formulas and fail-closed tests |
| FX quotation/pips | EUR/GBP/AUD/NZD/USD rank then alphabetic; JPY/KRW/HUF pip heuristic | Pair convention helper normalizes input | [CORE-CONVENTIONS-011](011-fx-date-and-quote-conventions.md) | Heuristic assertions |
| FX spot dates | Per-leg days; Latin-American USD intermediates; joint final USD gate | Caller lag/calendars; standard T1/T2 helper | [CORE-CONVENTIONS-011](011-fx-date-and-quote-conventions.md) | Concrete holiday examples |
| FX forward tenor maturity | Spot calendar-EOM preservation then joint-base/quote adjustment | Explicit EOM/BDC/calendars | [CORE-CONVENTIONS-011](011-fx-date-and-quote-conventions.md) | Kernel inspected |
| Monetary input precision | Exact decimal text; checked f64 conversion; optional configured ingest rounding | Unrounded default; configured default >=6dp | [CORE-CONVENTIONS-012](012-money-decimal-precision.md) | Precision/rejection examples |
| Monetary arithmetic/FX at rate | Currency-checked Decimal ops; float scalar ->Decimal; explicit rate conversion | No implicit conversion | [CORE-CONVENTIONS-012](012-money-decimal-precision.md) | Arithmetic and currency tests |
| Monetary rounding/output | Nearest-even, midpoint-away, truncate, floor, ceil | Display/minor-unit defaults; explicit options | [CORE-CONVENTIONS-012](012-money-decimal-precision.md) | Rounding mode tests |
| Currency identity/minor units | 159 static ISO-code/numeric entries and 0/2/3/4dp groups | Generated CSV lookup | [CORE-CONVENTIONS-012](012-money-decimal-precision.md) | Full generator artifact consistency passed |
| FX authoritative resolution | Global override > scoped pinned > observed > provider; direct/inverse by layer | Explicit stores/provider | [CORE-CONVENTIONS-013](013-fx-resolution-and-shocks.md) | Precedence/cache tests located |
| FX cross-rate calculation | Two leg product through one pivot; default USD enabled | Triangulation flag/pivot config | [CORE-CONVENTIONS-013](013-fx-resolution-and-shocks.md) | Cross/invalidation invariants |
| FX accounting date/average policy | Policies are query keys; inverse/product applied to supplied aggregates | CashflowDate default | [CORE-CONVENTIONS-013](013-fx-resolution-and-shocks.md) | PeriodAverage invariant test inspected |
| FX relative risk shock | Direct*(1+b), inverse/(1+b), derived-pair scope preserved | with_bumped_rate; no identity shocks | [CORE-CONVENTIONS-013](013-fx-resolution-and-shocks.md) | Scope/snapshot/overflow tests located |
| FX revision/cache/state | Three stable-state attempts; 256-entry LRU; authority-preserving snapshots | Versioned provider controls cacheability | [CORE-CONVENTIONS-013](013-fx-resolution-and-shocks.md) | Race/revision tests located |
| Reporting periods/fiscal grids | Half-open Gregorian/ISO/fiscal partitions with clamped starts and partial weeks | Explicit kind/config; Jan1 default | [CORE-CONVENTIONS-014](014-fiscal-periods-ltm.md) | Fiscal/ISO examples and partition invariants |
| Annualization/LTM | Daily252/Weekly52 factors; daily12-month rows vs fixed-count other kinds | PeriodKind and explicit target | [CORE-CONVENTIONS-014](014-fiscal-periods-ltm.md) | 365/261-day and leading-gap tests inspected |
| Cashflow economic categories | Interest/principal/repayment/cash predicates; PIK/default-notional noncash | CFKind explicit | [CORE-CONVENTIONS-015](015-cashflow-semantics.md) | Classification assertions |
| Accrual/principal metadata validation | Finite/nonnegative accrual; rate/date/currency checks; balance-date override | Explicit validate(); builders attach metadata | [CORE-CONVENTIONS-015](015-cashflow-semantics.md) | Validation/serialization tests located |
| Flat continuous discount factor | exp(-r*t), finite rate and nonnegative time | Explicit helper | [CORE-CONVENTIONS-016](016-discounting.md) | Closed form/invariance tests |
| Curve cashflow NPV | Future-only sum amount*DF(t)/DF(base), curve daycount/context | Supplied Discounting; Decimal or f64 amounts | [CORE-CONVENTIONS-016](016-discounting.md) | Base-date normalization and Bus252 context tests |
| IRR/XIRR inverse return | One-sign-change contract; log-scaled relative objective; Brent1e-12/100; residual1e-8 | Guess default10%; explicit XIRR daycount | [CORE-CONVENTIONS-017](017-irr.md) | Closed form/cancellation/scale and rejection tests |
| Rate and integer bp units | Decimal-assisted percent/bp scaling; rounded saturating integer accessor; truncating Bps division | Rate vs Bps selected by caller | [CORE-CONVENTIONS-018](018-rate-units.md) | Scaling/domain tests located |
| Credit rating grade/distance | AAA..C,NR,D ordinal; bucket mapping; signed ordinal differences | Generic CreditRating | [CORE-CONVENTIONS-019](019-credit-ratings.md) | Grade and antisymmetric-notch invariants |
| Scorecard rating scales | Descending0–100 scores/thresholds; default score50/S&P/unknown fallback | Embedded or config registry | [CORE-CONVENTIONS-019](019-credit-ratings.md) | Registry validation tests |
| Barrier semantic defaults | UpAndOut and AtHit defaults; direction/activation predicates | Enum/default caller selection | [CORE-CONVENTIONS-020](020-barrier-classification.md) | Type/wire assertions only |

## Evidence boundaries

- Day-count golden fixtures name QuantLib 1.32, but this audit did not regenerate vendor output; a fixture's `certified` label is not an approval statement. Calendar published-year fixtures and Japan's stored Cabinet Office list are distinct from the all-calendar golden digest, which is an implementation regression baseline.
- The 1970–2150 calendar cache/table horizon is not a uniform completeness horizon. Several calendars have narrower published moving-holiday coverage or named estimates; a successful business-day result does not surface that limitation at runtime.
- XIRR creates default day-count context, so context-dependent conventions cannot be used for nonempty periods.
- Numerical representation remains material: Decimal storage does not imply unlimited arithmetic precision; integer Bps conversions can quantize/saturate; IRR checks relative NPV residual rather than a uniform rate-error bound.

## Verification performed

`UV_CACHE_DIR=/tmp/finstack-methodology-uv uv run --no-project python finstack-quant/core/build/generate_reference_data.py --check` completed with exit 0. This is a generated-source/input consistency check only. The audit's static check verifies manifest equality, unique IDs, record existence and local links. Financial runtime tests and external publication refreshes were not executed.

The [core-conventions follow-ups](../FOLLOW_UPS.md#core-conventions) track this slice's specific discrepancies and unresolved choices; individual records contain the owning code, evidence role, alternatives and limitations.
