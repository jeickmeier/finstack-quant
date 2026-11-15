# VALUATIONS-STRUCTURED-CREDIT-08: Contractual instrument collateral, hazards and liquidity

- **Decision status:** Accepted 2026-11-15 — complete typed collateral ledger, collection cuts, credit and recovery authority, liquidity and dependency package.
- **Implementation status:** Observed in the owning paths below. Reconciliation to the accepted contract is pending.
- **Validation status:** Independent dated eligibility, principal and credit, floating shock and funded default calculations checked. Production pricing, risk and host qualification remain pending.
- **Audit date:** 2026-11-05.
- **Source revision:** `7e82fd03ad98346727769e88dd5ac5d84b56e9fc`; runtime source unchanged by this audit.

## Accepted decision for instrument collateral and liquidity

1. **Prepare one dated ledger from the underlying contracts.** Use the
   bond, term-loan and revolver cashflow owners. Distinguish accrual and
   fixing dates, effective principal and commitment changes, actual
   receipts and deal distribution. Carry signed cash and explicit
   principal effects separately, including zero-cash PIK. Restore an
   authorized cut-off snapshot or replay qualified actual history once,
   including same-day phases and unpaid claims. A restored drawn
   balance already incorporates its past draws. Use the contractual
   schedule as the basis for qualified credit and utilization overlays.
   [Accepted cashflow semantics](../core-conventions/015-cashflow-semantics.md).

2. **Honor collection eligibility and legal final.** Assign each
   receipt once to the first distribution whose contractual cut
   admits it. Preserve intra-period funding and account ordering on
   the actual dates and phases. Cash beyond legal final requires an
   expressly dated terminal sale or closeout or a qualified retained
   entitlement. Remove automatic acceleration to the last bucket.
   [SEC-filed collection-period example](https://www.sec.gov/Archives/edgar/data/1930087/000110465926005334/tm263361d1_ex10-1.htm),
   [Terminal cash](14-loss-recognition-and-terminal-cash.md#accepted-decision-for-loss-recognition-and-terminal-cash).

3. **Resolve one credit source and recovery authority per name.** A
   qualified named hazard supplies conditional default through local
   integrated intensity over actual exposure. Other names use the
   selected deal or path law. Preserve recognized defaults as dated
   claims. Select recovery from the instrument or claim, then expressly
   adopted hazard recovery metadata, then the qualified deal or path
   source. Sample recovery only for a realized default using an authorized
   stochastic source in the absence of stronger authority. Keep gross
   defaulted par, recovery amount and receipt date separate. Shocks
   perturb the source the name actually uses.
   [Accepted hazard evaluation](../core-market/hazard-survival.md).

4. **Apply credit and prepayment to eligible cash and par.** Use the
   midpoint convention for eligible expected interest. Fees and
   already-earned claims follow their own contractual contingency.
   On expressly eligible nonrevolvers apply prepayment to post-default,
   post-scheduled par. Resolve call and put interactions with the
   exercise owner. Reproject only pending floating resets using their
   contractual accrual basis, index dates and bounds. Preserve authorized
   fixings under forecast shocks. A zero base coupon remains eligible
   for a qualified pending shock.
   [Accepted floating projection](../cashflows/06-floating-rate-projection.md),
   [Accepted collateral exercise](09-collateral-exercise.md#accepted-decision-for-collateral-call-and-put-exercise).

5. **Separate mandatory draws, discretionary utilization and default
   conversions.** Check live commitment, drawn par, letters of credit
   and available capacity at each funding phase. Use expressly
   authorized default-conversion factors on disjoint undrawn and
   letter-of-credit exposures. Actual surviving funding increases
   live par. Funded default conversions enter gross default and
   dated recovery once.
   [Accepted revolver cashflows](../valuations-other-fixed-income/10-revolver-cashflows.md),
   [Accepted revolver credit model](../valuations-other-fixed-income/11-revolver-monte-carlo-credit.md).

6. **Qualify funding on the date and phase it is due.** Use authorized
   reserve cash first and then eligible principal already available.
   Give mandatory draws priority and fail valuation when they cannot
   be funded under the selected terms. Cap modeled requests only when
   the policy permits a shortfall. Allocate simultaneous equal-priority
   requests pro rata. Replenish the reserve from authorized unspent
   repayments. Report requested, funded and unfunded amounts separately
   from the stochastic fraction of paths with a shortfall.
   [SEC-filed funding account example](https://www.sec.gov/Archives/edgar/data/1974793/000119312525172599/d18620dex101.htm).

7. **Resolve the active dependency graph in Rust.** Include nested
   calendar, fixing, curve, credit, recovery, exercise and funding
   requirements consumed by the chosen route. Keep instrument-backed
   collateral exclusive of competing statistical collateral and generic
   reinvestment. Use the same checked ledger, dependency information and
   diagnostics in the existing Python and WASM interfaces.

## Independent calculations

- Initial par 100 less 20 settled amortization plus 5 zero-cash PIK
  gives as-of par 85. An eligible same-day fee 2 and principal payment
  10 produce cash 12 and par 75. If a later 75 cash entitlement is
  admitted through terminal authority, accelerating it from its
  actual factor 0.98 to 0.99 increases PV by 0.75.
- A draw of 10 with reserve 4 has a funding shortfall of 6 at its
  due phase. A principal receipt of 8 later in the same distribution
  period does not fund it retroactively.
- At a common eligible phase, reserve 40 plus principal 20 funds a
  mandatory draw of 30 first, then splits 30 across equal-priority
  default-conversion and discretionary requests of 35 and 15. The
  respective amounts funded are 21 and 9. The unfunded amounts sum to
  20. With drawn par 80, funded conversion 21 produces gross default
  101 and dated recovery 40.40 at 40%.
- A local hazard integral of 0.001 gives conditional default of
  about 0.0999500167% even when absolute survival underflows. For
  pending floating par 1,000, accrual fraction 0.25, index −25 bp,
  spread +25 bp and zero all-in floor, a +40 bp eligible shock
  changes cash from zero to 1. The corresponding authorized fixed cash
  remains zero.

## Observed implementation and scope

Instrument collateral reuses bond and loan schedules and overlays
pool credit, prepayment and funding. The following sections describe
observed behavior. The accepted contract above governs reconciliation.

## Algorithm and controls

Prepared collateral schedules are generated once and bucketed into the first deal payment date on or after the contractual flow. Flows after the final deal payment are assigned to the last bucket. Opening bond amortization and loan historical draws adjust exposure; future floating resets are reprojected with the instrument's rate parameters. Non-cash notional changes remain distinct from cash received.

Per-asset hazard curves supply conditional default probability when present; otherwise the configured deal/path default model supplies it. Explicit instrument or hazard-source recovery remains authoritative; otherwise recovery uses the deal/path specification. Realized copula defaults can sample recovery only when an authoritative instrument/hazard recovery is absent. Expected interest uses the half-period default reduction. Nonrevolver collateral can prepay; revolving collateral does not use that prepayment branch. At default, revolvers can draw undrawn facilities through loan-equivalent/letter-of-credit exposure factors. Actual draws are constrained by committed exposure and available deal liquidity.

Funding first uses applicable reserve resources, then eligible principal; capped allocation shares scarce cash across requests. Diagnostics report unfunded draws. A deterministic funding precheck rejects collateral structures that cannot fund contractual required amounts. Stochastic paths may record the shortfall fraction as a risk output instead.

## Conventions, failures and limitations

Positive collateral cash contributes to collections; funding is an outflow. Contract dates map to deal periods, so the simulation is not an exact event-date cash account. Instrument pools reject reinvestment and statistical-only delinquency/card configurations. Instrument-specific market dependencies and calendars are inherited through the collateral projection path; full closure of public deal dependency reporting is not established by that inheritance.

The observed last-bucket treatment can accelerate collateral flows beyond legal final. Expected hazard overlays do not establish that the underlying instrument schedule and credit exposure are a fully joint model. Opening history replay and future funding schedules require consistency, especially when valuation occurs after closing.

## Alternatives and tradeoffs

Statistical asset rows are supported when detailed contracts are unavailable. Exact event-driven cash-account simulation or explicit sale values at legal final would differ from period bucketing and cannot be inferred from these schedules. The chosen reuse reduces contract duplication but adds responsibility for matching instrument and deal timing.

## Implementation and validation evidence

- [structured_credit/types/collateral.rs](../../../../finstack-quant/valuations/structured-credit/src/structured_credit/types/collateral.rs)
- [structured_credit/pricing/simulation_engine/instrument_flows.rs](../../../../finstack-quant/valuations/structured-credit/src/structured_credit/pricing/simulation_engine/instrument_flows.rs)
- [structured_credit/pricing/simulation_engine/pool_flow_source.rs](../../../../finstack-quant/valuations/structured-credit/src/structured_credit/pricing/simulation_engine/pool_flow_source.rs)
- [structured_credit/pricing/simulation_engine/simulate_period/cash.rs](../../../../finstack-quant/valuations/structured-credit/src/structured_credit/pricing/simulation_engine/simulate_period/cash.rs)

Existing evidence: [structured_credit/unit/instrument_collateral_tests.rs](../../../../finstack-quant/valuations/structured-credit/tests/structured_credit/unit/instrument_collateral_tests.rs); [structured_credit/unit/instrument_pool_tests.rs](../../../../finstack-quant/valuations/structured-credit/tests/structured_credit/unit/instrument_pool_tests.rs); [structured_credit/unit/funding_tests.rs](../../../../finstack-quant/valuations/structured-credit/tests/structured_credit/unit/funding_tests.rs). These are behavior/formula/invariant tests; no test was rerun for this audit. Where only a test location was inspected, no independent reference validation is claimed.

The independent calculations above exercise selected balances and cash timing.
Production qualification of the joint contract, credit and funding paths remains pending.

## Follow-up tracking

Implementation reconciliation and validation work are tracked in
the [structured-credit follow-ups](../FOLLOW_UPS.md#structured-credit).
