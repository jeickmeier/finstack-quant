# MODELS-MONTE-CARLO-20: Autocall observation ordering, coupon memory and redemption

- Decision status: **Accepted — complete checked Monte Carlo autocallable payoff package, 2026-11-11**.
- Implementation status: Existing Rust payoff and dated asset GBM route observed. Exact event binding, shared replay, checked cash handling and caller reconciliation are pending.
- Validation status: Source, selected tests, issuer terms and independent observation, redemption and discount examples reviewed. Runtime, host and dated product qualification are pending.
- Audit date: 2026-11-05. Source revision: `7e82fd03ad98346727769e88dd5ac5d84b56e9fc`.
- Scope and coverage: [slice inventory](README.md), [file ledger](coverage.json).

## Accepted decision for checked Monte Carlo autocallable payoffs

1. **Require immutable single-underlying note terms.** Use one nonempty
   ordered contract schedule, one positive finite strike-set reference
   `S0`, one exact signed Money principal in the settlement currency,
   an explicit memory choice where coupons exist, and one maturity
   redemption variant. Positive principal represents the long position.
   Preserve literal zero after validating the terms. Use finite positive
   underlying levels under the selected equity law and finite nonnegative
   trigger ratios in units of `S0`. Require finite signed coupon
   fractions of principal. Keep only the selected variant's fields.
   Restore through the same checks. Remove parallel mutable term vectors,
   unused mandatory fields and silent economic defaults.
   [Accepted Money](../core-conventions/012-money-decimal-precision.md#accepted-decision-monetary-precision-rounding-and-currency-identity).

2. **Give each observation explicit coupon, call and payment terms.**
   Require one ordered row per contractual fixing, with independently
   selected coupon terms or absence, and a call threshold or absence.
   An absent coupon row leaves memory untouched. An explicit zero
   coupon can release memory. A literal zero call threshold triggers
   for a positive spot. Absent call terms disable calling for that row.
   Specify call eligibility on the last row as on any other row.
   Preserve one shared contractual payment date for coupon and call
   principal on a payable row, and the separately specified maturity
   principal date for an uncalled note. In this dated profile, require
   payable row dates to be nondecreasing and each to lie from its
   observation through maturity payment, inclusive. An observation-only
   row needs no payment date. Different coupon and call dates within
   one row require their own product contract.
   [Filed issuer examples](../../../REFERENCES.md#autocallable-observation-eligibility-and-payment-terms).

3. **Bind the strike-set level and exact observations.** Carry `S0`
   from its qualified contractual fixing even before the first observation.
   Pricing and risk keep it fixed. An owner can establish the fixing at
   an explicit strike-setting event. Today's spot supplies only the
   current model state unless it is that fixing. Bind the live suffix
   and model-coefficient changes to the complete exact stored grid.
   Dated future fixings require strictly positive, distinct ordered
   model times. An explicitly eligible origin observation belongs to
   its qualified event route. Reject a missing knot or clock collapse
   that conflates distinct dates. Check required node and named spot
   before committing any state. Retire tolerance matching, skipped
   observation reuse and end-of-grid catch-up.
   [Accepted grid](../core-numerics/time-grids.md#accepted-decision-checked-simulation-grids-and-exact-event-knots),
   [accepted terminal observations](17-vanilla-and-asian.md#accepted-decision-for-checked-terminal-and-asian-payoffs).

4. **Replay qualified history through the same transition.** Require
   one validated fixing per eligible historical row through the first
   call or the end of the observed prefix. The dated owner selects
   the observation and publication cutoff. A same-day fixing within
   that cutoff must be supplied as observed evidence. Reject ambiguous
   duplicate, missing or unqualified fixings. Keep the recognized
   prefix, downside state, outstanding contingent memory and earned
   cash with payment dates distinct. Derive the live starting state
   from that replay, with no unchecked aggregate seeds or infinite
   minima. Reset each leg to the validated state. Treat hypothetical
   risk-scenario observations under the product's scenario policy.

5. **Process the coupon and memory first.** On each eligible coupon
   row, the coupon test is inclusive `S>=S0*b_coupon`. On a hit,
   earn the current coupon plus all eligible unreleased memory
   when memory is enabled. Make the released amount payable on
   this row's date and reset memory. On a miss, add the current
   fraction only in memory mode. Otherwise it lapses. Signed fractions retain their
   sign through release. A qualifying zero coupon can earn the
   memory amount. No universal ordering applies between the coupon
   and call thresholds. Compare on the declared contract's level
   geometry in checked scale, with exact inclusive equality and
   no tolerance. A dated owner must qualify any term-sheet rounding
   or fixed-level rule before preparing the comparisons.
   [Inclusive touch](../core-conventions/020-barrier-classification.md#accepted-decision-barrier-classification-touch-and-conditional-payment-timing).

6. **Pay one principal on the first eligible call.** After the
   coupon decision, test `S>=S0*b_call` if this row is callable.
   A hit earns exactly one principal on the row payment date,
   keeps all previously earned cash including this row's eligible
   coupon, and closes the note for further observations. Memory
   unreleased at call lapses. No coupon qualifies from the call
   test alone. Future contractual rows supply no new fixings or
   cash after a call. Preserve the engine's predeclared path draw
   layout. Non-observation and post-call nodes require no payoff spot.

7. **Freeze the final fixing and select discrete downside monitoring.**
   For knock-in put redemption, monitor the inclusive down threshold
   at each contractual observation through the last eligible row,
   including the replayed history and the final fixing when uncalled.
   Latch a touch at `S<=S0*b_final`. An off-node dip has no effect.
   At the last row process coupon and any enabled call before
   uncalled redemption. If the note survives, use the spot from that
   final row once. A later maturity payment date supplies no spot,
   volatility checkpoint or new observation. Unreleased memory
   lapses at maturity unless the final coupon test already released it.
   Continuous downside monitoring needs a separate product law.
   [Accepted barrier observation scope](18-barrier-monitoring.md#accepted-decision-for-checked-monte-carlo-barrier-monitoring-and-rebates).

8. **Retain the three exact uncalled redemption ratios.** With final
   performance `x=S_final/S0` and `(z)^+=max(z,0)`, select one:
   - **Capital protection** pays `max(floor,p*min(x,cap))`. Require a
     finite nonnegative floor, finite signed participation `p` and
     finite positive cap. The cap limits the performance input. The
     formula has no additive par amount. The floor can be below par.
   - **Participation** pays `1+p*(min(x,cap)-1)^+`. Require finite
     nonnegative `p` and finite positive cap. A cap at or below one
     leaves par. The cap limits the performance input.
   - **Knock-in put** pays par unless the eligible down barrier
     touched. On touch it pays `max(1-(k-x)^+,0)`, with finite
     positive strike ratio `k` and finite nonnegative barrier
     ratio. The holder loses the capped put loss from principal.
     This variant consumes no performance cap.

   Evaluate caps and support branches in the original scale, even
   if an unbounded performance ratio would overflow. Use the same
   checked redemption calculation for simulated and fully observed
   uncalled notes. The three formulas are library
   contract choices and require product-specific term-sheet mapping.

9. **Value the actual cash legs with one deterministic discount source.**
   Keep coupon and principal as separate entitled cash legs with
   their original payment dates. Prepare coupon fractions through
   the accepted checked scalar conversion and sum memory in checked
   Decimal arithmetic. Apply exact Money principal once to each
   released coupon fraction or redemption ratio. Preserve scaled
   comparisons, the complete requested ratio arithmetic and signed
   cash. Reject unresolved or unrepresentable required results.
   For the live engine, use the maturity payment date `P` as
   the equivalent settlement reference. For each payable amount
   `C_d` use `C_d*D(d)/D(P)`, sum the equivalent amounts in
   checked Money, and apply `D(P)` once through the engine.
   Factors come from one qualified date-based source. Positive
   factors and ratios can exceed one. Maturity principal has
   unit ratio, while the final coupon keeps its own date.
   Require the equivalent amounts and conversions to satisfy
   the accepted Money contract, with no implicit FX or cash rounding.
   [Accepted discounting](../core-conventions/016-discounting.md#accepted-decision-dated-npv-selection-arithmetic-and-continuous-discounting),
   [accepted engine](01-engine-and-estimation.md#accepted-decision-for-checked-monte-carlo-execution-and-estimation).

10. **Resolve known claims and discharge at the dated owner.** Select
    the dated cash perimeter and same-day inclusion explicitly for
    earned coupons, called principal and maturity principal. An
    admitted payable same-day cash leg has factor one. A date or
    call flag supplies no proof of payment. Confirm pending, paid
    or otherwise discharged amounts and qualify any overdue cash.
    When called in history or fully observed without call, value
    the included known cash directly by its actual dates before
    unnecessary spot, vol or simulation. No remaining payable
    amounts yields exact zero in the principal currency. A past
    maturity date alone does not establish that outcome. A live
    simulation requires a nonempty remaining suffix. The completed
    route uses no dummy empty live payoff.
    [Accepted cash settlement](../core-conventions/015-cashflow-semantics.md#accepted-decision-cash-settlement-principal-effects-and-accrual-metadata),
    [dated autocall owner](../valuations-equity-and-composition/13-autocallable.md).

11. **Use the accepted live GBM and estimator profile.** Reconcile
    the existing asset route to explicitly selected ATM-term GBM,
    exact checkpoint and interval laws, and a simulation horizon
    at the last required live fixing. Prepare carry and volatility
    on their qualified model clock. Discount actual payment dates
    on the qualified discount clock. A later payment alone requires
    no simulated extension. Reuse the fixed-budget Philox pricing
    profile, explicit controls and independent estimator counts.
    Replace optional stopping with an explicitly requested post-run
    half-width gate under record 01. Keep model selection, curve
    mapping, risk and numerical price qualification with the dated
    product.
    [Accepted GBM](04-lognormal-diffusions.md#accepted-decision-for-checked-gbm-and-local-volatility-paths),
    [dated GBM owner](../valuations-equity-and-composition/12-path-gbm.md).

12. **Publish complete results and reconcile existing host routes.**
    Final value requires a completed call or every required row
    of an uncalled path. Validate a whole event before publishing
    its updated state or any cash. Optional capture can report
    call/fixing provenance and the actual earned dated legs, including
    coupons pending at the call. Capture leaves draws, state,
    settlement and errors unchanged. Path entitlements are diagnostic
    rather than a dated cash ledger or discharge record. Retain
    Rust-only payoff callbacks. Reconcile existing asset valuation,
    risk, Python/WASM instrument envelopes, docs, schema, parity and
    result diagnostics. Keep settlement and risk scenario inference
    at their owners.
    [Capture owner](28-capture-and-simulation-contracts.md).

13. **Qualify terms, model and product results independently.** Test
    call and coupon equality, last-row call versus uncalled redemption,
    zero and absent coupon, memory release and lapse, all three
    redemption branches, signed terms, exact fixing coverage and
    discrete downside touch. Verify delayed and same-day cash,
    duplicate history, outstanding status, reset and replay,
    Money conversion and scale, no fixing after call, final
    fixing before payment, host parity and capture transparency.
    Qualify the dated mapping against actual issuer terms and
    independently benchmark prices and barrier risk, separately
    from fixed-grid sampling uncertainty.

## Observed behavior and limitations

The [payoff](../../../../finstack-quant/models/src/monte_carlo/payoff/autocallable.rs)
checks parallel vector lengths and ordering, reads spot before checking
event eligibility, and uses a `1e-6` time tolerance. A coarse step can
evaluate several observations at one spot. Coupon release precedes
call. The uncalled final spot freezes when the last observation is
first reached. The payoff tracks a discrete minimum and uses three
redemption formulas. Construction accepts an empty schedule and
running state uses an infinite minimum and zero final-spot sentinel.

The [dated pricer](../../../../finstack-quant/valuations/equity/src/autocallable/pricer.rs)
replays history in a separate loop, can infer `S0` from current
spot before the first observation, and constructs an empty payoff
for the fully observed redemption calculation. Coupon payments and
early principal use the row date, while maturity principal uses
expiry. Historical same-day coupons and called principal are
excluded even though maturity principal can be included on expiry.
The model is built through the payment horizon and checkpoints are
tolerance-deduplicated. The [shared pricing driver](../../../../finstack-quant/valuations/equity/src/piecewise_gbm.rs)
currently forwards width-driven stopping.

## Independent review evidence

| Example | Expected outcome under the proposed contract |
| --- | --- |
| `S0=100`, first coupon `2%` missed, next observation `112.5`, coupon `3%` qualifies, call threshold `1.125`, principal `1000` and row factor `0.97` | Equality calls. Pay `50` of coupon and memory plus `1000` of principal. PV is `1018.50`. A failed coupon test on the call row pays principal only, PV `970`. |
| Same path with qualifying zero coupon on the call row, or with no coupon terms | Zero coupon releases `20` of memory and gives PV `989.40`. Absent coupon gives PV `970`. |
| Same call with qualifying signed coupon `-3%` and `2%` memory | The coupon leg is `-10` and the par call leg is `1000`. Both pay on the row date, for PV `960.30`. |
| Same qualifying observation is the last row, with `p=0.5,cap=1.25` participation and maturity factor `0.90` | With call enabled, PV is `1018.50`. With call disabled, pay the `50` coupon on the row date and `1062.50` at maturity, giving PV `1004.75`. |
| Earlier earned coupon `10` still payable at factor `0.99`, and later memory call of `1050` at factor `0.97` | PV is `1028.40`. The prior earned coupon remains payable after the call. |
| Capital protection `floor=0.9,p=1.25,cap=1.2` | At `x=0.6` redeem `0.9`. At `x=1.6` redeem `1.5`. |
| Same capital-protection terms with `S0=2^-1000` and `S_final=2^1000` | The performance ratio exceeds binary64 range. The capped formula still redeems `1.5`. |
| Participation `p=0.5,cap=1.2` | At `x=0.9` redeem `1`. At `x=1.5` redeem `1.10`. |
| Knock-in put `k=1,b_final=0.75,x=0.95` | Eligible minimum exactly `0.75` redeems `0.95`. With eligible minimum `0.875`, an off-node dip below `0.75` still gives par. At `k=1.75,x=0.50` after touch, redeem zero. |
| Uncalled principal `1000`, final coupon `2%` at factor `0.95`, redemption ratio `1.30` at factor `0.90` | PV is `1189`. Assigning both legs to maturity gives `1188`. If both are due and included today, their combined amount is `1320`. |

Existing tests include fixed-path coupon memory and settlement-gap
examples. The independent cases verify the proposed economic
distinctions. Runtime, host and market-price qualification remain
pending.
[Settlement-gap tests](../../../../finstack-quant/valuations/equity/tests/autocallable/test_settlement_gap.rs),
[issuer examples](../../../REFERENCES.md#autocallable-observation-eligibility-and-payment-terms).

## Follow-up tracking

Adoption questions and validation work are tracked in the
[scope follow-ups](../FOLLOW_UPS.md#monte-carlo-models).
