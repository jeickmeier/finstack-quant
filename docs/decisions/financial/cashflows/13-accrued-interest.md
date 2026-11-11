# CASHFLOWS-013 — Cash-coupon quotation allocation and ex-coupon boundaries

- Decision status: **Accepted — complete cash-coupon quotation and entitlement-boundary package, 2026-11-09**.
- Implementation status: **Observed; reconciliation pending**, scoped to `accrual.rs`, original-parent handoffs and actual consumer/host paths.
- Validation status: **Production evidence inspected/located, not executed**. Independent bounded exact/date examples and read-only source/primary/consumer reviews are reported below; they do not qualify complete bond, loan, accounting or exercise methods.
- Audit date: **2026-11-05**. Source revision: `7e82fd03ad98346727769e88dd5ac5d84b56e9fc` ([slice audit](README.md)).

## Choice and input domain

The observed implementation reconstructs accrued interest from schedule coupon amounts and accrual metadata, not instrument rate specifications. Its default configuration is Linear, includes PIK, has no ex-coupon rule and no supplied frequency. Interest-like kinds and optional PIK participate; fees do not. ACT/ACT ICMA may require caller frequency. Explicit issue metadata is required whenever coupons exist, even when flow accrual metadata supplies dates. The accepted replacement below selects a narrower, explicit cash-quotation operator using original parent authority.

## Accepted decision: original-parent cash quotation and inclusive first-ex dates

Accepted on **2026-11-09** after complete whole-record review. Runtime/caller/host
reconciliation and independent complete-path qualification remain pending. Inherit accepted
[001's immutable complete period/context](01-schedule-periods.md#accepted-decision-contractual-periods-calendars-and-day-count-context),
[005's original-parent cash amount authority](05-coupon-cash-and-pik.md#accepted-decision-parent-coupon-accrual-exact-splits-and-capitalization),
[007/008's partial-information boundary](08-floating-replay.md#accepted-decision-parent-bound-replay-exact-full-settlement-and-explicit-timing-ownership),
[012's complete source schedule and separate cash view](12-schedule-lifecycle.md#accepted-decision-validated-complete-schedules-explicit-balances-and-separate-cash-views)
and [core012's monetary restoration](../core-conventions/012-money-decimal-precision.md#accepted-decision-monetary-precision-rounding-and-currency-identity).
The formulas below allocate a supplied full cash coupon for quotation. They do
not measure actual earned interest, accounting recognition or a termination
claim. No runtime changes are authorized here.

1. **Choose linear cash-coupon quotation as the single generic operator.**
   Allocate each qualified original cash coupon C using its contractual accrual
   measure, then subtract the full cash claim when it is no longer carried.
   This is an explicit quotation convention. With changing stock, signed native
   contributions or overnight adjustments, it can differ from actual005/007
   earned-prefix calculations, including when the full coupon is zero. Native
   earning, exercise and accounting owners retain those calculations or report
   unsupported use; this utility is not a fallback for missing owner methods.

2. **Use the actual whole cash parent, not flattened-row guesses.** Prepare
   inputs from the SAME canonical001/005 parent owners and their exact stored
   full cash component C, retaining original identity, S/E/P, complete context
   and source/amount authority. Pair each owner with its explicit optional
   first_ex_date; do not copy parallel authoritative period/rate/source fields.
   Require an enabled cash component, including a stored zero; a pure PIK parent
   does not become a cash input by inventing C=0.
   A flat CashFlow row, optional single-slice metadata or a same-payment bucket
   cannot reconstruct a missing composite parent. Such inputs are unsupported
   until their actual owner supplies its authority. Reject repeated references
   to the same parent cash component while retaining distinct parents even if
   amounts, dates and conventions coincide.

3. **Require an explicit currency and return canonical Rust Money.** Currency
   ASSERTS the currency of every supplied parent cash component; it does not
   select or silently drop foreign parents. Owners explicitly prepare the desired
   native-currency parent set. Reject mismatches, including inactive/zero parents;
   perform no FX conversion or inference from principal opening/first row/USD.
   A genuinely empty admitted set returns Money0 in the requested currency.
   Optional012 scalar principal context is not required for this quotation law.

4. **Remove redundant methods and configuration defaults.** Delete
   AccrualMethod, the generic Compounded profile and its1e-12 linear fallback,
   AccrualConfig and global frequency. Retain only the selected linear operator;
   no method-selection alias/default or new fractional-growth model. A nonlinear
   instrument's actual earned cash curve belongs to its own model. The former
   negative-remaining-growth formula is not interchangeable with accrued
   allocation minus the full forfeited cash claim.

5. **Exclude PIK and other noncash/other-interest measures.** Remove
   include_pik. The selected amount is005's CASH component; PIK amount0 and its
   economic principal delta are neither coupon settlement cash nor an implicit
   accrued cash claim. Do not substitute pik_delta into C or derive earned PIK
   by comparing two quote indexes. Actual PIK earning/capitalization, default
   accrual, margin interest, fees, principal and termination amounts retain
   their owners. This removal does not forgive a legitimately owned PIK claim.

6. **State the supported payment profile explicitly.** Require a genuine
   parent S<E<=P. This selected quotation subset supports payment at or after
   accrual end. Reject P<E at admission instead of truncating unearned interest
   at an early cash date.001/005/008 still support their separately selected
   early-payment contracts; those products need their own applicable quotation/
   entitlement method. Do not impose this restriction on their source schedules.

7. **Use original-parent cumulative accrual, with one admission per endpoint.**
   Define A(t)=001 period_accrual(original_parent,S,t) with frozen full context;
   use005's finite nonnegative admitted Decimal prefixes, A(S)=0 and A_E=A(E).
   Cache that same endpoint authority and require monotone bounded prefixes.
   No first-positive factor, stored-factor rescaling, direct segment re-anchoring,
   elapsed clamp/epsilon repair, inferred365/366 denominator, issue/previous-payment
   start, global frequency or representative-day-count fallback. Keep complete
   ICMA/ACT365L/termination context, including composite and genuine zero cases.

8. **Keep zero and nonadditive measures explicit.** For A_E>0, the quotation
   ratio is the transient exact A(t)/A_E, not a separately restored Decimal
   factor. OneOne's first-positive-prefix jump follows the selected measure;
   it is not daily earning. A_E=0 with known C=0 yields a valid zero contribution
   after complete context/date validation. A_E=0 with C!=0 is unsupported, not
   skipped or assigned a fabricated denominator. A zero full cash coupon
   interpolates to zero under THIS law even if native intermediate earning is
   nonzero; it must not replace that different owner calculation.

9. **Take an inclusive first-ex date from the entitlement owner.** Replace
   ExCouponRule, ex_date/is_ex_coupon and days/calendar offsets by a per-parent
   optional first_ex_date X. Some requires X<=P and means the first settlement
   date WITHOUT this coupon; None deliberately selects full cum treatment until
   quote reset, never unknown entitlement. Owner rules determine X from actual
   record/ex-date clauses, valid settlement calendars and information. No366-day
   cap, global calendar inference or requirement X>S. X=P gives an empty ex
   quotation window; X at the minimum Date is directly representable. Unknown
   entitlement data errors at the owner instead of becoming None.

10. **Define all query boundaries independently of earning.** After point8's
    validated zero-measure case, for settlement
    t<P set f(t)=0 when t<=S, A(t)/A_E when S<t<E, and1 when t>=E. The unrounded
    contribution is C*(f(t)-I[X exists and X<=t]); set it to0 when t>=P. Thus
    first-ex equality is ex, a detached coupon before S contributes -C, and
    E<=t<P carries full ordinary C or zero ex adjustment. Quote reset at P is
    this operator's convention, not proof cash was received or permission for
    actual same-day settlement. Do not reuse012's cash cutoff as entitlement.

11. **Preserve full-coupon identity and signed results.** Compare cum versus
    detached entitlement at the SAME parent/date with X<=t<P: their unrounded
    adjustments differ by the exact full C, including future internal balance/
    context pieces. Before X or at/after P the two configurations agree. Do not subtract
    only an active segment or negative growth on a remaining stub. Positive C
    gives a nonpositive ex contribution; negative C can give a positive one.
    No absolute value, sign floor or claim that all ex quotation is negative.

12. **Restore Money once from the complete requested quote.** Evaluate the
    exact transient same-currency sum of all selected parent expressions,
    including their full-coupon subtractions, then core012 Q once. For an
    interior date use C*(A_t-I_ex*A_E)/A_E directly, without separately restoring
    f, ordinary accrued Money, rebate or per-parent contributions. Preserve
    signed cancellation; internal terms need not individually fit Money. Final
    overflow or nonzero-to-zero underflow errors; exact zero is valid. No binary64
    monetary sum, implicit minor-unit rounding or market settlement-penny rule.

13. **Keep known and projected cash qualifications intact.** Full C must be
    lawfully available as an actual qualified amount or owner-authorized forecast
    at its original information snapshot. The index retains shared immutable
    references to those same parent/source owners; a Money result alone asserts
    no known-contractual status. Persisted reports retain their input/query
    reference and forecast qualification. A projected full coupon supports only
    that labelled quotation estimate. Known overnight prefixes do not determine
    unknown final C/q; neither a settlement date nor a stored future path grants
    knowledge or authorizes actual008 partial payoff, earning or claim settlement.

14. **Share one-shot and indexed calculations.** Rename the canonical function
    accrued_interest_amount to accrued_interest and index query accrued_at to
    get_accrued, returning Money directly. AccrualIndex::build receives the
    prepared canonical parent/entitlement pairs and explicit currency, freezes
    them and uses exactly the same kernel as the one-shot function. Remove the
    old schedule/config reconstruction signature without aliases. Rebuilding
    after a genuine source/configuration change creates a new admitted index;
    querying never mutates observations, parent balances or capitalization.

15. **Validate the complete supplied input before shortcuts.** Guard every
    parent association, required full context, currency, C, S/E/P and X, even for
    zero/empty/outside-window requests. Validate queried prefix dependencies
    when required; do not convert missing, stale, mismatched or malformed data
    into zero. A snapshot used for reporting must remain coherent under its
    actual owner's information contract; do not repair conflicting provenance
    by date sorting or infer parent identity from matching metadata. Local build/
    query failure publishes no partial result; external provider effects are
    outside rollback. No public index serialization/checkpoint framework is added.

16. **Keep quotation adjustment separate from actual cash entitlement.**
    The literal X boundary applies only to this selected coupon cash. Actual
    bond/pricer owners must consistently select the buyer's coupon claim across
    static and floating paths and retain their source clause/settlement eligibility.
    This utility does not filter012 native settlement rows, forfeit redemption
    principal, implement special cum trades, due bills, legal transfer or final
    ex-period restrictions. Clean-plus-adjustment quote algebra does not prove
    an undiscounted fair-value jump at X or a complete instrument price.

17. **Reconcile product and accounting consumers explicitly.** Update bond
    quote/yield/settlement, return-floor/tree/LSMC and term-loan call paths for
    actual parent inputs, cash-only Money and product-owned nonlinear/PIK claims.
    Remove principal+total-index-minus-cash-index constructions as a generic
    earned-PIK definition. Statements currently use default generic AI as an
    accounting accrual; its owner must choose its actual earning/recognition
    policy rather than silently adopt this quotation law. Preserve supported
    product generation while unsupported borrowing/early-payment/exercise
    profiles error. No broader loan, statement or option policy is adopted here.

18. **Reconcile existing hosts and exact state paths together.** Update Rust
    re-exports, actual input adapters, JSON/schema, Python/WASM, stubs/facade/parity,
    examples and consumer exports for removed config/method/ex-rule/defaults and
    new canonical Money/currency/parent/query contracts. Bindings perform conversion
    and centralized error mapping; they must not wrap an f64 result in Money or
    infer currency from012's removed representative notional. Reuse canonical
    parent data/plain parameter values and existing index class, with validating
    host factory twins, without a parallel coupon model or new provenance class.

19. **Limit primary attribution to verified product clauses.** DMO's2024 gilt
    formula supports the cited linear cum/rebate profile for its specified cases,
    with separate stub/indexation rules and product rounding. The frozen LSE2024
    rulebook distinguishes equality for gilts and other fixed-interest securities.
    Their labels do not establish one universal ex-date recipe. Owner mapping to
    inclusive X is explicit; a last-cum date D<P maps to checked D+1 if that is
    the actual selected clause. No current-rule compliance, ICMA251 wording or
    generic compounded/PIK convention is certified by an unread bibliography.

20. **Qualify the selected model and its limits independently.** Cover original
    composite contexts and zero/nonadditive measures, signed/zero coupons and
    exact aggregation/restoration, coincident-but-distinct and duplicate parent
    identities, currency/empty input, before-S/at-X/at-E/at-P boundaries, X at
    minimum Date, lagged and unsupported early payments, known/projected final
    coupons and one-shot/index/host agreement. Retain actual source/version/query
    provenance. Bounded references support this quote operator, not native
    earning, accounting, legal entitlement, product calibration or full pricing.

## Independent bounded reference checks

On2026-11-09 independent inline Python Fraction/Decimal/date calculations, without
importing production functions, checked:

- Full cash C10 and fraction0.4 give cum4 and ex-6; C-10 gives cum-4 and ex+6.
  Full entitlement subtraction is exactly C before the final monetary restoration.
- With S10January, first_ex_date5January, query7January and C100, f=0 but the
  selected cash is already detached: quote-100. Query4January is0, equality
  5January is ex, and X at the minimum Date needs no predecessor.
- At E<P the ordinary lagged quote is C and the ex quote0; at P both reset0.
  P<E is an unsupported input under this profile, without changing accepted008.
- Changing stock100→200 over equal half-period weights at annual10% gives
  full native coupon15. At halfway native earned amount5 differs from full-
  coupon quotation7.5. Likewise signed native contributions+10/-10 can leave
  final C0 while interim earning is nonzero. These are distinct calculations.
- The removed compounded curve with N100,C21,f0.5 has ordinary growth10;
  growth minus full coupon is-11, while the former negative remaining-growth
  branch gives-10. For C-19, ordinary growth-10 implies ex+9, not the old+10.
- Let u=1e-28. C=u,f0.1 in ex state has complete quote-0.9u, restored to-u,
  even though separately restoring the ordinary0.1u would underflow. Two cum
  contributions0.4u+0.4u restore once to u. Required final overflow/zero-loss
  checks remain separate from mathematical cancellation and input validity.

These are bounded arithmetic/contract examples, not executed production index,
host, market-settlement or accounting tests.

## Observed period and amount preparation

1. Validate schedule and coupon currency against initial notional. Group same-payment rows only when accrual start/end, day count, calendar, nominal reference coupon and termination flag match. Sum cash and PIK separately; use the first positive supplied accrual factor as the bucket's factor.
2. Use explicit accrual start/end where available; otherwise infer first start from issue and later starts from the preceding bucket's end, with payment date as end. Ignore nonpositive-length periods. This fallback is an assumption for ad hoc schedules, not equivalent to full contractual metadata.
3. Reconstruct outstanding at each segment start. For related segments sharing payment/day-count/calendar/reference-period identity, remember the earliest coupon start so ex-coupon entitlement covers all segments, including segments not yet begun.
4. Coupon total `C` includes PIK according to configuration. Skip zero totals. Full fraction `A` is stored positive accrual factor or calculated day count; skip nonpositive `A`. Store opening notional `N` for compounded accrual.

## Observed active window, day count and ex-coupon logic

A coupon is active while `coupon_start <= as_of < payment_date`. Completed accrual can therefore remain fully accrued during a payment lag. If payment precedes accrual end, the active window ends at payment. For each segment, clip elapsed endpoint into its bounds. For ACT/365L use stored full fraction times actual elapsed/full days; otherwise rescale `elapsed = stored_A × daycount_elapsed/daycount_full` when full day count is finite positive. Clamp elapsed into `[0,A]`.

Ex-date is payment minus `days_before_coupon`: business days when a calendar ID is supplied, calendar days otherwise; maximum 366 days. Require ex-date strictly after the whole coupon's start. Settlement exactly on ex-date retains entitlement: ex-coupon applies only for `ex_date < as_of < payment_date`. In that interval use `e = elapsed−A ≤ 0`, including future balance segments with zero elapsed. Before ex-coupon, future segments are skipped. This subtracts the entire selected coupon entitlement over the relevant payment, not only the segment active on settlement.

## Observed accrual formulas

- Linear: `AI=C×e/A`.
- Compounded: require `N>0` (otherwise return zero), set `q=C/N`, require finite `q>-1`. If `|q|<1e-12`, use linear. Otherwise with `f=e/A`, ordinary accrual is `N×expm1(f×ln1p(q))`; ex-coupon accrual for `f<0` is `−N×expm1(−f×ln1p(q))`.

The compounded ex-coupon branch represents negative growth over the remaining fraction. It is not computed as elapsed compound growth minus the full coupon. Aggregate active segments with Neumaier compensation and reject nonfinite coupon totals/formula results/final totals. Coupon-rate fields are not used to recompute `C`.

## Alternatives and limitations

Observed Linear and Compounded are distinct choices; the latter is a fractional growth assumption over an already-known coupon total, not daily overnight replay. Accepted [CASHFLOWS-008](08-floating-replay.md#accepted-decision-parent-bound-replay-exact-full-settlement-and-explicit-timing-ownership) removes its former generic partial monetary method; actual partial payoff remains owner-selected. The observed bucket logic does not compare differing positive factors on otherwise identical rows; the first factor wins. The accepted package removes these inference/default/compounding paths rather than certifying them as universal accrued-interest conventions.

## Evidence

- [accrual.rs](../../../../finstack-quant/cashflows/src/accrual.rs), [contractual accrual helpers](../../../../finstack-quant/cashflows/src/builder/periods.rs).
- Located hand-formula tests in accrual source: `ex_coupon_negative_ai_linear_golden`, `record_date_retains_coupon_and_next_settlement_rebates_interest`, `compounded_accrual_positive_golden`, `compounded_accrual_ex_window_negative_golden`, plus invalid logarithm domain, future-leap denominator, ICMA frequency and coincident-payment identity checks. [compiler_regressions.rs](../../../../finstack-quant/cashflows/tests/cashflows/builder/compiler_regressions.rs) covers forfeiture across principal segments.
- [UK DMO formulae, fourth edition18December2024](https://www.dmo.gov.uk/media/334d05fo/yldeqns_v4.pdf), sectionTHREE(1)(i), printed17, gives specified standard-period cum/rebate formulas; following pages treat stubs/indexation separately. [DMO About Gilts](https://www.dmo.gov.uk/responsibilities/gilt-market/about-gilts/) identifies its last-cum equality, payment-date reset and final-period restrictions. These support those product clauses, not all original-parent ratios or this library's generic Money profile.
- [LSE rules effective5February2024](https://docs.londonstockexchange.com/sites/default/files/documents/rules-of-the-london-stock-exchange-effective-5-february-2024.pdf), printed52, rules3110.1–3110.3, distinguishes gilt/non-gilt equality. This is a verified frozen rule version, not a claim about current compliance. The [primary reference entries](../../../REFERENCES.md#uk-dmo-gilt-accrued-interest) record the precise scope. [ICMA](../../../REFERENCES.md#icma-rule-book) and [ISDA2006](../../../REFERENCES.md#isda-2006-definitions) remain general pointers, not page-level certification of generic Compounded or PIK defaults.
- Actual [bond quote conversion](../../../../finstack-quant/valuations/fixed-income/src/bond/pricing/quote_conversions/yield_price.rs), [term-loan exercise claims](../../../../finstack-quant/valuations/fixed-income/src/term_loan/pricing/call.rs) and [statements debt accrual](../../../../finstack-quant/statements/src/capital_structure/period_flows.rs) consume the current index/default configuration. [Python](../../../../finstack-quant-py/src/bindings/cashflows/accrual.rs) and [WASM](../../../../finstack-quant-wasm/src/api/cashflows/accrual.rs) currently infer/wrap currency around float calculations. These are static handoff findings, not executed reproductions.
- No production tests or independent complete-market accrued-interest benchmark executed; the bounded references above are reported separately.

## Follow-up tracking

Adoption questions, implementation reconciliation and missing validation are tracked in the
[cashflow follow-ups](../FOLLOW_UPS.md#cashflows).
