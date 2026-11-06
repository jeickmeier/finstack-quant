# Equity option exercise, expiry and settlement state

- **Record:** `VALUATIONS-EQUITY-AND-COMPOSITION-06`.
- **Decision status:** Draft — observed implementation; adoption pending.
- **Audit date/source:** 2026-11-05, `7e82fd03ad98346727769e88dd5ac5d84b56e9fc`.
- **Implementation status:** Observed in the owned paths listed below; shared kernels are identified explicitly.
- **Validation status:** Source and linked evidence inspected or located; no numerical tests executed in this audit. Located tests establish the stated local behavior only, not independent accuracy across the domain.

## Choice and rationale

Exercise/expiry observations are explicit contract state. Once exercise occurs, every equity-option adapter values the resulting cash or physical delivery rather than rerunning an option model. This preserves the observed fixing independently of subsequent spot moves.

## Scope and input domain

European, American and Bermudan cash/physically settled equity options. Strike is finite and positive; observed exercise spot is positive. Signed quantity is finite with magnitude at least `f64::EPSILON`. Settlement cannot precede exercise; European exercise must occur at expiry, an unexercised observation must be at expiry and exercised Bermudan observations must match the schedule.

## Algorithm

1. Without an observation, pricing on or after expiry errors. Before the observation date, continue live option pricing.
2. On/after observation, an unexercised option or a date strictly after settlement has zero value.
3. An exercised cash option pays the fixed observed intrinsic `max(±(Sobs−K),0)` times the rebased settlement DF and quantity. It remains due on settlement day.
4. Physical exercise becomes `±[prepaid_spot−K Dsettle] × quantity`. Prepaid spot is live spot minus PV of future cash dividends through settlement, or `S exp(-qT)` when no such dividend remains. Physical delivery is not floored at zero after exercise.
5. Lifecycle Greeks reflect this residual claim: cash delta/gamma/vega are zero; cash rho is `−T PV×10^-4`, daily theta `r PV/days_per_year`. Physical delta is signed quantity times prepaid-share carry; rho/theta include strike and dividend carry.
6. Generic theta crossing expiry can synthesize an observation at expiry from the supplied rolled market spot, with same-day settlement and strict in-the-money exercise. That scenario state is distinct from requiring an actual observation in ordinary pricing.

## Selection, controls and conventions

Default option day count ACT/365F, cash settlement, European exercise. Lifecycle resolution occurs before stochastic-model eligibility/calibration checks. Missing live calibration data is therefore unnecessary once the claim is fixed, except required settlement discount/live physical spot data.

## Limitations, failures and alternatives

This contract does not infer historical exercise from today's spot. The synthetic theta expiry assumption is a scenario convention and may differ from actual exercise/payment lag. The American observation does not validate optimality; callers supply the actual exercise action. Modeling settlement automatically from an exchange calendar is an unimplemented alternative.

## Evidence

Inline lifecycle cases in [types.rs](../../../../finstack-quant/valuations/equity/src/equity_option/types.rs) and [mod.rs](../../../../finstack-quant/valuations/equity/src/equity_option/pricing/mod.rs); [test_finance_regressions.rs](../../../../finstack-quant/valuations/tests/instruments/equity_option/test_finance_regressions.rs) locate expiry/dividend/exercise regressions. [Hull settlement context](../../../REFERENCES.md#hull-options-futures) is background; no exchange exercise-rule conformance was established.

## Follow-up tracking

Pending decisions, implementation reconciliation and validation work are tracked in
the [equity and composition follow-ups](../FOLLOW_UPS.md#equity-and-composition).

## Owned source files

- [types.rs](../../../../finstack-quant/valuations/equity/src/equity_option/types.rs)
- [parameters.rs](../../../../finstack-quant/valuations/equity/src/equity_option/parameters.rs)
- [inputs.rs](../../../../finstack-quant/valuations/equity/src/equity_option/pricing/inputs.rs)
