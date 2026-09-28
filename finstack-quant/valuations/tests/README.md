# Valuations tests

Integration tests for `finstack-quant-valuations`: instrument pricing, cashflow
generation, calibration, market quotes, risk metrics, external-reference golden
vectors, and the schema/serde contracts that keep the wire format stable.

Every `.rs` file directly under `tests/` is its own cargo integration target
(`--test <name>`). Directories beside them are not targets; they are pulled into
a target with `#[path = "..."]`. Unit tests for internal implementation details
stay in `#[cfg(test)]` modules under `src/`.

## Targets

Large, multi-directory targets:

| Target | Covers |
|--------|--------|
| `instruments` | Per-instrument construction, cashflows, pricing, metrics, validation across 48 instrument directories, plus registry/serde/fixture contract tests. See [`instruments/README.md`](instruments/README.md). |
| `calibration` | Plan-driven calibration: bootstrap, repricing, bump invariants, hazard / inflation / swaption-vol / SVI / base-correlation steps, parametric (Nelson-Siegel) curves, failure modes and envelope diagnostics, explainability, validation, Bloomberg accuracy, the `examples/market_bootstrap` reference envelopes, and standalone term-structure property tests. |
| `metrics` | Greeks and sensitivities: analytical-vs-FD convergence, determinism, sign conventions, mathematical relationships, invariants (proptest), edge cases, vanna-volga, historical VaR quantiles. |
| `market` | `MarketQuote` serde, quote schema helpers, bump logic, and instrument construction from rate and credit quotes. |
| `cashflows` | `CashflowProvider` contract compliance and the instrument-to-`finstack_quant_cashflows` bridge. |
| `integration` | End-to-end workflows (100-bond multi-currency portfolio, FX settlement), metrics strict mode, JSON round-trips, schema parity, TypeScript export. |
| `golden` | External-reference parity against Bloomberg, QuantLib, and closed-form values. See [`golden/README.md`](golden/README.md). |
| `sanity_invariants` | Mostly internal consistency — par-rate self-consistency, pay/receive symmetry, DV01 magnitude bands, cross-implementation parity. One exception: `test_quantlib_external_parity.rs` reads the `data/quantlib_parity/` fixtures and compares bond, IRS, and FX-forward values against QuantLib. |

Single-file targets:

| Target | Covers |
|--------|--------|
| `canonical_contracts` | Canonical byte encoding, content hashes, and key-insertion-order invariance for the instrument and calibration envelopes |
| `cashflow_export_schema` | JSON Schema coverage of `InstrumentCashflowEnvelope` and its row fields |
| `credit_calibration` | `finstack_quant_models::factor::credit::calibration` end-to-end |
| `credit_decomposition` | `finstack_quant_models::factor::credit::decomposition` levels and period decomposition |
| `cross_factor_metrics_tests` | Cross-factor gamma against manual four-corner repricing |
| `default_attribute_consistency` | Static source audit over every instrument `types.rs`: builder defaults carry a matching `#[serde(default)]`, and no type stores the legacy full-override bag |
| `phase2_strictness` | Persisted calibration steps reject unknown flattened fields |
| `portfolio_loss` | Copula-based portfolio credit-loss simulation: determinism, VaR/ES estimator, overflow rejection |
| `return_floor_example` | Readable public-API example: MOIC/XIRR return-floor loan, priced and verified through public APIs only |
| `sabr_core_parity` | This crate's `SABRModel` vs the canonical Hagan/Obloj expansion in `finstack_quant_core` |
| `schema_audit` | Every instrument `example()` serializes to valid JSON, round-trips through `InstrumentEnvelope`, and matches the checked-in schema |

## Directory map

```
tests/
├── <target>.rs             # one cargo integration target each (see above)
│
├── common/                 # Shared fixtures for calibration / market / metrics
│   ├── fixtures.rs         # base_date, USD discount curves, standard notional
│   ├── tolerances.rs       # TIGHT / STANDARD / LOOSE / PERCENT_* constants
│   ├── assertions.rs       # assert_approx_eq, assert_relative_eq, range/sign helpers
│   └── builders.rs         # TestMarketBuilder, TestOptionBuilder
│
├── support/                # Narrow builders shared across several targets
│   ├── date.rs             # date(y, m, d)
│   ├── discount_forward_curves.rs, commodity_curves.rs, volatility.rs
│   ├── rates.rs, credit.rs, equity_fx_options.rs
│   ├── calibration.rs      # quote-set and MarketContext splitting helpers
│   └── metrics_risk_test_utils.rs  # include!()d by src/ unit tests, not by a target
│
├── instruments/            # 48 instrument dirs + contract tests + generated fixtures
├── calibration/            # incl. term_structures/ property tests
├── metrics/
├── market/                 # incl. build/{credit,rates}.rs
├── cashflows/
├── sanity_invariants/
├── integration/            # e2e/, metrics/, schema/, serialization/
├── golden/                 # runner + schema + data/ (bloomberg, quantlib, regression)
│
└── data/
    ├── canonical/          # instrument + calibration canonical bytes and SHA-256
    └── quantlib_parity/    # T0/T1 fixtures shared with finstack-quant-attribution
```

## Shared helpers

There are two independent helper trees; which one you get depends on the target.

**`common/`** is wired into `calibration`, `market`, and `metrics` via
`#[path = "common/mod.rs"] mod common;`. It also glob re-exports its submodules
at the module root, so `crate::common::TIGHT` and `crate::common::tolerances::TIGHT`
both resolve; prefer the qualified form.

```rust
use crate::common::fixtures::{base_date, usd_discount_curve, STANDARD_NOTIONAL};
use crate::common::assertions::{assert_approx_eq, assert_relative_eq};
use crate::common::builders::{TestMarketBuilder, TestOptionBuilder};
use crate::common::tolerances::{TIGHT, STANDARD, LOOSE};
```

**`instruments/common/`** is a different module with a different tolerance
vocabulary, wired only into the `instruments` target as `crate::common`. See
[`instruments/common/README.md`](instruments/common/README.md). Do not mix the
two names up: `tolerances::STANDARD` and `tolerances::ANALYTICAL` come from
different files.

**`support/`** holds small builders that several targets need. Each target
declares the ones it wants explicitly, so the local module name differs per
target — `crate::test_support::*` in `instruments`,
`crate::calibration_support::*` in `calibration`, `crate::credit_support::*` and
`crate::option_support::*` in `metrics`. Check the target's entry-point `.rs`
file for the local name. One file here, `metrics_risk_test_utils.rs`, is not
wired into any target at all: `src/metrics/risk/{hvar,var_calculator}.rs`
`include!()` it into their own `#[cfg(test)]` modules.

## Tolerance policy

`common/tolerances.rs` — used by `calibration`, `market`, `metrics`:

| Constant | Value | Use case |
|----------|-------|----------|
| `TIGHT` | 1e-10 | Analytical solutions, round-trip identity |
| `STANDARD` | 1e-6 | Finite-difference vs analytical |
| `LOOSE` | 1e-3 | Monte Carlo, cross-methodology |
| `PERCENT_001` / `PERCENT_01` / `PERCENT_1` / `PERCENT_5` | 1e-4 / 1e-3 / 1e-2 / 5e-2 | Relative bands |
| `NEAR_ZERO` | 1e-8 | Near-zero guard for relative-tolerance fallback |

`instruments/common/test_helpers.rs` — used by `instruments`:
`ANALYTICAL` (1e-6), `NUMERICAL` (1e-4), `CURVE_PRICING` (5e-3), `RELATIVE`
(1e-2), `BUMP_VS_ANALYTICAL` (1.5e-2), `STATISTICAL` (2e-2).

Per [`.agents/rules/rust/testing-standards.md`](../../../.agents/rules/rust/testing-standards.md),
a failing test is fixed at the root cause, not by widening a tolerance. A
tolerance above its natural tier needs a comment naming the convention or
methodology difference that justifies it.

## Determinism

- No test derives a date from the system clock. Dates come from
  `time::macros::date!`, from `common::fixtures::base_date()`, or from
  `test_helpers::dates::TODAY`. The one `SystemTime::now()` call in the tree
  (`instruments/registry_coverage.rs`) only names a scratch directory.
- Monte Carlo and copula paths take explicit seeds; `portfolio_loss` asserts
  that repeated runs are bit-identical.
- Proptest regressions are committed next to their tests
  (`*.proptest-regressions`) and must not be deleted to make a run pass.
- No test reaches the network or an external service.

## Slow tests

Long-running cases carry `#[ignore = "slow: covered by mise rust-test-slow"]`;
the golden pricing walk carries
`#[ignore = "slow: covered by mise goldens-test or mise rust-test-slow"]`. Mark
a test slow when it is a large property run, a multi-scenario loop, a Monte
Carlo convergence check, a calibration round-trip, or a vendor-parity sweep.

## Running

Use `cargo nextest`; a bare `cargo test` also runs doc tests, which this project
does not want in the normal loop.

```bash
# Whole workspace (what CI runs)
mise run rust-test

# Whole crate
cargo nextest run -p finstack-quant-valuations

# One target
cargo nextest run -p finstack-quant-valuations --test valuations instruments
cargo nextest run -p finstack-quant-valuations --test calibration
cargo nextest run -p finstack-quant-valuations --test valuations metrics

# A subtree within a target (filter is a substring match on the test name)
cargo nextest run -p finstack-quant-valuations --test valuations instruments::bond::
cargo nextest run -p finstack-quant-valuations --test calibration term_structures::
cargo nextest run -p finstack-quant-valuations --test valuations metrics::metrics::sign_conventions::

# Ignored (slow) tests
mise run rust-test-slow
cargo nextest run -p finstack-quant-valuations --test valuations instruments --run-ignored only

# Golden layers (Rust + Python)
mise run goldens-test
mise run goldens-test-strict
```

Schema and fixture drift is gated separately, because it depends on generated
artifacts rather than on test code:

```bash
mise run rust-check-schemas   # includes schema_audit and integration schema_parity
mise run gen-check            # schemas, fixtures, and bindings are idempotent
```

## Adding tests

**New instrument** — create `instruments/<name>/`, add the `#[path = ...] mod`
entry to [`instruments.rs`](instruments.rs), register the tag in
`instruments/coverage_manifest.toml`, and regenerate its canonical fixture with
`cargo run -p finstack-quant-valuations --bin gen_schemas -- --write`. See
[`instruments/README.md`](instruments/README.md).

**New calibration / metrics / market / cashflows test** — add the file to the
directory and the `mod` line to that directory's `mod.rs`. The entry-point `.rs`
file does not change.

**New golden fixture** — add JSON under
`golden/data/pricing/<golden-type>/<instrument>/`
with a full `metadata` block (source, source detail, capture and review fields,
regen command) and a tolerance with a reason for each expected metric. Screenshot
evidence is mandatory for `bloomberg-screen` and `intex` sources. See
[`golden/README.md`](golden/README.md).

**New single-file target** — put the `.rs` file directly under `tests/`. Cargo
picks it up automatically; there is no `[[test]]` entry to add.

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
four `quantlib_parity.rs` modules under `instruments/` (`cap_floor`,
`convertible`, `fra`, `swaption`) predate that split.
