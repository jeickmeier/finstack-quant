# PORTFOLIO-EXECUTION-009 — Rich cashflow extraction, partial results and date/currency netting

- **Decision status:** Draft — observed implementation; adoption of the specific methodology remains pending. Existing accepted architectural and arithmetic requirements continue to apply.
- **Implementation status:** Observed for the source paths identified below; discrepancies and limits are stated explicitly.
- **Validation status:** Source and selected test evidence inspected or located; tests, builds and numerical benchmarks were not executed.
- **Audit date:** 2026-11-05. **Source revision:** `7e82fd03ad98346727769e88dd5ac5d84b56e9fc`; exact source hashes are in [coverage.json](coverage.json).

## Choice and scope

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

Implementation, adoption and validation work is tracked in the
[scope follow-ups](../FOLLOW_UPS.md#portfolio-execution). This record does not
authorize a runtime change or certify the observed method.
