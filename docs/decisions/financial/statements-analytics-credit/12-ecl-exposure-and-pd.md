# STATEMENTS-ANALYTICS-CREDIT-12: Exposure schedules, raw cumulative PD and conditional default

- Decision status: **Accepted for the full exposure and cumulative default probability contract (2026-11-15)**.
- Implementation status: **Reconciliation pending**.
- Validation status: **Existing evidence mapped. Independent qualification pending**.
- Audit date: 2026-11-05.
- Source revision: `7e82fd03ad98346727769e88dd5ac5d84b56e9fc`.

## Accepted decision for exposure and cumulative default inputs

1. Explicitly elect supplied total exposure at default (EAD) or drawn exposure plus undrawn amount multiplied by an explicit credit conversion factor (CCF) in `[0,1]`. Record the reporting date, currency, scale and maturity basis. Anchor a supplied schedule at time zero to the reported amount on its declared basis. Validate the knots, interpolate linearly and require horizon coverage or an explicit tail rule. [Monetary precision and currency identity](../core-conventions/012-money-decimal-precision.md#accepted-decision-monetary-precision-rounding-and-currency-identity).
2. Use sourced loss given default (LGD) in `[0,1]`. Declare whether maturity is contractual or behavioral and require it to be in `[0,100]` years. When discounting, declare the annual effective rate and clock. Require a finite rate above −100% and checked discount arithmetic. [Financial clocks and date boundaries](../conventions/0045-financial-clocks-and-date-boundaries.md#decision).
3. Record the cumulative default probability (PD) source, snapshot date, time clock and exact rating key. Use an explicit `(0,0)` anchor for raw knots, with strictly increasing times and nondecreasing cumulative PD in `[0,1]`. Interpolate linearly between knots and require coverage or a stated tail rule. Reject unknown rating keys and invalid queries.
4. For a cumulative curve `F`, the default mass for `(a,b]` is `F(b)−F(a)`. When survival at `a` is positive, conditional default probability is `(F(b)−F(a))/(1−F(a))`. Once survival is zero, the interval mass is zero and the conditional probability is inapplicable. Apply the same validation to custom PD sources.

For example, USD 100 drawn plus USD 40 undrawn at a 50% CCF gives EAD of USD 120. If cumulative PD moves from 2% to 5%, the interval default mass is 3 percentage points and the conditional probability is approximately 3.061%.

## Audited choice and rationale

ECL inputs use explicit decimal probabilities and an interpolated drawn-exposure schedule plus constant converted undrawn commitment. A cumulative-PD trait separates source construction from staging and measurement; raw curves choose linear interpolation of cumulative probability with flat tails.

## Audited scope and input domain

`Exposure` carries nonnegative finite drawn EAD and undrawn amounts, CCF and LGD in [0,1], finite nonnegative annual EIR and remaining maturity in [0,100] years. All monetary scalars share a caller-managed base currency. Optional EAD schedule times must be finite, nonnegative and strictly increasing, with nonnegative finite balances; a present schedule cannot be empty. Staging counters/default flags and rating labels are distinct from these measurement inputs.

## Audited algorithm

1. Validate exposure amounts and schedule at measurement entry. Drawn EAD at time t is the linear/flat interpolation of the schedule when present, otherwise constant `ead`. The schedule need not start at zero; flat left extrapolation uses its first balance. Add constant `undrawn*ccf` through the models EAD calculator. Schedule balances replace drawn EAD, not total converted exposure.
2. `RawPdCurve::new` and validating deserialization require at least two knots, an exact (0,0) first knot, strictly increasing finite times and nondecreasing cumulative PD in [0,1]. `anchor_knots` merely prepends (0,0) when the first supplied time is positive; the constructor still performs validation.
3. Raw cumulative PD is linearly interpolated between knots and flat outside. A raw curve requires its rating string to match the requested label. `RatingPdMap` instead dispatches by map key and interpolates the stored curve irrespective of its internal label; absent keys error. An empty map is valid but every lookup fails.
4. Default conditional marginal PD is `1−S(t2)/S(t1)`, with S=1−F. Validate S(t1) in [0,1] and S(t2) in [0,S(t1)]. When survival at t1 is zero, return zero additional default probability, preserving the absorbing state.
5. `contains_rating` defaults to a successful cumulative lookup at t=0; raw/map sources override it with membership checks. Custom sources can implement their own cumulative and marginal logic, so measurement entry points independently check requested probabilities.

## Audited selection and controls

Default CCF is 0.75 even though the type supports arbitrary [0,1] values; the amount remains unused when undrawn is zero. The audited source labels the default Basel IRB for revolving facilities. Rating labels are exact strings. Convenience schedule requests insert the zero anchor, while direct `RawPdCurve` construction requires it explicitly.

## Audited conventions

Times are years from reporting date for measurement curves and from initial recognition for an origination snapshot. PDs, LGD and CCF are fractions; EIR is an annual effective decimal rate as used by `(1+eir)^−t`. Exposure has no currency identifier or automatic FX conversion. Segments are labels only in this scope, not an aggregation algorithm.

## Audited limitations, failure and fallback

Flat cumulative-PD extrapolation implies zero additional defaults after the last knot, which is a material tail choice. A scheduled drawn balance can differ from `ead` at time zero. Negative EIR is rejected even when economically possible. Raw lookup does not separately validate the query time; engine callers provide controlled times. No undrawn schedule, prepayment process or joint PD/EAD dynamics is inferred.

## Audited alternatives and tradeoffs

Piecewise-flat hazard produces different interpolation/tails; parametric or transition-based sources can implement the trait. A total-EAD schedule or scheduled commitments would suit other facilities but needs unambiguous drawn-versus-total semantics. A typed currency envelope would strengthen portfolio aggregation safety.

## Evidence

[types.rs](../../../../finstack-quant/statements-analytics/src/analysis/ecl/types.rs) owns exposure/curve contracts; [models EAD](../../../../finstack-quant/models/src/credit/lgd/ead.rs) supplies converted commitment arithmetic. Inline tests locate validating deserialization, schedule/maturity bounds, CCF scaling, map lookup, interpolation and absorbing-default marginal PD. These are input/invariant and equation evidence, not empirical PD/CCF calibration.

The live raw and rating-map cumulative PD lookups reject nonfinite query times. Reconciliation of the broader accepted query and tail policy remains pending. [Live PD sources](../../../../finstack-quant/statements-analytics/src/analysis/ecl/types.rs).

## Follow-up tracking

Outstanding decisions, implementation reconciliation and qualification work are tracked in the [scoped follow-up register](../FOLLOW_UPS.md#statements-analytics-credit-checks-and-impairment).
