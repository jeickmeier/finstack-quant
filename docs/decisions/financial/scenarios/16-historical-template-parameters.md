# SCENARIOS-016: Embedded stress templates, assembly order and parameter provenance

- Decision status: **Draft — observed implementation; adoption pending.**
- Implementation status: **Observed for the stated source scope.** Actual discrepancies and unsupported cases are distinguished below.
- Validation status: **Evidence inspected/located, not executed.** No numerical test, build or external benchmark run is claimed.
- Audit date: **2026-11-05**. Source revision: `7e82fd03ad98346727769e88dd5ac5d84b56e9fc`.
- Scope: [inventory](README.md), [source coverage](coverage.json). Shared kernels are linked as dependencies, not reassigned implementations.

## Choice and rationale

Observed choice: provide reusable **stylized stress specifications** inspired by named events. The five embedded documents contain fixed market shocks and modern placeholder identifiers; they do not load historical time series, fit joint distributions, estimate probabilities or establish an observation window. Event names, dates and severity labels are descriptive metadata, not independently verified market facts or regulatory stress certification.

## Ordered assembly and validation

The compile-time load order is GFC 2008, COVID 2020, 2022 rate shock, SVB 2023 and LTCM 1998. Each versioned JSON document rejects unknown fields, requires nonblank template ID and nonempty components, requires exact component-ID equality/order with metadata, rejects duplicate component IDs and validates every component scenario.

Registration concatenates component operations in that document/metadata order: rates, credit, equity, volatility, FX. It does **not** call `ScenarioSpec::compose`, sort component priorities, or inherit component resolution/hazard policies; composite policies are defaults (`MostSpecificWins`, `SolveToPar`) and composite metadata supplies ID/name/description/priority. All present builtins use those defaults, so no present policy conflict is asserted. The registry is initialized once with `OnceLock<Result<...>>`, caching both success and initialization failure. `build` and `build_component` return exact cloned specifications; unknown IDs error. There is no runtime template fitting or automatic identifier remapping.

## Embedded calculation parameters

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

Implementation, adoption and validation work is tracked in the
[scope follow-ups](../FOLLOW_UPS.md#scenarios). This record does not
authorize a runtime change or certify the observed method.
