# VALUATIONS-STRUCTURED-CREDIT-04: Delinquency migration, modification and servicer advancing

- **Decision status:** Accepted 2026-11-15 — complete delinquency entry, dated migration, cure and modification, advances and reimbursement package.
- **Implementation status:** Observed in the owning paths below. Reconciliation to the accepted contract is pending.
- **Validation status:** Independent migration, target inversion, cohort cash and advance-ledger calculations checked. Production pricing, transition calibration and host qualification remain pending.
- **Audit date:** 2026-11-05.
- **Source revision:** `7e82fd03ad98346727769e88dd5ac5d84b56e9fc`; runtime source unchanged by this audit.

## Accepted decision for delinquency migration and advancing

1. **Separate delinquency entry from default.** Require an explicit
   monthly entry probability for the qualified loan, cohort or deal
   population. Entry moves current par to the first bucket. Terminal
   roll creates gross default. Remove the lookahead inversion that
   divides future default targets by a product of roll rates. Reject
   combined migration and original-par targets or direct simulated
   defaults on the same exposure absent a qualified mapping. The
   statistical collateral owner governs direct default amounts.
   [Accepted statistical collateral projection](03-statistical-collateral-projection.md#accepted-decision-for-statistical-collateral-projection).

2. **Apply one transition to the opening state at each qualified
   monthly node.** For opening bucket par `B` and modification
   share `m`, move `m*B` to the modification branch, then split
   the remainder into cure, forward roll and stay with respective
   weights `c`, `r` and `1-c-r`. Require compatible bucket vectors,
   probabilities in `[0,1]` and `r+c<=1`. Zero roll is valid.
   New entrants, forward rolls, cures and modifications become
   eligible for their next transition at the next monthly node.
   Execute every due node within longer payment periods. Cash
   events can occur in a stub without a migration node.

3. **Keep par, borrower arrears and servicer claims separate.** Admit
   a dated opening snapshot with bucket and cohort par, interest
   arrears, principal arrears and outstanding advances. Require
   qualified explicit zeroes where applicable. Propagate checked
   same-currency balances and allocate each bucket's claims along
   its own transition branches. Resolve missed dues from that
   cohort's contractual schedule, including when performing par
   is zero. Cure retains existing terms, settles qualified arrears
   once, retires any principal actually repaid and resumes ordinary
   payments at the next eligible due date. Require explicit phases
   on dates shared by cash and migration.

4. **Keep modified cashflows in their own expected cohort.** Require
   an effective date, revised coupon or spread, recast installment,
   amortization horizon and legal maturity. An amortization extension
   alone can leave a balloon at legal maturity. Select the treatment
   of interest and principal arrears separately, including any
   authorized repayment, deferral, forgiveness or capitalization.
   Rescheduling principal already in par preserves that balance.
   Genuine principal forgiveness is a noncash write-down. Servicer
   claims retain their own reimbursement rights.
   [Fannie Mae modification precedent](https://singlefamily.fanniemae.com/servicing/flex-modification).

5. **Advance only qualified unpaid current dues.** Select no advancing
   or a qualified principal-and-interest policy. Cap new cash by
   unpaid current eligible dues and the available headroom under
   the stated outstanding-claim cap on still-delinquent par. Use
   explicit component allocation. Record trust cash and the
   corresponding servicer claim as separate effects. Borrower par
   is retired by principal paid or a qualified noncash retirement.
   [Fannie Mae advance and reconciliation guidance](https://guide-servicing.fanniemae.com/svc/a2-1-01/general-servicer-duties-and-responsibilities).

6. **Reimburse from authorized dated cash and retain residuals.**
   Apply eligible cure receipts and loan-specific recovery proceeds
   to their associated claim on actual receipt. General-collections
   reimbursement requires express authority, eligible funding accounts
   and waterfall priority. Preserve authorized unpaid balances through
   their own disposition. Require a qualified charge-off, extension
   or workout for remaining delinquency at maturity. Keep gross
   defaulted par, recovery cash and servicing claims separate.
   [Waterfall claims](11-waterfall-claims-and-rounding.md),
   [Terminal loss and cash rules](14-loss-recognition-and-terminal-cash.md#accepted-decision-for-loss-recognition-and-terminal-cash).

7. **Use one checked Rust migration engine.** The active model covers
   expected balances on statistical collateral. Compatible stochastic
   and OAS routes may supply qualified path-dependent entry assumptions
   and execute expected migration within each path. Reject instrument-
   backed collateral and incompatible default routes. Use the same
   validation and state restoration in the existing Python and WASM
   interfaces. Report arrears, advances, reimbursement and loss
   separately.

## Independent calculations

- Opening current par 700 and two delinquent buckets 200 and 100,
  with 10% entry and 10% modification, and roll/cure pairs 40%/25%
  and 50%/20%, give 70 entries, 30 modifications, 63 cures and
  45 gross default. Closing current par 723 and buckets 133 and
  99 reconcile to 955 surviving par. A separate two-bucket case
  starting with 40 in the first bucket, 50% monthly roll in each
  and no other transitions gives charge-offs of 0, 10, 10 and 7.5
  in four months, even though a roll-product inversion could match
  a single earliest target of 10.
- An advance-ledger check gives 3.00 opening claims plus 0.75 new
  cash, less 0.75 cure reimbursement, 0.50 loan recovery and 0.10
  authorized general collections, leaving 2.40 outstanding, including
  0.15 of unpaid residual. A borrower-principal bridge closes at
  94 from opening 100 after 1 of cured principal and 5 of charge-off.
- Two 50-face bullet cohorts, one paying 12% and maturing in year one
  and the modified one paying 6% and maturing in year two, produce
  cash 59 then 53. Blending the rate and keeping the first maturity
  would produce 109 at year one.

## Observed implementation and scope

With delinquency configured, the observed default model primarily supplies entry into delinquency, and terminal roll-through creates charge-offs. The implementation models expected bucket balances. The following sections describe audited behavior. The accepted contract above governs reconciliation.

## Algorithm and input domain

Each monthly step sizes new delinquency entries, processes previously existing buckets through modification/cure/roll branches, and carries the remainder. Cure returns balance to current performance in that period. Arrears on curing balances repay servicer advances first; unrecovered arrears reach collections according to the state recursion. Modification can forgive arrears, lower coupon and extend term through weighted asset-level terms; it does not create independently tracked borrower vintages.

For timing/cumulative-loss default inputs, the engine looks ahead to expected future charge-offs and grosses the required delinquency entry up by the product of roll rates. Every roll rate must therefore be positive for that inversion. This is a heuristic conversion, not a calibrated competing-risk estimation. Buckets and cure/roll/modification arrays must have compatible shape and probabilities; opening bucket currency must match the asset and bucket totals cannot exceed balance.

Advances replace missing scheduled interest and principal, capped as a fraction of delinquent balance. Recoveries and cures repay those advances according to recoverability and policy. Nonrecoverable advances may be reimbursed from general collections when configured, reducing cash available to the waterfall. Advancing does not create new collateral principal.

## Controls, conventions and failure behavior

Monthly probabilities are fractions; bucket amounts and arrears are pool currency. The deal's month frequency is decomposed into monthly transitions. Missing deal delinquency configuration with seeded buckets errors, as does use with instrument-backed collateral. Modification rate/term aggregation, no-forward-roll conditions and initial-bucket behavior are material state choices.

## Alternatives and tradeoffs

Immediate hazard defaults are supported by omitting delinquency. Explicit per-loan Markov states and separate modification cohorts would preserve more path dependence but require different state and calibration data; they are not performed by the weighted recursion. The current expected-balance method is inexpensive and reproducible but combines vintages with potentially different cure/arrears behavior.

## Implementation and validation evidence

- [structured_credit/types/delinquency.rs](../../../../finstack-quant/valuations/structured-credit/src/structured_credit/types/delinquency.rs)
- [structured_credit/pricing/simulation_engine/delinquency.rs](../../../../finstack-quant/valuations/structured-credit/src/structured_credit/pricing/simulation_engine/delinquency.rs)
- [structured_credit/pricing/simulation_engine/pool_flows.rs](../../../../finstack-quant/valuations/structured-credit/src/structured_credit/pricing/simulation_engine/pool_flows.rs)
- [structured_credit/pricing/simulation_engine/state.rs](../../../../finstack-quant/valuations/structured-credit/src/structured_credit/pricing/simulation_engine/state.rs)

Existing evidence: [structured_credit/unit/delinquency_tests.rs](../../../../finstack-quant/valuations/structured-credit/tests/structured_credit/unit/delinquency_tests.rs). These are behavior/formula/invariant tests; no test was rerun for this audit. Where only a test location was inspected, no independent reference validation is claimed.

The inspected hand schedule uses 100% roll/no cure and checks interest to 0.02 currency units and terminal balances to 1 unit; more general transition calibration and independent mixed cure/modification references remain unestablished.

## Follow-up tracking

Implementation reconciliation and validation work are tracked in
the [structured-credit follow-ups](../FOLLOW_UPS.md#structured-credit).
