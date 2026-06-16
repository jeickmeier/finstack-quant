# Golden Test Data - finstack-quant-scenarios

golden test fixtures for validating scenario engine
implementations against known reference values.

## Framework

Tests use the unified `finstack_quant_core::golden` framework:

```rust
use finstack_quant_core::golden::{
    GoldenSuite, ExpectedValue, GoldenAssert, Expectation,
    load_suite_from_path, assert_expected_value,
};
```

## Directory Structure

```
golden/
├── README.md           # This file
├── mod.rs              # Test module
└── data/
    └── curve_shocks.json   # Curve shock golden tests
```

## Suites

### Curve Shocks (`data/curve_shocks.json`)

Tests for parallel and tenor point shocks on discount curves:
- Parallel up/down shocks
- Direction validation (positive shock -> lower DF)
- Shock magnitude validation

**Tolerance**: 1e-10 (floating point precision)

## Adding New Fixtures

1. Create JSON file in `data/` with canonical structure:

```json
{
  "meta": {
    "suite_id": "unique_id",
    "reference_source": { "name": "..." },
    "generated": { "at": "...", "by": "..." },
    "status": "certified"
  },
  "cases": [...]
}
```

2. Include both exact (`value` + `tolerance`) and range (`min`/`max`) expectations

3. Add tests in `mod.rs` that load and validate the fixture

## Expectation Types

- **Exact**: Use for precise numerical validation with tolerance

```json
{ "value": 0.95, "tolerance_abs": 1e-10 }
```

- **Range**: Use for directional or bounds validation

```json
{ "min": 0.0, "max": 1.0 }
```
