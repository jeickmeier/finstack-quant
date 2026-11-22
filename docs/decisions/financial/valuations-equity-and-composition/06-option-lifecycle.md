# VALUATIONS-EQUITY-AND-COMPOSITION-06: Equity option exercise, expiry and settlement state

- **Record:** `VALUATIONS-EQUITY-AND-COMPOSITION-06`.
- **Decision status:** Accepted 2026-11-14 — complete equity option exercise, expiry and settlement package.
- **Audit date/source:** 2026-11-05, `7e82fd03ad98346727769e88dd5ac5d84b56e9fc`.
- **Implementation status:** Observed in the owned paths below. Reconciliation to the accepted contract is pending.
- **Validation status:** Source and primary clearing rules inspected. Independent signed cash, physical delivery, residual risk and horizon arithmetic checked. Venue, runtime and host qualification pending.

## Accepted decision for checked equity option exercise and settlement

1. **Bind the exact contract and exercise rights.** Require the underlying
   and share or index-point basis, settlement currency, positive finite
   strike, finite nonzero signed quantity, explicit exercise style and
   settlement form, and qualified model clock. Apply the actual venue lot
   constraints where they govern. Remove the quantity epsilon floor. Bind
   the exact European expiry cut, American authorized window, or explicit
   ordered Bermudan exercise schedule. Keep trade/exercise cutoffs, payoff
   observation, publication, payment and delivery phases distinct. Use
   authored or effective venue terms with no generic settlement lag.
   [OCC equity option specifications](https://www.theocc.com/clearance-and-settlement/clearing/equity-options-product-specifications),
   [OCC index option specifications](https://www.theocc.com/clearance-and-settlement/clearing/index-options).

2. **Resolve authoritative action before live-model execution.** Use the
   admitted information cut and exact lot to resolve live, expired but
   unresolved, confirmed unexercised, exercised or assigned with outstanding
   obligations, or discharged. Preserve authorized contrary instructions
   and exact disjoint quantities for partial exercise. An exercise outcome
   and payable-amount fixing have separate eligibility. Confirmed
   unexercised and physical exercise outcomes require no dummy spot. A
   confirmed exercised cash claim can await a fixing without retaining live
   optionality. Leave unresolved dependent valuation unavailable. A live
   clock that collapses does not establish a fixing or exercise. For a live
   price, qualify how the selected model maps eligible exercise to the
   actual payoff and settlement terms. Live pricing methods belong to
   [record 07](07-black-tree-and-direct-greeks.md).
   [OCC exercise rules, Rules 805 and 1804](https://www.theocc.com/getcontentasset/9d3854cd-b782-450f-bcf7-33169b0576ce/dfc3d011-8f63-43f6-9ed8-4b444333a1d0/occ_rules.pdf%3B),
   [accepted dispatch and information phases](../valuations-framework/pricing-lifecycle.md).

3. **Value fixed cash on the actual payment basis.** Use the authoritative
   signed whole-lot amount or derive the signed cash from the eligible
   official fixing under the exact contractual payoff, unit scale and
   rounding. Apply side and size once and preserve lawful signed outcomes.
   Use positive-part intrinsic when prescribed. An authorized out-of-the-
   money exercise may create a payable under the selected clearing rules.
   Discount confirmed unpaid cash on the admitted relative payment basis,
   with factor one for an included same-day claim. A past scheduled due date
   establishes neither payment nor cancellation. Require explicit overdue
   treatment and proof of discharge before closing the residual.
   [OCC cash settlement, Rules 1805 and 1806](https://www.theocc.com/getcontentasset/9d3854cd-b782-450f-bcf7-33169b0576ce/dfc3d011-8f63-43f6-9ed8-4b444333a1d0/occ_rules.pdf%3B),
   [accepted dated cash selection](../core-conventions/016-discounting.md).

4. **Value the ordinary remaining exchange as shares and strike cash.**
   For an admitted same-currency single-security delivery, the exercised
   long call receives units and pays strike cash, and the exercised long
   put delivers units and receives strike cash. Reverse direction for
   assignment. Bind exact shares, signed strike cash and independently
   authorized dates and phases. For outstanding stock rights use record
   02's qualified prepaid-share carry under one elected dividend profile.
   Exclude rights retained away from the stock leg and use the applicable
   payment dates when valuing those rights. Keep separately conveyed
   dividends on their own entitled basis. Discount the unpaid strike on
   its actual dated payment source. Permit a negative residual exchange
   value. Track stock and cash performance separately. Hand the resulting
   shares and associated rights to their owner once. Adjusted deliverables,
   due bills and failures require separately qualified treatment.
   [Accepted cash-equity carry](02-spot-and-carry.md#accepted-decision-for-checked-cash-equity-valuation-and-dividend-carry),
   [option exercise and dividend entitlement](https://www.optionseducation.org/referencelibrary/faq/options-exercise).

5. **Make risk follow the live option or the remaining claim.** Fixed cash
   has zero native spot delta, gamma and option vega on the admitted fixed
   basis. It may retain dated discount risk. The qualified linear delivery
   claim has signed share delta on its actual prepaid-share basis and can
   retain funding and dividend risk. Reprice exact active dated sources for
   curve DV01. Local rho and time sensitivities require their own declared
   experiment. Leave option-volatility risk and implied-volatility inversion
   with eligible live optionality. A completed handoff leaves risk with
   the resulting position. [Accepted option sensitivities](../valuations-framework/option-sensitivities.md),
   [accepted direct curve risk](../valuations-framework/finite-difference-rates.md).

6. **Require explicit state for horizon crossings.** Remove automatic
   held-spot exercise and same-day settlement construction. A horizon
   across exercise, fixing or payment requires qualified actual or
   explicitly hypothetical state and the corresponding cash/delivery
   ledger. Use `theta=signed period cash+closing PV−opening PV` on the
   shared holder basis. Preserve actual history, keep projected effects
   labelled and count each cash payment or delivered position once.
   [Accepted horizon framework](../valuations-framework/carry-theta-breakeven.md).

7. **Preserve the resolved claim through reporting and hosts.** One
   checked prepared Rust state governs native/registered valuation, typed
   Python and WASM routes, JSON, schemas and portfolio consumers. Retain
   confirmed cash and stock delivery distinctly with dates, status, source
   and units. An empty residual requires a proved empty scope. Where a
   cash-only export cannot represent the stock or contingent claim,
   return unavailable. Preserve the distinction between option expiry
   and discharge of the last outstanding cash or delivery obligation.
   [Accepted cashflow export](../valuations-framework/cashflow-export.md).

## Observed behavior

### Choice and rationale

Exercise/expiry observations are explicit contract state. Once exercise occurs, every equity-option adapter values the resulting cash or physical delivery rather than rerunning an option model. This preserves the observed fixing independently of subsequent spot moves.

### Scope and input domain

European, American and Bermudan cash/physically settled equity options. Strike is finite and positive; observed exercise spot is positive. Signed quantity is finite with magnitude at least `f64::EPSILON`. Settlement cannot precede exercise; European exercise must occur at expiry, an unexercised observation must be at expiry and exercised Bermudan observations must match the schedule.

### Algorithm

1. Without an observation, pricing on or after expiry errors. Before the observation date, continue live option pricing.
2. On/after observation, an unexercised option or a date strictly after settlement has zero value.
3. An exercised cash option pays the fixed observed intrinsic `max(±(Sobs−K),0)` times the rebased settlement DF and quantity. It remains due on settlement day.
4. Physical exercise becomes `±[prepaid_spot−K Dsettle] × quantity`. Prepaid spot is live spot minus PV of future cash dividends through settlement, or `S exp(-qT)` when no such dividend remains. Physical delivery is not floored at zero after exercise.
5. Lifecycle Greeks reflect this residual claim: cash delta/gamma/vega are zero; cash rho is `−T PV×10^-4`, daily theta `r PV/days_per_year`. Physical delta is signed quantity times prepaid-share carry; rho/theta include strike and dividend carry.
6. Generic theta crossing expiry can synthesize an observation at expiry from the supplied rolled market spot, with same-day settlement and strict in-the-money exercise. That scenario state is distinct from requiring an actual observation in ordinary pricing.

### Selection, controls and conventions

Default option day count ACT/365F, cash settlement, European exercise. Lifecycle resolution occurs before stochastic-model eligibility/calibration checks. Missing live calibration data is therefore unnecessary once the claim is fixed, except required settlement discount/live physical spot data.

### Limitations, failures and alternatives

This contract does not infer historical exercise from today's spot. The synthetic theta expiry assumption is a scenario convention and may differ from actual exercise/payment lag. The American observation does not validate optimality; callers supply the actual exercise action. Modeling settlement automatically from an exchange calendar is an unimplemented alternative.

### Evidence

Inline lifecycle cases in [types.rs](../../../../finstack-quant/valuations/equity/src/equity_option/types.rs) and [tests.rs](../../../../finstack-quant/valuations/equity/src/equity_option/pricing/tests.rs); [test_finance_regressions.rs](../../../../finstack-quant/valuations/equity/tests/equity_option/test_finance_regressions.rs) locate expiry/dividend/exercise regressions. [Hull settlement context](../../../REFERENCES.md#hull-options-futures) is background; no exchange exercise-rule conformance was established.

## Independent arithmetic and qualification

- Under the signed cash rule, two exercised calls with 100 cash per point,
  fixing 98 and strike 100 owe `400` from the long perspective. Discount
  at `.99` for long PV `−396`.
- An exercised long call receives 100 shares with cum spot 98, a retained
  dividend of 2 per share at factor `.99`, and strike 100 payable at factor
  `.995`. The delivered-share PV is `9602` and strike PV is `9950`, for
  remaining value `−348`.
- With opening PV 47 and resolved payoff 50, a close with unpaid cash
  at payment factor `.995` gives theta `2.75`. A close after confirmed
  payment gives theta `3`, with 50 in period cash and zero residual.

Qualify venue exercise and publication states, exact signed clearing
amounts and delivery entitlements, selected risk and horizon scenarios,
and typed and JSON host behavior. Repository runtime suites were unrun.

## Follow-up tracking

Pending decisions, implementation reconciliation and validation work are tracked in
the [equity and composition follow-ups](../FOLLOW_UPS.md#equity-and-composition).

## Owned source files

- [types.rs](../../../../finstack-quant/valuations/equity/src/equity_option/types.rs)
- [parameters.rs](../../../../finstack-quant/valuations/equity/src/equity_option/parameters.rs)
- [inputs.rs](../../../../finstack-quant/valuations/equity/src/equity_option/pricing/inputs.rs)
