# SCENARIOS-007: Native hazard intensity shocks and exact CDS quote replay

- Decision status: **Accepted 2026-11-15 — complete native intensity, exact CDS replay and explicit replay-election package.**
- Implementation status: **Observed.** Reconciliation to the accepted package is pending.
- Validation status: **Independent native bucket and one-period hazard-versus-spread calculations verified.** Production replay, pricing, restoration and host qualification remain pending.
- Audit date: **2026-11-05**. Source revision: `7e82fd03ad98346727769e88dd5ac5d84b56e9fc`.
- Scope: [inventory](README.md), [source coverage](coverage.json). Shared kernels are linked as dependencies, not reassigned implementations.

## Accepted decision for native hazard shocks and exact CDS scenario replay

1. **Elect native intensity or CDS par-spread replay explicitly.** Retire
   the scenario-wide `HazardBumpMode`, `FirstOrderShift` and the
   spread-to-intensity conversion through loss given default. Preserve
   the exact hazard source and elected basis through direct and hierarchy
   dispatch. Bind at the action's execution point.
   [Accepted hazard risk](../core-market/hazard-quotes-and-risk.md#accepted-decision-explicit-cds-observations-hazard-risk-and-recovery) ·
   [Accepted hierarchy selection](03-hierarchy-resolution.md#accepted-decision-for-typed-scenario-hierarchy-expansion-and-overlap).

2. **Apply native moves to original intensity parameters.** Convert signed
   intensity bp to annual decimal intensity on the original source clock.
   A parallel move applies to every parameter. An individual target names
   an exact positive segment end. Sample accepted triangular weights at
   the original ends and retain the segment boundaries and constant tail.
   Keep recovery unchanged. Validate the whole request, including zero,
   and require finite nonnegative resulting intensities. Native shocks
   require neither discounting nor a recipe. An actual change preserves
   construction history and invalidates prior replay qualification.
   [Accepted intensity representation](../core-market/hazard-survival.md#accepted-decision-explicit-intensity-sources-and-local-probability-evaluation) ·
   [Accepted replay provenance](../core-market/calibration-provenance.md#accepted-decision-lossless-construction-records-and-request-time-replay-qualification).

3. **Qualify the original fit and bind the target market independently.**
   Reproduce original observations on their immutable source market with
   the original bound CDS terms, conventions, recovery and typed
   dependencies. Fit the elected action on the actual staged target
   market after preceding rate actions. Require an explicit typed
   discount binding. Retire currency-prefix and sole-curve guessing and
   target-market substitution for the original source.
   [Accepted hazard replay](../calibration-framework/21-hazard-quote-replay.md#accepted-decision-for-qualified-hazard-quote-replay-and-publication).

4. **Apply exact par-risk moves together in one refit.** Retain observed
   par quotes, fixed coupons, signed upfront observations and their
   settlement basis. Derive applicable par-risk inputs on the declared
   clean or dirty basis using the checked CDS pricer. Admit parallel
   moves or exact quote IDs, verified recipe indices or uniquely
   resolving literal contractual maturities. Reject missing, duplicate
   or ambiguous targets, invalid resulting quotes and nonzero moves
   rounded away. Retire approximate tenor matching and off-pillar
   splitting.
   [Accepted CDS quote settlement](../calibration-framework/07-cds-materialization.md#accepted-decision-for-single-name-cds-calibration-and-quote-settlement) ·
   [ISDA converter specification](https://cdsmodel.com/assets/cds-model/docs/ISDA%20Standard%20CDS%20Contract%20Converter%20Specification%20-%20Sept%204%2C%202009.pdf).

5. **Preserve the elected replay action, including at zero.** A zero-bp
   par-risk center fits the par-risk inputs. A dependency-only replay
   fits original forms and values and can change the curve on a changed
   discount state. Frozen-intensity recovery and original-quote recovery
   replay are separate elections. Require an explicit horizon requote
   with the accepted dated survival checks. Neither a later spread
   action nor an attached recipe elects it.
   [Accepted deal and horizon replay](../calibration-framework/22-hazard-deal-and-horizon.md#accepted-decision-for-deal-overrides-and-horizon-hazard-replay).

6. **Publish the admitted curve and evidence together.** Require
   complete quote repricing and applicable curve checks after replay.
   Advance replay lineage only with an admitted matching construction
   record and fit report. Preserve exact targets, requested and represented
   moves, monetary or rate basis as applicable, and typed failures.
   Use request-local caches on registered immutable market states.
   Publish through the accepted scenario transaction and retain report
   evidence under record 017.
   [Accepted scenario execution](02-execution-and-partial-success.md#accepted-decision-for-ordered-and-atomic-scenario-execution) ·
   [Accepted replay cache](../calibration-framework/23-recalibration-cache.md#accepted-decision-for-exact-request-local-recalibration-caching) ·
   [Reporting record 017](17-reports-and-replay.md).

## Independent calculation checks

- With native ends at years 1 and 3 and intensities `.02,.05`, a
  `+100` intensity-bp triangle centered at 2 with ends 0 and 4 has
  weight `.5` at both ends. The intensities become `.025,.055`
  and the tail stays at `.055` beyond year 3. The exact individual
  parameter target at year 2 is absent.
- In a stylized one-year CDS with unit discount, surviving premium,
  year-end protection settlement, no accrual on default, opening
  survival `.98` and recovery `.40`, the par spread is
  `.6(1−q)/q` for survival `q`. A `+25` intensity-bp move gives
  about `+15.325271` par bp. Delivering `+25` par bp requires about
  `+40.750192` intensity bp.

## Observed choice and rationale at the audit revision

Observed choice: ParCDS shocks default to quote-space recalibration with a lossless recipe. `FirstOrderShift` is an explicit alternative using the credit-triangle approximation. A curve's displayed par-spread samples do not substitute for the underlying calibration recipe. Reusing a provider allows replay caching without silently changing the economic shock definition.

## Ordered calculation and inputs

Both modes require an existing hazard curve and a validated finite `QuoteBump` (parallel bp or tenor/bp pairs).

For **FirstOrderShift**, a zero bump clones the base curve without needing recovery below one. Otherwise require `LGD=1-R>0`, convert each requested spread bp to hazard bp by division by LGD, and call direct hazard-rate bump methods. The resulting change in decimal intensity is `Δλ=bp*1e-4/(1-R)`. Core validates nonnegative/capped finite hazards; tenor shocks apply to their resolved hazard segment and repeated targets add. Direct reconstruction removes stored calibration/par-quote provenance that would be stale. A `HazardSpreadFirstOrder` warning accompanies nonzero shifts. No discount curve is selected in this branch, even if a supplied ID would be missing.

For **SolveToPar**, resolve discounting in this order: explicit ID (must exist); unique discount ID sharing the hazard ID's initial three uppercase ASCII characters; otherwise the sole available discount curve. Heuristic choices warn; zero available curves, several prefix matches or ambiguous remaining choices error. A three-character text prefix is not verified currency metadata.

Create `HazardRecalibrationRequest` from the current hazard, retained source-market snapshot, current target-market snapshot, selected discount ID and `SpreadBump`. `doc_clause`, CDS convention and deal quote overrides are unset, so the provider/recipe supplies them. The engine refreshes the retained source after rebuilding that hazard. Calibration algorithm, quote conversion, solver tolerances and cache identity belong to the provider, not this adapter.

Node shocks first use [tenor resolution](06-node-delivery-solve.md). The normalized split can be snapped to calibration quote maturities. Off-pillar solve-to-par delivery is explicitly only first-order and emits `InterpolatedNodeBumpFirstOrder`; an exact native pillar is the intended pillar-accurate route, subject to recipe quote mapping.

## Limits and alternatives

Rate-before-credit order affects the target discount market while retaining the appropriate original hazard calibration source. Direct hazard shifts do not certify a corresponding CDS price change, and the approximation grows sensitive as recovery approaches one. A later switch to solve-to-par may fail because a first-order shift removed its recipe. The whole composed scenario must choose one hazard mode.

The live scenario adapter still has discount inference, target-market
substitution and approximate tenor splitting. The live roll can infer
horizon requoting from a later credit action or an attached recipe.

## Evidence

Owning source: [adapters/curves.rs](../../../../finstack-quant/scenarios/src/adapters/curves.rs), [engine/mod.rs](../../../../finstack-quant/scenarios/src/engine/mod.rs). [Hazard quote/risk record](../core-market/hazard-quotes-and-risk.md) documents core distinctions. Inspected [par_cds_bump.rs](../../../../finstack-quant/scenarios/tests/par_cds_bump.rs) asserts `old_hazard+0.0025/(1-R)` for +25bp in first-order mode and a direction-of-change check for default replay. Inline tests reject display par spreads without a lossless recipe. Roll/replay and multiple independent curve tests are located in that integration file; direction and round-trip tests do not independently calibrate accuracy.

## Follow-up tracking

Implementation reconciliation and validation work are tracked in the
[scope follow-ups](../FOLLOW_UPS.md#scenarios).
