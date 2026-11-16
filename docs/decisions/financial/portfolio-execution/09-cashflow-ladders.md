# PORTFOLIO-EXECUTION-009 — Rich cashflow extraction, partial results and date/currency netting

- **Decision status:** Accepted 2026-11-15 — complete settlement selection, signed sizing, partial reporting, Money netting, prospective FX and report-state package.
- **Implementation status:** Observed. Reconciliation to the accepted package is pending.
- **Validation status:** Independent cutoff, signed sizing, ordering, partial coverage and Money arithmetic examples verified. Production extraction, restoration and host qualification remain pending.
- **Audit date:** 2026-11-05. **Source revision:** `7e82fd03ad98346727769e88dd5ac5d84b56e9fc`; exact source hashes are in [coverage.json](coverage.json).

## Accepted decision for checked portfolio cashflow ladders

1. **Use the owner’s validated settlement schedule.** Request each holding’s
   schedule once at `portfolio.as_of`. Require a separate payment cutoff and
   an explicit choice to include payments on that date. Validate the complete
   source schedule within its declared scope before applying the shared
   settlement selector. Preserve owner-declared `Contractual` or `Projected`
   status even when the selected view is empty. `NoResidual` requires an
   explicitly empty event scope declared by the owner. Unsupported schedules
   and `Placeholder` produce typed holding failures.
   [Schedule lifecycle](../cashflows/12-schedule-lifecycle.md#accepted-decision-validated-complete-schedules-explicit-balances-and-separate-cash-views).

2. **Scale and retain selected settlement rows.** Apply the signed holding
   multiplier once using checked Decimal Money arithmetic. Retain native
   currency, actual payment date, typed kind, duplicates, literal zeros and
   supplied reset, rate and accrual metadata. Include eligible derivative and
   protection settlements. Exclude noncash PIK and write-downs by kind. The
   source schedule owns principal effects. Order the combined rows by payment
   date, holding order and source row order.
   [Core cashflow semantics](../core-conventions/015-cashflow-semantics.md#accepted-decision-cash-settlement-principal-effects-and-accrual-metadata)
   and [signed holding sizing](01-position-sizing-and-validation.md#accepted-decision-for-checked-portfolio-holdings-and-signed-sizing).

3. **Keep strict extraction as the default.** Collect typed outcomes in
   holding order. Strict extraction fails with ordered diagnostics if a
   holding fails. Explicit partial mode omits every row of each failed holding
   and reports holding ID, instrument identity, failure stage and cause.
   Successful holdings with zero eligible rows count toward coverage. A
   report with failures remains partial even with no rows. Shared request
   errors and failures assembling the native buckets abort either mode.

4. **Keep native buckets and nets in Money.** Fold each payment date, currency
   and kind bucket in canonical event order. The requested-currency net folds
   the date’s kind buckets in first-appearance order. Retain actual zero
   totals and omit dates with no bucket in the requested currency. Apply the
   current accepted Money precision rule, including permitted half-even
   rounding below the minor unit. A failed sum aborts the derived net while
   preserving the native report.
   [Money precision](../core-conventions/012-money-decimal-precision.md).

5. **Apply prospective FX to each already-netted native bucket.** Use the
   report’s stored valuation date and the accepted conversion basis. Use cash
   FX for eligible same-day cash and qualified forwards for later cash. Reject
   selected past-dated buckets, including same-currency and zero buckets.
   Convert each admitted bucket once and sum in alphabetic currency order by
   date and kind. Publish nominal base Money with actual FX provenance and
   source coverage. A required FX or sum failure aborts the conversion.
   [Portfolio FX](08-spot-and-forward-fx.md#accepted-decision-for-checked-portfolio-nav-and-cashflow-fx).

6. **Restore one versioned report from ordered holding outcomes.** Store the
   request coordinates and one outcome per holding, with either ordered
   typed Money rows and owner representation or a typed issue. Derive event
   lists, buckets, counts and coverage in Rust. Apply bounded, atomic restore.
   Reject duplicate holding IDs, invalid types or dates, inconsistent outcomes
   and totals that fail recomputation. Retire the bare-ladder JSON path.

7. **Carry coverage through Rust, Python and WASM.** Use the same Rust restore
   and arithmetic at all published entry points. Expose exact amounts in
   canonical outputs and tables. Use a named binary64 display conversion for
   charts. Netting, converted exports and the tear sheet disclose partial
   coverage and failed holdings even when no rows are present. Reconcile the
   schema, bindings and reporting fixtures together.

The ladder reports nominal settlement cash within the owner’s declared scope.
Payment history, principal replay, probability weighting and PV reconciliation
require their own source authority.
[Valuation cashflow export](../valuations-framework/cashflow-export.md).

## Independent calculation checks

- A short holding at `−0.5` scales each duplicate payment of `120` to `−60`
  and retains both rows. An inclusive cutoff retains eligible same-day cash
  zero. The exclusive cutoff drops it, and noncash PIK is excluded by kind.
- Opposite kind buckets of `9,007,199,254,740,992.01 USD` and
  `−9,007,199,254,740,992.00 USD` net to `0.01 USD` in Money. Converting the
  operands to binary64 before summing yields zero.
- At a `1.20 USD/EUR` forward, a native EUR bucket netted from `130 − 30`
  converts to `120 USD`. Adding `7 USD` of the same kind gives `127 USD`.
  A separate kind of `−100 EUR` remains a separate `−120 USD` bucket.
- A successful empty holding and a failed holding produce a partial report
  with no rows. A second-row sizing failure omits the whole failed holding.

## Observed choice and scope at the audit revision

Portfolio cashflows aggregate each instrument's schedule representation and preserve economic event kinds. They are a ledger of returned cashflow events, not a reconstruction of portfolio PV or an unconditional claim of contractual completeness. Instrument providers may return contractual, projected or placeholder representations; each successful position records that representation and its event count.

## Algorithm and selection

1. At `portfolio.as_of`, call each instrument's `cashflow_schedule(market, as_of)`. Scale every returned Money amount with the holding multiplier while retaining payment date, CF kind, reset date, accrual factor and rate.
2. Collect all per-position extraction outcomes in portfolio order through the shared position mapper. A successful empty schedule remains in summaries and `by_position`. A failed provider or scale conversion creates an issue with position/instrument/type and error detail.
3. Default `allow_partial=false` turns any issues into a single error listing the failing positions. With `allow_partial=true`, return successful ladders and explicit issues. Partial means complete omission of a failed position's schedule, not partial cashflows from that position.
4. Stably sort combined events by date; equal-date events retain position/source order. Build `by_date[date][currency][kind]` using checked Money addition. The adapter does not add another past/future cutoff, survival weighting, discount factor or principal normalization.
5. `collapse_to_base_by_date_kind` converts each already-netted currency/kind bucket with [CIP forward FX](08-spot-and-forward-fx.md), then adds currencies in base Money. It retains kind and date, so same-date opposing kinds do not silently disappear from the rich ladder.

`net_in_currency_by_date` ignores FX and Neumaier-sums all kinds for the requested currency at each stored date. It omits dates without a nonempty bucket and preserves the `IndexMap` order. The JSON convenience function accepts either the complete report's `by_date` field or the bare ladder, accepts string kind names because it only nets amounts, validates that every Money currency matches its enclosing currency key, and sorts output by date. It does not reconstruct or check the report's other redundant views.

## Conventions and limitations

Positive/negative cash retains the instrument holder perspective multiplied by the signed position scale. Zero flows and zero net date amounts are not automatically removed. Native amounts remain partitioned by currency until explicit collapse. Rate/accrual fields are descriptive schedule data and are not recalculated by this adapter.

The three report views (`events`, `by_position`, `by_date`) are public/deserializable payloads; reading a report does not prove cross-view consistency. Aggregation produces consistent views from its own event pass, whereas the JSON netting helper trusts only the validated ladder. The forward-conversion result is a cash amount, not discounted base PV.

A fail-fast provider loop is an alternative to the implemented collect-all diagnostics. A PV-reconciled, survival-weighted or expected-cashflow report is a different product requiring explicit representation and reconciliation; it must not be inferred from this ladder.

## Evidence

[cashflows.rs](../../../../finstack-quant/portfolio/src/cashflows.rs), shared [executor](../../../../finstack-quant/portfolio/src/evaluation/executor.rs) and [FX helper](../../../../finstack-quant/portfolio/src/fx.rs). Inline tests locate bond events, provider failures, empty placeholders, MBS/CDS-index providers, CF-kind preservation and CIP collapse; [report serialization tests](../../../../finstack-quant/portfolio/tests/report_serialization.rs) exercise wire round trips. Schedule comparisons use production providers; they do not independently establish product-specific cashflow accuracy. [Compensated-summation context](../../../REFERENCES.md#kahan-1965) applies to the f64 net view, while bucket construction uses checked Money addition.

## Follow-up tracking

Implementation reconciliation and qualification are tracked in the
[scope follow-ups](../FOLLOW_UPS.md#portfolio-execution).
