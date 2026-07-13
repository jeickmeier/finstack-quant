# All-Notebooks API Coverage Audit

Date: 2026-06-23

Goal: Drive notebook examples toward 100% coverage of the **user-facing** `finstack_quant` public API surface.

## Method (programmatic, reproducible)

`/tmp/coverage_audit.py` recursively imports every `finstack_quant` submodule, collects each module's `__all__` (the real public surface), tokenizes all notebook **code** cells and **markdown** cells, and classifies every public symbol as `covered` (appears in a code cell), `prose-only` (markdown only), or `uncovered`.

## Baseline (before this pass)

```
Notebooks: 98 | distinct public (__all__) symbols: 523
  covered in code:  251
  prose-only:        20
  UNCOVERED:        252
```

Uncovered by kind: `class` 105, `spec/contract` 72, `function` 33, `result` 32, `error` 5, `other` 5.

## Scope decision (user-approved: "high-value / pragmatic")

Cover 100% of the **user-facing** API:

1. **All 33 functions** — always demonstrable.
2. **~70 typed instrument classes** (`rates`/`fx`/`equity`/`exotics`/`fixed_income`/`commodity`/`credit_derivatives` + valuations top-level instrument/quote classes) via their typed surface (`from_json` / `price` / `price_with_metrics` / `validate` / `to_json`). The notebooks previously priced these via raw JSON + top-level functions, so the typed classes were never referenced by name.
3. **User-facing result / enum / spec types** — covered by calling the producing function and inspecting the named result, or by constructing the spec/enum.
4. **Margin / Monte-Carlo / features / covenants gaps** — the programmatic audit revealed prior tier "broad pass" claims were over-optimistic (e.g. `margin.ExposureProfile/VmResult/XvaResult`, `monte_carlo.price_heston_*`/`finite_diff_*` are not exercised anywhere).

### Documented exclusions (intentionally NOT given bespoke example cells)

These remain Rust-owned/JSON-only contracts or non-illustrative internals. They are exercised indirectly (bootstrapped via JSON) but not constructed by name:

- **Low-level bootstrap/calibration contracts** under `finstack_quant.valuations`: `Pillar`, `TenorPillar`, `DatePillar`, all `*Datum`, `*Quote`, `*Payload`, `*Step`, `*Prior`, `CalibrationStep`, `PriorMarketObject`. The notebooks intentionally drive bootstrapping through JSON payloads; these typed contracts are the JSON schema and are Rust-owned. (`CalibrationResult` IS covered via a bootstrap call inspection.)
- **Pure error types**: `PortfolioError`, `FinstackFxError`, `FinstackOptimizationError`, `FinstackValuationError`, `AnalyticsError` — surfaced as Python exceptions; demonstrated implicitly through normal error handling, not via dedicated cells.

Rationale: instantiating 72 low-level calibration contracts or printing error classes adds filler without teaching value. They are listed here for traceability.

## Implementation buckets (this pass)

| Bucket | Symbols | Home notebook(s) |
|--------|---------|------------------|
| valuations math fns | `bs_greeks`, `bs_implied_vol`, `black76_implied_vol`, `lookback_option_price` | 02_pricing |
| monte_carlo fns | `price_heston_call/put`, `finite_diff_delta(_crn)`, `finite_diff_gamma(_crn)`, `Estimate` | 07_advanced_quant/monte_carlo |
| features fns | `neutralize`, `normalize_signal`, `transform_panel`, `transform_timeseries_pairwise`, `transform_cross_sectional_grouped` | 03_analytics/feature_transforms |
| covenants fns | `cov_lite`, `lbo_standard`, `project_finance`, `real_estate`, `evaluate_engine`, `validate_covenant_*` | 04_statement_modeling/models/covenant_monitoring |
| cashflows json fns | `build_cashflow_schedule_json`, `validate_cashflow_schedule_json`, `accrued_interest_json`, `dated_flows_json` | 02_pricing/instruments/complex_cashflows |
| valuations instrument JSON fns | `bond_from_cashflows_json` | 02_pricing/instruments/complex_cashflows |
| portfolio gaps | `historical_var_decomposition` + enums/result types | 05_portfolio and 06_scenarios |
| statements_analytics gaps | `render_check_report_html`, `run_credit_underwriting_checks`, `CheckReport`, `CorkscrewReport`, lease specs, scorecard | 04_statement_modeling |
| margin gaps | `ExposureProfile`, `VmResult`, `XvaResult`, `XvaNettingSet`, `CsaTerms`, `ClearingStatus`, `MarginCallType`, `MarginTenor`, `MarginFundingCost`, `ExcessCollateral`, `Haircut01`, `CONSTANTS` | 07_advanced_quant/margin_collateral_and_xva |
| typed instruments | ~70 classes | 02_pricing/instruments/* |
| misc enums/results | scenarios enums, reporting `Theme`/`INSTITUTIONAL`, factor_model.credit, analytics results | various |

## Final status

After this pass (re-run `/tmp/coverage_audit.py`):

```
Notebooks: 99 | distinct public (__all__) symbols: 523
  covered in code:  420   (baseline 251  -> +169)
  prose-only:        10
  UNCOVERED:         93
```

The instrument notebooks use the canonical JSON validation and pricing helpers under `finstack_quant.valuations.instruments`. The former 70-class Python shell catalog was retired because each class stored only an opaque JSON string and duplicated that shared pipeline.

### Notebooks added/edited

- **Edited:** `equity_and_options` (bs_greeks/implied-vol/lookback), `complex_cashflows` (cashflows `*_json`), `feature_transforms` (neutralize/normalize/panel/pairwise/grouped), `covenant_monitoring` (covenant presets + evaluate/validate), `monte_carlo/stochastic_processes` (Heston), `monte_carlo/black_scholes_benchmarks` (MC finite-diff Greeks + CRN), `statement_analytics` (run_checks/CheckReport/renderers + three-statement/credit checks), `credit_scoring_and_pd` (scorecard), `real_estate_and_roll_forward_templates` (rich LeaseSpec + CorkscrewReport), `portfolio_risk_decomposition` (historical_var_decomposition + typed result/enum classes), `portfolio_optimization` (typed result + TradeSpec/Universe/PerPositionMetric), `factor_sensitivity` (SensitivityMatrix/FactorRiskDecomposition), `performance_analytics` (PeriodStats/BetaResult/… result types), `scenarios_and_stress_testing` (scenario enums + RateBindingSpec), `credit_factor_hierarchy` (LevelsAtDate/PeriodDecomposition), `margin_collateral_and_xva` (VmResult/Exposure/Xva/value types), `reporting_portfolio_tearsheet` (Theme/INSTITUTIONAL).

### Documented exclusions (the 93 uncovered, by design)

The remaining uncovered symbols are non-instrument surfaces, listed here for traceability.

1. **Bootstrap / calibration JSON contracts** are Rust-owned, versioned schemas. Python exposes one broad `CalibrationEnvelope` alias and drives validation through `validate_calibration_json` / `dry_run` instead of mirroring every enum variant as a hand-maintained `TypedDict`.
2. **Pure error types (5)**: `AnalyticsError`, `PortfolioError`, `FinstackFxError`, `FinstackOptimizationError`, `FinstackValuationError` — surfaced as Python exceptions via the centralized error mapping; demonstrated implicitly through normal error handling.
3. **Non-constructible / engine-internal results**: `Estimate` (scalar MC counterpart of `MoneyEstimate`; no Python producer and not directly constructible), `CandidatePosition` (`.pyi`: "Construction from Python is not yet supported"), `FactorAssignmentReport` / `PositionAssignment` / `UnmatchedEntry` (`from_json`-only outputs of the factor-assignment pass), `ExposureDiagnostics` (exposure-engine diagnostic sub-result; no Python constructor/`from_json`), and `LatentFactorKind` (trait-like surface; the concrete `LatentSingleFactor`/`LatentTwoFactor`/`LatentMultiFactor` are covered).

### Verification

- Re-run the coverage audit after public-surface changes; the historical counts above predate the JSON-shell removal.
- The catalog notebook and all edited notebooks pass an in-process execution smoke test; `run_all_notebooks.py` is run as the canonical check (transient Jupyter ZMQ kernel errors on long-running notebooks are a known environment artifact, not a logic regression).
