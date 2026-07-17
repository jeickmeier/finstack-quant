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
└──────────┬───────────┘   (no cargo features; all-or-nothing)
           │
 ┌─────────┴──────────────────────────────────────────────────────────────────────────────────┐
 │ Domain crates (14 bound in Python/WASM)                                                     │
 │                                                                                             │
 │  core                 ← primitives: money/fx, dates, market data, math, expr engine, config │
 │  analytics            ← performance/risk statistics over numeric slices                     │
 │  attribution          ← multi-period P&L attribution (waterfall, Taylor, metrics-based)     │
 │  cashflows            ← schedule generation, accrual, currency-safe dated flows             │
 │  covenants            ← covenant definition, evaluation, breach forecasting                 │
 │  factor-model         ← factor primitives, matchers, credit calibration, covariance         │
 │  features             ← vectorized panel feature transforms (bindings-facing leaf)          │
 │  margin               ← CSA specs, VM/IM (SIMM, schedule, CCP), FRTB-SBA, SA-CCR, XVA       │
 │  monte_carlo          ← processes, discretization, Philox RNG, payoffs, MC engine           │
 │  valuations           ← instruments, pricing, models, calibration, metrics (mid-stack hub)  │
 │  statements           ← model graph (Value > Forecast > Formula), evaluation                │
 │  statements-analytics ← DCF, scenario sets, sensitivity, ECL, backtesting                   │
 │  scenarios            ← deterministic shock/roll DSL + engine                               │
 │  portfolio            ← positions/books; base-currency rollups (top of stack)               │
 │                                                                                             │
 │ Supporting crates                                                                           │
 │  valuations/macros    ← FinancialBuilder + FocusedPricingOverrides derives                  │
 │  test-utils           ← golden-test framework (dev-dependency only; not published surface)  │
 │  finstack-quant-py    ← Python bindings (PyO3); src/bindings/ mirrors the 14 domains        │
 │  finstack-quant-wasm  ← WASM bindings (wasm-bindgen); src/api/ + hand-written JS facade     │
 └─────────────────────────────────────────────────────────────────────────────────────────────┘

Dependency direction (verified 2026-07-16):
  core → {analytics, cashflows, covenants, features, margin, monte_carlo}
       → factor-model → valuations → {attribution, statements}
       → scenarios → portfolio
  `valuations` is the true mid-stack hub: it consumes margin, monte_carlo,
  factor-model, covenants, analytics and cashflows. Bindings depend on the Rust
  crates; no Rust crate depends on a binding.
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
- **analytics**: Performance/risk statistics (`Performance` entry point, `beta`, `correlation`).
- **attribution**: Multi-period P&L attribution, including waterfall, Taylor and metrics-based methods.
- **cashflows**: Schedule generation, accrual calculations and currency-safe dated flows.
- **covenants**: Covenant definitions, evaluation and breach forecasting.
- **factor-model**: Factor primitives, matching, credit calibration and covariance structures.
- **features**: Vectorized panel feature transforms.
- **valuations**: Instrument cashflows, pricing, risk; currency‑preserving period aggregation; explicit FX collapse with policy stamping; private‑credit and real‑estate readiness.
- **statements**: Deterministic period evaluation with precedence: **Value > Forecast > Formula**; corkscrew schedules; optional balance‑sheet articulation; long/wide DataFrame exports.
- **statements‑analytics**: Credit covenant forecasting, alignment analysis, reporting utilities.
- **scenarios**: DSL with quoting, selectors, and globs; deterministic preview/composition; phase‑ordered execution with precise cache invalidation.
- **portfolio**: Positions/books, period alignment, and deterministic aggregation to base currency with explicit FX.
- **margin**: CSA specifications, VM/IM calculators, netting sets, ISDA SIMM.
- **monte_carlo**: Simulation engine, time grids, PhiloxRng, path capture, pricing evaluation.

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
