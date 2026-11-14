# VALUATIONS-BONDS-05: MOIC/XIRR return floors as issuer exercise-price constraints

**Decision status:** Accepted — complete checked issue-basis return floor package, 2026-11-14.
**Implementation status:** Runtime and caller reconciliation pending. Observed behavior is recorded below.
**Validation status:** Source, filed terms and independent arithmetic reviewed. Runtime, contract, path replay and host qualification pending.
**Audit date:** 2026-11-05. **Source revision:** `7e82fd03ad98346727769e88dd5ac5d84b56e9fc`.

## Accepted decision for checked issue-basis return floors

1. **Apply one expressly elected early issuer-call constraint.**
   Elect either a finite positive MOIC multiple `m` or a finite
   annual effective XIRR target `r>−1` in decimal rate units.
   Apply it only to independently authorized full early issuer
   calls with exact eligible exercise dates and phases. Require
   the call terms even if the floor is authored at issuance.
   Require explicit Full, inclusive From or inclusive Between
   protection. Full covers `issue<E<maturity` on civil dates,
   subject to the actual call schedule. Intersect From or
   Between with that strict interior and the exact call schedule,
   eligibility and valuation cut. Allow an authored one-day
   Between window and reject reversed bounds. A well-formed
   floor with no remaining eligible overlap is inactive. A term
   with no separately authorized issuer call is incomplete.
   Remove floor-generated opportunities and endpoint, cash-date
   or month-end sampling. An expressly authorized call can be
   authored on a date for which the floor applies. Puts,
   maturity, default, acceleration, mandatory prepayments and
   partial or deferred redemptions have separate payoff contracts.
   The floor can neither supply a missing exercise right for model
   routing nor override a resolved model.
   [Accepted model selection](03-pv-models.md#accepted-decision-for-checked-bond-pv-selection-and-fractional-recovery-of-par)
   and [accepted exercise dates and phases](04-rates-tree-exercise.md#accepted-decision-for-checked-bond-rates-trees-and-exercise).

2. **Anchor protection to one explicit issue investment.**
   Resolve the one positive same-currency investment `V0` at
   issue using an explicit election of initial face amount,
   positive percentage of initial issued face or a positive
   exact Money amount in the bond currency. Percentage inputs
   are in percent of face, so 98 means 0.98 times initial face.
   Preserve original face, issue date and resolved investment
   independently of current outstanding, current-holder
   settlement price and market yield. Use one whole contractual
   claim funded once. Multi-draw terms, later funding and
   separately allocated partial tranches need their own return
   law. Treat original issue discount through the explicitly
   resolved investment when authorized by the terms. This
   post-issue cash profile supplies no separate issue-day credit.
   Remove the implicit par and Full protection choices and the
   shortcuts that rely on them. Require one validated authored
   spec at the Rust and host construction boundaries.
   [Accepted Money](../core-conventions/012-money-decimal-precision.md)
   and [bond schedules](01-schedule-and-holder-cashflows.md#accepted-decision-for-checked-bond-schedules-and-holder-cash).

3. **Credit only the elected paid-through cash.** For this gross
   issue-basis profile credit strictly positive cash coupons
   and ordinary scheduled amortization on their actual cash
   dates after issue, through the eligible exercise cash phase.
   Determine positivity at independently payable claim level.
   Reconcile split components of a single coupon by original
   parent, entitlement and payment identity before testing
   positivity. Keep distinct co-dated legal receipts distinct.
   Credit actual amortization cash and replay its separately dated
   principal effect. Credit co-dated cash only when the terms
   give it priority through that phase and it is paid independently
   of hold or call, including explicitly concurrent unconditional
   payment as admitted by the cut. Exclude outstanding unpaid
   claims, fees and charges, negative cash, unscheduled prepayment,
   default and recovery, PIK capitalization, and the terminal or
   alternative redemption being evaluated. A separately owed
   later receipt retains its own valuation entitlement. The
   exercise-payment credit is the eventual dirty strike itself.
   These eligibility filters affect only the contractual floor.
   Continue to price actual signed holder cash under its own rules.
   [Cash and principal semantics](../core-conventions/015-cashflow-semantics.md),
   [filed cash interest and exclusion terms](https://www.sec.gov/Archives/edgar/data/1871149/000119312524226945/d855258dex102.htm)
   and [filed terms with a different principal and PIK basis](https://www.sec.gov/Archives/edgar/data/1655210/000119312525115042/d917100dex101.htm).

4. **Freeze covered issue-to-cut history.** Require the dated
   authoritative settled ledger, or exact reconstruction from
   authoritative settled cash and qualified as-known event and
   fixing history. Preserve actual payment dates, amounts, currency,
   legal payment and original coupon identities, settlement status,
   coverage and the applicable phase at the cut. Validate historical
   balance, PIK, fixing and accrued state needed for pricing or
   replay. Empty receipts require documented coverage. Current curves
   and the secondary buyer's entitlement cannot reconstruct
   unidentified past receipts. The same paid coupon can count on
   the contractual issue basis even if the current holder never
   received it. Keep history fixed for repricing, OAS and market
   shocks. Revisions require a newly qualified as-known snapshot.
   After validating the terms, omit floor-specific history when
   the floor is proved inactive. Reject missing or conflicting
   active history. Preserve the original legal call terms and
   qualified state for an irrevocable prior election.
   [Information and cash entitlement](02-settlement-and-clocks.md#accepted-decision-for-checked-bond-settlement-quote-carry-and-clocks)
   and [filed actual-receipts XIRR terms](https://www.sec.gov/Archives/edgar/data/1500435/000150043526000003/ex103-amendno2farallonagre.htm).

5. **Calculate the required dirty payment directly.** At an
   admitted exercise date `E`, let `C_i` be the credited
   distributions, with `τ_i=(actual_days(issue,T_i))/365` and
   `τ_E=(actual_days(issue,E))/365`. MOIC requires
   `R=m*V0−Σ C_i`. XIRR requires
   `R=(V0−Σ C_i*exp(−g*τ_i))*exp(g*τ_E)`, where `g=log1p(r)`.
   XIRR uses fixed ACT/365F and requires no root solve to
   prepare redemption. Admit exact zero and valid negative
   target rates. Use checked monetary representations, ordered
   accurate accumulation and stable scaled or log arithmetic
   for discounting and the residual. Compare in a usable common
   scale when the target is already covered or a detached
   exponential is unrepresentable. A nonbinding shortfall requires
   no standalone cash result. Reject an unrepresentable required
   payable amount. Apply any expressly selected contractual cash
   rounding at its designated payment boundary, once. After that
   boundary a protected remittance must still satisfy the elected
   payable constraints. If the selected quantum and rule cannot
   meet them, return an error. A published computed issue-basis
   XIRR remains subject to the separate core return result contract.
   [Accepted IRR and XIRR](../core-conventions/017-irr.md)
   and [365-day XIRR timing](https://support.microsoft.com/en-us/excel/functions/xirr-function).

6. **Raise each authorized dirty call amount once.** At the
   exact exercise phase use outstanding `N` after senior
   economic changes and before replacement of ordinary final
   principal. Capitalized PIK affects `N` once. Pending PIK
   enters payable accrued only when the terms cash-settle that
   amount at exercise. Use actual payable accrued `A` under
   the right and coupon entitlement. A vanilla clean percentage
   first produces `N*p/100+A`. A make-whole first produces the
   full payable strike under its own admitted benchmark law.
   If `C` is the resulting dirty call amount, use `max(C,R)`.
   Include `N+A` in the maximum only when an independent
   contractual par minimum applies. The floor adds no par
   covenant by itself. If an admitted view needs clean proceeds,
   subtract the same payable `A` once. Compare alternatives
   on the full dirty basis. The issuer minimum across qualified
   calls, holder put priority and decision-only call friction
   follow the exercise owner after floor application. Preserve
   the other dated holder claims. A proved extinguished principal
   position supplies no call strike. Absent, negative or
   inconsistent required balance is an error.
   [Accepted rates-only strike and entitlement rules](04-rates-tree-exercise.md#accepted-decision-for-checked-bond-rates-trees-and-exercise).

7. **Use one floor calculation in deterministic and pathwise pricing.**
   Prepare a dated effective strike only when the selected
   pricing basis resolves all required state deterministically,
   with eligible cash, projected amounts where admitted, accrued
   and stock at the exact decision phase. Keep that output as
   derived price preparation with the original terms, eligible
   dates and state provenance. Use the resolved model and claim
   state to decide eligibility. In joint rates-credit pricing, seed
   a live floor from the same frozen history and credit eligible
   receipts only as realized through the current simulated decision
   phase. Carry the chronological state through checkpoints and
   actual path replay. The floor strike uses no future path
   realization, survival weights, pricing OAS or risk-free
   discounting in its protected cash arithmetic. Use state and
   pending accrued available at exercise before continuation
   comparison. Require deterministic reduction to agree when
   both routes receive economically identical data. Apply selected
   model, coupon and settlement capability rules and propagate bad
   state without substituting a deterministic or alternate model.
   [Joint payoff replay](06-joint-coupon-replay.md)
   and [joint exercise policy](07-lsmc-policy.md).

8. **Carry the terms honestly into callers, outputs and restored state.**
   Workout, spread and option-risk callers reuse this law and
   the same qualified terms and history. Require an expressly
   qualified projected workout for an unresolved future
   contractual floor, or report that view as unavailable.
   Record 11 owns workout candidate selection and record 20 owns
   published lifetime-return results. A gross issue-basis
   contractual hurdle covers the eligible receipts and full
   redemption it defines. Secondary acquisition returns,
   all-cash signed returns and unprotected exits follow their
   own laws. Report issue basis, eligible window and exercise
   provenance, selected target and state basis wherever reporting
   depends on the floor. Reconcile Rust preparation, typed
   Python and WASM APIs, builders, schema and atomic restoration.
   Restore no invented defaults or inferred settlement history.
   Fail requested active calculations with unresolved inputs.
   An independently supported quote mark can leave inactive
   model-only work unresolved under record 03.
   [Workout yield](11-workout-yield.md),
   [spread quotes](12-spread-quotes.md),
   [option risk](18-embedded-option-risk.md) and
   [lifetime returns](20-lifetime-returns.md).

## Observed behavior

### Choice and domain

A return floor creates or raises issuer-call redemption prices over an interior-life protection window. It is not a guaranteed minimum held-to-maturity return. Default issue cost is par; callers can supply a positive finite currency amount matching notional or percentage of initial par. MOIC must be positive; XIRR must exceed −1. Full protection means issue+1 day through maturity−1 day; From/Between windows are intersected with that life. The current XIRR clock is fixed ACT/365F.

### Algorithm and ordering

Using issue date `t0` and issue investment `V0`, collect positive Fixed/FloatReset/Stub coupons and Amortization distributions after issue, including cash on the exercise day. Ignore PIK as a distribution; include it in outstanding. For candidate `t`, required dirty redemption is `m·V0−Σ C_i` for MOIC, or `(V0−Σ C_i/(1+r)^τ_i)(1+r)^τ_t` for XIRR. Subtract accrued once, divide by after-event outstanding and floor the clean strike at 100% of outstanding. No extra right is created when outstanding is zero.

Deterministic lowering uses the tree candidate rule (endpoints, cash events, month ends), plus contractual call endpoints. At every contractual call date take the greater of its clean strike and the floor, preserving make-whole terms. Add floor-only call dates elsewhere in the protected window and preserve holder puts. `effective_for_pricing` replaces `return_floor` with this schedule so deterministic trees and workout calculations consume the same lowered rights.

Stochastic rates-credit keeps the floor as path state: it accumulates historical and simulated positive distributions, target-discounted distributions, PIK/outstanding and accrued cash/PIK. Required clean redemption is `max(required_dirty−accrued,outstanding,0)`; contractual calls and make-whole values are raised to that floor, then accrued is added. This prevents using future realized coupons to decide the floor today.

### Limitations, alternatives and located evidence

Negative coupon cash, fees and prepayments are not counted among protected distributions by these filters. The floor imposes issuer exercisability where no contractual call previously exists. Historical distributions are reconstructed from the schedule/market rather than supplied as investor cash history. These are material adoption choices, especially for seasoned PIK/FRNs and unusual cashflow kinds. A maturity guarantee, investor-specific acquisition return or constraint only on existing calls is a different, unimplemented contract.

Owning code: [types/validation](../../../../finstack-quant/valuations/fixed-income/src/bond/types/return_floor.rs), [lowering](../../../../finstack-quant/valuations/fixed-income/src/bond/pricing/return_floor.rs), [stochastic state formula](../../../../finstack-quant/valuations/fixed-income/src/bond/pricing/tree/lsmc/state.rs), [pathwise exercise](../../../../finstack-quant/valuations/fixed-income/src/bond/pricing/tree/lsmc/replay.rs). Inline return-floor tests inspect amortizing balances, date windows, accrued and merged rights; LSMC test `return_floor_subtracts_partial_accrual_before_adding_it_once` provides an explicit arithmetic invariant. They are not legal-contract validation. [Fixed income reference](../../../REFERENCES.md#fabozzi-fixed-income-handbook) is general background; no product-specific source accepting this floor contract was located.

Host and wire owners include the [Python bond binding](../../../../finstack-quant-py/src/bindings/valuations/instruments.rs),
[WASM bond binding](../../../../finstack-quant-wasm/src/api/valuations/typed/fixed_income.rs)
and [bond schema](../../../../finstack-quant/valuations/schemas/instruments/1/fixed_income/bond.schema.json).

## Independent arithmetic and qualification

- From an issue investment of 98, with credits of 7 at one
  ACT/365F year and 14 at two years just before exercise, a
  1.25× MOIC floor requires dirty redemption 101.50.
  A 10% XIRR instead requires 96.88 and gives zero NPV at the
  target. With remaining principal 80 and payable accrued 2,
  the MOIC clean equivalent is 99.50. A qualified make-whole
  already paying 102 wins without adding accrued a second time.
- With initial investment 100, a day-60 credit of 10 and
  a day-366 credit of 5 paid before exercise, a 12% fixed
  ACT/365F hurdle gives dirty redemption of about 96.03808389.
  A leap day stays in the actual day count and the denominator
  stays 365.
- For a 1.25× floor on issue investment 100, 12 historical
  credits and 20 co-dated amortization, a qualifying coupon of
  8 paid before exercise leaves dirty shortfall 85. If the
  coupon remains payable as an independent later claim, the
  decision-phase shortfall is 93. The later claim can still
  have holder value. A separately payable signed charge of 5
  excluded from the first floor stream would lower net realized
  MOIC to 1.20× even when gross recognized proceeds reach 1.25×.
- If remaining principal is 80, payable accrued 2 and the
  qualified contractual clean amount 75, a return shortfall
  of 76 leaves redemption at 77 without an authored par
  covenant. An applicable par covenant raises it to 82.

Qualify a dormant floor, negative and zero XIRR targets,
leap-year and same-date cash phases, contractual rounding,
split coupons, delayed amortization, PIK cash or principal
treatment, make-whole composition and absent or corrected
history. Reconcile exact authorized dates, deterministic and
pathwise replay, workout and risk callers, and host state.

## Follow-up tracking

Adoption, reconciliation and validation work is tracked in the
[scoped follow-up register](../FOLLOW_UPS.md#bond-valuations).
