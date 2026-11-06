# CALIBRATION-TARGETS-012 — Per-expiry SVI calibration and finite-grid density diagnostics

- **Decision status:** Draft — observed implementation; adoption pending.
- **Implementation status:** Observed for the stated source scope; behavior and scope limitations are described below.
- **Validation status:** Source and selected evidence inspected or located; not executed.
- **Audit date / source revision:** 2026-11-05 / `7e82fd03ad98346727769e88dd5ac5d84b56e9fc`. Scope and evidence boundaries: [README](README.md).

## Choice and inputs

Fit five-parameter raw SVI slices in log forward moneyness with the shared SVI model solver, then publish a Black surface via total variance. Each live expiry requires at least five quote entries; spot, strike and quoted vol are strictly positive. Independent slices do not themselves enforce a globally arbitrage-free surface.

## Algorithm

1. Public preflight requires positive finite target expiries/strikes and at least five target strikes. Target requires nonempty axes, matching equity options and positive spot. Resolve a discount/carry forward; without a discount curve, require explicit/market yield and assume zero discount rate [009](09-equity-forward-inputs.md).
2. Convert expiries to ACT/365F. Check each matching quote's positive strike and vol before retaining only `T>0`. Group exact expiry times; fail if no live group or fewer than five entries in any group.
3. Construct `LevenbergMarquardtSolver` directly with global numerical tolerance and max iterations, then call `calibrate_svi_with_solver(strikes,vols,F,T,solver)` for each slice. This bypasses generic target global-solver selection. Sum iterations and retain fitted SVI parameter records. Shared [SVI fit/density](../models-analytic/svi-fit-and-density.md) owns initialization, residuals, parameter transforms and raw formula `w(k)=a+b*(rho*(k-m)+sqrt((k-m)^2+sigma²))`.
4. At each target absolute strike, evaluate source SVI at `k_i=ln(K/F(T_i))`; total-variance-interpolate and always clamp outside fitted expiry range. Reject invalid/nonpositive resulting vol; materialize TotalVariance mode surface and recompute original live quote residuals through the artifact.
5. Run generic target-grid surface diagnostics. When `validation.check_arbitrage` is enabled, also scan each fitted slice's Durrleman `g(k)` at **201 equally spaced points**, from `ln(K_min/F)-1` through `ln(K_max/F)+1`. Nonfinite scan bounds/density error immediately; retain worst `g<-1e-10` per slice as a butterfly violation. A generic report-attachment policy controls whether recorded violations make validation fail.
6. Fit acceptance uses absolute decimal-vol error budget (default .001), separate from LM termination; report labels PerSliceLeastSquares and stores slices. SVI model evaluation errors include exact expiry/strike/forward/log-moneyness and all parameters instead of replacing failures with NaN.

## Alternatives and limitations

Five observations need not identify five parameters well; duplicate/clustered strikes and flat smiles can remain ill-conditioned. A finite 201-point wing scan is diagnostic evidence only, not continuous density proof or all-expiry certification. Out-of-range target expiries silently use the specified hardcoded variance clamp; no SVI extrapolation field exists. Alternatives include SSVI/global constraints, constrained price-space fitting, adaptive density/Lee-wing validation and explicit tail selection.

## Evidence

- Owning [svi.rs](../../../../finstack-quant/calibration/src/targets/svi.rs); [SVI models record](../models-analytic/svi-fit-and-density.md) and [framework surface policy](../calibration-framework/17-surface-validation.md) separate kernel from target diagnostics.
- Inspected flat-total-variance formula test; located nonlinear varying-forward calendar monotonicity, invalid spot/strike, enriched model-error and arbitrage-report cases inline. [Integration tests](../../../../finstack-quant/calibration/tests/calibration/svi_surface.rs) locate report-policy and published-grid behaviors. These are formulas/invariants and generated-model fits, not broker validation.
- [Gatheral 2004](../../../REFERENCES.md#gatheral-2004-svi) / [Gatheral–Jacquier 2014](../../../REFERENCES.md#gatheral-jacquier-2014-svi) identify relevant parameterization/no-arbitrage literature; code adoption of a particular finite scan is not established by the references.

## Follow-up tracking

Adoption, reconciliation and qualification work is tracked in the
[calibration-target follow-ups](../FOLLOW_UPS.md#calibration-quotes-and-curvesmile-targets).
