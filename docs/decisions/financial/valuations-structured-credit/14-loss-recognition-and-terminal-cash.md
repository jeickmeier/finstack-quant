# VALUATIONS-STRUCTURED-CREDIT-14: Loss recognition, deal calls and terminal recovery treatment

- **Decision status:** Accepted 2026-11-15 — complete loss recognition, note principal effects, deal calls and terminal cash package.
- **Implementation status:** Observed in the owning paths below. Reconciliation to the accepted contract is pending.
- **Validation status:** Independent loss, cleanup funding and dated terminal cash calculations checked. Production pricing, transaction terms and host qualification remain pending.
- **Audit date:** 2026-11-05.
- **Source revision:** `7e82fd03ad98346727769e88dd5ac5d84b56e9fc`; runtime source unchanged by this audit.

## Accepted decision for loss recognition and terminal cash

1. **Elect recognition and note write-down independently.** Require
   explicit at-default or at-liquidation loss recognition and explicit
   interim note write-down or par preservation. Remove family defaults.
   At default recognize gross defaulted par less the qualified recovery
   amount once. At liquidation recognize the finalized economic shortfall
   once, crediting eligible prior receipts. Seasoned state identifies gross
   defaults, qualified recoveries and their prior recognition and allocation.
   Corrections and note write-ups require explicit authority.
   [Filed loss and subsequent-recovery terms](https://www.sec.gov/Archives/edgar/data/1085309/000116231806001511/jpmac2006he3prospectussupple.htm).

2. **Allocate incremental loss against live face.** Apply the explicit
   contractual order, junior first for the elected junior-first profile,
   with pari passu allocation only when specified. Cap each action at live
   face after prior actual principal events. Opening recognized or allocated
   losses and exhaustion of performing collateral create no new loss
   allocation. Record write-down as a zero-cash negative principal effect,
   preserve earned coupons and arrears, and report any unallocated loss.
   Par preservation keeps the contractual coupon basis until a qualified
   principal event or authorized extinguishment. OC uses its separately
   defined carrying values.
   [Accepted OC rules](12-coverage-and-cures.md#accepted-decision-for-coverage-tests-and-cure-diversion) ·
   [Filed loss allocation](https://www.sec.gov/Archives/edgar/data/1176320/000000000015036204/filename1.pdf).

3. **Elect call terms and test the actual funding.** Use an explicit
   no-call, cleanup, or dated whole-deal call election. A call specifies
   the threshold basis and comparator where applicable, notice, detection,
   election, principal phase, redemption scope, actual settlement date and
   price. A cleanup requires contractual eligibility and sufficient
   pro forma settlement funding in the elected priority for eligible fees,
   advances and hedges, earned and deferred interest, actual redemption
   strike and other claims. Failed cleanup continues ordinary performance.
   An authorized forced scenario uses its explicit shortfall and discharge
   rules.
   [Filed whole-deal cleanup terms](https://www.sec.gov/Archives/edgar/data/799850/000117184325000643/exh_102.htm).

4. **Use qualified proceeds at call settlement.** Count available cash,
   actual authorized sale or assignment consideration, and eligible account
   releases. A retained recovery follows its own qualified receipt and
   distribution dates and named beneficiary. Count actual consideration for
   an assigned claim once. Track cash, gross retired par, redemption premium
   or discount, and collateral loss separately. Tranche price-to-call is
   a named hypothetical calculation with its own scope and ledger.

5. **Resolve each terminal entitlement on its authorized dates.** Assign
   every outstanding asset, claim and account a qualified actual sale or
   closeout or a retained entitlement with specified receipt and distribution
   dates and beneficiary. Reject unresolved or unqualified tails. Legal
   maturity, final distribution and discharge are distinct events. Release
   reserve, excess spread, funding and carried cash only at eligible legal
   dates and against the named source, recipient and priority. Use the same
   qualified equity incentive calculation at terminal settlement. Carry
   unpaid claims through authorized legal discharge and record any final
   note write-down as a separate noncash action. The residual recipient
   requires express authority. A date passing does not discharge claims.
   [Accepted incentive rules](11-waterfall-claims-and-rounding.md#accepted-decision-for-waterfall-claims-and-rounding) ·
   [Filed limited-recourse terms](https://www.sec.gov/Archives/edgar/data/1379785/000137978519000025/exhbiti101-cloindenture.htm).

6. **Remove the absolute 100-unit stop.** Close the path only when all
   remaining assets, commitments, recoveries, zero-cash claims, accounts,
   arrears and required actions are exhausted or discharged under the
   elected terms. Rust owns the dated ledger and state diagnostics.
   Existing Python and WASM consumers expose those same results.

## Independent calculation checks

- A new gross default of 70 with qualified recovery of 28 recognizes 42.
  After an earlier principal payment of 8, junior absorbs its live 10 and
  senior absorbs 32 of 60. At-default recognition leaves senior face 28
  immediately, while the at-liquidation election recognizes 42 upon
  finalization. The recovery receipt leaves prior at-default recognition
  unchanged. Par preservation keeps total face 70. Eligible carrying value
  28 produces OC 0.40 on face 70 and 1.00 on face 28. Earned coupon on 70
  at 8% over a 0.25 accrual remains 1.40.
- A remaining factor of 100/1,000 qualifies at a 10% cleanup threshold
  with an inclusive comparator. Sale proceeds 100 and available cash 12
  total 112 against redemption 105, interest 8 and fees 3, so cleanup
  fails. A retained future recovery of 20 supplies no settlement cash.
  An eligible assignment that settles for 16 brings cash to 128 and
  leaves 12 after claims. Retiring 80 par at a 95% strike uses 76 cash
  and records 4 of exercise discount.
- Senior and junior face of 100 and 25 have earned coupons 5 and 4.
  Permitted spread 9 settles the coupons. Authorized principal cash of
  50, 30 and 12 pays senior, leaving 8 senior and 25 junior face before
  legal discharge. Cash of 59 on June 30, 30 on August 31 and 12 on
  September 30 discounted by 0.98, 0.96 and 0.95 has PV 98.02.
  Moving all cash to June 30 instead gives PV 98.98.

## Observed implementation and scope

The audit found family defaults for loss recognition and note
write-down. RMBS and CMBS select liquidation recognition and note
write-down. CLO, CBO, ABS, auto and card select default recognition and
par preservation.

Incremental recognized net loss subtracts opening historical loss and already allocated writedowns, then is allocated bottom-up in note priority, capped at live outstanding principal. Principal repayments and previous writedowns therefore reduce remaining absorption capacity. Under par preservation, realized losses still affect tests but do not immediately reduce note coupon bases. Final unpaid face is written down regardless of interim policy.

A dated deal call or cleanup decision is made at a period boundary, then the period is simulated and remaining collateral liquidated. Cleanup requires proceeds sufficient for applicable claims; a forced contractual scenario call may create losses. Price and premium terms are percentages of par. Deal/cleanup calls include pending recovery amounts in immediate proceeds and drain those claims on redemption. Reserve, excess-spread and accumulated cash are released according to their terminal rules before unresolved balances are closed.

The loop can stop when the remaining performing pool is at or below 100 currency units. Ordinary terminal completion first releases current cash accounts, then preserves each pending recovery's full contractual lag: payment occurs at the calendar-adjusted later of default date plus lag and the last simulated payment date. Final unpaid-face writedown is dated at the last such recovery settlement, which may be after legal maturity. This differs from the immediate pending-claim treatment at calls. Residual flow recording identifies the maximum payment-priority class; this does not establish correct residual treatment for structures with no explicit equity or unusual residual placement.

Explicit `LossAllocationPolicy`, `LossRecognition`, call scope/date and liquidation price are supported. `CallScope` can limit redemption behavior; model inputs validate dates and debt IDs. Including pending recovery amounts immediately at a call is not the same as discounting or selling those claims. Ordinary completion permits recovery cash and final writedown after legal maturity; whether that matches each note's legal entitlement is a separate decision. Pool-exhaustion thresholds are absolute currency amounts and can materially affect very small deals. Tranche hypothetical call metrics are separate calculations and can treat premium as principal; they must not be confused with the deal liquidation path.

## Implementation and validation evidence

- [structured_credit/types/enums.rs](../../../../finstack-quant/valuations/structured-credit/src/structured_credit/types/enums.rs)
- [structured_credit/types/call.rs](../../../../finstack-quant/valuations/structured-credit/src/structured_credit/types/call.rs)
- [structured_credit/pricing/simulation_engine/orchestration.rs](../../../../finstack-quant/valuations/structured-credit/src/structured_credit/pricing/simulation_engine/orchestration.rs)
- [structured_credit/pricing/simulation_engine/simulate_period/losses.rs](../../../../finstack-quant/valuations/structured-credit/src/structured_credit/pricing/simulation_engine/simulate_period/losses.rs)
- [structured_credit/pricing/simulation_engine/simulate_period/record.rs](../../../../finstack-quant/valuations/structured-credit/src/structured_credit/pricing/simulation_engine/simulate_period/record.rs)
- [structured_credit/utils/simulation.rs](../../../../finstack-quant/valuations/structured-credit/src/structured_credit/utils/simulation.rs)

Existing evidence: [structured_credit/unit/loss_allocation_tests.rs](../../../../finstack-quant/valuations/structured-credit/tests/structured_credit/unit/loss_allocation_tests.rs); [structured_credit/unit/call_tests.rs](../../../../finstack-quant/valuations/structured-credit/tests/structured_credit/unit/call_tests.rs); [structured_credit/integration/simulation_edge_tests.rs](../../../../finstack-quant/valuations/structured-credit/tests/structured_credit/integration/simulation_edge_tests.rs). These are behavior/formula/invariant tests; no test was rerun for this audit. Where only a test location was inspected, no independent reference validation is claimed.

The delayed-loss recognition tests establish booking behavior, not the legal or economic appropriateness of either terminal policy.

## Follow-up tracking

Implementation reconciliation and validation work are tracked in
the [structured-credit follow-ups](../FOLLOW_UPS.md#structured-credit).
