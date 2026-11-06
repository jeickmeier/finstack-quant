# CORE-MARKET-08: Signed forward-price and volatility-index level curves

- Decision status: **Draft — observed implementation, adoption pending.**
- Implementation status: **Observed** in the owning paths identified below; discrepancies are stated explicitly.
- Validation status: **Evidence inspected or located, not executed.** Formula checks, invariants, and round trips are distinguished below; no production accuracy certification is implied.
- Audit date: **2026-11-05**. Source revision: `7e82fd03ad98346727769e88dd5ac5d84b56e9fc`.
- Scope: the implementation listed here; the [slice inventory](README.md) and [coverage ledger](coverage.json) identify exact file coverage.

## Choice and rationale

One `PriceCurve` representation supports signed native-unit prices and, via `PriceCurveKind::VolIndex`, nonnegative volatility-index levels. It interpolates supplied levels without deriving cost-of-carry, convenience yield, or a volatility-index replication model. For VolIndex, the documented input is the quoted futures/forward level; a square root of expected variance is not automatically the same quantity, and this curve applies no convexity adjustment. Type-specific validation and unit-specific shocks are financially material choices.

## Algorithm, selection, and conventions

Builder defaults are Price, Act/365F, Linear and FlatZero. A base date and at least two supplied knots are required before origin insertion. Knots must be ordered and values finite. Spot must be supplied unless the first knot is within 1e-14 of zero. The builder inserts an exact `(0,spot)` anchor when absent and rejects an exact-zero knot disagreeing with spot. Negative price knots and spot are allowed for Price; VolIndex rejects negative values. MonotoneConvex is rejected; positive-domain styles reject zero/negative source levels despite the broader Price domain.

For `t<=0`, `price(t)` returns spot; positive time uses the source interpolator, or the original interpolator evaluated at a retained roll offset. Date queries before base explicitly error. FlatZero holds boundary levels; FlatForward extends the selected kernel's endpoint slope and may leave the economic domain. A VolIndex constructor validates knots, not all extrapolated/interpolated queries.

Parallel additive Fraction bumps add native price/level units. Additive Percent and multiplicative Percent both scale by `1+value/100`; multiplicative Factor scales by its value. RateBp is rejected. Triangular shocks accept additive Fraction (absolute local price shift) or additive Percent (local relative scaling) and leave `t<=0` spot unchanged. Other key-rate combinations are rejected. These percentage semantics differ from unitless scalar percentage-point shocks.

Rolling by signed days changes the clock, keeps strictly future shifted knots, sets new spot to the old curve at the offset, and inserts a zero anchor. It errors if all future knots expire. Forward rolls retain source interpolation. Shared `InterpRollState` drops the retained source when cumulative offset becomes negative and rebuilds from available rolled pillars; this is an explicit backward-roll approximation. Shocking a retained source maps original knots in current-relative time, then checks the rolled-origin level. If interpolation moved that origin by more than 1e-12 scaled tolerance, an origin source knot is inserted and the shock rebuilt. Generic replacement knots deliberately remove retained roll state.

## Limitations and alternatives

A price shock is applied to stored/source knots and reinterpolated; its continuous shape need not equal the requested triangular weight between knots for nonlinear interpolation. Signed percentage shocks can reverse sign with factors below zero for Price, while VolIndex revalidation rejects resulting negative knots. There is no universal query-level positivity check or no-arbitrage carry constraint.

Supported interpolation styles trade shape and positivity; explicit no-extrapolation can yield NaN. Cost-of-carry construction or variance-native volatility-index modeling belongs to other layers and is not silently inferred from these quotes.

## Evidence

Owning code and inline tests: [price_curve.rs](../../../../finstack-quant/core/src/market_data/term_structures/price_curve.rs), [shared roll machinery](../../../../finstack-quant/core/src/market_data/term_structures/common/roll.rs). Located tests cover signed prices, local percentage shocks, spot preservation, VolIndex rejection, retained-source serde/bumps, and backward rebuilding. [Scalar-curve invariants](../../../../finstack-quant/core/tests/market_data/curves/scalar_curves_invariants.rs) and [lifecycle tests](../../../../finstack-quant/core/tests/market_data/lifecycle.rs) provide algebraic/round-trip checks, not independent commodity or VIX price validation. Reference context: [Hull](../../../REFERENCES.md#hull-options-futures), [CBOE VIX](../../../REFERENCES.md#cboe-vix-white-paper); storing a VolIndex level does not implement the latter replication formula.

## Follow-up tracking

Pending decisions, implementation reconciliation and validation work are tracked in the
[core-market follow-ups](../FOLLOW_UPS.md#core-market-data).
