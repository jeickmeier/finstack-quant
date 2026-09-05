# finstack-quant-scenarios

Deterministic shocks and time rolls applied to market data, financial-statement
models, and instrument collections. Scenarios are serde-stable data (`ScenarioSpec`
+ `OperationSpec`), not a parsed text DSL, and are executed by a single
phase-ordered engine.

## Where it sits

Depends on `finstack-quant-core` (market data, dates, hierarchy),
`finstack-quant-statements` (forecast nodes), `finstack-quant-valuations`
(pricing-aware time rolls, instrument shocks), and `finstack-quant-attribution`
(horizon P&L decomposition). `finstack-quant-portfolio` consumes it for
`scenario_pnl` / `apply_and_revalue`; the umbrella crate and both binding crates
re-export it. No cargo features.

Statement-*local* named scenario sets — scalar model overrides with no market or
instrument effects — live in `finstack-quant-statements-analytics` instead.

## Public surface

| Item | Role |
|------|------|
| `ScenarioSpec` | `id`, optional `name`/`description`, ordered `operations`, `priority`, `resolution_mode` |
| `OperationSpec` | One shock or roll; 24 variants across market, instrument, statement, and time families |
| `ScenarioEngine` | `new` / `with_config`, `try_compose`, `apply` |
| `ExecutionContext<'a>` | `&mut MarketContext`, optional `&mut FinancialModelSpec`, optional instruments, optional `rate_bindings`, optional calendar, `as_of` |
| `ApplicationReport` | Operation counters, `ScenarioChangeManifest`, `Warning`s, `ResultsMeta` stamp, optional `RollForwardReport` |
| `ApplicationEnvelope` | JSON envelope of the mutated market/model plus the report (used by bindings) |
| `RateBindingSpec` | Links a statement `NodeId` to a curve tenor, used by `OperationSpec::RateBinding` |
| `HorizonAnalysis` / `HorizonResult` | Scenario + attribution for one instrument's decomposed total return |
| `templates` | `TemplateRegistry`, `RegisteredTemplate`, `TemplateMetadata` |
| `envelope` | `ScenarioEnvelope`, `ScenarioSchema`, `SCENARIO_CONTRACT` for strict versioned persistence |
| `Warning`, `Error`, `Result` | Structured non-fatal warnings and the crate error type |

Supporting types re-exported at the crate root: `CurveKind` (`Discount`,
`Forward`, `ParCDS`, `Inflation`, `Commodity`), `TimeRollMode`
(`BusinessDays`, `CalendarDays`, `Approximate`), `TenorMatchMode` (`Exact`,
`Interpolate`) and `Compounding` are owned by this crate;
`HierarchyTarget` comes from `finstack_quant_core::market_data::hierarchy`,
`InstrumentType` from `finstack_quant_valuations::pricer`, and `NodeId` from
`finstack_quant_statements::types`.

`ScenarioSpec::resolution_mode` is a
`finstack_quant_core::market_data::hierarchy::ResolutionMode`
(`MostSpecificWins` by default, `Cumulative` the other variant). It is *not*
re-exported here — import it from `finstack-quant-core`.

The `adapters` and `utils` modules are `pub(crate)`; the only adapter items
exposed are `apply_time_roll_forward` and `ArbitrageViolation` at the crate root.

## Operation families

+ **Market data.** `MarketFxPct`, `EquityPricePct`, `CurveParallelBp`,
  `CurveNodeBp`, `VolIndexParallelPts`, `VolIndexNodePts`,
  `VolSurfaceParallelPct`, `VolSurfaceBucketPct`, `BaseCorrParallelPts`,
  `BaseCorrBucketPts`, `AssetCorrelationPts`, `PrepayDefaultCorrelationPts`.
+ **Instruments.** `InstrumentPricePctByType` / `ByAttr` and
  `InstrumentSpreadBpByType` / `ByAttr`. The `ByAttr` variants match against the
  instrument's own metadata map (`Attributes::meta`; tag sets are ignored) with
  AND semantics over case-insensitive key/value pairs. There is no glob or
  pattern syntax, and an empty attribute map is rejected at validation — use the
  `ByType` variant with an explicit instrument-type list for a broad shock.
+ **Statements.** `StmtForecastPercent`, `StmtForecastAssign`, `RateBinding`.
+ **Hierarchy-targeted.** `HierarchyCurveParallelBp`,
  `HierarchyVolSurfaceParallelPct`, `HierarchyEquityPricePct`,
  `HierarchyBaseCorrParallelPts`.
+ **Time.** `TimeRollForward` (period, `apply_shocks`, `roll_mode`).

## Units

+ `Pct` fields are percentage points: `5.0` means +5%.
+ `Bp` fields are additive basis points: 1 bp = 1e-4, except commodity
  `CurveParallelBp` / `CurveNodeBp` where `bp` is percent of the
  price-curve forward (`5.0` = +5%).
+ Vol-index `Pts` are index points (`1.0` on 18.5 → 19.5).
+ Correlation / base-corr `Pts` are decimal correlation (`0.02` = +0.02).
+ Vol-surface shocks are sticky-strike multiplicative grid moves. The
  calendar-spread check is fixed-strike and is not a no-arbitrage
  certificate.

## Execution order

`ScenarioEngine::apply` validates the spec, then runs fixed phases:

| Phase | Work |
|-------|------|
| −1 | Expand hierarchy-targeted operations into concrete market identifiers. Targets that match nothing emit `Warning::HierarchyNoMatch`. |
| 0 | `TimeRollForward`, if present. With `apply_shocks = false` the engine returns immediately after this phase. |
| 1 | Market data. `MarketBump` effects are batched within one operation, then flushed. |
| 2 | Rate bindings, when `ExecutionContext::rate_bindings` is set. |
| 3 | Statement forecast adjustments. |
| 4 | Statement re-evaluation. |

Phase 1 batches bumps per operation, not per scenario. Bumps queued by one
operation are flushed before the next operation's effects are generated, so
every adapter reads a fully-applied prior state — which is what sequential
cross-curve calibration depends on. A bump that would land on the same target
as an already-queued one also forces an early flush, so the two compose
(`pre × (1+a) × (1+b)`) instead of overwriting each other in the batch map.
Each flush rebuilds `MarketContext` through `bump`, so a scenario with N market
operations performs up to N rebuilds, not one.

`validate()` enforces a non-empty `id`, at most one `TimeRollForward`, and each
operation's own invariants. Statement operations against a `None` model return a
typed error rather than silently no-op'ing.

**Application is not atomic.** Operations mutate `ctx.market` (and the model) in
place; a later failure leaves earlier mutations applied with no rollback. Apply
to a clone and swap on success if you need all-or-nothing semantics — which is
exactly what the Python and WASM bindings do, since they operate on deserialized
copies.

## Composition

`ScenarioEngine::try_compose` merges specs with a stable sort on `priority`
(lower runs first), concatenates their operations, joins ids with `+`, and
returns a validation error if the merge would produce more than one
`TimeRollForward`. `resolution_mode` is preserved when all inputs agree and
falls back to `ResolutionMode::Cumulative` when they disagree.

## Change manifest and invalidation

`ApplicationReport::changes` is a `ScenarioChangeManifest` (in
`finstack_quant_scenarios::engine`) recording exactly what moved:

+ `market_targets` — resolved `ScenarioMarketTarget` values (curve, vol index,
  base correlation, vol surface, equity price, FX pair). These are post-expansion
  identifiers, never unresolved hierarchy paths.
+ `changed_instrument_indices` — zero-based indices of mutated instruments.
+ `as_of_changed`, `portfolio_shape_changed`, `all_dirty`.

`all_dirty` is set for an effective time roll, because date-sensitive values can
move without any explicit market target changing. Downstream selective
repricing (`finstack_quant_portfolio::valuation::revalue_affected`) reads this
manifest rather than guessing.

Counters distinguish `user_operations` (what the caller wrote),
`expanded_operations` (direct operations remaining after hierarchy expansion
and resolution-mode dedup), and `operations_applied` (low-level effects that
landed). These counts are not directly comparable: one operation may produce
zero, one, or many effects. Inspect `changes` and `warnings` for coverage.

## Templates

Five historical stress templates are embedded as JSON from
[`data/templates/`](data/templates): `gfc_2008`, `covid_2020`,
`rate_shock_2022`, `svb_2023`, `ltcm_1998`. Each carries `TemplateMetadata`
(event date, asset classes, tags, `Severity`) and named components, so a caller
can take the whole scenario or just one leg.

```rust
use finstack_quant_scenarios::templates::TemplateRegistry;

fn rates_leg() -> finstack_quant_scenarios::Result<()> {
    let registry = TemplateRegistry::with_embedded_builtins()?;
    for metadata in registry.list() {
        println!("{} — {}", metadata.id, metadata.name);
    }

    let gfc = registry.get("gfc_2008").expect("built-in template");
    let rates_only = gfc
        .component("gfc_2008_rates")
        .expect("component id from metadata.components");
    println!("{} operations", rates_only.operations.len());
    Ok(())
}
```

Template market-data identifiers (`USD-SOFR`, `USD-IG`, `SPX_VOL`, …) are modern
placeholders, not historical instruments. Rewrite them for your own market data
before applying.

## Quick start

```rust
use finstack_quant_core::market_data::context::MarketContext;
use finstack_quant_core::market_data::term_structures::DiscountCurve;
use finstack_quant_scenarios::{
    CurveKind, ExecutionContext, OperationSpec, ScenarioEngine, ScenarioSpec,
};
use time::macros::date;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let as_of = date!(2025 - 01 - 01);
    let mut market = MarketContext::new().insert(
        DiscountCurve::builder("USD_SOFR")
            .base_date(as_of)
            .knots([(0.0, 1.0), (5.0, 0.80)])
            .build()?,
    );

    let scenario = ScenarioSpec {
        id: "stress_test".into(),
        name: Some("Q1 Stress Test".into()),
        description: None,
        operations: vec![OperationSpec::CurveParallelBp {
            curve_kind: CurveKind::Discount,
            curve_id: "USD_SOFR".into(),
            discount_curve_id: None,
            bp: 50.0,
        }],
        priority: 0,
        resolution_mode: Default::default(),
        hazard_bump_mode: Default::default(),
    };

    let mut ctx = ExecutionContext {
        market: &mut market,
        model: None,
        instruments: None,
        rate_bindings: None,
        calendar: None,
        as_of,
    };

    let report = ScenarioEngine::default().apply(&scenario, &mut ctx)?;
    println!("Applied {} effects", report.operations_applied);
    for warning in &report.warnings {
        eprintln!("warning: {warning}");
    }
    Ok(())
}
```

## Bindings

+ **Python:** `finstack_quant.scenarios` — `ScenarioSpec`, `TemplateMetadata`,
  `OperationSpec`, `CurveKind`, `TimeRollMode`, `TenorMatchMode`, `Compounding`, `RateBindingSpec`,
  `apply_scenario`, `apply_scenario_to_market`, `compose_scenarios`,
  `validate_scenario_spec`, `parse_scenario_spec`, `build_scenario_spec`,
  `compute_horizon_return`, the template helpers, and
  `finstack_quant.scenarios.schema`.
+ **WASM:** the `scenarios` namespace from `finstack-quant-wasm/index.js`
  (`exports/scenarios.js`) — `parseScenarioSpec`, `buildScenarioSpec`,
  `composeScenarios`, `validateScenarioSpec`, `applyScenario`,
  `applyScenarioToMarket`, `computeHorizonReturn`, and the template helpers.
  Unsuffixed builders, composers, and template helpers return structured specs;
  JSON remains explicit through Python `to_json`/`from_json` or JSON-named inputs.

## Schemas

This crate owns [`schemas/scenarios/1/scenario.schema.json`](schemas/scenarios/1),
listed in [`schemas/index.json`](schemas/index.json).

```bash
cargo run -p finstack-quant-scenarios --bin gen_scenario_schemas -- --write
mise run rust-check-schemas
```

## Verification

```bash
mise run rust-test                                    # whole workspace, cargo-nextest
cargo nextest run -p finstack-quant-scenarios         # this crate only
cargo nextest run -p finstack-quant-scenarios --test spec_validation_test
mise run rust-lint
```

Do not invoke `cargo test` directly in this workspace — it pulls in doc tests.
Integration suites live under [`tests/`](tests). `mod.rs` aggregates `engine/`,
`hierarchy_targeting/`, `integration/`, and `shocks/`; `templates_integration.rs`
aggregates `templates/`. Contract tests `canonical_contract.rs` and
`schema_contract.rs` run against [`tests/data/canonical/`](tests/data/canonical).
Two suites stand alone: `par_cds_bump.rs` (par-CDS curve bump behaviour) and
`report_serialization.rs` (`RollForwardReport` / `ArbitrageViolation` wire
stability).
Benchmarks are documented in [`benches/README.md`](benches/README.md).

## References

Day-count and business-day conventions, period notation, and stress-test
sources: [`docs/REFERENCES.md`](../../docs/REFERENCES.md). Workspace-wide
guarantees: [INVARIANTS.md](../../INVARIANTS.md).

## License

MIT OR Apache-2.0
