# CALIBRATION-TARGETS-010 — Expiry interpolation and published surface acceptance

- **Decision status:** Accepted 2026-11-15 — complete equity surface materialization and delivered-grid acceptance package.
- **Implementation status:** Observed. Reconciliation to the accepted package is pending.
- **Validation status:** Independent tail, stable variance, delivered quote and fixed-moneyness calendar checks verified. Production smile calibration, market references, restoration and host qualification remain pending.
- **Audit date / source revision:** 2026-11-05 / `7e82fd03ad98346727769e88dd5ac5d84b56e9fc`. Scope and evidence boundaries: [README](README.md).

## Accepted decision for explicit total variance materialization and delivered surface fit

1. **Publish an explicit dated grid of annual Black volatility.** Require
   an output ID, strictly ordered positive expiries and absolute strikes,
   exact fixing dates and phases, the actual volatility clock, and one
   qualified forward per output expiry. Reject expiry collisions and
   incomplete grids. Select unshifted Black, Strike and TotalVariance
   explicitly. Store annual sigma. Zero sigma is valid grid data when
   the fitted model permits it. Normal and shifted Black require their
   own qualified materialization and diagnostics.
   [Accepted volatility grids](../core-market/volatility-grids-and-shocks.md#accepted-decision-explicit-annual-volatility-sources-and-exact-grid-shocks) ·
   [Accepted dated equity smile forwards](09-equity-forward-inputs.md#accepted-decision-for-explicit-equity-spot-carry-and-dated-smile-forwards).

2. **Interpolate total variance at fixed absolute strike.** At an exact
   fitted expiry, evaluate that slice directly at the requested strike.
   Between fitted expiries, evaluate both adjacent smiles at that same
   strike on their own expiries and frozen forwards. Blend `Tᵢσᵢ²`
   linearly in expiry and restore annual sigma using requested `T`
   and checked scaled arithmetic. Retire the `1e−12` expiry merge
   and epsilon denominators. Preserve exact nodes and propagate
   model-domain and arithmetic errors. The delivered grid uses the
   accepted grid interpolator between its own nodes.
   [Accepted volatility interpolation](../models-analytic/volatility-source-interpolation.md#accepted-decision-for-explicit-volatility-source-evaluation-and-materialization).

3. **Elect the fitted-slice tail separately from published lookup.**
   Require both SABR and SVI to elect `Error` or `FlatTotalVariance`
   for materialization outside fitted expiries. Flat total variance
   holds boundary variance and restores sigma using requested expiry.
   Published-grid coordinate `Clamp` instead uses effective boundary
   expiry and returns the boundary volatility at the same strike.
   Retire the SABR default and SVI's hard-coded tail.
   [Accepted stored-grid lookup](../models-analytic/volatility-source-interpolation.md#accepted-decision-for-explicit-volatility-source-evaluation-and-materialization).

4. **Gate every original quote on the delivered grid.** Keep prepared
   quote IDs, dates and strikes in authored order. Require strict
   coverage on both axes, with checked interpolation inside the
   grid. Report signed delivered-minus-observed annual sigma and
   apply an explicit positive native tolerance to each row, including
   zero-weight rows. Keep the source-slice fit distinct from delivered
   quote acceptance.
   [Accepted fit reporting](../calibration-framework/14-fit-reports.md#accepted-decision-for-fit-acceptance-and-report-semantics).

5. **Apply elected diagnostics to the exact delivered artifact.** Check
   same-expiry Black call bounds, vertical spreads and strike-spacing
   convexity under the accepted policy. Calendar checks use fixed
   `K/F` over common support and require qualified deterministic
   discounting and proportional carry. An absolute cash-dividend
   schedule makes that calendar test inapplicable. Required gaps
   block publication and advisory gaps remain incomplete. Keep
   fitted-SVI density findings separate. Surface repair requires an
   explicit operation.
   [Accepted surface validation](../calibration-framework/17-surface-validation.md#accepted-decision-for-surface-validation-and-publication) ·
   [Gatheral and Jacquier on calendar conditions](https://arxiv.org/pdf/1204.0646).

6. **Publish the grid and its construction evidence together.** Preserve
   fitted slice parameters, dates, forwards, volatility convention and
   clock, selected tail and lookup policies, exact grid, final quote
   rows and diagnostic coverage through the existing Rust-owned
   interfaces and restoration. Records 011 and 012 own the
   model-specific fit domains, minimum distinct strikes and solver
   choices.
   [Equity SABR](11-equity-sabr.md) ·
   [Equity SVI](12-equity-svi.md).

## Independent calculation checks

- A 20% one-year volatility gives 40% at three months and 10% at
  four years under flat total variance. Clamping a lookup to the
  same one-year grid row returns 20% at the same strike.
- At a midpoint between equal annual sigmas of `1e−200`, with
  source expiries one and three and requested expiry two, the
  annual sigma remains `1e−200`. Squaring first loses the
  input variance in binary64.
- A fitted 20% quote at strike 100 can reprice to `√.05`,
  about `22.3607%`, when the delivered expiry row holds only
  strike 90 at 10% and strike 110 at 30%. The resulting native
  sigma difference exceeds `.001`.
- For expiries one and two, forwards 100 and 120, and strikes
  80, 100 and 120, identical total-variance rows
  `(.0625,.0484,.04)` pass sampled Black call checks. Variance
  at `K/F = 1` decreases from `.0484` to `.04`, despite
  equality at each fixed strike.

## Observed choice and equation at the audit revision

Equity SABR and SVI fit individual quoted expiries, then materialize a requested expiry×absolute-strike grid by interpolating **total variance**. At target absolute strike K, evaluate each source smile at its own forward `F(Ti)` and own expiry, giving `wi(K)=sigma_i(F(Ti),K,Ti)^2*Ti`. For `T1<T<T2`, use `w(T,K)=(1-u)*w1+u*w2`, `u=(T-T1)/(T2-T1)`, and publish `sigma(T,K)=sqrt(w/T)`.

This preserves the source-slice strike meaning. It does not interpolate SABR/SVI parameter vectors across equity expiries. It only preserves nondecreasing total variance when the source values at that fixed K are already ordered; the method itself does not repair crossing smiles or prove global absence of calendar/butterfly arbitrage.

## Observed ordered behavior and controls

1. Require positive target expiry and nonempty fitted-slice map. Find before/after neighbors in ordered expiry map. If distinct by more than `1e-12`, linearly blend variance; coincident/single-neighbor cases use that source variance.
2. Equity SABR obeys `expiry_extrapolation`: default Error rejects target outside fitted interval; Clamp takes nearest source **total variance**, then divides by target T. Thus a clamped short-end vol grows like `T^(-1/2)`; it is not flat volatility. SVI hardcodes Clamp and exposes no expiry-tail policy in its params.
3. Reject nonfinite/negative resulting variance. SVI additionally rejects nonpositive final vol; SABR materialization allows zero at this helper level, subject to its kernel/surface validation. Output grid uses `VolInterpolationMode::TotalVariance`, default Black convention and absolute Strike axis. Core surface shape/domain validation still applies.
4. Final quote residuals are computed through the **published VolSurface** at each live matching equity quote's ACT/365F expiry/strike, not merely against stored fitted slices. Residual is fitted minus quoted decimal volatility. Duplicate IDs would overwrite map entries here; public envelope identity validation is a separate responsibility.
5. Compute target-grid forwards and run framework surface diagnostics; report fit tolerance is the vol-surface budget (default .001), independent of numerical solver tolerance. [Framework report/validation policy](../calibration-framework/17-surface-validation.md) controls marking/rejection. Source-slice parameters are retained in reports for provenance.

## Observed alternatives and limitations

Flat-vol, flat-forward-variance or model-consistent tails have different long/short-horizon behavior. Interpolating at fixed forward moneyness instead of absolute strike changes the modeled object; it is not interchangeable. A coarse requested grid can materially change an otherwise accurately fitted source smile and fail the final quote budget. Reported fit to known quotes does not validate unquoted wing/expiry behavior.

- Owning [util.rs](../../../../finstack-quant/calibration/src/targets/util.rs), [SABR surface](../../../../finstack-quant/calibration/src/targets/vol.rs) and [SVI surface](../../../../finstack-quant/calibration/src/targets/svi.rs) inspected.
- Inspected SVI flat-total-variance test checks `sqrt(.04/.75)` directly. SABR knot and Error/Clamp cases are located. The nonlinear varying-forward SVI calendar tests and [published-surface tests](../../../../finstack-quant/calibration/tests/calibration/svi_surface.rs) are located invariant/same-kernel evidence, not a market arbitrage certificate.
- [Gatheral surface reference](../../../REFERENCES.md#gatheral-volatility-surface) is model provenance, not proof of these tail choices.

## Follow-up tracking

Reconciliation and qualification work is tracked in the
[calibration-target follow-ups](../FOLLOW_UPS.md#calibration-quotes-and-curvesmile-targets).
