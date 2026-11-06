# VALUATIONS-STRUCTURED-CREDIT-19: Borrowing base and asset-backed facility synthesis

- **Decision status:** Draft — observed implementation; adoption pending agreement.
- **Implementation status:** Observed in the owning paths below; scope: eligible collateral, concentration limits, facility advances and lender cashflows.
- **Validation status:** Evidence inspected/located, not executed. Source and selected assertions establish implemented behavior, not general numerical accuracy or market approval.
- **Audit date:** 2026-11-05.
- **Source revision:** `7e82fd03ad98346727769e88dd5ac5d84b56e9fc`; runtime source unchanged by this audit.

## Choice and rationale

An asset-backed facility is represented as a two-class structured-credit deal: a senior facility note and residual collateral equity. It therefore reuses the same funding, coverage and loss engine. Borrowing-base eligibility is distinct from market valuation and requires lender-specific adoption.

## Algorithm and conventions

Each asset receives an exact asset-class advance rule, else wildcard `*`, else ineligible. Eligibility filters include default state, maturity, delinquency days, unresolved NPL and other rule fields. Live evaluation uses current balances/status rather than unchanged original input rows. Eligible balances are grouped by obligor, industry or asset class for concentration caps measured against total eligible collateral before exclusions. Limits are applied sequentially; over-cap groups are proportionally reduced. Missing grouping metadata escapes that dimension's cap. Advance rates weight adjusted collateral; cash/account additions and debt determine borrowing-base availability/deficiency.

The synthetic facility note begins at drawn amount; residual is collateral balance minus drawn. A borrowing-base test at 1.0 after note interest directs deficiency cash to senior paydown and suspends reinvestment. Notes preserve par and permit principal to support senior interest. Statistical collateral reinvests until effective revolving end; instrument collateral amortizes from closing. Scheduled draws become note-funded cash, and optional readvance uses available borrowing base capped by commitment. Term-out creates a deal call at repayment date, optionally relative to early-amortization occurrence, using the configured liquidation price.

Commitment fee is `(commitment - opening note balance)+ × fee_bp/10000 × accrual` until commitment end, prorated for a partial last accrual and paid on note payment date. Lender cashflows sum interest, principal and fees, subtract future funded draws, and aggregate by date. Facility IRR adds `-drawn` at valuation date and solves Act/365F XIRR. It is a return on current drawn capital, not a mark-to-market purchase-price yield.

Facility reporting uses initial collateral borrowing base, percent cushion `(base - drawn)/base × 100`, and decimal utilization `drawn/base`; nonpositive base errors for the ratios. Residual IRR is the synthetic equity IRR at par. Registered DV01 uses the shared combined discount/forward bump, bucketed DV01 triangular key rates, and CS01 the shared hazard-when-credit-curve / otherwise z-spread machinery. Its z-spread cashflow adapter includes lender draws and commitment fees, uses payment-frequency compounding and includes dates on or after valuation. These delegated conventions differ from the continuous-spread structured-note helper.

## Domain, limitations and alternatives

Facility amounts, commitment, drawn/current collateral, currency, dates and event units validate; cumulative-loss events are fractions, concentration caps percentages and advance rates fractions. Statistical synthetic residual must be positive under construction. Commitment fees are appended to lender receipts rather than competing as a collateral waterfall claim; this assumes an external payer or requires a separate source-of-cash justification. Overlapping concentration dimensions are sequential haircuts and can be order-sensitive; missing metadata can overstate availability.

A direct standalone loan model would be a different methodology. Supported controls include rule inputs, no readvance, scheduled draws, term-out and liquidation price; no automatic lender credit policy is inferred.

## Implementation and validation evidence

- [asset_backed_facility/types.rs](../../../../finstack-quant/valuations/structured-credit/src/asset_backed_facility/types.rs)
- [asset_backed_facility/pricing.rs](../../../../finstack-quant/valuations/structured-credit/src/asset_backed_facility/pricing.rs)
- [asset_backed_facility/metrics.rs](../../../../finstack-quant/valuations/structured-credit/src/asset_backed_facility/metrics.rs)
- [structured_credit/types/borrowing_base.rs](../../../../finstack-quant/valuations/structured-credit/src/structured_credit/types/borrowing_base.rs)
- [structured_credit/types/draws.rs](../../../../finstack-quant/valuations/structured-credit/src/structured_credit/types/draws.rs)

Existing evidence: [asset_backed_facility/event_unit_tests.rs](../../../../finstack-quant/valuations/structured-credit/tests/asset_backed_facility/event_unit_tests.rs); [structured_credit/unit/funding_tests.rs](../../../../finstack-quant/valuations/structured-credit/tests/structured_credit/unit/funding_tests.rs). These are behavior/formula/invariant tests; no test was rerun for this audit. Where only a test location was inspected, no independent reference validation is claimed.

The inspected event tests verify units and rejected retired keys, not an independent borrowing-base/legal facility reference.

## Follow-up tracking

Adoption questions, implementation reconciliation and validation work are tracked in
the [structured-credit follow-ups](../FOLLOW_UPS.md#structured-credit).
