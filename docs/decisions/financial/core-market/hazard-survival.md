# CORE-MARKET-04: Piecewise-constant hazard, survival, and conditional default probability

- Decision status: **Draft — observed implementation, adoption pending.**
- Implementation status: **Observed** in the owning paths identified below; discrepancies are stated explicitly.
- Validation status: **Evidence inspected or located, not executed.** Formula checks, invariants, and round trips are distinguished below; no production accuracy certification is implied.
- Audit date: **2026-11-05**. Source revision: `7e82fd03ad98346727769e88dd5ac5d84b56e9fc`.
- Scope: the implementation listed here; the [slice inventory](README.md) and [coverage ledger](coverage.json) identify exact file coverage.

## Choice and rationale

`HazardCurve` represents nonnegative piecewise-constant hazard rates attached to **interval-ending knots**, with log-linear survival and constant last-hazard extrapolation. Conditional default probability integrates interval hazards directly, avoiding loss of information when unconditional survival underflows. This does not implement CDS cashflow integration; that resides in the valuations/calibration scopes and [ADR 0030](../models-and-valuations-credit/0030-cds-survival-and-integration.md).

## Algorithm, domain, and conventions

The builder sorts finite nonnegative times, rejects duplicates and nonfinite/negative hazards, requires explicit recovery in [0,1], and caps hazard at 10 per year by default (override available). At least one supplied hazard point is required. It starts survival at `(0,1)`, skips knots with `t<=1e-9`, and accumulates `H_i += lambda_i*(t_i-t_previous)`, storing `S_i=exp(-H_i)`. Strict log-linear interpolation of these survival pillars reproduces constant interval hazard. Tail survival is `S(last)*exp(-lambda_last*(t-last))`; `sp(t<=0)=1`.

`hazard_rate(t)` chooses the first knot at or after `t`, or last beyond the tail; for nonpositive time it returns the first stored hazard. `default_prob(t1,t2)` is unconditional `S(t1)-S(t2)` and rejects reversed endpoints. Date-based `conditional_default_prob` maps both dates by stored day count (Act/365F default), floors times to zero, integrates overlap with each interval and tail, and returns `-expm1(-integral)`. It returns zero for equal dates and rejects reversed dates. `survival_at_dates` additionally clamps results to [0,1].

`from_survival_probs` sorts finite times and probabilities in (0,1], treats times <=1e-9 as origin requiring survival within 1e-12 of one, requires nonincreasing survival, and derives `lambda=-ln(S_i/S_previous)/dt`. Zero survival cannot be ingested. A curve whose stored survival pillars underflow to zero can fail strict interpolation construction even though later tail evaluation may legitimately underflow.

Rolling shifts surviving ending knots, inserts a new zero-time hazard sample, and rebuilds; it fails when no positive-time knot survives. This realizes conditional survival for retained piecewise segments. Stored par quotes are shifted and expired quotes removed; calibration recipe is dropped.

## Limitations and alternatives

The max-hazard override itself is not checked for a finite, nonnegative value; NaN bypasses `lambda > cap`. `default_prob` does not validate finite numeric endpoints, unlike the date-based conditional method. The `<=1e-9` origin tolerance is applied by survival integration but `hazard_rate` uses exact stored knots: a positive near-zero knot can report a hazard different from the derivative of survival over that tiny interval. Exact origin handling or a consistently applied tolerance needs agreement.

Time-varying smooth hazard, interpolated cumulative hazard, or directly stored log survival are alternative representations, not selectable here. A log-survival representation could extend extreme-survival construction range; the current direct conditional integral already protects short conditional periods after large accumulated hazards.

## Evidence

Owning code: [builder](../../../../finstack-quant/core/src/market_data/term_structures/hazard_curve/builder.rs), [survival/conditional kernels](../../../../finstack-quant/core/src/market_data/term_structures/hazard_curve/curve.rs), [defaults](../../../../finstack-quant/core/src/market_data/term_structures/hazard_curve/mod.rs), [roll/transform](../../../../finstack-quant/core/src/market_data/term_structures/hazard_curve/transform.rs), [wire](../../../../finstack-quant/core/src/market_data/term_structures/hazard_curve/wire.rs). Inspected [tests](../../../../finstack-quant/core/src/market_data/term_structures/hazard_curve/tests.rs) cover interval boundaries, derivative consistency away from knots, conditional survival under roll, direct overlap integrals, 1e-18 hazard after a large integral, and unconditional underflow. Evidence is analytic formula/invariant, not an independent CDS vendor reconciliation. References: [ISDA CDS model](../../../REFERENCES.md#isda-cds-standard-model), [Hull–White CDS](../../../REFERENCES.md#hull-white-2000-cds).

## Follow-up tracking

Pending decisions, implementation reconciliation and validation work are tracked in the
[core-market follow-ups](../FOLLOW_UPS.md#core-market-data).
