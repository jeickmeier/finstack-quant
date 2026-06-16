# Golden Test Data - finstack-quant-portfolio

golden test fixtures for validating portfolio
attribution and valuation calculations.

## Framework

Tests use the unified `finstack_quant_core::golden` framework:

```rust
use finstack_quant_core::golden::{
    GoldenSuite, Expectation, GoldenAssert,
    load_suite_from_path, assert_range,
};
```

## Directory Structure

```
golden/
├── README.md           # This file
├── mod.rs              # Test module
└── data/
    └── attribution.json    # P&L attribution golden tests
```

## Suites

### Attribution (`data/attribution.json`)

Tests for portfolio P&L attribution:
- **Rates P&L**: Interest rate sensitivity (direction + magnitude)
- **FX Translation P&L**: Currency translation effects
- **Carry/Theta**: Time value and accrual

**Expectation Types**:
- Range constraints for directional validation
- Magnitude bounds for reasonableness checks

## Expectation Patterns

Portfolio tests primarily use **range expectations** since exact P&L values
depend on many factors (curve construction, day count, etc.):

```json
{
  "rates_pnl_direction": {
    "min": null,
    "max": 0.0,
    "notes": "Positive rate shock -> negative P&L for long bond"
  }
}
```

For exact value tests, use the standard tolerance format:

```json
{
  "value": 100.0,
  "tolerance_abs": 0.01
}
```

## Adding New Fixtures

1. Create JSON file in `data/` with canonical structure
2. Use range expectations for directional/magnitude tests
3. Use exact expectations only when values are stable
4. Include detailed notes explaining the expected behavior
