# MODELS-NUMERICAL-007: Hull–White event-aligned trinomial and affine node bonds

- Decision status: **Draft — observed implementation, adoption pending**.
- Implementation status: Observed in the owning paths identified below; no implementation change authorized.
- Validation status: Evidence inspected/located, **not executed**. Formula comparisons, invariants and roundtrips are distinguished below; none establishes universal accuracy.
- Audit date: 2026-11-05. Source revision: `7e82fd03ad98346727769e88dd5ac5d84b56e9fc`; exact scope ledger in [coverage.json](coverage.json).

## Choice and domain

[hull_white_tree.rs](../../../../finstack-quant/models/src/trees/hull_white_tree.rs) supports negative normal short rates with κ>0, σ>0 and at least two requested steps. A positive piecewise-constant volatility schedule overrides scalar σ in interval evolution, but scalar σ is still validated. Default configuration is κ=0.03, σ=0.01, N=100, continuous compounding. Times are year fractions supplied by callers; the tree performs no calendar conversion.

## Construction, calibration and selection

Combine finite mandatory dates with interior volatility knots; discard dates within 1e−9 of endpoints or outside the horizon, sort/deduplicate, and subdivide each segment using max(round(segment/(T/N)),1). Actual N can differ from requested N. Set the arriving level's spacing dx=σ_i√(3dt_i). For node x choose central next index k=round(x(1−κdt)/dx_next), residual η=x(1−κdt)−k dx_next, A=(σ²dt+η²)/dx_next², B=η/dx_next, and probabilities ((A+B)/2,1−A,(A−B)/2). Reject negative/nonfinite probabilities rather than clipping.

Fit interval r=x+α to P(t_next). Continuous α=(ln ΣQ exp(−x dt)−ln P)/dt; other compounding uses Brent. Propagate Arrow–Debreu prices and reject nonfinite/negative masses or relative DF error>1e−5 (0.1 bp). Repeat the last α on the terminal row. Rollback uses the same stored branches and compounding then a callback. Bond and swaption callers pass coupon/exercise dates as mandatory times; `step_at_time` enforces matching while `time_to_step` snaps nearest, with ties to the lower point.

## Conditional bond method

`bond_price` is a separate continuous affine formula: P(t,T)=P(0,T)/P(0,t) exp(−B(x+c(t))−Var[x(t)]B²/2). B and integrated variance come from HW helpers; constant c(t)=σ²B(0,t)²/2. Scheduled c uses the difference of weighted variance integrals divided by κ, with a small-κ integral branch. It deliberately uses the centered x and instantaneous covariance shift rather than lattice interval α.

## Limitations, alternatives and evidence

The lattice uses Euler mean/variance while node bonds use exact integrated model moments; finite-grid consistency, particularly with noncontinuous discounting, is not established by curve fit alone. No final-maturity drift is calibrated. Invalid node-bond indices/nonfinite maturity yield NaN; matured bonds return 1; nonpositive P(0,t) returns 0. General accessors can return zero, a last dt or equal probabilities on invalid indices.

Inline `test_tree_calibration`, `calibrate_places_mandatory_dates_on_grid`, `scheduled_tree_includes_volatility_knots_in_grid`, `root_forward_swap_rate_matches_curve_par_rate_within_one_bp`, and unit-payoff tests cover curve consistency and dates, not universal derivative accuracy. [HW 1994](../../../REFERENCES.md#hull-white-1994-numerical-procedures) and [HW 1990](../../../REFERENCES.md#hull-white-1990-pricing-ird) are owning references. Exact OU moment branching, rejection-only accessors and grid-converged node bond values are unimplemented alternatives.

## Follow-up tracking

Adoption questions, implementation reconciliation and validation work are tracked in the
[scope follow-ups](../FOLLOW_UPS.md#trees-and-finite-differences).
