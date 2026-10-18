---
trigger: always_on
description:
globs:
---
# Finstack Quant (Rust) — Deterministic Financial Computation Library

## Overview

Finstack Quant is a deterministic, cross‑platform financial computation engine with a Rust core and first‑class Python and WebAssembly bindings. It emphasizes accounting‑grade correctness (Decimal numerics), currency‑safety, stable wire formats, and predictable performance for statements, valuations, scenarios, and portfolio analysis.

## Project Purpose

Finstack Quant aims to provide:

- **Determinism**: Decimal for monetary amounts, f64 for analytics/pricing internals (see INVARIANTS.md §1); serial and parallel runs produce identical results.
- **Currency‑safety**: No implicit cross‑currency math; explicit FX policies stamped in results.
- **Stable schemas**: Strict serde names for long‑lived pipelines and golden tests.
- **Performance**: Vectorized and parallel execution without changing Decimal results.
- **Parity**: Ergonomic, parity‑checked APIs for Python and WASM.

## Architecture

```
Workspace (umbrella crate: finstack-quant)
┌──────────────────────┐
│ finstack-quant       │  -> unconditional re-exports of every domain crate
└──────────┬───────────┘   (features: `json-schema`, `jsonschema-validate`; both default)
           │
 ┌─────────┴──────────────────────────────────────────────────────────────────────────────────┐
 │ Domain crates (14 bound in Python/WASM)                                                     │
 │                                                                                             │
 │  core                 ← primitives: money/fx, dates, market data, math, expr engine, config │
 │  analytics            ← performance/risk statistics, correlation-matrix helpers             │
 │  cashflows            ← schedule generation, accrual, currency-safe dated flows             │
 │  covenants            ← covenant definition, evaluation, breach forecasting                 │
 │  features             ← vectorized panel feature transforms (bindings-facing leaf)          │
 │  margin               ← CSA specs, VM/IM (SIMM, schedule, CCP), FRTB-SBA, SA-CCR, XVA       │
 │  models               ← model kernels: closed-form, Monte Carlo (Philox RNG, processes,     │
 │                         payoffs, engine), PDE, trees, Fourier, vol, short-rate, credit,     │
 │                         factor models, copulas/correlation, liquidity                       │
 │  valuations/core      ← pricing framework: Instrument trait, pricer/metric registries,     │
 │                         Family, results, market conventions (no instrument types)           │
 │  valuations/{rates,   ← one crate per instrument family (packages                           │
 │   fixed-income,          finstack-quant-valuations-<family>), each with its Family static;  │
 │   credit, equity,        depend on valuations/core only, never on each other                │
 │   fx, exotics}                                                                              │
 │  valuations           ← umbrella: re-exports core + the family crates under unchanged     │
 │                         paths; structured credit, CMO, ABF, basket, real estate, composite, │
 │                         json_loader, STANDARD registries, schemas (the closed world)        │
 │  calibration          ← curve/hazard/vol bootstraps and global calibration on valuations   │
 │  attribution          ← multi-period P&L attribution (waterfall, Taylor, metrics-based)     │
 │  statements           ← model graph (Value > Forecast > Formula), evaluation                │
 │  statements-analytics ← DCF, scenario sets, sensitivity, ECL, backtesting                   │
 │  scenarios            ← deterministic shock/roll DSL + engine                               │
 │  portfolio            ← positions/books; base-currency rollups (top of stack)               │
 │                                                                                             │
 │ Supporting crates (not re-exported by the umbrella crate — depend on them directly)         │
 │  valuations/macros    ← FinancialBuilder derive                                             │
 │  valuations/test-support ← dev-only market builders, fixtures, tolerances, parity (core-only) │
 │  arrow-interchange    ← finstack-quant-arrow: TableEnvelope → Arrow RecordBatch             │
 │  test-utils           ← golden-test framework (dev-dependency only; not published surface)  │
 │  finstack-quant-py    ← Python bindings (PyO3); src/bindings/ mirrors the 14 domains        │
 │  finstack-quant-wasm  ← WASM bindings (wasm-bindgen); src/api/ + hand-written JS facade     │
 └─────────────────────────────────────────────────────────────────────────────────────────────┘

Dependency direction (read off the manifests):
  core → {analytics, cashflows, covenants, features, margin}
  core + analytics + cashflows → models
  core + cashflows + covenants + margin + models → valuations/core
  valuations/core → valuations/{rates, fixed-income, credit, equity, fx, exotics} (in parallel)
  valuations/core + the six family crates → valuations
  valuations (+ core, cashflows, models) → calibration
  calibration + valuations → attribution
  valuations (+ core, cashflows) → statements → statements-analytics (+ covenants, models)
  {attribution, calibration, models, statements, valuations} → scenarios
  {attribution, calibration, cashflows, margin, models, scenarios, valuations} → portfolio

  Short form: core → cashflows/models → valuations/core → the family crates → valuations →
  calibration → attribution → statements/scenarios → portfolio.
  `finstack-quant-valuations-core` (`finstack-quant/valuations/core`) is the pricing
  framework; the six family crates implement it and depend on nothing else in
  valuations; the umbrella `finstack-quant-valuations` re-exports all of them under
  the same paths and adds the closed world, so downstream crates and bindings depend
  on the umbrella only. Instruments that hold other families' legs (composite,
  basket, levered real estate) and the structured-credit tier (structured credit,
  CMO, asset-backed facility; depends on fixed-income and rates) stay in the umbrella.
  `margin` depends only on `core`; exposure generation is out of its scope. The
  `Marginable` trait is the seam and `valuations` implements it.
  `models` depends on `analytics`, `cashflows` and `core` only; Monte Carlo and
  factor models are modules of it (`models::monte_carlo`, `models::factor`),
  not crates.
  `calibration` depends on `valuations` (it calibrates against valuations
  pricers); `valuations` does not depend on `calibration`.
  Bindings depend on the Rust crates; no Rust crate depends on a binding.
```

## Cross‑Cutting Invariants

- **Determinism**: Decimal mode; stable ordering; parallel ≡ serial.
- **Currency‑safety**: Arithmetic on `Money` requires same currency; explicit FX conversions only.
- **Rounding/Scale policy**: Global policy; active `RoundingContext` stamped into results metadata.
- **FX policy visibility**: Applied conversion strategy recorded per layer (e.g., valuations, statements, portfolio).
- **Serde stability**: Strict field names; unknown fields denied on inbound types.
- **Time‑series standard**: `core::table` is the canonical serializable columnar surface. There is no Polars dependency; `valuations::results::dataframe` emits flat JSON for downstream pandas/Polars consumers.

## Core Responsibilities (by crate)

- **core**: `Money`, `Currency`, `Rate`; FX interfaces (`FxProvider`, `FxMatrix`); periods/calendars/day-count; expression engine (DAG planning, scalar evaluation over `&[f64]`); validation; config (rounding/scale); errors; `table` columnar envelope.
- **analytics**: Performance/risk statistics (`Performance` entry point, `beta`) and correlation-matrix helpers (`correlation`).
- **attribution**: Multi-period P&L attribution, including waterfall, Taylor and metrics-based methods.
- **cashflows**: Schedule generation, accrual calculations and currency-safe dated flows.
- **covenants**: Covenant definitions, evaluation and breach forecasting.
- **models**: Model kernels shared by pricers and calibrators: closed-form (Black-Scholes, Black-76, Bachelier), Monte Carlo (processes, discretization, Philox RNG, payoffs, engine), PDE, lattice trees, Fourier/COS, volatility (SABR, surfaces), short-rate, credit (PD, migration), factor models (matching, credit calibration, covariance), copulas/correlation and liquidity.
- **calibration**: Discount/forward/hazard/inflation curve bootstraps, vol-surface and SABR fits, base correlation and global calibration with validation and reporting.
- **features**: Vectorized panel feature transforms.
- **valuations**: Instrument cashflows, pricer registry, metrics/risk; currency‑preserving period aggregation; explicit FX collapse with policy stamping; private‑credit and real‑estate readiness.
- **statements**: Deterministic period evaluation with precedence: **Value > Forecast > Formula**; corkscrew schedules; optional balance‑sheet articulation; long/wide DataFrame exports.
- **statements‑analytics**: Credit covenant forecasting, alignment analysis, reporting utilities.
- **scenarios**: DSL with quoting, selectors, and globs; deterministic preview/composition; phase‑ordered execution with precise cache invalidation.
- **portfolio**: Positions/books, period alignment, and deterministic aggregation to base currency with explicit FX.
- **margin**: CSA specifications, VM/IM calculators, netting sets, ISDA SIMM.

## Language Bindings

### Python (finstack-quant-py)

- Wheels for major OSes; heavy compute releases the GIL; DataFrame‑friendly outputs.
- Binding Rust code under `finstack-quant-py/src/bindings/` mirrors the 14 crate domains.
- Names match Rust (e.g. `Date`, `sharpe`); no legacy aliases.

### WebAssembly (finstack-quant-wasm)

- Browser/Node support; JSON IO parity with serde; feature flags for tree‑shaking and small bundles.
- Binding Rust code under `finstack-quant-wasm/src/api/` with a hand-written JS facade at `index.js`.
- Public API is accessed via crate-domain namespaces (e.g. `core.Currency`, `analytics.sharpe`).

## Key Features

### Performance

- Rayon parallelism (unconditional on native targets; gated off for wasm32 via `cfg`); caches for hot paths.

### Safety & Standards

- Currency type safety; strict serde; ISO‑4217 currencies; ISDA day‑count conventions; no `unsafe`.

### Policy Visibility

- Results include numeric mode, parallel flag, rounding context, and any applied FX policy.

## Primary Use Cases

- **Statements modeling**: Build/evaluate models over periods with deterministic precedence.
- **Instrument pricing & risk**: Cashflows, PV/NPV, yields/spreads, DV01/CS01, options Greeks.
- **Scenario analysis**: Deterministic DSL across market/statements/valuations with preview.
- **Portfolio aggregation**: Stable rollups by book/entity/currency with explicit FX collapse.
- **Data interchange**: Stable serde names and DataFrame outputs for pipelines and notebooks.

## Development Philosophy

1. **Correctness first**; 2. **Performance second** (without changing Decimal outputs);
2. **Ergonomic APIs**; 4. **Documentation** for every public API; 5. **Testing** across unit/property/golden/parity.

## Technical Guidelines

- Follow `.cursor/rules/[rust|python|wasm]/` standards; deny `unsafe`.
- Keep cross‑currency math explicit via `FxProvider` and record policies in results.
- Prefer compile‑time validation and strict deserialization; stable serde names.
- Use `core::table` for columnar interchange; avoid ad-hoc series types.
- Ensure serial ≡ parallel in Decimal mode; stamp `RoundingContext` in all result envelopes.
