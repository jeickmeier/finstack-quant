# VALUATIONS-STRUCTURED-CREDIT-05: NPL resolution and reperformance

- **Decision status:** Accepted 2026-11-15 — complete workout dates, expected par split, reperformance, carrying costs and deterministic dispatch package.
- **Implementation status:** Observed in the owning paths below. Reconciliation to the accepted contract is pending.
- **Validation status:** Independent calendar, par-conservation, cost-timing and nonlinear allocation calculations checked. Production pricing, workout calibration and host qualification remain pending.
- **Audit date:** 2026-11-05.
- **Source revision:** `7e82fd03ad98346727769e88dd5ac5d84b56e9fc`; runtime source unchanged by this audit.

## Accepted decision for NPL resolution and reperformance

1. **Book the workout date explicitly.** Supply either a literal economic
   resolution date or an elected workout anchor plus positive checked
   calendar months. Acquisition and deal closing are eligible anchors
   only by election. Admit economic resolution within deal legal maturity.
   Keep the resolution event, liquidation receipt and deal distribution
   at their respective qualified dates and phases. A surviving exposure
   at maturity or receipt after legal final needs express terminal
   authority. A post-resolution snapshot requires the actual outcome and
   outstanding or discharged cash. Carry that state forward without
   replaying the split.
   [Accepted date arithmetic](../core-conventions/007-tenor-date-arithmetic.md),
   [Terminal cash](14-loss-recognition-and-terminal-cash.md#accepted-decision-for-loss-recognition-and-terminal-cash).

2. **Keep pending workout par in its own state.** Retain unresolved
   NPL par in gross outstanding exposure. Exclude it from performing
   statistics, ordinary coupon and principal, default and prepayment
   inputs, delinquency migration and advances. Require one resolution
   authority for each exposure and reject a competing opening recovery
   claim on the same par. Coverage and calls consume the state under
   their own qualified rules.

3. **Conserve both branches and retire liquidated par once.** At the
   resolution event, apply an explicit reperformance probability
   `p in [0,1]` to opening par `B`. Reperforming par is `B*p` and
   liquidated par is the checked residual. Preserve each representable
   positive branch and reject an unrepresentable split. Retire gross
   liquidated par once and track pending recovery separately from
   principal. Use accepted Money and cashflow boundaries.
   [Accepted cashflow semantics](../core-conventions/015-cashflow-semantics.md).

4. **Resume performance on qualified dated terms.** Elect retained or
   revised coupon, amortization and maturity terms explicitly. Start
   accrual in the first eligible resumed interval and recast the
   installment on surviving par where required. Preserve the qualified
   floating index, spread and floors unless expressly changed. Keep
   separately authorized arrears as their own claims. Apply ordinary
   statistical projection from resumption.
   [Accepted statistical collateral projection](03-statistical-collateral-projection.md#accepted-decision-for-statistical-collateral-projection).

5. **Net only authorized same-date costs.** Apply explicit gross
   proceeds and carrying costs, each as a percentage in `[0,100]`
   of liquidated par. In the net-at-receipt profile, costs may not
   exceed gross proceeds. Costs due on other dates or exceeding
   proceeds require separately qualified funding and cash treatment.
   Report gross proceeds, costs, net recovery and undiscounted net
   principal shortfall separately. Pay recovery on the actual receipt
   date without an ordinary recovery lag. Apply eligible advance
   reimbursements or servicing fees once.
   [ECB workout and sale-cost guidance](https://www.bankingsupervision.europa.eu/ecb/pub/pdf/guidance_on_npl.en.pdf).

6. **Use deterministic expected-flow pricing for unresolved workouts.**
   Run one waterfall on the expected reperformance and liquidation
   cashflows. Nonlinear tranche outcomes can differ from averages
   across discrete resolution paths. Reject unresolved workouts in
   stochastic valuation and stochastic OAS under this profile. Require
   workout-specific authority for scenarios that change workout terms.
   Use the same checked Rust state, dates and validation in existing
   Python and WASM interfaces.
   [Accepted dispatch and assumptions](01-dispatch-and-assumptions.md#accepted-decision-for-structured-credit-dispatch-and-assumptions).

## Independent calculations

- An elected 2025-01-31 anchor plus one month resolves on 2025-02-28.
  A 2025-03-01 opening snapshot carries the resolved par and pending
  cash through a 2025-03-10 receipt and 2025-03-31 distribution without
  another split. With 100 par, 20% reperformance, 55% gross proceeds
  and 5% costs, the recognized amounts are 20 survivor par, 80
  retired par, 44 gross proceeds, 4 costs and 40 net recovery.
- On par 1,000, reperformance 25%, gross proceeds 72% and costs 6%,
  the split yields 250 survivor par and 750 liquidated par. Gross
  proceeds 540 less 45 costs gives 495 net recovery and 255 net
  principal shortfall. At receipt factor 0.95 the net PV is 470.25.
  If the costs instead pay earlier at factor 0.99, PV is 468.45.
- With senior liquidation claim capped at 400, one waterfall on
  expected liquidation cash 495 gives senior/residual 400/95.
  A separate all-or-nothing resolution with liquidation cash 660
  at probability 75% and zero otherwise gives mean senior/residual
  300/195. Each method has total expected liquidation cash of 495.

## Observed implementation and scope

An NPL row remains dormant until resolution, then splits its balance
in expectation between a reperforming slice and a liquidated slice.
The following sections describe observed behavior. The accepted
contract above governs reconciliation.

## Algorithm and domain

The asset produces no contractual interest or scheduled principal before its resolution date, anchored to acquisition/origination or deal closing plus `months_to_resolution`. At resolution, the reperforming probability determines a surviving slice; the remainder liquidates using specified net proceeds after carrying costs. Reperforming collateral begins its resumed schedule in the next period. The state books defaults, recoveries and losses on that timeline rather than through an already-defaulted opening claim.

Validation rejects simultaneous `defaulted`, `default_date` or `recovery_amount`, incompatible currencies/probabilities, instrument-backed collateral, and resolution beyond legal maturity. Acquisition-date anchoring matters for seasoned pools; the same months-to-resolution does not mean the same valuation-date horizon for every row.

## Selection, limitations and conventions

All amounts are in pool currency, probabilities are fractions, time is whole months. Expected reperformance is fractional collateral and can change later amortization/coupon state. The general recovery-lag/default model is a separate path; it must not also account for the same NPL loss. Costs and proceeds are scenario inputs; there is no endogenous collateral-price, foreclosure-cost or borrower-cure model.

## Alternatives and tradeoffs

Initial default claims with known pending recovery are supported separately. A discrete resolution simulation, stochastic recovery distribution or multiple workout stages would represent uncertainty differently. Nonlinear tranche values still require qualification on the accepted expected-flow basis.

## Implementation and validation evidence

- [structured_credit/types/npl.rs](../../../../finstack-quant/valuations/structured-credit/src/structured_credit/types/npl.rs)
- [structured_credit/types/instrument.rs](../../../../finstack-quant/valuations/structured-credit/src/structured_credit/types/instrument.rs)
- [structured_credit/pricing/simulation_engine/pool_flows.rs](../../../../finstack-quant/valuations/structured-credit/src/structured_credit/pricing/simulation_engine/pool_flows.rs)

Existing evidence: [structured_credit/unit/npl_tests.rs](../../../../finstack-quant/valuations/structured-credit/tests/structured_credit/unit/npl_tests.rs). These are behavior/formula/invariant tests; no test was rerun for this audit. Where only a test location was inspected, no independent reference validation is claimed.

Reference context: [LGD context](../../../REFERENCES.md#schuermann-2004-lgd). These references identify the method family; they do not certify the preset parameters or every implementation branch.

Independent workout datasets and parameter provenance were not established.

## Follow-up tracking

Implementation reconciliation and validation work are tracked in
the [structured-credit follow-ups](../FOLLOW_UPS.md#structured-credit).
