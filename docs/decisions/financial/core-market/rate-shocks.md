# CORE-MARKET-03: Bump units, continuous zero/forward shocks, and transform composition

- Decision status: **Draft — observed implementation, adoption pending.**
- Implementation status: **Observed** in the owning paths identified below; discrepancies are stated explicitly.
- Validation status: **Evidence inspected or located, not executed.** Formula checks, invariants, and round trips are distinguished below; no production accuracy certification is implied.
- Audit date: **2026-11-05**. Source revision: `7e82fd03ad98346727769e88dd5ac5d84b56e9fc`.
- Scope: the implementation listed here; the [slice inventory](README.md) and [coverage ledger](coverage.json) identify exact file coverage.

## Choice and rationale

Discount and forward curves retain the original interpolator and compose an analytic adjustment rather than perturbing only visible pillars and refitting. This preserves the requested shock between sparse knots and after rolling. The implementation contains both this continuous method and knot-weighted methods for other curve families; equivalence is not universal.

## Algorithm and controls

`BumpSpec` maps additive rate basis points to value/10,000, percentage points to value/100, fractions unchanged, and multiplicative factors unchanged. Other standard mode/unit pairs are rejected. Values must be finite; triangular neighbors, when present, must be finite and strictly ordered around the target. Negative time buckets are not prohibited. First buckets are flat at weight one to the left of target, last buckets to the right, interior weights rise/fall linearly and vanish outside neighboring buckets. With neither neighbor the constructor selects Parallel. The generic weight helper floors denominators at 1e-10.

For an unrolled discount curve an additive continuous-zero shock is `D_b(t)=D(t)*exp(-b*t*w(t))`. The shared adjustment stores its derivative `d[-b*t*w(t)]/dt` as piecewise-linear segments; its integral is accumulated using Neumaier compensation. For forward curves the added function is `b*w(t)` itself. Breakpoints from successive shocks are merged, sorted and deduplicated; redundant identical linear pieces are dropped. A parallel multiplicative forward shock scales both source scale and existing additive adjustments. Multiplicative discount shocks and multiplicative forward key-rate shocks are unsupported.

With original source interpolator `I`, offset `o`, and adjustment `a`, discount evaluation is `I(o+t)/I(o)*exp(integral(o,o+t,a))`, with exact one at zero. Forward evaluation is `scale*I(o+t)+a(o+t)`. Rolling advances offset using the curve's signed day-count fraction and refreshes discount normalization; it preserves previous shocks at their source times. Newly requested shocks are anchored to the current offset. Visible pillars are updated for reporting. Serialization carries source points and transform; rebuilding with replacement knots deliberately discards the old transform. Restoring a transform checks visible samples against evaluation at relative tolerance 1e-12 times `max(abs(actual),abs(sample),1)`.

## Failure and limitations

Discount shocks require positive finite DFs at visible pillars and at a triangular peak, then set `allow_non_monotonic=true` and remove the forward floor. Forward shocks require finite values at those locations. Neither proves finite/domain-valid results for every possible time or extrapolation horizon. Roll drops expired public knots and fails if no usable future pillars remain, even if the source interpolator can extrapolate. Negative rolls are accepted where the rebuilt state remains valid.

The continuous adjustment uses exact bucket widths while the generic knot-weight helper floors widths at 1e-10. Thus extremely close valid buckets can produce different shock weights in continuous discount/forward paths versus hazard/price/inflation knot paths. This is a concrete consistency gap, not an approved approximation.

## Alternatives, evidence

Knot-only refitting is implemented by other families and is cheaper conceptually but changes interpolation between pillars. Quote bump plus full recalibration is an economically different risk definition. Neither should silently replace this definition. [Tuckman–Serrat](../../../REFERENCES.md#tuckman-serrat-fixed-income) supplies risk context; exact shock-shape and serialization policies are repository choices.

Owning code: [bump specification](../../../../finstack-quant/core/src/market_data/bumps.rs), [piecewise-linear adjustment](../../../../finstack-quant/core/src/market_data/term_structures/common/adjustment.rs), [discount evaluation](../../../../finstack-quant/core/src/market_data/term_structures/discount_curve/evaluation.rs), [discount transformation](../../../../finstack-quant/core/src/market_data/term_structures/discount_curve/transform.rs), [forward evaluation](../../../../finstack-quant/core/src/market_data/term_structures/forward_curve/evaluation.rs), [forward transformation](../../../../finstack-quant/core/src/market_data/term_structures/forward_curve/transform.rs). Inspected [discount tests](../../../../finstack-quant/core/tests/market_data/curves/discount.rs) check off-pillar exponential formulas and bucket partition; inspected [forward tests](../../../../finstack-quant/core/src/market_data/term_structures/forward_curve/tests.rs) verify a narrow shock missed by the Simpson source grid still has exact area. Composition/serde tests are invariants, not external risk benchmarks.

## Follow-up tracking

Pending decisions, implementation reconciliation and validation work are tracked in the
[core-market follow-ups](../FOLLOW_UPS.md#core-market-data).
