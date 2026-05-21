# Golden test data

Reference fixtures that validate `finstack-statements` output against external tools.

Tests use `finstack_core::golden` (`GoldenSuite`, `ExpectedValue`, `GoldenAssert`, `load_suite_from_path`, `assert_abs`).

## Layout

```
golden/
├── golden_tests.rs          # Model evaluation
├── golden_parity.rs         # External parity
├── basic_model.json         # Model spec fixture
├── basic_model_results.json # Expected results
├── data/
│   └── excel_npv_scenarios.json
├── excel/                   # Legacy CSV (NPV)
└── pandas/                  # Legacy CSV (rolling/EWM)
```

## Suites

**Model evaluation** (`basic_model.json`) — serialization, node evaluation, period handling.

**Excel NPV** (`data/excel_npv_scenarios.json`) — NPV vs Microsoft Excel 365 (16.80), tolerance `0.01`.

**pandas rolling/EWM** (`pandas/*.csv`) — rolling and EWM vs pandas 2.1.3, tolerance `1e-10`.

## Fixture metadata

JSON fixtures should include:

- `meta.suite_id`
- `meta.reference_source.name`
- `meta.generated.at` and `meta.generated.by`
- `meta.status` — `"certified"` after validation, `"provisional"` before

## Tolerances

| Source | Tolerance | Notes |
|--------|-----------|-------|
| Excel | `1e-8` | double precision |
| pandas | `1e-10` | float64 |
| Statistical | `1e-3` | algorithm differences |
| Accounting | `0.01` | two decimal places |

## Adding fixtures

1. Add JSON under `data/` with full `meta` provenance.
2. Validate against the reference source.
3. Set `meta.status` to `"certified"`.
4. Document the suite here.
