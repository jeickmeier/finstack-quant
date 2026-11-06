# CASHFLOWS-014 — Currency-preserving reporting, calendar ladders and weighted life

Decision status: **Draft — observed implementation, adoption pending**. Implementation status: **Observed**, scoped below. Validation status: **Evidence inspected/located, not executed**. Audit date: 2026-11-05. Source revision: `7e82fd03ad98346727769e88dd5ac5d84b56e9fc` ([slice audit](README.md)).

## Choice and scope

Nominal reporting preserves native currency and uses compensated summation without per-flow ISO currency rounding. Periods use half-open boundaries. Weighted average life measures future positive par repayment timing on ACT/365F and uses economic principal dates when called through a canonical schedule. No discounting, survival weighting or FX conversion belongs to nominal aggregation or WAL.

## Period aggregation algorithm

1. Validate each period has start before end, periods are sorted and nonoverlapping, and PeriodIds are unique. Gaps are permitted. Validation runs even with no input flows.
2. Sort unsorted input flows by date (stable same-date order) or borrow an already sorted slice.
3. Scan once through periods and flows. Skip dates before a period's start, accumulate while `date < period.end`, and advance the shared flow cursor. A flow exactly at end is available to the next period; flows in gaps or outside all periods are omitted.
4. Accumulate each currency independently using a Neumaier f64 accumulator. Construct Money only from the final total. Empty periods are absent; a period containing flows whose amounts net to zero remains present. IndexMap preserves period/currency encounter order.

`aggregate_cashflows_checked` instead returns one total in a requested currency and errors on any differing input currency; dates do not affect its sum. `PeriodAggregation::try_from` and deserialization require each currency key to match its Money value. Lookup by period string compares exact rendered PeriodId strings. Money conversion rejects nonfinite/unrepresentable totals.

## Calendar-year ladder

Require equal lengths for date/kind/amount/PV input arrays; parse trimmed CFKind labels; require finite amounts and PVs. Group by cash date's calendar year in ascending order, with separate compensated totals for principal-like amounts, all other amounts and supplied PVs. The low-level array API has no currency field and does not filter historical or noncash rows. The schedule wrapper requires all native amounts to have one currency and one PV per canonical row; it passes all rows through unchanged. Callers must select an economically suitable row set and supply their own PV calculation.

## Weighted average life

For future strictly positive repayments, `WAL=Σ amount_i × ACT/365F(as_of,date_i) / Σ amount_i`; return zero when no positive future principal remains. Ignore dates on/before as_of and nonpositive amounts before checking currency, and require all included positive amounts have one currency. Neumaier sums are used in numerator and denominator.

`CashFlowSchedule::wal` selects `is_principal_repayment` kinds. It uses `−principal_delta` when present, otherwise cash amount, and dates each item at `get_balance_date()`. Thus discounted/premium principal cash does not change par weighting and settlement lag does not postpone economic life. The free kernel uses exactly the dates and Money supplied by its caller. It does not infer principal classification.

## Rationale, alternatives and limitations

Source comments give currency preservation, deterministic sorted accumulation and numerical cancellation protection as rationale. ACT/365F aligns with the cited repository WAL convention. Discounted duration, survival-weighted life, payment-date life and original-notional denominators are different metrics and are not selected here. The raw calendar-year API can combine heterogeneous amounts because it cannot inspect currency; this is a caller responsibility.

## Evidence

- [aggregation.rs](../../../../finstack-quant/cashflows/src/aggregation.rs), [schedule WAL/ladder wrappers](../../../../finstack-quant/cashflows/src/builder/schedule.rs).
- Located boundary/currency/validation tests: `boundary_flow_buckets_into_next_period_half_open`, `aggregate_by_period_preserves_currency_separation`, duplicate-period rejection, currency-key serde checks and ladder kind/length checks. Schedule tests locate `wal_uses_act365f_regardless_of_schedule_day_count`, `wal_uses_par_principal_delta_and_balance_date`, and revolving repayment inclusion. These are deterministic formula/invariant evidence.
- [SIFMA MBS Standard Formulas](../../../REFERENCES.md#sifma-mbs-standard-formulas) explicitly points to actual/365 WAL; [Fabozzi](../../../REFERENCES.md#fabozzi-fixed-income-handbook) is a practitioner pointer. No independent dataset accuracy comparison was executed.
- No algorithm inspection is outstanding for this source scope.

## Follow-up tracking

Adoption questions, implementation reconciliation and missing validation are tracked in the
[cashflow follow-ups](../FOLLOW_UPS.md#cashflows).
