# CALIBRATION-TARGETS-011 — Per-expiry equity SABR calibration

- **Decision status:** Draft — observed implementation; adoption pending.
- **Implementation status:** Observed for the stated source scope; behavior and scope limitations are described below.
- **Validation status:** Source and selected evidence inspected or located; not executed.
- **Audit date / source revision:** 2026-11-05 / `7e82fd03ad98346727769e88dd5ac5d84b56e9fc`. Scope and evidence boundaries: [README](README.md).

## Choice and scope

Fit a fixed-beta (default beta .5) SABR smile independently at each live equity option expiry, then publish a Black volatility grid using [total-variance interpolation](10-total-variance-materialization.md). The SABR evaluator, multistart fitting residuals, bounds and convergence kernel are owned by [models SABR calibration](../models-analytic/sabr-calibration.md) and [expansions](../models-analytic/sabr-expansions.md); this target selects quotes, carry, convention and fit budget.

## Algorithm and controls

1. Require nonempty target expiries and at least three target strikes; filter matching `OptionVol` ticker and group future expiries by exact ACT/365F time. Expired/nonpositive-time quotes are omitted. Resolve positive equity forwards through the required discount curve and explicit yield/cash carry [009](09-equity-forward-inputs.md).
2. Require `beta>=1e-4` so this path does not publish a normal-vol SABR branch as Black; full admissible beta/strike/vol domains remain enforced by the shared calibrator and surface construction. Require at least three input quote entries per expiry; shared model validation governs distinctness/identifiability.
3. For each expiry, collect strike/vol arrays, construct the SABR calibrator with maximum iterations from global solver config and relative tolerance `absolute_vol_budget/min(quoted_vol)`, and use `SabrShift::Auto`. Positive equity forwards/strikes ordinarily need no displacement; retained fitted parameters contain the actual shift. Other generic global-solver choices are not used by this target.
4. Any slice fitting error aborts the whole surface; no nearest fitted expiry is substituted for an unfitted live bucket. Empty live slice set errors. Store all fitted parameters, summed total iterations, winning start/iterations, residual evaluation count and bound-hit metadata.
5. Materialize target grid, calculate published-grid quote errors and arbitrage diagnostics, then construct a per-slice-least-squares report in quoted volatility units. Default absolute surface fit tolerance is .001 (0.10 vol points). The relative slice tolerance is a necessary scale conversion; final raw quote errors decide artifact acceptance.

## Alternatives, limitations and evidence

A shared beta, independent expiries and automatic displacement are model choices. Alternatives are free-beta fit, global term-structure fit, price/vega-weighted loss, or a directly arbitrage-constrained surface. Option call/put identity does not alter a quoted-vol fit. The public quote schema allows zero vol, while the fit path computes a ratio using the minimum quote and the model requires positive vols; error semantics need to remain explicit.

- Owning [vol.rs](../../../../finstack-quant/calibration/src/targets/vol.rs), [schema/defaults](../../../../finstack-quant/calibration/src/api/schema.rs), [SABR model](../../../../finstack-quant/models/src/volatility/sabr/calibration.rs) and linked model record identify actual ownership.
- Inline `vol_surface_fails_when_any_expiry_cannot_calibrate` and interpolation tests are located. Published total-variance/acceptance-policy tests in [svi_surface.rs](../../../../finstack-quant/calibration/tests/calibration/svi_surface.rs) cover both equity surface workflows. They reuse model-generated quotes and/or production evaluators; none was run here.

A fitted local smile and positive grid are insufficient proof of global arbitrage freedom.

## Follow-up tracking

Adoption, reconciliation and qualification work is tracked in the
[calibration-target follow-ups](../FOLLOW_UPS.md#calibration-quotes-and-curvesmile-targets).
