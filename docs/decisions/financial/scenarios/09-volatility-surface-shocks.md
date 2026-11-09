# SCENARIOS-009: Volatility bucket selection and post-shock diagnostics

- Decision status: **Draft — observed implementation; adoption pending.**
- Implementation status: **Observed for the stated source scope.** Actual discrepancies and unsupported cases are distinguished below.
- Validation status: **Evidence inspected/located, not executed.** No numerical test, build or external benchmark run is claimed.
- Audit date: **2026-11-05**. Source revision: `7e82fd03ad98346727769e88dd5ac5d84b56e9fc`.
- Scope: [inventory](README.md), [source coverage](coverage.json). Shared kernels are linked as dependencies, not reassigned implementations.

## Choice and rationale

Observed choice: shock stored volatility quotes by a relative percentage while retaining the original surface and quote metadata. Parallel and unfiltered bucket operations deliberately share the same core kernel. The adapter previews the same operation it will execute, rejects nonpositive grid volatility, and records calendar-total-variance warnings without repairing the surface.

## Domain, ordering and controls

1. Look up the exact `VolSurface`. A finite percentage is required. Emit a large-negative-shock warning when `pct<=-50`, regardless of whether the filters eventually select a cell.
2. Convert requested tenor strings from scenario as-of using Act365F, the context calendar, and `Unadjusted`. Snap each time to its nearest surface expiry only if the distance is at most `max(0.02,0.02*grid_expiry)` years; otherwise error. The second axis is passed through as `strikes`, even for a surface whose axis represents another coordinate such as rate tenor.
3. Core bucket matching uses `1e-9*max(1,abs(grid_coordinate))` tolerance for expiry and secondary-axis coordinates. `None` selects all; an empty list selects none. Unmatched finite strikes do not interpolate or snap and can produce a no-op with no no-match warning.
4. For selected cells, preview `vol'=max(vol*(1+pct/100),0)` through the core kernel. Overflow/nonfinite input errors. Re-evaluate every native grid cell through the model's surface resolver.
5. Reject any grid volatility `<=0`, including preexisting zero values outside the targeted bucket. For each second-axis coordinate, scan increasing expiries and warn if `vol²*expiry` decreases by more than absolute `1e-8`. Invalid expiry order produces a calendar warning. A successful preview emits the matching `MarketBump::VolBucketPct`; later execution uses the same kernel.

Thus a shock of -100% that hits a positive node becomes zero in the core preview and is rejected by the scenario adapter; a shock with empty/unmatched filters can leave the surface unchanged and still count as an applied effect.

## Limitations and alternatives

The calendar diagnostic is a fixed-grid squared-quote-time check. It does not inspect butterfly convexity, density, forward/strike normalization, moneyness movement, displaced-Black shift changes, or cross-model arbitrage. A `VolSurface` can carry normal, lognormal or shifted quote conventions; the same diagnostic code does not establish a universal no-arbitrage theorem for all of them. No interpolation repair, recalibration of SVI/SABR, or dynamic smile response occurs.

Alternatives are absolute vol-point moves, log-vol moves, calibrated parameter shocks or explicit no-arbitrage projection. They preserve different quantities and can change stress severity. The observed tolerance and -50% threshold are implementation controls, not market-calibrated evidence.

## Evidence

Owning source: [adapters/vol.rs](../../../../finstack-quant/scenarios/src/adapters/vol.rs); shared [volatility grids/shocks](../core-market/volatility-grids-and-shocks.md). Inspected unit tests compare parallel and unfiltered bucket shocks bit-for-bit, reject zeroing parallel/bucket shocks, flag a post-bump calendar variance decrease, and check invalid expiry order. [Bucket tests](../../../../finstack-quant/scenarios/tests/shocks/bucket_filtering_test.rs) locate snap/filter and preview/execution consistency checks. These are algorithmic invariants, not independent smile-arbitrage certification.

## Relationship to the accepted exact sigma-shock boundary

[CORE-MARKET-14's complete accepted package](../core-market/volatility-grids-and-shocks.md#accepted-decision-explicit-annual-volatility-sources-and-exact-grid-shocks) selects direct native
sigma delta/factor changes with strict nonnegative results, exact source targets
and full validation before intentional empty/no-op requests. This scenario's
explicitly relative percentage converts at its financial owner to a checked
factor; it does not justify a generic unit alias, flooring or nearest expiry
snapping. Zero sigma is source-admitted and actual pricing methods own their
limits; remove the unconditional zero rejection. Preserve metadata and qualify
clock-based date targets and convention-specific diagnostics. Record14 was
accepted on **2026-11-08**, with reconciliation/qualification pending; this
record's remaining diagnostic/report policies stay Draft.

## Follow-up tracking

Implementation, adoption and validation work is tracked in the
[scope follow-ups](../FOLLOW_UPS.md#scenarios). This record does not
authorize a runtime change or certify the observed method.
