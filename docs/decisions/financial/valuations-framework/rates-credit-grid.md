# VALUATIONS-FRAMEWORK-05 — Rates-credit calibration targets, recovery weights, and floating node coupons

- Decision status: Draft — observed implementation; adoption pending.
- Implementation status: Observed in the source paths cited below.
- Validation status: Evidence inspected/located, not executed; no runtime checks or external accuracy comparison performed.
- Audit date / source revision: 2026-11-05 / `7e82fd03ad98346727769e88dd5ac5d84b56e9fc`.

## Choice and input domain

The valuation adapter maps dated discount/survival curves onto the two-factor lattice's uniform time coordinate and maps floating resets/payments onto that grid. It also computes continuous fractional-recovery-of-par interval weights. These are separate from the lattice transition/backward-induction kernel in the models crate. The implementation's date-based targets preserve different curve bases/day counts rather than evaluating both curves at one assumed year fraction.

## Algorithm

For `steps=n`, date position at slice `k` is `k × calendar_days(origin,horizon_date)/n`. Query the lower and upper integer dates and log-interpolate each positive discount factor and conditional survival: `x=exp((1−w)ln x_lower+w ln x_upper)`. Survival is `S(date)/S(origin)`; nonpositive/nonfinite origin survival fails. Force slice zero to `(DF,S)=(1,1)`. Time values themselves are `k × horizon/n`; the caller supplies the horizon clock.

The daily bond adapter chooses `substeps_per_day=max(ceil(minimum_steps/days),1)` and total steps `days × substeps_per_day`, with horizon `days/365`. Thus the requested step count is a minimum, not the actual number of steps; every calendar date lands on a grid slice. Zero steps, nonpositive horizon/date span and overflow fail.

For an interval discount ratio `d` and survival ratio `s`, let `H=−ln(s)` and `x=−ln(ds)`. Continuous FRP weight is `H(1−exp(−x))/x`. At `|x|<1e−8`, use `H(1−x/2+x²/6)`; zero hazard returns zero. Discount must be positive finite, survival in `(0,1+1e−12]`, with the tiny excess above one clamped to one. This is the constant force-of-interest/hazard integral over an interval; callers multiply by recovery/notional and their start-state weight.

Floating node descriptors include only `FloatReset` flows with future reset, accrual metadata, a projected index rate and positive finite accrual. They snap accrual start and payment time upward to the next slice. A reset/payment collapsing to one slice errors and asks for a finer grid. Set `base_discount_forward=(DF(reset_slice)/DF(payment_slice)−1)/τ`, `timing_scale=DF(actual_payment_time)/DF(payment_slice)`, and take notional at the reset slice. Preserve the separately projected base index rate to retain multicurve basis. An explicit flag strips index floor/cap constraints; PIK detection independently identifies future PIK flows for caller gating.

## Selection, controls, and limitations

`resolve_rates_credit_config` reads HW rate volatility/reversion, hazard volatility/reversion and correlation from model overrides; each omitted scalar defaults to zero. It rejects a volatility schedule, Black–Derman–Toy/Black–Karasinski inputs, or an implied-vol quote used in place of HW volatility. Mean reversions obey the lattice's `KAPPA_MAX`; nonzero correlation requires both volatilities positive. This deterministic default is an actual fallback, not evidence that zero volatility was calibrated or approved.

Calibration target validation checks positivity, not independently monotonic survival; the downstream lattice owns additional feasibility checks. Skipping descriptors with absent projected rates or invalid accrual does not itself validate a complete floating schedule. The descriptor timing uses accrual start after using reset date only to decide whether the reset is future.

## Alternatives and evidence

A date-aligned irregular lattice, analytic discount-survival integration, or finer grids are alternatives, not implementations selected by this adapter.

- [rates_credit.rs](../../../../finstack-quant/valuations/core/src/instruments/pricing/rates_credit.rs): date-grid targets and FRP weight; inline tests provide implementation/invariant evidence.
- [floating_reset_descriptors.rs](../../../../finstack-quant/valuations/core/src/instruments/pricing/floating_reset_descriptors.rs): node coupon construction and grid failure.
- [pricing_overrides.rs](../../../../finstack-quant/valuations/core/src/instruments/pricing_overrides.rs): rates-credit configuration and incompatible-input checks.
- [Duffie–Singleton](../../../REFERENCES.md#duffie-singleton-1999) and [Hull–White numerical procedures](../../../REFERENCES.md#hull-white-1994-numerical-procedures) provide general model context; they do not validate this adapter's interpolation or zero-parameter defaults.

## Follow-up tracking

Adoption, implementation and validation work is tracked in the
[scope follow-ups](../FOLLOW_UPS.md#shared-valuation-framework).
