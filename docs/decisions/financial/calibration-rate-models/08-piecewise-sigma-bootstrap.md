# CALIBRATION-RATE-MODELS-008: Sequential cap/floor bootstrap on live-fixing volatility intervals

**Decision status:** Draft — observed implementation; adoption pending.

**Implementation status:** Observed for the source scope linked below; source/documentation discrepancies are stated explicitly.

**Validation status:** Evidence inspected/located, not executed. Internal round trips, analytic special cases and external provenance have distinct evidence roles.

**Audit date:** 2026-11-05. **Source revision:** `7e82fd03ad98346727769e88dd5ac5d84b56e9fc`. See the [scope inventory](README.md) and [source ledger](coverage.json).

## Choice and rationale

With κ fixed, the piecewise path solves one constant σ segment per cap/floor quote. Earlier segments remain frozen. The next segment starts at the preceding quote's last live fixing, because a cap price is insensitive to volatility after its final rate fixing; payment maturity would create unidentifiable adjacent segments. Source explicitly rejects repeated maturities instead of inventing a smile aggregation objective.

## Domain and preparation

Require at least one valid Normal cap/floor quote, finite positive fit tolerance, fixed κ in inclusive `[0.001,1]`, and finite bounds `0<σ_min<σ_max`. Public direct config does not impose the scalar solver's upper limit 2; the engine adapter supplies `[1e-5,2]`. Prepare [live schedules](02-cap-schedules-and-quote-inversion.md), keeping each schedule paired with its quote, then sort by maturity. Duplicate maturity is rejected. Each last fixing must be finite/positive and these horizons must increase strictly after sorting, independently of maturity ordering.

## Ordered algorithm

1. Initialize segment times `[0]` and an empty σ vector.
2. For the next sorted quote, compute its Bachelier strip premium. Append a candidate σ to the already-fitted segments and construct the piecewise HW model on the current times.
3. Reprice the full quote, including coupons affected by previous segments. Solve `P_HW(candidate)−P_market=0`; model-price errors become NaN in the scalar residual, not a replacement price.
4. Evaluate both configured σ endpoints. Reject nonfinite residuals or equal nonzero signs. Use Brent within the supplied bracket, tolerance `1e-12`, maximum 200 iterations. Reject a solution at/within relative `1e-6` of σ_max; a lower-bound solution is allowed.
5. Reconstruct that quote's Normal implied volatility and store signed error keyed by maturity. Freeze the solved σ. If another quote remains, append the current quote's last fixing horizon as the next segment start.
6. Construct the final piecewise model, whose last σ extrapolates flat. Return `SequentialBootstrap` diagnostics with quoted-vol residual units, fixed κ, schedule source and piecewise mode. Report iterations equals pillar count, not total Brent iterations.

## Selection and conventions

This is an explicit public bootstrap or the engine's `volatility_mode=piecewise`; scalar remains the engine default. Multiple strikes or cap/floor sides can be used at distinct maturities, but there is one equation per newly fitted segment. Sorting accepts out-of-order inputs; “strictly increasing maturities” describes the normalized order and uniqueness rather than requiring callers to pre-sort.

σ is absolute rate volatility, κ inverse years, and interval times are ACT/365F years. Coupon accruals remain contractual. Unlike scalar fitting, this path applies no aggregate-vega admission gate. Model lookup/integration is delegated to the existing piecewise HW kernel, not approximated by an average σ. Output series date conversion and persistence belong to the [engine adapter](../calibration-framework/18-hull-white-market-adapter.md).

## Limitations, failure and alternatives

Earlier segment choices are not revisited, and no smoothing/regularization or joint best fit is applied. Inconsistent quotes can be unbracketed; no automatic bracket expansion, clipping to a fitted endpoint or fallback to scalar volatility occurs. Final tolerance checks rely on implied-vol residuals recorded when each pillar was solved; later intervals begin at prior last-fixing horizons, so they cannot affect earlier option variance by construction.

A global piecewise fit could combine repeated strikes and regularize term structure, but changes identification, residual weights and solve behavior. The scalar model is simpler and may intentionally leave quote error; the bootstrap prioritizes exact attainable pillar premiums without claiming a smooth or stable term structure.

## Implementation and evidence

[cap_floor.rs](../../../../finstack-quant/calibration/src/hull_white/cap_floor.rs), [cap_schedule.rs](../../../../finstack-quant/calibration/src/hull_white/cap_schedule.rs) and [pricing.rs](../../../../finstack-quant/calibration/src/hull_white/pricing.rs). [tests.rs](../../../../finstack-quant/calibration/src/hull_white/tests.rs) creates a piecewise model with a 0.75-year breakpoint and recovers it from adjacent synthetic caps. [Contractual schedule tests](../../../../finstack-quant/calibration/src/hull_white/cap_schedule_tests.rs) reverse quote order and verify paired schedule sorting and repricing. These are same-kernel recovery/invariant tests, not external term-vol calibration evidence. No tests were run.

## Follow-up tracking

Implementation reconciliation, adoption questions and missing evidence are tracked in the
[rate-model calibration follow-ups](../FOLLOW_UPS.md#calibration-hullwhite-and-lmm).
