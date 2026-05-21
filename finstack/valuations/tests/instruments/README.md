# Instrument Test Suite

Per-instrument tests for `finstack-valuations` pricing, metrics, and JSON fixtures.

## Test Organization Patterns

Each instrument test suite follows a consistent structure:

```
instrument/
├── mod.rs                    # Module organization and documentation
├── construction.rs           # Builder tests, validation
├── cashflows.rs             # Cashflow generation tests
├── pricing.rs               # Core pricing engine tests
├── metrics/                 # Individual metric tests
│   ├── mod.rs
│   ├── dv01.rs
│   ├── theta.rs
│   └── ...
├── validation/              # Edge cases and boundaries
│   ├── mod.rs
│   └── edge_cases.rs
└── integration/             # Multi-metric and scenario tests
    ├── mod.rs
    └── ...
```

## Shared Test Helpers

All tests should use centralized helpers from `common/test_helpers.rs`:

```rust
use crate::instruments::common::test_helpers::{
    flat_discount_curve,    // Create flat discount curve
    flat_hazard_curve,      // Create flat hazard curve
    date,                   // Create test dates (y, m, d)
    usd, eur, gbp,          // Create Money values
    TOLERANCE,              // Standard numerical tolerance
};
```

### Example Usage

```rust
use crate::instruments::common::test_helpers::{date, flat_discount_curve};

#[test]
fn test_bond_pricing() {
    let as_of = date!(2025, 1, 1);
    let disc_curve = flat_discount_curve(0.05, as_of, "USD-OIS");
    // ... rest of test
}
```

## Coverage Expectations

### Minimum Coverage per Instrument

1. **Construction** (3-5 tests)
   - Builder happy path
   - Field validation
   - Edge case validation

2. **Cashflows** (3-5 tests)
   - Basic cashflow generation
   - Amortization/schedules
   - Special features (PIK, floating, etc.)

3. **Pricing** (3-5 tests)
   - Par pricing
   - Discount pricing
   - Premium pricing

4. **Metrics** (1-2 tests per metric)
   - Core metrics (DV01, Theta, YTM, etc.)
   - Instrument-specific metrics

5. **Validation** (3-5 tests)
   - Zero/extreme values
   - Very short/long maturities
   - Negative rates
   - Boundary conditions

## Running Tests

### Run All Instrument Tests

```bash
cargo test --lib instruments
```

### Run Specific Instrument

```bash
cargo test --lib instruments::bond
cargo test --lib instruments::irs
cargo test --lib instruments::term_loan
```

### Run Specific Test File

```bash
cargo test --lib bond::pricing
cargo test --lib term_loan::metrics::ytm
```

### Run With Output

```bash
cargo test --lib instruments -- --nocapture
```

## Test Writing Guidelines

### AAA Pattern

All tests follow Arrange-Act-Assert:

```rust
#[test]
fn test_example() {
    // Arrange: Set up test data
    let bond = Bond::fixed(...);
    let market = MarketContext::new()...;

    // Act: Execute the operation
    let pv = bond.value(&market, as_of)?;

    // Assert: Verify results
    assert!(pv.amount() > 0.0);
}
```

### Naming Conventions

- Test functions: `test_<component>_<scenario>_<expected>`
- Example: `test_ytm_par_bond_matches_coupon`

### Tolerance-Based Assertions

Use appropriate tolerances for floating-point comparisons:

```rust
use crate::instruments::common::test_helpers::TOLERANCE;

assert!((actual - expected).abs() < TOLERANCE);
```

## Tolerance Policy

Tests use standardized tolerances from `common/test_helpers.rs` to ensure consistency
across the test suite. Choose the appropriate tier based on the calculation type:

| Tier | Constant | Value | Use Case |
|------|----------|-------|----------|
| Analytical | `tolerances::ANALYTICAL` | 1e-6 (0.0001%) | Closed-form solutions (put-call parity, zero-coupon YTM) |
| Numerical | `tolerances::NUMERICAL` | 1e-4 (0.01%) | Iterative methods (Newton-Raphson, tree pricing) |
| Curve Pricing | `tolerances::CURVE_PRICING` | 5e-3 (0.5%) | Curve-based valuations with convention differences |
| Relative | `tolerances::RELATIVE` | 1e-2 (1%) | Proportional comparisons, textbook benchmarks |
| Bump vs Analytical | `tolerances::BUMP_VS_ANALYTICAL` | 1.5e-2 (1.5%) | Bump-and-reprice vs analytical approximations (e.g., DV01 vs Duration) |
| Statistical | `tolerances::STATISTICAL` | 2e-2 (2%) | Monte Carlo and statistical tests |

### Usage Example

```rust
use crate::instruments::common::test_helpers::{tolerances, scaled_tolerance};

// For analytical calculations (e.g., put-call parity)
assert!((computed - expected).abs() < tolerances::ANALYTICAL);

// For curve-based pricing with compounding mismatches
assert!((pv - par).abs() < notional * tolerances::CURVE_PRICING);

// For scaled tolerance (property tests)
let tol = scaled_tolerance(1e-4, intrinsic, 0.10);  // 0.01% with 0.10 floor
assert!(price >= intrinsic - tol);
```

### Convention Notes

- **Par bond pricing**: ~0.3% deviation from par is expected due to semi-annual vs
  continuous compounding mismatch between bond cashflows and discount curves.
- **Swaption parity**: Payer - Receiver should match theoretical (Annuity × (F - K) × N)
  within 1% tolerance.
- **CDS par spreads**: Validated against ISDA Standard Model reference values.
- **Options Greeks**: Should satisfy bounds (e.g., call delta ∈ [0, 1]) at all times.

## Instrument Status

| Instrument | Construction | Cashflows | Pricing | Metrics | Validation | Integration |
|------------|--------------|-----------|---------|---------|------------|-------------|
| Bond | ✓ | ✓ | ✓ | ✓ | ✓ | ✓ |
| IRS | ✓ | ✓ | ✓ | ✓ | ✓ | ✓ |
| CDS | ✓ | ✓ | ✓ | ✓ | ✓ | ✓ |
| CapFloor | ✓ | ✓ | ✓ | ✓ | ✓ | ✓ |
| Swaption | ✓ | ✓ | ✓ | ✓ | ✓ | ✓ |
| TermLoan | ✓ | ✓ | ✓ | ✓ | ✓ | ✓ |
| RevolvingCredit | ✓ | ✓ | ✓ | ✓ | ✓ | - |
| EquityOption | ✓ | ✓ | ✓ | ✓ | ✓ | ✓ |
| FxOption | ✓ | ✓ | ✓ | ✓ | ✓ | - |
| StructuredCredit | ✓ | ✓ | ✓ | ✓ | ✓ | ✓ |

## Best Practices

1. **Use shared helpers** - Avoid duplicating curve builders and fixtures
2. **Descriptive names** - Tests should clearly communicate intent
3. **Single focus** - One logical assertion per test
4. **Deterministic** - Fixed seeds, no randomness in non-MC tests
5. **Isolated** - Tests don't depend on each other
6. **Fast** - Keep tests quick; use appropriate tolerance for convergence
7. **Documented** - Complex tests should have explanatory comments
8. **Provenance** - Document where expected values come from (see below)

## Expected Value Provenance

**Critical**: Every expected value in a test must have documented provenance. This prevents
"silent corrections" where tests pass by matching incorrect library behavior.

### Good Examples

```rust
// ✅ Analytical derivation documented
// Expected: YTM = (100/80)^(1/5) - 1 ≈ 4.56% for 5Y zero-coupon at 80
let expected_ytm = (100.0 / 80.0_f64).powf(1.0 / 5.0) - 1.0;
assert!((ytm - expected_ytm).abs() < tolerances::ANALYTICAL);

// ✅ Mathematical invariant (no external reference needed)
// Put-call parity: C - P = S×e^(-qT) - K×e^(-rT)
let expected_diff = (forward_spot - pv_strike) * contract_size;
assert!((actual_diff - expected_diff).abs() < tolerance);

// ✅ External reference with source
// Expected PV from QuantLib 1.34 test_suite/swaption.cpp line 142
let quantlib_ref = 15_449.08;
assert_approx_eq(finstack_pv, quantlib_ref, tolerances::NUMERICAL, "QuantLib parity");

// ✅ Roundtrip test (expected is self-derived)
// Bootstrap hazard curve → reprice CDS → NPV should be ≈ 0
let npv = cds.value(&market, as_of).unwrap();
assert!(npv.amount().abs() < 1.0, "Par spread roundtrip");
```

### Bad Examples

```rust
// ❌ No provenance - where does 17_727.07 come from?
let expected_pv = 17_727.07;
assert_approx_eq(pv, expected_pv, 0.02, "swaption pricing");

// ❌ "Tolerance hack" that masks a bug
let expected = result * 1.001; // Silent correction!
assert!((result - expected).abs() < 0.01);

// ❌ Loose tolerance without explanation
assert!((pv - expected).abs() < 1000.0); // Why 1000?
```

### When External Reference Differs

If finstack produces different values than a reference (QuantLib, Bloomberg), document it:

```rust
// KNOWN DISCREPANCY: QuantLib reference produces 15_449.08
// Finstack implementation produces 17_727.07 (~15% higher)
//
// Root cause analysis:
// 1. Annuity calculation: Finstack uses quarterly fixed leg,
//    QuantLib uses semi-annual (market standard).
// 2. Day count: Finstack uses Act/360; QuantLib uses 30/360.
//
// RECOMMENDATION: Update fixtures to align with market conventions.
//
// Using finstack baseline for regression testing:
expected_pv: 17_727.07, // Finstack empirical baseline (not QuantLib reference)
```

## Special Notes

### Equity DV01

Equity instruments include DV01 metrics despite not having direct interest rate cashflows. This is because:

- Position values are discounted
- Forward pricing uses risk-free rates
- Portfolio-level aggregation mixes equities with fixed income

### Slow Tests

Tests that take longer to run (property-based tests, comprehensive parity checks, multi-scenario
validation) are marked with `#[ignore = "slow"]` so they are skipped by default:

```rust
#[test]
#[ignore = "slow"]
fn test_put_call_parity_atm() {
    // Comprehensive parity test...
}
```

**When to mark a test `#[ignore = "slow"]`:**
- Property-based tests with 50+ cases
- Multi-scenario validation loops
- Parity tests that create multiple instruments per test
- Calibration roundtrip tests
- Tests involving Monte Carlo simulation

**Running slow tests:**

```bash
# Run fast tests only (default CI)
cargo test --lib instruments

# Run all tests including slow (via `cargo test`)
cargo test --lib instruments -- --include-ignored

# Run slow tests only
cargo test --lib instruments -- --ignored
```

## Contributing

When adding new instrument tests:

1. Follow the standard directory structure
2. Use shared helpers from `common/test_helpers.rs`
3. Provide comprehensive coverage (construction, cashflows, pricing, metrics, validation)
4. Update this README with instrument status
5. Run `mise run all-lint` and `mise run rust-test` before committing
