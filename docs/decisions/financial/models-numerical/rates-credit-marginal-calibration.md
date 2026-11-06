# MODELS-NUMERICAL-008: Rates/credit marginal calibration, hazard floor and variance loss

- Decision status: **Draft — observed implementation, adoption pending**.
- Implementation status: Observed in the owning paths identified below; no implementation change authorized.
- Validation status: Evidence inspected/located, **not executed**. Formula comparisons, invariants and roundtrips are distinguished below; none establishes universal accuracy.
- Audit date: 2026-11-05. Source revision: `7e82fd03ad98346727769e88dd5ac5d84b56e9fc`; exact scope ledger in [coverage.json](coverage.json).

## Choice and domain

[calibration.rs](../../../../finstack-quant/models/src/trees/two_factor_rates_credit/calibration.rs) fits rate and hazard marginals separately to conditional discount factors and survival probabilities. [mod.rs](../../../../finstack-quant/models/src/trees/two_factor_rates_credit/mod.rs) defaults to 100 steps and zero volatility, mean reversion and correlation. Inputs have N+1 equally spaced times starting at zero, finite positive factors starting at one, nonincreasing survival≤1, and recovery in [0,1]. Volatilities are absolute normal rate/intensity units per sqrt(year); κ is annual and bounded to [0,0.15]. Negative rates are allowed; effective hazard is max(raw hazard,0).

## Algorithm and controls

Set each root to −ln(target[1])/dt. Rows are affine base+node·2σ√dt; next base starts one σ√dt below the current base. Reversion changes up probability to clamp(1/2−κ(x−x_ref)√dt/(2 max(σ,1e−12)),0,1), with x_ref fixed at the first-period rate/intensity. Calibration and pricing use the same probabilities on raw factor levels. Discount rate state prices by exp(−r dt), hazard state prices by exp(−max(h,0)dt).

For the next row's target maturity, unfloored drift has the analytical log-ratio solution; invalid/nonpositive model sums fall back to zero shift. Floored hazard drift solves ΣQ exp(−max(base+θ,0)dt)=target. Its lower bracket floors every node; if target≥incoming mass, return that all-floored shift and increment `unreachable_steps` (even equality is counted). Otherwise expand the upper bracket up to 200 times and use Brent. **Solver failure returns the bracket midpoint**, without a reported root error or final repricing gate. Build a candidate and publish only after correlation feasibility succeeds, but these internal numerical fallbacks remain successful calibration paths.

## Diagnostics, alternatives

Floor diagnostics report state-price-weighted mass at zero and worst step. Variance retention reports min 4p(1−p), clamp count/share and worst step over all pricing rows, without probability-mass weighting. A κ cap alone cannot control κT; long horizons can have deterministic tail moves. Additive dynamics plus a nonlinear hazard floor are an observed approximation. Strict failure/fit gates, acceptance limits for floor mass/variance loss, and positive-intensity alternatives such as CIR/lognormal lattices are not implemented here. Terminal rows are extrapolated with zero new shift.

## Evidence

[tests.rs](../../../../finstack-quant/models/src/trees/two_factor_rates_credit/tests.rs) includes `calibration_quality_survival_matching` (forward survival reconstruction to 1e−6), `calibration_quality_zcb_repricing`, `hazard_floor_saturation_reports_binding_floor`, `variance_retention_is_bounded_by_one_minus_kappa_t_squared`, and `long_horizon_clamping_is_reported_not_silent`. They establish selected invariants and expose distortion, not independent callable-credit accuracy. [Brent](../../../REFERENCES.md#brent-1973) covers the root method; the midpoint fallback, κ limit and floored marginal construction lack a dedicated adoption reference.

## Follow-up tracking

Adoption questions, implementation reconciliation and validation work are tracked in the
[scope follow-ups](../FOLLOW_UPS.md#trees-and-finite-differences).
