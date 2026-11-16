# SCENARIOS-009: Volatility bucket selection and post-shock diagnostics

- Decision status: **Accepted 2026-11-15 — complete exact volatility bucket, native sigma shock and separate diagnostics package.**
- Implementation status: **Observed.** Reconciliation to the accepted package is pending.
- Validation status: **Independent selective scaling, zero and range boundaries, and fixed-forward-moneyness comparison calculations verified.** Production execution, restoration and host qualification remain pending.
- Audit date: **2026-11-05**. Source revision: `7e82fd03ad98346727769e88dd5ac5d84b56e9fc`.
- Scope: [inventory](README.md), [source coverage](coverage.json). Shared kernels are linked as dependencies, not reassigned implementations.

## Accepted decision for exact scenario volatility grid shocks

1. **Bind the exact `VolSurface`.** Use one checked Rust path for
   parallel, bucket and retained hierarchy actions. A same-ID
   volatility cube or FX-delta source can coexist without blocking
   the selected surface. Missing and wrong-kind targets fail even
   for a zero shock.
   [Accepted typed market lifecycle](../core-market/context-lifecycle.md#accepted-decision-typed-targets-and-atomic-context-lifecycle) ·
   [Accepted scenario hierarchy](03-hierarchy-resolution.md#accepted-decision-for-typed-scenario-hierarchy-expansion-and-overlap).

2. **Resolve exact typed bucket coordinates.** Admit native expiry
   coordinates or actual dates mapped once on the admitted source
   clock. A tenor first resolves to a date under an explicit origin
   and roll convention. Keep the calibration origin fixed. The
   secondary filter declares `Strike` or `Tenor` and uses its
   exact native coordinates. Take the Cartesian product. `None`
   selects the full axis and an explicit empty list selects none.
   Validate both filters, their axis meaning, and every supplied
   coordinate before any no-effect shortcut. Reject unknown,
   nonfinite or duplicate resolved coordinates.
   [Accepted exact grid selection and source clock](../core-market/volatility-grids-and-shocks.md#accepted-decision-explicit-annual-volatility-sources-and-exact-grid-shocks) ·
   [Accepted tenor arithmetic](../core-conventions/007-tenor-date-arithmetic.md#accepted-decision-tenor-and-date-arithmetic-conventions).

3. **Scale stored annual sigma once and publish the admitted candidate.**
   Require finite relative percent `p ≥ −100` and apply its checked
   factor. Literal `−100` gives zero. Near that boundary use
   `(100+p)/100` to preserve the positive factor. Reject overflow
   and a positive product lost to zero. Preserve unselected quotes,
   existing zero quotes, surface ID, axes, quote convention, source
   clock, interpolation and row displacements. Both interpolation
   modes use the same stored-sigma change. Publish the same checked
   candidate used for effect evidence.
   [Accepted native sigma shocks](../core-market/volatility-grids-and-shocks.md#accepted-decision-explicit-annual-volatility-sources-and-exact-grid-shocks) ·
   [Accepted scenario execution](02-execution-and-partial-success.md#accepted-decision-for-ordered-and-atomic-scenario-execution).

4. **Keep diagnostics separately elected.** Retire the automatic
   fixed-strike calendar scan and hardcoded `−50%` warning.
   Explicit diagnostics use the accepted checker on the delivered
   artifact under its supported quote convention, forwards and
   carry assumptions. Preserve typed findings and sampled coverage.
   Publish the admitted shock without repair or recalibration.
   [Accepted surface diagnostics](../models-analytic/surface-arbitrage-diagnostics.md#accepted-decision-for-checked-surface-arbitrage-diagnostics-and-coverage) ·
   [Gatheral and Jacquier on fixed forward moneyness](https://arxiv.org/pdf/1204.0646#page=3).

5. **Report the actual stored-quote effect.** Preserve the authored
   percentage, represented factor, resolved cells and their native
   sigma before and after. Distinguish intentional empty selection,
   unchanged results and failed actions. Keep historical fit
   evidence and require fresh qualification of a changed grid
   before replay. Record 017 owns the report representation.
   [Reporting record 017](17-reports-and-replay.md).

## Independent calculation checks

- On the expiry-major grid `(.20,.30,0,.40)`, a `−100%` shock
  to only the second cell gives `(.20,0,0,.40)`. The original
  zero remains intact. The smallest positive binary64 sigma
  multiplied by `.5` rounds to zero and fails admission.
- At the represented percentage immediately above `−100`,
  `(100+p)/100` gives about `1.4210854715e−16`. Two successive
  `+10%` actions on sigma `.20` give `.242`.
- With forwards `100` and `120`, identical total-variance
  profiles `W=.02+.03 K/F` give `.050` and `.045` at strike
  `100`. At matched forward moneyness `K/F=1`, both values
  are `.050`.

## Observed choice and rationale at the audit revision

Observed choice: shock stored volatility quotes by a relative percentage while retaining the original surface and quote metadata. Parallel and unfiltered bucket operations deliberately share the same core kernel. The adapter previews the same operation it will execute, rejects nonpositive grid volatility, and records calendar-total-variance warnings without repairing the surface.

## Domain, ordering and controls

1. Look up the exact `VolSurface`. A finite percentage is required. Emit a large-negative-shock warning when `pct<=-50`, regardless of whether the filters eventually select a cell.
2. Convert requested tenor strings from scenario as-of using Act365F, the context calendar, and `Unadjusted`. Snap each time to its nearest surface expiry only if the distance is at most `max(0.02,0.02*grid_expiry)` years; otherwise error. The second axis is passed through as `strikes`, even for a surface whose axis represents another coordinate such as rate tenor.
3. Core bucket matching uses `1e-9*max(1,abs(grid_coordinate))` tolerance for expiry and secondary-axis coordinates. `None` selects all; an empty list selects none. Unmatched finite strikes do not interpolate or snap and can produce a no-op with no no-match warning.
4. For selected cells, preview `vol'=max(vol*(1+pct/100),0)` through the core kernel. Overflow/nonfinite input errors. Re-evaluate every native grid cell through the model's surface resolver.
5. Reject any grid volatility `<=0`, including preexisting zero values outside the targeted bucket. For each second-axis coordinate, scan increasing expiries and warn if `vol²*expiry` decreases by more than absolute `1e-8`. Invalid expiry order produces a calendar warning. A successful preview emits the matching `MarketBump::VolBucketPct`; later execution uses the same kernel.

Thus a shock of -100% that hits a positive node becomes zero in the core preview and is rejected by the scenario adapter; a shock with empty/unmatched filters can leave the surface unchanged and still count as an applied effect.

The live kernel already admits intentional zero and resolves both
axes exactly. The live scenario still prechecks other same-ID source
kinds, previews a separate candidate, and performs the fixed-strike
comparison for unshifted Black strike surfaces.

## Limitations and alternatives

The calendar diagnostic is a fixed-grid squared-quote-time check. It does not inspect butterfly convexity, density, forward/strike normalization, moneyness movement, displaced-Black shift changes, or cross-model arbitrage. A `VolSurface` can carry normal, lognormal or shifted quote conventions; the same diagnostic code does not establish a universal no-arbitrage theorem for all of them. No interpolation repair, recalibration of SVI/SABR, or dynamic smile response occurs.

Alternatives are absolute vol-point moves, log-vol moves, calibrated parameter shocks or explicit no-arbitrage projection. They preserve different quantities and can change stress severity. The observed tolerance and -50% threshold are implementation controls, not market-calibrated evidence.

## Evidence

Owning source: [adapters/vol.rs](../../../../finstack-quant/scenarios/src/adapters/vol.rs); shared [volatility grids/shocks](../core-market/volatility-grids-and-shocks.md). Inspected unit tests compare parallel and unfiltered bucket shocks bit-for-bit, reject zeroing parallel/bucket shocks, flag a post-bump calendar variance decrease, and check invalid expiry order. [Bucket tests](../../../../finstack-quant/scenarios/tests/shocks/bucket_filtering_test.rs) locate snap/filter and preview/execution consistency checks. These are algorithmic invariants, not independent smile-arbitrage certification.

## Relationship to the accepted exact sigma-shock boundary

[CORE-MARKET-14](../core-market/volatility-grids-and-shocks.md#accepted-decision-explicit-annual-volatility-sources-and-exact-grid-shocks)
owns the native sigma source and checked replacement kernel. This
scenario selects relative-percent input and uses the source's
exact stored-coordinate and nonnegative-sigma contract. Actual
pricing methods own their zero-limit and dated pricing support.

## Follow-up tracking

Implementation reconciliation and validation work are tracked in the
[scope follow-ups](../FOLLOW_UPS.md#scenarios).
