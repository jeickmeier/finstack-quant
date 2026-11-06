# VALUATIONS-OTHER-FIXED-INCOME-12: Revolver quote schedules, exposure and draw-option attribution

- **Decision status:** Draft — observed implementation; adoption and rationale require agreement.
- **Audit date/source:** 2026-11-05; `7e82fd03ad98346727769e88dd5ac5d84b56e9fc`.
- **Implementation status:** Observed in the entry points and kernels linked below.
- **Validation status:** Source and named evidence inspected or located; tests and numerical experiments were not executed. Evidence limits are stated below.
- **Scope inventory:** [README](README.md); [complete source mapping](coverage.json).

## Choice and rationale

[Quote metrics](../../../../finstack-quant/valuations/fixed-income/src/revolving_credit/metrics/quotes.rs) apply a clean price to funded principal while retaining future fees and signed funding in the holder schedule. Stochastic facilities use the expected cashflow schedule, not an expectation of individually solved path yields. [Exposure metrics](../../../../finstack-quant/valuations/fixed-income/src/revolving_credit/metrics/exposure.rs) make undrawn and LC default funding visible. These are separate economic measures; their grouping does not establish a common calibration or accounting standard.

## Quote and cost algorithms

1. Build/cache the schedule at the valuation anchor. Settlement follows the facility's business-day lag. Deterministic settlement drawn replays future events; stochastic settlement drawn uses the current anchor balance. Quoted target is `clean_percent × settlement_drawn/100 + accrued`. Without quote, subtract discount-only cashflows in `(as_of,settlement]` from model PV and divide by settlement DF; survival over the lag is not removed.
2. Accrued is full-period scheduled interest times elapsed/full contractual day count. This prorates the scheduled coupon even if balances change within the period; it does not replay elapsed utilization. Calls supply default day-count context, unlike the coupon engine's enriched context. No current period/interest returns zero.
3. Floating-only discount margin uses future cash settlement flows and the periodic z-spread kernel in [record 06](06-loan-quotes-eir.md); quoted DM can invert to clean points only when settlement drawn is positive. YTM solves XIRR against the same target, including negative future draws. AllInRate divides cash interest plus fees by time-integrated drawn balance; a seasoned schedule inserts an opening funding leg to reconstruct its starting balance.
4. [OID/EIR](../../../../finstack-quant/valuations/fixed-income/src/revolving_credit/metrics/oid_eir.rs) rebuilds the schedule at issue, adds opening negative drawn principal and upfront receipt, and delegates to shared origination amortized-cost reporting. Include-fees defaults on. Current `drawn` is treated as opening issue funding for this report; a seasoned current-balance snapshot is not a reconstructed historical origination record.
5. [Approximate weighted cost](../../../../finstack-quant/valuations/fixed-income/src/revolving_credit/metrics/weighted_average_cost.rs) freezes current drawn/LC/commitment and fee tiers. Floating index is the remaining-time-weighted average of one contractual forward per period. Cost is interest plus current commitment, usage, facility and LC/fronting fees, divided by **commitment**, not drawn. It excludes upfront/fixed future fee timing and future balance/tier changes. Zero commitment returns zero.

## Credit and option metrics

- EAD is current `drawn + LEQ × undrawn + LC_default_draw`; availability is commitment minus drawn minus LC, and displayed utilization is drawn/commitment (LC is included for fee tiers separately).
- ExpectedLoss returns risk-free counterfactual PV minus base PV. The counterfactual removes the hazard identifier, default-draw factors and stochastic credit spread. It returns zero immediately if no hazard identifier exists, including explicit stochastic-credit configurations. Reusing a seed does **not** imply unchanged utilization when spread sensitivity is nonzero: removing spread changes utilization drift, so the difference can include behavioral funding effects despite the source comment promising identical paths.
- Recovery01 and JumpToDefault require deterministic draw events plus hazard curve. Recovery01 uses a ±1 percentage-point recovery shock with one-sided bounds; replayable credit calibration rebases hazards to unchanged par spreads. Immediate-default value is `R×drawn + (R−1)×(LEQ×undrawn+LC_draw)`, with no accrued; JTD subtracts base PV.
- [CS01](../../../../finstack-quant/valuations/fixed-income/src/revolving_credit/metrics/cs01.rs) uses quote-rebootstrap/direct-hazard risk when a credit curve exists. Otherwise deterministic cashflows receive periodic z-spread parallel/bucketed bumps; stochastic facilities without hazard identifiers reject because no deterministic holder schedule is available.
- [Draw option cost](../../../../finstack-quant/valuations/fixed-income/src/revolving_credit/pricing/path_pricing.rs) is an attribution on each simulated path. At each retained observation calculate positive change in commitment × utilization; multiply it by spread widening from the anchor and by the remaining discounted survival-weighted accrual annuity, then negate and sum. Narrowing can produce a positive value. Constant-spread configurations return zero even though their simulated spread has small CIR volatility. The MC mean/interval uses [record 11](11-revolver-monte-carlo-credit.md) aggregation. This is not an optimal-exercise option price or a separate additive component of total PV.

## Conventions, alternatives and limitations

Rates/yields/DM are annual decimals; prices are points per 100 drawn; PV/exposure/cost attribution are lender-side money. Quote metrics can mix credit-adjusted model targets with unweighted expected contractual cashflows. Recovery metrics intentionally exclude stochastic credit. A state-conditioned draw-option model, realized-default simulation, historical EIR ledger and elapsed-balance accrued are unimplemented alternatives; each changes the economic contract, not merely a numerical tolerance.

## Evidence

[Draw-option tests](../../../../finstack-quant/valuations/fixed-income/tests/revolving_credit/draw_option_cost.rs), [weighted-cost tests](../../../../finstack-quant/valuations/fixed-income/tests/revolving_credit/weighted_average_cost.rs), [default-risk tests](../../../../finstack-quant/valuations/fixed-income/tests/default_risk.rs) and [review cases](../../../../finstack-quant/valuations/fixed-income/tests/revolving_credit/review_fixes.rs) provide located contract/limiting-case evidence, not executed validation. [Fixed-income handbook](../../../REFERENCES.md#fabozzi-fixed-income-handbook) supplies general quote context; no independent provenance for the exact draw-option attribution was established.

## Follow-up tracking

Adoption, implementation and validation work is tracked in the
[scope follow-ups](../FOLLOW_UPS.md#other-fixed-income).
