# finstack-quant-test-utils

Golden-fixture loading and comparison helpers shared by finstack-quant test
suites.

Directory: `finstack-quant/test-utils`. Package / import name:
`finstack-quant-test-utils` / `finstack_quant_test_utils`.

A golden suite is a JSON file pairing externally sourced reference values
(QuantLib, ISDA, Bloomberg, a vetted script) with provenance metadata and
explicit comparison tolerances. This crate owns the envelope format, the
loader, and the assertion helpers so that shape and provenance rules live in
one place instead of being re-implemented per crate.

## Position in the workspace

This is a dev-only supporting crate, not one of the 14 domain crates:

- **not** re-exported by the `finstack-quant` umbrella crate, and **not** part
  of the published API surface
- depends only on `serde`, `serde_json`, and `thiserror` — no finstack crates,
  so it cannot be pulled into a production dependency graph
- add it as a **`[dev-dependencies]`** entry only; today
  `finstack-quant-core` is the only crate that does

It is not the only golden harness in the workspace. `finstack-quant-valuations`
has its own pricing/attribution golden runner under
[`valuations/tests/golden/`](../valuations/tests/golden/README.md) (driven by
`mise run goldens-test`), which does not use this crate.

## Dependency

```toml
[dev-dependencies]
finstack-quant-test-utils = { path = "../test-utils", version = "0.9.0" }
```

## Public surface

Everything lives under `finstack_quant_test_utils::golden`, except the
`golden_path!` macro which is exported at the crate root.

| Item | Role |
|------|------|
| `GoldenSuite<T>` | Fixture envelope: `meta` + `cases: Vec<T>` |
| `SuiteMeta` | Suite id, description, provenance, status, `schema_version` |
| `ReferenceSource` | Name / version / vendor / url of the reference values |
| `GeneratedInfo` | `at`, `by`, optional `command` and `environment` |
| `ValidatedInfo` | Optional record of who checked the values, how, and when |
| `Expectation` | `Exact { value, tolerance?, notes? }` or `Range { min?, max?, notes? }` |
| `Tolerance` | `Abs`, `Rel`, `Bps`, `Pct` |
| `load_suite_from_path` | Read + parse a fixture file into `GoldenSuite<T>` |
| `load_suite_from_str` | Parse a fixture from an in-memory JSON string |
| `golden_path` | `(manifest_dir, relative)` → `<crate>/tests/golden/<relative>` |
| `golden_path!` | Macro wrapper that supplies `env!("CARGO_MANIFEST_DIR")` |
| `GoldenAssert` | Assertion context bound to a `SuiteMeta` and case id |

All fallible helpers return `Result<T, finstack_quant_test_utils::Error>`, where
`Error` is `#[non_exhaustive]` and today has a single `Validation(String)`
variant — match it with a wildcard arm. Assertions return errors rather than
panicking, so a caller decides whether to accumulate failures or panic on the
first one.

`Expectation` and `Tolerance` also carry predicates not listed above
(`Expectation::is_satisfied`; `Tolerance::is_within`, `compute_error`), useful
when a test builds an expectation in code instead of reading one from a
fixture — construct the enum variants directly (e.g. `Expectation::Exact {
value, tolerance: Some(Tolerance::Abs(tol)), notes: None }`), or use the
`assert` module's `approx_eq`/`relative_eq`/`in_range` helpers for a
lighter-weight panicking check. Full API detail is in the rustdoc.

## Fixture format

Fixtures use one canonical v1 envelope. Arrays and bare objects are rejected.

```json
{
  "meta": {
    "suite_id": "realized_variance",
    "description": "Realized variance estimators",
    "reference_source": { "name": "QuantLib", "version": "1.32" },
    "generated": {
      "at": "2026-08-02T12:00:00Z",
      "by": "gen_vol_golden.py",
      "command": "uv run python gen_vol_golden.py"
    },
    "status": "certified",
    "schema_version": 1
  },
  "cases": [
    {
      "id": "case_1",
      "inputs": { "...": "..." },
      "expected": { "value": 0.0412, "tolerance": { "type": "bp", "value": 0.5 } }
    }
  ]
}
```

`cases` is decoded as the caller's own case type, so each suite defines its own
input/expected shape. `meta.extra` (and `ReferenceSource.extra`) carry
suite-specific metadata without changing the envelope.

### Strictness

Deserialization is deliberately unforgiving so a malformed fixture fails loudly
instead of silently weakening a test:

- `meta.schema_version` is **required** and must be `1`.
- `GoldenSuite`, `SuiteMeta`, `Tolerance`, and `Expectation` deny unknown
  fields.
- `Expectation` is untagged, so exact-shaped and range-shaped fixtures cannot be
  mixed in one entry (`{"value":…, "min":…}` is rejected).
- Legacy flat tolerance keys (`tolerance_abs`, `tolerance_rel`, `tolerance_bp`,
  `tolerance_pct`) are rejected; use the tagged `tolerance` object.
- Top-level arrays and single-case objects are rejected.

### Tolerances

| Wire `type` | Rust variant | Meaning |
|-------------|--------------|---------|
| `abs` | `Tolerance::Abs(t)` | `abs(actual - expected) <= t` |
| `rel` | `Tolerance::Rel(t)` | `abs((actual - expected) / expected) <= t` (fraction) |
| `bp` | `Tolerance::Bps(t)` | `abs(actual - expected) * 10_000 <= t` |
| `pct` | `Tolerance::Pct(t)` | relative error expressed in percent `<= t` |

`bp` is an **absolute** difference scaled by 10 000, which is what you want for
rate-like quantities held as decimals: 0.05004 against 0.05000 is 0.4 bp. It is
not a relative measure, so it is the wrong unit for price-scale quantities.

For `rel` and `pct`, an expected value below `1e-15` in magnitude falls back to
comparing `abs(actual)` against the tolerance directly, avoiding a
divide-by-zero.

Omitting `tolerance` on an `Exact` expectation means scale-aware exact
comparison: `abs(actual - expected) <= max(abs(expected) * f64::EPSILON * 8, 1e-15)`.

### Provenance

Every fixture should record where its numbers came from, because that is the
only thing that makes a mismatch actionable:

- `meta.reference_source.name` — source of the expected values
- `meta.generated.at` / `meta.generated.by` — when and by what
- `meta.generated.command` — the exact regeneration command, when one exists
- `meta.status` — `certified`, `provisional`, or `pending_validation`

`status` defaults to `"unknown"` when absent; a suite worth trusting sets it.
See [`.agents/rules/rust/testing-standards.md`](../../.agents/rules/rust/testing-standards.md)
for the workspace policy on golden data.

## Writing a golden test

Fixtures live in `<crate>/tests/golden/data/`; the test module lives in
`<crate>/tests/golden/`. `golden_path!` resolves against that layout.

```rust
use finstack_quant_core::math::stats::{realized_variance_ohlc, RealizedVarMethod};
use finstack_quant_test_utils::golden::{load_suite_from_path, Expectation, GoldenAssert};
use finstack_quant_test_utils::golden_path;
use serde::Deserialize;

#[derive(Debug, Deserialize)]
struct VarianceInputs {
    open: Vec<f64>,
    high: Vec<f64>,
    low: Vec<f64>,
    close: Vec<f64>,
    annualization_factor: f64,
    method: String,
}

#[derive(Debug, Deserialize)]
struct VarianceExpected {
    annualized_variance: Expectation,
}

#[derive(Debug, Deserialize)]
struct VarianceCase {
    id: String,
    inputs: VarianceInputs,
    expected: VarianceExpected,
}

fn method_from_str(s: &str) -> RealizedVarMethod {
    match s.to_lowercase().as_str() {
        "parkinson" => RealizedVarMethod::Parkinson,
        "garman_klass" => RealizedVarMethod::GarmanKlass,
        _ => RealizedVarMethod::CloseToClose,
    }
}

#[test]
fn realized_variance_matches_golden() {
    let path = golden_path!("data/realized_variance.json");
    let suite = load_suite_from_path::<VarianceCase>(&path)
        .expect("should load realized_variance.json");
    assert!(!suite.cases.is_empty(), "suite should have cases");

    for case in &suite.cases {
        let actual = realized_variance_ohlc(
            &case.inputs.open,
            &case.inputs.high,
            &case.inputs.low,
            &case.inputs.close,
            method_from_str(&case.inputs.method),
            case.inputs.annualization_factor,
        )
        .expect("estimator should succeed for certified cases");

        GoldenAssert::new(&suite.meta, &case.id)
            .expected("annualized_variance", actual, &case.expected.annualized_variance)
            .unwrap_or_else(|e| panic!("{e}"));
    }
}
```

Assertion failures carry the suite id, case id, metric name, observed value,
tolerance, and computed error, e.g.
`[realized_variance/case_1] annualized_variance failed: actual=…, expected=…, tolerance=Bps(0.5), error=…`.

A second test asserting on `suite.meta` (suite id, non-empty
`reference_source.name`, non-empty `generated.at`/`generated.by`, expected
`status`) keeps provenance from rotting away — see
[`variance_tests.rs`](../core/tests/golden/variance_tests.rs).

`GoldenAssert::expected(metric, actual, &Expectation::Exact { value, tolerance: Some(Tolerance::Abs(tol)), notes: None })`
is the shortcut when the tolerance is hard-coded in the test rather than
carried in the fixture; `assert::approx_eq`/`relative_eq`/`in_range` are
lighter-weight, panicking alternatives for tests that don't need suite/case
provenance in the failure message.

## Regenerating fixtures

Fixture data is committed. Regeneration is per-suite and belongs with the
fixture, not in this crate. Record the command in `meta.generated.command` and
document the conventions next to the generator. Two existing patterns:

- A generator script sitting beside the data, e.g.
  [`gen_vol_golden.py`](../core/tests/golden/data/gen_vol_golden.py), run with
  `uv run --with QuantLib --with mpmath python gen_vol_golden.py`.
- A workspace task, for the valuations/Python golden layers:
  `mise run goldens-quantlib-generate` to regenerate and
  `mise run goldens-quantlib-check` to detect generator drift. Neither touches
  fixtures loaded through this crate.

When a vendor or library version changes, regenerate, bump
`reference_source.version` and `generated.at`, and note the rationale in the
suite README. Do not widen a tolerance to make a regenerated fixture pass —
see [INVARIANTS.md](../../INVARIANTS.md) and the testing standards.

## Verification

```bash
cargo nextest run -p finstack-quant-test-utils
cargo nextest run -p finstack-quant-core --test golden_tests
cargo clippy -p finstack-quant-test-utils --lib --bins --tests --examples --all-features -- -D warnings
```

Or the whole Rust layer: `mise run rust-test` and `mise run rust-lint`.

## License

Dual-licensed under MIT or Apache-2.0.
