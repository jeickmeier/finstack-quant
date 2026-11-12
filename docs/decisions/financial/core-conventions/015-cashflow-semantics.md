# CORE-CONVENTIONS-015: Cash settlement, principal balance events and accrual metadata

- Decision status: **Accepted — complete cash/balance/accrual package, 2026-11-07; accrual-endpoint clarification, 2026-11-08**.
- Implementation status: **Reconciled 2026-11-11 (G5); independent qualification pending.** `CFKind` gains `Floating` and `ProtectionPayment` with literal predicates; amount is signed settlement cash, PIK and default write-downs carry zero cash, and every principal-like row states a paired `principal_delta`/`principal_date` (zero included) that no consumer infers from cash or kind; non-principal rows carry neither. Accrual metadata is one complete `DayCountContextState` slice with literal factors. `CashFlow` and `CashFlowAccrual` have private fields, one validated constructor each (`CashFlow::new(CashFlowParams)`, `CashFlowAccrual::new`), atomic validated updates (`with_accrual`, `with_principal`), and validated deserialization; Python and WASM construct and update through them. Cash-only views (ladders, dated flows, portfolio events, margin conversion) filter to cash-settlement kinds, and exports carry `principal_date`. Not reconciled (owner input, CASHFLOWS-001/013): the accrued-interest engine's period-bound reference fallback and accrual-config frequency, the statements residual's synthesized accrual for rows without metadata, and the LSMC calendar fill (#8); the inflation-linked bond states its indexed redemption cash as the principal delta, and LSMC keeps a premium redemption static.
- Validation status: **Evidence inspected; independent classification/arithmetic examples passed; production qualification pending**. Documentation links/status/counts, complete 27-kind table coverage, signed cash/principal and zero-accrual examples passed. Linked Rust/host tests were located or inspected, not run; this does not certify the current implementation.
- Audit date: **2026-11-05**. Source revision: `7e82fd03ad98346727769e88dd5ac5d84b56e9fc`; see [scope and inventory](README.md).

## Choice, scope and rationale

This record owns core cash-versus-balance representation, kind classification,
principal/date semantics and accrual metadata admission. [Accepted 0044](../conventions/0044-financial-perspectives-and-boundary-conversions.md)
owns signed cash versus roll-forward perspectives; [accepted 0045](../conventions/0045-financial-clocks-and-date-boundaries.md)
owns distinct clocks and date roles. The [accepted day-count packages](001-actual-day-counts.md#accepted-decision-actual-day-conventions),
[coupon context](002-coupon-context-day-counts.md#accepted-decision-coupon-context-day-counts),
[synthetic conventions](003-thirty360.md#accepted-decision-synthetic-day-count-conventions),
[schedule boundaries](008-schedule-generation.md#accepted-decision-schedule-grids-events-and-failure-boundaries)
and [Money contract](012-money-decimal-precision.md#accepted-decision-monetary-precision-rounding-and-currency-identity)
retain their scopes. The complete accepted package below replaces unadopted primitive
alternatives; it does not certify projected schedules or adopt 016's NPV rules.

## Accepted decision: cash settlement, principal effects and accrual metadata

Accepted on **2026-11-07** after whole-record review. Runtime implementation
and independent production/host qualification remain pending;
observed source behavior is retained separately below.

Accrual-slice endpoint wording was clarified on **2026-11-08** with the
[accepted CASHFLOWS-001 package](../cashflows/01-schedule-periods.md#accepted-decision-contractual-periods-calendars-and-day-count-context):
metadata describes actual calculation endpoints when accrual adjustment was
explicitly selected; nominal reference dates remain separate. The selected
adjustment, day-count formulas and single-context limit are unchanged.

1. **Give `amount` one meaning: actual signed settlement cash.** For cash-capable
   kinds, Money is economic cash received by the entity/holder being valued when
   positive and paid when negative, under 0044. “Actual” describes the monetary
   cash role, not historical realization: future contractual/projected and owner-
   qualified conditional/expected cash amounts remain supported. This adds no
   realization flag or projection model. Zero is valid. Do not treat a par
   balance, capitalized interest, defaulted face amount, securities valuation or
   margin-call magnitude as paid cash. `Pik` and `DefaultedNotional` are noncash
   balance-event rows and require exact zero `amount` in their principal currency.
   Their economic value is carried only by explicit principal delta/date. This
   removes duplicate memorandum amounts rather than adding another amount role
   or compatibility fallback. All Money inputs retain 012's exact Decimal paths;
   validation must not detour through `amount()` binary64 views.

2. **Make kind meanings literal and the cash whitelist exhaustive.** Rename
   `FloatReset` to `Floating` (wire `floating`): it is a floating coupon cash
   settlement, never a standalone fixing event. `reset_date` is metadata, not its
   payment date. Add `ProtectionPayment` (`protection_payment`) for actual credit
   protection cash currently mislabeled `DefaultedNotional`; it is distinct from
   recovered asset cash (`Recovery`) and noncash principal write-down. Margin and
   substitution kinds represent actual monetary transfers/interest only, never
   an in-kind asset or its marked value. In-kind events remain with their owner;
   no physical-asset framework is added. Use an explicit match for every known
   variant, so a new kind cannot silently become cash. Retire `float_reset`
   without aliases. Human kind parsing retains ASCII case/outer-trim normalization;
   wire labels are exact canonical snake_case and unknown labels fail.

3. **Keep classification separate from measured economic effects.** Rename
   `is_interest_like` to `is_coupon`, preserving precisely the four ordinary
   coupon kinds below. It does not calculate all interest: PIK, default accrual
   and margin interest need their owner's explicit selection. Keep
   `is_principal_like` as a principal-capable category and rename the kind-level
   `is_principal_repayment` to `is_repayment_like`. These are classifications,
   not amount signs or proofs that a repayment occurred. A repayment calculation
   must select actual contractual repayments with negative explicit principal
   deltas and use `-delta` as the par magnitude, not cash sign, purchase price or
   redemption premium. Notional funding with positive/zero delta is not repayment;
   PIK/write-downs are not repayments merely because their deltas are negative.
   Specific WAL/credit/reporting eligibility remains with its owning algorithm;
   no universal repayment-measure helper is introduced here.

   | Kinds in the accepted table | Coupon | Principal-like | Repayment-like | Cash-capable |
   | --- | --- | --- | --- | --- |
   | Fixed, Floating, InflationCoupon, Stub | Yes | No | No | Yes |
   | Fee, CommitmentFee, UsageFee, FacilityFee, LcFee, FrontingFee | No | No | No | Yes |
   | Notional, Amortization, PrePayment, RevolvingRepayment | No | Yes | Yes | Yes |
   | RevolvingDraw | No | Yes | No | Yes |
   | Pik, DefaultedNotional | No | Yes | No | No |
   | Recovery, AccruedOnDefault, ProtectionPayment | No | No | No | Yes |
   | InitialMarginPost, InitialMarginReturn, VariationMarginReceive, VariationMarginPay, MarginInterest, CollateralSubstitutionIn, CollateralSubstitutionOut | No | No | No | Yes |

4. **Require complete explicit principal effects.** Every principal-like row
   must supply both `principal_delta` and `principal_date`, including an explicit
   zero delta when intentionally recording no additional balance movement. Other
   kinds reject both fields; split a capitalized fee/coupon or recovery plus
   reinstatement into correctly classified rows instead of hiding a balance
   effect on an unrelated cash kind. Delta currency must equal `amount.currency`.
   Delta is signed additive activity in a declared outstanding-principal balance:
   `ending = beginning + sum(delta)`, positive increases and negative decreases.
   Never derive it from amount, kind, cash perspective or a funding heuristic.
   Allow signed corrections and negative PIK components; ordinary net-capitalization,
   write-down and nonnegative-stock constraints belong to producer/schedule
   reconciliation. Do not automatically flip principal delta when cash perspective
   changes; position exposure mapping is separately owned.

5. **Count opening principal once and keep cash/par distinctions.** A producer
   chooses either an opening balance or an inception delta for the same funded
   principal. If opening notional already contains it, the funding cash row carries
   explicit delta zero, not an inferred second increase. A purchase/funding cash
   amount of -98 can coexist with +100 delta when the tracked balance starts at
   zero, or zero delta when 100 is already the opening balance. Redemption cash
   102 and principal delta -100 are likewise different quantities. Recovery cash
   does not by itself restore or reduce principal already written down. The core
   validates row shape; the owner reconciles complete schedules, cash/PIK splits,
   opening balances and event sequencing without epsilon repair or sign guessing.

6. **Keep settlement, balance and index dates distinct.** For cash-capable rows,
   `date` is the actual contractual payment/settlement date. For the two noncash
   kinds it is the event date and must equal their explicit `principal_date`;
   it must not pretend there is cash settlement. Cash principal rows may have a
   different balance-effective date with no universal before/after ordering.
   `get_balance_date` returns the explicit optional principal date, with no payment-
   date fallback. Reset means the one relevant index observation date, never a
   margin call, publication timestamp or inferred accrual boundary. Same-day reset
   is allowed; retain `reset_date <= date` as this primitive's supported single-
   observation contract. Owners represent provisional/future-fixing adjustments
   explicitly rather than passing an unsupported row. Publication eligibility
   remains 0045-owned, not proven by the date inequality. Multi-observation
   coupons need their owner's fixing state, not a fabricated representative reset.

7. **Use one complete existing day-count context in accrual metadata.** Keep
   `CashFlowAccrual` as an optional producer assertion about one actual accrual
   calculation slice: strictly increasing actual endpoints `start < end`,
   including explicitly adjusted accrual dates, selected `day_count`,
   optional projected decimal index rate before gearing/spread/caps/floors, and
   the existing complete `DayCountContextState`. Unadjusted nominal reference
   dates remain distinct from those actual slice endpoints; metadata must match
   the interval used to calculate its fraction. Replace parallel calendar/reference/
   termination fields with that context; do not introduce another partial context
   or frequency default. Follow accepted 001–003: Bus252 requires an explicit
   calendar and positive basis (omission selects its accepted 252); ICMA requires
   explicit eligible frequency and one containing regular reference interval;
   365L retains the full actual coupon bounds; 30E/360 ISDA requires explicit
   termination true/false, not omitted-as-false. Retain canonical calendar identity
   and owner-frozen calendar provenance; malformed/unknown IDs must fail resolution,
   not select weekdays or another calendar. Required-date coverage is checked when
   actually calculating, not claimed merely from storing an identifier.

8. **Do not fabricate accrual provenance for composite cash.** A single metadata
   slice cannot reconstruct a long ICMA stub crossing nominal reference periods,
   compounded coupon, merged rate/balance segments or arbitrary aggregate. Producers
   retain genuine calculation segments in their existing owner model; a composite
   CashFlow omits this optional single-slice metadata unless it truthfully describes
   that entire calculation. Do not attach only the last segment or invent a single
   reference period. Lack of optional metadata is allowed for already known cash,
   but any operation requiring contractual accrual must obtain the actual owner
   inputs or fail. Preserve full-coupon 365L and each ICMA slice's reference through
   subsegments. Payment/reset dates are not substitutes for contractual boundaries.

9. **Keep zero and signed rate metadata literal.** Require finite
   `accrual_factor >= 0`, including legitimate exact zero over a positive calendar
   interval. Never interpret zero as “missing, derive it.” Optional effective and
   projected index rates must be finite when present; finite negative/zero rates
   are allowed without a universal `rate > -1` or positivity rule. None means
   unknown/not supplied and differs from a known zero. Do not impose a universal
   upper accrual-factor bound, require rate metadata for every coupon, or calculate
   paid amount from `notional*rate*factor`: notional, compounding, index ratios,
   gearing, splits and contractual rounding may differ. Money remains authoritative
   for actual cash, explicit delta for balance activity. Producers own consistency
   and any claim that a stored effective rate/factor reproduces their calculation.

10. **Validate complete immutable values at every publication boundary.** Make
    CashFlow and CashFlowAccrual fields private with read-only accessors. One
    canonical `CashFlow::new(params) -> Result` accepts the complete params object;
    builders may hold incomplete raw inputs but publish only after validation.
    Apply the same rules to Rust construction, host construction, JSON/pickle,
    immutable updates and full event exports/restoration. Coupled principal
    delta/date updates are atomic, replacing independent infallible setters.
    Reuse selected day-count context admission rules; reject invalid chronology,
    missing required context, nonfinite scalars, currency/kind/principal mismatches
    and nonzero cash on noncash kinds. No universal accrual-end-versus-payment or
    principal-versus-payment inequality is imposed. Do not recompute or certify
    the monetary formula during primitive validation. Optional omissions remain
    genuinely optional; they never trigger inferred economic amounts/dates.
    Retain `validate` as an idempotent check, not an optional gate needed to make
    a publicly constructed object valid. Invalid updates leave the old object
    unchanged; serde cannot create a second unchecked route or turn NaN into null.

11. **Separate full events from dated-cash views.** Full event interchange must
    retain kind, exact cash amount/currency, explicit principal delta/date and
    optional reset/rate/accrual context without loss. Cash-only APIs filter to
    cash-capable kinds and use settlement date/amount; they do not export noncash
    rows as capitalized/write-down amounts. Never restore an intentionally lossy
    cash report as a complete balance-event schedule. Balance replay uses explicit
    principal deltas at their economic dates, independently of cash settlement.
    Credit protection cash uses ProtectionPayment; asset recovery remains Recovery.
    Collateral asset substitutions and margin-call timestamps stay with their
    actual owners unless there is a genuine monetary settlement. None of this
    selects valuation-date inclusion, curve currency, discount-factor behavior or
    NPV precision; those remaining choices belong to 016.

12. **Keep same-day sequencing and financial calculations owner-selected.**
    CFKind declaration order and serialization sort are not economic instructions
    for applying funding, interest, PIK, default, repayment or recovery on one date.
    Schedule/product owners retain explicit calculation order and balance-before/
    after conventions; a deterministic report tie-break does not redefine them.
    Do not add generic compounding, loss/recovery allocation, cash netting,
    asset settlement or principal-inference machinery to this primitive. A full
    projected schedule still needs its own validation and independent qualification.

13. **Reconcile all actual producers, consumers and hosts in one working slice.**
    Propagate the renamed/new kinds/predicates, exact Money, required principal
    fields, optional balance-date accessor, params construction, validated context
    and state through Rust re-exports/tests, Python/WASM errors/stubs/facade, schemas,
    parity, examples and financial consumers. Repair inferred principal replay,
    PIK-interest consumers reading cash amount, zero-factor fallback, payment-date
    accrual reconstruction, default-protection misclassification, margin substitutions/
    call-date mapping and lost principal-date exports. Audit every amount/delta/date
    consumer, not just constructors, and update all products using the primitives
    together. Remove old labels/setters/partial context and fallback paths without
    aliases or migrations. Financial calculation policies outside this scoped row
    contract remain with their existing records; acceptance does not implement or
    certify them automatically.

## Boundary examples for the accepted package

| Input / situation | Required accepted outcome |
| --- | --- |
| Holder receives/pays cash; same row viewed from opposite side | Positive/negative actual amount; explicit cash-perspective conversion, no automatic principal-delta inversion. |
| Pik amount0 USD, delta+5 USD, event/principal date equal | Valid noncash capitalization; adds5 to outstanding and0 to dated cash. |
| DefaultedNotional amount0 USD, delta-100 USD; separate Recovery cash30 USD | Outstanding falls100 once; cash receipt30 does not reapply the write-down. |
| Noncash kind has amount5 or mismatched event/principal dates | Error, not a memorandum cash amount or silently substituted date. |
| Cash funding -98 with opening0/delta+100 versus opening100/delta0 | Explicitly different balance accounting; never infer delta98 or count opening100 twice. |
| Redemption cash102/delta-100 versus initial funding delta+100/0 | Repayment candidates use contractual par delta, not premium cash or cash sign. |
| Principal-like row missing either principal field; nonprincipal with either field | Error; deliberate principal zero still requires explicit delta/date. |
| Delta currency differs from zero or nonzero cash currency | Currency error without conversion. |
| Negative PIK component or write-down correction | Signed delta allowed; owner validates period net effect and final stock. |
| Capitalized coupon/fee plus paid cash | Separate correctly classified zero-cash principal row and actual cash row; no hidden delta on a fee/coupon kind. |
| Actual CDS protection payout70 | ProtectionPayment cash70, not DefaultedNotional or asset Recovery. |
| Security collateral substituted at marked value100 | No cash100 row; only genuine monetary settlement belongs in this primitive. |
| Margin call date supplied as reset date | Not legitimate index-observation metadata; preserve call timing with margin owner. |
| Same-day index observation / reset after event or payment | Allowed / error under this supported single-observation contract. |
| Accrual interval precedes delayed payment; principal date differs from payment | Preserve each actual date; no payment-date accrual or balance-date inference. |
| Positive accrual interval with factor0 | Valid zero, not a signal to derive or replace it. |
| Negative finite rate; None; known rate0; NaN/infinity | Valid signed rate / unknown / distinct known zero / error. |
| ICMA missing frequency/reference; 365L missing full coupon; ISDA termination omitted | Required-context error; no implicit inference from payment schedule. |
| Long ICMA stub spans two nominal references | Owner keeps actual slices; one whole-flow metadata object cannot pretend a single reference suffices. |
| Composite coupon lacks optional accrual metadata | Known cash may remain valid; an accrual calculation needing missing inputs fails. |
| Immutable update or JSON restore introduces invalid metadata/currency/principal state | Reject atomically; retain original valid object, no null/repair fallback. |
| Cash export versus full event export | Cash view filters noncash; full view preserves explicit delta/date/context exactly. |
| Multiple economic events on one date | Owner-defined calculation order; enum order alone cannot choose accrued balance. |

## Observed algorithm and actual selection

Interest-like means Fixed, FloatReset, InflationCoupon or Stub. Principal-like means Notional, Pik, Amortization, PrePayment, RevolvingDraw, RevolvingRepayment or DefaultedNotional. Principal-repayment means Amortization, Notional, PrePayment or RevolvingRepayment. Cash settlement is every kind except Pik and DefaultedNotional. Recovery and AccruedOnDefault are cash but not principal-like according to these predicates; fee and margin/collateral kinds are also cash.

`get_balance_date` returns explicit principal_date, otherwise payment date. The constructor populates neither accrual metadata nor principal delta/date. Fluent setters merely attach values. `validate()` rejects nonfinite/negative accrual factor, nonfinite optional rate, reset after payment, principal-delta currency mismatch, nonincreasing accrual/reference period and nonfinite projected index rate. Negative finite rates and positive or negative amounts/deltas are allowed. Optional rates also have finite-value checks on serialization.

## Observed conventions and controls

Accrual metadata records unadjusted start/end, day count, projected index rate, optional calendar, optional regular coupon reference and termination flag. Rates are decimal representations; the cashflow amount remains authoritative. `principal_delta` is explicit balance impact, while `principal_date` can differ from delayed settlement. The predicate `is_principal_repayment` is a kind classification, not a sign check; Notional may still have either signed amount.

## Observed limitations and rejected alternatives

Construction/deserialization do not automatically call `validate`. Public fields can express an inconsistent amount/rate/accrual-factor relationship, unsupported calendar ID or principal event inconsistent with its kind. This validator does not calculate accrual, require a calendar for Bus252, verify frequency metadata or ensure the supplied reference period matches the actual schedule. It also does not filter events from NPV itself: [016](016-discounting.md) receives `(Date,Money)` pairs without kind.

The current optional principal-event fields exist, but their fallback/admission behavior is not adopted by the accepted package. Inferring balance changes solely from signed settlement amounts, eagerly validating every construction, or deriving amount from rate/accrual would be different contracts and are not implemented here.

## Evidence

[Principal replay](../../../../finstack-quant/cashflows/src/builder/schedule.rs),
[accrual consumers](../../../../finstack-quant/cashflows/src/accrual.rs),
[floating/PIK emission](../../../../finstack-quant/cashflows/src/builder/emission/coupons.rs),
[CDS-index protection rows](../../../../finstack-quant/valuations/credit/src/cds_index/pricer.rs),
[CDS-tranche protection rows](../../../../finstack-quant/valuations/credit/src/cds_tranche/pricing/engine.rs),
[margin-call conversion](../../../../finstack-quant/margin/src/types/repo_cashflows.rs)
and [full event export](../../../../finstack-quant/valuations/core/src/instruments/cashflow_export.rs)
show material producer/consumer reconciliation points. Current replay infers
principal changes from kind/amount when delta is absent; some actual protection
cash is tagged DefaultedNotional, PIK accrual reads amount, zero factors can trigger
derivation and event exports omit principal_date. These are source observations,
not executed reproductions of complete instrument prices or balance schedules.

[Existing complete day-count state](../../../../finstack-quant/core/src/dates/daycount/context.rs),
[Python primitives](../../../../finstack-quant-py/src/bindings/core/cashflow.rs)
and [WASM primitives](../../../../finstack-quant-wasm/src/api/core/cashflow.rs)
locate context/state/host propagation. Current partial metadata and source
termination defaults must be reconciled with accepted 001–003 rather than retained
as alternate authoritative context.

[AASB 107, paragraphs 43–44A](https://standards.aasb.gov.au/aasb-107-sep-2024)
distinguishes noncash capital changes from cash flows and requires their separate
reporting. It is background for keeping cash and balance effects distinct, not an
external definition of this internal kind table or adoption of an accounting regime.
The exact zero-cash, explicit-delta/date and primitive admission choices are library
decisions accepted here; their implementation and independent qualification remain pending.

[Owning implementation and inline tests](../../../../finstack-quant/core/src/cashflow/primitives.rs): `cfkind_is_interest_like_classifies_coupon_flows_only`, `cfkind_is_principal_like_classifies_balance_changing_flows`, `cashflow_validation_works`, and optional-rate serialization tests. These assert semantic categories and metadata checks; they do not validate a projected financial schedule. [Core cashflow exports](../../../../finstack-quant/core/src/cashflow/mod.rs) add no selection logic. No external standard defines this exact internal classification table.

Settlement date and balance date remain distinct in downstream models.

The 2026-11-07 whole-record review passed documentation links/anchors,
status/count and whitespace checks. Independent exact arithmetic examples checked
opening principal, cash versus par, signed capitalization/corrections, separate
write-down/recovery and legitimate zero accrual. The accepted table covers each
of its 27 kinds exactly once, with explicit coupon/principal/repayment/cash groups.
These checks qualify the stated examples, not production row validation, event
replay, accrual or instrument prices. No runtime source/tests/fixtures were changed
and no production Rust/host test or build pass was run.

## Follow-up tracking

Implementation reconciliation and missing evidence are tracked in the
[core-conventions follow-ups](../FOLLOW_UPS.md#core-conventions).
