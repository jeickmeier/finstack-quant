# Common Module Test Suite

This directory contains comprehensive unit tests for the `instruments/common` module, organized following industry best practices and the AAA (Arrange-Act-Assert) pattern.

## Test Structure

```
common/
├── mod.rs                      # Test module organization
├── test_helpers.rs             # Shared fixtures and utilities
├── test_traits.rs              # Core trait behavior tests (CashflowProvider, Priceable)
├── helpers/                    # Helper utilities
├── models/                     # Option pricing model tests
│   ├── test_binomial_tree.rs  # Comprehensive binomial tree tests (90+ tests)
│   ├── test_trinomial_tree.rs # Trinomial tree tests
│   ├── test_black.rs           # Black-Scholes/Black76 formula tests (40+ tests)
│   ├── test_sabr.rs            # SABR volatility model tests (40+ tests)
│   ├── test_short_rate_tree.rs # Short-rate tree tests
│   ├── test_tree_framework.rs  # Generic tree framework tests
│   └── test_two_factor_trees.rs # Two-factor model tests
├── metrics/                    # Risk metrics tests
│   └── test_theta_utils.rs    # Theta calculation tests (25+ tests)
├── parameters/                 # Parameter type tests
│   └── test_conventions.rs    # Market conventions tests
├── test_discountable.rs       # Discountable trait tests
└── test_pricing.rs            # Generic pricing tests
```

## Coverage Overview

### Models (200+ tests)

#### Binomial Tree (`test_binomial_tree.rs`)

- **Parameter Calculation** (10 tests): CRR, Leisen-Reimer, validation, edge cases
- **European Options** (15 tests): Convergence, put-call parity, ITM/OTM/ATM pricing
- **American Options** (5 tests): Early exercise premium, bounds checking
- **Bermudan Options** (5 tests): Exercise scheduling, boundary cases
- **Barrier Options** (10 tests): Knock-in/out, in-out parity, rebates
- **Greeks** (8 tests): Delta, gamma, theta calculations
- **Edge Cases** (10+ tests): Deep ITM/OTM, extreme volatility, short/long maturity

#### Black Formulas (`test_black.rs`)

- **d1/d2 Calculations** (10 tests): ATM, ITM, OTM, with dividends
- **Black76** (8 tests): Forward-based pricing, equivalence with BS
- **Black-Scholes** (15 tests): Put-call parity, monotonicity, symmetry
- **Numerical Stability** (10+ tests): Extreme values, negative rates

#### SABR Model (`test_sabr.rs`)

- **Parameter Validation** (10 tests): Alpha, beta, nu, rho bounds
- **ATM Volatility** (5 tests): Normal, lognormal, consistency
- **Implied Volatility** (12 tests): Smile generation, skew, term structure
- **Numerical Stability** (10 tests): ATM detection, chi function, extreme parameters
- **Shifted SABR** (5 tests): Negative rates, validation
- **Calibration** (5 tests): Basic, fit quality, auto-shift

### Metrics (25+ tests)

#### Theta Utils (`test_theta_utils.rs`)

- **Period Parsing** (12 tests): Days, weeks, months, years, edge cases
- **Date Calculations** (8 tests): Rolling forward, expiry caps, various periods
- **Integration** (5 tests): Short/long dated options, workflows

### Core Traits (7 tests)

#### Trait Behavior Tests (`test_traits.rs`)

- **CashflowProvider** (2 tests): Schedule building, NPV calculation with discount curves
- **Priceable** (5 tests): Value extraction, metrics computation, result stamping

Tests use mock implementations to validate trait contracts in isolation from real instruments.

### Test Utilities (`test_helpers.rs`)

Provides shared fixtures and utilities:

- **Comparison functions**: `assert_approx_eq`, `assert_relative_eq`, `assert_money_eq`
- **Standard tolerances**: `TOLERANCE`, `TIGHT_TOLERANCE`, `RELATIVE_TOLERANCE`
- **Market fixtures**: `standard_market()`, `flat_curve()`, `upward_curve()`
- **Black-Scholes reference**: `black_scholes_call()`, `black_scholes_put()`
- **Date utilities**: `test_date()`, `year_fraction()`

## Testing Methodology

### AAA Pattern

All tests follow the Arrange-Act-Assert pattern:

```rust
#[test]
fn test_example() {
    // Arrange: Set up test data
    let params = OptionMarketParams::call(100.0, 100.0, 0.05, 0.20, 1.0);

    // Act: Execute the operation
    let price = tree.price_european(&params).unwrap();

    // Assert: Verify results
    assert_approx_eq(price, expected, TOLERANCE, "Price matches");
}
```

### Coverage Goals

- **Critical paths**: All main pricing algorithms
- **Edge cases**: Extreme parameters, boundary conditions
- **Numerical stability**: Very small/large values
- **Mathematical properties**: Put-call parity, monotonicity, bounds
- **Market standards**: Industry conventions, standard test cases

### Validation Techniques

1. **Convergence tests**: Tree models → analytical solutions
2. **Parity relationships**: Put-call, in-out barriers
3. **Bounds checking**: Physical constraints (0 ≤ option ≤ spot)
4. **Monotonicity**: Price increases with spot, vol, time
5. **Numerical stability**: Finite results for extreme inputs

## Running Tests

```bash
# Run all common module tests
cargo test --lib common

# Run specific test file
cargo test --lib test_binomial_tree

# Run specific test
cargo test --lib test_crr_european_converges_to_black_scholes

# Run with output
cargo test --lib common -- --nocapture
```

## Test Metrics

- **Total tests**: 230+ comprehensive unit tests
- **Expected coverage**: >80% for critical paths
- **Test categories**:
  - Functionality: 60%
  - Edge cases: 20%
  - Numerical stability: 10%
  - Integration: 10%

## Best Practices

1. **Descriptive names**: `test_american_put_early_exercise_premium`
2. **Single assertion focus**: One logical concept per test
3. **Tolerance-based comparisons**: Appropriate for floating-point math
4. **Shared fixtures**: Reduce duplication via `test_helpers`
5. **Parameterized patterns**: Similar tests with different inputs
6. **Documentation**: Clear comments explaining test purpose
