# VALUATIONS-STRUCTURED-CREDIT-21: Risk bumps, breakeven default and scenario grids

- **Decision status:** Accepted 2026-11-15 — complete yield, curve, spread and collateral risk, breakeven default and scenario grid package.
- **Implementation status:** Observed in the owning paths below. Reconciliation to the accepted contract is pending.
- **Validation status:** Independent curve stencil, fixed-spread, recovery, breakeven and scenario-scope calculations checked. Production repricing, convergence, monotone model qualification and host qualification remain pending.
- **Audit date:** 2026-11-05.
- **Source revision:** `7e82fd03ad98346727769e88dd5ac5d84b56e9fc`; runtime source unchanged by this audit.

## Accepted decision for structured-credit risk and scenarios

1. **Calculate yield risk on the frozen buyer strip.** Use record 20's
   entitled cash, quote times and explicit nominal periodic yield `y`
   and frequency `m`. With `b=1+y/m`, take the PV-weighted moments
   of `t`, `t/b`, and `(t/b)^2+t/(m b^2)` for Macaulay duration,
   modified duration and unscaled convexity, each normalized by positive
   dirty PV. Yield DV01 is the signed currency derivative per upward
   bp, `−0.0001 Σ[(t/b) × PV]`. Entitled settlement cash contributes
   at `t=0`. Remove discount-curve bumps from yield risk.
   [Accepted yield-risk precedent](../valuations-bonds/14-yield-risk.md#accepted-decision-for-checked-quoted-yield-bond-risk) ·
   [Accepted structured-credit quotes](20-quotes-and-inverse-pricing.md#accepted-decision-for-structured-credit-settlement-quotes-and-inversion).

2. **Reprice the waterfall for live curve risk.** Identify actual active
   discount and projection sources and require explicit bump sizes.
   Parallel DV01 moves selected sources together. Key-rate DV01 directly
   reprices one exact source and triangular key shock at a time. Report
   signed PV change per actual bp width. Effective duration and
   convexity use the base-centered three-point stencil at actual up and
   down widths, including unequal widths. Require positive base PV for
   normalized risk and report unscaled convexity in years squared.
   Preserve history, holder claim and any qualified held spread.
   Rebuild affected projected cash and decisions. Report key-rate
   sums separately from simultaneous parallel repricing.
   [Accepted curve-risk precedent](../valuations-bonds/15-curve-risk.md#accepted-decision-for-checked-bond-curve-and-effective-risk) ·
   [FHFA on effective duration](https://www.fhfa.gov/sites/default/files/2023-05/Investment_Portfolio_Management_Module_Final_Version_1_0-508.pdf).

3. **Give fixed Z risk and par-credit risk explicit experiments.** On
   record 20's fixed buyer strip use an authorized explicit Z, a
   checked price-implied Z or an expressly elected zero-Z basis.
   Under the instrument key, report signed `+1 bp` price change.
   Spread duration is the negative change over `0.0001 × dirty PV`.
   For continuous Z, spread convexity is `Σ(PV_i × τ_i²)/dirty PV`
   on native spread time. Require positive PV for both normalized
   measures. For requested cash bands, assign each positive-time
   contribution to the first covering upper bound, with the last
   band including the tail. Identify the basis as cash bands.
   Par-credit CS01 requires an active named source, exact quote IDs
   and checked recalibration and repricing. Propagate replay failure.
   [Accepted credit stencils and cash bands](../valuations-framework/credit-sensitivities.md).

4. **Bump the operative default and prepayment laws.** Report signed
   currency per `+1 bp` annual effective CDR or CPR. Require explicit
   positive step and selected active sources. Shift eligible conditional
   vectors by a common amplitude and normalize by the achieved common
   width. Use both full admissible sides or one full side against base
   only when the other crosses a known coordinate bound. Propagate
   pricing failures and remove entrywise clamps and maximum-width
   normalization. Admit PSA and SDA peak coordinates where they identify
   the speed. Monthly ABS count speed and original-reference timing
   and cumulative-loss targets require separately named coordinates.
   Preserve timing shape for an approved cumulative-loss magnitude
   shock and require authored shape to shift a zero profile.
   [Standard prepayment and default benchmarks](https://www.sifma.org/wp-content/uploads/2017/08/chsf.pdf).

5. **Measure recovery and severity per percentage point.** Report signed
   currency per `+0.01` absolute recovery or LGD on the same
   full-vector and boundary rules. Move the authorized operative source,
   including primary severity entries when they supply recovery.
   Preserve known recoveries, vintages, receipt lags and unrelated
   assumptions. Elect frozen intensity or qualified quote-preserving
   refit where applicable. Remove unweighted severity averages.
   `Severity01=−Recovery01` applies on the identical complementary
   experiment.
   [Accepted recovery experiments](../valuations-framework/credit-sensitivities.md).

6. **Find a breakeven CDR threshold crossing.** Apply constant annual CDR
   to expressly selected deal-fallback exposures and retain authoritative
   asset inputs. Measure incremental post-cut note principal credit loss
   from the accepted ledger, excluding opening or noncredit loss.
   Require explicit native Money threshold, authored CDR interval
   within `[0,1]`, positive rate tolerance, work budget and a qualified
   monotone loss profile throughout the interval. Breach means loss
   strictly above threshold. Return a typed already-breached lower
   bound, no breach in range, or safe/breached interval from bisection.
   Loss jumps can produce crossings. Literal 100% can be a breached
   rate and has no sentinel meaning.
   [Accepted loss basis](15-stochastic-waterfall.md#accepted-decision-for-stochastic-waterfall-pricing-and-estimation).

7. **Use a checked deterministic scenario grid.** Validate CPR, CDR
   and severity axes in `[0,1]` and the checked 10,000-cell cap
   before projection. Preserve caller order and deliberate empty-table
   behavior. Elect retention or authorized replacement of recovery
   timing. Shock eligible deal-fallback assumptions while preserving
   asset overrides. Report affected exposure by role, including fully
   overridden axes. Use one deterministic waterfall per cell. Report
   matched quote, WAL and incremental credit loss with honest
   availability. Publish the complete ordered grid or the indexed
   failure. Grid cells are stresses with no probability weights.

8. **Use matched price contrasts for qualified stochastic risk.** Preserve
   the selected stochastic waterfall or OAS route, complete independent
   groups, matched logical draws and any qualified held spread.
   Verify alignment after changed events or grids. Average antithetic
   legs within each group before estimating uncertainty on paired
   price differences. Identify contrasts across trigger or call state
   changes as finite scenarios. Reconcile the actual step, coordinate,
   source, claim, units and availability across Rust, Python, WASM
   and stored results.
   [Accepted OAS](17-oas-scenarios.md#accepted-decision-for-structured-credit-oas-and-settlement-spread-fitting).

## Independent calculation checks

- Holder PV 100.150075, 100 and 99.7003 at −5, 0 and +10 bp gives
  DV01 −0.029985, effective duration 3 and effective convexity 6.
- Fixed-strip PV cash 5 at time zero, 30 at 0.5 years and 66 at
  2 years gives +1 bp Z price change −0.01469864, spread duration
  1.455311 and analytic spread convexity 2.688119.
- Recovery entries 0 and 0.60 with discounted defaulted exposures 10
  and 72 give 0.82 per percentage point under a common full upward
  move. Entrywise clipping and average-width normalization give 1.0267.
- The loss law `max(0,60 × CDR−20)` with threshold 1 is safe at
  35% CDR. Fourteen bisections on `[0,1]` give safe and breached
  endpoints 34.99756% and 35.00366%. In a separate 100-par pool
  with 40 at authoritative CDR 10% and 60 using the fallback, moving
  only the fallback from 10% to 25% raises gross default from 10 to 19.

## Observed implementation and scope

Risk metrics use several distinct experiments. Fixed-cashflow duration/convexity measures discount sensitivity; effective summary metrics rebuild cashflows under shifted discount and forward curves; credit-model bumps rerun the deal. Sharing the label “duration” does not make these methods equivalent.

## Algorithm and numerical conventions

Macaulay duration is PV-weighted Act/365F payment time. Modified duration uses a one-sided discount bump, default +1 bp, on fixed cashflows; convexity uses symmetric shifts, default ±10 bp. Both accept a finite positive `ytm_bump_bp`, with registry overrides from bump configuration. Tranche-summary effective duration/convexity shifts all discount and forward curves by ±1 bp and reruns the projection, including floating/reset/trigger effects. Registered DV01 delegates to the common combined-curve calculator; key-rate DV01 uses triangular buckets.

Z-spread CS01 is P&L from a +1 bp spread shift. Spread duration/convexity use fixed-flow discount-spread sensitivity. Bucketed CS01 distributes cashflow sensitivity linearly by payment time across buckets; it is not an independent node-by-node spread-curve repricing. Its total can reconcile while local market-curve behavior remains untested.

Default01 and Prepayment01 reproject up/down models and return `(PV_up - PV_down) / achieved_width × 0.0001`, targeting a 1 bp annual-equivalent probability change and transforming SDA/PSA peak multipliers and vectors as needed. Vector bump normalization uses maximum achieved change; cumulative-loss inputs scale their terminal loss. Constant upward model perturbations are not all capped at one at the bump boundary, so validation can reject edge cases. Recovery01 uses clamped up/down recovery changes and `(PV_up - PV_down) / achieved_width × 0.01`, one percentage point rather than one bp; Severity01 reverses recovery direction. Severity-vector effective recovery in helper calculations is an unweighted mean of severities.

Breakeven CDR bisects [0,1] under an assumed monotonic loss relation, with 1e-5 rate tolerance and loss tolerance 1e-4 of original notional. Scenario tables enumerate caller grids with a 10,000-cell cap; constant scenario defaults/prepayment replace deal models, while asset-level overrides remain active. They are stress scenarios, not a calibrated probability distribution.

## Domain, failures and alternatives

Currency/finite guards apply to returned summaries. Zero PV, zero principal and no-solution cases use method-specific errors or zeros; caller intent must choose the desired risk experiment. Supported alternatives are fixed-flow versus reprojected metrics and explicit scenario grids. Independently bumped spread curves and a uniform boundary-probability policy would be different methodologies; central credit-model bumps with achieved-width normalization are already implemented.

## Implementation and validation evidence

- [structured_credit/metrics/risk/duration.rs](../../../../finstack-quant/valuations/structured-credit/src/structured_credit/metrics/risk/duration.rs)
- [structured_credit/metrics/risk/convexity.rs](../../../../finstack-quant/valuations/structured-credit/src/structured_credit/metrics/risk/convexity.rs)
- [structured_credit/metrics/risk/spreads.rs](../../../../finstack-quant/valuations/structured-credit/src/structured_credit/metrics/risk/spreads.rs)
- [structured_credit/metrics/risk/default01.rs](../../../../finstack-quant/valuations/structured-credit/src/structured_credit/metrics/risk/default01.rs)
- [structured_credit/metrics/risk/prepayment01.rs](../../../../finstack-quant/valuations/structured-credit/src/structured_credit/metrics/risk/prepayment01.rs)
- [structured_credit/metrics/risk/recovery01.rs](../../../../finstack-quant/valuations/structured-credit/src/structured_credit/metrics/risk/recovery01.rs)
- [structured_credit/metrics/risk/severity01.rs](../../../../finstack-quant/valuations/structured-credit/src/structured_credit/metrics/risk/severity01.rs)
- [structured_credit/metrics/risk/breakeven_cdr.rs](../../../../finstack-quant/valuations/structured-credit/src/structured_credit/metrics/risk/breakeven_cdr.rs)
- [structured_credit/metrics/risk/mod.rs](../../../../finstack-quant/valuations/structured-credit/src/structured_credit/metrics/risk/mod.rs)
- [structured_credit/metrics/scenario.rs](../../../../finstack-quant/valuations/structured-credit/src/structured_credit/metrics/scenario.rs)
- [structured_credit/metrics/summary.rs](../../../../finstack-quant/valuations/structured-credit/src/structured_credit/metrics/summary.rs)
- [structured_credit/metrics/mod.rs](../../../../finstack-quant/valuations/structured-credit/src/structured_credit/metrics/mod.rs)

Existing evidence: [structured_credit/unit/metrics/risk_tests.rs](../../../../finstack-quant/valuations/structured-credit/tests/structured_credit/unit/metrics/risk_tests.rs); [structured_credit/test_bucketed_dv01.rs](../../../../finstack-quant/valuations/structured-credit/tests/structured_credit/test_bucketed_dv01.rs); [structured_credit/unit/config_sensitivity_tests.rs](../../../../finstack-quant/valuations/structured-credit/tests/structured_credit/unit/config_sensitivity_tests.rs). These are behavior/formula/invariant tests; no test was rerun for this audit. Where only a test location was inspected, no independent reference validation is claimed.

Convergence and independent economic risk references were not established.

## Follow-up tracking

Implementation reconciliation and validation work are tracked in
the [structured-credit follow-ups](../FOLLOW_UPS.md#structured-credit).
