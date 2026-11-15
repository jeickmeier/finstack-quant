# VALUATIONS-STRUCTURED-CREDIT-13: Step-down, shifting-interest and early-amortization regimes

- **Decision status:** Accepted 2026-11-15 — complete principal allocation, targeted OC, controlled accumulation, dated triggers and early-amortization package.
- **Implementation status:** Observed in the owning paths below. Reconciliation to the accepted contract is pending.
- **Validation status:** Independent shifting allocation, target OC, accumulation, and dated trigger calculations checked. Production pricing, transaction terms and host qualification remain pending.
- **Audit date:** 2026-11-05.
- **Source revision:** `7e82fd03ad98346727769e88dd5ac5d84b56e9fc`; runtime source unchanged by this audit.

## Accepted decision for principal regimes and early amortization

1. **Make step-down reversible at authorized assessments.** Activate on
   or after the effective date when all elected predicates pass.
   Maximum loss and delinquency use inclusive upper tests. Minimum
   OC and credit enhancement use inclusive lower tests. Use named
   qualified dated measures and honor applicable coverage hysteresis.
   A later breach restores the authored protective allocation.
   An expressly elected date-only rule is valid. Missing required
   inputs error. Weight covered step-down payments by positive live
   eligible note par.
   [Accepted coverage tests](12-coverage-and-cures.md#accepted-decision-for-coverage-tests-and-cure-diversion).

2. **Allocate scheduled and unscheduled principal separately.** Select
   the latest effective shifting-interest step using explicit dates
   or checked calendar-month offsets from original closing. Require
   stated protection before the first step and during a breach.
   For senior live eligible par share `s`, scheduled principal uses
   `s`. Unscheduled uses `s+z*(1-s)` under `ShiftOfSubordinate`, or
   `z` under `SeniorShare`. Allocate and round each source separately
   against shared remaining caps. Elect treatment of recoveries and
   of overflow from capped or retired senior notes.
   [Filed shifting-interest formula](https://www.sec.gov/Archives/edgar/data/1372936/000089968106000517/thornburg-fwp_082206.htm).

3. **Set target OC on qualified collateral and debt.** Use the larger
   of current fraction times eligible current collateral `B` and the
   floor fraction times fixed original par. With target `T` and remaining
   tested debt `D`, when the action leaves `B` unchanged the required
   paydown is `max(0,D-max(B-T,0))`. Count each debt balance once after
   prior payments. When funding consumes value already counted in `B`,
   recompute the actual post-action base and target. Apply authorized
   funding, priority, caps and residual routing.
   [Filed target OC example](https://www.sec.gov/Archives/edgar/data/1402756/000136231007001255/c70730e424b5.htm).

4. **Preserve the controlled-accumulation account.** Restore the
   authorized opening balance. From the start inclusive to the bullet
   exclusive, deposit eligible principal up to the cap and route
   overflow as specified. Release once at the bullet or effective
   acceleration, including when both occur together. Deposits transfer
   cash and actual note payments retire par. Preserve unpaid
   mandatory principal as a claim.
   [Accepted card accumulation](07-card-master-trust.md#accepted-decision-for-card-master-trust-allocation).

5. **Use dated history and latch permanent early amortization.** For
   the elected three-month profile use three consecutive completed
   calendar months at the authorized assessment, regardless of
   distribution frequency, with strict breach below the excess-spread
   floor. The elected cumulative realized-loss test breaches at or
   above its threshold on qualified fixed original par. Distinguish a
   startup test that is not yet due from missing required history.
   Restore sourced history and effective date and reason in seasoned
   state. Only authorized permanent acceleration establishes the
   term-out anchor.
   [Filed monthly trigger example](https://www.sec.gov/Archives/edgar/data/1174821/000000000015036220/filename1.pdf).

6. **Declare rule composition and preserve mandatory obligations.**
   Honor mandatory principal, binding obligations and legal final.
   Effective acceleration controls optional principal, stops new
   recycling and accumulation, and releases held cash under the
   accelerated rules. During accumulation apply its restrictions
   to discretionary principal. Elect one base allocation per cash
   stream and phase and reject overlapping conflicting step-down and
   shifting rules. Apply targeted OC only to designated tiers. Use
   the same checked Rust state, cash tags, allocation and diagnostics
   in existing Python and WASM interfaces.

## Independent calculations

- One scheduled and one unscheduled unit, senior share `s=0.5`
  and shifted subordinate share `z=0.5` give separate senior cash
  quotas of 0.50 and 0.75 units. With senior-first ties, separate
  settlement gives senior/subordinate 2/0. Pooling before rounding
  would give 1/1.
- Current eligible collateral 100, original 200, current fraction 10%
  and floor fraction 8% give target 16. Against debt 95, fixed-base
  paydown is 11. After prior debt retirement of 2, remaining need
  is 9. Paying 11 from cash already counted as collateral would
  leave collateral 89 and debt 84, for buffer 5 against target 16.
  Accumulation opening 15, cap 25 and collections 30 capture 10,
  route overflow 20 and release 25 once at a co-dated bullet and
  acceleration.
- Completed January through March spreads of +2%, −1% and −2%
  average −0.333333% and breach a zero floor at the authorized
  April 5 assessment. April spread +8% raises the next window to
  +1.666667%, with the prior early-amortization state still in force.
  A realized loss of 5 on qualified original par of 100 also
  meets an elected 5% loss threshold.

## Observed implementation and scope

The observed engine resolves the period waterfall using projected
collateral and note state. The following sections describe audited
behavior. The accepted contract above governs reconciliation.

## Algorithm and controls

Step-down becomes active on/after the specified date only if every supplied loss, OC, credit-enhancement and delinquency predicate passes. It switches principal to pro-rata current-balance weights; a later failed predicate can reverse it. Shifting-interest chooses the most recent dated schedule step measured in months from closing. In the default `ShiftOfSubordinate` mode, scheduled principal stays at the senior's pro-rata share, while the schedule shifts a fraction of subordinate unscheduled principal to senior. `SeniorShare` directly specifies the senior unscheduled share. The blend uses the period's unscheduled-principal fraction, clamped to [0,1]; remaining principal weights go to surviving subordinate balances. Trigger failure restores protective allocation.

Target OC computes `target = max(fraction_current × current_pool, floor_fraction_original × original_pool)`. Required note paydown is `max(0, notes - max(pool - target, 0))`, allocated in note priority and implemented as principal target balances. Controlled accumulation caps principal claims during the accumulation window and releases cash at the bullet.

Early amortization fires when cumulative realized loss/original pool reaches the loss threshold or the trailing realized annualized excess spread falls below its floor. It permanently ends revolving/reinvestment, even if later metrics improve. Despite the field name `min_excess_spread_3m`, the code averages the last three **payment periods**; quarterly deals therefore use a different calendar horizon from monthly deals.

## Conventions, limitations and alternatives

Loss/CE/delinquency limits and shifting shares are decimal fractions; OC is a ratio. Date boundaries and inclusive comparisons matter. Combining rules is order-sensitive: weights, target balances and lockouts modify the same waterfall. No rule interprets a legal covenant automatically. Pure sequential, unconditionally pro-rata and omitted optional regimes are supported alternatives; an event-driven daily covenant monitor is not implemented here.

## Implementation and validation evidence

- [structured_credit/pricing/resolve.rs](../../../../finstack-quant/valuations/structured-credit/src/structured_credit/pricing/resolve.rs)
- [structured_credit/pricing/simulation_engine/simulate_period/losses.rs](../../../../finstack-quant/valuations/structured-credit/src/structured_credit/pricing/simulation_engine/simulate_period/losses.rs)
- [structured_credit/pricing/simulation_engine/state.rs](../../../../finstack-quant/valuations/structured-credit/src/structured_credit/pricing/simulation_engine/state.rs)
- [structured_credit/types/waterfall/rules.rs](../../../../finstack-quant/valuations/structured-credit/src/structured_credit/types/waterfall/rules.rs)

Existing evidence: [structured_credit/unit/shifting_interest_tests.rs](../../../../finstack-quant/valuations/structured-credit/tests/structured_credit/unit/shifting_interest_tests.rs); [structured_credit/unit/target_oc_tests.rs](../../../../finstack-quant/valuations/structured-credit/tests/structured_credit/unit/target_oc_tests.rs); [structured_credit/unit/reserve_rules_tests.rs](../../../../finstack-quant/valuations/structured-credit/tests/structured_credit/unit/reserve_rules_tests.rs); [structured_credit/unit/card_master_trust_tests.rs](../../../../finstack-quant/valuations/structured-credit/tests/structured_credit/unit/card_master_trust_tests.rs). These are behavior/formula/invariant tests; no test was rerun for this audit. Where only a test location was inspected, no independent reference validation is claimed.

## Follow-up tracking

Implementation reconciliation and validation work are tracked in
the [structured-credit follow-ups](../FOLLOW_UPS.md#structured-credit).
