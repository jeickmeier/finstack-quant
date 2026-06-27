# Portfolio & Scenarios API Coverage Audit (05_portfolio and 06_scenarios notebooks)

Date: 2026-06-23

Scope: Public symbols under `finstack_quant.portfolio` and `finstack_quant.scenarios` per `parity_contract.toml` and the `.pyi` files. Focus on the 13 notebooks split across `05_portfolio/` and `06_scenarios/`.

Legend:
- ✅ Demonstrated with runnable example
- ⚠️ Partially (token print, prose mention, indirect return value, or only JSON path)
- ❌ Not demonstrated

## 1. `finstack_quant.portfolio`

### 1.1 Portfolio Construction & Valuation

| Symbol | Status | Notes / Notebook |
|--------|--------|------------------|
| `parse_portfolio_spec` | ✅ | portfolio_construction_and_valuation |
| `build_portfolio_from_spec` | ✅ | portfolio_construction_and_valuation |
| `Portfolio.from_spec` + `.id`/`.as_of`/`.base_ccy`/`.to_spec_json` | ✅ | Used via helpers; typed `Portfolio` shown |
| `value_portfolio` | ✅ | Widespread (construction, multi_asset, stress) |
| `PortfolioValuation` (from_json, props, len) | ✅ | construction |
| `PortfolioResult` (envelope) + `portfolio_result_total_value` / `portfolio_result_get_metric` | ✅ | construction |
| `aggregate_metrics` | ✅ | construction, multi_asset |
| `aggregate_full_cashflows` | ✅ | construction |
| `PortfolioCashflows` | ✅ | construction |

### 1.2 Performance (TWRR / MWRR)

| Symbol | Status | Notes |
|--------|--------|-------|
| `twrr_modified_dietz` | ❌ | Absent |
| `twrr_linked` | ❌ | Absent |
| `mwr_xirr` | ❌ | Absent |

### 1.3 Attribution (Brinson / Carino)

| Symbol | Status | Notes |
|--------|--------|-------|
| `brinson_fachler` | ❌ | Absent |
| `carino_link` | ❌ | Absent |

### 1.4 Optimization (JSON + Typed)

| Symbol | Status | Notes |
|--------|--------|-------|
| `optimize_portfolio` (JSON entry) | ✅ | portfolio_optimization |
| `optimize_portfolio_typed` | ❌ | Absent |
| `PortfolioOptimizationSpec`, `Objective`, `Constraint`, `CandidatePosition`, `TradeUniverse`, `TradeSpec`, `OptimizationStatus`, `WeightingScheme`, `MissingMetricPolicy`, `Inequality`, `MetricExpr`, `PerPositionMetric`, `PositionFilter`, `TradeDirection`, `TradeType` | ⚠️ | Some (Constraint, Objective) appear in optimization notebook; full typed construction + `_typed` call absent |
| Supporting: `allocate_weights`, `validate_allocation_json` | ✅ | risk_decomposition |

### 1.5 Risk & Factor Decomposition

| Symbol | Status | Notes |
|--------|--------|-------|
| `decompose_factor_risk` | ❌ | Absent (only higher wrappers) |
| `compute_factor_sensitivities` | ❌ | Absent |
| `compute_pnl_profiles` | ❌ | Absent |
| `SensitivityMatrix` | ❌ | Absent |
| `RiskDecomposition`, `FactorRiskDecomposition`, `PositionRiskDecomposition` etc. | ⚠️ | `RiskDecomposition` shown; many position/factor subtypes not directly exercised |
| `parametric_var_decomposition` / `_typed` | ✅ / ⚠️ | JSON path shown; typed variant lightly or not |
| `parametric_es_decomposition` | ❌ | Absent |
| `historical_var_decomposition` / `_typed` | ✅ / ⚠️ | JSON shown |
| `evaluate_risk_budget` / `_typed` | ✅ | Both shown in risk_decomp |
| `position_component_var`, `position_what_if` | ✅ | risk_decomp |
| `WhatIfResult`, `StressResult`, `StressAttribution`, `build_stress_attribution`, `factor_stress`, `build_credit_vol_report` | ✅ | risk_decomp notebook |
| `RiskBudgetResult`, `PositionBudgetEntry` | ⚠️ | Indirect via evaluate |

### 1.6 Liquidity & Impact

| Symbol | Status | Notes |
|--------|--------|-------|
| `days_to_liquidate`, `lvar_bangia`, `almgren_chriss_impact`, `liquidity_tier`, `amihud_illiquidity`, `kyle_lambda`, `roll_effective_spread` | ✅ | liquidity_risk.ipynb (good coverage) |

### 1.7 Scenario Application (Portfolio Layer)

| Symbol | Status | Notes |
|--------|--------|-------|
| `apply_scenario_and_revalue` | ✅ | scenarios_and_stress_testing, scenario_impact_analysis |
| `replay_portfolio` | ✅ | historical_replay |

### 1.8 Errors

| Symbol | Status | Notes |
|--------|--------|-------|
| `PortfolioError`, `FinstackValuationError`, `FinstackFxError`, `FinstackOptimizationError` | ⚠️ | May surface indirectly; no dedicated try/except demos visible in quick scan |

## 2. `finstack_quant.scenarios`

### 2.1 Authoring & Composition (JSON)

| Symbol | Status | Notes |
|--------|--------|-------|
| `parse_scenario_spec` | ✅ | scenarios_and_stress_testing |
| `build_scenario_spec` | ✅ | Multiple notebooks (stress, multi_asset, composite, credit) |
| `compose_scenarios` | ✅ | composite_stress_tests, credit_scenarios |
| `validate_scenario_spec` | ✅ | stress_testing, credit_scenarios |
| `list_builtin_templates`, `list_builtin_template_metadata` | ✅ | stress_testing |
| `build_from_template`, `list_template_components`, `build_template_component` | ✅ | stress_testing, credit_scenarios |

### 2.2 Application

| Symbol | Status | Notes |
|--------|--------|-------|
| `apply_scenario` | ✅ | multi_asset, stress_testing |
| `apply_scenario_to_market` | ✅ | multi_asset, stress_testing, composite |
| `compute_horizon_return`, `HorizonResult` | ✅ | horizon_total_return |

### 2.3 Typed Authoring Surface

| Symbol | Status | Notes |
|--------|--------|-------|
| `OperationSpec` (typed classmethods) | ❌ | Absent (all authoring via JSON dicts to build_*) |
| `RateBindingSpec` | ❌ | Absent |
| Enums: `CurveKind`, `VolSurfaceKind`, `TenorMatchMode`, `TimeRollMode`, `Compounding` | ⚠️ | May appear inside JSON strings; not shown as Python enum values |

## 3. Reporting (Tear Sheets) — Cross-cutting

| Symbol | Status | Notes |
|--------|--------|-------|
| `portfolio_risk_tearsheet`, `scenario_tearsheet`, `portfolio_tearsheet` (and siblings) | ❌ | Not demonstrated inside 05 notebooks (may be in 08 reporting tier) |

## 4. Notebook Inventory — High-Level Coverage

- `portfolio_construction_and_valuation.ipynb`: Strong core pipeline (parse/build/value/aggregate/cashflows + typed wrappers).
- `portfolio_optimization.ipynb`: `optimize_portfolio` (JSON) + partial spec classes.
- `portfolio_risk_decomposition.ipynb`: Risk/var/es/budget/whatif/stress/credit-vol/allocate.
- `liquidity_risk.ipynb`: Excellent liquidity surface coverage.
- `historical_replay.ipynb`: `replay_portfolio`.
- `horizon_total_return.ipynb`: Horizon return primitive.
- `scenarios_and_stress_testing.ipynb`: Templates, build/compose/validate/apply_to_market + portfolio revalue.
- `multi_asset_portfolio_at_scale.ipynb`: Large-scale valuation + scenario application.
- `scenarios/rate_scenarios.ipynb`, `credit_scenarios.ipynb`, `composite_stress_tests.ipynb`, `scenario_impact_analysis.ipynb`: Scenario composition, rate/credit/fx shocks, impact analysis.

## 5. Prioritized Gaps (for implementation)

1. **Performance & Attribution** (high visibility, currently zero runnable):
   - `twrr_modified_dietz`, `twrr_linked`, `mwr_xirr`
   - `brinson_fachler`, `carino_link`
2. **Optimization typed path**:
   - Full `PortfolioOptimizationSpec` construction + `optimize_portfolio_typed`
   - Richer use of `Objective`/`Constraint`/`CandidatePosition`/etc.
3. **Factor / Risk typed depth**:
   - `decompose_factor_risk`, `compute_factor_sensitivities`, `compute_pnl_profiles`
   - `SensitivityMatrix`, `FactorRiskDecomposition`, position-level contrib types
   - `parametric_es_decomposition`
4. **Scenarios typed authoring**:
   - `OperationSpec.*` factories + `RateBindingSpec`
   - Using the typed objects to build specs instead of raw dicts/JSON strings
5. **Ergonomics & completeness**:
   - Explicit JSON ↔ typed round-trips for Portfolio/Valuation/OptimizationResult/ScenarioSpec
   - Richer `Portfolio` / `PortfolioValuation` property walks and cashflow ladder access
   - Error type demonstrations (try/except for the Finstack*Error hierarchy)
   - Optional light tear sheet usage (`portfolio_risk_tearsheet`, `scenario_tearsheet`) if this tier is the natural home
6. **Cross-checks**:
   - Distinguish `apply_scenario_to_market` (market only) vs `apply_scenario_and_revalue` (portfolio + market)
   - Ensure `HorizonResult` and stress attribution objects are inspected (not just printed)

## 6. Recommendations

- Keep additions small and colocated (e.g., one new "Performance & Attribution" section in construction or risk notebook; one "Typed Optimization" cell; one "Typed Scenario Operations" cell in scenarios_and_stress_testing or a scenarios/ notebook).
- Prefer demonstrating the short canonical name (`twrr_modified_dietz`, `optimize_portfolio_typed`) as the primary path.
- Use existing notebook boilerplate (AS_OF, MARKET_JSON, simple instrument/portfolio JSON) for new cells.
- After changes: run `uv run python .../run_all_notebooks.py --directory 05_portfolio` and `--directory 06_scenarios`
- Focused type/lint only (notebooks do not affect surrounding .py sources).
- Update this audit as gaps close.

## 7. Files Consulted

- `parity_contract.toml` (portfolio + scenarios sections + public symbol lists)
- `finstack_quant/portfolio/__init__.pyi` and `scenarios/__init__.pyi`
- All 13 notebooks under `05_portfolio/` (via content search + execution scans)
- Precedent audits: `FOUNDATIONS_API_COVERAGE_AUDIT.md`, `04_statement_modeling/STATEMENTS_API_COVERAGE_AUDIT.md`

No edits made to notebooks during this initial read-only audit pass.
