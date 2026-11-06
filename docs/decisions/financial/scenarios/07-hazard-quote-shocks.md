# SCENARIOS-007: Par-CDS solve-to-par replay and explicit first-order hazard shifts

- Decision status: **Draft — observed implementation; adoption pending.**
- Implementation status: **Observed for the stated source scope.** Actual discrepancies and unsupported cases are distinguished below.
- Validation status: **Evidence inspected/located, not executed.** No numerical test, build or external benchmark run is claimed.
- Audit date: **2026-11-05**. Source revision: `7e82fd03ad98346727769e88dd5ac5d84b56e9fc`.
- Scope: [inventory](README.md), [source coverage](coverage.json). Shared kernels are linked as dependencies, not reassigned implementations.

## Choice and rationale

Observed choice: ParCDS shocks default to quote-space recalibration with a lossless recipe. `FirstOrderShift` is an explicit alternative using the credit-triangle approximation. A curve's displayed par-spread samples do not substitute for the underlying calibration recipe. Reusing a provider allows replay caching without silently changing the economic shock definition.

## Ordered calculation and inputs

Both modes require an existing hazard curve and a validated finite `QuoteBump` (parallel bp or tenor/bp pairs).

For **FirstOrderShift**, a zero bump clones the base curve without needing recovery below one. Otherwise require `LGD=1-R>0`, convert each requested spread bp to hazard bp by division by LGD, and call direct hazard-rate bump methods. The resulting change in decimal intensity is `Δλ=bp*1e-4/(1-R)`. Core validates nonnegative/capped finite hazards; tenor shocks apply to their resolved hazard segment and repeated targets add. Direct reconstruction removes stored calibration/par-quote provenance that would be stale. A `HazardSpreadFirstOrder` warning accompanies nonzero shifts. No discount curve is selected in this branch, even if a supplied ID would be missing.

For **SolveToPar**, resolve discounting in this order: explicit ID (must exist); unique discount ID sharing the hazard ID's initial three uppercase ASCII characters; otherwise the sole available discount curve. Heuristic choices warn; zero available curves, several prefix matches or ambiguous remaining choices error. A three-character text prefix is not verified currency metadata.

Create `HazardRecalibrationRequest` from the current hazard, retained source-market snapshot, current target-market snapshot, selected discount ID and `SpreadBump`. `doc_clause`, CDS convention and deal quote overrides are unset, so the provider/recipe supplies them. The engine refreshes the retained source after rebuilding that hazard. Calibration algorithm, quote conversion, solver tolerances and cache identity belong to the provider, not this adapter.

Node shocks first use [tenor resolution](06-node-delivery-solve.md). The normalized split can be snapped to calibration quote maturities. Off-pillar solve-to-par delivery is explicitly only first-order and emits `InterpolatedNodeBumpFirstOrder`; an exact native pillar is the intended pillar-accurate route, subject to recipe quote mapping.

## Limits and alternatives

Rate-before-credit order affects the target discount market while retaining the appropriate original hazard calibration source. Direct hazard shifts do not certify a corresponding CDS price change, and the approximation grows sensitive as recovery approaches one. A later switch to solve-to-par may fail because a first-order shift removed its recipe. The whole composed scenario must choose one hazard mode.

Alternative risk definitions include direct intensity shocks, par-quote bumps with full recalibration, or upfront-price quote shocks. They have different units and must not be inferred from the operation's generic name. Explicit discount IDs avoid heuristic routing; enforcing them would be a policy change.

## Evidence

Owning source: [adapters/curves.rs](../../../../finstack-quant/scenarios/src/adapters/curves.rs), [engine/mod.rs](../../../../finstack-quant/scenarios/src/engine/mod.rs). [Hazard quote/risk record](../core-market/hazard-quotes-and-risk.md) documents core distinctions. Inspected [par_cds_bump.rs](../../../../finstack-quant/scenarios/tests/par_cds_bump.rs) asserts `old_hazard+0.0025/(1-R)` for +25bp in first-order mode and a direction-of-change check for default replay. Inline tests reject display par spreads without a lossless recipe. Roll/replay and multiple independent curve tests are located in that integration file; direction and round-trip tests do not independently calibrate accuracy.

## Follow-up tracking

Implementation, adoption and validation work is tracked in the
[scope follow-ups](../FOLLOW_UPS.md#scenarios). This record does not
authorize a runtime change or certify the observed method.
