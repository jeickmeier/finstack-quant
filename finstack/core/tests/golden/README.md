# Golden test data

Reference fixtures for validating finstack-core implementations against known
values.

## Layout

```
golden/
├── README.md
├── mod.rs
├── daycount_quantlib_tests.rs
├── variance_tests.rs
└── data/
    ├── daycount_quantlib.json
    └── realized_variance.json
```

## Fixture format

```json
{
  "meta": {
    "suite_id": "unique_id",
    "description": "What this suite tests",
    "reference_source": {
      "name": "Source name",
      "version": "1.0",
      "vendor": "Organization",
      "url": "https://..."
    },
    "generated": {
      "at": "2025-01-26T00:00:00Z",
      "by": "tool or person"
    },
    "status": "certified",
    "schema_version": 1
  },
  "cases": []
}
```

## Suites

### `daycount_quantlib.json`

Day-count conventions validated against QuantLib output:

- 30/360 US (Bond Basis) — ISDA 2006 §4.16(f)
- 30E/360 (Eurobond) — ISDA 2006 §4.16(g)
- Act/Act ISDA — year-boundary splitting
- Act/365L (AFB) — Feb 29 detection

References: QuantLib 1.32; ISDA 2006 Definitions §4.16.

### `realized_variance.json`

Realized variance estimators:

- Parkinson (1980) — high-low range
- Garman-Klass (1980) — OHLC

## Adding fixtures

1. Add JSON under `data/` with full `meta` provenance
2. Set `status` to `"provisional"` until validated
3. Document the suite here
4. Set `status` to `"certified"` after review

## Using the golden framework

Golden loading and assertions live in `finstack-test-utils`:

```rust
use finstack_test_utils::golden::{load_suite_from_path, GoldenAssert};
use finstack_test_utils::golden_path;

#[test]
fn test_my_feature() {
    let path = golden_path!("data/my_suite.json");
    let suite = load_suite_from_path::<MyCase>(&path).expect("load suite");

    for case in &suite.cases {
        let actual = compute_something(&case.inputs);
        let assert = GoldenAssert::new(&suite.meta, &case.id);
        assert
            .abs("metric", actual, case.expected.value, case.expected.tolerance)
            .unwrap();
    }
}
```

See `finstack/core/tests/golden/daycount_quantlib_tests.rs` for a working example.
