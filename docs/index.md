# Finstack Quant Documentation

Finstack Quant is a Rust-native financial computation engine with Python and
WebAssembly bindings. Start here when you need the public documentation map for
concepts, package usage, examples, references, and contributor-facing design
notes.

## Getting Started

- [`README.md`](../README.md): product overview, quick start, package map, and
  common development commands.
- [`finstack-quant-py/README.md`](../finstack-quant-py/README.md): Python package
  overview, build commands, imports, stubs, parity checks, and common pitfalls.
- [`finstack-quant-py/examples/notebooks/README.md`](../finstack-quant-py/examples/notebooks/README.md):
  notebook curriculum from foundations through pricing, analytics, statement
  modeling, portfolio scenarios, Monte Carlo, margin, XVA, and capstone
  workflows.

## Concepts

- [`REFERENCES.md`](REFERENCES.md): canonical sources for formulas, conventions,
  and market practice.
- [Financial methodologies](decisions/financial/README.md): calculation-to-method
  inventory, adoption rationale, selection behavior, and open evidence gaps.
- [`SERDE_STABILITY.md`](SERDE_STABILITY.md): canonical serde contract rules,
  source-of-truth policy, and reproducible generation requirements.
- [`CONTRACTS.md`](CONTRACTS.md): current persisted-contract matrix, strict
  loaders, exact v1 markers, canonical JSON, generated schemas, structured
  diagnostics, and materialization API links.
- [`MATERIALIZATION_BENCHMARKS.md`](../benchmarks/MATERIALIZATION_BENCHMARKS.md): reproducible
  portfolio materialization fixtures, performance gates, profiling procedure,
  and reference results.
- [`../finstack-quant-py/DOCS_STYLE.md`](../finstack-quant-py/DOCS_STYLE.md):
  Python binding documentation style for PyO3 comments, `.pyi` docstrings, and
  examples.
- [`../finstack-quant-py/parity_contract.toml`](../finstack-quant-py/parity_contract.toml):
  Python and WASM binding parity contract.

## Tutorials And Examples

The notebook curriculum is the main tutorial path:

1. `01_foundations`: money, dates, calendars, schedules, market data, curves,
   math, and configuration.
2. `02_pricing`: instrument JSON, valuation results, deposits, swaps, CDS,
   equity options, FX options, exotics, and attribution.
3. `03_analytics`: performance, risk, factor analytics, attribution, and
   reporting tear sheets.
4. `04_statement_modeling`: statement formulas, forecasts, sensitivities,
   covenants, and credit scoring.
5. `05_portfolio`: portfolio valuation, optimization, horizon total return,
   liquidity risk, and risk decomposition.
6. `06_scenarios`: scenario authoring, stress testing, and impact analysis.
7. `07_advanced_quant`: Monte Carlo, correlation, credit models, margin, XVA,
   and regulatory capital.
8. `08_capstone`: end-to-end credit portfolio workflow.
9. `09_reporting`: reporting tear sheets.

Run the full curriculum from the repository root:

```bash
mise run python-examples
```

## Development Notes

- [`decisions/`](decisions/README.md): accepted project policies, code standards,
  specific code decisions, financial conventions, and labeled methodology drafts, with examples,
  enforcement requirements, and separate follow-up registers for outstanding work.
- [`../CONTRIBUTING.md`](../CONTRIBUTING.md): contribution workflow, project
  principles, and binding-change checklist.
- [`../INVARIANTS.md`](../INVARIANTS.md): cross-crate numerical and API
  invariants that must hold across releases.
- [`CONTRACTS.md`](CONTRACTS.md): maintained contract catalog, including
  generation and CI drift checks.
- [`REMOVING_THE_UI.md`](REMOVING_THE_UI.md): how to remove the component
  registry without taking the quant engine or the analyst reader with it.

## Package Surfaces

- Rust: `finstack-quant/` contains the umbrella crate and domain crates.
- Python: `finstack-quant-py/` contains PyO3 bindings, stubs, tests, parity
  checks, and notebooks.
- WebAssembly: `finstack-quant-wasm/` contains wasm-bindgen exports, the
  JavaScript facade, TypeScript declarations, and WASM tests.
