# MODELS-AND-VALUATIONS-CREDIT-14: Index constituent aggregation and informational cashflow projection

- Decision status: **Draft — observed implementation, adoption pending**.
- Implementation status: Observed in the paths listed below; discrepancies are stated explicitly.
- Validation status: Evidence inspected or located, **not executed**; no external accuracy certification.
- Audit date: 2026-11-05.
- Source revision: `7e82fd03ad98346727769e88dd5ac5d84b56e9fc`.

## Choice and rationale

Index pricing delegates analytic CDS legs to one synthetic curve or active constituents. The public projected schedule deliberately uses a coarser midpoint approximation.

## Scope and input domain

SingleCurve or Constituents mode; constituent weights sum to one within 1e−6 and declared defaults constrain index factor to surviving original weight. No active names contributes zero.

## Algorithm

Synthetic notional=N×indexFactor. Constituent notional=N×factor×weight/activeWeight when defaults exist; curves and recovery are per name. Aggregate analytic CDS NPVs/legs; apply index upfront once. Par spread divides summed protection by summed notional annuities, not average spreads.
The informational schedule weights contractual coupons by conditional payment-date survival plus half period default mass; protection is LGD×conditional default mass at midpoint hazard-time default date, then weekday settlement delay. It uses payment dates as survival endpoints, not analytic accrual endpoints; schedules merge constituents and append upfront.
visit_positions exits at as-of≥premium maturity, suppressing unpaid rolled coupons unlike single-name pricing. Projection separately retains dates≥as-of, so boundary behavior differs from analytic coupon pricing.

## Selection and controls

IndexPricing explicit selection; default/factor validation occurs before pricing. Synthetic CDS shares convention fields and excludes aggregate upfront.

## Conventions

Original weights and index factor must not double-count defaults. Projected flow amounts have premium side sign and opposite protection sign.

## Limitations, failure and fallback

Projected schedule is informational and will not generally discount back to analytic PV; half-AoD, payment survival and weekday settlement differ from CDS kernels. Maturity early-exit loses live payment receivables.

## Alternatives and tradeoffs

Unified analytic expected cashflow rows would improve reconciliation but require timing semantics; constituent pricing is already supported instead of single index hazard.

## Evidence

Owners: [pricer.rs](../../../../finstack-quant/valuations/credit/src/cds_index/pricer.rs), [types.rs](../../../../finstack-quant/valuations/credit/src/cds_index/types.rs), [parameters.rs](../../../../finstack-quant/valuations/credit/src/cds_index/parameters.rs). [Index tests](../../../../finstack-quant/valuations/credit/tests/cds_index) locate weight/default-factor and single-name aggregation checks.

## Follow-up tracking

Adoption questions and implementation or validation work are tracked in the
[credit-model and valuation follow-ups](../FOLLOW_UPS.md#credit-models-and-valuations).
