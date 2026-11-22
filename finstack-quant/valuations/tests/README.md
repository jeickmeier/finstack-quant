# Valuations tests

Integration tests for `finstack-quant-valuations`: cross-family instrument
pricing, cashflow generation, risk metrics, external-reference golden vectors,
and the registry, schema, and serde contracts that keep the wire format stable.

`finstack-quant-valuations` sets `autotests = false` in `Cargo.toml` and
compiles a single integration target, `[[test]] name = "valuations"` rooted at
[`tests/main.rs`](main.rs). Every module under `tests/` is pulled into `main.rs`
via `#[path = "..."]` so the crate links once instead of once per file.
[`tests/instruments.rs`](instruments.rs) is the product-tree module under
`valuations`. Unit tests for internal implementation details stay in
`#[cfg(test)]` modules under `src/`.

## Crate ownership

Single-family product tests live in their family crate under `<family>/tests/`
so changing one family does not rebuild or relink the umbrella crate.

The umbrella `valuations` target keeps:

- Cross-family tests (multi-family pricing, risk, cashflow, and payment-boundary suites)
- Registry, wire-key, schema, golden, and serialization contracts
- Framework-behaviour tests (`core_framework`, which exercises `valuations-core` over real instruments)
- Tests that require `finstack-quant-calibration` or JSON/envelope loading (`InstrumentJson`, `json_loader`, `json_examples`, `pricer::json`, `schema`)

`instruments/{cds,cds_index,cds_option,cds_tranche}/` remain in the umbrella
target rather than `finstack-quant-valuations-credit`: `calibration` depends on
`credit`, so `credit` cannot take a calibration dev-dependency without creating
a cycle, and helper-closure analysis shows every test file in `cds_index/`,
`cds_option/`, and `cds_tranche/` imports its calibration-pinned sibling helper
(`test_utils.rs`, `common.rs`, or `helpers.rs`), leaving zero self-contained
calibration-free files.

## Modules in `tests/main.rs`

Multi-file module trees:

| Module | Covers |
|--------|--------|
| `instruments` | Product tests (`tests/instruments.rs`), wire-key contracts, dependency completeness, and registry/serde/fixture contract tests. See [`instruments/README.md`](instruments/README.md). |
| `core_framework` | Framework behaviour (`tests/core.rs`): generic pricer, registry, enrichment, breakeven, DV01, FD Greeks, theta, VaR/HVaR, cross-factor, and cashflow export over built-in instruments. |
| `metrics` | Greeks and sensitivities (`tests/metrics.rs`): analytical-vs-FD convergence, determinism, sign conventions, invariants (proptest), cross-family edge cases, vanna-volga. |
| `cashflows` | `CashflowProvider` contract compliance and the instrument-to-`finstack_quant_cashflows` bridge. |
| `integration` | End-to-end workflows, metrics strict mode, JSON round-trips, schema parity, TypeScript export. |
| `golden` | External-reference parity against Bloomberg, QuantLib, and closed-form values. See [`golden/README.md`](golden/README.md). |
| `sanity_invariants` | Internal consistency checks (par-rate self-consistency, pay/receive symmetry, DV01 magnitude bands, cross-implementation parity) and `test_quantlib_external_parity.rs` against `data/quantlib_parity/`. |

Single-file modules:

| Module | Covers |
|--------|--------|
| `canonical_contracts` | Canonical byte encoding, content hashes, and key-insertion-order invariance for the instrument and calibration envelopes |
| `cashflow_export_schema` | JSON Schema coverage of `InstrumentCashflowEnvelope` and its row fields |
| `default_attribute_consistency` | Static source audit over every instrument `types.rs`: builder defaults carry a matching `#[serde(default)]`, and no type stores the legacy full-override bag |
| `field_units` | Field-unit tables (`results::*_field_units`) cover every field of each family result type in struct order |
| `metric_request_contract` | Requested metrics on JSON pricing entry points either appear in `ValuationResult::measures` or return an error |
| `payment_boundaries` | Period-cash boundary behaviour across payment dates |
| `performance_context_bump` | Context-bump capture and direct-stencil contract (`tests/performance/context_bump.rs`) |
| `phase2_strictness` | Persisted calibration steps reject unknown flattened fields |
| `production_*` | Production quant-audit regressions (`production_audit`, `production_barrier_ndf_audit`, `production_inflation_audit`, `production_risk_audit`, `production_simm_sensitivity_audit`, `production_simplicity_regressions`) |
| `registry_snapshot` | Pins the standard pricer and metric registries against `tests/data/registry_snapshot.json` |
| `schema_audit` | Every instrument `example()` serializes to valid JSON, round-trips through `InstrumentEnvelope`, and matches the checked-in schema |

## Directory map

```
tests/
├── main.rs                 # single [[test]] target (`valuations`); wires modules with #[path]
├── <module>.rs             # top-level modules pulled into main.rs
│
├── common/                 # Equity-option builders for the metrics module
│   └── builders.rs         # TestOptionBuilder, simple_call_option, simple_put_option
│
├── support/                # Instrument builders shared across modules
│   ├── mod.rs              # re-exports local helpers plus path-included family/calibration helpers
│   └── credit.rs           # CDS builders and calibrated hazard curves (needs calibration)
│
├── core/                   # framework-behaviour tests (wired by core.rs)
├── instruments/            # product directories + contract tests + generated fixtures
├── metrics/
├── cashflows/
├── performance/
├── sanity_invariants/
├── integration/            # metrics/, schema/, serialization/
├── golden/                 # runner + schema + data/ (bloomberg, quantlib, regression)
├── fixtures/               # JSON regression fixtures for production audits
│
└── data/
    ├── canonical/          # instrument + calibration canonical bytes and SHA-256
    ├── quantlib_parity/    # T0/T1 fixtures shared with finstack-quant-attribution
    └── registry_snapshot.json
```

## Shared helpers

Market-data builders, fixtures, tolerances, assertions and the parity helpers
live in the dev-only crate
[`finstack-quant-valuations-test-support`](../test-support/README.md), which
every valuations crate dev-depends on. Import them by crate path from any
module or `#[cfg(test)]` block:

```rust
use finstack_quant_valuations_test_support::fixtures::{base_date, usd_discount_curve};
use finstack_quant_valuations_test_support::builders::TestMarketBuilder;
use finstack_quant_valuations_test_support::tolerances::{ANALYTICAL, NUMERICAL, TIGHT};
use finstack_quant_valuations_test_support::test_helpers::{dates, flat_discount_curve};
use finstack_quant_valuations_test_support::{assert_parity, parity::ParityConfig};
```

Tolerances come from one module, `tolerances`, whose tiers run from `TIGHT`
through `ANALYTICAL`, `NUMERICAL`, `CURVE_PRICING`, `RELATIVE` and
`BUMP_VS_ANALYTICAL` to `STATISTICAL`.

**`support/`** holds the instrument builders that stay with this crate because
they build instruments across families or need `finstack-quant-calibration`
(`credit.rs`). `support/mod.rs` also path-includes shared helpers owned by
family crates (for example `../../rates/tests/support/rates.rs`) and
calibration (`../../../calibration/tests/support/calibration.rs`). Modules pull
them through `crate::instruments::test_support::*` or re-exports in `metrics.rs`
(`credit_support`, `option_support`).

## Tolerance policy

`test-support/src/tolerances.rs` — used by `metrics` and `core_framework`:

| Constant | Value | Use case |
|----------|-------|----------|
| `TIGHT` | 1e-10 | Analytical solutions, round-trip identity |
| `STANDARD` | 1e-6 | Finite-difference vs analytical |
| `LOOSE` | 1e-3 | Monte Carlo, cross-methodology |
| `PERCENT_001` / `PERCENT_01` / `PERCENT_1` / `PERCENT_5` | 1e-4 / 1e-3 / 1e-2 / 5e-2 | Relative bands |
| `NEAR_ZERO` | 1e-8 | Near-zero guard for relative-tolerance fallback |

`test-support/src/test_helpers.rs` (`tolerances` submodule) — used by `instruments`:
`ANALYTICAL` (1e-6), `NUMERICAL` (1e-4), `CURVE_PRICING` (5e-3), `RELATIVE`
(1e-2), `BUMP_VS_ANALYTICAL` (1.5e-2), `STATISTICAL` (2e-2).

Per [`.agents/rules/rust/testing-standards.md`](../../../.agents/rules/rust/testing-standards.md),
a failing test is fixed at the root cause, not by widening a tolerance. A
tolerance above its natural tier needs a comment naming the convention or
methodology difference that justifies it.

## Determinism

- No test derives a date from the system clock. Dates come from
  `time::macros::date!`, from `fixtures::base_date()`, or from
  `test_helpers::dates::TODAY` (both in the test-support crate). The one
  `SystemTime::now()` call in the tree (`instruments/registry_coverage.rs`)
  only names a scratch directory.
- Monte Carlo and stochastic paths take explicit seeds for bit-identical repeats.
- Proptest regressions are committed next to their tests
  (`*.proptest-regressions`) and must not be deleted to make a run pass.
- No test reaches the network or an external service.

## Slow tests

Long-running cases, including the golden pricing walk, carry
`#[ignore = "slow: covered by mise run rust-test --slow"]`. Mark a test slow
when it is a large property run, a multi-scenario loop, a Monte Carlo
convergence check, a calibration round-trip, or a vendor-parity sweep.

## Running

Use the `mise` test tasks (which route through `cargo nextest` and skip doc
tests in the normal loop):

```bash
# Whole valuations integration target
mise run rust-test-integration -- finstack-quant-valuations valuations

# Filter within the valuations crate (substring match on test path)
mise run rust-test-filter -- finstack-quant-valuations instruments::bond::
mise run rust-test-filter -- finstack-quant-valuations metrics::
mise run rust-test-filter -- finstack-quant-valuations registry_snapshot

# Family crate tests
mise run rust-test-crate -- finstack-quant-valuations-rates

# Whole workspace (final gate / CI)
mise run rust-test

# Ignored (slow) tests
mise run rust-test --slow
```

Schema and fixture drift is gated separately, because it depends on generated
artifacts rather than on test code:

```bash
mise run gen-check
```

## Adding tests

**Single-family product test** — add it to `<family>/tests/` so it runs without
rebuilding the umbrella crate.

**New instrument** — follow [`ADDING_AN_INSTRUMENT.md`](../ADDING_AN_INSTRUMENT.md)
(steps 7–9 are the tests, the persistence manifest and the regenerated fixture).

**New module in an existing umbrella tree (`metrics`, `cashflows`, `core`, `integration`, `sanity_invariants`)** — add the file to the directory and declare it in that directory's `mod.rs` (or `tests/<tree>.rs`).

**New top-level umbrella module** — add the `.rs` file under `tests/` and wire
it into [`tests/main.rs`](main.rs) with `#[path = "..."] mod ...;` (`autotests`
is disabled).

**New golden fixture** — add JSON under
`golden/data/pricing/<golden-type>/<instrument>/`
with a full `metadata` block (source, source detail, capture and review fields,
regen command) and a tolerance with a reason for each expected metric. Screenshot
evidence is mandatory for `bloomberg-screen` and `intex` sources. See
[`golden/README.md`](golden/README.md).

## Test-writing conventions

- Arrange / Act / Assert, one logical assertion per test.
- Name tests for the behavior, not for a ticket: `dv01_is_positive_for_a_long_position`,
  not `test_case_3`.
- Build objects through public constructors and builders. An integration test
  that needs a `pub(crate)` item is testing the wrong layer.
- Every expected number needs documented provenance — an analytical derivation,
  a mathematical invariant, a round-trip, or a golden fixture. A bare constant
  with no comment can silently pin a bug.
- Assert on policy stamps (rounding context, numeric mode, FX policy) where the
  result envelope carries them.

## Reference sources

| Source | Used for |
|--------|----------|
| Bloomberg (SWPM, CDSW, and related screens) | IRS, CDS, CDS option, cap/floor, swaption, FRA, callable bond |
| QuantLib | Deposits, FRA, bonds, equity/FX options, barriers, Asians, lookbacks, caps/floors, swaptions, CDS |
| ISDA Standard Model | CDS pricing conventions |
| Closed-form formulas and textbooks | Black-Scholes, Bachelier, Hagan SABR, Hull-White |

New vendor references belong in `golden/`, where the fixture schema forces
provenance metadata; internal-consistency checks belong in `sanity_invariants/`.
The pre-existing `sanity_invariants/test_quantlib_external_parity.rs` and the
`quantlib_parity.rs` modules under instrument directories predate that split.
