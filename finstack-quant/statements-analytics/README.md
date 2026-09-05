# finstack-quant-statements-analytics

Analysis, reporting, templates, and runtime extensions layered on top of the
[`finstack-quant-statements`](../statements/README.md) evaluation engine.
Nothing here re-implements evaluation: every workflow builds or evaluates a
`FinancialModelSpec` through the statements crate and then interprets the
resulting `StatementResult`.

## Where it sits

Depends on [`finstack-quant-statements`](../statements/README.md) (models and
evaluation), [`finstack-quant-covenants`](../covenants/README.md) (covenant
forecasting), [`finstack-quant-valuations`](../valuations/README.md) (the DCF
instrument, terminal-value specs, market context), and
[`finstack-quant-core`](../core/README.md). `rayon` is a dependency only on
non-`wasm32` targets.

It is a leaf of the domain stack: no other domain crate depends on it. Only the
umbrella crate `finstack-quant` (which re-exports it as
`finstack_quant::statements_analytics`) and the `finstack-quant-py` /
`finstack-quant-wasm` binding crates consume it.

It defines no error type of its own: most of the crate returns
`finstack_quant_statements::Result`, and the ECL staging and covenant-forecast
paths return `finstack_quant_core::Result`.

| Need | Crate |
|------|-------|
| Build and evaluate statement models, formula DSL, checks | `finstack-quant-statements` |
| Scenarios, sensitivity, DCF, LBO, ECL, comps, reports, templates | `finstack-quant-statements-analytics` |
| Covenant specs, engine, breach tracking | `finstack-quant-covenants` |
| Instrument pricing and risk | `finstack-quant-valuations` |
| Dates, money, curves, core types | `finstack-quant-core` |

## Quick start

`CorporateAnalysisBuilder` evaluates a model once and optionally adds DCF equity
valuation and per-instrument credit context:

```rust
use finstack_quant_core::{currency::Currency, dates::PeriodId, money::Money};
use finstack_quant_statements::builder::ModelBuilder;
use finstack_quant_statements::checks::{builtins::NonFiniteCheck, CheckSuite};
use finstack_quant_statements_analytics::analysis::CorporateAnalysisBuilder;
use finstack_quant_valuations::instruments::equity::dcf_equity::TerminalValueSpec;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let model = ModelBuilder::new("lbo-demo")
        .periods("2025Q1..Q4", None)?
        .value_money(
            "revenue",
            &[
                (PeriodId::quarter(2025, 1)?, Money::new(10_000_000.0, Currency::USD)?),
                (PeriodId::quarter(2025, 2)?, Money::new(10_500_000.0, Currency::USD)?),
                (PeriodId::quarter(2025, 3)?, Money::new(11_000_000.0, Currency::USD)?),
                (PeriodId::quarter(2025, 4)?, Money::new(11_500_000.0, Currency::USD)?),
            ],
        )
        .compute("ebitda", "revenue * 0.25")?
        .compute("ufcf", "ebitda * 0.6")?
        .with_meta("currency", serde_json::json!("USD"))
        .build()?;

    let checks = CheckSuite::builder("corporate")
        .add_check(NonFiniteCheck { nodes: vec![] })
        .build();

    let analysis = CorporateAnalysisBuilder::new(model)
        .dcf(0.10, TerminalValueSpec::GordonGrowth { growth_rate: 0.02 })
        .net_debt_override(20_000_000.0)
        .checks(checks)
        .analyze()?;

    if let Some(equity) = &analysis.equity {
        println!("Equity value: {}", equity.equity_value);
    }

    Ok(())
}
```

`dcf()` reads a monetary free-cash-flow series from `ufcf`; its currency must
match `model.meta["currency"]`. Valuation and credit analysis require a
`CheckSuite` containing `NonFiniteCheck`; production three-statement models
should use `three_statement_checks`. Credit analysis also requires explicit
`.cfads_node(...)` and `.interest_coverage_node(...)` mappings. `.as_of(date)`
is the DCF valuation date and, with `.market(ctx)`, the statement visibility and
curve date. DCF discounting remains WACC-only. `.ltv_value_node(...)` supplies a
per-period denominator; otherwise a positive DCF EV is broadcast.

## Module layout

Three public modules: `analysis`, `extensions`, `templates`. Inside `analysis`,
only `checks`, `backtesting`, `goal_seek`, `introspection`, and `reports` are
public module paths; everything else is `pub(crate)` and reaches callers through
re-exports at `analysis::*`. Import from `analysis` directly rather than naming
a submodule.

| Area | Key exports |
|------|-------------|
| Valuation | `CorporateAnalysisBuilder`, `CorporateAnalysis`, `CorporateValuationResult`, `evaluate_dcf_with_market`, `DcfOptions`, `dcf_sensitivity`, `DcfSensitivityResult`, `ExitMultipleBump`, `wacc` |
| LBO | `evaluate_lbo`, `LboConfig`, `LboResult`, `LboTranche`, `LboCheckMappings` |
| Scenarios | `ScenarioSet`, `ScenarioDefinition`, `ScenarioResults`, `ScenarioDiff` |
| Sensitivity | `SensitivityAnalyzer`, `SensitivityConfig`, `SensitivityMode`, `SensitivityResult`, `ParameterSpec`, `TornadoEntry`, `generate_tornado_entries` |
| Variance | `VarianceAnalyzer`, `VarianceConfig`, `VarianceReport`, `VarianceRow`, `BridgeChart`, `BridgeStep` |
| Credit | `compute_credit_context`, `CreditContextMetrics`, `forecast_covenant`, `forecast_breaches`, `StatementsAdapter`, `to_table` |
| Checks (`analysis::checks`) | `three_statement_checks`, `credit_underwriting_checks`, `lbo_model_checks`, `ThreeStatementMapping`, `CreditMapping`, `FormulaCheckSpec`, `CheckReportRenderer`, plus reconciliation / consistency / credit check types |
| ECL | `EclEngine`, `EclConfig`, `EclConfigBuilder`, `CeclEngine`, `CeclConfig`, `classify_stage`, `StagingConfig`, `PdTermStructure`, `PortfolioEclResult`, `ProvisionWaterfall`, `compute_waterfall` |
| Comps | `PeerSet`, `PeerFilter`, `PeerStats`, `compute_peer_multiples`, `compute_multiple`, `regression_fair_value`, `score_relative_value`, `percentile_rank`, `z_score` |
| Goal seek | `goal_seek` |
| Backtesting | `backtest_forecast`, `ForecastMetrics` |
| Introspection | `DependencyTracer`, `DependencyTree`, `FormulaExplainer`, `Explanation`, `render_tree_ascii`, `render_tree_detailed` |
| Reports | `PLSummaryReport`, `CreditAssessmentReport`, `CreditAssessment` |
| `extensions` | `CorkscrewExtension` + `CorkscrewConfig`/`CorkscrewReport`/`CorkscrewAccount`/`AccountType`/`CorkscrewStatus`; `CreditScorecardExtension` + `ScorecardConfig`/`ScorecardMetric`/`ScorecardReport`/`ScorecardStatus` |
| `templates` | the `roll_forward`, `vintage`, and `real_estate` modules (free functions over `ModelBuilder`) |

Item-level detail is in the rustdoc:
`cargo doc -p finstack-quant-statements-analytics --open`.

## Workflows

**Scenarios and variance.** `ScenarioSet` is an ordered map of named
`ScenarioDefinition`s, each with an optional `parent` and typed
`AmountOrScalar` node overrides. Child overrides win over ancestors, and
`trace` shows the resolution chain. `evaluate_all(&model)` runs every case;
`diff(&results, baseline, comparison, metrics, periods)` returns a
`ScenarioDiff`;
`to_comparison_table` renders the set as a `TableEnvelope`.
`VarianceAnalyzer::new(&baseline, &comparison)` then `.compute(&config)` yields a
`VarianceReport`, and `.bridge_decomposition(...)` a `BridgeChart`.
`SensitivityAnalyzer::new(&model).run(&config)` sweeps `ParameterSpec`s and
`generate_tornado_entries` ranks the result. Statement-model Monte Carlo lives in
`finstack-quant-statements`, not here.

**Credit and covenants.** `compute_credit_context` derives coverage, leverage,
and LTV metrics from a `StatementResult` plus capital-structure cashflows.
`CreditNumeratorNodes` separates CFADS for DSCR from EBITDA/EBIT for interest
coverage. Instrument cashflows must already be in the reporting currency;
mixed-currency ratios fail. LTV is `debt_t / value_t`; scalar EV references may
be broadcast across periods. Metrics include cash DSCR, total (PIK) DSCR, and
fee-inclusive DSCR. `StatementsAdapter` implements the covenants crate's
`ModelTimeSeries` bridge, so covenant forecasts do not make that crate depend
on statements. `to_table` renders a `CovenantForecast` as a `TableEnvelope`.

**Checks.** `analysis::checks` extends the structural checks in
`finstack_quant_statements::checks::builtins` with cross-statement
reconciliation (`CapexReconciliation`, `DepreciationReconciliation`,
`DividendReconciliation`, `InterestExpenseReconciliation`), internal consistency
(`EffectiveTaxRateCheck`, `GrowthRateConsistency`, `WorkingCapitalConsistency`),
and credit reasonableness (`CoverageFloorCheck`, `LeverageRangeCheck`,
`LiquidityRunwayCheck`, `FcfSignCheck`, `TrendCheck`). The pre-built suites take
typed node-id mappings rather than hard-coded names, so they work against any
chart of accounts: `three_statement_checks(ThreeStatementMapping)`,
`credit_underwriting_checks(CreditMapping)`, and
`lbo_model_checks(ThreeStatementMapping, CreditMapping)`. All three return a
`finstack_quant_statements::checks::CheckSuite`, ready for
`Evaluator::with_checks`.

**ECL.** IFRS 9 staging (`classify_stage`, `StagingConfig`, `StagingTrigger`)
and CECL (`CeclEngine`, `CeclMethodology`), with single-exposure, weighted, and
portfolio aggregation paths plus a `ProvisionWaterfall`. Stage 2/3 DPD
backstops fire at `days_past_due >= 30` / `>= 90` (bank / CECL alignment);
the display contract is `dpd_stage2 (dpd=30 >= 30)` /
`dpd_stage3 (dpd=90 >= 90)`. `Exposure` priced EAD is
`drawn + undrawn × ccf` via core `ead_revolver` (`undrawn` default `0.0`,
`ccf` default `0.75` / `DEFAULT_REVOLVER_CCF`). `RatingPdMap` is a
rating-keyed map of `RawPdCurve` values; a missing rating skips the SICR
PD-delta rather than failing the run. `EclConfig`, `StagingConfig` and
`CeclConfig` defaults come from the embedded `ecl_policy.v1.json` registry.

**Templates.** Build-time free functions over `ModelBuilder`:
`roll_forward::add_roll_forward` (beginning + increases − decreases =
ending), `vintage::add_vintage_buildup` (cohort convolution;
`decay_curve[k]` is in **model periods**, not calendar years), and
`real_estate` (`add_rent_roll`, `add_noi_buildup`, `add_ncf_buildup`,
`add_property_operating_statement`, the last driven by `LeaseSpec`,
`ManagementFeeSpec`, and friends). `LeaseGrowthConvention` defaults to
`AnnualEscalator` (Argus/NCREIF anniversary bumps); `PerPeriod` must be set
explicitly. Templates only add nodes; they add no runtime behavior.
Real-estate amounts are per model period, not annualized, unless a field
says otherwise.

**Runtime extensions.** `CorkscrewExtension` validates roll-forward articulation
after evaluation (`expected = prev + Σ changes − Σ decreases`); pair
`add_roll_forward` increase/disposal nodes with `CorkscrewAccount.changes` /
`decreases`. `CreditScorecardExtension` applies weighted metric scoring with
embedded S&P / Moody's / Fitch scales. Both are plain structs — `new(cfg)`
then `execute(&model, &results)` — not trait objects.

## Conventions

- Ratios (DSCR, coverage, leverage, valuation multiples) are plain scalars:
  `2.0` means `2.0x`.
- Percentage-style inputs (WACC, growth rates, discounts) use decimal form:
  `0.10` means `10%`.
- `ScenarioDefinition.overrides` preserve scalar/monetary type and currency and
  are broadcast across **forecast** periods only, so historical actuals survive.
  Incompatible target-node units fail before evaluation. Overrides use the
  statements crate's `Value > Forecast > Formula` precedence.
- Monetary outputs (`equity_value`, `enterprise_value`, `net_debt`,
  `terminal_value_pv`) are `Money` in the evaluated model's currency; coverage
  and leverage metrics are unitless scalars.
- A non-positive DCF enterprise value is not used as an LTV reference; the
  pipeline records that in `CorporateAnalysis::ev_suppressed_non_positive` and
  computes credit metrics without one. A positive EV is broadcast as one
  constant denominator per requested period; `.ltv_value_node` supplies a
  varying statement path instead.
- `SensitivityMode::Diagonal` runs in parallel on rayon on native targets and
  serially on `wasm32`. `FullGrid` and `Tornado` are always serial. Every mode
  drives `Evaluator::prepare` / `evaluate_prepared`, so the model's formulas are
  compiled once per sweep.

Workspace-wide invariants (Decimal vs f64, currency safety, serde strictness)
are in [INVARIANTS.md](../../INVARIANTS.md).

## Bindings

- **Python** — `finstack_quant.statements_analytics`: sensitivity, variance,
  scenario sets, backtesting, goal seek, introspection, DCF and corporate
  analysis, the check runners and report renderers, comps, ECL, the corkscrew and
  scorecard extensions, and the roll-forward / vintage / real-estate templates.
- **WASM** — `statements_analytics` namespace in
  `finstack-quant-wasm/exports/statements_analytics.js`: `runSensitivity`,
  `runVariance`, `evaluateScenarioSet`, `backtestForecast`,
  `generateTornadoEntries`, `goalSeek`, `dcfSensitivity`, `evaluateLbo`, `wacc`,
  `traceDependencies`, `explainFormula`, the report renderers, the check
  runners, and the comps helpers.

## Verification

```bash
cargo nextest run -p finstack-quant-statements-analytics --lib --test '*'
cargo test -p finstack-quant-statements-analytics --doc
cargo clippy -p finstack-quant-statements-analytics --lib --bins --tests --examples -- -D warnings
RUSTDOCFLAGS='-D warnings' cargo doc -p finstack-quant-statements-analytics --no-deps
```

Integration tests live in `tests/`: `analysis_corporate.rs`,
`analysis_orchestrator.rs`, `analysis_scenario_set.rs`, `analysis_ecl.rs`,
`analysis_goal_seek.rs`, `analysis_monte_carlo.rs`, and
`extensions_scorecards.rs` are standalone targets; `checks_all.rs`,
`extensions_all.rs`, `forecast_all.rs`, and `integration_all.rs` are harness
entry points for the `checks/`, `extensions/`, `forecast/`, and `integration/`
module directories.

## See also

- [`../statements/README.md`](../statements/README.md)
- [`../covenants/README.md`](../covenants/README.md)
- [`../valuations/README.md`](../valuations/README.md)
- [`../core/README.md`](../core/README.md)
- [`../../docs/REFERENCES.md`](../../docs/REFERENCES.md) — DCF, coverage, and
  leverage source references
