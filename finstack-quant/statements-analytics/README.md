# finstack-quant-statements-analytics

Analysis, reporting, templates, and runtime extensions on top of `finstack-quant-statements`.

## Where it fits

| Need | Crate |
|------|-------|
| Build and evaluate statement models | `finstack-quant-statements` |
| Scenarios, DCF, covenants, reports, templates, extensions | `finstack-quant-statements-analytics` |
| Instrument pricing and covenant engines | `finstack-quant-valuations` |
| Dates, money, curves, core types | `finstack-quant-core` |

## Quick start

`CorporateAnalysisBuilder` evaluates a model once and optionally adds DCF equity valuation and per-instrument credit context:

```rust
use finstack_quant_core::dates::PeriodId;
use finstack_quant_statements::builder::ModelBuilder;
use finstack_quant_statements::types::AmountOrScalar;
use finstack_quant_statements_analytics::analysis::CorporateAnalysisBuilder;
use finstack_quant_valuations::instruments::equity::dcf_equity::TerminalValueSpec;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let model = ModelBuilder::new("lbo-demo")
        .periods("2025Q1..Q4", None)?
        .value(
            "revenue",
            &[
                (PeriodId::quarter(2025, 1), AmountOrScalar::scalar(10_000_000.0)),
                (PeriodId::quarter(2025, 2), AmountOrScalar::scalar(10_500_000.0)),
                (PeriodId::quarter(2025, 3), AmountOrScalar::scalar(11_000_000.0)),
                (PeriodId::quarter(2025, 4), AmountOrScalar::scalar(11_500_000.0)),
            ],
        )
        .compute("ebitda", "revenue * 0.25")?
        .compute("ufcf", "ebitda * 0.6")?
        .with_meta("currency", serde_json::json!("USD"))
        .build()?;

    let analysis = CorporateAnalysisBuilder::new(model)
        .dcf(0.10, TerminalValueSpec::GordonGrowth { growth_rate: 0.02 })
        .net_debt_override(20_000_000.0)
        .coverage_node("ebitda")
        .analyze()?;

    if let Some(equity) = &analysis.equity {
        println!("Equity value: {}", equity.equity_value);
    }

    Ok(())
}
```

## Core workflows

**Scenarios and variance** — `ScenarioSet` registers named cases with optional parent inheritance and scalar overrides. Use `evaluate_all`, `diff`, `VarianceAnalyzer`, `SensitivityAnalyzer`, and `MonteCarloResults` for comparisons and sweeps.

**Credit and covenants** — `compute_credit_context()` derives coverage and leverage from statement results plus capital-structure cashflows. `forecast_breaches()` and `forecast_covenant(s)()` bridge into the `finstack-quant-valuations` covenant engine.

**Templates** — build-time `ModelBuilder` extensions: `TemplatesExtension` (roll-forward), `VintageExtension` (cohort buildup), `RealEstateExtension` (NOI, NCF, rent roll, property operating statement). Templates add nodes at build time; use `CorkscrewExtension` at runtime to validate roll-forward articulation.

**Runtime extensions** — `CorkscrewExtension` (balance-sheet roll-forward checks) and `CreditScorecardExtension` (weighted metric scoring with embedded S&P/Moody's/Fitch scales).

**Other analysis** — `goal_seek`, `backtest_forecast`, dependency tracing (`DependencyTracer`, `FormulaExplainer`), and formatted reports (`TableBuilder`, `PLSummaryReport`, `CreditAssessmentReport`).

## Module guide

| Module | Purpose | Key exports |
|--------|---------|-------------|
| `analysis::valuation` | DCF and corporate pipeline | `CorporateAnalysisBuilder`, `evaluate_dcf_with_market` |
| `analysis::credit` | Coverage, leverage, covenants | `compute_credit_context`, `forecast_breaches` |
| `analysis::scenarios` | Scenarios, sensitivity, variance, Monte Carlo | `ScenarioSet`, `SensitivityAnalyzer`, `VarianceAnalyzer` |
| `analysis::checks` | Reconciliation, consistency, credit checks | `three_statement_checks`, `FormulaCheck` |
| `analysis::ecl` | IFRS 9 / CECL staging and portfolio ECL | `EclEngine`, `CeclEngine`, `classify_stage` |
| `analysis::comps` | Peer multiples and relative value | `PeerSet`, `compute_peer_multiples` |
| `analysis::goal_seek` | Root-finding on model drivers | `goal_seek` |
| `analysis::backtesting` | Forecast accuracy | `backtest_forecast`, `ForecastMetrics` |
| `analysis::introspection` | Dependency and formula explanation | `DependencyTracer`, `FormulaExplainer` |
| `analysis::reports` | Formatted output | `TableBuilder`, `PLSummaryReport` |
| `extensions` | Runtime extensions | `CorkscrewExtension`, `CreditScorecardExtension` |
| `templates` | Build-time model helpers | `TemplatesExtension`, `VintageExtension`, `RealEstateExtension` |

Most types re-export from `finstack_quant_statements_analytics::analysis::*`.

## Conventions

- Ratios are plain scalars: `2.0` means `2.0x`, `0.40` means `40%`.
- Percentage inputs use decimal form: `0.10` means `10%`.
- `ScenarioDefinition.overrides` maps `node_id → scalar`, broadcast across forecast periods; historical actuals are preserved when the model has an actuals cutoff.
- On native targets, sensitivity diagonal runs and statement Monte Carlo paths use Rayon; results remain deterministic for a fixed seed.

## Verification

```bash
cargo test -p finstack-quant-statements-analytics
cargo doc -p finstack-quant-statements-analytics --no-deps
```

## See also

- `finstack-quant/statements/README.md`
- `finstack-quant/valuations/README.md`
- `finstack-quant/core/README.md`
