# MODELS-ANALYTIC-24: Rate model clocks, curve fitting and conditional forward projection

**Decision status:** Draft (observed implementation, adoption pending).

**Implementation status:** Observed in the source scope identified below; discrepancies are identified explicitly.

**Validation status:** Evidence inspected/located, not executed. Formula comparisons, round trips and invariants have different evidential strength; none establishes approval or universal accuracy.

**Audit date:** 2026-11-05. **Source revision:** `7e82fd03ad98346727769e88dd5ac5d84b56e9fc`.

## Scope and input domain

Curve-driven HW1F setup and term forwards. Model time is actual calendar days/365 from valuation date; source discount curve can have its own base date/day-count.

## Choice and rationale

Convert model time through dates before querying/rebasing the discount curve. Conditional affine bond ratios derive simple forwards; a deterministic additive spread can represent projection basis.

## Algorithm

Map t to365t calendar days, query source year fraction at floor date and interpolate linearly to next day for fractional day; reject nonfinite/negative/out-of-date-range t. Model DF=Psource(mapped t)/Psource(mapped0), requiring finite positive values. Estimate f0(t)=−central difference ln P at h=clamp(.001t,1e−6,1e−3) if t>h; near origin use−ln P(h)/h. `prepare_hw1f_params` supplies rebased discount callback to shared theta calibrator on monthly knots i/12 with ceil(12horizon) count (at least1); initial rate=f0(0). `period_coeffs` gets B and ln A at t=max(fixing,0),T=t+max(τ,0); conditional P=exp(ln A−Br), simple forward=(1/P−1)/τ+spread, or0 ifτ≤0. Deterministic curve_forward uses P(t)/P(t+τ).

## Selection and controls

Explicit HullWhiteCalibrationParams and discount curve/as_of/horizon. Monthly theta spacing is internal, not exposed in these helpers. `from_flat_rate` creates B=0 and ln A=−ln(max(1+rateτ,MIN_POSITIVE)); additive spread changes simple forward only, not discount_factor. CouponEvent carries accrual, discount factor, conditional coefficients and needs_path_sample flag.

## Conventions

Calendar model time and contractual coupon accrual are distinct. Source DF base date is rebased to valuation date. Projection from the modeled discount curve plus additive spread is deterministic-basis treatment, not independent stochastic forward dynamics.

## Limitations, failure and fallback

Negative finite fixing/tenor are clamped in period_coeffs, whereas curve_forward requires positive tenor. Flat-rate denominator is floored, silently modifying extreme rate×accrual inputs. Origin finite-difference formula assumes normalized P(0)=1, satisfied by ModelDiscountCurve. Theta fitting details live in the shared OU process implementation beyond this file slice.

## Alternatives and tradeoffs

An explicit discount/forward multi-factor model or exact analytic curve derivatives would change assumptions. Current date mapping preserves curve conventions while monthly theta introduces its own approximation.

## Implementation and validation evidence

[clock.rs](../../../../finstack-quant/models/src/rates/clock.rs), [curve.rs](../../../../finstack-quant/models/src/rates/hw1f/curve.rs), [coupon_event.rs](../../../../finstack-quant/models/src/rates/hw1f/coupon_event.rs) and [hull_white.rs](../../../../finstack-quant/models/src/rates/hull_white.rs). Inline flat-curve/curve-rebasing and invalid-DF tests locate mapping/formula evidence; no date-clock accuracy claim beyond inspected implementation is made. References: [Hull–White](../../../REFERENCES.md#hull-white-1990-pricing-ird), [Interest-rate modeling](../../../REFERENCES.md#andersen-piterbarg-interest-rate-modeling).

## Follow-up tracking

Adoption questions and implementation or validation work are tracked in the
[analytic-model follow-ups](../FOLLOW_UPS.md#analytic-models).
