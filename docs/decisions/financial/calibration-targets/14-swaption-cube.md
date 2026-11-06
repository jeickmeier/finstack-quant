# CALIBRATION-TARGETS-014 — SABR swaption buckets, parameter interpolation and cube acceptance

- **Decision status:** Draft — observed implementation; adoption pending.
- **Implementation status:** Observed for the stated source scope; behavior and scope limitations are described below.
- **Validation status:** Source and selected evidence inspected or located; not executed.
- **Audit date / source revision:** 2026-11-05 / `7e82fd03ad98346727769e88dd5ac5d84b56e9fc`. Scope and evidence boundaries: [README](README.md).

## Choice and domain

Fit fixed-beta SABR independently by quoted expiry/tenor bucket and publish a parameter cube with a stored forward at every target node. Normal, Black and shifted Black contracts are explicit; there is no automatic interpretation of bp as decimal vol. Pricing evaluator behavior is owned by [model volatility interpolation](../models-analytic/volatility-source-interpolation.md) and [SABR kernels](../models-analytic/sabr-calibration.md).

## Algorithm and controls

1. Group swaption quotes by `(round(expiry*10000),round(tenor*10000))` cast to unsigned integers. Use exact contractual times for models and retained axes, but a representative quote supplies a bucket's forward. Mixed convention IDs in one rounded bucket error. Nonpositive expiries are not silently filtered as in equity targets; shared fit/domain checks can fail them.
2. Quoted vols must be finite nonnegative decimal units and have exactly the quote type implied by plan convention. Require at least three strikes per bucket. Default absolute fit tolerance is `.0015` quoted-vol units, required positive/finite. Convert to shared relative fitting tolerance by dividing by minimum quote; final published-cube errors remain absolute and unscaled by vega.
3. Normal dispatch forces beta=0 and enables ATM pinning. Black uses configured beta (default .5) and explicitly no shift. Shifted Black requires finite positive fixed displacement and fits using that shift; shared SABR checks shifted forward/strike domains. Each fit receives global max iterations. Failure is saved per bucket and other buckets continue; success retains parameters, exact axes, calibration forward and diagnostics.
4. Target nodes first use the rounded exact-key lookup. Otherwise bracket exact successful expiry/tenor axes, reject outside bounds by default or clamp when configured. Interpolate alpha and nu in log space after flooring endpoints at `1e-16`; rho linearly then clamp to `[-.999,.999]`; beta and shift inherit from the lower anchor. Bilinear interpolation is two successive 1D interpolations. Exact zero nu survives direct node lookup but becomes a positive floor in interpolation.
5. Missing-corner fallback defaults false. If enabled, lower-left remains required; missing high-expiry/low-tenor and low-expiry/high-tenor use lower-left; missing high/high uses high-expiry/low-tenor after its fallback. The analogous 1D missing upper point uses lower. A missing required lower corner or unavailable target errors. Clamp counters and interpolated counts are reported.
6. Exact fitted target keys reuse their calibration forward; interpolated nodes recalculate index-default synthetic forwards [013](13-swaption-forwards.md). Construct expiry-major SabrParameterData/forward grids and VolCube. No cross-expiry arbitrage projection or generic equity-surface diagnostics are invoked here.
7. Re-evaluate **every original swaption quote through the published cube** with the clamped evaluator in Normal or Lognormal convention. Nonfinite evaluation errors. Report max absolute quoted-vol error versus the fit tolerance. Any saved failed bucket forces success/validation false even if fallback made the requested cube constructible; no silent partial-fit success. Framework execution policy determines whether a failed report may be returned.

## Limits and alternatives

Rounding at 1e-4 years can coalesce nearby distinct contracts and target key lookup can reuse a nearby bucket rather than interpolate. Exact axes do not remove that lookup convention. Parameter interpolation is not total-variance interpolation and does not guarantee calendar or butterfly absence. Lower-anchor beta/shift is safe only under the plan's common convention/fixed-beta assumptions. Missing-corner substitutions are directional, not nearest-neighbor distance minimization. A sparse target grid can discard a successful interior smile and fail artifact acceptance.

Alternatives include quote-space/total-variance cube interpolation, globally constrained SABR terms, explicit interpolated forwards or full-contract reference grids.

## Evidence

- Owning [swaption.rs](../../../../finstack-quant/calibration/src/targets/swaption.rs) fully inspected; [schema](../../../../finstack-quant/calibration/src/api/schema.rs) establishes defaults.
- Located inline tests for decimal quote types, normal ATM pinning, explicit shift, failed-bucket report, discarded interior smile and materialized coordinates. The inspected bilinear test checks positivity/rho bounds but does not assert a full independent numeric parameter grid. [Integration tests](../../../../finstack-quant/calibration/tests/calibration/swaption_vol.rs) locate Error/Clamp and pricing-type behavior.
- [Hagan SABR reference](../../../REFERENCES.md#hagan-2002-sabr) supports model provenance only.

No calibration execution was performed.

## Follow-up tracking

Adoption, reconciliation and qualification work is tracked in the
[calibration-target follow-ups](../FOLLOW_UPS.md#calibration-quotes-and-curvesmile-targets).
