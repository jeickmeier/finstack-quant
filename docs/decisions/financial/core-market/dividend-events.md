# CORE-MARKET-13: Dividend event representation and admissibility

- Decision status: **Draft — observed implementation, adoption pending.**
- Implementation status: **Observed** in the owning paths identified below; discrepancies are stated explicitly.
- Validation status: **Evidence inspected or located, not executed.** Formula checks, invariants, and round trips are distinguished below; no production accuracy certification is implied.
- Audit date: **2026-11-05**. Source revision: `7e82fd03ad98346727769e88dd5ac5d84b56e9fc`.
- Scope: the implementation listed here; the [slice inventory](README.md) and [coverage ledger](coverage.json) identify exact file coverage.

## Choice and rationale

The core dividend schedule stores chronological ex-dividend dates and cash, yield or stock-ratio event variants. Yield is documented as an annualized continuous fraction; stock ratio is additional shares per share (0.05 means 5%). It performs admissibility checks but does not convert these alternatives into equivalent dividends, discount their values, adjust ex dates, or infer an equity carry model. Those calculations belong to pricing callers.

## Algorithm, domain, and controls

The builder accepts an optional underlying label and optional currency tag; the public event-adder here constructs cash events. It sorts events by date, then validates each kind. Cash Money amounts must be nonnegative. Yield and stock ratios must be finite and nonnegative; neither has an upper bound of one. An empty schedule is accepted. Multiple same-date events remain distinct; there is no aggregation or duplicate-date rejection. Deserialization goes through the same builder validation and can populate all enum variants.

Cash amounts retain their own Money currency. The optional schedule currency is metadata and is not checked against each cash event currency. The date field is explicitly an ex-dividend date; there are no separate payment/record-date fields in this type. A pricing caller must decide how cash, yield and stock-ratio events interact and handle any payment timing outside this representation. Finite cash representation relies on Money's own construction/deserialization rather than a second float check here.

## Failure, alternatives

Negative events error; nonfinite yield/stock ratios error. No fallback, clipping or forecast filling is performed. Native-date stable sorting does not establish an economic ordering rule for multiple kinds on the same date. Currency enforcement, duplicate aggregation, and distinct ex/payment dates would be additional contract choices, not behavior inferred by this audit.

## Evidence

Owning code and inline construction test: [dividends.rs](../../../../finstack-quant/core/src/market_data/dividends.rs). The inline test checks construction and event count; no independent dividend PV or equity forward fixture is claimed. Context insertion/snapshot methods preserve the schedule unchanged. [Hull](../../../REFERENCES.md#hull-options-futures) provides dividend-adjusted pricing context but does not justify missing schedule-level currency/date constraints. This record covers the financially material data policy rather than inventing an unimplemented dividend pricing kernel.

## Follow-up tracking

Pending decisions, implementation reconciliation and validation work are tracked in the
[core-market follow-ups](../FOLLOW_UPS.md#core-market-data).
