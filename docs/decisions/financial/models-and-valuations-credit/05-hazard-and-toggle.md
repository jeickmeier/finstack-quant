# MODELS-AND-VALUATIONS-CREDIT-05: Leverage-driven hazard and issuer PIK exercise

- Decision status: **Draft — observed implementation, adoption pending**.
- Implementation status: Observed in the paths listed below; discrepancies are stated explicitly.
- Validation status: Evidence inspected or located, **not executed**; no external accuracy certification.
- Audit date: 2026-11-05.
- Source revision: `7e82fd03ad98346727769e88dd5ac5d84b56e9fc`.

## Choice and rationale

Leverage hazard maps and threshold/logistic/nested-equity PIK decisions are explicit scenario models. Their coefficients are not inferred from observed issuer behavior.

## Scope and input domain

Finite hazard reference, positive reference leverage for parametric maps; tabular nonnegative increasing leverage axis and nonnegative hazards. CreditState checks finiteness but does not enforce nonnegative economic levels. Optimal toggle requires positive nested path count/horizon and nonnegative volatility.

## Algorithm

1. Hazard maps are `h0(L/L0)^a`, `h0 exp(s(L−L0))`, or piecewise-linear tabular interpolation with flat endpoints. Results clamp to [0,1e6]; NaN leverage is preserved. Post-PIK leverage is accreted notional/asset value.
2. Conditional average market hazard is `−ln(Send/Sstart)/T` for positive nonincreasing survival. CIR coefficient conversion is fractional spread volatility times square root of reference spread.
3. Threshold PIK uses strict above/below comparison. Logistic PIK samples `u < logistic(intercept+sensitivity×state)`. Missing distance-to-default is zero.
4. Optimal toggle compares equity under cash payment (assets reduced by nonnegative coupon, debt unchanged) and PIK (assets unchanged, debt increased). Missing assets are inferred from debt/leverage, or twice debt if leverage is nonpositive. If cash starts below barrier choose PIK; if only PIK starts below barrier choose cash.
5. Nested GBM uses ceil(12T) discrete barrier observations and common PCG64 path streams for both actions. Sum terminal surviving equity; same count and discounting cancel in the comparison. The outer uniform bit pattern is mixed to obtain the nested seed; equal equity chooses cash.

## Selection and controls

Callers choose map and toggle variant explicitly, supplying the outer uniform. No fitted issuer-policy default or continuous optimal stopping is implemented.

## Conventions

Hazard annualized per model time, leverage debt/assets, coupon and asset/debt levels in consistent money units. Logistic sensitivities depend on chosen state units; missing state substitutions are material controls.

## Limitations, failure and fallback

Monthly monitoring misses intrastep barrier hits; nested Monte Carlo makes decisions noisy. Optimal compares a one-time coupon action, not an optimized sequence of future toggles. should_pik does not validate the supplied uniform or revalidate mutable state.

## Alternatives and tradeoffs

Continuous barrier correction, fitted probabilistic exercise and full dynamic programming are unimplemented alternatives. Supported threshold/logistic forms are cheaper and more transparent but less structural.

## Evidence

Owners: [endogenous_hazard.rs](../../../../finstack-quant/models/src/credit/endogenous_hazard.rs), [market_anchored.rs](../../../../finstack-quant/models/src/credit/market_anchored.rs), [toggle_exercise.rs](../../../../finstack-quant/models/src/credit/toggle_exercise.rs). Inline tests locate monotonicity/branch/determinism checks; these are behavioral evidence, not issuer-policy calibration. [Reduced-form credit](../../../REFERENCES.md#duffie-singleton-1999) is background, not provenance for the chosen leverage functions.

## Follow-up tracking

Adoption questions and implementation or validation work are tracked in the
[credit-model and valuation follow-ups](../FOLLOW_UPS.md#credit-models-and-valuations).
