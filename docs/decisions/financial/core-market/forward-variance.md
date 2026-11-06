# CORE-MARKET-17: Pointwise and interval forward-variance curves

- Decision status: **Draft — observed implementation, adoption pending.**
- Implementation status: **Observed** in the owning paths identified below; discrepancies are stated explicitly.
- Validation status: **Evidence inspected or located, not executed.** Formula checks, invariants, and round trips are distinguished below; no production accuracy certification is implied.
- Audit date: **2026-11-05**. Source revision: `7e82fd03ad98346727769e88dd5ac5d84b56e9fc`.
- Scope: the implementation listed here; the [slice inventory](README.md) and [coverage ledger](coverage.json) identify exact file coverage.

## Choice and rationale

`ForwardVarianceCurve` supplies positive forward-variance levels `xi0(t)=E^Q[V_t|F0]` under the consuming model's pricing measure. It supports linear point interpolation or right-continuous constant intervals, and flat tails. These are different representations with deliberately different behavior exactly at interval boundaries. The source explicitly distinguishes differencing ATM implied total variance as an optional proxy from an exact expected-variance/variance-swap bootstrap.

## Algorithm, domain, and selection

`flat(v0)` accepts only finite strictly positive variance and stores one point at zero. `from_points` accepts nonempty input, sorts by time, requires finite nonnegative unique times and finite strictly positive values. Singletons are allowed. For Linear evaluation, clamp to first/last value outside the knot span; inside, return an exact knot value or linear interpolation `v0+w*(v1-v0)`.

`from_intervals` requires the caller's interval ends to be finite, strictly increasing from a previous endpoint of zero, then uses the positive-value checks and marks ConstantIntervals. At query `t`, select the first endpoint strictly greater than `t`; clamp the resulting index to the final node. Thus values apply on `[previous_end,end)`, and the **next** interval begins at an exact endpoint. The last interval value extends indefinitely. Negative finite queries receive the first level. Any nonfinite query returns NaN for both modes.

Values are variance rates (squared decimal volatility per model year), not volatility or total integrated variance; 0.04 corresponds to a 20% volatility level. The curve itself carries no date, day count or currency. A caller must align its numerical time coordinate with the model. Serde preserves mode and validates aligned time/value lengths by calling the relevant constructor.

## Limitations and alternatives

Zero variance is rejected, even though other core volatility quote grids accept zero; there is no explicit deterministic boundary branch here. Piecewise-constant intervals are not derived automatically from total-variance quotes. No integration method, arbitrage calibration, smoothing or floor/repair exists in this container. Linear positive endpoint interpolation with flat tails preserves positive levels for finite queries; the policy says nothing about a consumer's option-price smile arbitrage.

Interpolating total variance and differentiating it, allowing a zero boundary, and enforcing time-basis metadata are additional choices. Supported point/interval alternatives trade continuous levels against exact interval averages if the input is constructed that way.

## Evidence

Owning code and tests: [forward_variance.rs](../../../../finstack-quant/core/src/market_data/term_structures/forward_variance.rs). Inspected tests assert 0.25 selects the next interval, final-value tail extension, linear midpoint 0.06 between 0.04 and 0.08, domain rejection and serialization-mode preservation. These are elementary formula and contract tests, not stochastic-volatility calibration benchmarks. Reference context: [Bayer–Friz–Gatheral](../../../REFERENCES.md#bayer-friz-gatheral-2016); stochastic simulation/fitting kernels are outside this data container's scope.

## Follow-up tracking

Pending decisions, implementation reconciliation and validation work are tracked in the
[core-market follow-ups](../FOLLOW_UPS.md#core-market-data).
