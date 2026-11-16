# SCENARIOS-016: Embedded stress templates, assembly order and parameter provenance

- Decision status: **Accepted 2026-11-15 — complete event-inspired stress catalogue, ordered composition, exact bindings and template lineage package.**
- Implementation status: **Observed.** Reconciliation to the accepted package is pending.
- Validation status: **Independent asset counts, component order and shock-unit calculations verified.** Production application, restoration and host qualification remain pending.
- Audit date: **2026-11-05**. Source revision: `7e82fd03ad98346727769e88dd5ac5d84b56e9fc`.
- Scope: [inventory](README.md), [source coverage](coverage.json). Shared kernels are linked as dependencies, not reassigned implementations.

## Accepted decision for checked event-inspired stress templates

1. **Classify the catalogue as stylized and event-inspired.** Expose
   that classification in discovery, built specifications and run
   provenance. Event dates, severity and asset-class labels serve
   discovery. Claims about observed moves, calibration probabilities
   or supervisory use require separate qualified evidence.
   [New York Fed SOFR launch notice](https://www.newyorkfed.org/markets/opolicy/operating_policy_180403).

2. **Preserve the five authored shock sets and units.** Retain the
   fixed GFC, COVID, 2022 rates, SVB and LTCM same-day magnitudes
   listed below. Equity, quoted sigma and ordered FX moves are
   relative percentages. Discount and spread moves are basis
   points. Severity is descriptive and does not scale the moves.
   [Shock units](01-specification-and-composition.md#accepted-decision-for-checked-scenario-specifications-and-ordered-composition).

3. **Admit one strict versioned Rust catalogue.** Require exact
   template IDs, globally unique template documents and unique
   component IDs within each template. Declared and actual
   component IDs and order must agree. Reject unknown fields or
   versions and duplicate JSON keys. Keep deterministic listings
   and independently owned builds. Cache only the immutable
   validated catalogue or its initialization failure. Python and
   WASM use the same Rust admission and lookup rules.

4. **Assemble through the accepted composer.** Use the exact
   registered ID for the full canonical build. Keep component
   operations in contiguous blocks, sort by ascending priority
   with authored order on ties, and select the minimum priority.
   Require declared order to agree with that result and one
   effective hierarchy policy across components. Retire the
   independent composite priority and policy reset.
   [Accepted ordered composition](01-specification-and-composition.md#accepted-decision-for-checked-scenario-specifications-and-ordered-composition).

5. **Require exact typed bindings and qualified operations.** Bind
   every selected role to its concrete target and required
   quote basis or dependencies. For discount nodes require an
   explicit tenor origin and date convention and use checked
   evaluated delivery within the original pillar span. Keep the
   GFC zero move at 5Y as a validated target. Elect par-CDS
   quote replay explicitly and bind the qualified original fit
   and actual staged discount source. FX requires exact ordered
   pair identity and a qualified frozen quote policy.
   [Evaluated nodes](06-node-delivery-solve.md#accepted-decision-for-exact-scenario-coordinates-and-checked-node-delivery) ·
   [CDS replay](07-hazard-quote-shocks.md#accepted-decision-for-native-hazard-shocks-and-exact-cds-scenario-replay) ·
   [Frozen FX shocks](04-fx-and-equity-shocks.md#accepted-decision-for-frozen-fx-and-checked-equity-scenario-shocks).

6. **Use the selected operations as execution prerequisites.** A full
   template requires each action to pass in order and publishes
   atomically. SVB requires inventory with supported exact
   `sector=regional_banks` matches and qualified settlement Z-spread
   centers. Its full template falls outside market-only horizon
   attribution. LTCM's rates component also requires an EM sovereign
   CDS replay route. Admit explicit component selection. A custom
   subset or edit receives its own checked scenario ID.
   [Atomic execution](02-execution-and-partial-success.md#accepted-decision-for-ordered-and-atomic-scenario-execution) ·
   [Instrument overrides](11-instrument-overrides.md#accepted-decision-for-checked-scenario-instrument-overrides) ·
   [Horizon scope](15-horizon-attribution-and-returns.md#accepted-decision-for-checked-scenario-horizon-attribution-and-returns).

7. **Carry template lineage into the run.** Record the schema version,
   immutable template content identity, ordered selected components,
   exact bindings and resulting scenario identity. Record edits when
   claiming derivative lineage. Record 017 owns replay
   representation and the additional required run inputs.
   [Reporting record 017](17-reports-and-replay.md).

## Independent asset and calculation checks

- The five documents contain 25 components, 40 operations split
  `9, 8, 7, 7, 9`, and priority `0–4` in each authored component
  order. The stable accepted composer preserves this order.
- For GFC, the parallel and node instructions request `−250`,
  `−200`, `−150` and `−125 bp` at 2Y, 5Y, 10Y and 30Y.
  These are requested net target moves.
- An opening quoted sigma of `20%` becomes `60%` under
  GFC's `+200%` action and `70%` under COVID's `+250%` action.

## Observed choice and rationale at the audit revision

Observed choice: provide reusable **stylized stress specifications** inspired by named events. The five embedded documents contain fixed market shocks and modern placeholder identifiers; they do not load historical time series, fit joint distributions, estimate probabilities or establish an observation window. Event names, dates and severity labels are descriptive metadata, not independently verified market facts or regulatory stress certification.

## Ordered assembly and validation

The compile-time load order is GFC 2008, COVID 2020, 2022 rate shock, SVB 2023 and LTCM 1998. Each versioned JSON document rejects unknown fields, requires nonblank template ID and nonempty components, requires exact component-ID equality/order with metadata, rejects duplicate component IDs and validates every component scenario.

Registration concatenates component operations in that document/metadata order: rates, credit, equity, volatility, FX. It does **not** call `ScenarioSpec::compose`, sort component priorities, or inherit component resolution/hazard policies; composite policies are defaults (`MostSpecificWins`, `SolveToPar`) and composite metadata supplies ID/name/description/priority. All present builtins use those defaults, so no present policy conflict is asserted. The registry is initialized once with `OnceLock<Result<...>>`, caching both success and initialization failure. `build` and `build_component` return exact cloned specifications; unknown IDs error. There is no runtime template fitting or automatic identifier remapping.

## Embedded parameter values and audit-era operation names

Rate entries below are discount continuous-zero basis-point changes on `USD-SOFR`; listed node changes are additional ordered node operations under `Interpolate`. Credit curve entries are ParCDS bp under default solve-to-par. Equity, surface volatility and FX entries are **relative percentages**, so a +200% vol shock triples quoted vol.

| Template | Rates: parallel; additional nodes | Credit | Equity | Volatility | FX |
|---|---|---|---|---|---|
| `gfc_2008` | -200bp; 2Y -50, 5Y 0, 10Y +50, 30Y +75bp | USD-IG +300bp; USD-HY +800bp | SPX -50%; RTY -55% | SPX_VOL +200% | EUR/USD -10%; GBP/USD -25% |
| `covid_2020` | -150bp; 2Y -30, 10Y +10, 30Y +20bp | USD-IG +200bp; USD-HY +600bp | SPX -34% | SPX_VOL +250% | EUR/USD -5%; BRL/USD -15% |
| `rate_shock_2022` | +300bp | USD-IG +100bp; USD-HY +200bp | SPX -25%; NDX -33% | SPX_VOL +50% | EUR/USD -15% |
| `svb_2023` | -100bp; 2Y -50, 10Y +25bp | Instrument spread +150bp where `sector=regional_banks` | KRE -20%; SPX -5% | SPX_VOL +75% | EUR/USD -1% |
| `ltcm_1998` | -75bp; 2Y -25, 30Y +50bp; EM-SOVEREIGN ParCDS +200bp in rates component | USD-IG +100bp; EM-CORPORATE +200bp | SPX -20% | SPX_VOL +100% | BRL/USD -30%; RUB/USD -50% |

The five JSON files contain 5 components each and respectively 9, 8, 7, 7 and 9 operation entries (GFC, COVID, 2022, SVB, LTCM). No time roll is included. No severity scaling is applied when building a template.

## Limitations, alternatives and evidence

Curve IDs must be mapped by editing the returned spec to the intended market. ParCDS targets require lossless calibration recipes and resolvable discount curves. SVB's credit operation requires an instrument inventory; the full SVB composite is rejected by `HorizonAnalysis` because it mutates instruments. Missing equity quotes can warn/skip while missing credit/volatility data errors, so application completeness differs across factor types. Components bearing an asset-class label can contain cross-domain shocks, as the LTCM rates component does.

Owning source: [templates/json.rs](../../../../finstack-quant/scenarios/src/templates/json.rs), [registry.rs](../../../../finstack-quant/scenarios/src/templates/registry.rs), [metadata.rs](../../../../finstack-quant/scenarios/src/templates/metadata.rs). Inspected data: [GFC](../../../../finstack-quant/scenarios/data/templates/gfc_2008.json), [COVID](../../../../finstack-quant/scenarios/data/templates/covid_2020.json), [2022](../../../../finstack-quant/scenarios/data/templates/rate_shock_2022.json), [SVB](../../../../finstack-quant/scenarios/data/templates/svb_2023.json), [LTCM](../../../../finstack-quant/scenarios/data/templates/ltcm_1998.json). Inline tests inspect registry order, exact parameter assertions, metadata/component order and priority-order conflicts. [Template integration](../../../../finstack-quant/scenarios/tests/templates_integration.rs) locates assembled fixture tests. Those are configuration regressions, not historical-data validation.

Empirically sourced event-window shocks, covariance-consistent macro factor stresses and calibrated reverse stresses are alternatives. No historical parameter adoption is inferred.

## Follow-up tracking

Implementation reconciliation and production qualification are tracked
in the [scope follow-ups](../FOLLOW_UPS.md#scenarios).
