# CASHFLOWS-014 — Currency-preserving reporting, calendar ladders and weighted life

- Decision status: **Accepted — complete nominal reporting and repayment-life package, 2026-11-09**.
- Implementation status: **Observed; reconciliation pending**, scoped to `aggregation.rs`, schedule wrappers and actual reporting/product/host consumers.
- Validation status: **Production evidence inspected/located, not executed**. Independent bounded exact/date examples and read-only source/primary/consumer reviews are reported below; they do not qualify complete product WAL or valuation methods.
- Audit date: **2026-11-05**. Source revision: `7e82fd03ad98346727769e88dd5ac5d84b56e9fc` ([slice audit](README.md)).

## Choice and scope

The observed implementation preserves native currencies in period reports but converts Money to binary64 before summation. It accepts context-free period slices, exposes a currency-free cash/PV ladder and offers an ACT/365F life helper plus an economic-date schedule wrapper. The accepted replacement below separates exact nominal reporting from qualified valuation outputs and owner-selected product life conventions. No runtime changes are authorized here.

## Accepted decision: exact nominal reporting and explicit repayment life

Accepted on **2026-11-09** after complete whole-record review. Runtime/caller/host
reconciliation and independent complete-path qualification remain pending. Inherit accepted
[core012's exact monetary addition and admission](../core-conventions/012-money-decimal-precision.md#accepted-decision-monetary-precision-rounding-and-currency-identity),
[core014's retained reporting-calendar authority](../core-conventions/014-fiscal-periods-ltm.md#accepted-decision-reporting-periods-fiscal-context-and-ltm-windows),
[core015's exhaustive cash/event semantics](../core-conventions/015-cashflow-semantics.md)
and [012's complete validated source schedules](12-schedule-lifecycle.md#accepted-decision-validated-complete-schedules-explicit-balances-and-separate-cash-views).
Nominal addition, principal cash, principal stock movement and product WAL remain
distinct quantities.

1. **Use exact Money for every nominal total.** Sum admitted Decimal amounts
   under core012's accepted complete exact-sum contract. Allow wide cancelling
   intermediates; require the final sum to be exactly representable as Money or
   error. Pure addition does not use transform rounding Q, binary64/Neumaier
   accumulation or implicit ISO currency rounding. Never round a nonrepresentable
   exact sum into an apparently successful result.

2. **Require the existing validated PeriodPlan.** Replace arbitrary period
   slices by the canonical nonempty, homogeneous, chronological, unique-ID,
   contiguous plan of at most100,000 periods. Retain its single PeriodCalendar,
   including FiscalConfig, canonical boundaries and actual/forecast flags.
   Rendered fiscal IDs alone do not identify a calendar. Remove gapped/custom
   window admission; introduce no alternate window or calendar framework.

3. **Keep half-open reporting selection explicit.** Bucket dates in
   `[start,end)`. A date exactly at a boundary belongs to the next period if
   that period is in the plan; dates outside the plan are omitted. This is the
   declared reporting window, not a missing-data error. Flags do not silently
   filter cash. Validate the entire plan and all supplied dated Money before
   selection, including ignored dates and empty-flow calls.

4. **Preserve contributions and canonical ordering.** Accept unsorted flows;
   stable date sorting may support the scan. Preserve duplicate and literal
   zero contributions. Exact totals are independent of input ordering. Emit
   plan order and canonical alphabetic currency-code order, rather than currency
   encounter order. No currency conversion or cross-currency netting occurs.

5. **Retain the full plan with sparse totals.** PeriodAggregation contains the
   SAME complete PeriodPlan once, with private sparse native-currency totals.
   A period/currency with no input contribution is absent; a contributed zero
   or an exact cancellation remains Money0. Here contributed does not mean
   historically realized. Empty flows return the valid plan and empty totals;
   an empty plan is invalid. Do not fill missing buckets with manufactured zeros.

6. **Guard result construction, restoration and lookup.** Remove Default,
   context-free map-only construction and raw-map Deref. Validate retained plan,
   bucket membership, currency-key/Money agreement and nonempty inner maps;
   reject duplicate wire keys before map collapse, unknown state fields and
   totals restored outside plan/canonical currency order. Do not silently sort
   malformed restored state into validity.
   Use typed PeriodId queries bound to the retained plan:
   `get_period(id)` and `get_amount(id,currency)` return Result/Option, with an
   error for an invalid/out-of-plan ID and None only for an in-plan missing
   total. Expose `get_plan` and immutable ordered rows without a parallel
   unchecked query authority. Parsing malformed text must not become a miss.

7. **Keep one explicitly denominated cash total.** Rename
   aggregate_cashflows_checked to `cash_total(flows,currency)` with no alias.
   Dates have no effect; the required currency asserts every supplied Money,
   including zero and historical rows. Empty input returns Money0 in that
   currency. Use exactly point1's addition and admission, with no inferred first
   currency, date filter or FX fallback.

8. **Make the calendar ladder a typed nominal-cash operation.** Replace the
   parallel date/kind/amount/PV arrays and string classification with admitted
   CashFlow rows. The direct kernel requires core015 cash kinds and rejects
   supplied PIK/write-down or other noncash rows, including zero rows. Validate
   every supplied row before grouping. The schedule convenience method first
   validates its complete012 source, then explicitly prepares the exhaustive
   cash-kind view; noncash events do not create cash years. No new cash-row class
   or classification framework is needed.

9. **Group literal cash by payment year and native currency.** Use the actual
   cash date's Gregorian calendar year, independently of economic dates or a
   fiscal PeriodPlan. Group by year AND currency, ordered by year then canonical
   currency code. Retain signs and duplicate cash rows; allow multiple native
   currencies without a schedule-level scalar currency or principal proof.
   Include all supplied cash dates, past and future. Any cutoff selection is
   explicit at the caller, not inferred from valuation metadata.

10. **Name the two cash columns honestly.** Return exact Money principal_cash
    for cash on core015 principal-capable cash kinds and other_cash for the
    remaining cash kinds. Principal_cash includes funding/draws, premium or
    discounted exchanges and cash with a literal zero principal effect; it is
    neither par movement nor necessarily repayment. Other_cash includes fees,
    recovery and other settlements; it is not universally interest. Never
    substitute `-principal_delta` for either cash column.

11. **Keep ladder rows sparse and exact.** Apply point1 per year/currency/column;
    preserve a row when an actual cash contribution is zero or nets to zero.
    Do not create absent years/currencies or noncash-only rows. The two defined
    columns may contain Money0 for a present row. Empty cash input returns no
    rows after full admission. Guard construction/restoration, unique ordered
    year/currency keys, years inside the supported Date range and matching Money
    currencies exactly as for period totals; host serialization must preserve
    Decimal amounts.

12. **Move qualified PV reporting to its actual valuation owner.** Remove the
    PV argument and column from the nominal ladder. Preserve the existing
    valuation export's required native-PV year report separately, explicitly
    labelled and aggregated as admitted native Money using point1. The
    valuation owner supplies the selected cash-row/PV association, currency,
    valuation date and coherent source snapshot; qualify that association before
    aggregation, including zero rows. Do not substitute nominal cash, zero PV
    or a different row when a valuation is missing or fails. Core016 and
    cashflows015 retain discount/survival/recovery/inclusion and price meaning.
    No generic paired-PV class or new pricing framework is selected here.

13. **Replace unqualified generic WAL by one repayment_life kernel.** Remove
    weighted_average_life_from_principal and CashFlowSchedule::wal, with no
    compatibility aliases or automatic replacement schedule method. Require
    exactly four inputs: owner-prepared dated nonnegative exact Money par
    weights, as_of, currency and include_as_of. The latter three are explicit;
    no opening notional, first currency or same-day default is inferred.

14. **Let owners select repayment weights and date roles.** Use exactly the
    supplied dated weights. The owner asserts genuine repayment par and its
    cash/economic/other qualified timing; the kernel does not infer CFKind,
    absolute cash, missing delta, get_balance_date or a terminal repayment.
    When preparing from classified schedules, only actual repayment-qualified
    negative principal effects become positive par weights. Cash premiums,
    fees, draws, PIK and write-downs do not automatically become repayments.
    No012 scalar opening/coverage proof is required for this stream statistic;
    completeness and projected/observed qualification remain with its owner.

15. **Validate the whole repayment stream before eligibility.** Require every
    Money to use the explicit currency and every weight to be nonnegative,
    including historical and zero rows. Negative weights error; do not take
    absolute values or quietly discard them. Zero weights are valid but add
    neither numerator nor denominator. Preserve duplicate supplied repayments.
    A later invalid item must invalidate the call even if earlier items would
    give an empty or zero result.

16. **Make same-day inclusion and unavailable life different.** With
    include_as_of=true select dates>=as_of; otherwise select dates>as_of. Return
    None if no eligible positive weight remains. Return Some(0) if eligible
    positive weights all occur on as_of. These outcomes cannot be interchanged
    in Rust, hosts, product metrics or Monte Carlo reduction.

17. **Fix and disclose this kernel's ACT/365F clock.** For selected weights P_i
    and exact nonnegative integer actual days d_i, calculate
    `L = sum(P_i*d_i) / (365*sum(P_i))`. Evaluate the complete expression with
    exact transient weights and one checked finite binary64 result conversion,
    targeting round-to-nearest, ties-to-even of the exact rational through the
    already used [num-rational BigRational conversion](../../../REFERENCES.md#binary64-decimal-quantization),
    not separately converted numerator and denominator. Require both Some and
    a finite value; numerator/denominator need not fit Money. Remove per-weight float conversion,
    separately rounded day-count factors, early bounded totals and unchecked
    division. Reject nonzero-to-zero conversion or numeric failure. Qualify that
    conversion target independently; it does not certify product inputs or a
    complete financial metric. The mathematical result is between
    the minimum and maximum selected times. ACT/365F here is a disclosed house
    statistic, not an inferred reporting frequency or universal market WAL.

18. **Limit the denominator and financial interpretation.** The denominator is
    the sum of the selected repayments, not original/current principal. No
    discounting, default probability, FX, current-exposure coverage or assurance
    of complete principal return is implied. An economic-date life and a
    payment-date life may differ. A product owner must explicitly choose its
    no-repayment outcome and whether it reports this statistic as a metric.
    Averaging path lives and life of expected repayments are different Monte
    Carlo estimators; this package does not select between them.

19. **Correct primary WAL attribution rather than codify the old comment.**
    SIFMA-hosted chapterSF G.1(a), printedSF48, defines receipt time on30/360;
    G.1(c), SF49, uses actual receipt of principal for average life. E.2, SF44,
    discusses actual cash delay. H.1, SF56–57, makes principal signs/product
    treatment material: GPM/ARM use signed payments whereas Z-bond treatment
    uses positive payments. These clauses do not establish ACT/365F economic-
    date WAL as a universal convention. Remove that attribution from the
    schedule Rustdoc and qualify product-specific WAL names/clocks/weights;
    do not mechanically migrate those products to point17.

20. **Reconcile actual consumers without losing their authority.** Pass the
    whole PeriodPlan through schema/reporting helpers and update schedule
    ladder wrappers without representative day-count or single-currency
    inference. Preserve the valuation export's CurrencyLadder/native_pv feature
    through point12. Review structured-credit principal_flows' actual cash
    dates and fee adjustment, tagged fallback's abs(cash)/funding admission,
    tranche/bond WAL definitions and conditional-path averaging separately.
    Owners must supply admitted qualified weights/dates or an explicit
    unsupported outcome, never a zero/default fallback.

21. **Update exact host/state contracts together.** Reconcile Rust re-exports,
    schedule/free methods, Python/WASM inputs and result wrappers, strict JSON/
    schema state, declarations, examples and parity ledger in one implementation
    slice. Remove obsolete float arrays, string-kind classifiers, generic WAL
    members and map-only state; bindings perform conversion/error mapping only.
    Required financial inputs have no host defaults. repayment_life remains
    Rust-only; affected existing product/host metrics must propagate its
    explicit nullable outcome under their own admitted policy rather than add
    a direct generic host function. Preserve exact Money, retained calendar
    context and genuine source errors; add no
    compatibility layer, generic registry or alternate numerical authority.

22. **Qualify definitions and limits independently.** Cover exact cancellation,
    representability versus magnitude overflow, fiscal context/plan/key errors,
    half-open boundaries, sparse absent versus contributed zero, duplicate
    rows/keys, native-currency ordering, cash versus stock/noncash classification,
    qualified PV association, same-day/empty life, invalid ignored weights,
    large transient sums and product/MC convention differences. Bounded reference
    examples and read-only reviews below support the selected definitions;
    production/host execution, numerical-domain accuracy and complete product
    qualification remain pending.

## Independent bounded reference checks

Independent Python Fraction/integer/date calculations on2026-11-09, without
importing production code, checked these definitions:

- `10^28-(10^28-1)=1`; converting the two terms to binary64 first loses this
  difference. For Money maximum M=`2^96-1`, `M+M-M=M` despite the wide intermediate.
  `10^28+0.01` is within M's magnitude but is not exactly representable by a
  96-bit Decimal coefficient at any admitted scale; `M+0.1` exceeds the magnitude.
  Both fail exact addition rather than silently rounding.
- Half-open adjacent periods place their shared boundary in the later period.
  Signed cash draw−98 and redemption105 give principal_cash7, while their
  principal effects+100/−100 net0; a PIK event with cash0/delta5 adds no cash row.
- Equal positive weights today and365days later give life0.5 with same-day
  inclusion and1 without. A positive today-only stream gives Some(0) or None
  respectively. Equal maximum-Money weights at365/730days give1.5 despite an
  unrepresentable Money denominator.
  Exact life ratios1+2^-53 and1+3*2^-53 give nearest-even targets1 and
  1+2^-51 respectively; these halfway references do not run the Rust converter.
- Owner-selected dates365 versus730days give1 versus2years for the same weight.
  Treating funding−100 at1year as abs(cash) alongside repayment100 at3years
  incorrectly gives2 instead of the repayment-only3. Equal-probability paths
  paying100 at1year or300 at3years give mean path life2, whereas life of their
  expected repayment stream is2.5. No universal owner choice follows from this.

These are reference calculations and counterexamples, not executions of the
current implementation, property-suite coverage or product-price certification.

## Observed period aggregation algorithm

1. Validate each period has start before end, periods are sorted and nonoverlapping, and PeriodIds are unique. Gaps are permitted. Validation runs even with no input flows.
2. Sort unsorted input flows by date (stable same-date order) or borrow an already sorted slice.
3. Scan once through periods and flows. Skip dates before a period's start, accumulate while `date < period.end`, and advance the shared flow cursor. A flow exactly at end is available to the next period; flows in gaps or outside all periods are omitted.
4. Accumulate each currency independently using a Neumaier f64 accumulator. Construct Money only from the final total. Empty periods are absent; a period containing flows whose amounts net to zero remains present. IndexMap preserves period/currency encounter order.

`aggregate_cashflows_checked` instead returns one total in a requested currency and errors on any differing input currency; dates do not affect its sum. `PeriodAggregation::try_from` and deserialization require each currency key to match its Money value. Lookup by period string compares exact rendered PeriodId strings. Money conversion rejects nonfinite/unrepresentable totals.

## Observed calendar-year ladder

Require equal lengths for date/kind/amount/PV input arrays; parse trimmed CFKind labels; require finite amounts and PVs. Group by cash date's calendar year in ascending order, with separate compensated totals for principal-like amounts, all other amounts and supplied PVs. The low-level array API has no currency field and does not filter historical or noncash rows. The schedule wrapper requires all native amounts to have one currency and one PV per canonical row; it passes all rows through unchanged. Callers must select an economically suitable row set and supply their own PV calculation.

## Observed weighted average life

For future strictly positive repayments, `WAL=Σ amount_i × ACT/365F(as_of,date_i) / Σ amount_i`; return zero when no positive future principal remains. Ignore dates on/before as_of and nonpositive amounts before checking currency, and require all included positive amounts have one currency. Neumaier sums are used in numerator and denominator.

`CashFlowSchedule::wal` selects `is_principal_repayment` kinds. It uses `−principal_delta` when present, otherwise cash amount, and dates each item at `get_balance_date()`. Thus discounted/premium principal cash does not change par weighting and settlement lag does not postpone economic life. The free kernel uses exactly the dates and Money supplied by its caller. It does not infer principal classification.

## Rationale, alternatives and limitations

Source comments give currency preservation, deterministic sorted accumulation and numerical cancellation protection as rationale. The accepted package removes nominal Money's binary64 detour, context-free period maps and currency-free cash/PV arrays. It replaces generic WAL with an explicitly timed stream statistic while leaving actual product WAL conventions with their owners. SIFMA's verified clauses do not support the old universal ACT/365F/economic-date attribution. No automatic repair, inferred principal or financial rounding is selected.

## Evidence

- [aggregation.rs](../../../../finstack-quant/cashflows/src/aggregation.rs), [schedule WAL/ladder wrappers](../../../../finstack-quant/cashflows/src/builder/schedule.rs).
- Located boundary/currency/validation tests: `boundary_flow_buckets_into_next_period_half_open`, `aggregate_by_period_preserves_currency_separation`, duplicate-period rejection, currency-key serde checks and ladder kind/length checks. Schedule tests locate `wal_uses_act365f_regardless_of_schedule_day_count`, `wal_uses_par_principal_delta_and_balance_date`, and revolving repayment inclusion. These are deterministic formula/invariant evidence.
- [SIFMA-hosted chapter SF](https://www.sifma.org/wp-content/uploads/2017/08/chsf.pdf), E.2/G.1(a),(c)/H.1, printedSF44/48–49/56–57, was checked for cash delay,30/360 receipt-time average life and product-specific principal treatment; see the corrected [reference entry](../../../REFERENCES.md#sifma-mbs-standard-formulas). [Fabozzi](../../../REFERENCES.md#fabozzi-fixed-income-handbook) remains a practitioner pointer, not verified clause-level support for the house clock. No independent dataset accuracy comparison was executed.
- Actual consumer evidence: [schema helper](../../../../finstack-quant/cashflows/src/schema.rs), [valuation cash/PV export](../../../../finstack-quant/valuations/core/src/instruments/cashflow_export.rs) and [structured-credit WAL](../../../../finstack-quant/valuations/structured-credit/src/structured_credit/metrics/pricing/wal.rs). Read-only independent source, calendar/Money, consumer and primary-reference reviews challenged the whole proposal; they did not execute production.
- No algorithm inspection is outstanding for this source scope.

## Follow-up tracking

Adoption questions, implementation reconciliation and missing validation are tracked in the
[cashflow follow-ups](../FOLLOW_UPS.md#cashflows).
