# CALIBRATION-TARGETS-012 — Per-expiry SVI calibration and finite-grid density diagnostics

- **Decision status:** Accepted 2026-11-15 — complete per-expiry equity SVI package.
- **Implementation status:** Observed. Reconciliation to the accepted package is pending.
- **Validation status:** Independent native-fit, sampled density and delivered-grid checks verified. Production fitting, market references, restoration and host qualification remain pending.
- **Audit date / source revision:** 2026-11-05 / `7e82fd03ad98346727769e88dd5ac5d84b56e9fc`. Scope and evidence boundaries: [README](README.md).

## Accepted decision for raw equity SVI and mandatory fitted-slice density

1. **Fit one raw SVI slice per selected live expiry.** Use the exact
   dated unshifted Black quotes, qualified volatility clock and
   frozen forward from 009. Require finite positive forward,
   expiry, strikes and observed annual vols. Require at least
   five distinct optimizer strikes. Reject distinct strikes that
   collapse to the same represented `k = log(K/F)`. At a shared
   strike elect one optimizer quote ID and keep each compatible
   companion as a separate required fit check. Size the output
   grid by actual quote support and diagnostic coverage.
   [Accepted dated equity forwards](09-equity-forward-inputs.md#accepted-decision-for-explicit-equity-spot-carry-and-dated-smile-forwards).

2. **Use the checked raw SVI model and one explicitly seeded solve.**
   Fit unweighted total-variance residuals `w(kᵢ) − Tσᵢ²` with
   the accepted central-difference, augmented-QR solver. Require
   one explicit interior seed, residual and step tolerances and
   an attempt budget. Use checked coordinates with positive
   minimum variance and both wing slopes in `(0,2)` for the
   nonlinear fit. Retire heuristic seeds, generic solver defaults
   and hidden retries. Validate the full request before using the
   analytic constant-variance slice for exact equal-volatility
   optimizer quotes. Report zero optimizer attempts on that path
   and apply every quote and density check.
   [Accepted checked raw SVI fit](../models-analytic/svi-fit-and-density.md#accepted-decision-for-checked-raw-svi-and-slice-fit-acceptance).

3. **Gate the fitted slice in native annual volatility.** Re-evaluate
   the returned slice on the original coordinates. Require a finite
   nonnegative explicit slice cap for optimizer quotes. Zero
   requires exact agreement. Check each companion quote against its
   declared native tolerance. Keep the variance objective, actual
   solver termination and native fit outcome distinct.
   [Accepted fit reporting](../calibration-framework/14-fit-reports.md#accepted-decision-for-fit-acceptance-and-report-semantics).

4. **Require a density scan on every fitted slice.** Require at
   least two finite, strictly increasing log-moneyness points
   spanning observed quotes and a finite nonnegative dimensionless
   tolerance. Scan the exact union of the supplied points and
   original quote coordinates. Reject undefined evaluations or any
   `g < −tolerance`. Report the minimum, its first location on
   ties and sample count. Retire the conditional 201-point scan,
   fixed wing margins and hard-coded tolerance. The result
   establishes the density check at its sampled coordinates.
   [Accepted checked SVI density](../models-analytic/svi-fit-and-density.md#accepted-decision-for-checked-raw-svi-and-slice-fit-acceptance) ·
   [Gatheral and Jacquier, Lemma 2.2](https://arxiv.org/pdf/1204.0646).

5. **Require every selected slice and the delivered grid to pass.**
   A failed slice blocks publication. Materialize at fixed absolute
   strike using each fitted slice's own forward and the explicit
   fitted-tail policy. Strictly reprice every consumed quote on the
   exact delivered grid in authored order against its positive
   per-quote tolerance. Apply the elected grid diagnostics separately.
   The fitted-slice density gate is mandatory regardless of advisory
   grid findings.
   [Accepted materialization](10-total-variance-materialization.md#accepted-decision-for-explicit-total-variance-materialization-and-delivered-surface-fit) ·
   [Accepted surface validation](../calibration-framework/17-surface-validation.md#accepted-decision-for-surface-validation-and-publication).

6. **Publish the grid and its fit evidence together.** Preserve exact
   dates, clock, forwards, raw parameters, optimizer and companion
   quote identities, seed and controls, actual solver or analytic
   status, work, native residuals and density scan coverage through
   the existing Rust-owned interfaces and restoration.

## Independent calculation checks

- At `T = .01`, annual sigma moving from `.20` to `.21`
  changes total variance by `.000041` and native annual volatility
  by `.01`.
- For raw SVI `w(k) = √(k² + .05²)`, the sample
  `0,.01,.02,.03,10` has positive checked `g` at
  every sampled coordinate. At `.12`,
  `g = −22662/28561`, about `−.79346`.
- At forward 100 and expiry one, raw tuple
  `(a,b,ρ,m,s) = (.04,.4,−.4,0,.2)` produces
  `σ ≈ .34641016` at strike 100. A delivered TotalVariance row
  sampled at strikes 80 and 120 gives `σ ≈ .39663956` at
  strike 100, a native difference of about `.05022940`.

## Observed choice and inputs at the audit revision

Fit five-parameter raw SVI slices in log forward moneyness with the shared SVI model solver, then publish a Black surface via total variance. Each live expiry requires at least five quote entries; spot, strike and quoted vol are strictly positive. Independent slices do not themselves enforce a globally arbitrage-free surface.

## Observed algorithm

1. Public preflight requires positive finite target expiries/strikes and at least five target strikes. Target requires nonempty axes, matching equity options and positive spot. Resolve a discount/carry forward; without a discount curve, require explicit/market yield and assume zero discount rate [009](09-equity-forward-inputs.md).
2. Convert expiries to ACT/365F. Check each matching quote's positive strike and vol before retaining only `T>0`. Group exact expiry times; fail if no live group or fewer than five entries in any group.
3. Construct `LevenbergMarquardtSolver` directly with global numerical tolerance and max iterations, then call `calibrate_svi_with_solver(strikes,vols,F,T,solver)` for each slice. This bypasses generic target global-solver selection. Sum iterations and retain fitted SVI parameter records. Shared [SVI fit/density](../models-analytic/svi-fit-and-density.md) owns initialization, residuals, parameter transforms and raw formula `w(k)=a+b*(rho*(k-m)+sqrt((k-m)^2+sigma²))`.
4. At each target absolute strike, evaluate source SVI at `k_i=ln(K/F(T_i))`; total-variance-interpolate and always clamp outside fitted expiry range. Reject invalid/nonpositive resulting vol; materialize TotalVariance mode surface and recompute original live quote residuals through the artifact.
5. Run generic target-grid surface diagnostics. When `validation.check_arbitrage` is enabled, also scan each fitted slice's Durrleman `g(k)` at **201 equally spaced points**, from `ln(K_min/F)-1` through `ln(K_max/F)+1`. Nonfinite scan bounds/density error immediately; retain worst `g<-1e-10` per slice as a butterfly violation. A generic report-attachment policy controls whether recorded violations make validation fail.
6. Fit acceptance uses absolute decimal-vol error budget (default .001), separate from LM termination; report labels PerSliceLeastSquares and stores slices. SVI model evaluation errors include exact expiry/strike/forward/log-moneyness and all parameters instead of replacing failures with NaN.

## Observed alternatives and limitations

Five observations need not identify five parameters well; duplicate/clustered strikes and flat smiles can remain ill-conditioned. A finite 201-point wing scan is diagnostic evidence only, not continuous density proof or all-expiry certification. Out-of-range target expiries silently use the specified hardcoded variance clamp; no SVI extrapolation field exists. Alternatives include SSVI/global constraints, constrained price-space fitting, adaptive density/Lee-wing validation and explicit tail selection.

## Evidence

- Owning [svi.rs](../../../../finstack-quant/calibration/src/targets/svi.rs); [SVI models record](../models-analytic/svi-fit-and-density.md) and [framework surface policy](../calibration-framework/17-surface-validation.md) separate kernel from target diagnostics.
- Inspected flat-total-variance formula test; located nonlinear varying-forward calendar monotonicity, invalid spot/strike, enriched model-error and arbitrage-report cases inline. [Integration tests](../../../../finstack-quant/calibration/tests/calibration/svi_surface.rs) locate report-policy and published-grid behaviors. These are formulas/invariants and generated-model fits, not broker validation.
- [Gatheral 2004](../../../REFERENCES.md#gatheral-2004-svi) / [Gatheral–Jacquier 2014](../../../REFERENCES.md#gatheral-jacquier-2014-svi) identify relevant parameterization/no-arbitrage literature; code adoption of a particular finite scan is not established by the references.

## Follow-up tracking

Reconciliation and qualification work is tracked in the
[calibration-target follow-ups](../FOLLOW_UPS.md#calibration-quotes-and-curvesmile-targets).
