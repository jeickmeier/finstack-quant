# CORE-MARKET-01: Discount factors, zero rates, and relative discounting

- Decision status: **Draft — observed implementation, adoption pending.**
- Implementation status: **Observed** in the owning paths identified below; discrepancies are stated explicitly.
- Validation status: **Evidence inspected or located, not executed.** Formula checks, invariants, and round trips are distinguished below; no production accuracy certification is implied.
- Audit date: **2026-11-05**. Source revision: `7e82fd03ad98346727769e88dd5ac5d84b56e9fc`.
- Scope: the implementation listed here; the [slice inventory](README.md) and [coverage ledger](coverage.json) identify exact file coverage.

## Choice and rationale

`DiscountCurve` stores positive discount factors (DFs) against a year-fraction clock. Its default is MonotoneConvex interpolation and FlatForward extrapolation. This is an implemented selection, not an approved policy for every currency or market. [ADR 0032](../calibration-targets/0032-curve-interpolation-and-calibration.md) records the wider selection question. Shape preservation is a plausible rationale; the allowed negative-rate regimes must be agreed separately.

## Scope and input domain

The builder sorts pillars, inserts `(0,1)` when the earliest time is positive, requires at least two points, finite nonnegative times, finite strictly positive DFs, a unique origin exactly equal to `(0,1)`, and minimum relative knot spacing. Default Act/365F is independent of the identifier. Bus/252 requires a resolved holiday calendar. `flat` accepts only finite continuous rates in [-1,1], explicitly permits increasing DFs for negative rates, uses LogLinear/FlatForward, and disables the forward floor. `from_zero_rates` converts the specified compounding to DFs, then uses the ordinary builder policy; it does not automatically relax the negative-rate policy. `from_dates` defaults to Act/365F and rejects dates before base.

## Algorithm and conventions

1. Interpolate DFs using the selected shared kernel. Linear works in DF values; LogLinear in log DFs; CubicHermite uses shape-preserving Hermite slopes; PiecewiseQuadraticForward interpolates log-discount structure through its forward kernel; MonotoneConvex uses its dedicated forward reconstruction. Detailed kernels and local positivity projection are in [value interpolation](../core-numerics/value-interpolation.md) and [forward interpolation](../core-numerics/forward-interpolation.md).
2. `zero_rate(t, compounding)` delegates the inverse DF conversion, returning exactly zero at `t=0` rather than the limiting short rate.
3. `forward(t1,t2) = -ln(DF(t2)/DF(t1))/(t2-t1)` is continuously compounded. It requires finite ordered endpoints, interval at least `min_forward_tenor` (default 1e-6 years), and positive finite endpoint DFs.
4. The `Discounting` trait computes relative discounting as `DF(to)/DF(from)` after each date is mapped from the curve base using its clock/calendar; equal endpoints return one before other evaluation. Reversed dates are permitted.
5. Its default instantaneous forward uses a finite difference of log DFs: epsilon is `clamp(abs(t)*1e-4,1e-6,1e-4)`, start is `max(t-epsilon,0)`, end is `t+epsilon`. `ParametricCurve` also inherits this trait default even though it has a separate analytic `forward_rate` method.

`FlatZero` holds the boundary **stored value**, so on this curve it holds a DF rather than a zero rate. `FlatForward` extends the boundary slope in the interpolation kernel's working space. `None` yields NaN outside the domain; direct `df` and `zero_rate` return floating values rather than checked results. Negative-time queries therefore follow the interpolator's left-tail policy.

## Selection, failure, and alternatives

`MarketStandard` rejects increasing pillar DFs (1e-14 tolerance) and imposes a -0.005 adjacent continuously compounded forward floor. `NegativeRateFriendly` permits increases and requires a chosen floor; `Raw` exposes both controls. These are **pillar** checks, not a universal between-pillar arbitrage certificate. Shocked curves deliberately relax these checks; see [rate shocks](rate-shocks.md). Public Rust setters for `min_forward_tenor` and raw `forward_floor` do not themselves enforce finite/positive control values; NaN can disable a comparison. Direct small/negative-time instantaneous-forward queries have no explicit domain check.

Supported interpolation alternatives trade smoothness, locality, and negative-rate admissibility. An explicit no-extrapolation policy is supported. Analytic instantaneous forwards, log-domain DF ratios for extreme magnitudes, and mandatory valid numerical controls are possible changes, not adopted alternatives.

## Evidence

Owning code: [builder](../../../../finstack-quant/core/src/market_data/term_structures/discount_curve/builder.rs), [evaluation API](../../../../finstack-quant/core/src/market_data/term_structures/discount_curve/curve.rs), [validation](../../../../finstack-quant/core/src/market_data/term_structures/discount_curve/validation.rs), [wire/calendar/defaults](../../../../finstack-quant/core/src/market_data/term_structures/discount_curve/mod.rs), [trait implementation](../../../../finstack-quant/core/src/market_data/term_structures/discount_curve/traits.rs), and [relative/finite-difference discounting](../../../../finstack-quant/core/src/market_data/traits.rs).

Inspected tests `forward_rate_analytical_verification` and `parallel_bump_magnitude_verification` in [discount tests](../../../../finstack-quant/core/tests/market_data/curves/discount.rs) assert independent elementary formulas. The same file locates negative-rate, interpolation-style, serialization, and roll regressions; these do not replace external curve/vendor comparisons. [Hagan–West](../../../REFERENCES.md#hagan-west-monotone-convex), [Hull](../../../REFERENCES.md#hull-options-futures), and [Andersen–Piterbarg](../../../REFERENCES.md#andersen-piterbarg-interest-rate-modeling) provide method context, not product approval.

## Follow-up tracking

Pending decisions, implementation reconciliation and validation work are tracked in the
[core-market follow-ups](../FOLLOW_UPS.md#core-market-data).
