# Instrument test suite

Per-instrument construction, cashflow, pricing, metrics, and validation tests for
`finstack-quant-valuations`, plus the cross-cutting contract tests that hold the
instrument registry, its generated JSON fixtures, and its serde surface together.

Everything here is compiled into one integration target,
[`tests/instruments.rs`](../instruments.rs), which wires each directory in with
`#[path = "instruments/<name>/mod.rs"]`. These are integration tests: there is no
`--lib` path into them, and they may only use the crate's public API.

## Layout

```
instruments/
├── common/                 # Shared fixtures, tolerances, parity helpers (see common/README.md)
├── <instrument>/           # 48 directories: one per instrument, plus exotic_harness/
├── json_examples/          # 71 GENERATED canonical instrument fixtures (do not hand-edit)
├── coverage_manifest.toml  # registry tag -> fixture path -> persistence policy
│
├── registry_coverage.rs    # Every registry tag has a manifest entry, fixture, schema, and strict round-trip
├── serde_skip_guard.rs     # #[serde(skip)] is limited to documented derived-artifact caches
├── override_wire_shape.rs  # Focused pricing-override bags use the three canonical keys
├── dividend_yield_dependency.rs # Dividend-yield IDs are market scalars, not series
├── curve_dependency_completeness.rs # Instruments declare every discount curve they read
├── forward_curve_dependency_completeness.rs
├── forward_dependency_completeness.rs
├── equity_dependency_completeness.rs
├── fx_dependency_completeness.rs
├── market_edge_tests.rs    # Upfront conventions, accrual-on-default, ex-coupon, stub periods
└── test_option_bounds.rs   # Arbitrage-free option bounds (property tests)
```

Instrument directories are flat by default (`mod.rs` plus topic files). The
larger ones split into subdirectories:

| Directory | Subdirectories |
|-----------|----------------|
| `bond`, `cap_floor`, `irs` | `metrics/`, `validation/`, `integration/` |
| `swaption` | `core/`, `pricing/`, `market/`, `metrics/`, `edge_cases/`, `integration/` |
| `structured_credit` | `unit/` (with `components/`, `metrics/`), `integration/` |
| `term_loan`, `revolving_credit`, `fra` | `metrics/`, `validation/` |
| `deposit`, `fx_spot`, `inflation_swap` | `metrics/`, `integration/` |
| `equity` | `real_estate/` |

Split only when a directory outgrows a flat layout. The authoritative module
list is `mod.rs` in each directory, not this README.

## Generated fixtures

`json_examples/` is **generated output**, not hand-written test data. Each file
is the single canonical `finstack_quant.instrument/1` envelope for one registry
tag, serialized from that instrument's own `example()` provider:

```bash
# Rewrite schemas, the schema index, and every canonical fixture
cargo run -p finstack-quant-valuations --bin gen_schemas -- --write

# Fail on drift instead of rewriting
cargo run -p finstack-quant-valuations --bin gen_schemas -- --check
```

The generator deletes fixtures whose tag no longer exists, so a stale file is a
build-output problem, not a merge conflict to resolve by hand.

`coverage_manifest.toml` maps each registry tag to its fixture path and its
persistence policy. `registry_coverage.rs` asserts the manifest, the registry,
and the directory agree exactly in both directions — a new instrument that is
registered but not manifested, or manifested but not on disk, fails there.
`scripts/check_generated_instrument_fixtures.py` (run by `mise run gen-check`)
enforces the same invariant outside cargo.

## Shared helpers

Import fixtures and tolerances from `crate::common::test_helpers`; see
[`common/README.md`](common/README.md) for the full inventory.

```rust
use crate::common::test_helpers::{dates, flat_discount_curve, tolerances};

#[test]
fn bond_prices_near_par_at_coupon_rate() {
    let as_of = dates::TODAY;
    let curve = flat_discount_curve(0.05, as_of, "USD-OIS");
    // ...
    assert!((pv - par).abs() < notional * tolerances::CURVE_PRICING);
}
```

Curve, quote, and option builders shared with the non-instrument test binaries
live in [`../support/`](../support/) and are reachable here as
`crate::test_support::*` (`date`, `rates`, `credit`, `volatility`,
`discount_forward_curves`, `commodity_curves`, `equity_fx_options`,
`calibration`).

## Coverage expectations

A new instrument directory should cover, at minimum:

1. **Construction** — builder happy path, field validation, rejected inputs.
2. **Cashflows** — schedule generation, amortization, product-specific features
   (PIK, floating resets, step-ups).
3. **Pricing** — par, discount, and premium cases against the pricing engine.
4. **Metrics** — one or two tests per metric the instrument registers.
5. **Validation** — zero and extreme inputs, very short and very long maturities,
   negative rates, boundary conditions.

Beyond that, the instrument must appear in `coverage_manifest.toml` with a
generated fixture, or `registry_coverage.rs` fails.

## Expected-value provenance

Every expected number needs documented provenance. Without it a test can pass by
matching incorrect library behavior, and a later fix looks like a regression.

Acceptable sources, in rough order of preference:

```rust
// Mathematical invariant — no external reference needed.
// Put-call parity: C - P = S*e^(-qT) - K*e^(-rT)
let expected_diff = (forward_spot - pv_strike) * contract_size;

// Analytical derivation, spelled out.
// YTM = (100/80)^(1/5) - 1 for a 5Y zero priced at 80
let expected_ytm = (100.0_f64 / 80.0).powf(1.0 / 5.0) - 1.0;

// Round-trip self-test — expected is derived from the input.
// Bootstrap hazard -> reprice the calibrating CDS -> NPV ~ 0
assert!(npv.amount().abs() < 1.0);
```

External vendor references (QuantLib, Bloomberg, ISDA) belong in
[`../golden/`](../golden/README.md), where the fixture schema forces a `source`,
`source_detail`, capture date, and reviewer alongside the number. Do not paste a
bare vendor value into a `#[test]` here.

What not to do: an unexplained constant, a tolerance widened until the test
passes, or an "expected" value computed from the result under test.

When Finstack and a reference genuinely disagree, record the root cause and the
convention difference in a comment next to the assertion, and keep the
Finstack-side value labelled as a regression baseline rather than a parity
target.

## Running

```bash
# Whole instruments target
cargo nextest run -p finstack-quant-valuations --test valuations instruments

# One instrument
cargo nextest run -p finstack-quant-valuations --test valuations instruments::bond::
cargo nextest run -p finstack-quant-valuations --test valuations instruments::irs::
cargo nextest run -p finstack-quant-valuations --test valuations instruments::structured_credit::

# One file or one metric
cargo nextest run -p finstack-quant-valuations --test valuations instruments::bond::pricing::
cargo nextest run -p finstack-quant-valuations --test valuations instruments::bond::metrics::ytm::

# Contract tests only
cargo nextest run -p finstack-quant-valuations --test valuations instruments::registry_coverage::
```

### Slow tests

Long-running cases carry `#[ignore = "slow: covered by mise rust-test-slow"]`
and are skipped by default. They currently live in `cds`, `cds_index`,
`cds_option`, `cds_tranche`, `equity_option`, `exotic_harness`,
`structured_credit`, and rates swaption/cap-floor Monte Carlo paths.
Mark a test slow when it is a large property run, a
multi-scenario loop, a Monte Carlo convergence check, or a calibration
round-trip.

```bash
mise run rust-test-slow   # workspace-wide, ignored tests only
cargo nextest run -p finstack-quant-valuations --test valuations instruments --run-ignored only
```

## Contributing

1. Follow the flat-by-default directory shape; add subdirectories only when the
   file count justifies it.
2. Use `crate::common::test_helpers` and `crate::test_support` rather than
   rebuilding curves inline.
3. Register the instrument in `coverage_manifest.toml` and regenerate its
   fixture with `gen_schemas -- --write`.
4. Give every expected value a provenance comment.
5. Run `mise run rust-lint` and the targeted `cargo nextest run` filter before
   committing.
