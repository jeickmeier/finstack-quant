# finstack-quant-core integration tests

Public-API and cross-module tests for `finstack-quant-core`. Unit tests stay in
`#[cfg(test)]` blocks next to the code they cover; this directory exercises the
crate the way a downstream caller would.

## Layout

Each integration target is a single root `.rs` file. Targets that grew past one
file keep a same-named directory beside them and pull submodules in with
`#[path = "..."]` (Rust's `tests/` discovery only treats top-level files as
targets, so the directory is not compiled twice).

```
tests/
├── cashflow.rs             + cashflow/    discounting, irr, primitives
├── contract.rs             + contract/    canonical bytes, descriptor, diagnostics, load limits
├── dates.rs                + dates/       rules, calendars, adjustment, daycount, schedules, DateExt
├── expr.rs                 + expr/        AST, context, eval, functions, serde
├── infrastructure.rs       + infrastructure/  config, explain, ResultsMeta
├── market_data.rs          + market_data/ curves/, surfaces/, context, bumps, diff, fx, scalars,
│                                          credit_index, hierarchy, serde
├── math.rs                 + math/        interp, solver, integration, stats, summation
├── money.rs                + money/       FX conversion, rounding contexts
├── serde.rs                + serde/       wire-format goldens, roundtrips
├── types.rs                + types/       Rate / Bps / Percentage
├── golden_tests.rs         + golden/      reference-value fixtures (see golden/README.md)
├── canonical_api.rs                       npv/irr/quadrature result and error consistency
├── phase2_strictness.rs                   nested serde strictness + finite-value regressions
├── sobol_golden.rs                        Sobol direction numbers vs Joe & Kuo (2008)
└── data/                                  fixture data (see data/README_sobol.md)
    ├── canonical/                         MarketContextState canonical bytes + sha256 pin
    └── sobol_joe_kuo_d2_40.txt
```

## Helpers

There is no shared `tests/common/` module. Helpers are scoped to the target that
needs them, so a change to one domain's fixtures cannot silently move another
domain's numbers.

| Location | Provides |
|----------|----------|
| `dates/common.rs` | `make_date(y, m, d)`, `TestCal` (in-memory holiday calendar), `DAYCOUNT_TOLERANCE = 1e-12` |
| `math/common.rs` | `approx_eq(a, b, tol)`, `standard_knots`/`standard_dfs`, `two_point_knots`/`two_point_dfs` |
| `market_data/test_helpers.rs` | `sample_base_date()` (2024-01-01) and `sample_*_curve(id)` / `sample_vol_surface()` builders |
| `expr/common.rs` | Placeholder; expression tests use `SimpleContext` directly |

Tolerances that are not shared live next to their assertions, for example
`XIRR_TOLERANCE = 1e-6` in `cashflow/irr.rs` (Excel-compatible XIRR precision)
and `financial_tolerance(notional)` in `cashflow/discounting.rs`, which scales
with the amount rather than fixing an absolute epsilon.

## Running

```bash
# Everything in the crate (lib unit tests + all integration targets)
mise run rust-test-crate -- finstack-quant-core

# One target
mise run rust-test-integration -- finstack-quant-core cashflow

# One test, by substring
mise run rust-test-filter -- finstack-quant-core npv_100_cashflows

```

Workspace-wide: `mise run rust-test`. Do not invoke a bare `cargo test` — it
also runs doc tests, which this project keeps to a separate pass
(`mise run rust-doc`).

## Adding a test

1. Put the file under the subdirectory for its domain.
2. Wire it from the domain root file with `#[path = "domain/file.rs"] mod file;`.
3. Give the file a `//!` doc comment stating its scope.
4. Reuse the domain helper module for fixtures and tolerances rather than
   inventing new ones.

Name tests after the scenario and the expected outcome — existing examples are
`npv_100_cashflows_maintains_precision`, `npv_negative_rate_inflates_value`,
`market_context_state_rejects_unknown_top_level_fields`. Each test builds its own
fixtures; nothing may depend on execution order, wall-clock time, locale, or the
network. See
[`.agents/rules/rust/testing-standards.md`](../../../.agents/rules/rust/testing-standards.md)
for the workspace policy, and [INVARIANTS.md](../../../INVARIANTS.md) for the
determinism and currency-safety contracts these tests defend.

Reference-value fixtures go through the golden harness instead — see
[`golden/README.md`](golden/README.md).
