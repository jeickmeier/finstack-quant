# CALIBRATION-TARGETS-014 — SABR swaption buckets, parameter interpolation and cube acceptance

- **Decision status:** Accepted 2026-11-15 — complete SABR swaption cube calibration, construction and publication package.
- **Implementation status:** Observed. Reconciliation to the accepted package is pending.
- **Validation status:** Independent native-error, parameter interpolation and delivered-quote calculations verified. Production fitting, market references, restoration and host qualification remain pending.
- **Audit date / source revision:** 2026-11-05 / `7e82fd03ad98346727769e88dd5ac5d84b56e9fc`. Scope and evidence boundaries: [README](README.md).

## Accepted decision for exact SABR swaption smiles and strict cube publication

1. **Group selected quotes on exact dated smile coordinates.** Require
   explicit quote IDs and prepare each dated reference under 013. Group
   by exact expiry and tenor on one qualified reference family and
   clock. Reject incompatible contracts that collide on the axes.
   Require at least three distinct represented strikes and positive
   finite annual volatilities per fitted smile. Preserve quote IDs
   and authored order through fit evidence. Retire rounded bucket keys
   and representative-quote schedules.
   [Accepted dated swaption preparation](13-swaption-forwards.md#accepted-decision-for-dated-swaption-coordinates-and-calibration-forwards).

2. **Declare the quote law, model displacement, beta and fit mode.**
   Select Normal, unshifted Black or shifted Black. Normal uses
   absolute annual rate volatility and may carry an explicitly
   declared model displacement. Black uses relative annual volatility
   and requires absent displacement. Shifted Black requires the exact
   finite signed quote displacement, including explicit zero, with
   positive effective forward and strikes. Require literal fixed beta
   and explicitly elect Free or observed-ATM Pinned. A pin requires a
   unique quote at the prepared forward and the accepted checked
   bracket. Apply the selected model's checked domain.
   [Accepted SABR fit](../models-analytic/sabr-calibration.md#accepted-decision-for-explicit-checked-sabr-calibration).

3. **Gate each fitted smile in native quote units.** Reuse the checked
   nine-start vega-weighted relative fit with explicit bounds and
   solver controls. Require every quote to pass a positive finite
   absolute native volatility cap before a candidate can win.
   Retire the default `.0015` and the cap divided by minimum quote.
   A failed required smile blocks publication.

4. **Use genuine fitted support to construct requested nodes.**
   Preserve complete fitted nodes on exact reference matches. Use
   checked convex arithmetic on alpha, nu and rho with the actual
   adjacent fitted corners, one common beta and the declared
   displacement. An exact axis uses only its own index. Reject
   out-of-range targets and missing active corners. Retire geometric
   floors, rho caps and corner substitutions. Pair blended parameters
   with the new node's own checked dated forward under 013.
   [Accepted parameter interpolation](../models-analytic/volatility-source-interpolation.md#accepted-decision-for-explicit-volatility-source-evaluation-and-materialization).

5. **Declare the delivered cube's evaluation mode.** Publish the
   canonical parameter and forward grid with explicit `ParameterSpace`
   or `TotalVariance` mode. Total-variance evaluation combines quotes
   and yields no parameter tuple. Each evaluation selects its output
   convention explicitly. Consumers may elect a supported clamp policy.
   Construction and delivered-fit acceptance use strict lookup.
   [Accepted SABR source contract](../core-market/sabr-cube-data.md#accepted-decision-canonical-sabr-sources-and-explicit-evaluated-quote-meaning).

6. **Replay every consumed quote through the delivered cube.** Use its
   actual checked evaluator, exact coordinates, qualified strike and
   forward basis, native quote law and applicable displacement. Require
   each finite signed residual to satisfy its positive effective native
   tolerance, including equality. An omitted fitted node is eligible
   only when the delivered forward basis is qualified and strict replay
   passes. Preserve fit rows in authored order.
   [Accepted fit reporting](../calibration-framework/14-fit-reports.md#accepted-decision-for-fit-acceptance-and-report-semantics).

7. **Publish atomically with fit evidence.** Preserve the selected quotes,
   dated references, controls, fitted parameters, construction forwards,
   signed residuals, typed failures and actual work in Rust-owned state.
   Exploratory failures can retain diagnostics. Only an admitted cube
   may enter the market, serve dependencies or qualify for replay.
   Arbitrage diagnostics require a separately qualified swaption profile.
   [Accepted execution gate](../calibration-framework/03-dependencies-and-execution.md#accepted-decision-for-calibration-dependencies-preflight-and-ordered-execution) ·
   [Accepted diagnostic scope](../calibration-framework/17-surface-validation.md#accepted-decision-for-surface-validation-and-publication).

## Independent calculation checks

- An annual Normal volatility error of `.00003` on a `.004`
  quote passes the observed relative proxy `.00001/.001` but
  exceeds the native `.00001` cap.
- The arithmetic midpoint of nu `0` and `.20` is `.10`.
  The observed `1e-16` floor and geometric interpolation give
  approximately `4.472136e-9` at that midpoint.
- At one-year expiry with common forward equal to strike,
  Normal SABR with beta and rho zero and nu `.6` gives
  ATM sigma `1.03 × alpha`. Local ATM vols of `.005`,
  `.040` and `.020` at tenors 1, 3 and 5 years yield
  `.0125` at the middle tenor under parameter-space interpolation
  of only the outer delivered nodes. The native residual is
  `−.0275`, or −275 bp per square root year.

## Observed choice and domain at the audit revision

Fit fixed-beta SABR independently by quoted expiry/tenor bucket and publish a parameter cube with a stored forward at every target node. Normal, Black and shifted Black contracts are explicit; there is no automatic interpretation of bp as decimal vol. Pricing evaluator behavior is owned by [model volatility interpolation](../models-analytic/volatility-source-interpolation.md) and [SABR kernels](../models-analytic/sabr-calibration.md).

## Observed algorithm and controls

1. Group swaption quotes by `(round(expiry*10000),round(tenor*10000))` cast to unsigned integers. Use exact contractual times for models and retained axes, but a representative quote supplies a bucket's forward. Mixed convention IDs in one rounded bucket error. Nonpositive expiries are not silently filtered as in equity targets; shared fit/domain checks can fail them.
2. Quoted vols must be finite nonnegative decimal units and have exactly the quote type implied by plan convention. Require at least three strikes per bucket. Default absolute fit tolerance is `.0015` quoted-vol units, required positive/finite. Convert to shared relative fitting tolerance by dividing by minimum quote; final published-cube errors remain absolute and unscaled by vega.
3. Normal dispatch forces beta=0 and enables ATM pinning. Black uses configured beta (default .5) and explicitly no shift. Shifted Black requires finite positive fixed displacement and fits using that shift; shared SABR checks shifted forward/strike domains. Each fit receives global max iterations. Failure is saved per bucket and other buckets continue; success retains parameters, exact axes, calibration forward and diagnostics.
4. Target nodes first use the rounded exact-key lookup. Otherwise bracket exact successful expiry/tenor axes, reject outside bounds by default or clamp when configured. Interpolate alpha and nu in log space after flooring endpoints at `1e-16`; rho linearly then clamp to `[-.999,.999]`; beta and shift inherit from the lower anchor. Bilinear interpolation is two successive 1D interpolations. Exact zero nu survives direct node lookup but becomes a positive floor in interpolation.
5. Missing-corner fallback defaults false. If enabled, lower-left remains required; missing high-expiry/low-tenor and low-expiry/high-tenor use lower-left; missing high/high uses high-expiry/low-tenor after its fallback. The analogous 1D missing upper point uses lower. A missing required lower corner or unavailable target errors. Clamp counters and interpolated counts are reported.
6. Exact fitted target keys reuse their calibration forward; interpolated nodes recalculate index-default synthetic forwards [013](13-swaption-forwards.md). Construct expiry-major SabrParameterData/forward grids and VolCube. No cross-expiry arbitrage projection or generic equity-surface diagnostics are invoked here.
7. Re-evaluate **every original swaption quote through the published cube** with the clamped evaluator in Normal or Lognormal convention. Nonfinite evaluation errors. Report max absolute quoted-vol error versus the fit tolerance. Any saved failed bucket forces success/validation false even if fallback made the requested cube constructible; no silent partial-fit success. Framework execution policy determines whether a failed report may be returned.

## Observed limits and alternatives

Rounding at 1e-4 years can coalesce nearby distinct contracts and target key lookup can reuse a nearby bucket rather than interpolate. Exact axes do not remove that lookup convention. Parameter interpolation is not total-variance interpolation and does not guarantee calendar or butterfly absence. Lower-anchor beta/shift is safe only under the plan's common convention/fixed-beta assumptions. Missing-corner substitutions are directional, not nearest-neighbor distance minimization. A sparse target grid can discard a successful interior smile and fail artifact acceptance.

Alternatives include quote-space/total-variance cube interpolation, globally constrained SABR terms, explicit interpolated forwards or full-contract reference grids.

## Evidence

- Owning [swaption.rs](../../../../finstack-quant/calibration/src/targets/swaption.rs) fully inspected; [schema](../../../../finstack-quant/calibration/src/api/schema.rs) establishes defaults.
- Located inline tests for decimal quote types, normal ATM pinning, explicit shift, failed-bucket report, discarded interior smile and materialized coordinates. The inspected bilinear test checks positivity/rho bounds but does not assert a full independent numeric parameter grid. [Integration tests](../../../../finstack-quant/calibration/tests/calibration/swaption_vol.rs) locate Error/Clamp and pricing-type behavior.
- [Hagan SABR reference](../../../REFERENCES.md#hagan-2002-sabr) supports model provenance only.

No calibration execution was performed.

## Relationship to the accepted SABR source boundary

[CORE-MARKET-16's complete accepted package](../core-market/sabr-cube-data.md#accepted-decision-canonical-sabr-sources-and-explicit-evaluated-quote-meaning)
requires published canonical parameter/forward nodes, explicit cube mode and
actual expiry clock/reference association. Retain fitted-node forwards and
actual quote/contract construction provenance; interpolated synthetic contracts
must be explicitly qualified under [record 013](13-swaption-forwards.md#accepted-decision-for-dated-swaption-coordinates-and-calibration-forwards).
Rechecks select the actual output convention and checked evaluator. Exact
node admission preserves nu equal to zero. The accepted calibration package
above supplies the producer's fit, construction and publication policies.

## Follow-up tracking

Adoption, reconciliation and qualification work is tracked in the
[calibration-target follow-ups](../FOLLOW_UPS.md#calibration-quotes-and-curvesmile-targets).
