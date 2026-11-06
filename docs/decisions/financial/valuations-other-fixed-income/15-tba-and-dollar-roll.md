# VALUATIONS-OTHER-FIXED-INCOME-15: TBA assumed collateral, settlement ownership and dollar-roll carry

- **Decision status:** Draft — observed implementation; adoption and rationale require agreement.
- **Audit date/source:** 2026-11-05; `7e82fd03ad98346727769e88dd5ac5d84b56e9fc`.
- **Implementation status:** Observed in the entry points and kernels linked below.
- **Validation status:** Source and named evidence inspected or located; tests and numerical experiments were not executed. Evidence limits are stated below.
- **Scope inventory:** [README](README.md); [complete source mapping](coverage.json).

## Choice and rationale

[TBA pricing](../../../../finstack-quant/valuations/fixed-income/src/tba/pricer.rs) values delivery of a deterministic assumed MBS pool. [Dollar-roll pricing](../../../../finstack-quant/valuations/fixed-income/src/dollar_roll/pricer.rs) takes long back TBA minus long front TBA. Carry diagnostics share the projected pool's coupon and prepayment assumptions. This enforces internal cashflow consistency but does not price delivery choice or forecast market specialness.

## TBA algorithm and selection

1. Use explicit settlement date when supplied; otherwise use published SIFMA year/month/class lookup. Class is explicit or inferred from agency and term. Missing published dates error; no invented calendar formula replaces them.
2. Without explicit pool, create one at the first of settlement month with current face equal to trade notional, original face `notional/factor`, maturity after contract term and WAC equal pass-through coupon plus servicing and guarantee fees. [Embedded defaults](../../../../finstack-quant/valuations/fixed-income/data/assumptions/tba_assumptions.v1.json) are factor 1, servicing 25 bp, agency guarantee 25 bp or GNMA 6 bp, PSA multiplier 1. An explicit prepayment model overrides PSA. Generic accrual is 30/360.
3. Explicit pool must match trade currency, coupon, optional supplied factor and delivery agency. FNMA/FHLMC are interchangeable in the implemented agency predicate; GNMA I/II remain separate. Rebase pool to settlement-month ownership and scale to the purchased current face. Earlier month's accrued P&I stays with the seller even if its agency payment is later.
4. Before settlement, value is `DF(as_of,settlement) × [MBS_PV_at_settlement − notional×trade_clean_points/100 − settlement_accrued]`. MBS cashflows follow [record 13](13-mbs-projection.md). At and after settlement, forward PV is zero; delivered collateral must be a separate position.
5. Parallel/bucketed DV01 reprice with the MBS refinancing adjustment from [record 14](14-mbs-risk-oas.md). Explicit pools are rebuilt directly; generic pools get the adjusted prepayment specification. TBA's cashflow exporter rejects physical delivery as lacking a standalone money-only schedule.

## Dollar-roll carry algorithm

[Carry](../../../../finstack-quant/valuations/fixed-income/src/dollar_roll/carry.rs) uses the front generic pool for the interval between actual settlement dates. Project through the interval, accrue coupon on declining balances with pool day count, and recognize principal reductions once at next month's accrual boundary. Both quantities are points per 100 initial current face. With `drop=front_clean−back_clean`,

`implied_financing = [coupon + paydown×(100−back_clean)/100 − drop] / front_dirty × 360/calendar_days`.

Front dirty includes coupon accrued from month start. This correctly makes a wider drop lower implied financing under the implemented convention. [Specialness](../../../../finstack-quant/valuations/fixed-income/src/dollar_roll/metrics.rs) is `(repo_rate−implied_financing)×10,000`. Repo is the simple Actual/360 rate `(1/DF(front,back)−1)/tau` from explicit repo curve or fallback discount curve. Roll days must be positive. Forward legs preserve explicit settlement overrides; risk rebuild applies the front pool's refinancing speed adjustment to both legs.

## Limitations and alternatives

Generic collateral assumptions are embedded values, not calibrated delivery-pool observations. TBA delivery tests are partial eligibility checks, not the complete SIFMA good-delivery rulebook. Neither engine models cheapest-to-deliver choice, specified-pool payups, stochastic drop or balance-sheet scarcity. The roll's back pool is a new generic pool for its settlement month; carry references the front pool, so carry is a diagnostic and is not separately added to PV.

[Dollar-roll cashflow export](../../../../finstack-quant/valuations/fixed-income/src/dollar_roll/types.rs) emits positive dirty front sale proceeds and negative dirty back purchase payment as contractual Notional cashflows. It does not export delivered collateral or its income. Thus discounting that cash schedule does not generally reproduce roll PV, unlike a cash-settled derivative schedule. This differs deliberately in observed behavior from TBA's rejection, but a common physical-delivery reporting contract remains unagreed. Explicit delivery-pool pricing is supported for standalone TBA; portfolio-of-pools optional delivery valuation is unimplemented.

## Evidence

Colocated tests in [TBA pricer](../../../../finstack-quant/valuations/fixed-income/src/tba/pricer.rs) cover settlement-month entitlement and accrued. Dollar-roll carry contains `implied_financing_uses_pool_prepayment_and_dirty_front_price`, a hand-derived monthly amortization/carry case; metrics contain `specialness_resolves_the_supplied_repo_curve` and deterministic prepayment-risk rebuild comparisons. These are narrower than external trade-price validation. [SIFMA formulas](../../../REFERENCES.md#sifma-mbs-standard-formulas) and [good delivery](../../../REFERENCES.md#sifma-tba-good-delivery) are canonical reference pointers.

## Follow-up tracking

Adoption, implementation and validation work is tracked in the
[scope follow-ups](../FOLLOW_UPS.md#other-fixed-income).
