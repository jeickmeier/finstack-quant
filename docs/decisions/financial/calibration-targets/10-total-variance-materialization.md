# CALIBRATION-TARGETS-010 — Expiry interpolation and published surface acceptance

- **Decision status:** Draft — observed implementation; adoption pending.
- **Implementation status:** Observed for the stated source scope; behavior and scope limitations are described below.
- **Validation status:** Source and selected evidence inspected or located; not executed.
- **Audit date / source revision:** 2026-11-05 / `7e82fd03ad98346727769e88dd5ac5d84b56e9fc`. Scope and evidence boundaries: [README](README.md).

## Choice and equation

Equity SABR and SVI fit individual quoted expiries, then materialize a requested expiry×absolute-strike grid by interpolating **total variance**. At target absolute strike K, evaluate each source smile at its own forward `F(Ti)` and own expiry, giving `wi(K)=sigma_i(F(Ti),K,Ti)^2*Ti`. For `T1<T<T2`, use `w(T,K)=(1-u)*w1+u*w2`, `u=(T-T1)/(T2-T1)`, and publish `sigma(T,K)=sqrt(w/T)`.

This preserves the source-slice strike meaning. It does not interpolate SABR/SVI parameter vectors across equity expiries. It only preserves nondecreasing total variance when the source values at that fixed K are already ordered; the method itself does not repair crossing smiles or prove global absence of calendar/butterfly arbitrage.

## Ordered behavior and controls

1. Require positive target expiry and nonempty fitted-slice map. Find before/after neighbors in ordered expiry map. If distinct by more than `1e-12`, linearly blend variance; coincident/single-neighbor cases use that source variance.
2. Equity SABR obeys `expiry_extrapolation`: default Error rejects target outside fitted interval; Clamp takes nearest source **total variance**, then divides by target T. Thus a clamped short-end vol grows like `T^(-1/2)`; it is not flat volatility. SVI hardcodes Clamp and exposes no expiry-tail policy in its params.
3. Reject nonfinite/negative resulting variance. SVI additionally rejects nonpositive final vol; SABR materialization allows zero at this helper level, subject to its kernel/surface validation. Output grid uses `VolInterpolationMode::TotalVariance`, default Black convention and absolute Strike axis. Core surface shape/domain validation still applies.
4. Final quote residuals are computed through the **published VolSurface** at each live matching equity quote's ACT/365F expiry/strike, not merely against stored fitted slices. Residual is fitted minus quoted decimal volatility. Duplicate IDs would overwrite map entries here; public envelope identity validation is a separate responsibility.
5. Compute target-grid forwards and run framework surface diagnostics; report fit tolerance is the vol-surface budget (default .001), independent of numerical solver tolerance. [Framework report/validation policy](../calibration-framework/17-surface-validation.md) controls marking/rejection. Source-slice parameters are retained in reports for provenance.

## Alternatives and limitations

Flat-vol, flat-forward-variance or model-consistent tails have different long/short-horizon behavior. Interpolating at fixed forward moneyness instead of absolute strike changes the modeled object; it is not interchangeable. A coarse requested grid can materially change an otherwise accurately fitted source smile and fail the final quote budget. Reported fit to known quotes does not validate unquoted wing/expiry behavior.

- Owning [util.rs](../../../../finstack-quant/calibration/src/targets/util.rs), [SABR surface](../../../../finstack-quant/calibration/src/targets/vol.rs) and [SVI surface](../../../../finstack-quant/calibration/src/targets/svi.rs) inspected.
- Inspected SVI flat-total-variance test checks `sqrt(.04/.75)` directly. SABR knot and Error/Clamp cases are located. The nonlinear varying-forward SVI calendar tests and [published-surface tests](../../../../finstack-quant/calibration/tests/calibration/svi_surface.rs) are located invariant/same-kernel evidence, not a market arbitrage certificate.
- [Gatheral surface reference](../../../REFERENCES.md#gatheral-volatility-surface) is model provenance, not proof of these tail choices.

## Relationship to the accepted published-grid boundary

[CORE-MARKET-14's complete accepted package](../core-market/volatility-grids-and-shocks.md#accepted-decision-explicit-annual-volatility-sources-and-exact-grid-shocks) requires the materialized
annual-sigma grid to carry explicit actual quote/axis/interpolation choices,
compatible displacements and truthful source-clock metadata from dated
producers. Remove inherited Black/Strike defaults and zero-expiry anchors;
TotalVariance is a working quantity, not stored W or an arbitrage certificate.
Reconcile stable sigma reconstruction and full exact-node/source compatibility.
This target's fit, expiry interpolation and tail choices above remain Draft;
record14's complete source package was accepted on **2026-11-08**, with runtime
reconciliation and independent qualification pending.

## Follow-up tracking

Adoption, reconciliation and qualification work is tracked in the
[calibration-target follow-ups](../FOLLOW_UPS.md#calibration-quotes-and-curvesmile-targets).
